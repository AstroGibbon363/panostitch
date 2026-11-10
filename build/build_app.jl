# Builds the standalone program: a folder with the `panostitch` executable
# (panostitch.exe on Windows) and everything it needs, Julia included, so it
# runs on computers without Julia. Build on each target system (Windows,
# macOS, Linux) separately; the result only runs on the kind of system it was
# built on.
#
#   julia build/build_app.jl [output folder]       (default: build/PanoStitch)
#
# Takes a long time (about half an hour) and several GB of memory. Needs a
# display: the build opens the window hidden to compile the drawing code.

import Pkg
Pkg.activate(@__DIR__; io = devnull)
Pkg.instantiate()
using PackageCompiler

const ROOT = normpath(joinpath(@__DIR__, ".."))
const APP = joinpath(ROOT, "app")
const OUT = abspath(get(ARGS, 1, joinpath(@__DIR__, "PanoStitch")))

Pkg.activate(APP; io = devnull)        # the app's own versions, from app/Manifest.toml
Pkg.instantiate()
Pkg.activate(@__DIR__; io = devnull)

create_app(APP, OUT;
           executables = ["panostitch" => "julia_main"],
           precompile_execution_file = joinpath(APP, "precompile_app.jl"),
           include_lazy_artifacts = true,
           force = true)

# The manuals go along.
docs = joinpath(OUT, "docs")
mkpath(docs)
for f in readdir(joinpath(ROOT, "docs"))
    endswith(f, ".pdf") && cp(joinpath(ROOT, "docs", f), joinpath(docs, f); force = true)
end
cp(joinpath(ROOT, "README.md"), joinpath(OUT, "README.md"); force = true)
println("Built ", OUT, ": start ", joinpath(OUT, "bin", Sys.iswindows() ? "panostitch.exe" : "panostitch"))
