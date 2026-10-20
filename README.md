# subl-bypass

Patcher for Sublime Text 4 build 4200 x64 (Windows). It applies two in-place
byte patches:

| Patch | File offset | Change |
| --- | --- | --- |
| UI/license flag | `0x46B80` | Replaces the `movzx`/`xor` flag sequence with a direct set |
| Automatic update-check override | `0x32623` | Changes `JNZ` to `JMP`, skipping the forced `update_check` write |

With `"update_check": false`, the automatic update request is skipped. Manual
`Help > Check for Updates...` still works.

The patcher is signature-based. It rejects unsupported or ambiguous files
without writing, detects patches that are already applied, and verifies the
result after writing.

## Build

```powershell
cargo build --release
```

## Usage

```powershell
# Interactive path prompt
.\target\release\subl-bypass.exe

# Explicit path
.\target\release\subl-bypass.exe "C:\Program Files\Sublime Text\sublime_text.exe"

# Report what would change without writing
.\target\release\subl-bypass.exe --dry-run

# Test against a copy without closing or relaunching Sublime
.\target\release\subl-bypass.exe .\sublime_text.exe --dry-run --no-close --no-restart
```

Options:

- `--dry-run` — report patch results without writing anything
- `--no-close` — do not close Sublime Text before writing
- `--no-restart` — do not launch Sublime Text after patching
- `--help`, `-h` — show help

The tool closes Sublime Text before writing and relaunches it with
`--command about` afterward. Run it from an elevated terminal when patching
files under `C:\Program Files`.

## Backups and restore

If `sublime_text.bak` does not exist, it is created next to the target before
the first write. If it already exists, a timestamped
`sublime_text.bak.<epoch>` backup is created instead, so an existing pristine
backup is never overwritten. To restore, close Sublime and copy the backup
over the target:

```powershell
Copy-Item "C:\Program Files\Sublime Text\sublime_text.bak" `
          "C:\Program Files\Sublime Text\sublime_text.exe" -Force
```

## Scope

The signatures target Sublime Text 4 build 4200 x64. Other builds are rejected
without modification.
