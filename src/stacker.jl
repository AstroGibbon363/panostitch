# Star stacking core (the GUI is stacker_gui.jl), after Sequator:
#
#   calibration masters (bias, darks, flats, flat darks) → every star image:
#   load linear, calibrate, fix hot pixels, detect stars in the sky region →
#   align to the base image (triangle matching + RANSAC, optionally with lens
#   distortion and a smooth correction grid) → score and reject frames →
#   combine (average, sigma clipping, freeze ground, star trails; optionally
#   every N frames for a time-lapse) → post-processing (`postprocess`, can be
#   redone on the kept linear stack): background extraction, colour
#   calibration, light pollution, star enhancement → brightness, HDR, tone
#   (sRGB curve, histogram stretch or linear; sRGB or Adobe RGB) → save.
#
# The frames are kept as Float16, in RAM or (if they do not fit) in a temporary
# file next to the output: deleted at once on Linux / macOS (the space is freed
# when done), after the run on Windows (which cannot delete a mapped file).

module Stacker

using ColorTypes, LinearAlgebra, Mmap, Statistics
import ..Stitcher as St
using ..Stitcher: Img

export stack

const PREVIEW_PX = 1600          # longest side in the quick preview
const PREVIEW_FRAMES = 10        # star images the quick preview uses
const PREVIEW_CALIB = 5          # calibration frames per kind in the quick preview
const MAX_STARS = 500            # stars kept per frame (brightest)
const ALIGN_STARS = 50           # brightest stars used for triangle matching
const DETECT_SIGMA = 5.0         # star detection threshold (× background noise)

"""Defaults for every setting; `stack` fills in what a configuration leaves out."""
const DEFAULTS = (
    stars = String[], base = nothing, darks = String[], bias = String[], flats = String[],
    flatdarks = String[], output = "", excluded = String[],
    raw = (wb = :camera, quality = 3, half = false),
    unify_exposure = false, homogenize_vignetting = false,
    composition = "Align stars", align_mode = "Select best pixels", selective = false,
    motion_effect = false, model = :homography, interpolation = "Lanczos",
    sigma = (low = 3.0, high = 3.0, iters = 3),
    reject = (auto = true, min_light = 0.6, min_stars = 0.5, max_fwhm = 1.5, max_elong = 1.3),
    sky = (mode = "None", line = nothing, gradient = 0.1, mask = falses(0, 0)),
    auto_brightness = true, hdr = false, dynamic_noise = false, merge4 = false,
    color_space = "sRGB", tone = "sRGB curve", stretch = (auto = true, black = 0.0, mid = 0.25, white = 1.0),
    bg_extraction = (on = false, degree = 2), color_calibration = false,
    color_noise = (on = false, strength = 50), distortion = "Off",
    light_pollution = (filter = "Off", strength = 50, iaa = false, degree = 2, tiles = 20),
    enhance_stars = (on = false, strength = 50),
    timelapse = (on = false, frames = 10),
)

"""
Base pixel → frame pixel: a homography H, optionally with radial lens
distortion k1 (the same for all frames; r relative to the half diagonal R,
around c) and a smooth correction grid (frame pixels at nodes `cell` apart).
"""
struct Warp
    H::Matrix{Float64}
    Hi::Matrix{Float64}
    k1::Float64
    c::NTuple{2,Float64}
    R::Float64
    grid::Array{Float64,3}            # (2, ny, nx); empty = no correction
    cell::Float64
end
Warp(H; k1 = 0.0, c = (0.0, 0.0), R = 1.0, grid = zeros(2, 0, 0), cell = 1.0) =
    Warp(H, inv(H), k1, c, R, grid, cell)

struct Star
    x::Float64
    y::Float64
    flux::Float64
    fwhm::Float64
    elong::Float64
    peak::Float64
end

"""What happened to one star image (shown in the frame report)."""
Base.@kwdef mutable struct FrameInfo
    path::String
    stars::Int = 0
    fwhm::Float64 = NaN               # median star size (px)
    elong::Float64 = NaN              # median elongation (1 = round)
    background::Float64 = NaN         # median sky brightness (linear)
    light::Float64 = NaN              # brightness of the brightest stars, relative to the base image
    matched::Int = 0                  # stars matched to the base image
    err::Float64 = NaN                # alignment error (RMS px)
    status::Symbol = :pending         # :base, :used, :rejected, :failed, :excluded, :skipped
    reason::String = ""
    T::Matrix{Float64} = Matrix{Float64}(I, 3, 3)    # base pixel → frame pixel (homography)
    warp::Warp = Warp(Matrix{Float64}(I, 3, 3))       # … with the distortion correction
    level::NTuple{3,Float32} = (0f0, 0f0, 0f0)        # median sky level per channel
    gain::NTuple{3,Float32} = (1f0, 1f0, 1f0)         # exposure unification
    detected::Vector{Star} = Star[]
end

used(fi) = fi.status in (:base, :used)

# --------------------------------------------------------------------------
# Loading
# --------------------------------------------------------------------------

srgb_to_linear(c) = c <= 0.04045f0 ? c / 12.92f0 : ((c + 0.055f0) / 1.055f0)^2.4f0
linear_to_srgb(c) = c <= 0.0031308f0 ? 12.92f0 * c : 1.055f0 * c^(1 / 2.4f0) - 0.055f0

"""
Linear RGB data of a file: RAW decoded without tone curve, JPEG/PNG converted
from sRGB, TIFF, FITS and XISF taken as they are (they should be linear already).
"""
function load_linear(path, cfg; preview = false)
    ext = lowercase(splitext(path)[2])
    img = St.load_any(path; raw_half = cfg.raw.half || preview, linear = true, wb = cfg.raw.wb,
                      quality = cfg.raw.quality)
    if ext in (".jpg", ".jpeg", ".png", ".bmp")
        img = map(c -> RGB{Float32}(srgb_to_linear(red(c)), srgb_to_linear(green(c)),
                                    srgb_to_linear(blue(c))), img)
    end
    if preview
        f = PREVIEW_PX / maximum(size(img))
        f < 1 && (img = St.resize(img, f))
    end
    return img
end

# --------------------------------------------------------------------------
# Frame storage: Float16 (3, width, height, frames), in RAM or memory-mapped
# --------------------------------------------------------------------------

const TEMP_FILES = String[]             # Windows: frame files to delete after the run
const TEMP_LOCK = ReentrantLock()

function framestore(h, w, K, dir)
    # macOS counts the file cache as used, so its "free" memory is misleadingly small.
    avail = Sys.isapple() ? 0.4 * Sys.total_memory() : Float64(Sys.free_memory())
    6.0 * h * w * K < 0.5 * avail && return Array{Float16,4}(undef, 3, w, h, K)
    path = joinpath(isdir(dir) ? dir : tempdir(), "panostitch-frames-$(getpid())-$(rand(UInt32)).tmp")
    A = open(path, "w+") do io
        Mmap.mmap(io, Array{Float16,4}, (3, w, h, K))
    end
    if Sys.iswindows()
        lock(() -> push!(TEMP_FILES, path), TEMP_LOCK)
    else
        rm(path; force = true)       # the mapping stays valid; the space is freed when done
    end
    return A
end

"""Delete the frame files of finished runs (Windows): their mappings go with the garbage collection."""
function remove_temp_files()
    lock(TEMP_LOCK) do
        isempty(TEMP_FILES) && return
        GC.gc()
        filter!(TEMP_FILES) do p
            try
                rm(p; force = true)
                false
            catch
                true                     # still mapped somewhere: try again after the next run
            end
        end
    end
end

function store!(D, k, img)
    h, w = size(img)
    Threads.@threads for y in 1:h
        for x in 1:w
            c = img[y, x]
            D[1, x, y, k] = red(c); D[2, x, y, k] = green(c); D[3, x, y, k] = blue(c)
        end
    end
