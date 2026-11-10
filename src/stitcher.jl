# Stitching core, split into steps so the GUI (gui.jl) can run, inspect and
# adjust each one:
#
#   load (RAW/JPEG/TIFF) → corner features → match neighbours → RANSAC motion
#   model → chain to a reference image → project (5 projections) → blend.
#
# Assumes the images are given in order (each overlaps the next one) and were
# shot by rotating the camera (standard panorama case).

module Stitcher

using ColorTypes, ColorVectorSpace, FixedPointNumbers, FileIO, LinearAlgebra, Random, Statistics
import Dates, LibRaw_jll, Zlib_jll, Zstd_jll

export stitch, load_any, save_panorama

const Img = Matrix{RGB{Float32}}
const MODELS = (:homography, :affine, :similarity, :translation)
const BLENDS = (:smart, :feather, :seam, :average)
const PROJECTIONS = (:rectilinear, :cylindrical, :spherical, :mercator, :stereographic,
                     :transverse_cylindrical, :transverse_mercator, :fisheye, :fisheye_equisolid,
                     :little_planet, :tunnel, :orthographic, :hammer, :mollweide)

# --------------------------------------------------------------------------
# Small parallel helpers
# --------------------------------------------------------------------------

"""Apply `f` to chunks of `r` on all threads and concatenate the returned vectors."""
function pcollect(f, r, T)
    isempty(r) && return T[]
    n = cld(length(r), 4 * Threads.nthreads())
    tasks = [Threads.@spawn f(c) for c in Iterators.partition(r, n)]
    return reduce(vcat, fetch.(tasks); init = T[])
end

# --------------------------------------------------------------------------
# Image basics (blur, resize, luminance)
# --------------------------------------------------------------------------

luminance(c::RGB) = 0.299f0 * red(c) + 0.587f0 * green(c) + 0.114f0 * blue(c)
luminance(img::AbstractMatrix{<:RGB}) = map(luminance, img)

function gauss_kernel(σ)
    r = ceil(Int, 3σ)
    k = Float32[exp(-x^2 / (2σ^2)) for x in -r:r]
    return k ./ sum(k)
end

"""Separable Gaussian blur with clamped borders (multi-threaded)."""
function blur(A::AbstractMatrix{T}, σ) where {T}
    σ <= 0 && return copy(A)
    k = gauss_kernel(σ); r = length(k) ÷ 2
    h, w = size(A)
    B = similar(A); C = similar(A)
    Threads.@threads for j in 1:w
        for i in 1:h
            s = zero(T)
            for t in -r:r
                s += k[t+r+1] * A[clamp(i + t, 1, h), j]
            end
            B[i, j] = s
        end
    end
    Threads.@threads for j in 1:w
        for i in 1:h
            s = zero(T)
            for t in -r:r
                s += k[t+r+1] * B[i, clamp(j + t, 1, w)]
            end
            C[i, j] = s
        end
    end
    return C
end

"""Average k×k blocks (fast integer downscale)."""
function block_average(A::AbstractMatrix{T}, k::Int) where {T}
    h, w = size(A) .÷ k
    B = Matrix{T}(undef, h, w)
    Threads.@threads for j in 1:w
        for i in 1:h
            s = zero(T)
            for dj in 1:k, di in 1:k
                s += A[(i-1)*k+di, (j-1)*k+dj]
            end
            B[i, j] = s / k^2
        end
    end
    return B
end

# Bilinear interpolation at (y, x); positions outside the image use its edge
# pixels (never extrapolates), and 1-pixel-wide images are allowed.
@inline function bilinear(img, y, x)
    h, w = size(img)
    y0 = clamp(floor(Int, y), 1, max(h - 1, 1)); x0 = clamp(floor(Int, x), 1, max(w - 1, 1))
    y1 = min(y0 + 1, h);                          x1 = min(x0 + 1, w)
    dy = clamp(Float32(y - y0), 0f0, 1f0);        dx = clamp(Float32(x - x0), 0f0, 1f0)
    top = (1 - dx) * img[y0, x0] + dx * img[y0, x1]
    bot = (1 - dx) * img[y1, x0] + dx * img[y1, x1]
    return (1 - dy) * top + dy * bot
end

"""Resize by `ratio` (≤ 1 gives an anti-aliased downscale)."""
function resize(A::AbstractMatrix, ratio)
    ratio == 1 && return copy(A)
    k = ratio < 1 ? floor(Int, 1 / ratio) : 1
    if k >= 2                                   # most of the reduction by block averaging
        A = block_average(A, k)
        ratio *= k
    end
    ratio < 1 && (A = blur(A, 0.5 * (1 / ratio - 1)))
    h, w = size(A)
    H, W = max(1, round(Int, h * ratio)), max(1, round(Int, w * ratio))
    B = similar(A, H, W)
    Threads.@threads for j in 1:W
        for i in 1:H
            B[i, j] = bilinear(A, (i - 0.5) / ratio + 0.5, (j - 0.5) / ratio + 0.5)
        end
    end
    return B
end

# Pixel centre in a ratio-`s` resized image → pixel centre in the original.
to_full(v, s) = (v - 0.5) / s + 0.5

# --------------------------------------------------------------------------
# Loading
# --------------------------------------------------------------------------

const RAW_EXTS = Set([".cr2", ".cr3", ".nef", ".nrw", ".arw", ".srf", ".dng",
                      ".raf", ".orf", ".rw2", ".pef", ".srw", ".raw", ".3fr", ".iiq"])
const FITS_EXTS = Set([".fits", ".fit", ".fts"])
const XISF_EXTS = Set([".xisf"])
const IMAGE_EXTS = union(RAW_EXTS, FITS_EXTS, XISF_EXTS, Set([".jpg", ".jpeg", ".png", ".tif", ".tiff", ".bmp"]))

isfits(path) = lowercase(splitext(path)[2]) in FITS_EXTS
isxisf(path) = lowercase(splitext(path)[2]) in XISF_EXTS
"""FITS or XISF: astronomy formats, saved as linear 32-bit float."""
isastro(path) = isfits(path) || isxisf(path)

"""
Load RAW, FITS, XISF, JPEG or TIFF into a Matrix{RGB{Float32}}. For RAW files,
`raw_decoder` picks the converter (see RAW_DECODERS); `raw_half` decodes RAW
files and colour-camera (Bayer) FITS / XISF files at half size.

For stacking, `linear = true` keeps RAW data linear (no tone curve; LibRaw or
dcraw only), `wb` is :camera, :auto (average of the image) or :daylight, and
`quality` the demosaicing (0 bilinear, 1 VNG, 2 PPG, 3 AHD; nothing = default).
"""
function load_any(path::AbstractString; raw_decoder = :auto, raw_half = false, linear = false,
                  wb = :camera, quality = nothing)::Img
    ext = lowercase(splitext(path)[2])
    ext in FITS_EXTS && return read_fits(path; half = raw_half)
    ext in XISF_EXTS && return read_xisf(path; half = raw_half)
    img = ext in RAW_EXTS ? load_raw(path; decoder = raw_decoder, half = raw_half, linear, wb, quality) :
                            load(path)
    img = collect(img)
    ndims(img) == 3 && (img = img[:, :, 1])   # multi-page TIFF → first page
    return RGB{Float32}.(img)
end

# --------------------------------------------------------------------------
# RAW conversion
#
# All decoders write a 16-bit TIFF into a private temporary directory (never next
# to the original, which may be on a read-only card). Supported formats:
#   libraw      LibRaw's dcraw_emu: CR2, CR3, NEF, ARW, DNG, RAF, ORF, RW2, …
#               Neutral rendering, no per-image auto-brightening, so a
#               panorama's frames keep their relative exposure. Comes with the
#               program (LibRaw_jll), so it works without installing anything.
#   rawtherapee rawtherapee-cli: same formats; applies your edits from a
#               <photo>.pp3 sidecar if one exists, else its default profile.
#   darktable   darktable-cli with its default pipeline (incl. lens
#               correction where available), slowest.
#   dcraw       classic dcraw: no CR3 support.
# The other three are optional programs the user may have installed. :auto
# tries them in that order (RawTherapee first if the photo has a .pp3 sidecar)
# and uses the first that succeeds.
# --------------------------------------------------------------------------

const RAW_DECODERS = (:auto, :libraw, :rawtherapee, :darktable, :dcraw)
const RAW_EXE = Dict(:libraw => "dcraw_emu", :rawtherapee => "rawtherapee-cli",
                     :darktable => "darktable-cli", :dcraw => "dcraw")

"""RAW decoders that can be used, in the order :auto tries them."""
available_raw_decoders() =
    [d for d in RAW_DECODERS[2:end]
     if d === :libraw ? LibRaw_jll.is_available() : Sys.which(RAW_EXE[d]) !== nothing]

"""Decode a RAW file into an RGB matrix with the chosen decoder (or the first that works)."""
function load_raw(path::AbstractString; decoder = :auto, half = false, linear = false,
                  wb = :camera, quality = nothing)
    isfile(path) || error("File not found: $path")
    found = available_raw_decoders()
    # Only LibRaw and dcraw can skip the tone curve and choose the demosaicing.
    (linear || wb !== :camera || quality !== nothing) && filter!(d -> d in (:libraw, :dcraw), found)
    if decoder === :auto
        isempty(found) && error("No suitable RAW decoder available: the bundled LibRaw does not " *
                                "run on this system. Install dcraw" * (linear ? " (needed for linear data)." :
                                ", RawTherapee or darktable."))
        lowercase(splitext(path)[2]) == ".cr3" && filter!(!=(:dcraw), found)
        # A RawTherapee sidecar means the photo was edited there: honour it first.
        isfile(path * ".pp3") && :rawtherapee in found &&
            (found = [:rawtherapee; filter(!=(:rawtherapee), found)])
        tries = found
    else
        decoder in found || error("RAW decoder $decoder ($(RAW_EXE[decoder])) is not installed.")
        tries = [decoder]
    end
    errs = String[]
    for d in tries
        dir = mktempdir()
        try
            out = joinpath(dir, "decoded.tif")
            log = joinpath(dir, "log.txt")
            try
                run(pipeline(raw_cmd(d, path, out, dir, half; linear, wb, quality); stdout = log, stderr = log))
            catch
                msg = isfile(log) ? strip(read(log, String)) : ""
                error("$(RAW_EXE[d]) failed" * (isempty(msg) ? "" : ": " * last(msg, 300)))
            end
            isfile(out) || error("$(RAW_EXE[d]) produced no output")
            img = RGB{Float32}.(collect(load(out)))
            # Only LibRaw and dcraw can decode at half size themselves.
            half && d in (:rawtherapee, :darktable) && (img = resize(img, 0.5))
            @info "Decoded $(basename(path)) with $d ($(size(img, 2))×$(size(img, 1)))"
            return img
        catch e
            push!(errs, "  $d: " * sprint(showerror, e))
        finally
            rm(dir; recursive = true, force = true)
        end
    end
    error("Could not decode $(basename(path)):\n" * join(errs, "\n"))
