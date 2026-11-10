# Interactive front-end (GLMakie): the stitching tab, the window around both
# tabs (stacker_gui.jl is the star stacking tab) and the appearance settings.
#
# Every step runs only when you press its button, and each result can be
# inspected (keypoints, matches with inliers/outliers, placed outlines and
# links) and overridden (pairs, reference image, projection, horizon, nudges,
# brightness).
#
# Navigation: left-drag = pan, scroll / + / - = zoom, 0 or "Fit" = reset.
# Image list: click, Ctrl-click, Shift-click to select; Delete removes.
# Panorama view: click an image to select it; Ctrl + drag or the arrow keys
# (shift = ×10) move it together with the images attached beyond it. When zoomed in, the visible area
# is re-rendered at screen resolution after a short pause.

module PanoGUI

using GLMakie, LinearAlgebra, Statistics, TOML
using GLMakie.Makie: GridLayoutBase
using RelocatableFolders: @path
using ColorTypes: RGB
import PanoStitch.Stitcher as St
import PanoStitch.Stacker as Sk

const DISP_MAX = 1600            # longest side of the on-screen copy of each image
const LIST_ROWS = 9              # visible rows of the image list
const MODES = ("all pairs", "sequential", "grid")
const PROJECT_EXT = ".panoproj"     # project files (TOML text)
const UNDO_STEPS = 100
# Fonts shipped with the program (DejaVu, see assets/fonts/LICENSE-DejaVu.txt): the
# monospaced lists and the modern look must not depend on what is installed.
const FONT_DIR = @path joinpath(@__DIR__, "..", "..", "assets", "fonts")
font_file(name) = joinpath(FONT_DIR, name)
mono_font() = font_file("DejaVuSansMono.ttf")

# Starting points for typical subjects; every value can still be changed.
const PRESETS = Dict(
    "day landscape" => (det_px = 2000, nkp = 5000, thr = 0.001, denoise = 0.5, mode = "all pairs",
                        ratio = 0.8, maxd = 0.35, tol = 4.0, mininl = 20, model = "homography",
                        refine = true, proj = "cylindrical", blend = "smart", ev = 0.0,
                        note = "Daylight panorama, camera rotated in place."),
    "astro / night landscape" => (det_px = 2000, nkp = 6000, thr = 0.0005, denoise = 2.0, mode = "all pairs",
                        ratio = 0.85, maxd = 0.38, tol = 5.0, mininl = 16, model = "homography",
                        refine = true, proj = "spherical", blend = "smart", ev = 2.0,
                        note = "Night or Milky Way panorama: more denoising, looser matching, " *
                               "spherical projection. Use Auto exposure after the preview."),
    "deep-sky mosaic" => (det_px = 3000, nkp = 4000, thr = 0.002, denoise = 0.75, mode = "all pairs",
                        ratio = 0.75, maxd = 0.3, tol = 3.0, mininl = 12, model = "similarity",
                        refine = false, proj = "rectilinear", blend = "smart", ev = 0.0,
                        note = "Telescope mosaic of star fields (e.g. FITS): tiny field of view, so the " *
                               "frames only shift and rotate (field rotation); stars are small, so little " *
                               "denoising. Use Auto stretch to see faint detail."),
    "macro / microscope" => (det_px = 2000, nkp = 5000, thr = 0.001, denoise = 1.0, mode = "all pairs",
                        ratio = 0.8, maxd = 0.35, tol = 3.0, mininl = 15, model = "translation",
                        refine = false, proj = "rectilinear", blend = "smart", ev = 0.0,
                        note = "Stage or slide moved under a fixed camera: frames only shift. " *
                               "Switch Model to similarity if the sample also rotates."),
    "scan / flat document" => (det_px = 2500, nkp = 5000, thr = 0.001, denoise = 0.5, mode = "all pairs",
                        ratio = 0.8, maxd = 0.35, tol = 3.0, mininl = 20, model = "affine",
                        refine = false, proj = "rectilinear", blend = "smart", ev = 0.0,
                        note = "Flat object (map, painting, document) scanned or photographed in parts."),
)

mutable struct Item
    path::String
    img::Matrix{RGB{Float32}}
    disp::Matrix{RGB{Float32}}
    f::Float64                    # disp = img resized by f
    feats::Any                    # (pts, desc) from St.detect, or nothing
    featkey::Any                  # detection parameters `feats` was computed with
    offset::Vector{Float64}       # manual nudge (output pixels: yaw, pitch × focal)
    gain::Float64
end

struct PairResult
    p1::Matrix{Float64}           # matched points in image a (full resolution)
    p2::Matrix{Float64}           # ... and in image b
    H::Matrix{Float64}            # maps image b into image a
    inl::Vector{Int}
    ok::Bool                      # accepted and used for placement
    listed::Bool                  # shown in the Pair menu
    msg::String
end

Base.@kwdef mutable struct State
    items::Vector{Item} = Item[]
    # Matching results keyed by the two images (a, b); kept when images are
    # reordered or other images removed.
    pairs::Dict{Tuple{Item,Item},PairResult} = Dict{Tuple{Item,Item},PairResult}()
    sel::Int = 0                  # current image
    selected::Set{Int} = Set{Int}()   # multi-selection in the list
    ref::Union{Nothing,Item} = nothing   # nothing = automatic (centre of the group)
    view::Symbol = :image         # :image, :pair, :pano
    pairkeys::Vector{Tuple{Item,Item}} = Tuple{Item,Item}[]   # Pair menu entries
    pairsel::Union{Nothing,Tuple{Item,Item}} = nothing
    busy::Bool = false
    preview_pending::Bool = false
    layout::Any = nothing         # placement: tree, transforms, focal length
    pano::Any = nothing           # NamedTuple describing the last preview
    detail_plot::Any = nothing
    view_version::Int = 0
    list_top::Int = 0
    crop::Any = nothing           # (box = output box, key = geometry it belongs to)
    undo::Vector{Any} = Any[]     # (state, label) entries, oldest first
    redo::Vector{Any} = Any[]
    committed::Any = nothing      # state after the last recorded change
    pending::Any = nothing        # a change seen once, recorded when it stops changing
    restoring::Bool = false
    project::Union{Nothing,String} = nothing
    api::Any = nothing            # project / undo functions, for scripting and tests
    closed::Bool = false          # the window was replaced (appearance switched)
end

refidx(st) = st.layout === nothing ? 0 : st.layout.ref
shortname(it) = basename(it.path)
center(it) = ((size(it.img, 2) + 1) / 2, (size(it.img, 1) + 1) / 2)
indexof(st) = IdDict(it => i for (i, it) in enumerate(st.items))

function make_item(path; raw_decoder = :auto, raw_half = false)
    img = St.load_any(path; raw_decoder, raw_half)
    f = min(1.0, DISP_MAX / maximum(size(img)))
    disp = f < 1 ? St.resize(img, f) : img
    return Item(path, img, disp, f, nothing, nothing, [0.0, 0.0], 1.0)
end

# Map display-copy pixels to full-resolution pixels.
undisp(f) = [1/f 0 0.5-0.5/f; 0 1/f 0.5-0.5/f; 0 0 1]

# Run `f` on a worker thread while the window keeps redrawing.
bg(f) = fetch(Threads.@spawn f())

include("dialogs.jl")

function expand_paths(s)
    s = expanduser(strip(s))
    isdir(s) || return [s]
    # Skip earlier results saved into the same folder (default name "panorama…").
    fs = filter(f -> lowercase(splitext(f)[2]) in St.IMAGE_EXTS &&
                     !startswith(lowercase(basename(f)), "panorama"), readdir(s; join = true))
    return sort(fs)
end

"""Is (x, y) inside the polygon `pts` (NaN points are skipped)?"""
function inside(pts, x, y)
    P = [p for p in pts if all(isfinite, p)]
    c = false
    for k in eachindex(P)
        (x1, y1), (x2, y2) = P[k], P[mod1(k + 1, length(P))]
        ((y1 > y) != (y2 > y)) && x < x1 + (y - y1) * (x2 - x1) / (y2 - y1) && (c = !c)
    end
    return c
end

# --------------------------------------------------------------------------
# Pages (tabs) and shared widgets
# --------------------------------------------------------------------------

"""
Several GridLayouts sharing one cell of `parent`, one shown at a time
(`P.current[] = k`). Makie has no tab widget, so the hidden pages are taken out
of the layout and parked off-screen, where they receive no mouse events.
"""
struct Pages
    parent::GridLayout
    rows::Any
    cols::Any
    pages::Vector{GridLayout}
    current::Observable{Int}
end

function Pages(parent, rows, cols, n; kw...)
    pages = [parent[rows, cols] = GridLayout(; valign = :top, halign = :left, kw...) for _ in 1:n]
    P = Pages(parent, rows, cols, pages, Observable(0))
    on(k -> showpage!(P, k), P.current)
    return P
end

function showpage!(P::Pages, k)
    for (i, p) in enumerate(P.pages)
        gc = GridLayoutBase.gridcontent(p)        # stays set (without a parent) once removed
        attached = gc !== nothing && gc.parent !== nothing
        if i == k
            attached || (P.parent[P.rows, P.cols] = p)
        elseif attached
            GridLayoutBase.remove_from_gridlayout!(gc)
            p.layoutobservables.suggestedbbox[] = Rect2f(-100_000, -100_000, 1000, 1000)
        end
    end
    GridLayoutBase.update!(P.parent)
end

# A label + slider + number box in row k of `grid`. Type a value into the box
# and press Enter to set it (snapped to the slider's step and range).
fmt(v) = v isa Integer ? string(v) : (r = round(v; digits = 4); r == round(r) ? string(round(Int, r)) : string(r))
function numslider!(grid, k, sp, numboxes)
    Label(grid[k, 1], sp.label, halign = :left)
    sl = Slider(grid[k, 2], range = sp.range, startvalue = sp.startvalue)
    tb = Textbox(grid[k, 3], stored_string = fmt(sl.value[]), width = 62, validator = Float64,
                 textpadding = (4, 4, 3, 3), fontsize = 13)
    syncing_box = Ref(false)                     # box and slider update each other
    on(sl.value) do v
        syncing_box[] && return
        syncing_box[] = true
        tb.stored_string[] = fmt(v); tb.displayed_string[] = fmt(v)
        syncing_box[] = false
    end
    on(tb.stored_string) do str
        syncing_box[] && return
        v = tryparse(Float64, str)
        v === nothing || set_close_to!(sl, clamp(v, first(sp.range), last(sp.range)))
        syncing_box[] = true
        tb.stored_string[] = fmt(sl.value[]); tb.displayed_string[] = fmt(sl.value[])
        syncing_box[] = false
    end
    push!(numboxes, tb)
    return sl
end


# --------------------------------------------------------------------------
# Appearance: light or dark colours, classic or modern look (flat, rounded,
# icons), full or simple layout (advanced controls left out, for beginners).
# The choice is kept in ui_file(); changing it rebuilds the window (run_gui).
# --------------------------------------------------------------------------

"""
Folder for the program's settings: %APPDATA%\\PanoStitch on Windows,
~/Library/Application Support/PanoStitch on macOS, ~/.config/panostitch on
Linux. Found when needed, not when the package is built (a compiled app runs
on other machines).
"""
function config_dir()
    Sys.iswindows() && return joinpath(get(ENV, "APPDATA", joinpath(homedir(), "AppData", "Roaming")), "PanoStitch")
    Sys.isapple() && return joinpath(homedir(), "Library", "Application Support", "PanoStitch")
    return joinpath(get(ENV, "XDG_CONFIG_HOME", joinpath(homedir(), ".config")), "panostitch")
end
ui_file() = joinpath(config_dir(), "ui.toml")
const UI = Ref((dark = false, modern = false, simple = false))

function load_ui()
    f = ui_file()
    d = try isfile(f) ? TOML.parsefile(f) : Dict() catch; Dict() end
    return (dark = get(d, "dark", false) === true, modern = get(d, "modern", false) === true,
            simple = get(d, "simple", false) === true)
end
function save_ui(ui)
    try
        f = ui_file()
        mkpath(dirname(f))
        open(io -> TOML.print(io, Dict(string(k) => v for (k, v) in pairs(ui))), f, "w")
    catch e
        @warn "Could not save the appearance settings" exception = e
    end
