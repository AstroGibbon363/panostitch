# Star stacking from the command line (the same engine as the "Star stacking" tab).
#
#   julia bin/panostack.jl [options] STAR_IMAGES_OR_FOLDER...
#
#   -o FILE             output: .tif (16-bit), .jpg, .fits / .xisf (32-bit float, linear);
#                       default stacked.tif next to the first star image
#   --base FILE         base image (default: the middle one)
#   --darks X           noise images: a folder or comma-separated files
#   --bias X, --flats X, --flatdarks X   the other calibration frames, the same way
#   --mode M            accumulation | best (default) | freeze | alignonly | trails | trails-motion
#   --fade F            star trails: in | out | both (fade the trails)
#   --fill-gaps         star trails: fill the gaps between frames along the star motion
#   --horizon V         sky region: horizontal line at V (0 = top, 1 = bottom) …
#   --gradient G        … with a transition zone of G (fraction of the height)
#   --auto-sky          sky region: detected automatically in the base image
#   --foreground FILE   the ground from this image (same framing) instead of the stack; needs a sky region
#   --fg-ev EV          … brighter / darker by EV (after matching it to the stacked ground)
#   --fg-no-match       … without matching its brightness
#   --sigma LO,HI       sigma clipping (default 3,3)
#   --half              decode RAW files at half size (faster, less memory)
#   --no-reject         keep every frame (no automatic rejection)
#   --distortion D      lens | complex | auto (distortion correction)
#   --lp F[:S]          light pollution: deep[:strength] or uneven[:strength] (strength 0–100, default 50)
#   --enhance S         enhance star light with strength S (0–100)
#   --timelapse N       stack every N frames into a numbered sequence (folder <output>_timelapse)
#   --bgx [DEG]         background extraction (polynomial degree, default 2)
#   --colorcal          basic colour calibration (neutral sky, white stars)
#   --color-noise S     reduce colour noise with strength S (5–100)
#   --stretch           automatic histogram stretch instead of the sRGB curve
#   --hdr, --no-auto-brightness, --linear (no tone curve), --adobe, --merge4, --dynamic-noise
#
# The frame report is printed at the end.

const SELF = @__FILE__
if abspath(PROGRAM_FILE) == SELF && Threads.nthreads() == 1 && !haskey(ENV, "PANOSTACK_RELAUNCHED")
    cmd = `$(Base.julia_cmd()) -t auto $SELF $ARGS`
    exit(run(ignorestatus(addenv(cmd, "PANOSTACK_RELAUNCHED" => "1"))).exitcode)
end

# The core package only (no graphics), with the standard library.
import Pkg
Pkg.activate(joinpath(@__DIR__, ".."); io = devnull)
try
    @eval using PanoStitch: Stacker, Stitcher
catch e
    e isa ArgumentError || rethrow()          # a package is not installed yet
    Pkg.instantiate()
    @eval using PanoStitch: Stacker, Stitcher
end

function files_of(x)
    isdir(x) && return sort(filter(f -> lowercase(splitext(f)[2]) in Stitcher.IMAGE_EXTS, readdir(x; join = true)))
    return String.(split(x, ','; keepempty = false))
end