end

function raw_cmd(d, path, out, dir, half; linear = false, wb = :camera, quality = nothing)
    # -w camera WB (-a average, none = daylight), -W no auto-brighten, -6 16-bit,
    # -g 1 1 linear, -q demosaicing, -h half size, -T TIFF, -Z output file
    opts = [wb === :camera ? ["-w"] : wb === :auto ? ["-a"] : String[];
            "-W"; "-6"; linear ? ["-g", "1", "1"] : String[];
            quality === nothing ? String[] : ["-q", string(quality)]; half ? ["-h"] : String[]]
    if d === :libraw
        return `$(LibRaw_jll.dcraw_emu()) $opts -T -Z $out $path`     # bundled, with its libraries
    elseif d === :dcraw
        return pipeline(`dcraw -c $opts -T $path`, stdout = out)
    elseif d === :rawtherapee
        # -s apply the photo's .pp3 sidecar (your RawTherapee edits) if present, else
        # -d the default profile; -t TIFF, -b16 16-bit, -Y overwrite, -c input (last)
        return `rawtherapee-cli -q -s -d -t -b16 -Y -o $out -c $path`
    else
        # A private config dir avoids clashing with a running darktable.
        return `darktable-cli $path $out --hq true --apply-custom-presets false
                --core --configdir $(joinpath(dir, "dtconf"))
                --conf plugins/imageio/format/tiff/bpp=16`
    end
end

# --------------------------------------------------------------------------
# FITS (astronomy images)
#
# Reads the first image of a FITS file: 8/16/32/64-bit integer or 32/64-bit
# float, mono (NAXIS = 2) or RGB planes (NAXIS3 = 3). Mono images with a Bayer
# pattern (BAYERPAT, from colour astro cameras) are debayered. ROWORDER is
# honoured (FITS default: bottom row first). Values are scaled to about 0…1.
# Tile-compressed files (.fz) are not supported.
# --------------------------------------------------------------------------

"""Parse the value of a FITS header card (after "= ")."""
function fits_value(v)
    v = strip(v)
    if startswith(v, "'")                                  # string: '' is an escaped quote
        i, out = 2, IOBuffer()
        while i <= lastindex(v)
            if v[i] == '\''
                i < lastindex(v) && v[i+1] == '\'' ? (write(out, '\''); i += 2; continue) : break
            end
            write(out, v[i]); i = nextind(v, i)
        end
        return rstrip(String(take!(out)))
    end
    v = strip(split(v, '/')[1])                            # drop the comment
    v == "T" && return true
    v == "F" && return false
    x = tryparse(Int, v)
    x === nothing || return x
    x = tryparse(Float64, replace(v, 'D' => 'E'))
    return x === nothing ? v : x
end

"""Header of the HDU starting at byte `pos` (1-based): (Dict, start of its data)."""
function fits_header(bytes, pos)
    hdr = Dict{String,Any}()
    while true
        pos + 2879 <= length(bytes) || error("Truncated FITS header")
        block = String(bytes[pos:pos+2879]); pos += 2880
        for k in 0:35
            card = block[80k+1:80k+80]
            key = strip(card[1:8])
            key == "END" && return hdr, pos
            card[9:10] == "= " && (hdr[key] = fits_value(card[11:end]))
        end
    end
end

fits_type(bitpix) = Dict(8 => UInt8, 16 => Int16, 32 => Int32, 64 => Int64,
                         -32 => Float32, -64 => Float64)[bitpix]

"""Read the first image of a FITS file as RGB (see the section comment)."""
function read_fits(path; half = false)
    bytes = read(path)
    pos = 1
    while true
        hdr, data = fits_header(bytes, pos)
        naxis = get(hdr, "NAXIS", 0)
        dims = [hdr["NAXIS$k"] for k in 1:naxis]
        T = fits_type(hdr["BITPIX"])
        n = naxis == 0 ? 0 : prod(dims)
        if naxis >= 2 && n > 0 && get(hdr, "XTENSION", "IMAGE") == "IMAGE"
            return fits_image(bytes, data, hdr, T, dims; half)
        end
        get(hdr, "XTENSION", "") == "BINTABLE" && get(hdr, "ZIMAGE", false) == true &&
            error("$(basename(path)) is tile-compressed (.fz); decompress it first (e.g. with funpack).")
        # Skip this HDU's data (padded to 2880 bytes) and try the next one.
        nbytes = n * sizeof(T) + get(hdr, "PCOUNT", 0)
        pos = data + 2880 * cld(nbytes, 2880)
        pos > length(bytes) && error("$(basename(path)) contains no image.")
    end
end

function fits_image(bytes, data, hdr, T, dims; half = false)
    n = prod(dims)
    raw = ntoh.(reinterpret(T, bytes[data:data+n*sizeof(T)-1]))
    bscale = Float32(get(hdr, "BSCALE", 1.0)); bzero = Float32(get(hdr, "BZERO", 0.0))
    A = reshape(Float32.(raw) .* bscale .+ bzero, dims[1], dims[2], length(dims) >= 3 ? dims[3] : 1)
    # Scale to about 0…1.
    if T <: Integer
        bits = 8sizeof(T)
        top = T === UInt8 ? 255f0 : bzero ≈ 2f0^(bits - 1) ? 2f0^bits - 1 : 2f0^(bits - 1) - 1
    else
        # Floats are usually 0…1 (bright stars may go above) or 0…65535: judge by
        # the bulk of the pixels, not the brightest one.
        fin = filter(isfinite, vec(A)[1:max(1, length(A) ÷ 1_000_000):end])
        hi = isempty(fin) ? 1f0 : quantile(fin, 0.99)
        top = hi <= 1.5f0 ? 1f0 : hi <= 65535 * 1.01f0 ? 65535f0 : maximum(fin)
    end
    A = map(x -> isfinite(x) ? x / top : 0f0, A)
    # FITS stores x (NAXIS1) fastest: plane k is A[:, :, k] with A[x, y].
    planes = [permutedims(A[:, :, k]) for k in 1:size(A, 3)]         # → [y, x]
    pat = uppercase(string(get(hdr, "BAYERPAT", get(hdr, "COLORTYP", ""))))
    if length(planes) == 1 && pat in ("RGGB", "BGGR", "GRBG", "GBRG")
        # Bayer offsets shift the pattern by one row / column.
        isodd(get(hdr, "XBAYROFF", 0)) && (pat = pat[[2, 1, 4, 3]])
        isodd(get(hdr, "YBAYROFF", 0)) && (pat = pat[[3, 4, 1, 2]])
        img = debayer(planes[1], pat; half)
    elseif length(planes) >= 3
        img = RGB{Float32}.(planes[1], planes[2], planes[3])
    else
        img = RGB{Float32}.(planes[1], planes[1], planes[1])
    end
    # Default FITS row order is bottom-up; show the top row first.
    uppercase(string(get(hdr, "ROWORDER", "BOTTOM-UP"))) == "TOP-DOWN" || (img = img[end:-1:1, :])
    return img
end

"""
Colour image from a Bayer mosaic `M` (rows top first as stored) with pattern
`pat` (e.g. "RGGB": top-left R, top-right G, then G, B). `half = true` makes
one colour pixel per 2×2 cell; otherwise each colour is interpolated bilinearly.
"""
function debayer(M, pat; half = false)
    h, w = size(M)
    chan(r, c) = pat[2(r - 1) + c]                         # colour at cell position (r, c) ∈ 1:2
    if half
        H, W = h ÷ 2, w ÷ 2
        out = Matrix{RGB{Float32}}(undef, H, W)
        Threads.@threads for j in 1:W
            for i in 1:H
                v = Dict('R' => 0f0, 'G' => 0f0, 'B' => 0f0)
                for r in 1:2, c in 1:2
                    v[chan(r, c)] += M[2i-2+r, 2j-2+c]
                end
                out[i, j] = RGB{Float32}(v['R'], v['G'] / 2, v['B'])
            end
        end
        return out
    end
    # Bilinear: average of the known neighbours of each colour (3×3, centre weighted).
    wts = (1f0, 2f0, 1f0)
    out = Matrix{RGB{Float32}}(undef, h, w)
    Threads.@threads for j in 1:w
        for i in 1:h
            acc = Dict('R' => 0f0, 'G' => 0f0, 'B' => 0f0); cnt = Dict('R' => 0f0, 'G' => 0f0, 'B' => 0f0)
            own = chan(mod1(i, 2), mod1(j, 2))
            for di in -1:1, dj in -1:1
                ii, jj = i + di, j + dj
                (1 <= ii <= h && 1 <= jj <= w) || continue
                ch = chan(mod1(ii, 2), mod1(jj, 2))
                wgt = wts[di+2] * wts[dj+2]
                acc[ch] += wgt * M[ii, jj]; cnt[ch] += wgt
            end
            val(ch) = ch == own ? M[i, j] : acc[ch] / max(cnt[ch], 1f-6)
            out[i, j] = RGB{Float32}(val('R'), val('G'), val('B'))
        end
    end
    return out
end

"""
Write `img` as a 32-bit float FITS file with three colour planes (bottom row
first, the FITS default), so astronomy software can process it further.
"""
function write_fits(path, img)
    h, w = size(img)
    card(k, v) = rpad(rpad(k, 8) * "= " * (v isa AbstractString ? rpad("'$v'", 20) : lpad(v isa Bool ? (v ? "T" : "F") : string(v), 20)), 80)
    cards = [card("SIMPLE", true), card("BITPIX", -32), card("NAXIS", 3), card("NAXIS1", w),
             card("NAXIS2", h), card("NAXIS3", 3), card("BZERO", 0.0), card("BSCALE", 1.0),
             card("ROWORDER", "BOTTOM-UP"), card("CREATOR", "PanoStitch"), rpad("END", 80)]
    header = join(cards)
    header = rpad(header, 2880 * cld(length(header), 2880))
    data = Vector{Float32}(undef, 3h * w)
    k = 0
    for f in (red, green, blue), y in h:-1:1, x in 1:w
        v = f(img[y, x]); data[k += 1] = isfinite(v) ? v : 0f0
    end
    open(path, "w") do io
        write(io, header)
        write(io, hton.(data))
        write(io, zeros(UInt8, 2880 * cld(4length(data), 2880) - 4length(data)))
    end
    return path
end

# --------------------------------------------------------------------------
# XISF (PixInsight images)
#
# Reads the first image of an XISF file: 8/16/32-bit integer or 32/64-bit
# float, mono or RGB, planar or interleaved, little or big endian, stored as an
# attachment, uncompressed or compressed with zlib, LZ4 or Zstandard (with or
# without byte shuffling). Mono images with a colour filter array (one-shot
# colour cameras) are debayered. Writes 32-bit float RGB, uncompressed.
# --------------------------------------------------------------------------

xml_attrs(s) = Dict(String(m[1]) => String(m[2]) for m in eachmatch(r"([\w:]+)\s*=\s*\"([^\"]*)\"", s))