end

"""Colours of the parts drawn by hand (lists, status line, accents) for an appearance."""
function palette(ui)
    if ui.dark
        return (bg = RGBf(0.13, 0.13, 0.15), panel = RGBf(0.21, 0.21, 0.24), fg = RGBf(0.9, 0.9, 0.92),
                muted = RGBf(0.6, 0.6, 0.65), list = RGBf(0.09, 0.09, 0.11), border = RGBf(0.33, 0.33, 0.38),
                button = RGBf(0.24, 0.24, 0.28), hover = RGBf(0.31, 0.31, 0.36), off = RGBf(0.42, 0.42, 0.46),
                accent = RGBf(0.2, 0.42, 0.65), go = RGBf(0.17, 0.5, 0.3), onaccent = RGBf(0.97, 0.97, 0.97),
                tab = RGBf(0.2, 0.42, 0.65), status = RGBf(1.0, 0.62, 0.5), ok = RGBf(0.45, 0.85, 0.5),
                bad = RGBf(1.0, 0.45, 0.42), info = RGBf(0.52, 0.72, 1.0), slider = RGBf(0.4, 0.6, 0.95),
                knob = RGBf(0.92, 0.92, 0.95))
    elseif ui.modern
        return (bg = RGBf(0.97, 0.97, 0.98), panel = RGBf(0.9, 0.91, 0.93), fg = RGBf(0.1, 0.1, 0.12),
                muted = RGBf(0.42, 0.43, 0.47), list = RGBf(1, 1, 1), border = RGBf(0.8, 0.81, 0.84),
                button = RGBf(0.9, 0.91, 0.94), hover = RGBf(0.84, 0.86, 0.9), off = RGBf(0.72, 0.72, 0.75),
                accent = RGBf(0.2, 0.47, 0.85), go = RGBf(0.16, 0.6, 0.38), onaccent = RGBf(1, 1, 1),
                tab = RGBf(0.2, 0.47, 0.85), status = RGBf(0.65, 0.12, 0.1), ok = RGBf(0.0, 0.45, 0.15),
                bad = RGBf(0.75, 0.15, 0.12), info = RGBf(0.2, 0.4, 0.85), slider = RGBf(0.2, 0.47, 0.85),
                knob = RGBf(1, 1, 1))
    end
    return (bg = RGBf(1, 1, 1), panel = RGBf(0.92, 0.92, 0.92), fg = RGBf(0, 0, 0), muted = RGBf(0.42, 0.42, 0.42),
            list = RGBf(1, 1, 1), border = RGBf(0.75, 0.75, 0.75), button = RGBf(0.94, 0.94, 0.94),
            hover = RGBf(0.85, 0.85, 0.85), off = RGBf(0.7, 0.7, 0.7), accent = RGBf(0.68, 0.85, 0.9),
            go = RGBf(0.56, 0.93, 0.56), onaccent = RGBf(0, 0, 0), tab = RGBf(0.69, 0.77, 0.87),
            status = RGBf(0.55, 0, 0), ok = RGBf(0, 0.39, 0), bad = RGBf(0.7, 0.13, 0.13),
            info = RGBf(0.25, 0.41, 0.88), slider = RGBf(0.4, 0.6, 0.95), knob = RGBf(1, 1, 1))
end
const PAL = Ref(palette(UI[]))

"""A button label with an icon in front (modern look only)."""
icon(sym, txt) = UI[].modern ? "$sym $txt" : txt

"""Remove blocks (simple mode) and let their layouts close the gaps."""
function remove_blocks!(blocks)
    parents = unique([gc.parent for gc in (GridLayoutBase.gridcontent(b) for b in blocks)
                      if gc !== nothing && gc.parent !== nothing])
    foreach(delete!, blocks)
    for gl in parents                            # empty rows would share the spare height
        used = Set(r for gc in gl.content for r in gc.span.rows)
        foreach(r -> r in used || rowsize!(gl, r, Fixed(0)), 1:nrows(gl))
        GridLayoutBase.update!(gl)
    end
end

"""Makie theme for an appearance (the classic light look keeps Makie's defaults)."""
function ui_theme(ui)
    (ui.dark || ui.modern) || return Theme()
    P = palette(ui)
    r = ui.modern ? 9 : 4
    th = Theme(
        backgroundcolor = P.bg, textcolor = P.fg,
        Button = (buttoncolor = P.button, buttoncolor_hover = P.hover, buttoncolor_active = P.accent,
                  labelcolor = P.fg, labelcolor_hover = P.fg, labelcolor_active = P.onaccent,
                  cornerradius = r, strokecolor = P.border, strokewidth = ui.modern ? 0 : 1),
        Menu = (cell_color_inactive_even = P.button, cell_color_inactive_odd = P.button,
                cell_color_hover = P.hover, cell_color_active = P.accent, selection_cell_color_inactive = P.button,
                textcolor = P.fg, textcolor_active = P.onaccent, textcolor_hover = P.fg, dropdown_arrow_color = P.muted),
        Textbox = (boxcolor = P.list, boxcolor_hover = P.list, boxcolor_focused = P.list, bordercolor = P.border,
                   bordercolor_hover = P.muted, bordercolor_focused = P.accent, textcolor = P.fg,
                   textcolor_placeholder = P.muted, cursorcolor = P.fg, cornerradius = r),
        Toggle = (framecolor_inactive = P.off, framecolor_active = P.accent, buttoncolor = P.knob),
        Slider = (color_inactive = P.panel, color_active = P.slider, color_active_dimmed = P.slider),
        Label = (color = P.fg,),
        Axis = (backgroundcolor = P.list, xgridcolor = (P.fg, 0.1), ygridcolor = (P.fg, 0.1),
                leftspinecolor = P.border, rightspinecolor = P.border, topspinecolor = P.border,
                bottomspinecolor = P.border, xtickcolor = P.muted, ytickcolor = P.muted,
                xticklabelcolor = P.fg, yticklabelcolor = P.fg, titlecolor = P.fg, xlabelcolor = P.fg,
                ylabelcolor = P.fg),
    )
    if ui.modern                                   # the shipped DejaVu fonts have the icon symbols
        th = merge(th, Theme(fonts = (; regular = font_file("DejaVuSans.ttf"), bold = font_file("DejaVuSans-Bold.ttf"))))
    end
    return th
end

include("stacker_gui.jl")

# --------------------------------------------------------------------------
# GUI
# --------------------------------------------------------------------------

const TABS = ("Panorama stitcher", "Star stacking (landscape / trails)")

"""
Build the window: tab bar (with the appearance switches) and both tabs. `ui`
is the appearance; `restore` (from a previous window) brings back both tabs'
work; `on_ui(new_ui, restore)` is called when the appearance is switched.
"""
function build_gui(paths = String[]; watch = true, ui = UI[], restore = nothing, on_ui = nothing)
    UI[] = ui
    PAL[] = palette(ui)
    with_theme(ui_theme(ui)) do
        build_window(paths; watch, restore, on_ui)
    end
end

function build_window(paths; watch, restore, on_ui)
    P, ui = PAL[], UI[]
    fig = Figure(size = (1750, 1010), fontsize = ui.modern ? 13 : 14, backgroundcolor = P.bg)
    bar = fig[1, 1] = GridLayout(halign = :left, tellwidth = false)
    look = fig[1, 1] = GridLayout(halign = :right, tellwidth = false)
    tabs = Pages(fig.layout, 2, 1, length(TABS); width = Relative(1), height = Relative(1))
    tabbuttons = [Button(bar[1, k], label = l, width = 290, font = :bold) for (k, l) in enumerate(TABS)]
    on(tabs.current) do k
        for (i, b) in enumerate(tabbuttons)
            b.buttoncolor[] = i == k ? P.tab : P.button
            b.labelcolor[] = i == k && (ui.dark || ui.modern) ? P.onaccent : P.fg
        end
    end
    for (k, b) in enumerate(tabbuttons)
        on(_ -> tabs.current[] != k && (tabs.current[] = k), b.clicks)
    end
    switches = map(enumerate((("☾", "Dark", ui.dark), ("◆", "Modern", ui.modern), ("✓", "Simple", ui.simple)))) do (k, (sym, l, v))
        t = Toggle(look[1, 2k-1], active = v)
        Label(look[1, 2k], icon(sym, l), halign = :left, padding = (0, 14, 0, 0))
        t
    end
    stackprojs = filter(p -> endswith(lowercase(p), STACK_PROJECT_EXT), paths)
    # Lay the window out once at the end instead of after every one of the
    # hundreds of widgets (that alone took over a second).
    fig.layout.block_updates = true
    local st, sst
    try
        st = build_stitcher!(fig, tabs.pages[1], setdiff(paths, stackprojs); watch, active = () -> tabs.current[] == 1)
        sst = build_stacker!(fig, tabs.pages[2]; active = () -> tabs.current[] == 2)
    finally
        fig.layout.block_updates = false
    end
    st.api = merge(st.api, (; stacker = sst))
    sst.to_stitcher = ps -> (st.api.add(ps); tabs.current[] = 1)     # stacked panels → panorama
    tabs.current[] = 1                           # lays out the window (showpage!)
    isempty(stackprojs) || (tabs.current[] = 2; sst.api.open_project(first(stackprojs)))
    if restore !== nothing
        restore.stitch === nothing || st.api.open_project(restore.stitch)
        restore.stack === nothing || sst.api.open_project(restore.stack)
        restore.result === nothing || sst.api.set_result(restore.result)
        tabs.current[] = restore.tab
    end

    # The work of both tabs, to carry over into a rebuilt window.
    function carry()
        dir = mktempdir()
        sp = isempty(st.items) ? nothing : st.api.save_project(joinpath(dir, "pano"))
        kp = sst.api.save_project(joinpath(dir, "stack"))
        return (stitch = sp, stack = kp, result = sst.result, tab = tabs.current[])
    end
    onany((t.active for t in switches)...) do d, m, sm
        new = (dark = d, modern = m, simple = sm)
        save_ui(new)
        on_ui === nothing || on_ui(new, carry())
    end
    return fig, st
end