end

# Bilinear sample of frame k at pixel position (x, y); nothing outside the frame.
@inline function sample(D, k, x, y, w, h)
    (0.5 <= x <= w + 0.5 && 0.5 <= y <= h + 0.5) || return nothing
    x0 = clamp(floor(Int, x), 1, max(w - 1, 1)); y0 = clamp(floor(Int, y), 1, max(h - 1, 1))
    x1 = min(x0 + 1, w); y1 = min(y0 + 1, h)
    fx = clamp(Float32(x - x0), 0f0, 1f0); fy = clamp(Float32(y - y0), 0f0, 1f0)
    return ntuple(Val(3)) do c
        a = (1 - fx) * Float32(D[c, x0, y0, k]) + fx * Float32(D[c, x1, y0, k])
        b = (1 - fx) * Float32(D[c, x0, y1, k]) + fx * Float32(D[c, x1, y1, k])
        (1 - fy) * a + fy * b
    end
end

# Lanczos-3 kernel, tabulated.
const LANCZOS = [(x = 3 * (i - 1) / 1023; x == 0 ? 1f0 : Float32(3 * sinpi(x) * sinpi(x / 3) / (π^2 * x^2))) for i in 1:1024]
@inline lanczos(t) = (a = abs(t); a >= 3 ? 0f0 : @inbounds LANCZOS[1 + round(Int, a * 341)])

"""
Lanczos-3 sample (sharper than bilinear, keeps the noise grain fine). The
result is clamped to the four nearest pixels, so bright stars get no dark rings.
"""
@inline function sample_lanczos(D, k, x, y, w, h)
    (0.5 <= x <= w + 0.5 && 0.5 <= y <= h + 0.5) || return nothing
    x0, y0 = floor(Int, x), floor(Int, y)
    (3 <= x0 <= w - 3 && 3 <= y0 <= h - 3) || return sample(D, k, x, y, w, h)
    fx, fy = Float32(x - x0), Float32(y - y0)
    wx = ntuple(i -> lanczos(fx - (i - 3)), Val(6)); wy = ntuple(i -> lanczos(fy - (i - 3)), Val(6))
    sx, sy = sum(wx), sum(wy)
    return ntuple(Val(3)) do c
        acc = 0f0
        @inbounds for j in 1:6
            row = 0f0
            for i in 1:6
                row += wx[i] * Float32(D[c, x0+i-3, y0+j-3, k])
            end
            acc += wy[j] * row
        end
        acc /= sx * sy
        @inbounds a, b, d, e = Float32(D[c, x0, y0, k]), Float32(D[c, x0+1, y0, k]),
                               Float32(D[c, x0, y0+1, k]), Float32(D[c, x0+1, y0+1, k])
        clamp(acc, min(a, b, d, e), max(a, b, d, e))
    end
end

"""Combine frames 1:K of a store pixel by pixel with `f(values)` per channel."""
function reduce_store(D, K, f)
    _, w, h, _ = size(D)
    out = Img(undef, h, w)
    Threads.@threads for rows in collect(Iterators.partition(1:h, 8))
        v = Vector{Float32}(undef, K)
        for y in rows, x in 1:w
            c = ntuple(Val(3)) do ch
                for k in 1:K
                    v[k] = D[ch, x, y, k]
                end
                f(v)
            end
            out[y, x] = RGB{Float32}(c...)
        end
    end
    return out
end

# --------------------------------------------------------------------------
# Calibration
# --------------------------------------------------------------------------

"""Median of several frames (nothing if there are none)."""
function master(paths, load, dir; progress, what)
    isempty(paths) && return nothing
    first_ = load(paths[1])
    h, w = size(first_)
    D = framestore(h, w, length(paths), dir)
    for (k, p) in enumerate(paths)
        progress("Master $what: $k/$(length(paths)) $(basename(p))", nothing)
        img = k == 1 ? first_ : load(p)
        size(img) == (h, w) || error("$(basename(p)) is $(size(img, 2))×$(size(img, 1)), not $(w)×$(h).")
        store!(D, k, img)
    end
    return reduce_store(D, length(paths), median!)
end

struct Calibration
    dark::Union{Nothing,Img}       # subtracted from star images (dark, or bias without darks)
    flat::Union{Nothing,Img}       # star images are divided by it (mean 1)
    hot::Vector{CartesianIndex{2}} # hot pixels found in the master dark
end

function calibration(cfg, load, dir; progress)
    bias = master(cfg.bias, load, dir; progress, what = "bias")
    dark = master(cfg.darks, load, dir; progress, what = "dark")
    flat = master(cfg.flats, load, dir; progress, what = "flat")
    fdark = master(cfg.flatdarks, load, dir; progress, what = "flat dark")
    dark === nothing && (dark = bias)
    if flat !== nothing
        off = fdark !== nothing ? fdark : bias
        off !== nothing && (flat = flat .- off)
        cfg.homogenize_vignetting && (flat = homogenize(flat))
        h, w = size(flat)
        mid = flat[h÷4+1:3h÷4, w÷4+1:3w÷4]
        m = (mean(red, mid), mean(green, mid), mean(blue, mid))
        flat = map(c -> RGB{Float32}(max(red(c) / m[1], 0.05f0), max(green(c) / m[2], 0.05f0),
                                     max(blue(c) / m[3], 0.05f0)), flat)
    end
    hot = CartesianIndex{2}[]
    if !isempty(cfg.darks)
        L = St.luminance(dark)
        v = vec(L)[1:max(1, length(L) ÷ 200_000):end]
        med = median(v); σ = 1.4826 * median(abs.(v .- med))
        hot = findall(>(med + max(10σ, 0.002f0)), L)
    end
    return Calibration(dark, flat, hot)
end

"""Vignetting averaged over the four mirrored quadrants and smoothed (for uneven flats)."""
function homogenize(F)
    S = (F .+ reverse(F; dims = 1) .+ reverse(F; dims = 2) .+ reverse(F; dims = (1, 2))) ./ 4
    return St.blur(S, 0.01 * maximum(size(F)))
end

function calibrate!(img, cal)
    if cal.dark !== nothing
        size(cal.dark) == size(img) || error("The noise / bias images do not have the size of the star images.")
        img .-= cal.dark
    end
    if cal.flat !== nothing
        size(cal.flat) == size(img) || error("The vignetting images do not have the size of the star images.")
        img .= map((c, f) -> RGB{Float32}(red(c) / red(f), green(c) / green(f), blue(c) / blue(f)), img, cal.flat)
    end
    fix_pixels!(img, cal.hot)
    return img
end

"""Replace pixels by the median of their 8 neighbours."""
function fix_pixels!(img, idx)
    h, w = size(img)
    for I in idx
        y, x = Tuple(I)
        nb = [img[j, i] for j in max(1, y - 1):min(h, y + 1), i in max(1, x - 1):min(w, x + 1) if (j, i) != (y, x)]
        img[y, x] = RGB{Float32}(median(red.(nb)), median(green.(nb)), median(blue.(nb)))
    end
    return img
end

"""Single pixels much brighter than all their neighbours (hot pixels without darks)."""
function dynamic_hot_pixels(img)
    L = St.luminance(img)
    h, w = size(L)
    v = vec(L)[1:max(1, length(L) ÷ 200_000):end]
    med = median(v); σ = 1.4826 * median(abs.(v .- med)) + 1f-6
    return St.pcollect(2:h-1, CartesianIndex{2}) do rows
        out = CartesianIndex{2}[]
        for y in rows, x in 2:w-1
            l = L[y, x]
            l > med + 8σ || continue
            m = max(L[y-1, x-1], L[y-1, x], L[y-1, x+1], L[y, x-1], L[y, x+1], L[y+1, x-1], L[y+1, x], L[y+1, x+1])
            l > m + 6σ && l > 2m && push!(out, CartesianIndex(y, x))
        end
        out
    end