"""Read the first image of an XISF file as RGB (see the section comment)."""
function read_xisf(path; half = false)
    bytes = read(path)
    (length(bytes) >= 16 && String(bytes[1:8]) == "XISF0100") || error("$(basename(path)) is not an XISF file.")
    hlen = Int(ltoh(reinterpret(UInt32, bytes[9:12])[1]))
    xml = String(bytes[17:16+hlen])
    m = match(r"<Image\b([^>]*?)(?:/>|>(.*?)</Image>)"s, xml)
    m === nothing && error("$(basename(path)) contains no image.")
    a = xml_attrs(m[1])
    body = something(m[2], "")
    g = parse.(Int, split(a["geometry"], ':'))
    W, H, C = g[1], g[2], length(g) >= 3 ? g[end] : 1
    length(g) <= 3 || error("$(basename(path)): only 2-D images are supported.")
    fmt = get(a, "sampleFormat", "UInt16")
    T = get(Dict("UInt8" => UInt8, "UInt16" => UInt16, "UInt32" => UInt32, "Float32" => Float32,
                 "Float64" => Float64), fmt, nothing)
    T === nothing && error("$(basename(path)): sample format $fmt is not supported.")
    loc = split(get(a, "location", ""), ':')
    loc[1] == "attachment" || error("$(basename(path)): $(loc[1]) image data is not supported (only attachments).")
    pos, size_ = parse(Int, loc[2]), parse(Int, loc[3])
    blk = xisf_decompress(bytes[pos+1:pos+size_], get(a, "compression", ""), get(a, "subblocks", ""))
    n = W * H * C
    length(blk) >= n * sizeof(T) || error("$(basename(path)): the image data is incomplete.")
    big = get(a, "byteOrder", "little") == "big"
    v = Float32.((big ? ntoh : ltoh).(reinterpret(T, blk[1:n*sizeof(T)])))
    # Scale to 0…1: integers by their range, floats by the declared bounds.
    if T <: Integer
        v ./= Float32(typemax(T))
    elseif haskey(a, "bounds")
        lo, hi = parse.(Float32, split(a["bounds"], ':'))
        (lo, hi) == (0, 1) || hi <= lo || (v .= (v .- lo) ./ (hi - lo))
    end
    replace!(x -> isfinite(x) ? x : 0f0, v)
    # Planar: one channel after the other; normal: channels interleaved. Rows top first, x fastest.
    planes = get(a, "pixelStorage", "Planar") == "Normal" ?
        [permutedims(reshape(v, C, W, H)[c, :, :]) for c in 1:C] :
        [permutedims(reshape(v[(c-1)*W*H+1:c*W*H], W, H)) for c in 1:C]
    cfa = match(r"<ColorFilterArray\b([^>]*)>", body)
    pat = cfa === nothing ? "" : uppercase(get(xml_attrs(cfa[1]), "pattern", ""))
    C == 1 && pat in ("RGGB", "BGGR", "GRBG", "GBRG") && return debayer(planes[1], pat; half)
    C >= 3 && return RGB{Float32}.(planes[1], planes[2], planes[3])
    return RGB{Float32}.(planes[1], planes[1], planes[1])
end

function xisf_decompress(blk, comp, subblocks)
    isempty(comp) && return blk
    parts = split(comp, ':')
    codec, usize = parts[1], parse(Int, parts[2])
    item = length(parts) >= 3 ? parse(Int, parts[3]) : 1
    shuffled = endswith(codec, "+sh")
    codec = replace(codec, "+sh" => "")
    chunks = isempty(subblocks) ? [(length(blk), usize)] :
             [(c = parse.(Int, split(sb, ',')); (c[1], c[2])) for sb in split(subblocks, ':')]
    out = UInt8[]
    p = 1
    for (cs, us) in chunks
        src = blk[p:p+cs-1]; p += cs
        append!(out, codec == "zlib" ? zlib_inflate(src, us) :
                     codec in ("lz4", "lz4hc") ? lz4_block(src, us) :
                     codec == "zstd" ? zstd_decompress(src, us) :
                     error("XISF compression “$(codec)” is not supported."))
    end
    return shuffled ? unshuffle(out, item) : out
end

function zlib_inflate(src, n)
    out = Vector{UInt8}(undef, n)
    len = Ref{Culong}(n)
    r = ccall((:uncompress, Zlib_jll.libz), Cint, (Ptr{UInt8}, Ref{Culong}, Ptr{UInt8}, Culong),
              out, len, src, length(src))
    r == 0 || error("zlib: damaged data (error $r)")
    return resize!(out, len[])
end

function zstd_decompress(src, n)
    out = Vector{UInt8}(undef, n)
    r = ccall((:ZSTD_decompress, Zstd_jll.libzstd), Csize_t, (Ptr{UInt8}, Csize_t, Ptr{UInt8}, Csize_t),
              out, n, src, length(src))
    ccall((:ZSTD_isError, Zstd_jll.libzstd), Cuint, (Csize_t,), r) == 0 || error("zstd: damaged data")
    return resize!(out, r)
end

"""Decode an LZ4 block (the format XISF uses for lz4 and lz4hc)."""
function lz4_block(src, n)
    out = Vector{UInt8}(undef, n)
    i = o = 1
    len(l) = (if l == 15; while true; b = src[i]; i += 1; l += b; b == 255 || break; end; end; l)
    while i <= length(src)
        tok = src[i]; i += 1
        lit = len(Int(tok >> 4))
        copyto!(out, o, src, i, lit); o += lit; i += lit
        i > length(src) && break                         # the last sequence has only literals
        off = Int(src[i]) | Int(src[i+1]) << 8; i += 2
        ml = len(Int(tok & 0x0f)) + 4
        for k in 0:ml-1                                  # may overlap its own output
            out[o+k] = out[o-off+k]
        end
        o += ml
    end
    o - 1 == n || error("LZ4: damaged data")
    return out
end

"""Undo byte shuffling: byte j of every item was stored together."""
function unshuffle(b, item)
    item <= 1 && return b
    n = length(b) ÷ item
    out = similar(b)
    for j in 0:item-1, i in 0:n-1
        out[i*item+j+1] = b[j*n+i+1]
    end
    out[n*item+1:end] .= b[n*item+1:end]
    return out
end

"""
Write `img` as a 32-bit float XISF file (RGB, planar, uncompressed). Negative
values become 0; values above 1 are kept, with the bounds widened to match
(readers such as PixInsight then scale the image to 0…1).
"""
function write_xisf(path, img)
    h, w = size(img)
    data = Vector{Float32}(undef, 3h * w)
    k = 0
    for f in (red, green, blue), y in 1:h, x in 1:w
        v = f(img[y, x]); data[k += 1] = isfinite(v) ? max(v, 0f0) : 0f0
    end
    hi = max(1f0, maximum(data))
    stamp = Dates.format(Dates.now(Dates.UTC), "yyyy-mm-ddTHH:MM:SS") * "Z"
    header(pos) = """<?xml version="1.0" encoding="UTF-8"?>
        <xisf version="1.0" xmlns="http://www.pixinsight.com/xisf" xmlns:xsi="http://www.w3.org/2001/XMLSchema-instance" xsi:schemaLocation="http://www.pixinsight.com/xisf http://pixinsight.com/xisf/xisf-1.0.xsd">
        <Image geometry="$w:$h:3" sampleFormat="Float32" bounds="0:$hi" colorSpace="RGB" pixelStorage="Planar" byteOrder="little" location="attachment:$pos:$(4length(data))"/>
        <Metadata>
        <Property id="XISF:CreationTime" type="TimePoint" value="$stamp"/>
        <Property id="XISF:CreatorApplication" type="String">PanoStitch</Property>
        </Metadata>
        </xisf>"""
    pos = 4096                                    # the data starts on a 4 KiB boundary
    while 16 + ncodeunits(header(pos)) > pos
        pos += 4096
    end
    xml = header(pos)
    open(path, "w") do io
        write(io, "XISF0100", htol(UInt32(ncodeunits(xml))), UInt32(0), xml)
        write(io, zeros(UInt8, pos - 16 - ncodeunits(xml)))
        write(io, htol.(data))
    end
    return path
end

# --------------------------------------------------------------------------
# Histogram stretch (for linear data such as deep-sky images)
#
# Black point b and white point w clip the range, then the midtones transfer
# function brightens the shadows: mid = 0.5 is linear, smaller is brighter.
# --------------------------------------------------------------------------

"""Midtones transfer function: maps 0 → 0, `m` → 0.5, 1 → 1."""
@inline mtf(m, x) = x <= 0 ? zero(x) : x >= 1 ? one(x) : (m - 1) * x / ((2m - 1) * x - m)

"""Apply black point, midtones and white point to every colour channel."""
function hist_stretch(img, black, mid, white)
    s = 1f0 / max(Float32(white - black), 1f-6)
    b, m = Float32(black), Float32(mid)
    f(v) = mtf(m, clamp((v - b) * s, 0f0, 1f0))
    return map(c -> RGB{Float32}(f(red(c)), f(green(c)), f(blue(c))), img)
end

"""
Automatic stretch (as PixInsight's screen transfer function does): black point
just below the sky background, midtones so that the background ends up at
`target` brightness. Black pixels (outside the panorama) are ignored.
Returns (black, mid, white).
"""
function auto_stretch(img; target = 0.25, clip = 2.8)
    v = [luminance(c) for c in img[1:max(1, length(img) ÷ 200_000):end] if luminance(c) > 0]
    isempty(v) && return (0.0, 0.5, 1.0)
    med = median(v)
    mad = 1.4826 * median(abs.(v .- med))
    black = clamp(med - clip * mad, 0.0, 0.99)
    x = (med - black) / (1 - black)
    mid = x > 0 ? clamp(mtf(target, x), 0.0001, 0.9999) : 0.5
    return (black, mid, 1.0)
end

"""Histogram (counts in `n` bins over 0…1) of the luminance of non-black pixels."""
function lum_histogram(img; n = 256)
    hcount = zeros(Int, n)
    for c in img[1:max(1, length(img) ÷ 500_000):end]
        l = luminance(c)
        l > 0 && (hcount[clamp(floor(Int, l * n) + 1, 1, n)] += 1)
    end
    return hcount
end

# --------------------------------------------------------------------------
# Features: Harris corners spread over a grid + BRIEF binary descriptors
# --------------------------------------------------------------------------

const PATCH = 13                                # descriptor sampling radius (px)
const Desc = NTuple{4,UInt64}                   # 256-bit descriptor

# Fixed random point pairs inside the patch (BRIEF sampling pattern).
const PAIRS = let rng = MersenneTwister(20240607), P = NTuple{4,Int}[]
    while length(P) < 256
        x1, y1, x2, y2 = round.(Int, 6.0 .* randn(rng, 4))
        x1^2 + y1^2 <= PATCH^2 && x2^2 + y2^2 <= PATCH^2 && (x1, y1) != (x2, y2) &&
            push!(P, (x1, y1, x2, y2))
    end
    P
end

