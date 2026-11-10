# The window (GLMakie), as a package extension: Julia loads it automatically
# once both PanoStitch and GLMakie are loaded. The core package therefore
# stays free of graphics, and scripts / the command-line stacker load fast.

module PanoStitchGUIExt

using PanoStitch, GLMakie

include("gui.jl")

# Compile building the window and drawing its first frame, so the window opens
# quickly. Drawing needs an OpenGL context; without one (a headless build
# machine) that part is skipped, and the first frame is compiled at run time.
using PrecompileTools
@setup_workload begin
    @compile_workload begin
        fig, _ = PanoGUI.build_gui(; watch = false)
        try
            GLMakie.activate!()
            screen = GLMakie.Screen(; visible = false, start_renderloop = false)
            display(screen, fig)
            GLMakie.Makie.colorbuffer(screen)
            close(screen)
        catch
        end
        GLMakie.closeall()
        GLMakie.Makie.cleanup_globals()
    end
end

end # module