end

merge4(img) = St.block_average(img, 2) .* 4f0

# --------------------------------------------------------------------------
# Sky region: weight 1 = sky, 0 = foreground (fractions in between: gradient)
# --------------------------------------------------------------------------

function sky_weight(sky, h, w)
    W = ones(Float32, h, w)
    if sky.mode in ("Boundary line", "Gradient") && sky.line !== nothing
        u1, v1, u2, v2 = sky.line
        abs(u2 - u1) < 0.02 && return W
        gw = max(sky.mode == "Gradient" ? sky.gradient : 0.0, 0.005)   # a little feathering
        Threads.@threads for x in 1:w
            u = (x - 0.5) / w
            v0 = v1 + (u - u1) * (v2 - v1) / (u2 - u1)
            for y in 1:h
                W[y, x] = clamp(1 - ((y - 0.5) / h - v0) / gw, 0, 1)
            end
        end
    elseif sky.mode == "Irregular mask" && !isempty(sky.mask) && any(sky.mask)
        M = St.blur(Float32.(sky.mask), 0.7)
        mh, mw = size(M)
        Threads.@threads for x in 1:w
            for y in 1:h
                W[y, x] = St.bilinear(M, (y - 0.5) / h * mh + 0.5, (x - 0.5) / w * mw + 0.5)
            end
        end
    end
    return W
end

# --------------------------------------------------------------------------
# Stars
# --------------------------------------------------------------------------

"""Median and noise (MAD σ) of `L` on a grid of `tile`-pixel tiles."""
function background_grid(L, tile = 64)
    h, w = size(L)
    ny, nx = cld(h, tile), cld(w, tile)
    B = zeros(Float32, ny, nx); N = zeros(Float32, ny, nx)
    Threads.@threads for j in 1:nx
        for i in 1:ny
            v = vec(L[(i-1)*tile+1:2:min(i * tile, h), (j-1)*tile+1:2:min(j * tile, w)])
            m = median(v)
            B[i, j] = m; N[i, j] = 1.4826f0 * median(abs.(v .- m))
        end
    end
    return B, N, tile
end
grid_at(G, tile, y, x) = St.bilinear(G, (y - 0.5) / tile + 0.5, (x - 0.5) / tile + 0.5)

"""
Stars in the sky region (weight ≈ 1, not the gradient zone): local maxima of the smoothed luminance
DETECT_SIGMA × noise above the background, measured by moments (centroid, flux,
FWHM, elongation). Brightest first, at most MAX_STARS.
"""
function detect_stars(img, W)
    L = St.luminance(img)
    S = St.blur(L, 1.0)
    B, N, t = background_grid(S)
    h, w = size(L)
    r, R, margin = 3, 6, 9
    stars = St.pcollect(margin+1:h-margin, Star) do rows
        out = Star[]
        for y in rows, x in margin+1:w-margin
            v = S[y, x]
            bg = grid_at(B, t, y, x); σ = max(grid_at(N, t, y, x), 1f-6)
            (v > bg + DETECT_SIGMA * σ && W[y, x] >= 0.99) || continue
            ismax = true
            for dy in -r:r, dx in -r:r
                (dx == 0 && dy == 0) && continue
                u = S[y+dy, x+dx]
                (u > v || (u == v && (dy, dx) < (0, 0))) && (ismax = false; break)
            end
            ismax || continue
            s = sx = sy = 0.0
            for dy in -R:R, dx in -R:R
                p = L[y+dy, x+dx] - bg - σ
                p > 0 || continue
                s += p; sx += p * dx; sy += p * dy
            end
            s > 0 || continue
            mx, my = sx / s, sy / s
            mxx = myy = mxy = 0.0
            for dy in -R:R, dx in -R:R
                p = L[y+dy, x+dx] - bg - σ
                p > 0 || continue
                mxx += p * (dx - mx)^2; myy += p * (dy - my)^2; mxy += p * (dx - mx) * (dy - my)
            end
            mxx /= s; myy /= s; mxy /= s
            d = sqrt(((mxx - myy) / 2)^2 + mxy^2)
            l1, l2 = (mxx + myy) / 2 + d, max((mxx + myy) / 2 - d, 1e-3)
            # FWHM from the area above half the peak (robust to noise and the window size)
            half = (L[y, x] - bg) / 2
            area = count(L[y+dy, x+dx] - bg > half for dy in -R:R, dx in -R:R)
            fwhm = 2sqrt(area / π)
            area < 0.6 * (2R + 1)^2 || continue       # fills the window: not a star
            push!(out, Star(x + mx, y + my, s, fwhm, sqrt(l1 / l2), L[y, x]))
        end
        out
    end
    # Extended sources (horizon edges, lit clouds, nebulae) are not stars.
    isempty(stars) || (mf = median(s.fwhm for s in stars); filter!(s -> s.fwhm < 2.5mf, stars))
    sort!(stars; by = s -> -s.flux)
    return stars[1:min(end, MAX_STARS)]
end

# --------------------------------------------------------------------------
# Alignment: triangles of neighbouring bright stars are matched by their shape
# (ratios of the sides), which gives candidate star pairs; RANSAC picks the
# consistent ones, then all stars refine the transform.
# --------------------------------------------------------------------------

function triangles(stars)
    n = min(length(stars), ALIGN_STARS)
    P = [(s.x, s.y) for s in stars[1:n]]
    dist(i, j) = hypot(P[i][1] - P[j][1], P[i][2] - P[j][2])
    tris = Set{NTuple{3,Int}}()
    for i in 1:n
        nb = sortperm([dist(i, j) for j in 1:n])[2:min(6, n)]     # 5 nearest neighbours
        for a in eachindex(nb), b in a+1:length(nb)
            push!(tris, Tuple(sort([i, nb[a], nb[b]])))
        end
    end
    out = Tuple{NTuple{2,Float64},NTuple{3,Int}}[]
    for (i, j, k) in tris
        # vertices ordered by the length of the opposite side (shortest first)
        sides = sort([(dist(j, k), i), (dist(i, k), j), (dist(i, j), k)])
        (l0, a), (l1, b), (l2, c) = sides
        l0 > 3 || continue
        push!(out, ((l2 / l1, l1 / l0), (a, b, c)))
    end
    return out
end

"""
Transform T (base pixel → frame pixel) from the stars of both images. Returns
(T, number of matched stars, RMS error in px); throws if they cannot be matched.
"""
function align_stars(base, frame, w; model = :homography)
    length(base) >= 8 && length(frame) >= 8 || error("too few stars")
    tb, tf = triangles(base), triangles(frame)
    bp, fp = Float64[], Float64[]
    for (inv, v) in tf
        isempty(tb) && break
        d, k = findmin(t -> hypot(t[1][1] - inv[1], t[1][2] - inv[2]), tb)
        d < 0.02 || continue
        for q in 1:3
            b, f = base[tb[k][2][q]], frame[v[q]]
            append!(bp, (b.x, b.y)); append!(fp, (f.x, f.y))
        end
    end
    length(bp) >= 12 || error("no matching star patterns")
    tol = max(3.0, 0.004w)
    T, inl = St.ransac(reshape(bp, 2, :), reshape(fp, 2, :); model = :similarity, thresh = tol, iters = 1000)
    length(inl) >= 9 || error("star patterns do not agree")
    # Refine with every star: nearest neighbours under the current transform.
    B = reduce(hcat, [[s.x, s.y] for s in base]); F = reduce(hcat, [[s.x, s.y] for s in frame])
    matched, err = 0, NaN
    for (it, r) in enumerate((2tol, tol, max(1.5, 0.001w), max(1.0, 0.0007w)))
        src, dst = Float64[], Float64[]
        for i in axes(B, 2)
            px, py = St.apply_h(T, B[1, i], B[2, i])
            d, j = findmin(j -> hypot(F[1, j] - px, F[2, j] - py), axes(F, 2))
            d < r && (append!(src, B[:, i]); append!(dst, F[:, j]))
        end
        n = length(src) ÷ 2
        n >= 6 || error("only $n stars match")
        S, Dm = reshape(src, 2, :), reshape(dst, 2, :)
        T = St.fit_model(n >= 15 ? model : :similarity, S, Dm)
        matched = n
        err = sqrt(mean(St.reproj_err(T, S, Dm, i)^2 for i in 1:n))
    end
    return T, matched, err
