# Tests of the stacking engine on synthetic night landscapes (no files, no
# window):  julia --project=. -t auto test/runtests.jl
#
# A scene: sky with stars that turn around a pole above the frame, a hilly dark
# foreground that stays in place, background noise. Frames are made in memory
# and handed to Stacker.stack through its `loader`.

using Test, Random, Statistics
using PanoStitch: Stacker as Sk, Stitcher as St
using PanoStitch.Stitcher: RGB, red, green, blue

const H, W = 300, 420
horizon_of(x) = 0.62H + 18sin(x / 37) + 10sin(x / 11 + 1)        # ground below

function scene(; nstars = 300, seed = 1)
    rng = MersenneTwister(seed)
    stars = [(rand(rng) * 1.6W - 0.3W, rand(rng) * 1.4H - 0.3H, 0.05 + rand(rng)^4 * 1.5) for _ in 1:nstars]
    ground = St.blur(rand(rng, Float32, H, W), 2.0)
    return (; stars, ground)
end

"""Frame k: the stars turned by k·dθ around the pole, each a short arc (exposure fraction `expo`)."""
function frame(sc, k; dθ = 0.004, pole = (W / 2, -1.5H), expo = 0.0, seed = 100 + k, fgain = 1.0, σ = 1.1)
    rng = MersenneTwister(seed)
    img = fill(RGB{Float32}(0.03, 0.035, 0.05), H, W)
    nsub = expo > 0 ? 6 : 1
    for (x0, y0, f) in sc.stars, j in 1:nsub
        θ = (k + expo * (j - 1) / max(nsub - 1, 1)) * dθ
        dx, dy = x0 - pole[1], y0 - pole[2]
        x = pole[1] + cos(θ) * dx - sin(θ) * dy
        y = pole[2] + sin(θ) * dx + cos(θ) * dy
        (4 <= x <= W - 4 && 4 <= y <= H - 4) || continue
        for yy in floor(Int, y)-3:floor(Int, y)+4, xx in floor(Int, x)-3:floor(Int, x)+4
            (1 <= yy <= H && 1 <= xx <= W) || continue
            v = Float32(f / nsub * exp(-((xx - x)^2 + (yy - y)^2) / (2σ^2)))
            img[yy, xx] += RGB{Float32}(v, v, v)
        end
    end
    for y in 1:H, x in 1:W
        if y > horizon_of(x)
            g = 0.008f0 + 0.02f0 * sc.ground[y, x]
            img[y, x] = RGB{Float32}(g, 0.9g, 0.8g)
        end
        n = 0.002f0 * randn(rng, Float32)
        img[y, x] = (img[y, x] + RGB{Float32}(n, n, n)) * Float32(fgain)
    end
    return img
end

"""A configuration over K in-memory frames: paths "f01" … with a loader."""
function memstack(imgs; extra...)
    paths = ["mem/f$(lpad(k, 2, '0')).tif" for k in eachindex(imgs)]
    lookup = Dict(zip(paths, imgs))
    loader = p -> copy(lookup[p])
    return (; stars = paths, output = joinpath(mktempdir(), "stacked.tif"), extra...), loader, lookup
end

line_sky = (mode = "Boundary line", line = (0.0, 0.5, 1.0, 0.5), gradient = 0.0, mask = falses(0, 0))
true_mask(rows, cols) = [((r - 0.5) / rows * H) < horizon_of((c - 0.5) / cols * W) for r in 1:rows, c in 1:cols]