function main(args)
    cfg = Dict{Symbol,Any}()
    stars = String[]
    sky = (mode = "None", line = nothing, gradient = 0.0, mask = falses(0, 0))
    i = 1
    next() = (i += 1; i <= length(args) || error("$(args[i-1]) needs a value"); args[i])
    while i <= length(args)
        a = args[i]
        if a == "-o"; cfg[:output] = next()
        elseif a == "--base"; cfg[:base] = abspath(next())
        elseif a in ("--darks", "--bias", "--flats", "--flatdarks"); cfg[Symbol(a[3:end])] = files_of(next())
        elseif a == "--mode"
            m = next()
            m in ("trails", "trails-motion") ? (cfg[:composition] = "Star trails"; cfg[:motion_effect] = m == "trails-motion") :
                (cfg[:align_mode] = Dict("accumulation" => "Accumulation", "best" => "Select best pixels",
                                         "freeze" => "Freeze ground", "alignonly" => "Align only")[m])
        elseif a == "--fade"
            f = Dict("in" => "Fade in", "out" => "Fade out", "both" => "Both ends")[next()]
            cfg[:trails] = merge(get(cfg, :trails, Stacker.DEFAULTS.trails), (fade = f,))
        elseif a == "--fill-gaps"
            cfg[:trails] = merge(get(cfg, :trails, Stacker.DEFAULTS.trails), (fill_gaps = true,))
        elseif a == "--auto-sky"; sky = merge(sky, (mode = "Automatic",))
        elseif a == "--foreground"
            cfg[:foreground] = merge(get(cfg, :foreground, Stacker.DEFAULTS.foreground), (path = abspath(next()),))
        elseif a == "--fg-ev"
            cfg[:foreground] = merge(get(cfg, :foreground, Stacker.DEFAULTS.foreground), (ev = parse(Float64, next()),))
        elseif a == "--fg-no-match"
            cfg[:foreground] = merge(get(cfg, :foreground, Stacker.DEFAULTS.foreground), (match = false,))
        elseif a == "--horizon"
            v = parse(Float64, next()); sky = merge(sky, (mode = sky.gradient > 0 ? "Gradient" : "Boundary line", line = (0.0, v, 1.0, v)))
        elseif a == "--gradient"
            g = parse(Float64, next()); sky = merge(sky, (gradient = g, mode = sky.line === nothing ? "None" : "Gradient"))
        elseif a == "--sigma"
            lo, hi = parse.(Float64, split(next(), ',')); cfg[:sigma] = (low = lo, high = hi, iters = 3)
        elseif a == "--half"; cfg[:raw] = merge(Stacker.DEFAULTS.raw, (half = true,))
        elseif a == "--no-reject"; cfg[:reject] = merge(Stacker.DEFAULTS.reject, (auto = false,))
        elseif a == "--distortion"; cfg[:distortion] = titlecase(next())
        elseif a == "--lp"
            f = split(next(), ':')
            cfg[:light_pollution] = merge(Stacker.DEFAULTS.light_pollution,
                (filter = f[1] == "deep" ? "Deep sky" : "Uneven", strength = length(f) > 1 ? parse(Int, f[2]) : 50))
        elseif a == "--enhance"; cfg[:enhance_stars] = (on = true, strength = parse(Int, next()))
        elseif a == "--timelapse"; cfg[:timelapse] = (on = true, frames = parse(Int, next()))
        elseif a == "--adobe"; cfg[:color_space] = "Adobe RGB"
        elseif a == "--bgx"
            deg = i < length(args) && all(isdigit, args[i+1]) ? parse(Int, next()) : 2
            cfg[:bg_extraction] = (on = true, degree = deg)
        elseif a == "--colorcal"; cfg[:color_calibration] = true
        elseif a == "--color-noise"; cfg[:color_noise] = (on = true, strength = parse(Int, next()))
        elseif a == "--stretch"; cfg[:tone] = "Stretch"
        elseif a == "--hdr"; cfg[:hdr] = true
        elseif a == "--no-auto-brightness"; cfg[:auto_brightness] = false
        elseif a == "--linear"; cfg[:tone] = "Linear"
        elseif a == "--merge4"; cfg[:merge4] = true
        elseif a == "--dynamic-noise"; cfg[:dynamic_noise] = true
        elseif startswith(a, "-"); error("Unknown option $a (see the top of bin/panostack.jl)")
        else append!(stars, abspath.(files_of(a)))
        end
        i += 1
    end
    isempty(stars) && error("No star images given.")
    get!(cfg, :output, joinpath(dirname(stars[1]), "stacked.tif"))
    cfg[:stars] = stars
    cfg[:sky] = sky
    last_ = Ref("")
    r = Stacker.stack(NamedTuple(cfg); progress = (m, _) -> m == last_[] || (last_[] = m; println(m)))
    print(Stacker.report_text(r.report))
    println("\nProcess log:"); foreach(l -> println("  ", l), r.log)
    println("Saved: ", join(r.saved, ", "))
end

main(ARGS)
