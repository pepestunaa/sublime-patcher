#Requires -Version 5.1
<#
.SYNOPSIS
  In-place patcher for Sublime Text 4 build 4200 x64 (Windows).

.DESCRIPTION
  Applies the UI/license-flag patch and the automatic update-check override
  in one run. Refuses to write unless every signature is found in the
  expected file, and creates a backup before any write.

.EXAMPLE
  .\sublime-patcher.ps1

  Applies both patches to the default install path. Requires an elevated
  PowerShell session.

.EXAMPLE
  .\sublime-patcher.ps1 -Path 'C:\temp\sublime_text.exe' -DryRun

  Dry-run mode: reports what would change without saving anything.

.EXAMPLE
  Set-ExecutionPolicy Bypass -Scope Process -Force
  & ([ScriptBlock]::Create((irm 'https://raw.githubusercontent.com/pepestunaa/sublime-patcher/main/sublime-patcher.ps1')))

  Runs the hosted one-liner in the current session.
#>
[CmdletBinding()]
param(
    [ValidateNotNullOrEmpty()]
    [string]$Path = 'C:\Program Files\Sublime Text\sublime_text.exe',

    [switch]$DryRun,

    [switch]$NoRestart,

    # Never self-elevate; print the manual command instead.
    [switch]$NoElevate
)

$ErrorActionPreference = 'Stop'

$SublimeProcess = 'sublime_text'
$PatchTarget = [IO.Path]::GetFullPath($Path)
$ScriptUrl = 'https://raw.githubusercontent.com/pepestunaa/sublime-patcher/main/sublime-patcher.ps1'

$Patches = @(
    @{
        Name        = 'UI/license flag'
        Description = 'movzx/xor flag sequence -> direct set (file offset 0x46B80).'
        Original    = [byte[]](0x0F, 0xB6, 0x51, 0x05, 0x83, 0xF2, 0x01)
        Replacement = [byte[]](0xC6, 0x41, 0x05, 0x01, 0xB2, 0x00, 0x90)
    },
    @{
        Name        = 'automatic update-check override'
        Description = 'JNZ -> JMP so the forced update_check write is skipped (file offset 0x32623).'
        Original    = [byte[]](0x80, 0x78, 0x05, 0x00, 0x75, 0x07, 0xC6, 0x05, 0xB4, 0xDA, 0x6C, 0x00, 0x01)
        Replacement = [byte[]](0x80, 0x78, 0x05, 0x00, 0xEB, 0x07, 0xC6, 0x05, 0xB4, 0xDA, 0x6C, 0x00, 0x01)
    }
)

function Write-SublPatch {
    param(
        [string]$Message,
        [ConsoleColor]$Color = 'Gray'
    )
    Write-Host $Message -ForegroundColor $Color
}

function Test-SublPatchFile {
    param([string]$Candidate)
    return (Test-Path -LiteralPath $Candidate -PathType Leaf)
}

function Get-SublPatchHits {
    param(
        [byte[]]$Data,
        [byte[]]$Pattern
    )
    $hits = @()
    if ($Data.Length -lt $Pattern.Length) {
        return $hits
    }

    $hex = [BitConverter]::ToString($Data)
    $needle = [BitConverter]::ToString($Pattern)
    $index = $hex.IndexOf($needle, [StringComparison]::Ordinal)
    while ($index -ge 0) {
        $hits += [int]($index / 3)
        $index = $hex.IndexOf($needle, $index + 1, [StringComparison]::Ordinal)
    }
    return $hits
}

function Get-SublPatchChangedIndex {
    param(
        [byte[]]$Original,
        [byte[]]$Replacement
    )
    for ($i = 0; $i -lt $Original.Length; $i++) {
        if ($Original[$i] -ne $Replacement[$i]) { return $i }
    }
    return 0
}

function Get-SublPatchBackupPath {
    param([string]$TargetFile)

    $default = [IO.Path]::ChangeExtension($TargetFile, '.bak')
    if (-not (Test-Path -LiteralPath $default -PathType Leaf)) {
        return $default
    }

    $stamp = [DateTimeOffset]::UtcNow.ToUnixTimeSeconds()
    while ($true) {
        $candidate = [IO.Path]::ChangeExtension($TargetFile, ".bak.$stamp")
        if (-not (Test-Path -LiteralPath $candidate -PathType Leaf)) {
            return $candidate
        }
        $stamp++
    }
}

