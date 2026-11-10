# Builds the PDF manuals from the Markdown ones:
#
#   julia docs/make_pdf.jl            # all languages
#   julia docs/make_pdf.jl en cs      # some of them
#
# pandoc turns the Markdown into Typst, Typst typesets it. Both come as Julia
# packages (docs/Project.toml), so nothing needs installing, on any system.
# The fonts are the program's own (assets/fonts), so the PDFs look the same
# everywhere and have every character the manuals use.

import Pkg
Pkg.activate(@__DIR__; io = devnull)
Pkg.instantiate(; io = devnull)
using pandoc_jll, Typst_jll

const FONTS = normpath(joinpath(@__DIR__, "..", "assets", "fonts"))
const TITLES = Dict("en" => "PanoStitch: User Manual", "cs" => "PanoStitch: uživatelská příručka",
                    "de" => "PanoStitch: Benutzerhandbuch")

# Layout additions for pandoc's Typst template: smaller code blocks (the window
# sketches are 86 characters wide), table cells a little tighter.
const HEADER = """
#show raw.where(block: true): set text(size: 6.8pt)
#show raw.where(block: true): block.with(width: 100%, inset: 6pt, fill: luma(245), radius: 2pt)
#set table(inset: 4pt, align: left)
#show table.cell: set align(left)
#show table: set text(size: 8.5pt)
#show link: set text(fill: rgb("#1f5fa8"))
"""

function build(lang)
    md = joinpath(@__DIR__, "manual_$lang.md")
    pdf = joinpath(@__DIR__, "manual_$lang.pdf")
    mktempdir() do dir
        hdr = joinpath(dir, "header.typ")
        write(hdr, HEADER)
        typ = joinpath(dir, "manual.typ")
        run(`$(pandoc()) $md --from gfm --to typst --standalone --output $typ
             --include-in-header $hdr
             -V mainfont=DejaVu\ Sans -V codefont=DejaVu\ Sans\ Mono -V fontsize=10pt
             -V papersize=a4 -V margin.x=2cm -V margin.y=2cm -V lang=$lang
             --metadata title-meta=$(TITLES[lang]) --metadata pagetitle=$(TITLES[lang])`)
        run(`$(typst()) compile --ignore-system-fonts --font-path $FONTS $typ $pdf`)
    end
    println("Wrote ", pdf)
end

foreach(build, isempty(ARGS) ? ["en", "cs", "de"] : ARGS)
