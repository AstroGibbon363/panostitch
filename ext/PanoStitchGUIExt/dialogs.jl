# File dialogs with the tools each system already has, so nothing extra needs
# installing:
#   Linux    zenity (GNOME, XFCE, …), kdialog (KDE, tried first there) or yad
#   macOS    AppleScript (osascript)
#   Windows  PowerShell with the .NET file dialogs
# Without any of them the path boxes still work. The dialogs run as separate
# programs, so the window keeps redrawing (call them through `bg`).

"""The dialog tool to use, or nothing."""
function dialog_tool()
    if Sys.iswindows()
        for t in ("powershell", "pwsh")
            Sys.which(t) !== nothing && return Symbol(t)
        end
        return nothing
    end
    Sys.isapple() && return Sys.which("osascript") !== nothing ? :osascript : nothing
    kde = occursin("KDE", uppercase(get(ENV, "XDG_CURRENT_DESKTOP", "")))
    for t in (kde ? ("kdialog", "zenity", "yad") : ("zenity", "kdialog", "yad"))
        Sys.which(t) !== nothing && return Symbol(t)
    end
    return nothing
end

"""Run a dialog; its output, or nothing if it was cancelled (non-zero exit or no output)."""
function run_dialog(cmd)
    out = IOBuffer()
    p = try
        run(pipeline(ignorestatus(cmd); stdout = out, stderr = devnull))
    catch
        return nothing
    end
    s = strip(String(take!(out)))
    return p.exitcode == 0 && !isempty(s) ? String(s) : nothing
end

applescript(s) = "\"" * replace(s, "\\" => "\\\\", "\"" => "\\\"") * "\""   # quoted string
powershell(s) = "'" * replace(s, "'" => "''") * "'"
const PS_HEAD = "[Console]::OutputEncoding = [System.Text.Encoding]::UTF8; " *
                "Add-Type -AssemblyName System.Windows.Forms; "

"""
Choose files to open. `filter` = (description, extensions such as ".fits").
Returns the chosen paths (empty if cancelled), or nothing if there is no dialog tool.
`tool` picks the tool (tests).
"""
function open_dialog(; title, multiple = false, filter = nothing, tool = dialog_tool())
    tool === nothing && return nothing
    name, exts = filter === nothing ? ("All files", String[]) : filter
    # Linux file systems are case-sensitive: offer both cases.
    globs = unique([e2 for e in exts for e2 in ("*$e", "*$(uppercase(e))")])
    if tool === :zenity
        cmd = `zenity --file-selection --title=$title $(multiple ? ["--multiple", "--separator=\n"] : String[])
               $(isempty(exts) ? String[] : ["--file-filter=$name | $(join(globs, ' '))", "--file-filter=All files | *"])`
    elseif tool === :kdialog
        cmd = `kdialog --title $title --getopenfilename $(homedir())
               $(isempty(exts) ? String[] : ["$name ($(join(globs, ' ')))"])
               $(multiple ? ["--multiple", "--separate-output"] : String[])`
    elseif tool === :yad
        cmd = `yad --file --title=$title --width=1000 --height=650 $(multiple ? ["--multiple", "--separator=\n"] : String[])
               $(isempty(exts) ? String[] : ["--file-filter=$name | $(join(globs, ' '))", "--file-filter=All files | *"])`
    elseif tool === :osascript
        pick = "choose file with prompt $(applescript(title))" * (multiple ? " with multiple selections allowed" : "")
        lines = multiple ?
            ["set fs to ($pick)", "set out to \"\"",
             "repeat with f in fs", "set out to out & POSIX path of f & linefeed", "end repeat", "return out"] :
            ["return POSIX path of ($pick)"]
        cmd = `osascript $(reduce(vcat, [["-e", l] for l in lines]))`
    else
        pats = isempty(exts) ? "All files|*.*" : "$name|$(join(["*$e" for e in exts], ';'))|All files|*.*"
        script = PS_HEAD * "\$d = New-Object System.Windows.Forms.OpenFileDialog; " *
                 "\$d.Title = $(powershell(title)); \$d.Filter = $(powershell(pats)); " *
                 "\$d.Multiselect = \$$(multiple); " *
                 "if (\$d.ShowDialog() -eq 'OK') { [Console]::Out.Write((\$d.FileNames -join \"`n\")) }"
        cmd = `$(string(tool)) -NoProfile -STA -Command $script`
    end
    out = run_dialog(cmd)
    out === nothing && return String[]
    return sort([String(strip(l)) for l in split(out, '\n') if !isempty(strip(l))])
end

"""
Choose where to save, starting at `default`. Returns the path, nothing if
cancelled, or `default` itself if there is no dialog tool (as before).
"""
function save_dialog(default; title, tool = dialog_tool())
    tool === nothing && return default
    dir, name = dirname(default), basename(default)
    if tool === :zenity
        cmd = `zenity --file-selection --save --title=$title --filename=$default`
    elseif tool === :kdialog
        cmd = `kdialog --title $title --getsavefilename $default`
    elseif tool === :yad
        cmd = `yad --file --save --confirm-overwrite --title=$title --width=1000 --height=650 --filename=$default`
    elseif tool === :osascript
        loc = isdir(dir) ? " default location (POSIX file $(applescript(dir)))" : ""
        cmd = `osascript -e $("return POSIX path of (choose file name with prompt $(applescript(title)) " *
                              "default name $(applescript(name))$loc)")`
    else
        script = PS_HEAD * "\$d = New-Object System.Windows.Forms.SaveFileDialog; " *
                 "\$d.Title = $(powershell(title)); \$d.FileName = $(powershell(name)); " *
                 (isdir(dir) ? "\$d.InitialDirectory = $(powershell(dir)); " : "") *
                 "if (\$d.ShowDialog() -eq 'OK') { [Console]::Out.Write(\$d.FileName) }"
        cmd = `$(string(tool)) -NoProfile -STA -Command $script`
    end
    return run_dialog(cmd)
end

# The dialogs the two tabs use.
pick_files() = open_dialog(; title = "Add images", multiple = true, filter = ("Images", sort(collect(St.IMAGE_EXTS))))
pick_save(default; title = "Save panorama") = save_dialog(default; title)
function pick_project(ext = PROJECT_EXT, what = "PanoStitch projects")
    ps = open_dialog(; title = "Open project", filter = (what, [ext]))
    return ps === nothing || isempty(ps) ? nothing : first(ps)
end

const NO_DIALOG = "No file dialog available (install zenity or kdialog on Linux): " *
                  "type a path into the box instead."
