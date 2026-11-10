# Core of PanoStitch: panorama stitching (Stitcher) and star stacking (Stacker).
# Loading it does not load any graphics: the window lives in the package
# extension ext/PanoStitchGUIExt, which Julia loads when GLMakie is loaded too
# (`using PanoStitch, GLMakie`, as app/ and bin/panostitch.jl do).

module PanoStitch

include("stitcher.jl")
include("stacker.jl")

export run_gui, Stitcher, Stacker

"""
    run_gui(paths = String[])

Open the window (stitching and star stacking tabs), optionally with images or a
project file loaded. Needs GLMakie: `using PanoStitch, GLMakie`. Returns the
GLMakie screen; `wait(screen)` blocks until the window is closed.
"""
function run_gui(paths = String[])
    ext = Base.get_extension(@__MODULE__, :PanoStitchGUIExt)
    ext === nothing && error("The window needs GLMakie: run `using GLMakie` first " *
                             "(or start the program with bin/panostitch.jl).")
    return Base.invokelatest(ext.PanoGUI.run_gui, paths)
end

# Compile the common code paths once, when the package is precompiled, so that
# the first detect / match / render / stack are fast. (The window has its own
# workload in the extension.)
using PrecompileTools
@setup_workload begin
    using ColorTypes: RGB
    using Random: MersenneTwister
    rng = MersenneTwister(1)
    scene = Stitcher.blur(rand(rng, RGB{Float32}, 420, 520), 1.0)
    # 2 rows × 2 columns of overlapping crops
    imgs = [scene[1+r*160:r*160+260, 1+c*220:c*220+300] for r in 0:1 for c in 0:1]
    @compile_workload begin
        S = Stitcher
        S.resize(imgs[1], 0.5)
        F = [S.detect(im; scale = 1.0, nkeypoints = 500) for im in imgs]
        for m in S.MODELS
            p1, p2 = S.match_pair(F[1], F[2])
            try S.ransac(p2, p1; model = m, iters = 50) catch end
        end
        H = Dict{Tuple{Int,Int},Matrix{Float64}}()
        M = Tuple{Int,Int,Matrix{Float64},Matrix{Float64}}[]
        for (i, j) in S.candidate_pairs(4, :all)
            p1, p2 = S.match_pair(F[i], F[j])
            try
                Hij, inl = S.ransac(p2, p1; iters = 200)
                S.accept_pair(size(p1, 2), length(inl), Hij, 261, 300; min_inliers = 8)[1] || continue
                H[(i, j)] = Hij
                push!(M, (i, j, p1[:, inl], p2[:, inl]))
            catch
            end
        end
        S.candidate_pairs(6, :grid; cols = 3, snake = true)
        adj = S.spanning_tree(4, [(i, j, size(P, 2)) for (i, j, P, _) in M])
        ref = S.tree_center(adj)
        parent, order, comp = S.walk_tree(adj, [ref; 1:4])
        cs = [(150.5, 131.0) for _ in 1:4]
        f = something(S.estimate_focal([(Hij, cs[i], cs[j]) for ((i, j), Hij) in H]), 300.0)
        G = S.place(H, parent, order, ref, fill(300, 4))
        G, f, _ = S.bundle_adjust(G, cs, ref, f, findall(==(comp[ref]), comp), M; iters = 3)
        S.estimate_gains(imgs, parent, order,
                         Dict((parent[v], v) => (zeros(2, 0), zeros(2, 0)) for v in order if parent[v] != 0))
        for p in S.PROJECTIONS, b in S.BLENDS
            o = [S.outline(Gi, size(im)..., p, f, cs[ref]) for (Gi, im) in zip(G, imgs)]
            S.compose(imgs, G, S.bounds(o); scale = 0.25, projection = p, f, c = cs[ref], blend = b)
        end
        # star stacking: a shifted star field (with a lamp in the foreground)
        stars = [(rand(rng) * 300, rand(rng) * 200, 0.2 + rand(rng)) for _ in 1:80]
        frames = Dict(map(0:4) do t
            A = fill(RGB{Float32}(0.02, 0.02, 0.02), 200, 300)
            for (x, y, b) in stars, j in 1:200, i in 1:300
                d2 = (i - x - 1.5t)^2 + (j - y - 0.7t)^2
                d2 < 16 && (A[j, i] += RGB{Float32}(1, 1, 1) * Float32(b * exp(-d2 / 2)))
            end
            "f$t" => A
        end)
        sky = (mode = "Gradient", line = (0.0, 0.8, 1.0, 0.8), gradient = 0.05, mask = falses(0, 0))
        for (comp, mode) in (("Align stars", "Select best pixels"), ("Align stars", "Freeze ground"),
                             ("Star trails", "Accumulation"))
            Stacker.stack((stars = sort(collect(keys(frames))), composition = comp, align_mode = mode, sky);
                          preview = true, loader = p -> copy(frames[p]))
        end
        Stacker.stack((stars = sort(collect(keys(frames))), sky, distortion = "Complex",
                       light_pollution = merge(Stacker.DEFAULTS.light_pollution, (filter = "Uneven", iaa = true)),
                       enhance_stars = (on = true, strength = 50), timelapse = (on = true, frames = 3),
                       color_space = "Adobe RGB"); preview = true, loader = p -> copy(frames[p]))
    end
end

end # module