@inline hamming(a::Desc, b::Desc) = count_ones(a[1] ⊻ b[1]) + count_ones(a[2] ⊻ b[2]) +
                                    count_ones(a[3] ⊻ b[3]) + count_ones(a[4] ⊻ b[4])

"""Stretch to the 1st–99th percentile range (from a subsample, for speed)."""
function stretch(g)
    s = g[1:7:end]
    lo, hi = quantile(s, (0.01, 0.99))
    hi > lo || return g
    return clamp.((g .- lo) ./ (hi - lo), 0f0, 1f0)
end

function describe(S, i, j, oriented)
    c, s = 1.0, 0.0
    if oriented                                 # intensity-centroid orientation
        m10 = m01 = 0.0
        for dy in -PATCH:PATCH, dx in -PATCH:PATCH
            dx^2 + dy^2 <= PATCH^2 || continue
            v = S[i+dy, j+dx]
            m10 += dx * v; m01 += dy * v
        end
        θ = atan(m01, m10); c, s = cos(θ), sin(θ)
    end
    w = zeros(UInt64, 4)
    for (k, (x1, y1, x2, y2)) in enumerate(PAIRS)
        if oriented
            x1, y1 = round(Int, c * x1 - s * y1), round(Int, s * x1 + c * y1)
            x2, y2 = round(Int, c * x2 - s * y2), round(Int, s * x2 + c * y2)
        end
        S[i+y1, j+x1] < S[i+y2, j+x2] && (w[(k-1)>>6+1] |= UInt64(1) << ((k - 1) & 63))
    end
    return (w[1], w[2], w[3], w[4])
end

"""
Corner keypoints and descriptors.
- `scale`: detect on a copy resized by this factor (faster, less noise)
- `nkeypoints`: at most this many, spread evenly over a grid of the image
- `threshold`: minimum corner strength as a fraction of the strongest corner
- `denoise`: Gaussian blur σ (px) against sensor noise (night shots, grass, sky)
- `oriented`: rotation-invariant descriptors (only needed for rolled frames)
Returns (2×N full-resolution points (x, y), descriptors).
"""
function detect(img; scale = 0.5, nkeypoints = 5000, threshold = 0.001, denoise = 1.5,
                oriented = false)
    g = luminance(img)
    scale < 1 && (g = resize(g, scale))
    g = stretch(blur(g, denoise))
    h, w = size(g)
    gx = zeros(Float32, h, w); gy = zeros(Float32, h, w)
    Threads.@threads for j in 2:w-1
        for i in 2:h-1
            gx[i, j] = (g[i, j+1] - g[i, j-1]) / 2
            gy[i, j] = (g[i+1, j] - g[i-1, j]) / 2
        end
    end
    xx = blur(gx .* gx, 1.5); yy = blur(gy .* gy, 1.5); xy = blur(gx .* gy, 1.5)
    R = @. xx * yy - xy^2 - 0.04f0 * (xx + yy)^2
    m = PATCH + 3
    (h > 2m && w > 2m) || return zeros(2, 0), Desc[]
    thr = threshold * maximum(R)
    # Local maxima in 5×5 neighbourhoods.
    cands = pcollect(m+1:w-m, Tuple{Float32,Int,Int}) do cols
        out = Tuple{Float32,Int,Int}[]
        for j in cols, i in m+1:h-m
            r = R[i, j]
            r > thr || continue
            ismax = true
            for dj in -2:2, di in -2:2
                (di == 0 && dj == 0) && continue
                if R[i+di, j+dj] > r || (R[i+di, j+dj] == r && (dj, di) < (0, 0))
                    ismax = false; break
                end
            end
            ismax && push!(out, (r, i, j))
        end
        out
    end
    sort!(cands; rev = true)
    # Even spread: each grid cell gets a quota first, then the strongest of the rest.
    gxn = 16; gyn = max(1, round(Int, 16h / w))
    quota = cld(nkeypoints, gxn * gyn)
    used = zeros(Int, gyn, gxn)
    chosen = Tuple{Int,Int}[]; rest = Tuple{Int,Int}[]
    for (_, i, j) in cands
        ci = min(gyn, 1 + (i - 1) * gyn ÷ h); cj = min(gxn, 1 + (j - 1) * gxn ÷ w)
        if used[ci, cj] < quota && length(chosen) < nkeypoints
            used[ci, cj] += 1; push!(chosen, (i, j))
        else
            push!(rest, (i, j))
        end
    end
    append!(chosen, rest[1:min(end, nkeypoints - length(chosen))])
    S = blur(g, 1.2)                            # smoothing for stable descriptor bits
    n = length(chosen)
    pts = zeros(2, n); desc = Vector{Desc}(undef, n)
    Threads.@threads for k in 1:n
        i, j = chosen[k]
        # Sub-pixel position from a parabola through the corner response.
        sub(a, b, c) = (d = a - 2b + c; d < 0 ? clamp((a - c) / (2d), -0.5, 0.5) : 0.0)
        pts[1, k] = to_full(j + sub(R[i, j-1], R[i, j], R[i, j+1]), scale)
        pts[2, k] = to_full(i + sub(R[i-1, j], R[i, j], R[i+1, j]), scale)
        desc[k] = describe(S, i, j, oriented)
    end
    return pts, desc
end

"""Index of the nearest descriptor in `ys` to `x`, with the best and second-best distances."""
function nearest2(x, ys)
    d1, d2, j1 = typemax(Int), typemax(Int), 0
    for (j, y) in enumerate(ys)
        d = hamming(x, y)
        d < d1 ? (d2 = d1; d1 = d; j1 = j) : d < d2 && (d2 = d)
    end
    return d1, d2, j1
end

"""
Match two feature sets from `detect`; returns 2×M full-resolution point arrays.
A match must be mutual (each is the other's nearest neighbour), clearly better
than the runner-up (best < `ratio` × second best, Lowe's test), and closer than
`max_distance` (fraction of descriptor bits that differ).
"""
function match_pair(f1, f2; ratio = 0.8, max_distance = 0.35)
    pts1, a = f1
    pts2, b = f2
    (isempty(a) || isempty(b)) && return zeros(2, 0), zeros(2, 0)
    back = pcollect(r -> [nearest2(b[k], a)[3] for k in r], eachindex(b), Int)
    m = pcollect(eachindex(a), Tuple{Int,Int}) do r
        out = Tuple{Int,Int}[]
        for i in r
            d1, d2, j = nearest2(a[i], b)
            d1 < ratio * d2 && d1 <= max_distance * 256 && back[j] == i && push!(out, (i, j))
        end
        out
    end
    return pts1[:, first.(m)], pts2[:, last.(m)]
end

# --------------------------------------------------------------------------
# Motion models (normalised DLT for homographies, least squares otherwise) + RANSAC
# --------------------------------------------------------------------------

@inline function apply_h(H, x, y)
    w = H[3, 1] * x + H[3, 2] * y + H[3, 3]
    return (H[1, 1] * x + H[1, 2] * y + H[1, 3]) / w,
           (H[2, 1] * x + H[2, 2] * y + H[2, 3]) / w
end

translation(dx, dy) = [1.0 0 dx; 0 1 dy; 0 0 1]

function normalizer(P)
    c = mean(P; dims = 2)
    d = mean(sqrt.(sum((P .- c) .^ 2; dims = 1)))
    s = d > 0 ? sqrt(2) / d : 1.0
    return [s 0 -s*c[1]; 0 s -s*c[2]; 0 0 1]
end

"""Homography H with H * src ≈ dst (both 2×N, N ≥ 4)."""
function dlt(src, dst)
    Ts, Td = normalizer(src), normalizer(dst)
    n = size(src, 2)
    A = zeros(2n, 9)
    for i in 1:n
        x, y = apply_h(Ts, src[1, i], src[2, i])
        u, v = apply_h(Td, dst[1, i], dst[2, i])
        A[2i-1, :] = [-x, -y, -1, 0, 0, 0, u * x, u * y, u]
        A[2i, :]   = [0, 0, 0, -x, -y, -1, v * x, v * y, v]
    end
    h = svd(A; full = true).V[:, end]        # null vector of A
    H = inv(Td) * permutedims(reshape(h, 3, 3)) * Ts
    return H ./ H[3, 3]
end

"""Minimum number of correspondences that determine `model`."""
min_samples(model) = model === :translation ? 1 : model === :similarity ? 2 :
                     model === :affine ? 3 : 4

"""Least-squares fit of `model` (see MODELS) with H * src ≈ dst."""
function fit_model(model, src, dst)
    model === :homography && return dlt(src, dst)
    cs, cd = mean(src; dims = 2), mean(dst; dims = 2)
    if model === :translation
        return translation(cd[1] - cs[1], cd[2] - cs[2])
    end
    s, d = src .- cs, dst .- cd                # centred for conditioning
    if model === :similarity                   # [a -b; b a]: rotation + uniform scale
        n = size(s, 2)
        A = zeros(2n, 2)
        for i in 1:n
            A[2i-1, :] = [s[1, i], -s[2, i]]
            A[2i, :]   = [s[2, i],  s[1, i]]
        end
        a, b = A \ vec(d)
        M = [a -b; b a]
    else                                       # :affine
        M = d / s
    end
    t = vec(cd) - M * vec(cs)
    return [M t; 0 0 1]
end

function reproj_err(H, src, dst, i)
    x, y = apply_h(H, src[1, i], src[2, i])
    return hypot(x - dst[1, i], y - dst[2, i])
end

inliers(H, src, dst, thresh) = [i for i in axes(src, 2) if reproj_err(H, src, dst, i) < thresh]

"""
RANSAC fit of `model` with H * src ≈ dst. `thresh` is the inlier distance in
full-resolution pixels. Returns (H, inlier indices).
"""
function ransac(src, dst; model = :homography, iters = 2000, thresh = 5.0,
                rng = MersenneTwister(1))
    n, k = size(src, 2), min_samples(model)
    need = max(k + 2, 4)
    n >= need || error("Only $n matches; need ≥ $need. Images may not overlap enough.")
    best = Int[]
    for _ in 1:iters
        idx = randperm(rng, n)[1:k]
        H = try fit_model(model, src[:, idx], dst[:, idx]) catch; continue end
        all(isfinite, H) || continue
        inl = inliers(H, src, dst, thresh)
        length(inl) > length(best) && (best = inl)
    end
    length(best) >= need || error("RANSAC found only $(length(best)) inliers; images may not overlap.")
    H = fit_model(model, src[:, best], dst[:, best])
    best = inliers(H, src, dst, thresh)          # refit once on the refined inlier set
    return fit_model(model, src[:, best], dst[:, best]), best
end

# --------------------------------------------------------------------------
# Placement
# --------------------------------------------------------------------------