function Test-SublPatchWritable {
    param([string]$Directory)
    $probe = [IO.Path]::Combine($Directory, ('.__subl_bypass_write_{0}' -f [Guid]::NewGuid().ToString('N')))
    try {
        [IO.File]::WriteAllText($probe, [string]::Empty)
        return $true
    } catch {
        return $false
    } finally {
        if (Test-Path -LiteralPath $probe -PathType Leaf) {
            Remove-Item -LiteralPath $probe -Force
        }
    }
}

function Close-SublPatchProcess {
    if (-not (Get-Process -Name $SublimeProcess -ErrorAction SilentlyContinue)) {
        return $null
    }

    Write-SublPatch '[*] Closing running Sublime Text...' 'Cyan'
    & taskkill /IM "$SublimeProcess.exe" 2>$null | Out-Null

    for ($i = 0; $i -lt 10; $i++) {
        if (-not (Get-Process -Name $SublimeProcess -ErrorAction SilentlyContinue)) {
            return $true
        }
        Start-Sleep -Milliseconds 500
    }
    return $false
}

function Show-SublPatchElevationHint {
    Write-SublPatch '    Open an elevated PowerShell session and run the same one-liner again:' 'Yellow'
    if ($MyInvocation.ScriptName) {
        Write-SublPatch ("    powershell -ExecutionPolicy Bypass -File `"{0}`"" -f $MyInvocation.ScriptName) 'Yellow'
    } else {
        Write-SublPatch "    powershell -ExecutionPolicy Bypass -Command `"Set-ExecutionPolicy Bypass -Scope Process -Force; irm '$ScriptUrl' | iex`"" 'Yellow'
    }
}

function Test-SublPatchElevated {
    $identity = [Security.Principal.WindowsIdentity]::GetCurrent()
    $principal = New-Object Security.Principal.WindowsPrincipal($identity)
    return $principal.IsInRole([Security.Principal.WindowsBuiltInRole]::Administrator)
}

function Get-SublPatchSelfPath {
    if ($MyInvocation.ScriptName -and (Test-Path -LiteralPath $MyInvocation.ScriptName -PathType Leaf)) {
        return (Get-Item -LiteralPath $MyInvocation.ScriptName).FullName
    }
    # Invoked via irm|iex: materialize a temp copy the elevated child can execute.
    $temp = Join-Path $env:TEMP 'sublime-patcher-self.ps1'
    Invoke-WebRequest -Uri $ScriptUrl -OutFile $temp -UseBasicParsing
    return $temp
}

function Start-SublPatchElevation {
    param(
        [string]$ChildScript,
        [string]$TargetPath,
        [switch]$DryRun,
        [switch]$NoRestart
    )
    try {
        $arguments = @('-NoProfile','-ExecutionPolicy','Bypass','-File', ('"{0}"' -f $ChildScript), '-NoElevate')
        if ($DryRun)      { $arguments += '-DryRun' }
        if ($NoRestart)   { $arguments += '-NoRestart' }
        if ($TargetPath)  { $arguments += @('-Path', ('"{0}"' -f $TargetPath)) }

        Write-SublPatch '[*] Relaunching as Administrator. Accept the UAC prompt to continue...' 'Yellow'
        Start-Process -FilePath 'powershell.exe' -Verb RunAs -Wait -ArgumentList $arguments | Out-Null
        return $true
    } catch {
        Write-SublPatch ("[-] Could not self-elevate: {0}" -f $_.Exception.Message) 'Red'
        return $false
    }
}

if (-not (Test-SublPatchFile $PatchTarget)) {
    Write-SublPatch ('[-] Sublime Text executable not found: {0}' -f $PatchTarget) 'Red'
    Write-SublPatch '    Install Sublime Text 4 first, or pass -Path to a copy.' 'Red'
    if ($DryRun) { Show-SublPatchElevationHint }
    exit 1
}

Write-SublPatch 'Sublime Text 4 build 4200 x64 patcher' 'Cyan'
Write-SublPatch ('[*] Target: {0}' -f $PatchTarget) 'Cyan'
$bytes = [IO.File]::ReadAllBytes($PatchTarget)
Write-SublPatch ('[*] Size:   {0} bytes' -f $bytes.Length) 'Cyan'

if (-not $DryRun) {
    $writable = Test-SublPatchWritable (Split-Path -Parent $PatchTarget)
    if (-not $writable) {
        if (Test-SublPatchElevated) {
            Write-SublPatch ('[-] Elevated session still cannot write to: {0}' -f (Split-Path -Parent $PatchTarget)) 'Red'
            exit 1
        }
        if (-not $NoElevate) {
            try {
                $self = Get-SublPatchSelfPath
                if (Start-SublPatchElevation -ChildScript $self -TargetPath $PatchTarget -DryRun:$DryRun -NoRestart:$NoRestart) {
                    exit 0
                }
            } catch {
                Write-SublPatch ("[-] Could not fetch a self copy for elevation: {0}" -f $_.Exception.Message) 'Red'
            }
        }
        Write-SublPatch ('[-] Directory is not writable: {0}' -f (Split-Path -Parent $PatchTarget)) 'Red'
        Show-SublPatchElevationHint
        exit 1
    }
}

$closedProcess = $false
if (-not $DryRun) {
    $closedProcess = Close-SublPatchProcess
    if ($null -ne $closedProcess -and -not $closedProcess) {
        Write-SublPatch '[-] Sublime Text is still running. Close it manually and run this again.' 'Red'
        exit 1
    }
}

# Work entirely in memory first; no file is touched until every patch checks out.
$working = [byte[]]::new($bytes.Length)
[Array]::Copy($bytes, $working, $bytes.Length)

$failed = $false
$pending = 0
foreach ($patch in $Patches) {
    $originalHits = @(Get-SublPatchHits -Data $working -Pattern $patch.Original)
    $replacementHits = @(Get-SublPatchHits -Data $working -Pattern $patch.Replacement)

    switch ($true) {
        ($originalHits.Count -eq 1 -and $replacementHits.Count -eq 0) {
            $changedIndex = Get-SublPatchChangedIndex $patch.Original $patch.Replacement
            $offset = $originalHits[0] + $changedIndex
            [Array]::Copy($patch.Replacement, 0, $working, $originalHits[0], $patch.Replacement.Length)
            $pending++
            Write-SublPatch ('[+] {0}: {1}' -f $patch.Name, $patch.Description) 'Green'
            if ($DryRun) {
                Write-SublPatch ('[+]   Would patch byte at 0x{0:X}' -f $offset) 'Green'
            } else {
                Write-SublPatch ('[+]   Patched byte at 0x{0:X}' -f $offset) 'Green'
            }
        }
        ($originalHits.Count -eq 0 -and $replacementHits.Count -eq 1) {
            Write-SublPatch ('[=] {0}: already patched' -f $patch.Name) 'DarkGray'
        }
        ($originalHits.Count -eq 0 -and $replacementHits.Count -eq 0) {
            Write-SublPatch ('[-] {0}: signature not found (unsupported build or wrong file)' -f $patch.Name) 'Red'
            $failed = $true
        }
        default {
            Write-SublPatch ('[-] {0}: ambiguous signature ({1} original / {2} replacement matches); refusing to patch' -f $patch.Name, $originalHits.Count, $replacementHits.Count) 'Red'
            $failed = $true
        }
    }
}

if ($failed) {
    Write-SublPatch '[-] Cannot patch this binary; no changes were written.' 'Red'
    exit 1
}

if ($pending -eq 0) {
    Write-SublPatch '[*] Nothing to do: all patches are already applied.' 'Cyan'
    exit 0
}

if ($DryRun) {
    Write-SublPatch ('[*] Dry run complete. {0} patch(es) would be written.' -f $pending) 'Yellow'
    exit 0
}

$backup = Get-SublPatchBackupPath $PatchTarget
Write-SublPatch ('[+] Backup:  {0}' -f $backup) 'Cyan'
[IO.File]::WriteAllBytes($backup, $bytes)

try {
    [IO.File]::WriteAllBytes($PatchTarget, $working)
} catch {
    Write-SublPatch ('[-] Could not write {0}: {1}' -f $PatchTarget, $_.Exception.Message) 'Red'
    Show-SublPatchElevationHint
    exit 1
}

# Re-read from disk; don't trust in-memory state alone.
$verify = [IO.File]::ReadAllBytes($PatchTarget)
foreach ($patch in $Patches) {
    if (@(Get-SublPatchHits -Data $verify -Pattern $patch.Replacement).Count -ne 1 -or
        @(Get-SublPatchHits -Data $verify -Pattern $patch.Original).Count -ne 0) {
        [IO.File]::WriteAllBytes($PatchTarget, $bytes)
        Write-SublPatch ('[-] Verification failed for {0}; original bytes restored from disk.' -f $patch.Name) 'Red'
        exit 1
    }
}

Write-SublPatch '[+] Patched and verified.' 'Green'

if ($closedProcess -and -not $NoRestart) {
    Write-SublPatch '[*] Relaunching Sublime Text...' 'Cyan'
    Start-Process -FilePath $PatchTarget | Out-Null
}
