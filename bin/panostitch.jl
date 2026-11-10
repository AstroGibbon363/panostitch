# PanoStitch: opens the window (panorama stitching and star stacking).
#
#   julia bin/panostitch.jl [image or project files ...]
# or, in VS Code / the REPL:  include("bin/panostitch.jl")
#
# The first start after installing or changing the code precompiles the
# program (a few minutes, once); after that the window opens in seconds.

# Restart with all CPU threads if started without `-t` (detection and
# rendering run in parallel).
const SELF = @__FILE__
if abspath(PROGRAM_FILE) == SELF && Threads.nthreads() == 1 && !haskey(ENV, "PANOSTITCH_RELAUNCHED")
    cmd = `$(Base.julia_cmd()) -t auto $SELF $ARGS`
    exit(run(ignorestatus(addenv(cmd, "PANOSTITCH_RELAUNCHED" => "1"))).exitcode)
end

# The app environment (app/Project.toml): PanoStitch plus GLMakie, with pinned
# versions in app/Manifest.toml.
import Pkg
Pkg.activate(joinpath(@__DIR__, "..", "app"); io = devnull)
try
    @eval using PanoStitch, GLMakie
catch e
    e isa ArgumentError || rethrow()          # a package is not installed yet
    Pkg.instantiate()
    @eval using PanoStitch, GLMakie
end

files = isinteractive() ? String[] : ARGS
screen = PanoStitch.run_gui(files)
isinteractive() || wait(screen)      # keep the program open until the window is closed