end

starmatrix(stars) = isempty(stars) ? zeros(2, 0) : reduce(hcat, [[s.x, s.y] for s in stars])

"""Star pairs (base 2×n, frame 2×n) that land within r of each other under `f`."""
function star_pairs(f, B, F, r)
    src, dst = Float64[], Float64[]
    size(F, 2) == 0 && return zeros(2, 0), zeros(2, 0)
    for i in axes(B, 2)
        px, py = f(B[1, i], B[2, i])
        d, j = findmin(j -> hypot(F[1, j] - px, F[2, j] - py), axes(F, 2))
        d < r && (append!(src, B[:, i]); append!(dst, F[:, j]))
    end
    return reshape(src, 2, :), reshape(dst, 2, :)
end

# --------------------------------------------------------------------------
# Distortion: frames shot with a wide lens over a long time do not differ by a
# homography alone (lens distortion, refraction near the horizon), so stars in
# the corners would trail. "Lens" fits one radial distortion term for all
# frames; "Complex" adds, per frame, a smooth correction grid learned from the
# remaining star offsets.
# --------------------------------------------------------------------------

@inline function distort(wp::Warp, x, y)
    wp.k1 == 0 && return x, y
    dx, dy = (x - wp.c[1]) / wp.R, (y - wp.c[2]) / wp.R
    f = 1 + wp.k1 * (dx^2 + dy^2)
    return wp.c[1] + dx * f * wp.R, wp.c[2] + dy * f * wp.R
end
@inline function undistort(wp::Warp, x, y)
    wp.k1 == 0 && return x, y
    ux, uy = x, y
    for _ in 1:5
        dx, dy = (ux - wp.c[1]) / wp.R, (uy - wp.c[2]) / wp.R
        f = 1 + wp.k1 * (dx^2 + dy^2)
        ux, uy = wp.c[1] + (x - wp.c[1]) / f, wp.c[2] + (y - wp.c[2]) / f
    end
    return ux, uy
end
@inline function correction(wp::Warp, x, y)
    isempty(wp.grid) && return 0.0, 0.0
    _, ny, nx = size(wp.grid)
    gx = clamp((x - 1) / wp.cell + 1, 1, nx); gy = clamp((y - 1) / wp.cell + 1, 1, ny)
    x0 = min(floor(Int, gx), nx - 1); y0 = min(floor(Int, gy), ny - 1)
    fx, fy = gx - x0, gy - y0
    G = wp.grid
    return ntuple(Val(2)) do c
        (1 - fy) * ((1 - fx) * G[c, y0, x0] + fx * G[c, y0, x0+1]) +
        fy * ((1 - fx) * G[c, y0+1, x0] + fx * G[c, y0+1, x0+1])
    end
end
"""Where base pixel (x, y) lies in the frame."""
@inline function mapto(wp::Warp, x, y)
    ux, uy = undistort(wp, x, y)
    px, py = distort(wp, St.apply_h(wp.H, ux, uy)...)
    cx, cy = correction(wp, x, y)
    return px + cx, py + cy
end
"""The inverse: base position of frame pixel (x, y)."""
@inline function unmap(wp::Warp, x, y)
    qx, qy = distort(wp, St.apply_h(wp.Hi, undistort(wp, x, y)...)...)
    a, b, c, d = wp.Hi[1, 1] / wp.Hi[3, 3], wp.Hi[1, 2] / wp.Hi[3, 3], wp.Hi[2, 1] / wp.Hi[3, 3], wp.Hi[2, 2] / wp.Hi[3, 3]
    for _ in 1:3
        px, py = mapto(wp, qx, qy)
        ex, ey = x - px, y - py
        qx += a * ex + b * ey; qy += c * ex + d * ey
    end
    return qx, qy
end

"""Homography of undistorted star pairs and its RMS error (undistorted pixels)."""
function lens_fit(lens, B, F)
    n = size(B, 2)
    UB = reduce(hcat, [collect(undistort(lens, B[1, i], B[2, i])) for i in 1:n])
    UF = reduce(hcat, [collect(undistort(lens, F[1, i], F[2, i])) for i in 1:n])
    H = St.fit_model(:homography, UB, UF)
    e = sort([St.reproj_err(H, UB, UF, i)^2 for i in 1:n])
    return H, sqrt(mean(e[1:max(1, round(Int, 0.8n))]))        # the worst 20 % may be wrong pairs
end

"""Radial distortion k1 that lets one homography per frame fit best (golden-section search)."""
function fit_k1(pairs, c, R)
    cost(k) = sum(lens_fit(Warp(Matrix{Float64}(I, 3, 3); k1 = k, c, R), B, F)[2]^2 * size(B, 2) for (B, F) in pairs)
    a, b = -0.3, 0.3
    g = (sqrt(5) - 1) / 2
    x1, x2 = b - g * (b - a), a + g * (b - a)
    f1, f2 = cost(x1), cost(x2)
    for _ in 1:30
        if f1 < f2
            b, x2, f2 = x2, x1, f1; x1 = b - g * (b - a); f1 = cost(x1)
        else
            a, x1, f1 = x1, x2, f2; x2 = a + g * (b - a); f2 = cost(x2)
        end
    end
    k = (a + b) / 2
    return cost(k) < cost(0.0) ? k : 0.0
end

"""Correction grid: star offsets left by `wp`, smoothed (kernel average, shrunk to 0 where stars are sparse)."""
function fit_grid(wp, B, F, h, w; nx = 12)
    cell = (w - 1) / (nx - 1)
    ny = max(2, ceil(Int, (h - 1) / cell) + 1)
    r = [collect((F[1, i], F[2, i]) .- mapto(wp, B[1, i], B[2, i])) for i in axes(B, 2)]
    G = zeros(2, ny, nx)
    σ2 = 2 * (1.2cell)^2
    for j in 1:nx, i in 1:ny
        gx, gy = (j - 1) * cell + 1, (i - 1) * cell + 1
        sw, sx, sy = 2.0, 0.0, 0.0                      # 2.0: pull towards no correction
        for k in axes(B, 2)
            wk = exp(-((B[1, k] - gx)^2 + (B[2, k] - gy)^2) / σ2)
            sw += wk; sx += wk * r[k][1]; sy += wk * r[k][2]
        end
        G[1, i, j], G[2, i, j] = sx / sw, sy / sw
    end
    return Warp(wp.H, wp.Hi, wp.k1, wp.c, wp.R, G, cell)
end

