# Star stacking page, modelled on Sequator but with what it lacks (bias and
# flat-dark frames, a foreground image, trail fades and gap filling, automatic
# sky detection, frame scoring, a quality chart and a log of every step): the
# input files (star images, base image, calibration frames, foreground, output) on the top left; the processing settings
# below; the options of the highlighted entry under them; Quick preview and
# Start. On the right: the base image (where the sky region is drawn), the
# result, and the frame report. The work is done by Stacker (stacker.jl).
#
# Base image view: in "Boundary line" / "Gradient" mode drag a line along the
# horizon; in "Irregular mask" mode paint the sky with the left button, erase
# with the right one, mouse wheel = brush size ("Detect sky" paints a first
# mask). Frame report: click a frame (in the table or the chart) to exclude it
# (or include it again). Result: drag to pan, wheel to zoom.
#
# Projects (.stackproj, TOML text) keep the file lists, the sky region and
# every setting; files are found by absolute path, else relative to the project.

const STACK_GROUPS = ("Star images", "Base image", "Noise images", "Bias images", "Vignetting images",
                      "Flat-dark images", "Foreground image", "Output")
const FG = 7                      # the foreground image (one file, like the base image)
const OUT = 8                     # the Output entry; groups 1–7 hold files
const GROUP_KEYS = ("stars", "base", "darks", "bias", "flats", "flatdarks", "foreground")   # in project files
const STACK_PROJECT_EXT = ".stackproj"
const TREE_ROWS = 9               # visible rows of the input list
const ROW_PX = 18                 # height of one row in the lists
const MASK_COLS = 400             # resolution of the painted sky mask
const PREVIEW_MAX = 1600          # longest side of the base image view
const RESULT_MAX = 3000           # longest side of the result as shown

const COMPOSITIONS = ("Align stars", "Star trails")
const ALIGN_MODES = ("Accumulation", "Select best pixels", "Freeze ground", "Align only")
const MODELS_ = ("homography", "similarity")
const SKY_MODES = ("None", "Boundary line", "Gradient", "Irregular mask")
const DISTORTION = ("Off", "Auto", "Lens", "Complex")
const POLLUTION = ("Off", "Deep sky", "Uneven")
const COLOR_SPACES = ("sRGB", "Adobe RGB")
const TONES = ("sRGB curve", "Stretch", "Linear")
const INTERPOLATIONS = ("Lanczos", "Bilinear")
const WHITE_BALANCE = ("Camera" => :camera, "Average of image" => :auto, "Daylight" => :daylight)
const FADES = Sk.FADES
const CHART_SERIES = (("stars", :stars, :dodgerblue), ("FWHM", :fwhm, :orange), ("elongation", :elong, :mediumpurple),
                      ("light", :light, :gold), ("sky level", :background, :gray60), ("align error", :err, :tomato))
const DEMOSAIC = ("AHD" => 3, "DHT" => 11, "AAHD" => 12, "DCB" => 4, "PPG" => 2, "VNG" => 1, "Bilinear" => 0)


# Explanations shown under the options of each entry (after the Sequator manual).
const STACK_NOTES = Dict(
    "Star images" => "Star shots with the same focal length, aperture and exposure; they are " *
        "stacked onto the base image. RAW is best; TIFF, FITS or XISF only if the data is still " *
        "linear.\n\n" *
        "Unify exposure: evens out the brightness of frames taken under different conditions " *
        "(e.g. other nights), which otherwise leave rectangular borders.\n\nQuick preview stacks " *
        "up to 10 frames at low resolution and saves nothing.",
    "Base image" => "The frame the others are aligned and stacked onto. If none is set, the " *
        "middle star image (by file name) is used.\n\nHighlight a star image and press " *
        "“Use as base” to choose one.",
    "Noise images" => "Dark frames: lens cap on, same ISO and exposure time as the star images, " *
        "taken right before and after them (noise changes with temperature). Their median is " *
        "subtracted, and the hot pixels found in them are replaced.",
    "Bias images" => "Bias frames: shortest exposure with the lens cap on, same ISO: the sensor's " *
        "offset. Subtracted from the flats when there are no flat darks, and from the star " *
        "images when there are no noise images.",
    "Vignetting images" => "Flat frames: an even, white subject (paper or cloth over the lens " *
        "against a uniform light, a white wall), same focal length and aperture, properly " *
        "exposed. They correct the light fall-off towards the corners (and dust shadows).\n\n" *
        "Homogenize vignetting averages and blurs the four quadrants, for uneven flats. Not " *
        "suitable if there are dust shadows.",
    "Flat-dark images" => "Dark frames for the flats: same ISO and exposure time as the " *
        "vignetting images, lens cap on. Subtracted from the flats (else the bias is).",
    "Foreground image" => "A separate shot of the ground from the same tripod position – at blue " *
        "hour, a long exposure or low ISO – used below the sky region instead of the stacked " *
        "foreground (which is noisy or dark at night). Needs a sky region. Not aligned: the ground " *
        "stays in place.\n\nMatch brightness scales it to the stacked ground, so it fits the night; " *
        "Exposure (EV) brightens or darkens it from there (+1 EV = twice as bright).",
    "Output" => "TIFF (.tif): 16-bit, as shown. JPEG (.jpg): 8-bit. FITS (.fits) or XISF (.xisf): " *
        "32-bit float, linear: only the brightness gain is applied, no tone curve, nothing " *
        "clipped – the most room for processing in Siril, PixInsight etc.\nAlign only writes the " *
        "aligned frames into an “aligned” folder next to the output (FITS / XISF if the output " *
        "is, else TIFF).",
    "Composition" => "Align stars:\n" *
        "  Accumulation – average of the aligned frames.\n" *
        "  Select best pixels – sigma-clipped average (see Sigma clipping).\n" *
        "  Freeze ground – the sky is aligned, the foreground is not, so both stay sharp. " *
        "Needs a sky region and a fixed tripod.\n" *
        "  Align only – writes the aligned frames as linear images.\n" *
        "Selective: sigma clipping also in the sky (removes planes, meteors).\nModel: homography " *
        "follows the perspective of wide-angle shots; similarity only shifts, turns, scales.\n" *
        "Interpolation: Lanczos is sharper, bilinear a little smoother and faster.\n\n" *
        "Star trails: no alignment; the sky gets the brightest value, the foreground the " *
        "average.\n  Trail fade – fade in (Sequator's motion effect: the trails start faint), fade " *
        "out, or both ends (a comet-like taper).\n  Fill gaps – the short breaks between frames " *
        "(the camera's interval) are filled along the measured star motion, so the trails are " *
        "continuous instead of dashed.",
    "Sky region" => "Marks the sky: stars are detected there, Freeze ground, Star trails and the " *
        "foreground image treat the rest as foreground.\n  Boundary line – drag a line along the " *
        "horizon.\n  Gradient – the line plus a transition zone below it.\n  Irregular mask – paint " *
        "the sky (left button), erase (right); mouse wheel or the slider = brush size.\nDetect sky " *
        "paints a first mask from brightness and stars (approximate: check it and correct with the " *
        "brush). Mask feather softens the mask's edge, inside the sky only, so the foreground gets " *
        "none of the sky. Auxiliary highlight brightens the view; Show detected stars marks the " *
        "stars the last run found in the shown frame.",
    "Sigma clipping" => "Select best pixels, Freeze ground and the star-trails foreground drop " *
        "values further than low / high × σ from the centre, a few times over. Lower values " *
        "remove more (planes, satellites, hot pixels) but leave more noise; 2–3 is usual. " *
        "Needs 4 or more frames, works best with 10+.",
    "Frame rejection" => "Every star image is measured: stars found, star size (FWHM), " *
        "elongation, sky level, brightness of its brightest stars compared with the base image " *
        "(light), alignment error. Automatic rejection leaves out frames clearly worse than the " *
        "typical one: dimmer stars (clouds, haze, dew), few stars, blurred or elongated stars " *
        "(focus, wind, bumped tripod). The base image is never rejected.\nSee the Frame report " *
        "after Preview or Start: a chart of every measure per frame (÷ its median, so outliers " *
        "stand out; red = left out) above the table, and the log of every step. Click a frame in " *
        "either to exclude or include it.",
    "RAW conversion" => "RAW files are decoded by LibRaw (dcraw_emu) without tone curve, as " *
        "stacking needs. White balance: the camera's, the image average, or daylight. " *
        "Demosaicing: AHD (default); DHT and AAHD often give less colour noise at high ISO; DCB, " *
        "PPG, VNG, bilinear. Half size skips demosaicing: 4× fewer " *
        "pixels, much faster, less memory. The quick preview always uses half size.",
    "Auto brightness" => "Puts the sky background at a pleasant level without letting more than " *
        "the brightest 0.1 % of the sky clip. Off: the brightness of a single frame.",
    "Background extraction" => "Subtracts a smooth model of the sky background (gradients from " *
        "light pollution, moonlight, vignetting), as Siril's background extraction does; the sky " *
        "keeps its average level. Degree 1 = a plain gradient, higher = more curved.",
    "Color calibration" => "Basic colour calibration without plate solving: the sky background " *
        "becomes neutral grey and the average star white.",
    "Reduce colour noise" => "Smooths only the colour (not the brightness): removes the coloured " *
        "speckles of high-ISO shots. Stronger = smoother, but small stars lose some colour.",
    "Tone" => "How the linear stack becomes a viewable image: sRGB curve (as a camera does), " *
        "Stretch (black point, midtones, white point – automatic like Siril's autostretch, or by " *
        "hand), or Linear (no curve, for further processing). FITS / XISF are always saved linear.",
    "Linear" => "Linear (for further processing): everything here off, no brightness change, no " *
        "tone curve, output as 32-bit FITS – what Siril, PixInsight etc. expect. Use their " *
        "background extraction, colour calibration and stretch afterwards.",
    "High dynamic range" => "Compresses the highlights instead of clipping them (bright stars, " *
        "lights); with Auto brightness the dark parts are lifted more.",
    "Reduce dynamic noises" => "Finds hot pixels without noise images (single pixels much " *
        "brighter than all their neighbours) and replaces them. No detection is perfect.",
    "Reduce distor. effects" => "Lens distortion and refraction move the stars differently " *
        "across the frame, which leaves short trails in the corners of long sequences.\n" *
        "  Lens – one radial distortion for all frames (simple lenses, telescopes).\n" *
        "  Complex – also a smooth correction per frame, learned from the stars (ultra " *
        "wide-angle, refraction near the horizon). Needs many stars.\n  Auto – Complex when " *
        "enough stars match, else Lens.\nThe Frame report's align column shows the remaining error.",
    "Reduce light pollution" => "Removes a model of the sky background, leaving an even, " *
        "neutral sky. Correct the vignetting first (vignetting images), or it gets worse.\n" *
        "  Deep sky – a smooth polynomial; Degree 1 = a plain gradient, 2–4 = more curved glows.\n" *
        "  Uneven – follows the glow tile by tile (wide-angle, domes of city light); fewer tiles " *
        "= smoother. Very large structures (the Milky Way) may be reduced with many tiles.\n" *
        "I.A.A. (Uneven + sky region): also corrects foreground areas with the colour of the glow.\n" *
        "Show removed background shows the model in the Result view. Don't overdo it.",
    "Enhance star light" => "Lifts the stars above their surroundings in the sky region; " *
        "the background and its noise are left alone.",
    "Merge 4 pixels" => "Adds up 2×2 neighbouring pixels: brighter and less noisy, but half the " *
        "width and height.",
    "Time-lapse" => "Stacks every N consecutive frames into one low-noise frame: K star " *
        "images give K − N + 1 frames, numbered 0001… in a folder named after the output " *
        "(e.g. stacked_timelapse). Each is aligned to its middle frame, so the sky still moves; " *
        "all get the same brightness (no flicker). Quick preview shows the first one.",
    "Color space" => "Colours of the TIFF / JPEG output: sRGB (default) or Adobe RGB (wider " *
        "gamut; no ICC profile is embedded – assign Adobe RGB in your editor). FITS / XISF are " *
        "always linear sRGB.",
)