"""
Pairs of images (i < j) to try matching:
- `:sequential`: each image with the next one (a single row)
- `:grid`: neighbours (right, below, diagonals) in rows of `cols` images; with
  `snake = true` every second row runs right-to-left
- `:all`: every pair (when the layout is unknown)
"""
function candidate_pairs(n, mode; cols = n, snake = false)
    mode === :all && return [(i, j) for i in 1:n for j in i+1:n]
    mode === :sequential && return [(i, i + 1) for i in 1:n-1]
    cols = clamp(cols, 1, n)
    function pos(k)
        r, c = divrem(k - 1, cols)
        return (r, snake && isodd(r) ? cols - 1 - c : c)
    end
    at = Dict(pos(k) => k for k in 1:n)
    out = Tuple{Int,Int}[]
    for k in 1:n
        r, c = pos(k)
        for (dr, dc) in ((0, 1), (1, 0), (1, 1), (1, -1))
            j = get(at, (r + dr, c + dc), 0)
            j > 0 && push!(out, minmax(k, j))
        end
    end
    return unique(out)
end

"""
Is a RANSAC result a real overlap? Needs at least `min_inliers` inliers, more
than 8 + 0.2 × matches (random matches between unrelated photos rarely agree),
and a plausible transform (see `plausible`). Returns (accepted, reason).
"""
function accept_pair(nmatches, ninliers, H, h, w; min_inliers = 20)
    ninliers >= min_inliers || return false, "only $ninliers inliers (need $min_inliers)"
    ninliers >= 8 + 0.2 * nmatches ||
        return false, "$ninliers inliers of $nmatches matches is too few to be a real overlap"
    plausible(H, h, w) || return false, "the transform folds or strongly distorts the image"
    return true, ""
end

"""
Could H come from rotating the camera? The mapped h×w image must stay a convex,
non-flipped quadrilateral whose area is between ¼ and 4× the original.
"""
function plausible(H, h, w)
    q = [H * [x, y, 1.0] for (x, y) in ((1, 1), (w, 1), (w, h), (1, h))]
    all(v -> v[3] > 0, q) || return false
    c = [(v[1] / v[3], v[2] / v[3]) for v in q]
    cross(k) = ((a, b, d) = (c[k], c[mod1(k + 1, 4)], c[mod1(k + 2, 4)]);
                (b[1] - a[1]) * (d[2] - b[2]) - (b[2] - a[2]) * (d[1] - b[1]))
    all(k -> cross(k) > 0, 1:4) || return false          # convex, same orientation
    area = 0.5 * abs(sum(c[k][1] * c[mod1(k + 1, 4)][2] - c[mod1(k + 1, 4)][1] * c[k][2] for k in 1:4))
    return 0.25 < area / (w * h) < 4
end

"""
Maximum spanning tree over accepted pairs `edges` = [(i, j, weight)], so every
image is reached through its strongest connections. Returns (adjacency lists).
"""
function spanning_tree(n, edges)
    uf = collect(1:n)
    find(x) = (while uf[x] != x; uf[x] = uf[uf[x]]; x = uf[x]; end; x)
    adj = [Int[] for _ in 1:n]
    for (i, j, _) in sort(edges; by = e -> -e[3])
        a, b = find(i), find(j)
        a == b && continue
        uf[a] = b
        push!(adj[i], j); push!(adj[j], i)
    end
    return adj
end

"""Breadth-first walk of tree `adj` from `roots` (in order): (parent, order, component)."""
function walk_tree(adj, roots)
    n = length(adj)
    parent = zeros(Int, n); comp = zeros(Int, n); order = Int[]
    c = 0
    for r in roots
        comp[r] == 0 || continue
        c += 1; comp[r] = c
        queue = [r]
        while !isempty(queue)
            v = popfirst!(queue); push!(order, v)
            for u in adj[v]
                comp[u] == 0 || continue
                comp[u] = c; parent[u] = v; push!(queue, u)
            end
        end
    end
    return parent, order, comp
end

"""Best reference image: the centre (fewest hops to the farthest image) of the largest group."""
function tree_center(adj)
    n = length(adj)
    _, _, comp = walk_tree(adj, 1:n)
    big = argmax([count(==(c), comp) for c in 1:maximum(comp)])
    best, beste = 0, typemax(Int)
    for v in 1:n
        comp[v] == big || continue
        p, order, _ = walk_tree(adj, [v])
        depth = Dict(v => 0)
        for u in order
            u == v || (depth[u] = depth[p[u]] + 1)
        end
        e = maximum(values(depth))
        e < beste && ((best, beste) = (v, e))
    end
    return best
end

"""
Transforms of all images into the frame of `ref`, following the tree
(parent, order) from `walk_tree`. `H[(i, j)]` maps image j into image i.
Images not connected to `ref` are placed side by side to its right.
Each result is scaled to determinant 1 so that the sign of its homogeneous
coordinate tells front from back.
"""
function place(H, parent, order, ref, widths)
    n = length(parent)
    G = Vector{Matrix{Float64}}(undef, n)
    x = 1.05 * widths[ref]
    for v in order
        p = parent[v]
        if p == 0
            v == ref ? (G[v] = Matrix{Float64}(I, 3, 3)) :
                       (G[v] = translation(x, 0); x += 1.05 * widths[v])
        else
            G[v] = haskey(H, (p, v)) ? G[p] * H[(p, v)] : G[p] * inv(H[(v, p)])
        end
    end
    return [Gi ./ cbrt(det(Gi)) for Gi in G]
end

"""
Focal length (px) implied by homographies of a rotating camera (Szeliski's
method, as in OpenCV). `pairs` holds (H, c_i, c_j) with H mapping image j into
image i and c the image centres. Returns `nothing` if no pair gives an estimate.
"""
function estimate_focal(pairs)
    est = Float64[]
    for (H, c1, c2) in pairs
        h = vec(permutedims(translation(-c1[1], -c1[2]) * H * translation(c2...)))  # row-major
        h ./= h[9]
        f1 = let d1 = h[7] * h[8], d2 = (h[8] - h[7]) * (h[8] + h[7])
            v1 = d1 != 0 ? -(h[1] * h[2] + h[4] * h[5]) / d1 : -1.0
            v2 = d2 != 0 ? (h[1]^2 + h[4]^2 - h[2]^2 - h[5]^2) / d2 : -1.0
            v1 < v2 && ((v1, v2) = (v2, v1))
            v1 > 0 && v2 > 0 ? sqrt(abs(d1) > abs(d2) ? v1 : v2) : v1 > 0 ? sqrt(v1) : NaN
        end
        f0 = let d1 = h[1] * h[4] + h[2] * h[5], d2 = h[1]^2 + h[2]^2 - h[4]^2 - h[5]^2
            v1 = d1 != 0 ? -h[3] * h[6] / d1 : -1.0
            v2 = d2 != 0 ? (h[6]^2 - h[3]^2) / d2 : -1.0
            v1 < v2 && ((v1, v2) = (v2, v1))
            v1 > 0 && v2 > 0 ? sqrt(abs(d1) > abs(d2) ? v1 : v2) : v1 > 0 ? sqrt(v1) : NaN
        end
        f = sqrt(f0 * f1)
        isfinite(f) && f > 0 && push!(est, f)
    end
    return isempty(est) ? nothing : median(est)
end

# --------------------------------------------------------------------------
# Global refinement (bundle adjustment of a rotating camera)
# --------------------------------------------------------------------------

skew(w) = [0 -w[3] w[2]; w[3] 0 -w[1]; -w[2] w[1] 0]

"""Rotation matrix for the axis-angle vector `w`."""
function rodrigues(w)
    θ = norm(w)
    θ < 1e-12 && return Matrix{Float64}(I, 3, 3) + skew(w)
    K = skew(w ./ θ)
    return I + sin(θ) * K + (1 - cos(θ)) * K * K
end

"""Closest rotation matrix to M."""
function nearest_rotation(M)
    F = svd(M)
    R = F.U * F.Vt
    det(R) < 0 && (R = F.U * Diagonal([1, 1, -1]) * F.Vt)
    return R
end

kmat(f, c) = [f 0 c[1]; 0 f c[2]; 0 0 1]