"""
Replace each aligned frame's homography by a warp with distortion correction
(`kind` :lens or :complex). Updates matched stars and errors.
"""
function correct_distortion!(frames, base, kind, h, w)
    c, R = ((w + 1) / 2, (h + 1) / 2), hypot(w, h) / 2
    B = starmatrix(base.detected)
    rwide, rtight = max(4.0, 0.006w), max(1.5, 0.0012w)
    pairs = Dict(fi => star_pairs((x, y) -> St.apply_h(fi.T, x, y), B, starmatrix(fi.detected), rwide)
                 for fi in frames if fi !== base)
    filter!(kv -> size(kv[2][1], 2) >= 12, pairs)
    isempty(pairs) && return 0.0
    k1 = fit_k1(collect(values(pairs)), c, R)
    lens = Warp(Matrix{Float64}(I, 3, 3); k1, c, R)
    base.warp = lens                                     # the identity, whatever k1
    for (fi, (Bp, Fp)) in pairs
        wp = Warp(lens_fit(lens, Bp, Fp)[1]; k1, c, R)
        kind === :complex && (wp = fit_grid(wp, Bp, Fp, h, w))
        # Pairs again with the better model, tighter; refit once.
        Bp, Fp = star_pairs((x, y) -> mapto(wp, x, y), B, starmatrix(fi.detected), rtight)
        size(Bp, 2) >= 12 || continue
        wp = Warp(lens_fit(lens, Bp, Fp)[1]; k1, c, R)
        kind === :complex && (wp = fit_grid(wp, Bp, Fp, h, w))
        fi.warp = wp
        fi.matched = size(Bp, 2)
        fi.err = sqrt(mean(hypot((Fp[:, i] .- mapto(wp, Bp[1, i], Bp[2, i]))...)^2 for i in axes(Bp, 2)))
    end
    return k1
end

# --------------------------------------------------------------------------
# Combining
# --------------------------------------------------------------------------

"""Mean of v[1:n] after iterative sigma clipping (first centre: the median)."""
function sigma_mean!(v, n, lo, hi, iters)
    n == 0 && return 0f0
    n <= 2 && return sum(view(v, 1:n)) / n
    c = median!(view(v, 1:n))
    m = n
    for it in 1:iters
        μ = sum(view(v, 1:m)) / m
        σ = sqrt(sum(x -> (x - μ)^2, view(v, 1:m)) / (m - 1))
        σ > 0 || break
        ctr = it == 1 ? c : μ
        j = 0
        for i in 1:m
            ctr - lo * σ <= v[i] <= ctr + hi * σ && (j += 1; v[j] = v[i])
        end
        (j == m || j < 2) && break
        m = j
    end
    return sum(view(v, 1:m)) / m
end

"""
The stacked image (linear). `mode`: :accumulation (average), :best (sigma-clipped
average), :freeze (sky aligned, foreground not, each sigma-clipped), :trails
(sky: brightest value, optionally fading; foreground: sigma-clipped average).
"""
function combine(D, frames, W, cfg, mode; progress, ref = nothing)
    _, w, h, _ = size(D)
    K = length(frames)
    lo, hi, its = cfg.sigma.low, cfg.sigma.high, cfg.sigma.iters
    warps = [f.warp for f in frames]
    lz = cfg.interpolation == "Lanczos"
    gains = [f.gain for f in frames]
    fade = [cfg.motion_effect ? Float32(0.1 + 0.9 * (k / K)^2) : 1f0 for k in 1:K]
    sel = cfg.selective
    out = Img(undef, h, w)
    chunks = collect(Iterators.partition(1:h, 4))
    done = Threads.Atomic{Int}(0)
    Threads.@threads for rows in chunks
        sky = [Vector{Float32}(undef, K) for _ in 1:3]
        gnd = [Vector{Float32}(undef, K) for _ in 1:3]
        for y in rows, x in 1:w
            m = W[y, x]
            # aligned samples (sky) and samples in place (foreground, star trails)
            ns = ng = 0
            if mode !== :trails && (m > 0 || mode !== :freeze)
                # time-lapse: output pixels belong to the window's reference frame
                bx, by = ref === nothing ? (Float64(x), Float64(y)) : unmap(ref, x, y)
                for k in 1:K
                    px, py = mapto(warps[k], bx, by)
                    s = lz ? sample_lanczos(D, k, px, py, w, h) : sample(D, k, px, py, w, h)
                    s === nothing && continue
                    ns += 1
                    for c in 1:3
                        sky[c][ns] = s[c] * gains[k][c]
                    end
                end
            end
            if (mode === :freeze && m < 1) || mode === :trails
                for k in 1:K
                    ng += 1
                    for c in 1:3
                        gnd[c][ng] = Float32(D[c, x, y, k]) * gains[k][c]
                    end
                end
            end
            val = ntuple(Val(3)) do c
                if mode === :trails
                    t = m > 0 ? maximum(fade[k] * gnd[c][k] for k in 1:ng) : 0f0
                    g = m < 1 ? sigma_mean!(gnd[c], ng, lo, hi, its) : 0f0
                    m * t + (1 - m) * g
                elseif mode === :freeze
                    s = m > 0 ? (sel ? sigma_mean!(sky[c], ns, lo, hi, its) :
                                 ns == 0 ? 0f0 : sum(view(sky[c], 1:ns)) / ns) : 0f0
                    g = m < 1 ? sigma_mean!(gnd[c], ng, lo, hi, its) : 0f0
                    m * s + (1 - m) * g
                elseif mode === :best
                    sigma_mean!(sky[c], ns, lo, hi, its)
                else
                    a = ns == 0 ? 0f0 : sum(view(sky[c], 1:ns)) / ns
                    sel && m > 0 ? m * sigma_mean!(sky[c], ns, lo, hi, its) + (1 - m) * a : a
                end
            end
            out[y, x] = RGB{Float32}(val...)
        end
        Threads.atomic_add!(done, length(rows))
        Threads.threadid() == 1 && progress("Combining: $(round(Int, 100done[] / h)) %", nothing)
    end
    return out
end

"""Frame k resampled onto the base image (for Align only)."""
function resample(D, k, wp; lanczos = true)
    _, w, h, _ = size(D)
    out = Img(undef, h, w)
    Threads.@threads for y in 1:h
        for x in 1:w
            s = lanczos ? sample_lanczos(D, k, mapto(wp, x, y)..., w, h) : sample(D, k, mapto(wp, x, y)..., w, h)
            out[y, x] = s === nothing ? RGB{Float32}(0, 0, 0) : RGB{Float32}(s...)
        end
    end
    return out
end

# --------------------------------------------------------------------------
# Light pollution: a model of the sky background is subtracted (by `strength`),
# leaving a neutral, even sky at 40 % of its median level.
#   Deep sky: a smooth polynomial of degree 1–4 (linear gradients, telescopes).
#   Uneven:   tile levels (a low percentile, so faint nebulosity survives),
#             foreground tiles filled from the sky, smoothed (wide-angle).
#   I.A.A.:   also corrects foreground pixels with the colour of the glow.
# --------------------------------------------------------------------------

"""Per-channel level (quantile q) of the sky on `n` tiles across: (3, ny, nx), NaN where no sky."""
function tile_levels(img, W, n; q = 0.5)
    h, w = size(img)
    t = max(8, w ÷ n)
    ny, nx = cld(h, t), cld(w, t)
    L = fill(NaN32, 3, ny, nx)
    Threads.@threads for j in 1:nx
        for i in 1:ny
            ys, xs = (i-1)*t+1:2:min(i * t, h), (j-1)*t+1:2:min(j * t, w)
            px = [img[y, x] for y in ys, x in xs if W[y, x] >= 0.99]
            length(px) >= 0.3 * length(ys) * length(xs) || continue
            for (c, f) in enumerate((red, green, blue))
                L[c, i, j] = quantile(f.(px), q)
            end
        end
    end
    return L, t
end

"""Gaussian smoothing of a tile grid that ignores (and fills) NaN tiles."""
function smooth_tiles(L, σ)
    _, ny, nx = size(L)
    out = similar(L)
    r = ceil(Int, 3σ)
    for c in 1:3, j in 1:nx, i in 1:ny
        s = sw = 0.0
        for dj in -r:r, di in -r:r
            (1 <= i + di <= ny && 1 <= j + dj <= nx) || continue
            v = L[c, i+di, j+dj]
            isnan(v) && continue
            wk = exp(-(di^2 + dj^2) / (2σ^2)); s += wk * v; sw += wk
        end
        out[c, i, j] = sw > 1e-6 ? s / sw : NaN32
    end
    return out
end

