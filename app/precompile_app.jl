# What the standalone app runs while it is being built (build/build_app.jl), so
# that this code is compiled into it: the window is opened (hidden), images are
# loaded and stitched with "Run all steps", and the result is saved. Needs a
# display (OpenGL), like the app itself.

using PanoStitchApp, PanoStitch, GLMakie
using PanoStitch: Stitcher as St
const RGB = St.RGB                 # (ColorTypes, through PanoStitch)
using Random: MersenneTwister

const GUI = Base.get_extension(PanoStitch, :PanoStitchGUIExt).PanoGUI

let dir = mktempdir()
    # Three overlapping crops of a random smooth scene, as JPEG, PNG and TIFF.
    rng = MersenneTwister(1)
    scene = St.blur(rand(rng, RGB{Float32}, 600, 1300), 2.0)
    paths = String[]
    for (k, ext) in enumerate((".jpg", ".png", ".tif"))
        p = joinpath(dir, "frame$k$ext")
        St.save_panorama(p, scene[:, 1+(k-1)*400:(k-1)*400+500])
        push!(paths, p)
    end

    GUI.load_glyph_cache()
    for ui in ((dark = true, modern = true, simple = false), (dark = false, modern = false, simple = true))
        fig, st = GUI.build_gui(paths; ui, watch = false)
        screen = GLMakie.Screen(; visible = false)
        display(screen, fig)
        GLMakie.colorbuffer(screen)
        # Wait for the images to load, then "Run all steps".
        t = time()
        while (length(st.items) < 3 || st.busy) && time() - t < 120
            sleep(0.2)
        end
        run_all = only(b for b in fig.content
                       if b isa Button && b.label[] isa AbstractString && occursin("Run all steps", b.label[]))
        run_all.clicks[] += 1
        sleep(0.5)
        t = time()
        while st.busy && time() - t < 300
            sleep(0.2)
        end
        GLMakie.colorbuffer(screen)
        st.pano === nothing || St.save_panorama(joinpath(dir, "pano.jpg"), st.pano.img)
        println("Precompile run: $(length(st.items)) images, panorama ",
                st.pano === nothing ? "not made" : join(size(st.pano.img), "×"))
        st.api.close()
        close(screen)
    end
    GUI.save_glyph_cache()
end
