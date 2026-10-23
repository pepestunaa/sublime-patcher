# sublime-patcher

Patches Sublime Text 4 build 4200 x64 (Windows) in place. Three byte patches:

- **UI/license flag** — file offset `0x46B80`
- **automatic update-check override** — file offset `0x32623`
- **update-check startup gate** — file offset `0x31B52`

With `"update_check": false` the automatic update request is skipped; manual
`Help > Check for Updates...` still works. Other builds are rejected without
modification.

The startup gate matters because build 4200 only honors `update_check` for
licensed copies: at launch an unlicensed copy never reads the setting, so the
internal auto-check flag keeps its on-disk default of `true` and a due check
fires even with the override patch alone. The gate patch makes the startup
path load the setting unconditionally.

If you need to install Sublime Text first, grab the official x64 build 4200
setup from [here](https://download.sublimetext.com/sublime_text_build_4200_x64_setup.exe) and then run the one-liner or from source.

## One-liner

Run this from any PowerShell prompt. The first run asks for Administrator
(UAC) when it needs to write to `C:\Program Files\Sublime Text`, then closes
Sublime Text, patches, backs up, and relaunches it:

```powershell
Set-ExecutionPolicy Bypass -Scope Process -Force; irm https://raw.githubusercontent.com/pepestunaa/sublime-patcher/main/sublime-patcher.ps1 | iex
```

Pass `-NoElevate` if you want to decline the UAC ask.

## From source

```powershell
cargo build --release
.\target\release\sublime-patcher.exe
```

Flags:

- `--dry-run` — report patch results without writing anything
- `--no-close` — do not close Sublime Text before writing
- `--no-restart` — do not launch Sublime Text after patching
- `--help`, `-h` — show help

## Backups and restore

`sublime_text.bak` is created next to the target on the first write. Later
writes use `sublime_text.bak.<epoch>`, so the pristine backup is never
overwritten. To restore, close Sublime and copy the backup over the target:

```powershell
Copy-Item "C:\Program Files\Sublime Text\sublime_text.bak" `
          "C:\Program Files\Sublime Text\sublime_text.exe" -Force
```