Base.@kwdef mutable struct StackState
    files::Vector{Vector{String}} = [String[] for _ in 1:OUT-1]   # per group; base, foreground: 0 or 1
    output::String = ""
    excluded::Set{String} = Set{String}()
    highlight::Tuple{Symbol,Int} = (:group, 1)   # (:group, g), (:file, row in the input list) or (:setting, k)
    list_top::Int = 0
    report_top::Int = 0
    shown::Union{Nothing,String} = nothing       # file in the base image view
    disp::Any = nothing                          # its display copy
    previews::Dict{String,Matrix{RGB{Float32}}} = Dict{String,Matrix{RGB{Float32}}}()
    line::Any = nothing                          # horizon (u1, v1, u2, v2), as fractions of width / height
    mask::Matrix{Bool} = falses(0, 0)            # painted sky, over the whole frame
    busy::Bool = false                           # loading a view
    running::Bool = false                        # stacking
    cancel::Threads.Atomic{Bool} = Threads.Atomic{Bool}(false)
    result::Any = nothing                        # last Stacker.stack result
    result_disp::Any = nothing                   # its on-screen copy
    result_bg::Any = nothing                     # … of the removed light pollution
    raw_small::Any = nothing                     # the linear stack at screen size, for live post-processing
    W_small::Any = nothing
    fwhm_small::Float64 = NaN
    post_lin::Any = nothing                      # post-processed, linear (for the histogram)
    post_version::Int = 0
    post_busy::Bool = false
    settings::Any = nothing                      # () -> settings for Stacker.stack
    project::Union{Nothing,String} = nothing     # last saved / opened project file
    api::Any = nothing                           # functions for scripting and tests
    to_stitcher::Any = nothing                   # paths -> nothing: hand results to the Stitcher tab
end