"""Tile grid → full-size image (bilinear between tile centres)."""
function upsample(L, t, h, w)
    B = Img(undef, h, w)
    P = [L[c, :, :] for c in 1:3]
    Threads.@threads for x in 1:w
        for y in 1:h
            v = ntuple(c -> St.bilinear(P[c], (y - 0.5) / t + 0.5, (x - 0.5) / t + 0.5), 3)
            B[y, x] = RGB{Float32}(v...)
        end
    end
    return B
end

function uneven_background(img, W, tiles)
    L, t = tile_levels(img, W, tiles; q = 0.3)
    all(isnan, L) && return nothing
    σ = 1.5
    while any(isnan, L)                                  # fill the foreground from the sky
        F = smooth_tiles(L, σ)
        L = map((a, b) -> isnan(a) ? b : a, L, F)
        σ *= 2
    end
    _, ny, nx = size(L)
    M = similar(L)                                       # 3×3 median: ignores single odd tiles
    for c in 1:3, j in 1:nx, i in 1:ny
        M[c, i, j] = median(L[c, max(1, i - 1):min(ny, i + 1), max(1, j - 1):min(nx, j + 1)])
    end
    return upsample(smooth_tiles(M, 1.0), t, size(img)...)
end

function poly_background(img, W, degree)
    L, t = tile_levels(img, W, 32)
    _, ny, nx = size(L)
    h, w = size(img)
    pts = [(i, j) for j in 1:nx for i in 1:ny if !isnan(L[1, i, j])]
    length(pts) < 6 && return nothing
    terms(u, v) = [u^a * v^b for a in 0:degree for b in 0:degree-a]
    nu(x) = 2 * (x - 0.5) / w - 1; nv(y) = 2 * (y - 0.5) / h - 1
    A = reduce(vcat, [terms(nu((j - 0.5) * t), nv((i - 0.5) * t))' for (i, j) in pts])
    coef = map(1:3) do c
        z = [Float64(L[c, i, j]) for (i, j) in pts]
        keep = trues(length(z))
        k = A \ z
        for _ in 1:3                                     # drop tiles far above the fit (Milky Way, nebulae)
            r = z .- A * k
            σ = 1.4826 * median(abs.(r[keep])) + 1e-9
            keep = -4σ .< r .< 2.5σ
            count(keep) >= size(A, 2) + 2 || break
            k = A[keep, :] \ z[keep]
        end
        k
    end
    B = Img(undef, h, w)
    Threads.@threads for x in 1:w
        for y in 1:h
            tv = terms(nu(x), nv(y))
            B[y, x] = RGB{Float32}((Float32(dot(tv, coef[c])) for c in 1:3)...)
        end
    end
    return B
end

chroma(c) = (s = red(c) + green(c) + blue(c); s > 0 ? (red(c) / s, green(c) / s) : (1 / 3, 1 / 3))

"""Returns (corrected image, background model or nothing)."""
function remove_pollution(img, W, cfg)
    lp = cfg.light_pollution
    lp.filter == "Off" && return img, nothing
    B = lp.filter == "Deep sky" ? poly_background(img, W, lp.degree) : uneven_background(img, W, lp.tiles)
    B === nothing && return img, nothing
    s = Float32(lp.strength / 100)
    n = length(B)
    ped = 0.4f0 * Float32(median(St.luminance(B[i]) for i in 1:max(1, n ÷ 100_000):n))
    iaa = lp.iaa && lp.filter == "Uneven" && cfg.sky.mode != "None"
    out = similar(img)
    Threads.@threads for i in eachindex(img)
        m = W[i]
        if iaa && m < 1
            p, b = img[i], B[i]
            sim = St.luminance(p) >= 0.5f0 * St.luminance(b) ?
                  clamp(1 - hypot((chroma(p) .- chroma(b))...) / 0.08, 0, 1) : 0.0
            m += (1 - m) * Float32(sim)
        end
        out[i] = img[i] - (m * s) * (B[i] - RGB{Float32}(ped, ped, ped))
    end
    return out, B
end

"""Stars brought out: the part of each star above its surroundings, amplified (sky region only)."""
function enhance_stars(img, W, strength, fwhm)
    σ = max(1.5, 1.5 * (isfinite(fwhm) ? fwhm : 2.5))
    Hp = img .- St.blur(img, 2σ)
    n = length(img)
    l = [St.luminance(Hp[i]) for i in 1:max(1, n ÷ 200_000):n]
    noise = 1.4826f0 * median(abs.(l .- median(l))) + 1f-6
    a = Float32(2 * strength / 100)
    out = similar(img)
    Threads.@threads for i in eachindex(img)
        h = Hp[i]
        wgt = clamp((St.luminance(h) / noise - 2) / 3, 0f0, 1f0) * W[i]
        out[i] = img[i] + (a * wgt) * RGB{Float32}(max(red(h), 0f0), max(green(h), 0f0), max(blue(h), 0f0))
    end
    return out
end

# --------------------------------------------------------------------------
# Background extraction (as Siril's: a smooth polynomial of the sky is
# subtracted and its median level added back) and basic colour calibration
# (no plate solving: the sky background is made neutral and the average star
# white).
# --------------------------------------------------------------------------

function extract_background(img, W, degree)
    B = poly_background(img, W, degree)
    B === nothing && return img
    n = length(B)
    sub = 1:max(1, n ÷ 100_000):n
    m = RGB{Float32}(median(red(B[i]) for i in sub), median(green(B[i]) for i in sub), median(blue(B[i]) for i in sub))
    return img .- B .+ Ref(m)
end

function calibrate_color(img, W)
    n = length(img)
    idx = [i for i in 1:max(1, n ÷ 200_000):n if W[i] >= 0.5]
    isempty(idx) && (idx = collect(1:max(1, n ÷ 200_000):n))
    bg = (median(red(img[i]) for i in idx), median(green(img[i]) for i in idx), median(blue(img[i]) for i in idx))
    # Average star colour, from aperture sums above the background.
    h, w = size(img)
    stars = detect_stars(img, W)
    top = isempty(stars) ? 1.0 : maximum(s.peak for s in stars)
    rg, bgr = Float64[], Float64[]
    for st in stars[1:min(end, 400)]
        st.peak < 0.8top || continue
        r = ceil(Int, max(3, 1.5st.fwhm))
        xi, yi = round(Int, st.x), round(Int, st.y)
        (r < xi <= w - r && r < yi <= h - r) || continue
        f = [0.0, 0.0, 0.0]
        for y in yi-r:yi+r, x in xi-r:xi+r
            c = img[y, x]
            f[1] += red(c) - bg[1]; f[2] += green(c) - bg[2]; f[3] += blue(c) - bg[3]
        end
        all(>(0), f) && (push!(rg, f[1] / f[2]); push!(bgr, f[3] / f[2]))
    end
    kr, kb = length(rg) >= 15 ? (1 / median(rg), 1 / median(bgr)) : (1.0, 1.0)
    kr, kb = Float32(clamp(kr, 0.2, 5)), Float32(clamp(kb, 0.2, 5))
    L = St.luminance(RGB{Float32}(bg...))
    return map(c -> RGB{Float32}((red(c) - bg[1]) * kr + L, green(c) - bg[2] + L, (blue(c) - bg[3]) * kb + L), img)
end

"""Colour noise reduction: the colour (difference from the luminance) is smoothed, the luminance kept."""
function reduce_color_noise(img, strength)
    σ = 0.5 + 2.5 * strength / 100
    L = St.luminance(img)
    C = St.blur(map((c, l) -> c - RGB{Float32}(l, l, l), img, L), σ)
    return map((c, l) -> c + RGB{Float32}(l, l, l), C, L)
end

# --------------------------------------------------------------------------
# Output: brightness, HDR, tone (sRGB / Adobe RGB curve, stretch, linear)
# --------------------------------------------------------------------------

# Linear sRGB → linear Adobe RGB (1998), both D65.
to_adobe(c) = RGB{Float32}(0.7152f0 * red(c) + 0.2848f0 * green(c), green(c), 0.0412f0 * green(c) + 0.9588f0 * blue(c))

"""
Returns (linear image × brightness gain, image for TIFF/JPEG, gain, stretch
used). Auto brightness puts the sky background at a fixed level, without letting
more than the brightest 0.1 % of the sky clip (HDR compresses the highlights
instead). Tone: "sRGB curve" (the colour space's curve), "Stretch" (black point,
midtones, white point; automatic or given) or "Linear" (no curve).
"""
function finish(img, W, cfg; gain = nothing)
    n = length(img)
    idx = [i for i in 1:max(1, n ÷ 300_000):n if W[i] >= 0.5]
    isempty(idx) && (idx = collect(1:max(1, n ÷ 300_000):n))
    lum = [St.luminance(img[i]) for i in idx]
    fixed = gain !== nothing                      # time-lapse: the same for every frame
    fixed || (gain = 1.0)
    if cfg.auto_brightness && !fixed
        bg, hi = quantile(lum, (0.5, 0.999))
        target = cfg.hdr ? 0.045 : 0.03
        gain = bg > 0 ? target / bg : 1.0
        cfg.hdr || hi <= 0 || (gain = min(gain, 1.0 / hi))
        gain = clamp(gain, 0.05, 500.0)
    end
    lin = img .* Float32(gain)
    disp = lin
    if cfg.hdr
        Lw = max(1f0, Float32(quantile([St.luminance(lin[i]) for i in 1:max(1, n ÷ 300_000):n], 0.9999)))
        disp = map(lin) do c
            l = St.luminance(c)
            l > 0 ? c * Float32(l * (1 + l / Lw^2) / (1 + l) / l) : c
        end
    end
    cs, tone = cfg.color_space, cfg.tone
    cs == "Linear sRGB" && (cs = "sRGB"; tone = "Linear")          # older settings
    cs == "Adobe RGB" && (disp = map(to_adobe, disp))
    str = nothing
    if tone == "Stretch"
        st = cfg.stretch
        str = st.auto ? St.auto_stretch(disp) : (st.black, st.mid, st.white)
        disp = St.hist_stretch(disp, str...)
    else
        enc(v) = (v = clamp(v, 0f0, 1f0); tone == "Linear" ? v : cs == "Adobe RGB" ? v^(1 / 2.19921875f0) :
                                                                  linear_to_srgb(v))
        disp = map(c -> RGB{Float32}(enc(red(c)), enc(green(c)), enc(blue(c))), disp)
    end
    return lin, disp, gain, str
end

"""
Everything after stacking, on the linear stack `raw` (all optional, in this
order): background extraction, colour calibration, colour noise reduction,
light pollution, star enhancement, then brightness, HDR and tone. Returns (linear, image, gain,
background = removed light pollution as shown or nothing, stretch).
"""
function postprocess(raw, W, cfg; fwhm = NaN, gain = nothing)
    cfg = merge(DEFAULTS, cfg)
    img = raw
    cfg.bg_extraction.on && (img = extract_background(img, W, cfg.bg_extraction.degree))
    cfg.color_calibration && (img = calibrate_color(img, W))
    cfg.color_noise.on && (img = reduce_color_noise(img, cfg.color_noise.strength))
    img, bgm = remove_pollution(img, W, cfg)
    cfg.enhance_stars.on && (img = enhance_stars(img, W, cfg.enhance_stars.strength, fwhm))
    lin, disp, gain, str = finish(img, W, cfg; gain)
    background = bgm === nothing ? nothing : finish(bgm, W, cfg; gain)[2]
    return (linear = lin, image = disp, gain = gain, background = background, stretch = str)
end

"""Save a stack result again with (possibly changed) post-processing settings: FITS / XISF linear."""
function save_result(res, cfg)
    p = postprocess(res.raw, res.W, cfg; fwhm = res.fwhm)
    St.save_panorama(cfg.output, St.isastro(cfg.output) ? p.linear : p.image)
    return cfg.output
end

# --------------------------------------------------------------------------
# The whole run
# --------------------------------------------------------------------------

function mode_of(cfg)
    cfg.composition == "Star trails" && return :trails
    return Dict("Accumulation" => :accumulation, "Select best pixels" => :best,
                "Freeze ground" => :freeze, "Align only" => :alignonly)[cfg.align_mode]
end

"""Mark frames that are much worse than the typical one (needs `reject.auto`)."""
function reject!(infos, cfg, aligned)
    for fi in infos
        fi.status === :pending || continue
        if aligned && !isfinite(fi.err)
            fi.status = :failed
        else
            fi.status = :used
        end
    end
    cfg.reject.auto || return
    ok = [fi for fi in infos if used(fi)]
    length(ok) < 3 && return
    ms, mf, me = median([fi.stars for fi in ok]), medfin([fi.fwhm for fi in ok]), medfin([fi.elong for fi in ok])
    r = cfg.reject
    for fi in ok
        fi.status === :base && continue
        why = fi.light < r.min_light ? "dim stars ($(round(Int, 100fi.light)) % of the base image) – clouds, haze or dew?" :
              fi.stars < r.min_stars * ms ? "few stars ($(fi.stars), typical $(round(Int, ms))) – clouds?" :
              fi.fwhm > r.max_fwhm * mf ? "blurred stars (FWHM $(round(fi.fwhm, digits = 1)) px, typical $(round(mf, digits = 1)))" :
              fi.elong > r.max_elong * me ? "elongated stars ($(round(fi.elong, digits = 2)), typical $(round(me, digits = 2))) – shake / trailing?" : ""
        isempty(why) || (fi.status = :rejected; fi.reason = why)
    end
end
medfin(v) = (v = filter(isfinite, v); isempty(v) ? NaN : median(v))

"""
Stack the star images described by `cfg` (see DEFAULTS; missing settings use
the defaults). `preview = true` makes a quick, small version from up to
PREVIEW_FRAMES images and saves nothing. `progress(message, nothing)` is
called along the way and may throw to cancel; `loader(path)` replaces reading
files (tests). Returns (image for display, linear image, frame report, gain, saved paths).
"""
function stack(cfg; preview = false, progress = (msg, f) -> nothing, loader = nothing)
    try
        return stack_run(cfg; preview, progress, loader)
    finally
        remove_temp_files()
    end
end

function stack_run(cfg; preview, progress, loader)
    cfg = merge(DEFAULTS, cfg)
    load = loader !== nothing ? loader : p -> load_linear(p, cfg; preview)
    stars = sort(unique(cfg.stars))
    infos = [FrameInfo(path = p, status = p in cfg.excluded ? :excluded : :pending) for p in stars]
    cand = [fi for fi in infos if fi.status === :pending]
    isempty(cand) && error("No star images to stack.")
    bi = something(findfirst(fi -> fi.path == cfg.base, cand), cld(length(cand), 2))
    if preview && length(cand) > PREVIEW_FRAMES
        keep = unique([round.(Int, range(1, length(cand), PREVIEW_FRAMES)); bi])
        foreach(fi -> fi.status = :skipped, cand[setdiff(eachindex(cand), keep)])
        foreach(fi -> fi.reason = "not in the quick preview", cand[setdiff(eachindex(cand), keep)])
        cand = cand[sort(keep)]
        bi = findfirst(==(bi), sort(keep))
    end
    base = cand[bi]
    mode = mode_of(cfg)
    aligned = mode !== :trails
    mode in (:best, :freeze) && length(cand) < 4 && error("Select best pixels and Freeze ground need at least 4 star images.")
    mode === :freeze && cfg.sky.mode == "None" &&
        error("Freeze ground needs a sky region (boundary line, gradient or irregular mask).")
    dir = isempty(cfg.output) ? tempdir() : dirname(abspath(cfg.output))
    pick(ps) = preview ? ps[1:min(end, PREVIEW_CALIB)] : ps
    cal = calibration(merge(cfg, (darks = pick(cfg.darks), bias = pick(cfg.bias), flats = pick(cfg.flats),
                                  flatdarks = pick(cfg.flatdarks))), load, dir; progress)

    # Load, calibrate and measure every frame (the base first: it fixes the size).
    function prepare(fi)
        img = calibrate!(load(fi.path), cal)
        cfg.dynamic_noise && fix_pixels!(img, dynamic_hot_pixels(img))
        cfg.merge4 && (img = merge4(img))
        return img
    end
    progress("Loading the base image $(basename(base.path))…", nothing)
    img0 = prepare(base)
    h, w = size(img0)
    W = sky_weight(cfg.sky, h, w)
    D = framestore(h, w, length(cand), dir)
    function measure!(fi, k, img)
        size(img) == (h, w) || error("$(basename(fi.path)) is $(size(img, 2))×$(size(img, 1)), the base image $(w)×$(h).")
        fi.detected = detect_stars(img, W)
        good = [s for s in fi.detected if s.peak < 0.9][1:min(end, 30)]   # bright, unsaturated
        fi.stars = length(fi.detected)
        fi.fwhm = isempty(good) ? NaN : median(s.fwhm for s in good)
        fi.elong = isempty(good) ? NaN : median(s.elong for s in good)
        idx = [i for i in 1:max(1, length(img) ÷ 100_000):length(img) if W[i] >= 0.5]
        isempty(idx) && (idx = collect(1:max(1, length(img) ÷ 100_000):length(img)))
        fi.level = (median(red(img[i]) for i in idx), median(green(img[i]) for i in idx),
                    median(blue(img[i]) for i in idx))
        fi.background = St.luminance(RGB{Float32}(fi.level...))
        fi.light = sum(s.flux for s in fi.detected[1:min(end, 20)]; init = 0.0)   # made relative below
        store!(D, k, img)
    end
    measure!(base, bi, img0)
    img0 = nothing
    base.status = :base
    done = Threads.Atomic{Int}(1)
    sem = Base.Semaphore(clamp(Threads.nthreads() ÷ 2, 1, 4))
    @sync for (k, fi) in enumerate(cand)
        k == bi && continue
        Threads.@spawn Base.acquire(sem) do
            try
                measure!(fi, k, prepare(fi))
                if aligned
                    try
                        fi.T, fi.matched, fi.err = align_stars(base.detected, fi.detected, w; model = cfg.model)
                        fi.warp = Warp(fi.T)
                    catch e
                        fi.reason = "alignment failed: " * (e isa ErrorException ? e.msg : sprint(showerror, e))
                    end
                end
            catch e
                fi.status = :failed
                fi.reason = sprint(showerror, e)
            end
            Threads.atomic_add!(done, 1)
            try progress("Loading, calibrating, aligning: $(done[])/$(length(cand))", nothing) catch end
        end
    end
    progress("Loaded $(length(cand)) frames.", nothing)          # may cancel
    base.matched, base.err = base.stars, 0.0
    bl = base.light
    foreach(fi -> fi.light = bl > 0 ? fi.light / bl : NaN, cand)
    reject!(cand, cfg, aligned)
    frames = [fi for fi in cand if used(fi)]
    if cfg.unify_exposure
        for fi in frames
            fi.gain = ntuple(c -> fi.level[c] > 0 ? Float32(base.level[c] / fi.level[c]) : 1f0, 3)
        end
    end
    ks = [findfirst(==(fi), cand) for fi in frames]
    kind = Dict("Off" => :off, "Lens" => :lens, "Complex" => :complex, "Auto" => :auto)[cfg.distortion]
    if aligned && kind !== :off && length(frames) > 1
        if kind === :auto           # a correction grid needs plenty of stars everywhere
            kind = median([fi.matched for fi in frames if fi !== base]) >= 80 ? :complex : :lens
        end
        progress("Correcting distortion ($kind)…", nothing)
        correct_distortion!(frames, base, kind, h, w)
    end
    saved = String[]
    if mode === :alignonly
        if !preview
            outdir = joinpath(dir, "aligned"); mkpath(outdir)
            ext = St.isastro(cfg.output) ? splitext(cfg.output)[2] : ".tif"
            for (n, (fi, k)) in enumerate(zip(frames, ks))
                progress("Writing aligned frame $n/$(length(frames))", nothing)
                p = joinpath(outdir, splitext(basename(fi.path))[1] * ext)
                g = fi.gain
                St.save_panorama(p, map(c -> RGB{Float32}(red(c) * g[1], green(c) * g[2], blue(c) * g[3]),
                                        resample(D, k, fi.warp; lanczos = cfg.interpolation == "Lanczos")))
                push!(saved, p)
            end
        end
        stacked = resample(D, bi, base.warp)
    elseif cfg.timelapse.on
        # Every N consecutive frames → one output frame, aligned to its middle frame
        # (so the sky moves through the sequence); the brightness of the first for all.
        N = clamp(cfg.timelapse.frames, 1, length(frames))
        nout = preview ? 1 : length(frames) - N + 1
        stem, ext = splitext(basename(cfg.output))
        outdir = joinpath(dir, stem * "_timelapse")
        preview || mkpath(outdir)
        gain0 = nothing
        stacked = nothing
        for n in 1:nout
            win = n:n+N-1
            progress("Time-lapse frame $n/$nout…", nothing)
            ref = mode === :trails ? nothing : frames[win[cld(N, 2)]].warp
            raw_ = combine(view(D, :, :, :, ks[win]), frames[win], W, cfg, mode; progress, ref)
            pp = postprocess(raw_, W, cfg; fwhm = base.fwhm, gain = gain0)
            gain0 === nothing && (gain0 = pp.gain; stacked = raw_)
            if !preview
                p = joinpath(outdir, lpad(n, 4, '0') * (isempty(ext) ? ".tif" : ext))
                St.save_panorama(p, St.isastro(p) ? pp.linear : pp.image)
                push!(saved, p)
            end
        end
    else
        # A view of the store with only the frames in use.
        Dv = length(ks) == size(D, 4) ? D : view(D, :, :, :, ks)
        stacked = combine(Dv, frames, W, cfg, mode; progress)
    end
    progress("Post-processing…", nothing)
    pp = postprocess(stacked, W, cfg; fwhm = base.fwhm)
    lin, disp, gain = pp.linear, pp.image, pp.gain
    if !preview && mode !== :alignonly && !cfg.timelapse.on
        isempty(cfg.output) && error("No output file.")
        progress("Saving $(basename(cfg.output))…", nothing)
        St.save_panorama(cfg.output, St.isastro(cfg.output) ? lin : disp)
        push!(saved, cfg.output)
    end
    return (image = disp, linear = lin, report = infos, gain = gain, saved = saved, base = base.path,
            background = pp.background, stretch = pp.stretch, raw = stacked, W = W, fwhm = base.fwhm)
end

"""The frame report as a text table."""
function report_text(infos)
    io = IOBuffer()
    println(io, rpad("frame", 26), lpad("stars", 6), lpad("FWHM", 7), lpad("elong", 7), lpad("sky", 8), lpad("light", 7),
            lpad("align", 8), "  status")
    for fi in infos
        f(v, d) = isfinite(v) ? string(round(v; digits = d)) : "–"
        println(io, rpad(first(basename(fi.path), 25), 26), lpad(fi.stars, 6), lpad(f(fi.fwhm, 1), 7),
                lpad(f(fi.elong, 2), 7), lpad(f(fi.background, 4), 8), lpad(f(fi.light, 2), 7), lpad(f(fi.err, 2), 8), "  ",
                fi.status, isempty(fi.reason) ? "" : ": " * fi.reason)
    end
    return String(take!(io))
end

end # module
