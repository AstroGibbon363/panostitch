# Stitching from your own script, without the window. The code lives in
# src/stitcher.jl; this loads only the core package (no graphics).
#
#   include("examples/scripted_stitch.jl")
#   pano = Stitcher.stitch(["a.NEF", "b.NEF", "c.NEF"]; projection = :cylindrical)
#   Stitcher.save_panorama("pano.tif", pano)
#
# For the window run bin/panostitch.jl instead.

import Pkg
Pkg.activate(joinpath(@__DIR__, ".."); io = devnull)
using PanoStitch: Stitcher, Stacker