"""
Refine all images together, assuming the camera only rotated and kept its
focal length. Unlike chaining pairs one after another (where errors add up and
loops between rows never close), this uses every accepted pair at once and
spreads the error evenly, which removes most ghosting.

- `G`: initial transforms (image → reference plane), e.g. from `place`
- `members`: images to refine (those connected to `ref`)
- `matches`: (i, j, Pi, Pj) inlier points of each accepted pair (2×m arrays)
- `huber`: residuals above this (px) count less, so stray matches cannot dominate

With `distortion = true` it also estimates the lens's radial distortion k1
(see `distort`), which matters for wide-angle lenses.

Returns (G, f, k1, rms_before, rms_after, median error in px of each entry of
`matches`, NaN for pairs that were not used).
"""
function bundle_adjust(G, centers, ref, f, members, matches; iters = 40, huber = 3.0,
                       maxpts = 60, distortion = true)
    n = length(G)
    inset = falses(n); inset[members] .= true
    sub(P) = P[:, round.(Int, range(1, size(P, 2), min(maxpts, size(P, 2))))]
    used = [k for (k, (i, j, Pi, _)) in enumerate(matches) if inset[i] && inset[j] && size(Pi, 2) > 0]
    E = [(matches[k][1], matches[k][2], sub(matches[k][3]), sub(matches[k][4])) for k in used]
    isempty(E) && return G, f, 0.0, NaN, NaN, fill(NaN, length(matches))
    # Camera rotations from the initial transforms: G_i = K_ref R_iᵀ K_i⁻¹.
    R0 = [inset[i] ? nearest_rotation(inv(kmat(f, centers[ref])) * G[i] * kmat(f, centers[i]))' :
          Matrix{Float64}(I, 3, 3) for i in 1:n]
    free = [i for i in members if i != ref]               # the reference stays fixed
    col = Dict(i => 3k - 2 for (k, i) in enumerate(free))
    P = 3 * length(free) + 2                              # + log focal length, k1
    rot(w, i) = haskey(col, i) ? rodrigues(w[col[i]:col[i]+2]) * R0[i] : R0[i]
    function resid(Ri, Rj, fl, k1, e)
        i, j, Pi, Pj = e
        H = kmat(fl, centers[j]) * Rj * Ri' * inv(kmat(fl, centers[i]))   # image i → image j
        lens = (k1, fl)
        r = similar(Pi)
        for k in axes(Pi, 2)
            x, y = apply_h(H, undistort(Pi[1, k], Pi[2, k], centers[i], lens)...)
            u, v = undistort(Pj[1, k], Pj[2, k], centers[j], lens)
            r[1, k] = x - u; r[2, k] = y - v
        end
        return r
    end
    ρ(d) = d <= huber ? d^2 : 2huber * d - huber^2
    cost(w, lf, k1) = sum(sum(ρ(hypot(r[1, k], r[2, k])) for k in axes(r, 2))
                          for r in (resid(rot(w, e[1]), rot(w, e[2]), exp(lf), k1, e) for e in E))
    rms(w, lf, k1) = sqrt(mean(vcat([vec(sum(abs2, resid(rot(w, e[1]), rot(w, e[2]), exp(lf), k1, e); dims = 1))
                                     for e in E]...)))
    w = zeros(P - 2); lf = log(f); k1 = 0.0
    rms0 = rms(w, lf, k1)
    c0 = cost(w, lf, k1); λ = 1e-3
    for _ in 1:iters
        JtJ = zeros(P, P); Jtr = zeros(P)
        for e in E
            i, j = e[1], e[2]
            Ri, Rj, fl = rot(w, i), rot(w, j), exp(lf)
            r = resid(Ri, Rj, fl, k1, e)
            # Robust (Huber) weight per point.
            wt = [(d = hypot(r[1, k], r[2, k]); d <= huber ? 1.0 : huber / d) for k in axes(r, 2)]
            W = repeat(wt'; inner = (2, 1))
            cols = Int[]; Js = Vector{Float64}[]
            for (img, Rimg) in ((i, Ri), (j, Rj))
                haskey(col, img) || continue
                for a in 1:3
                    w2 = copy(w); w2[col[img]+a-1] += 1e-6
                    R2 = rot(w2, img)
                    r2 = img == i ? resid(R2, Rj, fl, k1, e) : resid(Ri, R2, fl, k1, e)
                    push!(cols, col[img] + a - 1); push!(Js, vec((r2 .- r) ./ 1e-6))
                end
            end
            r2 = resid(Ri, Rj, exp(lf + 1e-6), k1, e)
            push!(cols, P - 1); push!(Js, vec((r2 .- r) ./ 1e-6))
            if distortion
                r2 = resid(Ri, Rj, fl, k1 + 1e-6, e)
                push!(cols, P); push!(Js, vec((r2 .- r) ./ 1e-6))
            end
            J = reduce(hcat, Js)
            JW = J .* vec(W)
            JtJ[cols, cols] .+= J' * JW
            Jtr[cols] .+= JW' * vec(r)
        end
        improved = false
        for _ in 1:10
            δ = -(JtJ + λ * Diagonal(diag(JtJ) .+ 1e-9)) \ Jtr
            wn, lfn, k1n = w .+ δ[1:end-2], lf + δ[end-1], clamp(k1 + δ[end], -0.5, 0.5)
            cn = cost(wn, lfn, k1n)
            if cn < c0
                rel = (c0 - cn) / c0
                w, lf, k1, c0 = wn, lfn, k1n, cn
                λ = max(λ / 3, 1e-9); improved = true
                rel < 1e-7 && (improved = false)
                break
            end
            λ *= 4
        end
        improved || break
    end
    fn = exp(lf)
    Gn = copy(G)
    for i in members
        Gn[i] = kmat(fn, centers[ref]) * rot(w, i)' * inv(kmat(fn, centers[i]))
        Gn[i] ./= cbrt(det(Gn[i]))
    end
    med = fill(NaN, length(matches))
    for (k, e) in zip(used, E)
        med[k] = median(vec(sqrt.(sum(abs2, resid(rot(w, e[1]), rot(w, e[2]), fn, k1, e); dims = 1))))
    end
    return Gn, fn, k1, rms0, rms(w, lf, k1), med
end

"""
Camera rotation (yaw right, pitch down, roll clockwise; radians) as a homography
of the reference image plane.
"""
function rotation_h(yaw, pitch, f, c; roll = 0.0)
    K = [f 0 c[1]; 0 f c[2]; 0 0 1]
    Ry = [cos(yaw) 0 sin(yaw); 0 1 0; -sin(yaw) 0 cos(yaw)]
    Rx = [1 0 0; 0 cos(pitch) sin(pitch); 0 -sin(pitch) cos(pitch)]
    Rz = [cos(roll) -sin(roll) 0; sin(roll) cos(roll) 0; 0 0 1]
    return K * Rz * Ry * Rx * inv(K)
end

# --------------------------------------------------------------------------
# Projections
#
# A viewing direction (X, Y, Z) of the reference camera (x right, y down, z
# forward) maps to unit-free coordinates (a, b); the output pixel grid is
# (u, v) = f·(a, b), so one unit ≈ one reference pixel near the centre.
# --------------------------------------------------------------------------

# Largest |a|, |b| each projection can show (keeps canvases finite).
function proj_limits(p)
    p === :rectilinear && return (4.0, 4.0)
    p === :cylindrical && return (π, 4.0)
    p === :spherical && return (π, π / 2)
    p === :mercator && return (π, 3.0)
    p === :transverse_cylindrical && return (4.0, π)
    p === :transverse_mercator && return (3.0, π)
    p === :fisheye && return (π, π)
    p === :fisheye_equisolid && return (2.0, 2.0)
    p === :orthographic && return (1.0, 1.0)
    p in (:hammer, :mollweide) && return (2sqrt(2), sqrt(2))
    p in (:little_planet, :tunnel) && return (8.0, 8.0)
    return (4.0, 4.0)                                      # stereographic
end

# Stereographic projection of a direction around the +Z axis.
@inline function stereo(X, Y, Z, zmin)
    n = sqrt(X^2 + Y^2 + Z^2)
    Z / n <= zmin && return NaN, NaN
    k = 2 / (1 + Z / n)
    return k * X / n, k * Y / n
end

# Mollweide auxiliary angle θ with 2θ + sin 2θ = π sin φ (Newton's method).
@inline function mollweide_theta(φ)
    abs(φ) >= π / 2 - 1e-9 && return copysign(π / 2, φ)
    θ = φ
    for _ in 1:30
        d = (2θ + sin(2θ) - π * sin(φ)) / (2 + 2cos(2θ))
        θ -= d
        abs(d) < 1e-10 && break
    end
    return θ
end

"""
Direction → (a, b); NaN if the projection cannot show it. x points right, y
down, z forward; `little_planet` puts the ground (+y) in the centre, `tunnel`
the sky (-y).
"""
@inline function proj_forward(p, X, Y, Z)
    if p === :rectilinear
        Z <= 1e-9 && return NaN, NaN
        a, b = X / Z, Y / Z
    elseif p === :stereographic
        a, b = stereo(X, Y, Z, -0.9)
    elseif p === :little_planet                            # nadir in the centre
        a, b = stereo(X, -Z, Y, -0.97)
    elseif p === :tunnel                                   # zenith in the centre
        a, b = stereo(X, Z, -Y, -0.97)
    elseif p === :orthographic
        n = sqrt(X^2 + Y^2 + Z^2)
        Z <= 0 && return NaN, NaN
        a, b = X / n, Y / n
    elseif p === :fisheye || p === :fisheye_equisolid
        n = sqrt(X^2 + Y^2 + Z^2)
        θ = acos(clamp(Z / n, -1.0, 1.0))                  # angle from the view axis
        r = p === :fisheye ? θ : 2sin(θ / 2)
        q = hypot(X, Y)
        a, b = q < 1e-12 ? (0.0, 0.0) : (r * X / q, r * Y / q)
    elseif p === :transverse_cylindrical || p === :transverse_mercator
        b = atan(Y, Z); ρ = hypot(Y, Z)                    # the cylinder lies horizontally
        a = p === :transverse_cylindrical ? X / ρ : asinh(X / ρ)
    elseif p === :hammer || p === :mollweide
        λ = atan(X, Z); φ = atan(Y, hypot(X, Z))           # longitude, latitude (down positive)
        if p === :hammer
            z = sqrt(1 + cos(φ) * cos(λ / 2))
            a, b = 2sqrt(2) * cos(φ) * sin(λ / 2) / z, sqrt(2) * sin(φ) / z
        else
            θ = mollweide_theta(φ)
            a, b = 2sqrt(2) / π * λ * cos(θ), sqrt(2) * sin(θ)
        end
    else
        a = atan(X, Z); ρ = hypot(X, Z)
        b = p === :cylindrical ? Y / ρ : p === :spherical ? atan(Y, ρ) : asinh(Y / ρ)
    end
    la, lb = proj_limits(p)
    return clamp(a, -la, la), clamp(b, -lb, lb)
end

"""(a, b) → direction (X, Y, Z); `nothing` outside the projection's range."""
@inline function proj_inverse(p, a, b)
    la, lb = proj_limits(p)
    (abs(a) > la || abs(b) > lb) && return nothing
    if p === :rectilinear
        return (a, b, 1.0)
    elseif p === :stereographic || p === :little_planet || p === :tunnel
        r2 = a^2 + b^2
        X, Y, Z = 4a, 4b, 4 - r2                           # around the projection's own axis
        return p === :stereographic ? (X, Y, Z) : p === :little_planet ? (X, Z, -Y) : (X, -Z, Y)
    elseif p === :orthographic
        r2 = a^2 + b^2
        r2 > 1 && return nothing
        return (a, b, sqrt(1 - r2))
    elseif p === :fisheye || p === :fisheye_equisolid
        r = hypot(a, b)
        p === :fisheye_equisolid && r > 2 && return nothing
        θ = p === :fisheye ? r : 2asin(r / 2)
        θ > π && return nothing
        r < 1e-12 && return (0.0, 0.0, 1.0)
        return (sin(θ) * a / r, sin(θ) * b / r, cos(θ))
    elseif p === :transverse_cylindrical
        return (a, sin(b), cos(b))
    elseif p === :transverse_mercator
        return (sinh(a), sin(b), cos(b))
    elseif p === :hammer
        z2 = 1 - (a / 4)^2 - (b / 2)^2
        (z2 < 0.5 || a^2 / 8 + b^2 / 2 > 1) && return nothing   # outside the ellipse
        z = sqrt(z2)
        λ = 2atan(z * a, 2(2z2 - 1)); φ = asin(clamp(z * b, -1.0, 1.0))
        return (sin(λ) * cos(φ), sin(φ), cos(λ) * cos(φ))
    elseif p === :mollweide
        abs(b) > sqrt(2) && return nothing
        θ = asin(b / sqrt(2))
        abs(cos(θ)) < 1e-12 && return nothing
        λ = π * a / (2sqrt(2) * cos(θ))
        abs(λ) > π && return nothing
        φ = asin(clamp((2θ + sin(2θ)) / π, -1.0, 1.0))
        return (sin(λ) * cos(φ), sin(φ), cos(λ) * cos(φ))
    elseif p === :cylindrical
        return (sin(a), b, cos(a))
    else
        φ = p === :spherical ? b : atan(sinh(b))
        return (sin(a) * cos(φ), sin(φ), cos(a) * cos(φ))
    end
end

# --------------------------------------------------------------------------
# Lens distortion
#
# Radial (barrel / pincushion) distortion, as wide-angle lenses show it:
# observed = c + (ideal - c)·(1 + k1·r²), with r the distance from the image
# centre c in units of the camera focal length fc. `lens = (k1, fc)`.
# --------------------------------------------------------------------------

"""Ideal (distortion-free) pixel → observed pixel."""
@inline function distort(x, y, c, lens)
    k1, fc = lens
    k1 == 0 && return x, y
    dx, dy = (x - c[1]) / fc, (y - c[2]) / fc
    s = 1 + k1 * (dx^2 + dy^2)
    return c[1] + dx * s * fc, c[2] + dy * s * fc
end

"""Observed pixel → ideal pixel (inverse of `distort`)."""
@inline function undistort(x, y, c, lens)
    k1, fc = lens
    k1 == 0 && return x, y
    ox, oy = (x - c[1]) / fc, (y - c[2]) / fc
    ux, uy = ox, oy
    for _ in 1:6
        s = 1 + k1 * (ux^2 + uy^2)
        ux, uy = ox / s, oy / s
    end
    return c[1] + ux * fc, c[2] + uy * fc
end

"""
Outline of an h×w image placed by G (image → reference plane), in output (u, v)
coordinates: `n` points per edge, NaN where the projection cannot show it.
"""
function outline(G, h, w, p, f, c; lens = (0.0, 1.0), n = 24)
    ci = ((w + 1) / 2, (h + 1) / 2)
    xs = [range(1, w, n); fill(w, n); range(w, 1, n); fill(1, n)]
    ys = [fill(1, n); range(1, h, n); fill(h, n); range(h, 1, n)]
    return map(zip(xs, ys)) do (x, y)
        q = G * [undistort(x, y, ci, lens)..., 1.0]
        a, b = proj_forward(p, (q[1] - c[1] * q[3]) / f, (q[2] - c[2] * q[3]) / f, q[3])
        (f * a, f * b)
    end
end

"""Bounding box (umin, umax, vmin, vmax) of outlines (from `outline`)."""
function bounds(outlines)
    pts = [pt for o in outlines for pt in o if all(isfinite, pt)]
    isempty(pts) && return (-1.0, 1.0, -1.0, 1.0)
    return floor(minimum(first, pts)), ceil(maximum(first, pts)),
           floor(minimum(last, pts)),  ceil(maximum(last, pts))
end

"""
Per-image brightness gains that make overlaps agree, propagated along the tree
(parent, order) from `walk_tree`. `points[(p, v)]` holds the inlier points
(in image p, in image v) of the pair linking image v to its parent p.
"""
function estimate_gains(imgs, parent, order, points)
    # Mean luminance of 7×7 patches around the points (tolerates small position errors).
    function lum(img, P)
        h, w = size(img)
        mean(Float64(luminance(img[clamp(round(Int, P[2, k]) + dy, 1, h), clamp(round(Int, P[1, k]) + dx, 1, w)]))
             for k in axes(P, 2), dy in -3:3, dx in -3:3)
    end
    g = ones(length(imgs))
    for v in order
        p = parent[v]
        (p == 0 || !haskey(points, (p, v))) && continue
        Pp, Pv = points[(p, v)]
        size(Pp, 2) == 0 && (g[v] = g[p]; continue)
        g[v] = g[p] * lum(imgs[p], Pp) / max(lum(imgs[v], Pv), 1e-6)
    end
    return g
end

# --------------------------------------------------------------------------
# Rendering
# --------------------------------------------------------------------------

"""
Render images into the output box (umin, umax, vmin, vmax) at `scale` with
projection `projection`, focal length `f` and reference centre `c`.
Output pixel centre px ↔ u = (px-0.5)/scale + umin - 0.5.

- `G[i]`: maps (distortion-free) full-resolution pixels of image i into the
  reference image plane
- `imgs[i]`: the pixels to sample, which may be a copy of image i resized by
  `scales[i]`; `sizes[i]` is the full-resolution size (h, w)
- `lens`: radial distortion (k1, camera focal length), see `distort`
- `sharpness`: feathering weight = (distance to the border)^sharpness; higher
  values narrow the transition between photos (less ghosting, harder joins)
- `blend`: `:feather` mixes overlapping photos smoothly; `:seam` takes each pixel
  from the most central photo; `:smart` feathers where the photos agree and
  switches to the most central photo where they disagree (moving objects,
  parallax), which removes double images ("ghosts"); `:average` is a plain mean
- `deghost`: relative brightness difference above which `:smart` treats
  overlapping photos as disagreeing
- `coverage = true` returns (image, mask of the output pixels any photo covers)
"""
function compose(imgs, G, box; scale = 1.0, projection = :rectilinear, f = 1000.0,
                 c = (0.0, 0.0), blend = :feather, gains = ones(length(imgs)),
                 scales = ones(length(imgs)), sizes = size.(imgs), lens = (0.0, 1.0),
                 sharpness = 1.0, deghost = 0.12, coverage = false)
    umin, umax, vmin, vmax = box
    Wc = max(1, round(Int, (umax - umin + 1) * scale))
    Hc = max(1, round(Int, (vmax - vmin + 1) * scale))
    K = [f 0 c[1]; 0 f c[2]; 0 0 1]
    Ms = [inv(Gi) * K for Gi in G]                       # direction → ideal full-res pixel
    p32 = Float32(sharpness)

    # Lens distortion is only meaningful within a photo: far outside it the
    # model folds back and would map distant directions onto the photo's border
    # (thin curved streaks). So only directions within each photo's own extent
    # (its undistorted corners, plus a small margin) may sample it.
    rlim2 = map(sizes) do (h, w)
        ci = ((w + 1) / 2, (h + 1) / 2)
        corners = [undistort(x, y, ci, lens) for (x, y) in ((1, 1), (w, 1), (1, h), (w, h))]
        1.05^2 * maximum(((ux - ci[1])^2 + (uy - ci[2])^2) / lens[2]^2 for (ux, uy) in corners)
    end

    # Colour, blending weight and distance from the border (full-resolution px)
    # of image i at output pixel (x, y), or nothing.
    function sample(i, x, y)
        d = proj_inverse(projection, ((x - 0.5) / scale + umin - 0.5) / f,
                                     ((y - 0.5) / scale + vmin - 0.5) / f)
        d === nothing && return nothing
        X, Y, Z = d
        M = Ms[i]
        sw = M[3, 1] * X + M[3, 2] * Y + M[3, 3] * Z
        sw > 0 || return nothing                         # behind this camera
        h, w = sizes[i]
        ci = ((w + 1) / 2, (h + 1) / 2)
        ix = (M[1, 1] * X + M[1, 2] * Y + M[1, 3] * Z) / sw
        iy = (M[2, 1] * X + M[2, 2] * Y + M[2, 3] * Z) / sw
        ((ix - ci[1])^2 + (iy - ci[2])^2) / lens[2]^2 <= rlim2[i] || return nothing
        ox, oy = distort(ix, iy, ci, lens)
        (1 <= ox <= w && 1 <= oy <= h) || return nothing
        # Feathering: weight grows with distance from the image border.
        dist = min(ox, w - ox + 1, oy, h - oy + 1)
        wt = blend === :average ? 1.0f0 : (Float32(dist) / 100f0)^p32
        s = scales[i]
        return Float32(gains[i]) * bilinear(imgs[i], (oy - 0.5) * s + 0.5, (ox - 0.5) * s + 0.5), wt, dist
    end

    acc  = zeros(RGB{Float32}, Hc, Wc)
    wsum = zeros(Float32, Hc, Wc)
    smart = blend === :smart
    best  = smart ? zeros(RGB{Float32}, Hc, Wc) : acc   # most central photo's colour
    bestw = smart ? zeros(Float32, Hc, Wc) : wsum
    besti = zeros(Int32, smart ? Hc : 0, smart ? Wc : 0)
    tocanvas(u, v) = ((u - umin + 0.5) * scale + 0.5, (v - vmin + 0.5) * scale + 0.5)
    for i in eachindex(imgs)
        h, w = sizes[i]
        o = filter(pt -> all(isfinite, pt), outline(G[i], h, w, projection, f, c; lens))
        isempty(o) && continue
        cs = [tocanvas(pt...) for pt in o]
        x0 = clamp(floor(Int, minimum(first, cs)), 1, Wc); x1 = clamp(ceil(Int, maximum(first, cs)), 1, Wc)
        y0 = clamp(floor(Int, minimum(last, cs)), 1, Hc);  y1 = clamp(ceil(Int, maximum(last, cs)), 1, Hc)
        Threads.@threads for x in x0:x1
            for y in y0:y1
                r = sample(i, x, y)
                r === nothing && continue
                col, wt, _ = r
                if blend === :seam                       # most central image wins
                    wt > wsum[y, x] && (acc[y, x] = col; wsum[y, x] = wt)
                else
                    acc[y, x]  += wt * col
                    wsum[y, x] += wt
                    smart && wt > bestw[y, x] && (best[y, x] = col; bestw[y, x] = wt; besti[y, x] = i)
                end
            end
        end
    end
    done(img) = coverage ? (img, wsum .> 0) : img
    blend === :seam && return done(acc)
    out = [ws > 0 ? a / ws : zero(a) for (a, ws) in zip(acc, wsum)]
    smart || return done(out)

    # Deghosting. Where the blend differs clearly from the most central photo,
    # the overlapping photos disagree (something moved, or parallax).
    mask = map(out, best) do o, b
        lo, lb = luminance(o), luminance(b)
        d = abs(lo - lb) / (max(lo, lb) + 0.02f0)
        clamp((d - Float32(deghost)) / Float32(deghost), 0f0, 1f0)
    end
    region = blur(mask, max(1.0, 3scale)) .> 0.02f0
    # Which photo each output pixel takes its detail from: the most central one,
    # except that each connected area of disagreement comes from ONE photo (the
    # one dominating it), so a moved person or light stays whole.
    sel = copy(besti)
    stack = Tuple{Int,Int}[]
    seen = falses(Hc, Wc)
    for x0 in 1:Wc, y0 in 1:Hc
        (region[y0, x0] && !seen[y0, x0]) || continue
        votes = Dict{Int32,Float64}()
        pix = Tuple{Int,Int}[]
        seen[y0, x0] = true; push!(stack, (y0, x0))
        while !isempty(stack)
            y, x = pop!(stack); push!(pix, (y, x))
            besti[y, x] > 0 && (votes[besti[y, x]] = get(votes, besti[y, x], 0.0) + bestw[y, x])
            for (dy, dx) in ((1, 0), (-1, 0), (0, 1), (0, -1))
                yy, xx = y + dy, x + dx
                (1 <= yy <= Hc && 1 <= xx <= Wc && region[yy, xx] && !seen[yy, xx]) || continue
                seen[yy, xx] = true; push!(stack, (yy, xx))
            end
        end
        isempty(votes) && continue
        k = argmax(votes)
        # Not right up to the chosen photo's edge: near its border the usual
        # choice stays, so the switch happens inside the overlap, where the
        # multi-band blend makes it gradual instead of a hard edge.
        margin = 0.06 * minimum(sizes[k])
        for (y, x) in pix
            r = sample(k, x, y)
            r !== nothing && r[3] >= margin && (sel[y, x] = k)
        end
    end
    return done(multiband(sample, length(imgs), sel, wsum, Hc, Wc,
                          [outline(G[i], sizes[i]..., projection, f, c; lens) for i in eachindex(imgs)], tocanvas))
end

"""
Largest axis-aligned rectangle of `true` cells in `M`, as (row range, column
range), or nothing. Used for auto-crop: `M` marks the covered output pixels.
"""
function largest_rectangle(M::AbstractMatrix{Bool})
    H, W = size(M)
    heights = zeros(Int, W)
    best, bestarea = nothing, 0
    stack = Int[]
    for r in 1:H
        for c in 1:W
            heights[c] = M[r, c] ? heights[c] + 1 : 0
        end
        # Largest rectangle under the histogram `heights` (bars ending at row r).
        empty!(stack)
        for c in 1:W+1
            h = c <= W ? heights[c] : 0
            while !isempty(stack) && heights[stack[end]] >= h
                top = pop!(stack)
                left = isempty(stack) ? 1 : stack[end] + 1
                area = heights[top] * (c - left)
                if area > bestarea
                    bestarea = area
                    best = (r - heights[top] + 1:r, left:c-1)
                end
            end
            c <= W && push!(stack, c)
        end
    end
    return best
end

# One step down / up a Gaussian pyramid: level-l pixel i sits at level-(l+1) position (i+1)/2.
pyr_down(A) = blur(A, 1.0)[1:2:end, 1:2:end]
function pyr_up(A, h, w)
    B = similar(A, h, w)
    Threads.@threads for j in 1:w
        for i in 1:h
            B[i, j] = bilinear(A, (i + 1) / 2, (j + 1) / 2)
        end
    end
    return B
end

"""
Multi-band blending (Burt & Adelson): fine detail comes from the photo chosen in
`sel` for each output pixel (crisp, no double images), while coarser and coarser
brightness variations are blended over wider and wider zones (no hard joins).
`sample(i, x, y)` gives image i's colour at output pixel (x, y) or nothing.
"""
function multiband(sample, n, sel, covered, Hc, Wc, outlines, tocanvas)
    L = clamp(floor(Int, log2(min(Hc, Wc) / 16)), 0, 7)       # pyramid levels
    L == 0 && return [covered[y, x] > 0 && sel[y, x] > 0 ?
                      something(sample(sel[y, x], x, y), (zero(RGB{Float32}), 0f0))[1] : zero(RGB{Float32})
                      for y in 1:Hc, x in 1:Wc]
    dims = [(Hc, Wc)]
    for _ in 1:L
        push!(dims, cld.(dims[end], 2))
    end
    num = [zeros(RGB{Float32}, d) for d in dims]
    den = [zeros(Float32, d) for d in dims]
    step = 2^L
    for i in 1:n
        o = filter(pt -> all(isfinite, pt), outlines[i])
        isempty(o) && continue
        cs = [tocanvas(pt...) for pt in o]
        # Image bounding box, aligned to the coarsest level so the levels line up.
        x0 = 1 + step * fld(clamp(floor(Int, minimum(first, cs)), 1, Wc) - 1, step)
        y0 = 1 + step * fld(clamp(floor(Int, minimum(last, cs)), 1, Hc) - 1, step)
        x1 = clamp(ceil(Int, maximum(first, cs)), 1, Wc); y1 = clamp(ceil(Int, maximum(last, cs)), 1, Hc)
        (x1 >= x0 && y1 >= y0) || continue
        h, w = y1 - y0 + 1, x1 - x0 + 1
        I = zeros(RGB{Float32}, h, w); V = zeros(Float32, h, w); W = zeros(Float32, h, w)
        Threads.@threads for x in x0:x1
            for y in y0:y1
                r = sample(i, x, y)
                r === nothing && continue
                I[y-y0+1, x-x0+1] = r[1]; V[y-y0+1, x-x0+1] = 1
                sel[y, x] == i && (W[y-y0+1, x-x0+1] = 1)
            end
        end
        sum(W) == 0 && continue
        # Gaussian pyramids; colours normalised by coverage so the area outside
        # the photo does not darken its border.
        GI = Matrix{RGB{Float32}}[]; GV = Matrix{Float32}[]; GW = Matrix{Float32}[]
        Iv, Vl, Wl = I .* V, V, W
        for l in 0:L
            push!(GI, [v > 1f-3 ? c / v : zero(c) for (c, v) in zip(Iv, Vl)]); push!(GV, Vl); push!(GW, Wl)
            l == L && break
            Iv, Vl, Wl = pyr_down(Iv), pyr_down(Vl), pyr_down(Wl)
        end
        # Pull-push: outside the photo, continue each level smoothly from the
        # coarser one. Otherwise the coarse levels would mix the photo with black
        # near its edge and leave thin dark or bright lines along the frame border.
        valid = [v > 1f-3 for v in GV[L+1]]
        top = GI[L+1]
        fillc = any(valid) ? sum(top[valid]) / count(valid) : zero(RGB{Float32})
        GI[L+1] = [ok ? c : fillc for (c, ok) in zip(top, valid)]
        for l in L-1:-1:0
            up = pyr_up(GI[l+2], size(GI[l+1])...)
            GI[l+1] = [v > 1f-3 ? c : u for (c, v, u) in zip(GI[l+1], GV[l+1], up)]
        end
        for l in 0:L
            lap = l == L ? GI[l+1] : GI[l+1] .- pyr_up(GI[l+2], size(GI[l+1])...)
            oy, ox = (y0 - 1) ÷ 2^l, (x0 - 1) ÷ 2^l
            hh = min(size(lap, 1), dims[l+1][1] - oy); ww = min(size(lap, 2), dims[l+1][2] - ox)
            (hh > 0 && ww > 0) || continue
            @views num[l+1][oy+1:oy+hh, ox+1:ox+ww] .+= lap[1:hh, 1:ww] .* GW[l+1][1:hh, 1:ww]
            @views den[l+1][oy+1:oy+hh, ox+1:ox+ww] .+= GW[l+1][1:hh, 1:ww]
        end
    end
    lev = [[d > 1f-6 ? c / d : zero(c) for (c, d) in zip(num[l], den[l])] for l in 1:L+1]
    out = lev[L+1]
    for l in L:-1:1
        out = pyr_up(out, dims[l]...) .+ lev[l]
    end
    return [covered[y, x] > 0 ? out[y, x] : zero(RGB{Float32}) for y in 1:Hc, x in 1:Wc]
end

# --------------------------------------------------------------------------
# Scripted entry point (the GUI runs these same steps one at a time)
# --------------------------------------------------------------------------

"""
    stitch(paths; layout=:all, cols=length(paths), snake=false, refine=true,
           feature_scale=0.5, nkeypoints=5000, denoise=1.5, ratio=0.8,
           max_distance=0.35, model=:homography, ransac_thresh=5.0, min_inliers=20,
           projection=:cylindrical, blend=:feather, raw_half=false) -> Matrix{RGB{Float32}}

Stitch images. `layout` chooses which pairs are matched (see `candidate_pairs`):
`:all` tries every pair, so the photos may be in any order (one or several rows);
`:sequential` for one row in shooting order; `:grid` for rows of `cols` images
(`snake = true` if every second row was shot in the opposite direction).
`refine` runs the global refinement (`bundle_adjust`, homography model only).
"""
function stitch(paths::AbstractVector{<:AbstractString};
                layout = :all, cols = length(paths), snake = false, refine = true,
                feature_scale = 0.5, nkeypoints = 5000, denoise = 1.5, ratio = 0.8,
                max_distance = 0.35, model = :homography, ransac_thresh = 5.0, min_inliers = 20,
                projection = :cylindrical, blend = :smart, sharpness = 3.0, raw_half = false)
    imgs = asyncmap(p -> fetch(Threads.@spawn load_any(p; raw_half)), paths)
    return stitch(imgs; layout, cols, snake, refine, feature_scale, nkeypoints, denoise, ratio,
                  max_distance, model, ransac_thresh, min_inliers, projection, blend, sharpness)
end

function stitch(imgs::AbstractVector{<:AbstractMatrix};
                layout = :all, cols = length(imgs), snake = false, refine = true,
                feature_scale = 0.5, nkeypoints = 5000, denoise = 1.5, ratio = 0.8,
                max_distance = 0.35, model = :homography, ransac_thresh = 5.0, min_inliers = 20,
                projection = :cylindrical, blend = :smart, sharpness = 3.0)
    n = length(imgs)
    n >= 2 || error("Need at least two images.")
    feats = fetch.([Threads.@spawn detect(img; scale = feature_scale, nkeypoints, denoise) for img in imgs])
    H = Dict{Tuple{Int,Int},Matrix{Float64}}()
    matches = Tuple{Int,Int,Matrix{Float64},Matrix{Float64}}[]
    for (i, j) in candidate_pairs(n, layout; cols, snake)
        p_i, p_j = match_pair(feats[i], feats[j]; ratio, max_distance)
        try
            Hij, inl = ransac(p_j, p_i; model, thresh = ransac_thresh)
            accept_pair(size(p_i, 2), length(inl), Hij, size(imgs[j])...; min_inliers)[1] || continue
            H[(i, j)] = Hij
            push!(matches, (i, j, p_i[:, inl], p_j[:, inl]))
            @info "Pair $(i)-$(j): $(size(p_i, 2)) matches, $(length(inl)) inliers"
        catch e
            e isa ErrorException || rethrow()
        end
    end
    centers = [((size(im, 2) + 1) / 2, (size(im, 1) + 1) / 2) for im in imgs]
    adj = spanning_tree(n, [(i, j, size(Pi, 2)) for (i, j, Pi, _) in matches])
    ref = tree_center(adj)
    parent, order, comp = walk_tree(adj, [ref; 1:n])
    members = findall(==(comp[ref]), comp)
    length(members) < n && @warn "Images $(findall(!=(comp[ref]), comp)) are not connected."
    f = something(estimate_focal([(Hij, centers[i], centers[j]) for ((i, j), Hij) in H]),
                  Float64(maximum(size(imgs[ref]))))
    G = place(H, parent, order, ref, [size(im, 2) for im in imgs])
    if refine && model === :homography
        G, f, k1, e0, e1, _ = bundle_adjust(G, centers, ref, f, members, matches; huber = ransac_thresh)
        lens = (k1, f)
        @info "Global refinement: rms error $(round(e0, digits = 2)) → $(round(e1, digits = 2)) px, " *
              "lens distortion k1 = $(round(k1, digits = 3))"
    else
        lens = (0.0, f)
    end
    o = [outline(Gi, size(im)..., projection, f, centers[ref]; lens) for (Gi, im) in zip(G, imgs)]
    return compose(imgs, G, bounds(o); projection, f, c = centers[ref], blend, lens, sharpness)
end

"""
Save with a sensible bit depth: 16-bit for TIFF/PNG, 8-bit for JPEG, and 32-bit
float FITS / XISF (values kept as they are, not clipped) for .fits/.fit/.fts/.xisf.
"""
function save_panorama(path, img)
    isfits(path) && return write_fits(path, img)
    isxisf(path) && return write_xisf(path, img)
    c01(x) = isnan(x) ? 0f0 : clamp(x, 0f0, 1f0)
    img = map(p -> RGB{Float32}(c01(red(p)), c01(green(p)), c01(blue(p))), img)
    ext = lowercase(splitext(path)[2])
    out = ext in (".tif", ".tiff", ".png") ? RGB{N0f16}.(img) : RGB{N0f8}.(img)
    save(path, out)
end

end # module