function build_stacker!(fig, root; active = () -> true)
    s = StackState()
    status = Observable("Add star images to begin. Highlight an entry to see its options below.")
    numboxes = Textbox[]

    P = PAL[]
    left  = root[1, 1] = GridLayout(tellheight = false, valign = :top)
    right = root[1, 2] = GridLayout(tellwidth = false, valign = :top)
    post  = root[1, 3] = GridLayout(tellheight = false, valign = :top)
    colsize!(root, 1, Fixed(390))
    colsize!(root, 3, Fixed(330))
    adv = Any[]                                  # advanced controls (removed in simple mode)
    mark() = length(fig.content)
    advanced!(n0) = append!(adv, fig.content[n0+1:end])

    passive!(a) = (hidedecorations!(a); foreach(k -> deregister_interaction!(a, k), collect(keys(interactions(a)))))

    # ---------------- left: inputs, settings, options ----------------
    treeax = Axis(left[1, 1:3], height = TREE_ROWS * ROW_PX, backgroundcolor = P.list)
    passive!(treeax)
    b_add, b_rm, b_clear, b_base = (sub = left[2, 1:3] = GridLayout();
        [Button(sub[1, k], label = l, tellwidth = false)
         for (k, l) in enumerate((icon("✚", "Add…"), icon("✖", "Remove"), icon("⌫", "Clear"), "Use as base"))])
    pathbox = Textbox(left[3, 1:3], placeholder = "…or type a file/folder path + Enter",
                      width = 370, halign = :left)
    NSET = 9
    setax = Axis(left[4, 1:3], height = ((UI[].simple ? 2 : NSET) + 1) * ROW_PX, backgroundcolor = P.list)
    passive!(setax)
    optitle = Observable("")
    Label(left[5, 1:3], optitle; font = :bold, fontsize = 16, halign = :left, padding = (0, 0, 0, 2))
    opts = Pages(left, 6, 1:3, OUT + NSET)
    runrow = left[7, 1:3] = GridLayout()
    b_quick = Button(runrow[1, 1], label = icon("◐", "Quick preview"), buttoncolor = P.accent,
                     labelcolor = P.onaccent, height = 40, width = 180)
    b_start = Button(runrow[1, 2], label = icon("▶", "Start"), buttoncolor = P.go, labelcolor = P.onaccent,
                     height = 40, width = 180)
    rowgap!(left, 5)

    # Options panel of each entry: rows of controls, then the explanation.
    page(k) = opts.pages[k]
    spage(k) = opts.pages[OUT+k]
    function note!(g, name)
        Label(g[isempty(g.content) ? 1 : nrows(g) + 1, 1:3], STACK_NOTES[name]; word_wrap = true,
              width = 370, halign = :left, justification = :left, fontsize = 12, color = P.muted)
        rowgap!(g, 5); colgap!(g, 6)
    end
    function toggle!(g, r, label; active = false)
        sub = g[r, 1:3] = GridLayout(halign = :left)
        t = Toggle(sub[1, 1], active = active)
        Label(sub[1, 2], label, halign = :left)
        return t
    end
    function menu!(g, r, label, options; default = first(options), width = 230)
        Label(g[r, 1], label, halign = :left)
        return Menu(g[r, 2:3], options = collect(options), default = default, width = width)
    end
    slider!(g, r, label, range, start) = numslider!(g, r, (; label, range, startvalue = start), numboxes)

    g = page(1)
    t_unify = toggle!(g, 1, "Unify exposure"); note!(g, "Star images")
    note!(page(2), "Base image")
    note!(page(3), "Noise images")
    note!(page(4), "Bias images")
    g = page(5)
    t_homog = toggle!(g, 1, "Homogenize vignetting"); note!(g, "Vignetting images")
    note!(page(6), "Flat-dark images")
    g = page(FG)
    t_fgmatch = toggle!(g, 1, "Match brightness to the stacked ground"; active = true)
    s_fgev = slider!(g, 2, "Exposure (EV)", -3:0.1:3, 0.0)
    note!(g, "Foreground image")
    g = page(OUT)
    outbox = Textbox(g[1, 1:2], placeholder = "output file (.tif / .jpg / .fits / .xisf)", width = 290, halign = :left)
    b_browse = Button(g[1, 3], label = "Browse…")
    note!(g, "Output")

    g = spage(1)
    m_comp = menu!(g, 1, "Composition", COMPOSITIONS)
    m_align = menu!(g, 2, "Stacking", ALIGN_MODES; default = "Select best pixels")
    m_model = menu!(g, 3, "Model", MODELS_)
    m_interp = menu!(g, 4, "Interpolation", INTERPOLATIONS)
    t_select = toggle!(g, 5, "Selective (sigma clipping in the sky)")
    m_fade = menu!(g, 6, "Trail fade", FADES)
    t_gaps = toggle!(g, 7, "Fill gaps between frames (star trails)")
    note!(g, "Composition")
    g = spage(2)
    m_sky = menu!(g, 1, "Sky region", SKY_MODES)
    s_grad = slider!(g, 2, "Gradient (% height)", 1:1:50, 10)
    s_brush = slider!(g, 3, "Brush size (% width)", 0.5:0.5:30, 5.0)
    s_feather = slider!(g, 4, "Mask feather (% width)", 0:0.5:10, 1.0)
    t_aux = toggle!(g, 5, "Auxiliary highlight")
    t_stars = toggle!(g, 6, "Show detected stars")
    skyb = g[7, 1:3] = GridLayout()
    b_detect = Button(skyb[1, 1], label = icon("◎", "Detect sky"), tellwidth = false)
    b_skyreset = Button(skyb[1, 2], label = "Reset sky region", tellwidth = false)
    note!(g, "Sky region")
    g = spage(3)
    s_siglo = slider!(g, 1, "Sigma low", 1.0:0.1:5.0, 3.0)
    s_sighi = slider!(g, 2, "Sigma high", 1.0:0.1:5.0, 3.0)
    s_sigit = slider!(g, 3, "Iterations", 1:1:10, 3)
    note!(g, "Sigma clipping")
    g = spage(4)
    t_rej = toggle!(g, 1, "Reject bad frames automatically"; active = true)
    s_rlight = slider!(g, 2, "Min. light (%)", 10:5:95, 60)
    s_rstars = slider!(g, 3, "Min. stars (%)", 10:5:95, 50)
    s_rfwhm = slider!(g, 4, "Max. FWHM (×)", 1.1:0.05:3.0, 1.5)
    s_relong = slider!(g, 5, "Max. elongation (×)", 1.05:0.05:2.0, 1.3)
    note!(g, "Frame rejection")
    g = spage(5)
    m_wb = menu!(g, 1, "White balance", first.(WHITE_BALANCE))
    m_dem = menu!(g, 2, "Demosaicing", first.(DEMOSAIC))
    t_half = toggle!(g, 3, "Half size")
    note!(g, "RAW conversion")
    g = spage(6)
    t_dyn = toggle!(g, 1, "Reduce dynamic noises"); note!(g, "Reduce dynamic noises")
    g = spage(7)
    m_dist = menu!(g, 1, "Correction", DISTORTION); note!(g, "Reduce distor. effects")
    g = spage(8)
    t_merge = toggle!(g, 1, "Merge 4 pixels"); note!(g, "Merge 4 pixels")
    g = spage(9)
    t_tl = toggle!(g, 1, "Time-lapse")
    s_tl = slider!(g, 2, "Frames per stack", 2:1:200, 10)
    tl_count = Observable("")
    Label(g[3, 1:3], tl_count, halign = :left, fontsize = 13)
    note!(g, "Time-lapse")

    # ---------------- far right: post-processing, live on the result ----------------
    Label(post[1, 1:3], "Post-processing"; font = :bold, fontsize = 16, halign = :left)
    Label(post[2, 1:3], "Applied to the result at once. Start saves with these settings; " *
          "Save result… saves again."; fontsize = 12, color = P.muted, halign = :left,
          justification = :left, word_wrap = true, width = 320)
    prow = Ref(2)
    pnext() = (prow[] += 1)
    ptoggle!(label; active = false) = toggle!(post, pnext(), label; active)
    pslider!(label, range, start) = slider!(post, pnext(), label, range, start)
    pmenu!(label, options) = menu!(post, pnext(), label, options; width = 190)
    t_bgx = ptoggle!("Background extraction")
    n0 = mark(); s_bgdeg = pslider!("Degree", 1:1:4, 2); advanced!(n0)
    t_ccal = ptoggle!("Color calibration")
    t_cnr = ptoggle!("Reduce colour noise")
    n0 = mark(); s_cnr = pslider!("Strength", 5:5:100, 50); advanced!(n0)
    m_poll = pmenu!("Light pollution", POLLUTION)     # stays in simple mode
    s_poll = pslider!("Strength", 0:1:100, 50)
    n0 = mark()
    s_deg = pslider!("Degree (deep sky)", 1:1:4, 2)
    s_tiles = pslider!("Tiles (uneven)", 6:1:48, 20)
    t_iaa = ptoggle!("I.A.A.")
    t_showbg = ptoggle!("Show removed background")
    t_enh = ptoggle!("Enhance star light")
    s_enh = pslider!("Strength", 0:1:100, 50)
    advanced!(n0)
    t_autob = ptoggle!("Auto brightness"; active = true)
    n0 = mark(); t_hdr = ptoggle!("High dynamic range"); advanced!(n0)
    m_tone = pmenu!("Tone", TONES)
    n0 = mark()
    histax = Axis(post[pnext(), 1:3], height = 55, backgroundcolor = :gray12, xticklabelsize = 9,
                  yticksvisible = false, yticklabelsvisible = false, xgridvisible = false,
                  ygridvisible = false, limits = (0, 1, 0, 1.05))
    foreach(k -> deregister_interaction!(histax, k), collect(keys(interactions(histax))))
    t_sauto = ptoggle!("Automatic stretch"; active = true)
    s_black = pslider!("Black point", 0:0.0005:0.5, 0.0)
    s_mid = pslider!("Midtones", 0.0005:0.0005:0.9995, 0.25)
    s_white = pslider!("White point", 0.5:0.0005:1.0, 1.0)
    advanced!(n0)
    m_cs = pmenu!("Color space", COLOR_SPACES)
    prowb = post[pnext(), 1:3] = GridLayout()
    b_linear = Button(prowb[1, 1], label = "Linear (for processing)", tellwidth = false)
    b_saveres = Button(prowb[1, 2], label = icon("⬇", "Save result…"), buttoncolor = P.go,
                       labelcolor = P.onaccent, tellwidth = false)
    b_tostitch = Button(post[pnext(), 1:3], label = icon("⇄", "Use in the Stitcher (panorama panel)"),
                        tellwidth = false)
    postnote = Observable(STACK_NOTES["Tone"])
    Label(post[pnext(), 1:3], postnote; fontsize = 12, color = P.muted, halign = :left,
          justification = :left, word_wrap = true, width = 320)
    rowgap!(post, 3); colgap!(post, 6)

    # ---------------- right: base image, result, frame report ----------------
    bar = right[1, 1] = GridLayout(halign = :right)
    viewtitle = Observable("Base image preview:")
    Label(right[2, 1], viewtitle, halign = :left, font = :bold, tellwidth = false)
    b_vbase, b_vres, b_vrep, b_fit, b_open, b_saveproj = (Button(bar[1, k], label = l)
        for (k, l) in enumerate(("Base image", "Result", "Frame report", icon("▣", "Fit"),
                                 icon("⇪", "Open project…"), icon("⬇", "Save project…"))))
    colgap!(bar, 4, 30)                                   # gap before the project buttons
    views = Pages(right, 3, 1, 3; width = Relative(1), height = Relative(1))
    pax = Axis(views.pages[1][1, 1], yreversed = true, aspect = DataAspect(), backgroundcolor = :gray20)
    passive!(pax)
    rax = Axis(views.pages[2][1, 1], yreversed = true, aspect = DataAspect(), backgroundcolor = :gray20)
    hidedecorations!(rax)
    deregister_interaction!(rax, :rectanglezoom)          # left-drag pans instead
    rax.panbutton = Mouse.left
    chartax = Axis(views.pages[3][1, 1], backgroundcolor = P.list, xlabel = "frame", ylabel = "÷ median",
                   xlabelsize = 11, ylabelsize = 11, xticklabelsize = 10, yticklabelsize = 10,
                   xgridvisible = false, limits = (nothing, (0, 2.6)))
    foreach(k -> deregister_interaction!(chartax, k), collect(keys(interactions(chartax))))
    repax = Axis(views.pages[3][2, 1], backgroundcolor = P.list)
    passive!(repax)
    rowsize!(views.pages[3], 1, Relative(0.34))
    Label(right[4, 1], status; halign = :left, justification = :left, word_wrap = true,
          tellwidth = false, color = P.status, fontsize = 15)
    rowsize!(right, 3, Relative(0.86))

    # ---------------- inputs ----------------

    stars() = s.files[1]
    base() = isempty(s.files[2]) ? (isempty(stars()) ? nothing : sort(stars())[cld(length(stars()), 2)]) :
                                   s.files[2][1]

    """Rows of the input list: (group, k) – k = 0 header, k > 0 file, -1 automatic base, -2 output."""
    function tree_rows()
        rows = Tuple{Int,Int}[]
        for g in eachindex(STACK_GROUPS)
            push!(rows, (g, 0))
            if g == OUT
                isempty(s.output) || push!(rows, (OUT, -2))
            elseif g == 2 && isempty(s.files[2]) && base() !== nothing
                push!(rows, (2, -1))
            else
                append!(rows, ((g, k) for k in eachindex(s.files[g])))
            end
        end
        return rows
    end
    """Status dot: green = given, red = required but missing, grey = optional / automatic."""
    function dot(g)
        g == OUT && return isempty(s.output) ? :red : :green
        g == 1 && isempty(stars()) && return :red
        return isempty(s.files[g]) ? P.off : :green
    end
    function row_text((g, k))
        k == 0 && return STACK_GROUPS[g] * (g ∉ (2, FG, OUT) && !isempty(s.files[g]) ? " ($(length(s.files[g])))" : "")
        k == -1 && return "     (auto) $(basename(base()))"
        k == -2 && return "     $(basename(s.output))"
        f = s.files[g][k]
        return "     $(basename(f))" * (g == 1 && f in s.files[2] ? "  ← base" : "") *
               (g == 1 && f in s.excluded ? "  ✗ excluded" : "")
    end
    """Group the highlighted entry belongs to (0 if a setting is highlighted)."""
    function hgroup()
        kind, k = s.highlight
        kind === :group && return k
        kind === :file && return tree_rows()[k][1]
        return 0
    end

    function draw_tree!()
        empty!(treeax)
        rows = tree_rows()
        n = length(rows)
        s.list_top = clamp(s.list_top, 0, max(0, n - TREE_ROWS))
        top = s.list_top
        limits!(treeax, 0, 1, -(top + TREE_ROWS), -top)
        vis = top+1:min(n, top + TREE_ROWS)
        hl = s.highlight[1] === :file ? s.highlight[2] :
             s.highlight[1] === :group ? findfirst(==((s.highlight[2], 0)), rows) : nothing
        hl !== nothing && hl in vis && poly!(treeax, Rect2f(0, -hl, 1, 1); color = (:dodgerblue, 0.35))
        hdr = [k for k in vis if rows[k][2] == 0]
        isempty(hdr) || scatter!(treeax, [Point2f(0.03, -(k - 0.5)) for k in hdr]; color = [dot(rows[k][1]) for k in hdr],
                 markersize = 11, strokecolor = P.muted, strokewidth = 0.8)
        text!(treeax, [Point2f(0.065, -(k - 0.5)) for k in vis]; text = [row_text(rows[k]) for k in vis],
              align = (:left, :center), fontsize = 13,
              color = [rows[k][2] == -1 || (rows[k][1] == 1 && rows[k][2] > 0 && stars()[rows[k][2]] in s.excluded) ?
                       P.muted : P.fg for k in vis])
        n > TREE_ROWS && text!(treeax, 0.99, -(top + 0.5); text = "$(first(vis))–$(last(vis)) of $n",
                               align = (:right, :center), fontsize = 11, color = P.muted)
    end

    # ---------------- settings list ----------------

    onoff(t) = t.active[] ? "On" : "Off"
    """Rows of the settings list: (setting, text, is it doing something)."""
    function setting_rows()
        comp = m_comp.selection[]
        rows = [
            (1, "Composition: $comp", true),
            (1, "     " * (comp == "Star trails" ? (m_fade.selection[] == "Off" ? "Lighten" : m_fade.selection[]) *
                                                 (t_gaps.active[] ? ", gaps filled" : "") :
                           m_align.selection[] * (t_select.active[] ? ", selective" : "")), true),
            (2, "Sky region: $(m_sky.selection[])", m_sky.selection[] != "None"),
            (3, "Sigma clipping: $(fmt(s_siglo.value[])) / $(fmt(s_sighi.value[])) σ, $(s_sigit.value[])×", true),
            (4, "Frame rejection: " * (t_rej.active[] ? "Auto" : "Off"), t_rej.active[]),
            (5, "RAW conversion: $(m_wb.selection[]) WB, $(m_dem.selection[])" *
                (t_half.active[] ? ", half size" : ""), true),
            (6, "Reduce dynamic noises: $(onoff(t_dyn))", t_dyn.active[]),
            (7, "Reduce distor. effects: $(m_dist.selection[])", m_dist.selection[] != "Off"),
            (8, "Merge 4 pixels: $(onoff(t_merge))", t_merge.active[]),
            (9, "Time-lapse: " * (t_tl.active[] ? "every $(s_tl.value[]) frames" : "Off"), t_tl.active[]),
        ]
        return UI[].simple ? [r for r in rows if r[1] <= 2] : rows       # simple mode: the essentials
    end

    function draw_settings!()
        empty!(setax)
        rows = setting_rows()
        limits!(setax, 0, 1, -length(rows), 0)
        if s.highlight[1] === :setting
            for (r, row) in enumerate(rows)
                row[1] == s.highlight[2] && poly!(setax, Rect2f(0, -r, 1, 1); color = (:dodgerblue, 0.35))
            end
        end
        first_of = [r for (r, row) in enumerate(rows) if r == 1 || rows[r-1][1] != row[1]]
        scatter!(setax, [Point2f(0.03, -(r - 0.5)) for r in first_of];
                 color = [rows[r][3] ? :royalblue : P.off for r in first_of],
                 markersize = 11, strokecolor = P.muted, strokewidth = 0.8)
        text!(setax, [Point2f(0.065, -(r - 0.5)) for r in eachindex(rows)]; text = [row[2] for row in rows],
              align = (:left, :center), fontsize = 13)
        tl_count[] = "$(length(stars())) star images → $(max(0, length(stars()) - s_tl.value[] + 1)) frames"
    end

    function highlight!(h)
        s.highlight = h
        g = hgroup()
        opts.current[] = g > 0 ? g : OUT + h[2]
        optitle[] = g > 0 ? STACK_GROUPS[g] :
                    replace(split(setting_rows()[findfirst(r -> r[1] == h[2], setting_rows())][2], ':')[1], " *" => "")
        draw_tree!(); draw_settings!()
    end

    # ---------------- base image view and sky region ----------------

    overlay = Observable(fill(RGBAf(0, 0, 0, 0), 1, 1))
    horizon = Observable(Point2f[])              # the boundary line and the end of the gradient
    brush = Observable(Point2f[])                # brush outline under the mouse
    starpts = Observable(Point2f[])              # stars found in the shown frame by the last run

    dsize() = s.disp === nothing ? (1, 1) : size(s.disp)
    to_uv(p) = (sz = dsize(); ((p[1] - 0.5) / sz[2], (p[2] - 0.5) / sz[1]))
    """Height (fraction of the frame) of the boundary line at u, or nothing."""
    function line_v(u)
        s.line === nothing && return nothing
        u1, v1, u2, v2 = s.line
        abs(u2 - u1) < 0.02 && return nothing
        return v1 + (u - u1) * (v2 - v1) / (u2 - u1)
    end
    mask_size() = (sz = dsize(); (max(1, round(Int, MASK_COLS * sz[1] / sz[2])), MASK_COLS))

    """Hatching over everything that is not sky (sparser in the gradient zone)."""
    function update_overlay!()
        mode = m_sky.selection[]
        mh, mw = mask_size()
        O = fill(RGBAf(0, 0, 0, 0), mh, mw)
        hatch = RGBAf(0.8, 0.85, 1.0, 0.55)
        gw = s_grad.value[] / 100
        if mode == "Irregular mask"
            # A mask made for another aspect ratio no longer fits (none while nothing is shown).
            s.disp === nothing || size(s.mask) == (mh, mw) || (s.mask = falses(mh, mw))
            if size(s.mask) == (mh, mw)
                Wm = any(s.mask) ? Sk.mask_weight(s.mask, s_feather.value[] / 100) : zeros(Float32, mh, mw)
                for c in 1:mw, r in 1:mh
                    wm = Wm[r, c]
                    wm < 0.999 && mod(r + c, wm > 0 ? 18 : 9) < 2 && (O[r, c] = hatch)   # sparse: the feather
                end
            end
        elseif mode != "None"
            for c in 1:mw
                v0 = line_v((c - 0.5) / mw)
                v0 === nothing && continue
                for r in 1:mh
                    v = (r - 0.5) / mh
                    zone = mode == "Gradient" && v < v0 + gw
                    v > v0 && mod(r + c, zone ? 18 : 9) < 2 && (O[r, c] = hatch)
                end
            end
        end
        overlay[] = permutedims(O)
        h, w = dsize()
        pts = Point2f[]
        if mode in ("Boundary line", "Gradient") && line_v(0.0) !== nothing
            for dv in (mode == "Gradient" ? (0.0, gw) : (0.0,))
                append!(pts, [Point2f(0.5, (line_v(0.0) + dv) * h + 0.5),
                              Point2f(w + 0.5, (line_v(1.0) + dv) * h + 0.5), Point2f(NaN, NaN)])
            end
        end
        horizon[] = pts
    end

    """Circles on the stars the last run detected in the shown frame (Show detected stars)."""
    function update_stars!()
        pts = Point2f[]
        if t_stars.active[] && s.result !== nothing && s.disp !== nothing
            fi = findfirst(f -> f.path == s.shown, s.result.report)
            if fi !== nothing
                rh, rw = size(s.result.image)
                dh, dw = size(s.disp)
                pts = [Point2f((st.x - 0.5) * dw / rw + 0.5, (st.y - 0.5) * dh / rh + 0.5)
                       for st in s.result.report[fi].detected]
            end
        end
        starpts[] = pts
    end

    function draw_preview!()
        empty!(pax)
        s.disp === nothing && return
        h, w = size(s.disp)
        img = t_aux.active[] ? map(c -> RGB{Float32}(min.(1, 3 .* (c.r, c.g, c.b) .^ 0.7)...), s.disp) : s.disp
        image!(pax, 0.5 .. w + 0.5, 0.5 .. h + 0.5, permutedims(img))
        image!(pax, 0.5 .. w + 0.5, 0.5 .. h + 0.5, overlay; interpolate = false)
        lines!(pax, horizon; color = :orange, linewidth = 2)
        lines!(pax, brush; color = (:lightblue, 0.9), linewidth = 1.5)
        scatter!(pax, starpts; marker = :circle, markersize = 13, color = (:white, 0.0),
                 strokecolor = (:lime, 0.85), strokewidth = 1.2)
        limits!(pax, 0.5, w + 0.5, h + 0.5, 0.5)      # y descending: keeps the axis reversed
        update_overlay!()
        update_stars!()
        update_title!()
    end

    function update_title!()
        k = views.current[]
        viewtitle[] = k == 1 ? (s.shown === nothing ? "Base image preview:" :
                                (s.shown == base() ? "Base image preview: " : "Preview: ") * basename(s.shown)) :
                      k == 2 ? (s.result === nothing ? "Result: press Quick preview or Start" :
                                (showing_bg() ? "Removed background" : "Result") *
                                (s.result.preview ? " (quick preview)" : "") * ": " *
                                (isempty(s.result.saved) ? "not saved" : basename(s.result.saved[end]))) :
                      "Frame report – click a frame to exclude / include it"
    end
    function show_view!(k)
        views.current[] = k
        k == 3 && draw_report!()
        update_title!()
    end

    """Show `path` (the base image when nothing else is highlighted)."""
    function preview!(path)
        views.current[] == 1 || show_view!(1)
        path == s.shown && return
        if path === nothing
            s.shown = nothing; s.disp = nothing
            draw_preview!(); update_title!()
            return
        end
        if haskey(s.previews, path)
            s.shown = path; s.disp = s.previews[path]
            draw_preview!()
            return
        end
        s.busy && return
        s.busy = true
        status[] = "Loading preview of $(basename(path))…"
        @async try
            img = bg(() -> (A = St.load_any(path; raw_half = true);
                            f = PREVIEW_MAX / maximum(size(A)); f < 1 ? St.resize(A, f) : A))
            s.previews[path] = img
            s.shown = path; s.disp = img
            draw_preview!()
            s.running || (status[] = "Ready.")
        catch e
            e isa TaskFailedException && (e = e.task.exception)
            status[] = "Cannot preview $(basename(path)): " * sprint(showerror, e)
        finally
            s.busy = false
        end
    end

    # ---------------- result and frame report ----------------

    showing_bg() = t_showbg.active[] && s.result !== nothing && s.result.background !== nothing
    """Show a stack result: screen-size copies for the live post-processing."""
    function set_result!(r)
        img = r.image
        f = min(1.0, RESULT_MAX / maximum(size(img)))
        s.result_disp = f < 1 ? bg(() -> St.resize(img, f)) : img
        B = r.background
        s.result_bg = B === nothing ? nothing : f < 1 ? bg(() -> St.resize(B, f)) : B
        s.raw_small = f < 1 ? bg(() -> St.resize(r.raw, f)) : r.raw
        s.W_small = f < 1 ? bg(() -> St.resize(r.W, f)) : r.W
        s.fwhm_small = r.fwhm * f
        s.post_lin = nothing
        s.result = r
        s.report_top = 0
        draw_result!()
        update_stars!()
        render_post!()
    end

    """Post-process the screen-size stack again (shortly after the last change)."""
    syncing_post = Ref(false)
    function render_post!()
        s.result === nothing && return
        s.post_version += 1
        v = s.post_version
        @async begin
            sleep(0.2)
            (v == s.post_version && !s.post_busy) || return
            s.post_busy = true
            try
                cfg = settings()
                pp = bg(() -> Sk.postprocess(s.raw_small, s.W_small, cfg; fwhm = s.fwhm_small))
                s.result_disp, s.result_bg, s.post_lin = pp.image, pp.background, pp.linear
                if pp.stretch !== nothing && t_sauto.active[]
                    syncing_post[] = true            # show the automatic values
                    set_close_to!(s_white, pp.stretch[3]); set_close_to!(s_black, pp.stretch[1])
                    set_close_to!(s_mid, pp.stretch[2])
                    syncing_post[] = false
                end
                draw_result!(); draw_hist!(); update_title!()
            catch e
                status[] = "Post-processing failed: " * sprint(showerror, e isa TaskFailedException ? e.task.exception : e)
            finally
                s.post_busy = false
                v != s.post_version && render_post!()  # changed meanwhile
            end
        end
    end

    """Histogram (log) of the post-processed linear result, with the stretch."""
    function draw_hist!()
        UI[].simple && return
        empty!(histax)
        s.post_lin === nothing && return
        hc = St.lum_histogram(s.post_lin)
        y = log10.(hc .+ 1); y ./= max(maximum(y), 1)
        xs = range(0.5 / length(y), 1 - 0.5 / length(y), length(y))
        band!(histax, collect(xs), zeros(length(y)), y; color = (:gray70, 0.8))
        if m_tone.selection[] == "Stretch"
            b, m, w = s_black.value[], s_mid.value[], s_white.value[]
            cx = range(0, 1, 200)
            lines!(histax, cx, [St.mtf(m, clamp((x - b) / max(w - b, 1e-6), 0, 1)) for x in cx];
                   color = :orange, linewidth = 1.5)
            vlines!(histax, [b]; color = :deepskyblue, linewidth = 1.5)
            vlines!(histax, [w]; color = :white, linewidth = 1.5)
        end
    end

    function draw_result!()
        empty!(rax)
        s.result_disp === nothing && return
        h, w = size(s.result.image)                  # in result pixels, whatever is shown
        img = showing_bg() ? s.result_bg : s.result_disp
        image!(rax, 0.5 .. w + 0.5, 0.5 .. h + 0.5, permutedims(img))
        limits!(rax, 0.5, w + 0.5, h + 0.5, 0.5)
    end

    report_lines() = s.result === nothing ? String[] :
        vcat(split(chomp(Sk.report_text(s.result.report)), '\n'),
             haskey(s.result, :log) ? vcat(["", "Process log (time per step):"], s.result.log) : String[])
    nreport() = s.result === nothing ? 0 : length(s.result.report)

    """Every measure per frame, divided by its median over the frames used; left-out frames shaded."""
    function draw_chart!()
        empty!(chartax)
        s.result === nothing && return
        infos = s.result.report
        n = length(infos)
        n == 0 && return
        xs = collect(1:n)
        for (k, fi) in enumerate(infos)
            off = fi.status in (:rejected, :failed) ? (P.bad, 0.25) :
                  (fi.status in (:excluded, :skipped) || fi.path in s.excluded) ? (P.muted, 0.25) :
                  fi.status === :base ? (P.info, 0.25) : nothing
            off === nothing || vspan!(chartax, [k - 0.5], [k + 0.5]; color = off)
        end
        hlines!(chartax, [1.0]; color = (P.muted, 0.6), linestyle = :dash)
        drawn = false
        for (label, f, col) in CHART_SERIES
            v = Float64[Float64(getfield(fi, f)) for fi in infos]
            okv = [v[k] for k in 1:n if infos[k].status in (:used, :base) && isfinite(v[k]) && v[k] > 0]
            isempty(okv) && continue
            m = median(okv)
            m > 0 || continue
            y = [isfinite(x) && !(f === :err && infos[k].status === :base) ? clamp(x / m, 0, 2.5) : NaN
                 for (k, x) in enumerate(v)]
            scatterlines!(chartax, xs, y; color = col, markersize = 6, linewidth = 1.5, label = label)
            drawn = true
        end
        xlims!(chartax, 0.5, n + 0.5)
        drawn && axislegend(chartax; position = :lt, orientation = :horizontal, labelsize = 10, framevisible = false,
                   patchsize = (10, 6), backgroundcolor = (:white, 0.0), padding = 2, rowgap = 0, colgap = 6)
    end
    function report_rows()
        px = widths(repax.scene.viewport[])[2]
        return max(5, floor(Int, px / ROW_PX))
    end
    function draw_report!()
        empty!(repax)
        s.result === nothing && empty!(chartax)
        lines = report_lines()
        nrow = report_rows()
        if isempty(lines)
            limits!(repax, 0, 1, -nrow, 0)
            text!(repax, 0.01, -0.5; text = "No report yet: press Quick preview or Start.",
                  align = (:left, :center), fontsize = 13, color = P.muted)
            return
        end
        infos = s.result.report
        draw_chart!()
        n = length(lines)
        s.report_top = clamp(s.report_top, 0, max(0, n - nrow))
        top = s.report_top
        limits!(repax, 0, 1, -(top + nrow), -top)
        vis = top+1:min(n, top + nrow)
        col(k) = k == 1 ? P.fg : k > nreport() + 1 ? P.muted :
                 (fi = infos[k-1]; fi.path in s.excluded && fi.status !== :excluded ? P.muted :
                  fi.status === :base ? P.info : fi.status === :used ? P.ok :
                  fi.status in (:rejected, :failed) ? P.bad : P.muted)
        text_of(k) = (k == 1 || k > nreport() + 1) ? lines[k] :
                     (fi = infos[k-1]; lines[k] * (fi.path in s.excluded && fi.status !== :excluded ?
                                                   "  → excluded next run" :
                                                   !(fi.path in s.excluded) && fi.status === :excluded ?
                                                   "  → included next run" : ""))
        top == 0 && poly!(repax, Rect2f(0, -1, 1, 1); color = P.panel)
        text!(repax, [Point2f(0.005, -(k - 0.5)) for k in vis]; text = [text_of(k) for k in vis],
              align = (:left, :center), fontsize = 12.5, font = mono_font(), color = [col(k) for k in vis])
    end

    # ---------------- inputs: changes ----------------

    function inputs_changed!()
        filter!(kv -> any(fs -> kv[1] in fs, s.files), s.previews)
        intersect!(s.excluded, Set(stars()))
        draw_tree!(); draw_settings!()
        kind, k = s.highlight
        rows = tree_rows()
        kind === :file && k > length(rows) && highlight!((:group, rows[end][1]))
        preview!(base())
    end

    function add!(ps)
        g = hgroup()
        g == 0 && (g = 1)
        ps = g == OUT ? ps : [p for p in ps if !(p in s.files[g])]
        isempty(ps) && return
        if g == OUT
            set_output!(first(ps))
        elseif g == 2
            s.files[2] = [first(ps)]
            first(ps) in stars() || push!(stars(), first(ps))
        elseif g == FG
            s.files[FG] = [first(ps)]
        else
            append!(s.files[g], ps)
            sort!(s.files[g])
        end
        if isempty(s.output) && !isempty(stars())
            set_output!(joinpath(dirname(abspath(first(stars()))), "stacked.tif"))
        end
        highlight!((:group, g))
        inputs_changed!()
        status[] = "Added $(length(ps)) file(s) to $(STACK_GROUPS[g])."
    end

    function set_output!(p)
        s.output = p
        outbox.stored_string[] = p; outbox.displayed_string[] = p
        draw_tree!()
    end

    """Remove the highlighted file, or all files of the highlighted group."""
    function remove!(; all = false)
        kind, k = s.highlight
        kind === :setting && return
        g = hgroup()
        if g == OUT
            set_output!("")
        elseif kind === :file && !all
            gg, i = tree_rows()[k]
            i > 0 || return
            f = s.files[gg][i]
            deleteat!(s.files[gg], i)
            gg == 1 && filter!(!=(f), s.files[2])
        else
            g == 1 && empty!(s.files[2])
            empty!(s.files[g])
        end
        inputs_changed!()
    end

    function toggle_exclude!(path)
        if path == (s.result === nothing ? base() : s.result.base) || path == base()
            status[] = "The base image cannot be excluded (choose another base first)."
            return
        end
        path in s.excluded ? delete!(s.excluded, path) : push!(s.excluded, path)
        draw_tree!(); views.current[] == 3 && draw_report!()
        status[] = "$(basename(path)) will be " * (path in s.excluded ? "left out of" : "used in") * " the next run."
    end

    # ---------------- running ----------------

    settings() = (
        stars = copy(stars()), base = isempty(s.files[2]) ? nothing : s.files[2][1],
        darks = copy(s.files[3]), bias = copy(s.files[4]), flats = copy(s.files[5]),
        flatdarks = copy(s.files[6]), output = s.output, excluded = collect(s.excluded),
        raw = (wb = Dict(WHITE_BALANCE)[m_wb.selection[]], quality = Dict(DEMOSAIC)[m_dem.selection[]],
               half = t_half.active[]),
        unify_exposure = t_unify.active[], homogenize_vignetting = t_homog.active[],
        composition = m_comp.selection[], align_mode = m_align.selection[], model = Symbol(m_model.selection[]),
        interpolation = m_interp.selection[],
        selective = t_select.active[], motion_effect = false,
        trails = (fade = m_fade.selection[], fill_gaps = t_gaps.active[]),
        foreground = (path = isempty(s.files[FG]) ? "" : s.files[FG][1], match = t_fgmatch.active[], ev = s_fgev.value[]),
        sigma = (low = s_siglo.value[], high = s_sighi.value[], iters = s_sigit.value[]),
        reject = (auto = t_rej.active[], min_light = s_rlight.value[] / 100, min_stars = s_rstars.value[] / 100,
                  max_fwhm = s_rfwhm.value[], max_elong = s_relong.value[]),
        sky = (mode = m_sky.selection[], line = s.line, gradient = s_grad.value[] / 100, mask = copy(s.mask),
               feather = s_feather.value[] / 100),
        auto_brightness = t_autob.active[], hdr = t_hdr.active[], dynamic_noise = t_dyn.active[],
        merge4 = t_merge.active[], color_space = m_cs.selection[], tone = m_tone.selection[],
        stretch = (auto = t_sauto.active[], black = s_black.value[], mid = s_mid.value[], white = s_white.value[]),
        bg_extraction = (on = t_bgx.active[], degree = s_bgdeg.value[]), color_calibration = t_ccal.active[],
        color_noise = (on = t_cnr.active[], strength = s_cnr.value[]),
        distortion = m_dist.selection[],
        light_pollution = (filter = m_poll.selection[], strength = s_poll.value[], iaa = t_iaa.active[],
                           degree = s_deg.value[], tiles = s_tiles.value[]),
        enhance_stars = (on = t_enh.active[], strength = s_enh.value[]),
        timelapse = (on = t_tl.active[], frames = s_tl.value[]))

    function summary(r)
        rep = r.report
        c(st) = count(fi -> fi.status === st, rep)
        nused = c(:used) + c(:base)
        rej = [fi for fi in rep if fi.status in (:rejected, :failed)]
        return (r.preview ? "Quick preview: " : "Done: ") * "$nused frame(s) stacked" *
               (isempty(rej) ? "" : ", $(length(rej)) left out (" *
                join(["$(basename(fi.path)): $(fi.reason)" for fi in rej[1:min(end, 2)]], "; ") *
                (length(rej) > 2 ? "; …" : "") * ") – see Frame report") *
               (c(:excluded) > 0 ? ", $(c(:excluded)) excluded by you" : "") *
               ". Brightness ×$(round(r.gain, digits = 2))." *
               (isempty(r.saved) ? "" : " Saved " * (length(r.saved) == 1 ? r.saved[1] :
                                                    "$(length(r.saved)) files in $(dirname(r.saved[1]))") * ".")
    end

    function run!(preview)
        if s.running
            s.cancel[] = true
            status[] = "Cancelling…"
            return
        end
        isempty(stars()) && (status[] = "Add star images first."; return)
        !preview && isempty(s.output) && (status[] = "Set the output file first (Output)."; return)
        cfg = settings()
        s.running = true; s.cancel[] = false
        msg = Ref("Starting…")
        progress = (m, _) -> (s.cancel[] && error("Cancelled."); msg[] = m; nothing)
        b_start.label[] = b_quick.label[] = "Cancel"
        @async while s.running
            status[] = msg[]
            sleep(0.25)
        end
        @async try
            t0 = time()
            r = bg(() -> Sk.stack(cfg; preview, progress))
            set_result!(merge(r, (preview = preview,)))
            show_view!(2)
            draw_tree!()
            s.running = false
            status[] = summary(s.result) * " ($(round(Int, time() - t0)) s)"
        catch e
            e isa TaskFailedException && (e = e.task.exception)
            e isa CompositeException && (e = first(e.exceptions)); e isa TaskFailedException && (e = e.task.exception)
            s.running = false
            status[] = s.cancel[] ? "Cancelled." : "Error: " * sprint(showerror, e)
            s.cancel[] || @error "Stacking failed" exception = (e, catch_backtrace())
        finally
            s.running = false
            b_start.label[] = "Start"; b_quick.label[] = "Quick preview"
        end
    end

    # ---------------- callbacks ----------------

    on(b_add.clicks) do _
        if hgroup() == OUT
            @async (p = bg(() -> pick_save(isempty(s.output) ? "stacked.tif" : s.output; title = "Output"));
                    p === nothing || set_output!(p))
        else
            @async (ps = bg(pick_files);
                    ps === nothing ? (status[] = NO_DIALOG) : add!(ps))
        end
    end
    on(_ -> remove!(), b_rm.clicks)
    on(_ -> remove!(all = true), b_clear.clicks)
    on(b_base.clicks) do _
        kind, k = s.highlight
        rows = tree_rows()
        if kind === :file && rows[k][1] == 1
            s.files[2] = [stars()[rows[k][2]]]
            delete!(s.excluded, s.files[2][1])
            inputs_changed!()
            status[] = "Base image: $(basename(s.files[2][1]))."
        else
            status[] = "Highlight one of the star images first."
        end
    end
    on(pathbox.stored_string) do str
        (str === nothing || isempty(strip(str))) && return
        p = expanduser(strip(str))
        endswith(lowercase(p), STACK_PROJECT_EXT) ? open_project!(p) : add!(expand_paths(str))
    end
    on(outbox.stored_string) do str
        str === nothing || str == s.output || (s.output = expanduser(strip(str)); draw_tree!())
    end
    on(b_browse.clicks) do _
        @async (p = bg(() -> pick_save(isempty(s.output) ? "stacked.tif" : s.output; title = "Output"));
                p === nothing || set_output!(p))
    end
    on(_ -> run!(true), b_quick.clicks)
    on(_ -> run!(false), b_start.clicks)
    on(_ -> (preview!(something(s.shown, base())); show_view!(1)), b_vbase.clicks)
    on(_ -> show_view!(2), b_vres.clicks)
    on(_ -> show_view!(3), b_vrep.clicks)
    on(b_fit.clicks) do _
        (views.current[] == 2 && s.result !== nothing) || return
        h, w = size(s.result.image)
        limits!(rax, 0.5, w + 0.5, h + 0.5, 0.5)
    end
    on(_ -> views.current[] == 3 && draw_report!(), repax.scene.viewport)

    for o in (m_comp.selection, m_align.selection, t_select.active, m_fade.selection, t_gaps.active, m_sky.selection,
              s_siglo.value, s_sighi.value, s_sigit.value, t_rej.active, m_wb.selection, m_dem.selection,
              t_half.active, t_dyn.active, m_dist.selection, t_merge.active, t_tl.active, s_tl.value)
        on(_ -> draw_settings!(), o)
    end
    on(_ -> update_overlay!(), m_sky.selection)
    on(_ -> update_overlay!(), s_grad.value)
    on(_ -> update_overlay!(), s_feather.value)
    on(_ -> draw_preview!(), t_aux.active)
    on(t_stars.active) do on_
        update_stars!()
        on_ && s.result === nothing && (status[] = "Stars are shown after a run (Quick preview or Start).")
    end
    on(b_detect.clicks) do _
        s.disp === nothing && (status[] = "Add star images first: the sky is detected in the shown frame."; return)
        img, shown = s.disp, s.shown
        status[] = "Detecting the sky in $(basename(shown))…"
        @async try
            mask, _ = bg(() -> Sk.detect_sky(img; cols = MASK_COLS, rows = mask_size()[1]))
            s.mask = mask
            m_sky.i_selected[] = findfirst(==("Irregular mask"), m_sky.options[])
            update_overlay!()
            status[] = "Sky detected in $(basename(shown)): $(round(Int, 100 * count(mask) / length(mask))) % of the " *
                       "frame (hatched = foreground). Approximate: correct it with the brush – left button " *
                       "paints sky, right erases, wheel = brush size."
        catch e
            status[] = "Sky detection failed: " * sprint(showerror, e isa TaskFailedException ? e.task.exception : e)
        end
    end
    on(b_tostitch.clicks) do _
        r = s.result
        if r === nothing || r.preview || isempty(r.saved)
            status[] = "Press Start first: the saved result goes to the Stitcher."
        elseif length(r.saved) > 1
            status[] = "This run saved $(length(r.saved)) files (time-lapse / align only); the Stitcher takes one stacked panel."
        elseif s.to_stitcher === nothing
            status[] = "No Stitcher in this window."
        else
            s.to_stitcher([r.saved[1]])
            status[] = "$(basename(r.saved[1])) added to the Stitcher. Stack the next panel here (same settings, " *
                       "a new output name), add it too, then stitch them there."
        end
    end
    post_controls = (t_bgx.active => "Background extraction", s_bgdeg.value => "Background extraction",
        t_ccal.active => "Color calibration", t_cnr.active => "Reduce colour noise",
        s_cnr.value => "Reduce colour noise", m_poll.selection => "Reduce light pollution",
        s_poll.value => "Reduce light pollution", s_deg.value => "Reduce light pollution",
        s_tiles.value => "Reduce light pollution", t_iaa.active => "Reduce light pollution",
        t_enh.active => "Enhance star light", s_enh.value => "Enhance star light",
        t_autob.active => "Auto brightness", t_hdr.active => "High dynamic range", m_tone.selection => "Tone",
        t_sauto.active => "Tone", m_cs.selection => "Color space")
    for (o, name) in post_controls
        on(o) do _
            postnote[] = STACK_NOTES[name]
            render_post!()
        end
    end
    for sl in (s_black, s_mid, s_white)                 # moving a stretch slider: by hand from now on
        on(sl.value) do _
            syncing_post[] && return
            t_sauto.active[] && (t_sauto.active[] = false)
            postnote[] = STACK_NOTES["Tone"]
            render_post!()
        end
    end
    on(b_linear.clicks) do _
        t_bgx.active[] = false; t_ccal.active[] = false; t_cnr.active[] = false; t_enh.active[] = false
        t_autob.active[] = false; t_hdr.active[] = false
        m_poll.i_selected[] = 1; m_cs.i_selected[] = 1
        m_tone.i_selected[] = findfirst(==("Linear"), m_tone.options[])
        if !isempty(s.output) && !St.isastro(s.output)
            set_output!(splitext(s.output)[1] * ".fits")
        end
        postnote[] = STACK_NOTES["Linear"]
        status[] = "Linear output: no post-processing, no brightness change, no tone curve" *
                   (isempty(s.output) ? "." : "; output $(basename(s.output)) (32-bit).")
    end
    on(b_saveres.clicks) do _
        s.result === nothing && (status[] = "Nothing to save yet: press Start."; return)
        s.result.preview && (status[] = "This is a quick preview (small); press Start for the full-size result."; return)
        @async begin
            p = bg(() -> pick_save(isempty(s.output) ? "stacked.tif" : s.output; title = "Save result"))
            p === nothing && return
            status[] = "Saving $(basename(p))…"
            try
                bg(() -> Sk.save_result(s.result, merge(settings(), (output = p,))))
                status[] = "Saved $p" * (St.isastro(p) ? " (linear 32-bit)." : ".") *
                           (t_tl.active[] ? " (Time-lapse: only the first frame.)" : "")
            catch e
                status[] = "Error: " * sprint(showerror, e isa TaskFailedException ? e.task.exception : e)
            end
        end
    end
    on(t_showbg.active) do _
        draw_result!(); update_title!()
        views.current[] == 2 || show_view!(2)
    end
    on(b_skyreset.clicks) do _
        s.line = nothing; s.mask = falses(0, 0)
        update_overlay!()
    end

    # Lists: click to highlight, wheel scrolls.
    on(events(fig).mousebutton, priority = 5) do ev
        (active() && ev.button == Mouse.left && ev.action == Mouse.press) || return Consume(false)
        if is_mouseinside(treeax)
            k = floor(Int, -mouseposition(treeax)[2]) + 1
            rows = tree_rows()
            1 <= k <= length(rows) || return Consume(true)
            g, i = rows[k]
            highlight!(i == 0 ? (:group, g) : (:file, k))
            i > 0 && g < OUT && preview!(s.files[g][i])
            i == -1 && preview!(base())
            return Consume(true)
        elseif is_mouseinside(setax)
            k = floor(Int, -mouseposition(setax)[2]) + 1
            rows = setting_rows()
            1 <= k <= length(rows) && highlight!((:setting, rows[k][1]))
            return Consume(true)
        elseif is_mouseinside(chartax) && s.result !== nothing
            k = round(Int, mouseposition(chartax)[1])
            1 <= k <= nreport() && toggle_exclude!(s.result.report[k].path)
            return Consume(true)
        elseif is_mouseinside(repax) && s.result !== nothing
            k = floor(Int, -mouseposition(repax)[2]) + 1
            2 <= k <= length(s.result.report) + 1 && toggle_exclude!(s.result.report[k-1].path)
            return Consume(true)
        end
        return Consume(false)
    end

    # Base image view: draw the boundary line, or paint / erase the sky mask.
    stroke = Ref{Any}(nothing)                   # :line, true (paint) or false (erase)
    function paint!(u, v, val)
        mh, mw = size(s.mask)
        cx, cy = u * mw + 0.5, v * mh + 0.5
        rr = s_brush.value[] / 100 * mw
        for c in max(1, floor(Int, cx - rr)):min(mw, ceil(Int, cx + rr)),
            r in max(1, floor(Int, cy - rr)):min(mh, ceil(Int, cy + rr))
            (c - cx)^2 + (r - cy)^2 <= rr^2 && (s.mask[r, c] = val)
        end
        update_overlay!()
    end
    function brush_at!(p)
        if m_sky.selection[] != "Irregular mask" || p === nothing
            isempty(brush[]) || (brush[] = Point2f[])
            return
        end
        rr = s_brush.value[] / 100 * dsize()[2]
        brush[] = [Point2f(p[1] + rr * cos(t), p[2] + rr * sin(t)) for t in range(0, 2π, 65)]
    end
    on(events(fig).mousebutton, priority = 5) do ev
        if ev.action == Mouse.release && stroke[] !== nothing
            stroke[] = nothing
            return Consume(true)
        end
        (active() && ev.action == Mouse.press && s.disp !== nothing && is_mouseinside(pax)) ||
            return Consume(false)
        p = mouseposition(pax)
        u, v = to_uv(p)
        mode = m_sky.selection[]
        if mode == "Irregular mask" && ev.button in (Mouse.left, Mouse.right)
            size(s.mask) == mask_size() || (s.mask = falses(mask_size()...))
            stroke[] = ev.button == Mouse.left
            paint!(u, v, stroke[])
        elseif mode in ("Boundary line", "Gradient") && ev.button == Mouse.left
            stroke[] = :line
            s.line = (u, v, u, v)
            update_overlay!()
        elseif mode == "None"
            status[] = "Choose a sky region mode (Sky region in the list) to draw on the preview."
        end
        return Consume(stroke[] !== nothing)
    end
    on(events(fig).mouseposition, priority = 5) do _
        (active() && s.disp !== nothing) || return Consume(false)
        inside = is_mouseinside(pax)
        p = inside ? mouseposition(pax) : nothing
        brush_at!(p)
        (stroke[] === nothing || p === nothing) && return Consume(false)
        u, v = to_uv(p)
        if stroke[] === :line
            s.line = (s.line[1], s.line[2], u, v)
            update_overlay!()
        else
            paint!(u, v, stroke[])
        end
        return Consume(true)
    end
    on(events(fig).scroll, priority = 5) do (_, dy)
        active() || return Consume(false)
        if is_mouseinside(treeax)
            s.list_top -= round(Int, 2 * sign(dy))
            draw_tree!()
            return Consume(true)
        elseif is_mouseinside(repax)
            s.report_top -= round(Int, 3 * sign(dy))
            draw_report!()
            return Consume(true)
        elseif is_mouseinside(pax) && m_sky.selection[] == "Irregular mask"
            set_close_to!(s_brush, clamp(s_brush.value[] * 1.15^sign(dy), 0.5, 30))
            brush_at!(mouseposition(pax))
            return Consume(true)
        end
        return Consume(false)
    end
    on(events(fig).keyboardbutton) do ev
        (active() && ev.action == Keyboard.press && !pathbox.focused[] && !outbox.focused[] &&
         !any(tb -> tb.focused[], numboxes)) || return Consume(false)
        if ev.key in (Keyboard.delete, Keyboard.backspace) && is_mouseinside(treeax)
            remove!()
            return Consume(true)
        end
        return Consume(false)
    end

    # ---------------- projects ----------------

    menu_by_name = Dict("composition" => m_comp, "stacking" => m_align, "model" => m_model, "sky region" => m_sky,
                        "white balance" => m_wb, "demosaicing" => m_dem, "distortion" => m_dist,
                        "light pollution" => m_poll, "color space" => m_cs, "tone" => m_tone,
                        "interpolation" => m_interp, "trail fade" => m_fade)
    toggle_by_name = Dict("unify exposure" => t_unify, "homogenize vignetting" => t_homog, "selective" => t_select,
                          "fill gaps" => t_gaps, "auxiliary highlight" => t_aux, "reject" => t_rej,
                          "foreground match" => t_fgmatch, "show stars" => t_stars,
                          "half size" => t_half, "auto brightness" => t_autob, "hdr" => t_hdr,
                          "dynamic noises" => t_dyn, "iaa" => t_iaa, "show background" => t_showbg,
                          "background extraction" => t_bgx, "color calibration" => t_ccal,
                          "colour noise" => t_cnr, "automatic stretch" => t_sauto, "enhance star light" => t_enh,
                          "merge 4 pixels" => t_merge, "time-lapse" => t_tl)
    slider_by_name = Dict("gradient" => s_grad, "brush" => s_brush, "sigma low" => s_siglo, "sigma high" => s_sighi,
                          "sigma iterations" => s_sigit, "min light" => s_rlight, "min stars" => s_rstars,
                          "max fwhm" => s_rfwhm, "max elongation" => s_relong, "light pollution strength" => s_poll,
                          "light pollution degree" => s_deg, "light pollution tiles" => s_tiles,
                          "background degree" => s_bgdeg, "colour noise strength" => s_cnr,
                          "black point" => s_black, "midtones" => s_mid, "white point" => s_white,
                          "enhance strength" => s_enh, "time-lapse frames" => s_tl, "feather" => s_feather,
                          "foreground ev" => s_fgev)

    # The painted mask as run lengths (alternately not sky / sky, column by column).
    function runs(v)
        out, cur, n = Int[], false, 0
        for b in v
            b == cur ? (n += 1) : (push!(out, n); cur = b; n = 1)
        end
        return push!(out, n)
    end
    function unruns(r, len)
        v, p = falses(len), 0
        for (k, n) in enumerate(r)
            iseven(k) && (v[p+1:p+n] .= true)
            p += n
        end
        return v
    end

    function save_project!(path)
        endswith(lowercase(path), STACK_PROJECT_EXT) || (path *= STACK_PROJECT_EXT)
        dir = dirname(abspath(path))
        ref(p) = Dict("path" => abspath(p), "relative" => relpath(abspath(p), dir))
        d = Dict{String,Any}(
            "format" => "PanoStitch stacking project", "version" => 2,
            "files" => Dict(GROUP_KEYS[g] => [ref(p) for p in s.files[g]] for g in 1:OUT-1),
            "excluded" => [k for (k, p) in enumerate(stars()) if p in s.excluded],
            "menus" => Dict(n => m.selection[] for (n, m) in menu_by_name),
            "toggles" => Dict(n => t.active[] for (n, t) in toggle_by_name),
            "sliders" => Dict(n => sl.value[] for (n, sl) in slider_by_name))
        isempty(s.output) || (d["output"] = ref(s.output))
        s.line === nothing || (d["sky_line"] = collect(Float64.(s.line)))
        any(s.mask) && (d["sky_mask"] = Dict("size" => collect(size(s.mask)), "runs" => runs(vec(s.mask))))
        open(io -> TOML.print(io, d), path, "w")
        s.project = path
        return path
    end

    function open_project!(path)
        s.running && (status[] = "Wait until the stacking has finished."; return)
        d = try
            TOML.parsefile(path)
        catch e
            status[] = "Cannot read $(basename(path)): " * sprint(showerror, e)
            return
        end
        get(d, "format", "") == "PanoStitch stacking project" ||
            (status[] = "$(basename(path)) is not a stacking project."; return)
        dir = dirname(abspath(path))
        resolve(r) = isfile(r["path"]) ? r["path"] :
                     (p = normpath(joinpath(dir, r["relative"])); isfile(p) ? p : nothing)
        missing_ = String[]
        found = map(1:OUT-1) do g
            map(get(get(d, "files", Dict()), GROUP_KEYS[g], [])) do r
                p = resolve(r)
                p === nothing && push!(missing_, basename(r["path"]))
                p
            end
        end
        s.files = [String[p for p in ps if p !== nothing] for ps in found]
        s.excluded = Set(found[1][k] for k in get(d, "excluded", Int[])
                         if k <= length(found[1]) && found[1][k] !== nothing)
        # The output need not exist yet: keep its absolute path if that folder exists.
        out = get(d, "output", nothing)
        set_output!(out === nothing ? "" : isdir(dirname(out["path"])) ? out["path"] :
                                           normpath(joinpath(dir, out["relative"])))
        s.shown = nothing; s.disp = nothing; empty!(s.previews)
        for (n, m) in menu_by_name
            v = get(get(d, "menus", Dict()), n, nothing)
            v !== nothing && v in m.options[] && (m.i_selected[] = findfirst(==(v), m.options[]))
        end
        for (n, t) in toggle_by_name
            v = get(get(d, "toggles", Dict()), n, nothing)
            v === nothing || (t.active[] = v)
        end
        # Version 1 projects: "motion effect" became the trail fade "Fade in".
        if get(get(d, "toggles", Dict()), "motion effect", false) == true && !haskey(get(d, "menus", Dict()), "trail fade")
            m_fade.i_selected[] = findfirst(==("Fade in"), m_fade.options[])
        end
        for (n, sl) in slider_by_name
            v = get(get(d, "sliders", Dict()), n, nothing)
            v === nothing || set_close_to!(sl, v)
        end
        s.line = haskey(d, "sky_line") ? Tuple(Float64.(d["sky_line"])) : nothing
        mk = get(d, "sky_mask", nothing)
        s.mask = mk === nothing ? falses(0, 0) : reshape(unruns(mk["runs"], prod(mk["size"])), mk["size"]...)
        s.result = nothing; s.result_disp = nothing; empty!(rax)
        s.list_top = 0
        s.project = path
        highlight!((:group, 1))
        inputs_changed!()
        show_view!(1)
        status[] = "Opened $(basename(path)): $(length(stars())) star images" *
                   join([", $(length(s.files[g])) $(lowercase(STACK_GROUPS[g]))" for g in 3:OUT-1 if !isempty(s.files[g])]) *
                   "." * (isempty(missing_) ? "" : " Missing files: $(join(missing_, ", ")).")
    end

    on(b_open.clicks) do _
        @async begin
            p = bg(() -> pick_project(STACK_PROJECT_EXT, "Stacking projects"))
            p === nothing ? (status[] = "No project chosen (without a file dialog, type the path of a " *
                                        "$STACK_PROJECT_EXT file into the path box).") : open_project!(p)
        end
    end
    on(b_saveproj.clicks) do _
        @async begin
            default = s.project !== nothing ? s.project :
                      isempty(stars()) ? "stacking$STACK_PROJECT_EXT" :
                      joinpath(dirname(abspath(first(stars()))), "stacking$STACK_PROJECT_EXT")
            out = bg(() -> pick_save(default; title = "Save stacking project"))
            out === nothing && return
            try
                status[] = "Saved project $(save_project!(out))."
            catch e
                status[] = "Error: " * sprint(showerror, e)
            end
        end
    end

    s.settings = settings
    s.api = (; add = add!, highlight = highlight!, set_output = set_output!, run = run!,
             show_view = show_view!, exclude = toggle_exclude!, save_project = save_project!,
             open_project = open_project!, set_result = set_result!)
    UI[].simple && remove_blocks!(adv)
    highlight!((:group, 1))
    show_view!(1)
    return s
end