function build_stitcher!(fig, root, paths; watch = true, active = () -> true)
    st = State()
    status = Observable("Add images to begin (any order).")
    pairtxt = Observable("")

    P = PAL[]
    left  = root[1, 1] = GridLayout(tellheight = false, valign = :top)
    mid   = root[1, 2] = GridLayout(tellwidth = false, valign = :top)
    right = root[1, 3] = GridLayout(tellheight = false, valign = :top)
    colsize!(root, 1, Fixed(390))
    colsize!(root, 3, Fixed(330))
    # Advanced controls, removed in simple mode (their values stay at the defaults / preset).
    adv = Any[]
    mark() = length(fig.content)
    advanced!(n0; keep = ()) = append!(adv, [b for b in fig.content[n0+1:end] if !any(k -> k === b, keep)])

    row = 0
    nextrow() = (row += 1)
    header(g, r, txt) = Label(g[r, 1:4], txt; font = :bold, fontsize = 16,
                              halign = :left, padding = (0, 0, 0, 4))
    buttonrow(g, labels) = (sub = g[nextrow(), 1:4] = GridLayout();
                            [Button(sub[1, k], label = l, tellwidth = false) for (k, l) in enumerate(labels)])

    # Rows of label + slider + number box.
    numboxes = Textbox[]
    function numsliders(g, specs...)
        grid = g[nextrow(), 1:4] = GridLayout()
        sliders = [numslider!(grid, k, sp, numboxes) for (k, sp) in enumerate(specs)]
        colgap!(grid, 6); rowgap!(grid, 4)
        return (; sliders, labels = [sp.label for sp in specs])
    end

    # ---------------- left: images, features, matching ----------------
    b_open, b_saveproj = buttonrow(left, (icon("⇪", "Open project…"), icon("⬇", "Save project…")))
    header(left, nextrow(), "Images")
    Label(left[row, 2:4], "Ctrl/Shift-click: select several", color = P.muted, fontsize = 12,
          halign = :right)
    b_add, b_rm, b_clear, b_up, b_down = buttonrow(left, (icon("✚", "Add…"), icon("✖", "Remove"),
                                                          icon("⌫", UI[].modern ? "Clear" : "Clear all"), "▲", "▼"))
    pathbox = Textbox(left[nextrow(), 1:4], placeholder = "…or type a file/folder path + Enter",
                      width = 370, halign = :left)
    listax = Axis(left[nextrow(), 1:4], height = LIST_ROWS * 17,
                  backgroundcolor = P.list)
    hidedecorations!(listax)
    foreach(k -> deregister_interaction!(listax, k), collect(keys(interactions(listax))))
    n0 = mark()
    Label(left[nextrow(), 1], "Reference", halign = :left)
    m_ref = Menu(left[row, 2:4], options = ["auto (centre)"], width = 270)
    Label(left[nextrow(), 1], "RAW via", halign = :left)
    m_raw = Menu(left[row, 2], options = ["auto"; string.(St.available_raw_decoders())], width = 100)
    t_half = Toggle(left[row, 3], active = true, halign = :right)
    Label(left[row, 4], "half size", halign = :left)
    advanced!(n0)
    Label(left[nextrow(), 1], "Preset", halign = :left, font = :bold)
    m_preset = Menu(left[row, 2:4], options = ["custom"; collect(keys(PRESETS))], width = 270)

    n0 = mark()
    header(left, nextrow(), "1 · Features")
    sg_feat = numsliders(left,
        (label = "Detection size (px)", range = 500:100:6000, startvalue = 2000),
        (label = "Max keypoints",    range = 500:500:15000, startvalue = 5000),
        (label = "Corner threshold", range = 0.0:0.0001:0.02, startvalue = 0.001),
        (label = "Denoise (σ px)",   range = 0:0.05:4,      startvalue = 1.5))
    b_detect, b_all = buttonrow(left, ("Detect features", icon("▶", "Run all steps")))

    header(left, nextrow(), "2 · Matching")
    Label(left[nextrow(), 1], "Pairs", halign = :left)
    m_mode = Menu(left[row, 2], options = collect(MODES), width = 110)
    t_snake = Toggle(left[row, 3], active = false, halign = :right)
    Label(left[row, 4], "snake", halign = :left)
    sg_match = numsliders(left,
        (label = "Grid columns",     range = 1:60,           startvalue = 5),
        (label = "Ratio test",       range = 0.5:0.005:0.95, startvalue = 0.8),
        (label = "Max distance",     range = 0.1:0.005:0.5,  startvalue = 0.35),
        (label = "RANSAC tol. (px)", range = 0.5:0.1:20,     startvalue = 5.0),
        (label = "Min. inliers",     range = 6:1:100,        startvalue = 20))
    s_cols, s_ratio, s_maxd, s_tol, s_mininl = sg_match.sliders
    Label(left[nextrow(), 1], "Model", halign = :left)
    m_model = Menu(left[row, 2], options = collect(string.(St.MODELS)), width = 110)
    t_refine = Toggle(left[row, 3], active = true, halign = :right)
    Label(left[row, 4], "refine", halign = :left)
    b_match, b_match1, b_discard = buttonrow(left, ("Match", "Re-match pair", "Discard pair"))
    Label(left[nextrow(), 1], "Pair", halign = :left)
    m_pair = Menu(left[row, 2], options = ["(none)"], width = 150)
    t_out = Toggle(left[row, 3], active = true, halign = :right)
    Label(left[row, 4], "outliers", halign = :left)
    Label(left[nextrow(), 1:4], pairtxt; halign = :left, justification = :left,
          word_wrap = true, width = 370, fontsize = 13)
    advanced!(n0; keep = (b_all,))
    rowgap!(left, 3)

    # ---------------- right: projection and composition ----------------
    row = 0
    header(right, nextrow(), "3 · Projection")
    m_proj = Menu(right[nextrow(), 1:4], options = collect(string.(St.PROJECTIONS)),
                  default = "cylindrical", width = 310)
    # Focal / yaw / pitch / roll stay in simple mode: straightening the horizon is basic.
    sg_view = numsliders(right,
        (label = "Focal (× auto)", range = 0.3:0.005:3.0, startvalue = 1.0),
        (label = "Yaw (°)",        range = -180:0.1:180,  startvalue = 0.0),
        (label = "Pitch (°)",      range = -90:0.1:90,    startvalue = 0.0),
        (label = "Roll (°)",       range = -45:0.05:45,   startvalue = 0.0))
    s_focal, s_yaw, s_pitch, s_roll = sg_view.sliders
    b_resetview = Button(right[nextrow(), 1:4], label = "Reset focal / yaw / pitch / roll")
    header(right, nextrow(), "4 · Compose")
    n0 = mark()
    Label(right[nextrow(), 1], "Blend", halign = :left)
    m_blend = Menu(right[row, 2:4], options = collect(string.(St.BLENDS)), width = 220)
    advanced!(n0)
    # One grid, so the three sliders line up; only Exposure (row 2) stays in simple mode.
    n0 = mark()
    sg_comp = numsliders(right,
        (label = "Preview size (px)", range = 500:100:4000,  startvalue = 1800),
        (label = "Exposure (EV)",     range = -3:0.05:8,     startvalue = 0.0),
        (label = "Brightness (sel.)", range = 0.3:0.005:3.0, startvalue = 1.0))
    s_prev, s_ev, s_gain = sg_comp.sliders
    append!(adv, [b for b in fig.content[n0+1:end] if GridLayoutBase.gridcontent(b).span.rows != 2:2])
    n0 = mark()
    t_unl = Toggle(right[nextrow(), 1], active = false, halign = :left)
    Label(right[row, 2:4], "show unlinked images", halign = :left, fontsize = 13)
    advanced!(n0)
    b_autoev, b_autogain, b_resetadj = buttonrow(right, ("Auto exposure", "Even out", "Reset moves"))
    b_crop, b_uncrop = buttonrow(right, ("Auto crop", "Uncrop"))
    n0 = mark()
    Label(right[nextrow(), 1:4], "Nudge selected image (or arrow keys):", halign = :left,
          fontsize = 13)
    nrow = nextrow()
    b_nl, b_nu, b_nd, b_nr = (Button(right[nrow, i], label = l, width = 64)
        for (i, l) in enumerate(("←", "↑", "↓", "→")))
    header(right, nextrow(), "5 · Stretch")
    histax = Axis(right[nextrow(), 1:4], height = 70, backgroundcolor = :gray12,
                  xticklabelsize = 10, yticksvisible = false, yticklabelsvisible = false,
                  xgridvisible = false, ygridvisible = false, limits = (0, 1, 0, 1.05))
    foreach(k -> deregister_interaction!(histax, k), collect(keys(interactions(histax))))
    sg_str = numsliders(right,
        (label = "Black point", range = 0:0.0005:0.5,         startvalue = 0.0),
        (label = "Midtones",    range = 0.0005:0.0005:0.9995, startvalue = 0.5),
        (label = "White point", range = 0.5:0.0005:1.0,       startvalue = 1.0))
    s_black, s_mid, s_white = sg_str.sliders
    b_autostr, b_resetstr = buttonrow(right, ("Auto stretch", "Reset stretch"))
    advanced!(n0)
    b_preview = Button(right[nextrow(), 1:4], label = icon("◐", "Preview panorama"), buttoncolor = P.accent,
                       labelcolor = P.onaccent, height = 40)
    b_save = Button(right[nextrow(), 1:4], label = icon("⬇", "Save full resolution…"), buttoncolor = P.go,
                    labelcolor = P.onaccent, height = 40)
    rowgap!(right, 4)

    # ---------------- middle: view ----------------
    n0 = mark()
    b_vimg  = Button(mid[1, 1], label = "Image + keypoints")
    b_vpair = Button(mid[1, 2], label = "Pair matches")
    advanced!(n0)
    b_vpano = Button(mid[1, 3], label = "Panorama")
    b_zin   = Button(mid[1, 4], label = icon("⊕", "Zoom +"))
    b_zout  = Button(mid[1, 5], label = icon("⊖", "Zoom -"))
    b_fit   = Button(mid[1, 6], label = icon("▣", "Fit"))
    t_ovl = Toggle(mid[1, 7], active = true)
    Label(mid[1, 8], "overlays", halign = :left)
    b_undo = Button(mid[1, 9], label = icon("↶", "Undo"))
    b_redo = Button(mid[1, 10], label = icon("↷", "Redo"))
    ax = Axis(mid[2, 1:10], yreversed = true, aspect = DataAspect(),
              xgridvisible = false, ygridvisible = false)
    deregister_interaction!(ax, :rectanglezoom)      # left-drag pans instead
    ax.panbutton = Mouse.left
    Label(mid[3, 1:10], status; halign = :left, justification = :left, word_wrap = true,
          tellwidth = false, color = P.status, fontsize = 15)
    rowsize!(mid, 2, Relative(0.92))

    featparams() = (det_px = sg_feat.sliders[1].value[], nkeypoints = sg_feat.sliders[2].value[],
                    threshold = sg_feat.sliders[3].value[], denoise = sg_feat.sliders[4].value[])
    model() = Symbol(m_model.selection[])
    blend() = Symbol(m_blend.selection[])
    projection() = Symbol(m_proj.selection[])
    refc() = center(st.items[refidx(st)])
    focal() = st.layout.f * s_focal.value[]

    # ---------------- background tasks ----------------

    function task!(f, msg)
        if st.busy
            status[] = "Busy – wait for the current step to finish."
            return
        end
        st.busy = true
        status[] = msg
        @async try
            sleep(0.05)                               # let the status message render
            f()
        catch e
            e isa TaskFailedException && (e = e.task.exception)
            status[] = "Error: " * sprint(showerror, e)
            @error "PanoGUI step failed" exception = (e, catch_backtrace())
        finally
            st.busy = false
            if st.preview_pending
                st.preview_pending = false
                request_preview()
            end
        end
    end

    # ---------------- layout (which image goes where) ----------------

    """Accepted pairs as (i, j, PairResult) with current indices (H maps j into i)."""
    function edges()
        idx = indexof(st)
        return [(idx[a], idx[b], pr) for ((a, b), pr) in st.pairs
                if pr.ok && haskey(idx, a) && haskey(idx, b)]
    end

    """Inlier points of the pair linking images i and j, as (in i, in j)."""
    function pair_points(i, j)
        a, b = st.items[i], st.items[j]
        haskey(st.pairs, (a, b)) && (pr = st.pairs[(a, b)]; return pr.p1[:, pr.inl], pr.p2[:, pr.inl])
        pr = st.pairs[(b, a)]
        return pr.p2[:, pr.inl], pr.p1[:, pr.inl]
    end

    """
    Recompute the placement: spanning tree over accepted pairs, reference image,
    transforms, and (if enabled) the global refinement. Pairs that do not fit the
    refined solution are rejected and the placement is redone once.
    """
    function relayout!()
        n = length(st.items)
        n == 0 && (st.layout = nothing; return "")
        nrej = 0
        for pass in 1:4
            E = edges()
            adj = St.spanning_tree(n, [(i, j, length(pr.inl)) for (i, j, pr) in E])
            idx = indexof(st)
            ref = st.ref !== nothing && haskey(idx, st.ref) ? idx[st.ref] :
                  isempty(E) ? cld(n, 2) : St.tree_center(adj)
            parent, order, comp = St.walk_tree(adj, [ref; 1:n])
            members = findall(==(comp[ref]), comp)
            cs = [center(it) for it in st.items]
            Hd = Dict((i, j) => pr.H for (i, j, pr) in E)
            f = something(St.estimate_focal([(pr.H, cs[i], cs[j]) for (i, j, pr) in E]),
                          Float64(maximum(size(st.items[ref].img))))
            G = St.place(Hd, parent, order, ref, [size(it.img, 2) for it in st.items])
            err = nothing; k1 = 0.0
            if t_refine.active[] && model() === :homography && length(members) > 1
                M = [(i, j, pair_points(i, j)...) for (i, j, _) in E]
                tol = s_tol.value[]
                G, f, k1, e0, e1, med = bg(() -> St.bundle_adjust(G, cs, ref, f, members, M; huber = tol))
                fin = filter(isfinite, med)
                typ = isempty(fin) ? 0.0 : median(fin)
                err = (before = e0, after = e1, typical = typ)
                # Reject pairs far off the common solution, or clearly worse than
                # typical while resting on few inliers (false matches, drifting clouds).
                weak = 2 * s_mininl.value[]
                bad = [k for k in eachindex(E) if isfinite(med[k]) &&
                       (med[k] > max(4tol, 8) || (med[k] > max(3typ, 2tol) && length(E[k][3].inl) < weak))]
                if pass < 4 && !isempty(bad)
                    for k in bad
                        i, j, pr = E[k]
                        a, b = st.items[i], st.items[j]
                        key = haskey(st.pairs, (a, b)) ? (a, b) : (b, a)
                        st.pairs[key] = PairResult(pr.p1, pr.p2, pr.H, pr.inl, false, true,
                            "Pair $(i)–$(j) REJECTED by the global refinement: it is off by " *
                            "$(round(med[k], digits = 1)) px from all other pairs, so it is " *
                            "probably a false match (or something moved). Re-match it to try again.")
                    end
                    nrej += length(bad)
                    continue
                end
            end
            G = park_unlinked(G, parent, order, comp, ref, members, f, cs)
            st.layout = (; parent, order, comp, ref, members, G, f, err, lens = (k1, f))
            break
        end
        return nrej == 0 ? "" : " Rejected $nrej pair(s) that did not fit the others."
    end

    """
    Move each group of images not linked to the reference into a row below the
    panorama (as camera rotations), keeping each group's internal arrangement.
    """
    function park_unlinked(G, parent, order, comp, ref, members, f, cs)
        roots = [v for v in order if parent[v] == 0 && comp[v] != comp[ref]]
        isempty(roots) && return G
        Kr = St.kmat(f, cs[ref])
        # Lowest pitch (y points down) of the linked images' centres.
        pitch(i) = (d = Kr \ (G[i] * [cs[i]..., 1.0]); atan(d[2], hypot(d[1], d[3])))
        h, w = size(st.items[ref].img)
        vfov, hfov = 2atan(h / 2f), 2atan(w / 2f)
        p0 = min(maximum(pitch(i) for i in members) + 1.1vfov, deg2rad(80))
        G = copy(G)
        for (k, r) in enumerate(roots)
            yaw = (k - (length(roots) + 1) / 2) * 1.1hfov
            M = St.rotation_h(yaw, p0, f, cs[ref]) * inv(G[r])
            for v in order
                comp[v] == comp[r] && (G[v] = M * G[v]; G[v] ./= cbrt(det(G[v])))
            end
        end
        return G
    end

    """Indices of the images that go into the panorama."""
    visible() = (L = st.layout; [i for i in eachindex(st.items)
                                 if t_unl.active[] || L.comp[i] == L.comp[L.ref]])

    """Suggest the spherical projection for panoramas that reach far up or down."""
    function tall_hint()
        L = st.layout
        (L === nothing || projection() === :spherical) && return ""
        cs = [center(it) for it in st.items]
        Kr = St.kmat(L.f, cs[L.ref])
        steep = any(L.members) do i
            d = Kr \ (L.G[i] * [cs[i]..., 1.0])
            abs(atan(d[2], hypot(d[1], d[3]))) > deg2rad(45)
        end
        return steep ? " Tip: the panorama reaches far up/down; the spherical projection suits it better." : ""
    end

    """Transform of every image into the (rotated) reference plane, including nudges."""
    function placements(f, c)
        L = st.layout
        nud(i) = St.rotation_h(st.items[i].offset[1] / f, st.items[i].offset[2] / f, f, c)
        N = [Matrix{Float64}(I, 3, 3) for _ in st.items]      # nudges carry down the tree
        for v in L.order
            v == L.ref && continue
            N[v] = L.parent[v] == 0 ? nud(v) : N[L.parent[v]] * nud(v)
        end
        V = St.rotation_h(deg2rad(s_yaw.value[]), deg2rad(s_pitch.value[]), f, c;
                          roll = deg2rad(s_roll.value[]))
        return [V * N[i] * L.G[i] for i in eachindex(st.items)]
    end

    # ---------------- image list ----------------

    function draw_list!()
        empty!(listax)
        n = length(st.items)
        st.list_top = clamp(st.list_top, 0, max(0, n - LIST_ROWS))
        top = st.list_top
        # Row k occupies y ∈ [-k, -k+1], so the first image is at the top.
        limits!(listax, 0, 1, -(top + LIST_ROWS), -top)
        if n == 0
            text!(listax, 0.02, -(top + 0.5); text = "(no images)", align = (:left, :center),
                  fontsize = 13, color = PAL[].muted)
            return
        end
        rows = top+1:min(n, top + LIST_ROWS)
        sel = [Rect2f(0, -k, 1, 1) for k in rows if k in st.selected]
        isempty(sel) || poly!(listax, sel; color = (:dodgerblue, 0.25))
        st.sel in rows && poly!(listax, Rect2f(0, -st.sel, 1, 1); color = (:dodgerblue, 0.45))
        L = st.layout; r = refidx(st)
        matched = any(pr.ok for pr in values(st.pairs))
        labels = map(rows) do i
            it = st.items[i]
            kp = it.feats === nothing ? "" : " $(size(it.feats[1], 2))kp"
            link = matched && L !== nothing && L.comp[i] != L.comp[r] ? "  UNLINKED" : ""
            "$(i == r ? "R" : " ")$(lpad(i, 3)) $(first(shortname(it), 24))$kp$link"
        end
        text!(listax, [Point2f(0.01, -(k - 0.5)) for k in rows]; text = labels, align = (:left, :center),
              fontsize = 12.5, font = mono_font())
        n > LIST_ROWS && text!(listax, 0.99, -(top + 0.5); text = "$(first(rows))–$(last(rows)) of $n",
                               align = (:right, :center), fontsize = 11, color = PAL[].muted)
    end

    function scroll_to_sel!()
        st.sel < st.list_top + 1 && (st.list_top = st.sel - 1)
        st.sel > st.list_top + LIST_ROWS && (st.list_top = st.sel - LIST_ROWS)
    end

    # Setting menu options re-fires their selection callbacks; `syncing` mutes them.
    syncing = Ref(false)
    function refresh_lists!()
        syncing[] = true
        try
            sync_lists!()
        finally
            syncing[] = false
        end
    end
    function sync_lists!()
        n = length(st.items)
        draw_list!()
        names = ["$i: $(shortname(it))" for (i, it) in enumerate(st.items)]
        m_ref.options[] = ["auto (centre)"; names]
        idx = indexof(st)
        m_ref.i_selected[] = st.ref !== nothing && haskey(idx, st.ref) ? idx[st.ref] + 1 : 1
        st.pairkeys = sort([k for (k, pr) in st.pairs if pr.listed && haskey(idx, k[1]) && haskey(idx, k[2])];
                           by = k -> (idx[k[1]], idx[k[2]]))
        m_pair.options[] = isempty(st.pairkeys) ? ["(none)"] :
            [(pr = st.pairs[k]; "$(idx[k[1]])–$(idx[k[2]])  " * (pr.ok ? "$(length(pr.inl)) inl" : "rejected"))
             for k in st.pairkeys]
        k = findfirst(==(st.pairsel), st.pairkeys)
        isempty(st.pairkeys) || (m_pair.i_selected[] = something(k, 1))
        st.pairsel = isempty(st.pairkeys) ? nothing : st.pairkeys[something(k, 1)]
        n > 0 && set_close_to!(s_gain, st.items[st.sel].gain)
    end

    function list_changed!()
        n = length(st.items)
        st.selected = Set(i for i in st.selected if i <= n)
        st.sel = clamp(st.sel, min(n, 1), n)
        st.pano = nothing
        relayout!()
        refresh_lists!()
        show_current!(fit = true)
    end

    """Load images (several at once: RAW conversion runs in external programs)."""
    function load_items(ps, dec, half)
        done = Ref(0)
        failed = String[]
        items = asyncmap(ps; ntasks = max(2, Threads.nthreads())) do p
            it = try
                bg(() -> make_item(p; raw_decoder = dec, raw_half = half))
            catch e
                e isa TaskFailedException && (e = e.task.exception)
                push!(failed, sprint(showerror, e))
                nothing
            end
            done[] += 1
            status[] = "Loading $(done[])/$(length(ps)): $(basename(p))"
            it
        end
        return items, failed
    end

    function add_paths!(ps)
        isempty(ps) && return
        dec = Symbol(m_raw.selection[]); half = t_half.active[]
        task!("Loading $(length(ps)) image(s)…") do
            items, failed = load_items(ps, dec, half)
            append!(st.items, filter(!isnothing, items))
            st.sel = length(st.items); st.selected = Set([st.sel])
            scroll_to_sel!()
            st.view = :image
            fitsitems = [it for it in items if it !== nothing && St.isastro(it.path)]
            autonote = ""
            if !isempty(fitsitems) && !stretched()
                # FITS / XISF data is usually linear: stretch for viewing, based on all
                # of them together (a single over- or underexposed frame must not decide).
                auto_stretch!(reduce(vcat, [vec(it.disp[1:7:end]) for it in fitsitems]))
                autonote = " Auto stretch applied for viewing (FITS / XISF data is linear)."
            end
            list_changed!()
            status[] = "Loaded $(count(!isnothing, items)) image(s). Next: detect features " *
                       "(or Run all steps)." * autonote *
                       (isempty(failed) ? "" : "\nSkipped $(length(failed)):\n" * join(failed, "\n"))
        end
    end

    function remove!(idxs)
        (st.busy || isempty(idxs)) && return
        gone = Set(st.items[i] for i in idxs)
        st.items = [it for it in st.items if !(it in gone)]
        filter!(kv -> !(kv[1][1] in gone || kv[1][2] in gone), st.pairs)
        st.ref in gone && (st.ref = nothing)
        st.selected = Set{Int}()
        st.sel = clamp(minimum(idxs), min(1, length(st.items)), length(st.items))
        st.sel > 0 && push!(st.selected, st.sel)
        list_changed!()
        status[] = "Removed $(length(gone)) image(s); $(length(st.items)) left."
    end

    function move!(d)
        (st.busy || isempty(st.items)) && return
        sel = sort(collect(isempty(st.selected) ? Set([st.sel]) : st.selected))
        (d < 0 ? first(sel) == 1 : last(sel) == length(st.items)) && return
        for i in (d < 0 ? sel : reverse(sel))
            st.items[i], st.items[i+d] = st.items[i+d], st.items[i]
        end
        st.selected = Set(i + d for i in sel); st.sel += d
        scroll_to_sel!()
        list_changed!()
    end

    # ---------------- stretch ----------------

    stretched() = !(s_black.value[] == 0 && s_mid.value[] == 0.5 && s_white.value[] == 1)
    """How a linear image is shown (and saved, except as FITS): exposure gain, then the stretch."""
    function look(A; gain = 1.0)
        B = gain == 1 ? A : A .* Float32(gain)
        return stretched() ? St.hist_stretch(B, s_black.value[], s_mid.value[], s_white.value[]) : B
    end
    """The linear image the histogram describes: the panorama preview or the current image."""
    function hist_source()
        st.view === :pano && st.pano !== nothing && return st.pano.img
        isempty(st.items) && return nothing
        return st.items[st.sel].disp .* Float32(2.0^s_ev.value[])
    end
    function auto_stretch!(src)
        b, m, w = St.auto_stretch(src)
        set_close_to!(s_white, w); set_close_to!(s_black, b); set_close_to!(s_mid, m)
    end
    """Histogram (log scale) with black / white point markers and the transfer curve."""
    function draw_histogram!()
        UI[].simple && return                    # not shown in simple mode
        empty!(histax)
        src = hist_source()
        src === nothing && return
        hc = St.lum_histogram(src)
        y = log10.(hc .+ 1); y ./= max(maximum(y), 1)
        xs = range(0.5 / length(y), 1 - 0.5 / length(y), length(y))
        band!(histax, collect(xs), zeros(length(y)), y; color = (:gray70, 0.8))
        b, m, w = s_black.value[], s_mid.value[], s_white.value[]
        cx = range(0, 1, 200)
        lines!(histax, cx, [St.mtf(m, clamp((x - b) / max(w - b, 1e-6), 0, 1)) for x in cx];
               color = :orange, linewidth = 1.5)
        vlines!(histax, [b]; color = :deepskyblue, linewidth = 1.5)
        vlines!(histax, [w]; color = :white, linewidth = 1.5)
    end
    """Redraw the current view with the new stretch (keeping the zoom)."""
    function restretch()
        st.restoring && return
        st.view === :pano ? (show_pano!(); render_detail!()) : show_current!()
        draw_histogram!()
    end

    # ---------------- drawing ----------------

    function show_image!(; fit = true)
        empty!(ax); st.detail_plot = nothing
        isempty(st.items) && (ax.title = ""; return)
        it = st.items[st.sel]
        h, w = size(it.img)
        image!(ax, 0.5 .. w + 0.5, 0.5 .. h + 0.5, permutedims(look(it.disp; gain = 2.0^s_ev.value[])))
        if it.feats !== nothing
            pts = it.feats[1]
            scatter!(ax, pts[1, :], pts[2, :]; marker = :circle, color = :transparent,
                     strokecolor = :yellow, strokewidth = 1.2, markersize = 8)
        end
        kp = it.feats === nothing ? "no features yet" : "$(size(it.feats[1], 2)) keypoints"
        ax.title = "Image $(st.sel): $(shortname(it))  ($(w)×$(h), $kp)"
        fit && autolimits!(ax)
    end

    function show_pair!(; fit = true)
        empty!(ax); st.detail_plot = nothing
        key = st.pairsel
        idx = indexof(st)
        if key === nothing || !haskey(st.pairs, key) || !haskey(idx, key[1]) || !haskey(idx, key[2])
            ax.title = "No pairs matched yet"; pairtxt[] = ""
            return
        end
        A, B = key
        i, j = idx[A], idx[B]
        hA, wA = size(A.img); hB, wB = size(B.img)
        xo = wA + 0.03 * wA
        g = 2.0^s_ev.value[]
        image!(ax, 0.5 .. wA + 0.5, 0.5 .. hA + 0.5, permutedims(look(A.disp; gain = g)))
        image!(ax, xo + 0.5 .. xo + wB + 0.5, 0.5 .. hB + 0.5, permutedims(look(B.disp; gain = g)))
        pr = st.pairs[key]
        isin = falses(size(pr.p1, 2)); isin[pr.inl] .= true
        seg(k) = Point2f(pr.p1[1, k], pr.p1[2, k]) => Point2f(pr.p2[1, k] + xo, pr.p2[2, k])
        if t_out.active[]
            linesegments!(ax, [seg(k) for k in findall(!, isin)]; color = (:red, 0.6), linewidth = 1)
        end
        linesegments!(ax, [seg(k) for k in findall(isin)]; color = (:lime, 0.9), linewidth = 1.2)
        ax.title = "Pair $(i)–$(j): $(size(pr.p1, 2)) matches, $(length(pr.inl)) inliers (green)" *
                   (t_out.active[] ? ", outliers red" : "") * (pr.ok ? "" : " · NOT USED")
        pairtxt[] = pr.msg
        fit && autolimits!(ax)
    end

    # Image extents (u, v) of a canvas rendered over `box` at `scale`.
    extents(box, scale, img) = (box[1] - 0.5 .. size(img, 2) / scale + box[1] - 0.5,
                                box[3] - 0.5 .. size(img, 1) / scale + box[3] - 0.5)

    function show_pano!(; fit = false)
        empty!(ax); st.detail_plot = nothing
        if st.pano === nothing || st.layout === nothing
            ax.title = "Panorama: press “Preview panorama”"
            return
        end
        P = st.pano
        translate!(image!(ax, extents(P.box, P.scale, P.img)..., permutedims(look(P.img))), 0, 0, -2)
        r = refidx(st)
        L = st.layout
        t_ovl.active[] && draw_overlays!(P, L, r)
        w = size(st.items[r].img, 2)
        fov = round(2 * atand(w / 2 / P.f), digits = 1)
        err = L.err === nothing ? "" : " · typical error $(round(L.err.typical, digits = 1)) px"
        ax.title = "$(P.projection): $(Int(P.box[2]-P.box[1]+1))×$(Int(P.box[4]-P.box[3]+1)) px · " *
                   "focal $(round(Int, P.f)) px (ref. FOV $(fov)°)$err" *
                   (t_ovl.active[] ? " · orange = selected, cyan = reference, yellow = links" : "")
        draw_histogram!()
        fit && autolimits!(ax)
    end

    """Outlines, numbers and links of the images on top of the panorama."""
    function draw_overlays!(P, L, r)
        mids = map(P.outlines) do o
            fin = filter(p -> all(isfinite, p), o)
            isempty(fin) ? Point2f(NaN, NaN) : Point2f(mean(first, fin), mean(last, fin))
        end
        # Links used for placement (the spanning tree).
        links = [mids[L.parent[v]] => mids[v] for v in L.order
                 if L.parent[v] != 0 && all(isfinite, mids[v]) && all(isfinite, mids[L.parent[v]])]
        isempty(links) || linesegments!(ax, links; color = (:yellow, 0.8), linewidth = 2, linestyle = :dash)
        for (i, o) in enumerate(P.outlines)
            isempty(o) && continue
            col = i == st.sel ? :orange : i in st.selected ? :deepskyblue : i == r ? :cyan : (:white, 0.7)
            lines!(ax, Point2f.([o; o[1:1]]); color = col, linewidth = i == st.sel ? 3 : 1.2)
            all(isfinite, mids[i]) || continue
            text!(ax, mids[i]; text = i == r ? "$i (ref)" : "$i", color = col, fontsize = 22,
                  align = (:center, :center), strokecolor = :black, strokewidth = 1)
        end
    end

    function show_current!(; fit = false)
        st.view === :image ? show_image!(; fit) :
        st.view === :pair  ? show_pair!(; fit)  : show_pano!(; fit)
        draw_histogram!()
    end

    # ---------------- steps ----------------

    function detect_all!()
        fp = featparams()
        todo = [it for it in st.items if it.featkey != fp]
        isempty(todo) && return
        status[] = "Detecting features in $(length(todo)) image(s)…"
        feats = bg() do
            fetch.([Threads.@spawn St.detect(it.img; scale = min(1.0, fp.det_px / maximum(size(it.img))),
                                             fp.nkeypoints, fp.threshold, fp.denoise) for it in todo])
        end
        changed = Set(todo)
        for (it, f) in zip(todo, feats)
            it.feats = f; it.featkey = fp
        end
        filter!(kv -> !(kv[1][1] in changed || kv[1][2] in changed), st.pairs)
        relayout!()
        refresh_lists!()
    end

    function compute_pair(i, j, A, B, p, listed)
        p1, p2 = St.match_pair(A.feats, B.feats; ratio = p.ratio, max_distance = p.maxd)
        name = "Pair $(i)–$(j) ($(p.model))"
        try
            H, inl = St.ransac(p2, p1; model = p.model, thresh = p.tol)
            ok, why = St.accept_pair(size(p1, 2), length(inl), H, size(B.img)...; min_inliers = p.mininl)
            err = isempty(inl) ? NaN : mean(St.reproj_err(H, p2, p1, k) for k in inl)
            msg = "$name: $(size(p1, 2)) matches, $(length(inl)) inliers " *
                  "($(round(Int, 100length(inl)/max(1, size(p1, 2))))%), mean error " *
                  "$(round(err, digits = 2)) px" * (ok ? "" : "\nREJECTED: $why.")
            return PairResult(p1, p2, H, inl, ok, listed || ok, msg)
        catch e
            e isa ErrorException || rethrow()
            return PairResult(p1, p2, zeros(3, 3), Int[], false, listed, "$name FAILED: $(e.msg)")
        end
    end

    """Match the pairs `ij` (index tuples) and redo the placement. Returns a summary."""
    function match!(ij; listed = true)
        detect_all!()
        p = (ratio = s_ratio.value[], maxd = s_maxd.value[], tol = s_tol.value[],
             mininl = s_mininl.value[], model = model())
        status[] = "Matching $(length(ij)) pair(s)…"
        items = st.items
        res = bg() do
            fetch.([Threads.@spawn compute_pair(i, j, items[i], items[j], p, listed) for (i, j) in ij])
        end
        for ((i, j), r) in zip(ij, res)
            st.pairs[(items[i], items[j])] = r
        end
        status[] = "Placing images" * (t_refine.active[] ? " and refining globally…" : "…")
        note = relayout!()
        L = st.layout
        nacc = count(r -> r.ok, res)
        unl = [i for i in eachindex(st.items) if L.comp[i] != L.comp[L.ref]]
        return "$nacc of $(length(ij)) pairs accepted." * note *
               (L.err === nothing ? "" : " Typical pair error after refinement: " *
                                         "$(round(L.err.typical, digits = 1)) px.") *
               (isempty(unl) ? " All images are linked." :
                " NOT linked: images $(join(unl, ", ")) – hidden; switch on “show unlinked images” " *
                "to place them by hand, or try other matching settings.") * tall_hint()
    end

    function candidate_pairs()
        mode = m_mode.selection[]
        n = length(st.items)
        mode == "all pairs" && return St.candidate_pairs(n, :all), false
        mode == "sequential" && return St.candidate_pairs(n, :sequential), true
        return St.candidate_pairs(n, :grid; cols = s_cols.value[], snake = t_snake.active[]), true
    end

    function render_preview!()
        isempty(st.items) && return
        f, c, proj = focal(), refc(), projection()
        G = placements(f, c)
        vis = visible()
        lens = st.layout.lens
        outlines = [i in vis ? St.outline(G[i], size(it.img)..., proj, f, c; lens) : NTuple{2,Float64}[]
                    for (i, it) in enumerate(st.items)]
        full = St.bounds(outlines[vis])
        key = hash((proj, f, vis, G))                    # the geometry this preview shows
        if st.crop !== nothing
            st.crop.key === :pending && (st.crop = (box = st.crop.box, key = key))   # from a project file
            st.crop.key == key || (st.crop = nothing)    # geometry changed: the crop no longer fits
        end
        box = st.crop === nothing ? full : st.crop.box
        side = max(box[2] - box[1], box[4] - box[3]) + 1
        p = min(1.0, s_prev.value[] / side)
        V = st.items[vis]
        use_disp = p <= minimum(it.f for it in V)
        imgs = [use_disp ? it.disp : it.img for it in V]
        scales = [use_disp ? it.f : 1.0 for it in V]
        sizes = [size(it.img) for it in V]
        gains = [it.gain * 2.0^s_ev.value[] for it in V]
        bl = blend()
        uncropped = st.crop === nothing
        res = bg(() -> St.compose(imgs, G[vis], box; scale = p, projection = proj, f, c, blend = bl,
                                  gains, scales, sizes, lens, coverage = uncropped))
        img, covered = uncropped ? res : (res, nothing)
        st.pano = (img = img, box = box, scale = p, G = G, vis = vis, outlines = outlines, f = f,
                   c = c, projection = proj, blend = bl, gains = gains, lens = lens,
                   covered = covered, key = key)
    end

    function request_preview(; fit = false)
        if st.busy
            st.preview_pending = true
            return
        end
        task!("Rendering preview…") do
            render_preview!()
            st.view = :pano
            show_pano!(; fit)
            status[] = "Preview ready. Drag to pan, scroll to zoom (detail sharpens). Click an image " *
                       "to select it; Ctrl+drag or arrow keys move it."
        end
    end

    # Re-render the visible part of the panorama at screen resolution.
    detail_busy = Ref(false)
    function render_detail!()
        P = st.pano
        (st.view === :pano && P !== nothing && !detail_busy[]) || return
        fl = ax.finallimits[]
        (x0, y0), (w, h) = fl.origin, fl.widths
        box = (max(P.box[1], floor(x0)), min(P.box[2], ceil(x0 + w)),
               max(P.box[3], floor(y0)), min(P.box[4], ceil(y0 + h)))
        (box[2] > box[1] && box[4] > box[3]) || return
        px = widths(ax.scene.viewport[])[1]
        need = min(1.0, px / w)
        if need <= 1.25 * P.scale
            st.detail_plot !== nothing && (delete!(ax, st.detail_plot); st.detail_plot = nothing)
            return
        end
        V = st.items[P.vis]
        use_disp = need <= minimum(it.f for it in V)
        imgs = [use_disp ? it.disp : it.img for it in V]
        scales = [use_disp ? it.f : 1.0 for it in V]
        sizes = [size(it.img) for it in V]
        version = st.view_version
        detail_busy[] = true
        @async try
            img = bg(() -> St.compose(imgs, P.G[P.vis], box; scale = need, projection = P.projection,
                                      f = P.f, c = P.c, blend = P.blend, gains = P.gains, scales, sizes,
                                      lens = P.lens))
            # Drop it if the view or the preview changed meanwhile.
            if version == st.view_version && st.pano === P && st.view === :pano
                st.detail_plot !== nothing && delete!(ax, st.detail_plot)
                st.detail_plot = image!(ax, extents(box, need, img)..., permutedims(look(img)))
                translate!(st.detail_plot, 0, 0, -1)          # keep outlines on top
            end
        catch e
            @error "Detail render failed" exception = (e, catch_backtrace())
        finally
            detail_busy[] = false
        end
    end
    on(ax.finallimits) do _
        st.view === :pano || return
        st.view_version += 1
        v = st.view_version
        @async (sleep(0.35); v == st.view_version && render_detail!())
    end

    function zoom!(k)
        fl = ax.finallimits[]
        c = fl.origin .+ fl.widths ./ 2
        w = fl.widths ./ k
        limits!(ax, c[1] - w[1] / 2, c[1] + w[1] / 2, c[2] - w[2] / 2, c[2] + w[2] / 2)
    end
    function pan!(dx, dy)
        fl = ax.finallimits[]
        o = fl.origin .+ (dx, dy) .* fl.widths
        limits!(ax, o[1], o[1] + fl.widths[1], o[2], o[2] + fl.widths[2])
    end

    # ---------------- callbacks: images ----------------

    on(b_add.clicks) do _
        @async begin
            ps = bg(pick_files)
            ps === nothing ? (status[] = NO_DIALOG) : add_paths!(ps)
        end
    end
    on(pathbox.stored_string) do s
        (s === nothing || isempty(strip(s))) && return
        endswith(lowercase(strip(s)), PROJECT_EXT) ? open_project!(expanduser(strip(s))) :
                                                     add_paths!(expand_paths(s))
    end
    on(_ -> remove!(sort(collect(isempty(st.selected) ? Set([st.sel]) : st.selected))), b_rm.clicks)
    on(_ -> remove!(collect(eachindex(st.items))), b_clear.clicks)
    on(_ -> move!(-1), b_up.clicks)
    on(_ -> move!(+1), b_down.clicks)

    # List: click / Ctrl-click / Shift-click to select, wheel to scroll.
    on(events(fig).mousebutton, priority = 5) do ev
        (ev.button == Mouse.left && ev.action == Mouse.press && is_mouseinside(listax)) ||
            return Consume(false)
        k = floor(Int, -mouseposition(listax)[2]) + 1
        1 <= k <= length(st.items) || return Consume(true)
        if ispressed(fig, Keyboard.left_shift | Keyboard.right_shift) && st.sel > 0
            union!(st.selected, min(st.sel, k):max(st.sel, k))
        elseif ispressed(fig, Keyboard.left_control | Keyboard.right_control)
            k in st.selected ? delete!(st.selected, k) : push!(st.selected, k)
        else
            st.selected = Set([k])
        end
        st.sel = k
        refresh_lists!()
        if st.view === :pano
            show_pano!()
        else
            st.view = :image; show_image!()
        end
        return Consume(true)
    end
    on(events(fig).scroll, priority = 5) do (_, dy)
        is_mouseinside(listax) || return Consume(false)
        st.list_top -= round(Int, 2 * sign(dy))
        draw_list!()
        return Consume(true)
    end

    function apply_preset!(name)
        P = PRESETS[name]
        pick!(m, v) = (m.i_selected[] = findfirst(==(v), m.options[]))
        foreach(((sl, v),) -> set_close_to!(sl, v),
                zip(sg_feat.sliders, (P.det_px, P.nkp, P.thr, P.denoise)))
        foreach(((sl, v),) -> set_close_to!(sl, v),
                zip((s_ratio, s_maxd, s_tol, s_mininl), (P.ratio, P.maxd, P.tol, P.mininl)))
        pick!(m_mode, P.mode); pick!(m_model, P.model); pick!(m_proj, P.proj); pick!(m_blend, P.blend)
        t_refine.active[] = P.refine
        set_close_to!(s_ev, P.ev)
        status[] = "Preset “$(name)”: $(P.note) Next: Run all steps."
    end
    on(m_preset.selection) do name
        name in keys(PRESETS) && apply_preset!(name)
    end

    on(m_ref.i_selected) do i
        (syncing[] || i === nothing || isempty(st.items)) && return
        new = i == 1 ? nothing : st.items[i-1]
        new === st.ref && return
        st.ref = new
        task!("Re-placing around the new reference…") do
            relayout!(); refresh_lists!()
            st.pano === nothing ? show_current!() : (render_preview!(); st.view = :pano; show_pano!(fit = true))
            status[] = "Reference: image $(refidx(st))."
        end
    end
    on(m_pair.i_selected) do i
        (syncing[] || i === nothing || isempty(st.pairkeys)) && return
        st.pairsel = st.pairkeys[i]
        st.view = :pair
        show_pair!()
    end
    on(_ -> st.view === :pair && show_pair!(fit = false), t_out.active)

    # ---------------- callbacks: steps ----------------

    on(b_detect.clicks) do _
        task!("Detecting features…") do
            isempty(st.items) && error("Add images first.")
            detect_all!()
            st.view = :image
            show_image!()
            status[] = "Features detected. Look for keypoints spread over the overlap areas; " *
                       "if too few, lower the corner threshold or raise Denoise. Then match."
        end
    end

    on(b_match.clicks) do _
        task!("Matching…") do
            length(st.items) < 2 && error("Add at least two images.")
            ij, listed = candidate_pairs()
            msg = match!(ij; listed)
            refresh_lists!()
            st.view = :pair
            show_pair!()
            status[] = msg * " Inspect pairs in the Pair menu, then preview."
        end
    end
    on(b_match1.clicks) do _
        task!("Re-matching pair…") do
            key = st.pairsel
            key === nothing && error("Choose a pair in the Pair menu first.")
            idx = indexof(st)
            msg = match!([(idx[key[1]], idx[key[2]])])
            refresh_lists!()
            st.view = :pair
            show_pair!()
            status[] = msg
        end
    end
    on(b_discard.clicks) do _
        task!("Discarding pair…") do
            key = st.pairsel
            key === nothing && error("Choose a pair in the Pair menu first.")
            pr = st.pairs[key]
            st.pairs[key] = PairResult(pr.p1, pr.p2, pr.H, pr.inl, false, true,
                                       "Discarded by you. Use Re-match pair to try it again.")
            note = relayout!()
            refresh_lists!()
            show_pair!()
            st.pano !== nothing && render_preview!()
            status[] = "Pair discarded; placement updated." * note
        end
    end

    on(b_all.clicks) do _
        task!("Running all steps…") do
            length(st.items) < 2 && error("Add at least two images.")
            ij, listed = candidate_pairs()
            msg = match!(ij; listed)
            refresh_lists!()
            render_preview!()
            st.view = :pano
            show_pano!(fit = true)
            status[] = "Done. " * msg
        end
    end

    on(_ -> request_preview(fit = st.view !== :pano), b_preview.clicks)
    # Live update of the preview when the projection / view settings change.
    live(_) = (!st.restoring && st.pano !== nothing && request_preview(fit = false); nothing)
    on(_ -> (!st.restoring && st.pano !== nothing && request_preview(fit = true)), m_proj.selection)
    on(live, m_blend.selection)
    on(live, s_focal.value); on(live, s_yaw.value); on(live, s_pitch.value); on(live, s_roll.value)
    on(s_ev.value) do _
        st.restoring && return
        st.view === :pano ? live(nothing) : show_current!()
    end
    # Stretch: only the display changes, so no re-rendering is needed.
    on(_ -> restretch(), s_black.value); on(_ -> restretch(), s_mid.value); on(_ -> restretch(), s_white.value)
    on(b_autostr.clicks) do _
        src = hist_source()
        src === nothing && (status[] = "Load images first."; return)
        auto_stretch!(src)
        status[] = "Auto stretch: black $(round(s_black.value[], digits = 4)), midtones " *
                   "$(round(s_mid.value[], digits = 4))."
    end
    on(b_resetstr.clicks) do _
        set_close_to!(s_black, 0.0); set_close_to!(s_mid, 0.5); set_close_to!(s_white, 1.0)
    end
    on(_ -> (!st.restoring && st.pano !== nothing && request_preview(fit = true)), t_unl.active)
    on(_ -> (st.view === :pano && (show_pano!(); render_detail!())), t_ovl.active)
    # Auto exposure: middle grey for the typical brightness, without clipping
    # more than the brightest 0.5 % (stars, lamps).
    on(b_autoev.clicks) do _
        st.pano === nothing && (status[] = "Preview the panorama first."; return)
        lum = [St.luminance(c) for c in st.pano.img if St.luminance(c) > 0]
        isempty(lum) && return
        med, hi = quantile(lum, (0.5, 0.995))
        ev = s_ev.value[] + clamp(min(log2(0.18 / med), log2(1.0 / hi)), -6, 10)
        set_close_to!(s_ev, clamp(ev, -3, 8))
        status[] = "Exposure set to $(round(s_ev.value[], digits = 1)) EV."
    end
    on(b_resetview.clicks) do _
        set_close_to!(s_focal, 1.0); set_close_to!(s_yaw, 0.0); set_close_to!(s_pitch, 0.0)
        set_close_to!(s_roll, 0.0)
    end
    on(t_refine.active) do _
        (st.restoring || isempty(st.items)) && return
        task!("Re-placing…") do
            note = relayout!(); refresh_lists!()
            st.pano !== nothing && (render_preview!(); show_pano!())
            status[] = "Global refinement " * (t_refine.active[] ? "on." : "off.") * note
        end
    end

    # Brightness applies to all selected images.
    on(s_gain.value) do g
        (syncing[] || isempty(st.items)) && return
        targets = isempty(st.selected) ? [st.sel] : collect(st.selected)
        all(i -> st.items[i].gain ≈ g, targets) && return
        foreach(i -> st.items[i].gain = g, targets)
        live(nothing)
    end
    on(b_autogain.clicks) do _
        task!("Estimating brightness…") do
            L = st.layout
            (L === nothing || !any(pr.ok for pr in values(st.pairs))) && error("Match the images first.")
            pts = Dict((L.parent[v], v) => pair_points(L.parent[v], v) for v in L.order if L.parent[v] != 0)
            g = St.estimate_gains([it.img for it in st.items], L.parent, L.order, pts)
            foreach((it, gi) -> it.gain = gi, st.items, g)
            refresh_lists!()
            render_preview!()
            st.view = :pano
            show_pano!()
            status[] = "Brightness: " * join(["$i: $(round(gi, digits = 2))" for (i, gi) in enumerate(g)], ", ")
        end
    end

    function nudge!(dx, dy)
        (isempty(st.items) || st.layout === nothing) && return
        if st.sel == refidx(st)
            status[] = "The reference image stays fixed; select another image to nudge " *
                       "(or use Pitch / Roll to move the whole panorama)."
            return
        end
        p = st.pano === nothing ? 1.0 : st.pano.scale
        st.items[st.sel].offset .+= (dx / p, dy / p)      # one step = one preview pixel
        request_preview()
    end
    on(_ -> nudge!(-1, 0), b_nl.clicks); on(_ -> nudge!(1, 0), b_nr.clicks)
    on(_ -> nudge!(0, -1), b_nu.clicks); on(_ -> nudge!(0, 1), b_nd.clicks)
    on(b_resetadj.clicks) do _
        foreach(it -> fill!(it.offset, 0), st.items)
        live(nothing)
    end

    on(events(fig).keyboardbutton) do ev
        (!active() || ev.action == Keyboard.release || pathbox.focused[] || any(tb -> tb.focused[], numboxes)) &&
            return Consume(false)
        k = ev.key
        ctrl = ispressed(fig, Keyboard.left_control | Keyboard.right_control)
        shift = ispressed(fig, Keyboard.left_shift | Keyboard.right_shift)
        if ctrl && k == Keyboard.z
            shift ? redo!() : undo!()
        elseif ctrl && k == Keyboard.y
            redo!()
        elseif k in (Keyboard.delete, Keyboard.backspace) && is_mouseinside(listax)
            remove!(sort(collect(isempty(st.selected) ? Set([st.sel]) : st.selected)))
        elseif k == Keyboard.a && ctrl && is_mouseinside(listax)
            st.selected = Set(eachindex(st.items)); draw_list!()
        elseif k in (Keyboard.equal, Keyboard.kp_add)
            zoom!(1.5)
        elseif k in (Keyboard.minus, Keyboard.kp_subtract)
            zoom!(1 / 1.5)
        elseif k in (Keyboard._0, Keyboard.kp_0, Keyboard.home)
            autolimits!(ax)
        elseif k in (Keyboard.left, Keyboard.right, Keyboard.up, Keyboard.down)
            d = k == Keyboard.left ? (-1, 0) : k == Keyboard.right ? (1, 0) :
                k == Keyboard.up ? (0, -1) : (0, 1)
            if st.view === :pano
                m = ispressed(fig, Keyboard.left_shift | Keyboard.right_shift) ? 10 : 1
                nudge!((m .* d)...)
            else
                pan!((0.1 .* d)...)
            end
        else
            return Consume(false)
        end
        return Consume(true)
    end

    """Image whose outline contains (x, y) in the panorama view (nearest centre wins); 0 if none."""
    function pick(x, y)
        best, bestd = 0, Inf
        for (i, o) in enumerate(st.pano.outlines)
            fin = filter(p -> all(isfinite, p), o)
            (isempty(fin) || !inside(fin, x, y)) && continue
            d = hypot(x - mean(first, fin), y - mean(last, fin))
            d < bestd && ((best, bestd) = (i, d))
        end
        return best
    end
    function select_image!(k)
        st.sel = k; st.selected = Set([k])
        scroll_to_sel!()
        refresh_lists!()
    end

    # Ctrl + drag in the panorama view moves an image (with the images attached
    # beyond it) by hand, e.g. frames that could not be matched. Only its outline
    # follows the mouse; the panorama is re-rendered on release.
    dragging = Ref{Any}(nothing)
    on(events(fig).mousebutton, priority = 10) do ev
        ev.button == Mouse.left || return Consume(false)
        if ev.action == Mouse.press && st.view === :pano && st.pano !== nothing && is_mouseinside(ax) &&
           ispressed(fig, Keyboard.left_control | Keyboard.right_control)
            x, y = mouseposition(ax)
            k = pick(x, y)
            k == 0 && return Consume(true)
            select_image!(k)
            if k == refidx(st)
                status[] = "The reference image stays fixed; drag another image (or use Pitch / Roll)."
                show_pano!()
                return Consume(true)
            end
            show_pano!()
            o = Observable(Point2f.([st.pano.outlines[k]; st.pano.outlines[k][1:1]]))
            ax.panbutton = Mouse.middle                  # no panning while dragging
            dragging[] = (k = k, start = Point2f(x, y), off0 = copy(st.items[k].offset), pts = o[],
                          obs = o, plot = lines!(ax, o; color = :orange, linewidth = 3, linestyle = :dash))
            return Consume(true)
        elseif ev.action == Mouse.release && dragging[] !== nothing
            D = dragging[]; dragging[] = nothing
            ax.panbutton = Mouse.left
            x, y = mouseposition(ax)
            st.items[D.k].offset .= D.off0 .+ (x - D.start[1], y - D.start[2])
            request_preview()
            return Consume(true)
        end
        return Consume(false)
    end
    on(events(fig).mouseposition, priority = 10) do _
        D = dragging[]
        D === nothing && return Consume(false)
        d = Point2f(mouseposition(ax)) - D.start
        D.obs[] = D.pts .+ Ref(d)
        return Consume(true)
    end

    # Click (without dragging) on the panorama selects the image under the cursor.
    press = Ref(Point2f(0, 0))
    on(events(fig).mousebutton) do ev
        ev.button == Mouse.left || return Consume(false)
        if ev.action == Mouse.press
            press[] = events(fig).mouseposition[]
            return Consume(false)
        end
        (st.view === :pano && st.pano !== nothing && is_mouseinside(ax)) || return Consume(false)
        norm(events(fig).mouseposition[] - press[]) < 4 || return Consume(false)   # was a drag
        k = pick(mouseposition(ax)...)
        if k > 0 && k != st.sel
            select_image!(k)
            show_pano!()
            render_detail!()
        end
        return Consume(false)
    end

    on(_ -> (st.view = :image; show_image!()), b_vimg.clicks)
    on(_ -> (st.view = :pair; show_pair!()), b_vpair.clicks)
    on(_ -> (st.view = :pano; show_pano!(fit = true)), b_vpano.clicks)
    on(_ -> zoom!(1.5), b_zin.clicks)
    on(_ -> zoom!(1 / 1.5), b_zout.clicks)
    on(_ -> autolimits!(ax), b_fit.clicks)

    # ---------------- auto crop ----------------

    on(b_crop.clicks) do _
        st.pano === nothing && (status[] = "Preview the panorama first."; return)
        task!("Finding the largest fully covered rectangle…") do
            st.crop = nothing
            render_preview!()                            # uncropped, with coverage
            P = st.pano
            r = St.largest_rectangle(P.covered)
            r === nothing && error("Nothing to crop.")
            rows, cols = r
            # Output coordinates, one preview pixel inside on every side.
            pu(c) = (c - 0.5) / P.scale + P.box[1] - 0.5
            pv(r) = (r - 0.5) / P.scale + P.box[3] - 0.5
            box = (ceil(pu(first(cols) + 1)), floor(pu(last(cols) - 1)),
                   ceil(pv(first(rows) + 1)), floor(pv(last(rows) - 1)))
            (box[2] > box[1] && box[4] > box[3]) || error("The covered area is too small to crop.")
            full = (P.box[2] - P.box[1] + 1) * (P.box[4] - P.box[3] + 1)
            area = (box[2] - box[1] + 1) * (box[4] - box[3] + 1)
            st.crop = (box = box, key = P.key)
            render_preview!()
            st.view = :pano
            show_pano!(fit = true)
            status[] = "Cropped to $(Int(box[2]-box[1]+1))×$(Int(box[4]-box[3]+1)) px " *
                       "($(round(Int, 100area / full)) % of the panorama). The crop is used when " *
                       "saving; it is removed when you change the geometry."
        end
    end
    on(b_uncrop.clicks) do _
        st.crop === nothing && return
        st.crop = nothing
        request_preview(fit = true)
    end

    # ---------------- undo / redo ----------------
    #
    # The program remembers the state after every change that has settled (a
    # slider drag counts once). Images are never modified, so a state is small:
    # the image list, matching results, manual moves, brightness and settings.

    undo_sliders = [sg_feat.sliders; sg_match.sliders; sg_view.sliders; s_prev; s_ev; sg_str.sliders]
    undo_names = [sg_feat.labels; sg_match.labels; sg_view.labels; "Preview size"; "Exposure";
                  sg_str.labels]
    undo_menus = [m_mode, m_model, m_proj, m_blend]
    menu_names = ["Pairs", "Model", "Projection", "Blend"]
    undo_toggles = [t_snake, t_refine, t_unl]
    toggle_names = ["snake", "refine", "show unlinked images"]

    snapshot() = (items = copy(st.items), offsets = [copy(it.offset) for it in st.items],
                  gains = [it.gain for it in st.items], pairs = copy(st.pairs), ref = st.ref,
                  layout = st.layout, crop = st.crop,
                  sliders = [sl.value[] for sl in undo_sliders],
                  menus = [m.i_selected[] for m in undo_menus],
                  toggles = [t.active[] for t in undo_toggles])

    same(a, b) = a.items == b.items && a.offsets == b.offsets && a.gains == b.gains &&
                 a.ref === b.ref && a.layout === b.layout && a.crop == b.crop &&
                 a.sliders == b.sliders && a.menus == b.menus && a.toggles == b.toggles &&
                 length(a.pairs) == length(b.pairs) &&
                 all(k -> haskey(b.pairs, k) && b.pairs[k] === a.pairs[k], keys(a.pairs))

    """What changed from state a to state b: (which control, description)."""
    function change(a, b)
        if a.items != b.items
            return "items", length(b.items) > length(a.items) ? "add images" :
                            length(b.items) < length(a.items) ? "remove images" : "reorder images"
        end
        a.offsets != b.offsets && return "offsets", "move images"
        a.gains != b.gains && return "gains", "brightness"
        for (n, x, y) in zip(undo_names, a.sliders, b.sliders)
            x != y && return "slider $n", "$n $(fmt(x)) → $(fmt(y))"
        end
        for (n, m, x, y) in zip(menu_names, undo_menus, a.menus, b.menus)
            x != y && return "menu $n", "$n → $(m.options[][y])"
        end
        for (n, x, y) in zip(toggle_names, a.toggles, b.toggles)
            x != y && return "toggle $n", "$n $(y ? "on" : "off")"
        end
        a.crop != b.crop && return "crop", b.crop === nothing ? "uncrop" : "auto crop"
        a.ref !== b.ref && return "ref", "reference image"
        return "pairs", "matching / pairs"
    end
    describe(a, b) = change(a, b)[2]

    """Record the state when it has changed and then stayed the same for a moment."""
    function commit!(s)
        push!(st.undo, (state = st.committed, label = describe(st.committed, s)))
        length(st.undo) > UNDO_STEPS && popfirst!(st.undo)
        empty!(st.redo)
        st.committed = s; st.pending = nothing
    end
    function check_changes!()
        (st.busy || st.restoring || st.committed === nothing) && return
        s = snapshot()
        if same(s, st.committed)
            st.pending = nothing
        elseif st.pending === nothing
            st.pending = s
        elseif same(s, st.pending)
            commit!(s)                                   # the change has settled
        else
            # Still changing. A change of a different control is a step of its own.
            change(st.committed, st.pending)[1] == change(st.pending, s)[1] || commit!(st.pending)
            st.pending = s
        end
    end

    function watch_changes!()
        opened = Ref(false)
        on(o -> (o && (opened[] = true)), events(fig).window_open)
        @async while !st.closed && !(opened[] && !events(fig).window_open[])
            sleep(0.4)
            try
                check_changes!()
            catch e
                @error "Undo bookkeeping failed" exception = (e, catch_backtrace())
            end
        end
    end

    function restore!(s)
        st.restoring = true; syncing[] = true
        try
            st.items = copy(s.items)
            for (it, o, g) in zip(st.items, s.offsets, s.gains)
                it.offset = copy(o); it.gain = g
            end
            st.pairs = copy(s.pairs); st.ref = s.ref; st.layout = s.layout; st.crop = s.crop
            foreach(((sl, v),) -> set_close_to!(sl, v), zip(undo_sliders, s.sliders))
            foreach(((m, i),) -> (m.i_selected[] = i), zip(undo_menus, s.menus))
            foreach(((t, v),) -> (t.active[] = v), zip(undo_toggles, s.toggles))
        finally
            st.restoring = false; syncing[] = false
        end
        n = length(st.items)
        st.selected = Set(i for i in st.selected if i <= n)
        st.sel = clamp(st.sel, min(n, 1), n)
        st.layout === nothing && n > 0 && relayout!()
        refresh_lists!()
        st.committed = snapshot(); st.pending = nothing
        if n > 0 && any(pr.ok for pr in values(st.pairs))
            render_preview!()                            # the view stays as it was
        else
            st.pano = nothing
            st.view === :pano && (st.view = :image)
        end
        show_current!()
    end

    function step!(from, to, verb)
        (st.busy || isempty(from)) && return
        check_changes!()                                 # a change still settling counts first
        st.pending !== nothing && commit!(st.pending)
        isempty(from) && return
        e = pop!(from)
        push!(to, (state = st.committed, label = e.label))
        task!("$verb: $(e.label)…") do
            restore!(e.state)
            status[] = "$verb: $(e.label). ($(length(st.undo)) undo / $(length(st.redo)) redo steps left)"
        end
    end
    undo!() = step!(st.undo, st.redo, "Undone")
    redo!() = step!(st.redo, st.undo, "Redone")
    on(_ -> undo!(), b_undo.clicks)
    on(_ -> redo!(), b_redo.clicks)

    # ---------------- projects ----------------

    toggle_by_name = Dict("snake" => t_snake, "refine" => t_refine, "show unlinked images" => t_unl,
                          "overlays" => t_ovl)
    menu_by_name = Dict("pairs" => m_mode, "model" => m_model, "projection" => m_proj, "blend" => m_blend)

    function save_project!(path)
        endswith(lowercase(path), PROJECT_EXT) || (path *= PROJECT_EXT)
        dir = dirname(abspath(path))
        idx = indexof(st)
        d = Dict{String,Any}(
            "format" => "PanoStitch project", "version" => 1,
            "images" => [Dict("path" => abspath(it.path), "relative" => relpath(abspath(it.path), dir),
                              "offset" => it.offset, "gain" => it.gain) for it in st.items],
            "loading" => Dict("raw_decoder" => m_raw.selection[], "half_size" => t_half.active[]),
            "sliders" => Dict(n => sl.value[] for (n, sl) in zip(undo_names, undo_sliders)),
            "menus" => Dict(n => m.selection[] for (n, m) in menu_by_name),
            "toggles" => Dict(n => t.active[] for (n, t) in toggle_by_name),
            "reference" => st.ref === nothing ? 0 : idx[st.ref],
            "pairs" => [Dict("a" => idx[a], "b" => idx[b], "H" => [pr.H[r, :] for r in 1:3],
                             "inliers" => pr.inl, "x1" => pr.p1[1, :], "y1" => pr.p1[2, :],
                             "x2" => pr.p2[1, :], "y2" => pr.p2[2, :], "accepted" => pr.ok,
                             "listed" => pr.listed, "message" => pr.msg)
                        for ((a, b), pr) in st.pairs if haskey(idx, a) && haskey(idx, b)])
        st.crop === nothing || (d["crop"] = collect(st.crop.box))
        open(io -> TOML.print(io, d), path, "w")
        st.project = path
        return path
    end

    function open_project!(path)
        task!("Opening project $(basename(path))…") do
            isfile(path) || error("Project file not found: $path")
            d = TOML.parsefile(path)
            get(d, "format", "") == "PanoStitch project" || error("$(basename(path)) is not a PanoStitch project.")
            dir = dirname(abspath(path))
            L = get(d, "loading", Dict())
            dec = get(L, "raw_decoder", "auto")
            dec in m_raw.options[] || (dec = "auto")
            m_raw.i_selected[] = findfirst(==(dec), m_raw.options[])
            t_half.active[] = get(L, "half_size", true)
            # Find the images: stored absolute path, else relative to the project file.
            recs = d["images"]
            paths = map(recs) do r
                isfile(r["path"]) ? r["path"] : isfile(joinpath(dir, r["relative"])) ?
                    normpath(joinpath(dir, r["relative"])) : nothing
            end
            missing_ = [basename(r["path"]) for (r, p) in zip(recs, paths) if p === nothing]
            want = [k for k in eachindex(paths) if paths[k] !== nothing]
            items, failed = load_items(paths[want], Symbol(dec), t_half.active[])
            newidx = Dict{Int,Item}()
            for (k, it) in zip(want, items)
                it === nothing && continue
                it.offset = Float64.(recs[k]["offset"]); it.gain = Float64(recs[k]["gain"])
                newidx[k] = it
            end
            st.restoring = true; syncing[] = true
            try
                st.items = [newidx[k] for k in sort(collect(keys(newidx)))]
                for (n, sl) in zip(undo_names, undo_sliders)
                    haskey(get(d, "sliders", Dict()), n) && set_close_to!(sl, d["sliders"][n])
                end
                for (n, m) in menu_by_name
                    v = get(get(d, "menus", Dict()), n, nothing)
                    v !== nothing && v in m.options[] && (m.i_selected[] = findfirst(==(v), m.options[]))
                end
                for (n, t) in toggle_by_name
                    v = get(get(d, "toggles", Dict()), n, nothing)
                    v === nothing || (t.active[] = v)
                end
                st.pairs = Dict{Tuple{Item,Item},PairResult}()
                for r in get(d, "pairs", [])
                    (haskey(newidx, r["a"]) && haskey(newidx, r["b"])) || continue
                    H = Matrix{Float64}(permutedims(reduce(hcat, r["H"])))
                    p1 = Matrix{Float64}(permutedims(hcat(r["x1"], r["y1"])))
                    p2 = Matrix{Float64}(permutedims(hcat(r["x2"], r["y2"])))
                    st.pairs[(newidx[r["a"]], newidx[r["b"]])] =
                        PairResult(p1, p2, H, Int.(r["inliers"]), r["accepted"], r["listed"], r["message"])
                end
                ref = get(d, "reference", 0)
                st.ref = ref > 0 && haskey(newidx, ref) ? newidx[ref] : nothing
            finally
                st.restoring = false; syncing[] = false
            end
            st.sel = min(1, length(st.items)); st.selected = Set(st.sel > 0 ? [st.sel] : Int[])
            st.list_top = 0
            st.pano = nothing
            haskey(d, "crop") && (st.crop = (box = Tuple(Float64.(d["crop"])), key = :pending))
            relayout!()
            refresh_lists!()
            if any(pr.ok for pr in values(st.pairs))
                render_preview!(); st.view = :pano; show_pano!(fit = true)
            else
                st.view = :image; show_current!(fit = true)
            end
            empty!(st.undo); empty!(st.redo); st.committed = snapshot(); st.pending = nothing
            st.project = path
            status[] = "Opened $(basename(path)): $(length(st.items)) images, " *
                       "$(count(pr -> pr.ok, values(st.pairs))) pairs." *
                       (isempty(missing_) ? "" : " Missing files: $(join(missing_, ", ")).") *
                       (isempty(failed) ? "" : " Could not load: $(join(failed, "; ")).")
        end
    end

    on(b_open.clicks) do _
        @async begin
            p = bg(pick_project)
            p === nothing ? (status[] = "No project chosen (without a file dialog, type the path of a " *
                                        "$PROJECT_EXT file into the path box).") : open_project!(p)
        end
    end
    on(b_saveproj.clicks) do _
        isempty(st.items) && (status[] = "Nothing to save yet."; return)
        @async begin
            default = st.project !== nothing ? st.project :
                      joinpath(dirname(abspath(st.items[1].path)), "project$PROJECT_EXT")
            out = bg(() -> pick_save(default; title = "Save project"))
            out === nothing && return
            try
                status[] = "Saved project $(save_project!(out))."
            catch e
                status[] = "Error: " * sprint(showerror, e)
            end
        end
    end

    on(b_save.clicks) do _
        (isempty(st.items) || st.layout === nothing) && (status[] = "Nothing to save."; return)
        @async begin
            # Astronomy inputs: save in their format (linear); otherwise TIFF.
            ext = all(it -> St.isfits(it.path), st.items) ? "fits" :
                  all(it -> St.isastro(it.path), st.items) ? "xisf" : "tif"
            default = joinpath(dirname(abspath(st.items[1].path)), "panorama.$ext")
            out = bg(() -> pick_save(default))
            out === nothing && return
            task!("Rendering full-resolution panorama…") do
                f, c, proj = focal(), refc(), projection()
                vis = visible()
                Gall = placements(f, c)
                G = Gall[vis]
                V = st.items[vis]
                lens = st.layout.lens
                box = St.bounds([St.outline(Gi, size(it.img)..., proj, f, c; lens) for (Gi, it) in zip(G, V)])
                cropped = st.crop !== nothing && st.crop.key == hash((proj, f, vis, Gall))
                cropped && (box = st.crop.box)
                W, H = Int(box[2] - box[1] + 1), Int(box[4] - box[3] + 1)
                max(W, H) <= 40_000 || error("The panorama would be $(W)×$(H) px; check the pairs " *
                                             "or use another projection.")
                imgs = [it.img for it in V]
                gains = [it.gain * 2.0^s_ev.value[] for it in V]
                bl = blend()
                # FITS / XISF keep the linear data for further processing; other
                # formats are saved as shown (with the stretch).
                fits = St.isastro(out)
                bg(() -> St.save_panorama(out, (img = St.compose(imgs, G, box; projection = proj, f, c,
                                                                 blend = bl, gains, lens);
                                                fits ? img : look(img))))
                status[] = "Saved $out ($(W)×$(H) px, $(length(V)) images" * (cropped ? ", cropped" : "") *
                           (fits ? ", linear 32-bit $(uppercase(splitext(out)[2][2:end])))." : stretched() ? ", stretched as shown)." : ").")
            end
        end
    end

    refresh_lists!()
    show_current!()
    st.committed = snapshot()
    projs = filter(p -> endswith(lowercase(p), PROJECT_EXT), paths)
    isempty(projs) ? add_paths!(paths) : open_project!(first(projs))
    st.api = (; save_project = save_project!, open_project = open_project!, undo = undo!, redo = redo!, add = add_paths!,
              close = () -> (st.closed = true))
    UI[].simple && remove_blocks!(adv)
    watch && watch_changes!()
    return st