@testset "PanoStitch stacking" begin
    sc = scene()

    @testset "automatic sky detection" begin
        img = frame(sc, 0)
        mask, hz = Sk.detect_sky(img; cols = 200)
        truth = true_mask(size(mask)...)
        acc = mean(mask .== truth)
        ground_as_sky = count(mask .& .!truth) / count(.!truth)
        @info "sky detection" acc ground_as_sky
        @test acc > 0.95
        @test ground_as_sky < 0.02                       # stays off the foreground
        # A frame that is all sky: everything is sky.
        sky_only = [RGB{Float32}(0.03, 0.035, 0.05) + RGB{Float32}(0.002randn(), 0, 0) for _ in 1:H, _ in 1:W]
        @test mean(Sk.detect_sky(sky_only; cols = 100)[1]) > 0.97
    end

    @testset "mask feather (inside the sky only)" begin
        m = falses(50, 100); m[1:25, :] .= true
        Wm = Sk.mask_weight(m, 0.1)
        @test all(Wm[26:end, :] .== 0)                   # foreground untouched
        @test Wm[1, 50] ≈ 1
        @test 0 < Wm[24, 50] < 0.5                        # soft just inside the edge
        @test all(Sk.mask_weight(m, 0.0)[1:23, :] .> 0.99)
    end

    @testset "trail fades" begin
        @test Sk.trail_fade("Off", 5) == ones(Float32, 5)
        fin, fout, both = Sk.trail_fade("Fade in", 9), Sk.trail_fade("Fade out", 9), Sk.trail_fade("Both ends", 9)
        @test issorted(fin) && fin[end] == 1
        @test issorted(fout; rev = true) && fout[1] == 1
        @test both[1] < both[5] && both[9] < both[5] && both[5] == 1
        @test Sk.fade_kind(merge(Sk.DEFAULTS, (motion_effect = true,))) == "Fade in"   # older projects
    end

    @testset "align stars + log" begin
        imgs = [frame(sc, k; dθ = 0.002) for k in 1:6]
        cfg, loader, _ = memstack(imgs; sky = line_sky)
        r = Sk.stack(cfg; loader)
        @test count(fi -> fi.status in (:used, :base), r.report) == 6
        @test all(fi -> fi.status === :base || fi.err < 0.5, r.report)
        @test any(l -> occursin("Combined", l), r.log) && occursin("in total", r.log[end])
    end

    @testset "star trails: gap filling" begin
        # Point-like stars moving ~8 px per frame: dashes without gap filling.
        dθ = 0.015
        imgs = [frame(sc, k; dθ, σ = 1.0) for k in 0:7]
        base_cfg = (composition = "Star trails", sky = line_sky,
                    reject = merge(Sk.DEFAULTS.reject, (auto = false,)), auto_brightness = false, tone = "Linear")
        cfg0, loader, _ = memstack(imgs; base_cfg...)
        cfg1 = merge(cfg0, (trails = (fade = "Off", fill_gaps = true),))
        r0 = Sk.stack(cfg0; loader); r1 = Sk.stack(cfg1; loader)
        # Halfway between the positions of a bright star in frames 3 and 4.
        pole = (W / 2, -1.5H)
        bright = [s for s in sc.stars if s[3] > 0.4]
        gains = Float64[]
        for (x0, y0, _) in bright
            pos(k) = (θ = k * dθ; dx = x0 - pole[1]; dy = y0 - pole[2];
                      (pole[1] + cos(θ) * dx - sin(θ) * dy, pole[2] + sin(θ) * dx + cos(θ) * dy))
            p = pos(3.5)
            x, y = round.(Int, p)
            (20 <= x <= W - 20 && 20 <= y <= 0.45H) || continue
            push!(gains, St.luminance(r1.raw[y, x]) / max(St.luminance(r0.raw[y, x]), 1e-4))
        end
        @info "gap filling" n = length(gains) median_gain = median(gains)
        @test length(gains) >= 3
        @test median(gains) > 3                           # the gap is filled
        @test any(l -> occursin("Gap filling", l), r1.log)
        # The foreground is not touched by the trails.
        @test St.luminance(r1.raw[H - 5, 10]) ≈ St.luminance(r0.raw[H - 5, 10]) atol = 1e-4
    end

    @testset "foreground image" begin
        imgs = [frame(sc, k; dθ = 0.002) for k in 1:5]
        fg = [RGB{Float32}(0.2, 0.1, 0.05) for _ in 1:H, _ in 1:W]           # a bright, brown "blue hour" ground
        sky = (mode = "Boundary line", line = (0.0, 0.45, 1.0, 0.45), gradient = 0.0, mask = falses(0, 0))
        cfg, loader, lookup = memstack(imgs; sky, auto_brightness = false, tone = "Linear", align_mode = "Accumulation")
        lookup["mem/fg.tif"] = fg
        r_match = Sk.stack(merge(cfg, (foreground = (path = "mem/fg.tif", match = true, ev = 0.0),)); loader)
        r_raw = Sk.stack(merge(cfg, (foreground = (path = "mem/fg.tif", match = false, ev = 1.0),)); loader)
        r_none = Sk.stack(cfg; loader)
        g = r_raw.raw[H - 3, 5]
        @test red(g) ≈ 0.4 rtol = 0.01                                        # 2^1 × the foreground
        @test blue(g) ≈ 0.1 rtol = 0.02
        @test red(r_raw.raw[H - 3, 5]) / green(r_raw.raw[H - 3, 5]) ≈ 2 rtol = 0.01  # its colour
        # Matched: the brightness of the stacked ground.
        lum_ground(r) = median(St.luminance(r.raw[y, x]) for y in round(Int, 0.47H):H, x in 1:W)
        @test lum_ground(r_match) ≈ lum_ground(r_none) rtol = 0.1
        @test r_match.raw[10, 10] == r_none.raw[10, 10]                       # the sky is the stack's
        @test_throws ErrorException Sk.stack(merge(cfg, (sky = merge(sky, (mode = "None",)),
                                                         foreground = (path = "mem/fg.tif", match = true, ev = 0.0))); loader)
    end

    @testset "automatic sky in a run" begin
        imgs = [frame(sc, k; dθ = 0.002) for k in 1:4]
        cfg, loader, _ = memstack(imgs; sky = (mode = "Automatic", line = nothing, gradient = 0.0, mask = falses(0, 0)),
                                  align_mode = "Freeze ground")
        r = Sk.stack(cfg; loader)
        @test r.mask !== nothing && mean(r.mask .== true_mask(size(r.mask)...)) > 0.95
        @test any(l -> occursin("Sky detected", l), r.log)
    end
end
