# The program as a whole: PanoStitch with its window. `julia_main` is the entry
# point of the standalone app built by build/build_app.jl (PackageCompiler);
# bin/panostitch.jl does the same when Julia itself is installed.

module PanoStitchApp

using PanoStitch, GLMakie

"""Open the window with `args` (images or project files); return when it is closed."""
function main(args = String[])
    screen = PanoStitch.run_gui(String.(args))
    wait(screen)
    return 0
end

function julia_main()::Cint
    # Detection and rendering run on all cores. The number of threads is fixed
    # when the program starts, so start again with all of them if needed.
    if Threads.nthreads() == 1 && !haskey(ENV, "PANOSTITCH_RELAUNCHED")
        exe = unsafe_string(Base.JLOptions().julia_bin)
        cmd = addenv(`$exe $ARGS`, "JULIA_NUM_THREADS" => "auto", "PANOSTITCH_RELAUNCHED" => "1")
        return run(ignorestatus(cmd)).exitcode
    end
    try
        return main(ARGS)
    catch e
        e isa InterruptException && return 0
        Base.invokelatest(Base.display_error, e, catch_backtrace())
        return 1
    end
end

end # module