end

# Makie draws text from a texture of rendered glyphs (an "atlas"). Its atlas
# is built into Makie's precompiled code with only the glyphs of Makie's own
# fonts, so the glyphs of our fonts were rendered anew on every start, which
# took about two seconds. They are kept in a file of ours (per Makie version)
# and loaded before the first window is drawn. Uses Makie internals; if those
# change, the loading fails quietly and the start is only slower.
"""Folder for files that can be recreated (the glyph cache)."""
function cache_dir()
    Sys.iswindows() && return joinpath(get(ENV, "LOCALAPPDATA", joinpath(homedir(), "AppData", "Local")), "PanoStitch")
    Sys.isapple() && return joinpath(homedir(), "Library", "Caches", "PanoStitch")
    return joinpath(get(ENV, "XDG_CACHE_HOME", joinpath(homedir(), ".cache")), "panostitch")
end
const ATLAS_KEY = (2048, 64)                     # the atlas GLMakie draws with
glyph_file() = joinpath(cache_dir(), "glyphs_makie-$(pkgversion(GLMakie.Makie))_" *
                                     "$(GLMakie.Makie.SERIALIZATION_FORMAT_VERSION).bin")
const GLYPHS_SAVED = Ref(typemax(Int))          # glyphs in our file; more in memory → save

function load_glyph_cache()
    M = GLMakie.Makie
    try
        f = glyph_file()
        if isfile(f)
            atlas = M.load_texture_atlas(f)
            length(atlas.mapping) > length(M.get_texture_atlas(ATLAS_KEY...).mapping) &&
                (M.TEXTURE_ATLASES[ATLAS_KEY] = atlas)
        end
        GLYPHS_SAVED[] = length(M.get_texture_atlas(ATLAS_KEY...).mapping)
    catch e
        @debug "Could not load the glyph cache" exception = e
    end
end

function save_glyph_cache()
    M = GLMakie.Makie
    try
        atlas = M.get_texture_atlas(ATLAS_KEY...)
        n = length(atlas.mapping)
        n > GLYPHS_SAVED[] || return
        f = glyph_file()
        mkpath(dirname(f))
        tmp = "$(f).$(getpid()).tmp"             # renamed when complete: never half a file
        M.store_texture_atlas(tmp, atlas)
        mv(tmp, f; force = true)
        GLYPHS_SAVED[] = n
    catch e
        @debug "Could not save the glyph cache" exception = e
    end
end

"""Open the stitcher window. Returns the GLMakie screen (wait(screen) blocks until closed)."""
function run_gui(paths = String[])
    GLMakie.activate!(title = "PanoStitch")
    Threads.nthreads() == 1 && @warn "Running on one thread, so detection and rendering are slower. " *
        "Run `julia bin/panostitch.jl` in a terminal, or in VS Code set \"julia.NumThreads\": \"auto\" " *
        "(Settings → Julia: Num Threads) and restart the REPL."
    load_glyph_cache()                           # before anything is drawn
    screen = GLMakie.Screen()
    function show!(paths, ui, restore)
        local st
        fig, st = build_gui(paths; ui, restore, on_ui = (new, carry) -> @async begin
            st.api.close()                       # stop the old window's background loop
            show!(String[], new, carry)
        end)
        display(screen, fig)
        save_glyph_cache()
    end
    show!(paths, load_ui(), nothing)
    atexit(save_glyph_cache)                     # glyphs that appeared while working
    return screen
end

end # module
