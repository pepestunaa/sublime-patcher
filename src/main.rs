use std::env;
use std::fs;
use std::io::{self, Write};
use std::path::{Path, PathBuf};
use std::process::{Command, ExitCode};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

const BUILD: &str = "4200";

struct Patch {
    name: &'static str,
    original: &'static [u8],
    replacement: &'static [u8],
}

const PATCHES: &[Patch] = &[
    Patch {
        name: "UI/license flag",
        // FILE OFFSET 0x46B80 (VA 0x140047780)
        // 0F B6 51 05   movzx edx, byte ptr [rcx+5]
        // 83 F2 01      xor   edx, 1
        original: &[0x0F, 0xB6, 0x51, 0x05, 0x83, 0xF2, 0x01],
        replacement: &[0xC6, 0x41, 0x05, 0x01, 0xB2, 0x00, 0x90],
    },
    Patch {
        name: "automatic update-check override",
        // FILE OFFSET 0x3261F (VA 0x14003321F)
        // 80 78 05 00        cmp byte ptr [rax+5], 0
        // 75 07              jnz +7                 <- byte at 0x32623
        // C6 05 B4 DA 6C 00 01  mov byte ptr [0x140700CE0], 1
        original: &[
            0x80, 0x78, 0x05, 0x00, 0x75, 0x07, 0xC6, 0x05, 0xB4, 0xDA, 0x6C, 0x00, 0x01,
        ],
        replacement: &[
            0x80, 0x78, 0x05, 0x00, 0xEB, 0x07, 0xC6, 0x05, 0xB4, 0xDA, 0x6C, 0x00, 0x01,
        ],
    },
];

#[derive(Debug, PartialEq, Eq)]
enum PatchOutcome {
    Applied(usize),
    AlreadyApplied,
    NotFound,
    Ambiguous(usize),
}

#[derive(Default)]
struct Options {
    path: Option<PathBuf>,
    dry_run: bool,
    no_close: bool,
    no_restart: bool,
}

fn main() -> ExitCode {
    match run() {
        Ok(()) => ExitCode::SUCCESS,
        Err(error) => {
            eprintln!("[-] Error: {error}");
            ExitCode::FAILURE
        }
    }
}

fn run() -> Result<(), String> {
    let options = Options::parse()?;
    println!("Sublime Text {BUILD} patcher");

    let target = resolve_target(&options)?;
    let mut data = fs::read(&target)
        .map_err(|error| format!("could not read {}: {error}", target.display()))?;

    println!("[*] Target: {}", target.display());
    println!("[*] Size: {} bytes", data.len());

    let mut failed = false;
    let mut changed = false;

    for patch in PATCHES {
        match apply_patch(&mut data, patch) {
            PatchOutcome::Applied(offset) => {
                println!("[+] {}: patched byte at 0x{offset:X}", patch.name);
                changed = true;
            }
            PatchOutcome::AlreadyApplied => {
                println!("[=] {}: already patched", patch.name);
            }
            PatchOutcome::NotFound => {
                println!(
                    "[-] {}: signature not found (unsupported build or unexpected file)",
                    patch.name
                );
                failed = true;
            }
            PatchOutcome::Ambiguous(count) => {
                println!(
                    "[-] {}: signature is ambiguous ({count} matches); refusing to patch",
                    patch.name
                );
                failed = true;
            }
        }
    }

    if failed {
        return Err("one or more patches could not be applied; no changes were written".into());
    }

    if !changed {
        println!("[*] Nothing to do.");
        return Ok(());
    }

    if options.dry_run {
        println!("[*] Dry run: no changes were written.");
        return Ok(());
    }

    if !options.no_close {
        close_sublime()?;
    }

    let backup = make_backup(&target)?;
    println!("[+] Backup: {}", backup.display());

    fs::write(&target, &data).map_err(|error| {
        format!(
            "could not write {}: {error} (run as Administrator)",
            target.display()
        )
    })?;

    verify_written_file(&target)?;
    println!("[+] Success: file patched and verified.");

    if !options.no_restart {
        restart_sublime(&target);
    }

    Ok(())
}

impl Options {
    fn parse() -> Result<Self, String> {
        let mut options = Self::default();

        for argument in env::args().skip(1) {
            match argument.as_str() {
                "--dry-run" => options.dry_run = true,
                "--no-close" => options.no_close = true,
                "--no-restart" => options.no_restart = true,
                "--help" | "-h" => {
                    print_help();
                    std::process::exit(0);
                }
                other if other.starts_with('-') => {
                    return Err(format!("unknown option: {other}"));
                }
                other => {
                    if options.path.is_some() {
                        return Err("only one target path may be supplied".into());
                    }
                    options.path = Some(PathBuf::from(other));
                }
            }
        }

        Ok(options)
    }
}

fn resolve_target(options: &Options) -> Result<PathBuf, String> {
    let path = match &options.path {
        Some(path) => path.clone(),
        None => {
            let default_path = default_sublime_path();
            println!("[*] Default path for your OS: {default_path}");
            println!("[*] Press ENTER to use the default, or paste a custom path:");
            print!("> ");
            io::stdout()
                .flush()
                .map_err(|error| format!("could not write prompt: {error}"))?;

            let mut input = String::new();
            io::stdin()
                .read_line(&mut input)
                .map_err(|error| format!("could not read input: {error}"))?;

            let input = input.trim();
            if input.is_empty() {
                PathBuf::from(default_path)
            } else {
                PathBuf::from(input.trim_matches(|character| character == '"' || character == '\''))
            }
        }
    };

    if !path.is_file() {
        return Err(format!("file not found: {}", path.display()));
    }

    Ok(path)
}

fn default_sublime_path() -> &'static str {
    if cfg!(target_os = "windows") {
        r"C:\Program Files\Sublime Text\sublime_text.exe"
    } else if cfg!(target_os = "macos") {
        "/Applications/Sublime Text.app/Contents/MacOS/sublime_text"
    } else {
        "/opt/sublime_text/sublime_text"
    }
}

fn apply_patch(buffer: &mut [u8], patch: &Patch) -> PatchOutcome {
    debug_assert_eq!(patch.original.len(), patch.replacement.len());

    let original_matches = find_all(buffer, patch.original);
    let replacement_matches = find_all(buffer, patch.replacement);

    match (original_matches.len(), replacement_matches.len()) {
        (1, 0) => {
            let start = original_matches[0];
            let changed_index = patch
                .original
                .iter()
                .zip(patch.replacement)
                .position(|(original, replacement)| original != replacement)
                .unwrap_or(0);

            buffer[start..start + patch.replacement.len()].copy_from_slice(patch.replacement);
            PatchOutcome::Applied(start + changed_index)
        }
        (0, 1) => PatchOutcome::AlreadyApplied,
        (0, 0) => PatchOutcome::NotFound,
        (count, _) => PatchOutcome::Ambiguous(count),
    }
}

fn find_all(haystack: &[u8], needle: &[u8]) -> Vec<usize> {
    if needle.is_empty() || haystack.len() < needle.len() {
        return Vec::new();
    }

    haystack
        .windows(needle.len())
        .enumerate()
        .filter_map(|(index, window)| (window == needle).then_some(index))
        .collect()
}

fn make_backup(target: &Path) -> Result<PathBuf, String> {
    let backup = available_backup_path(target);

    fs::copy(target, &backup)
        .map_err(|error| format!("could not create backup {}: {error}", backup.display()))?;

    let original = fs::read(target)
        .map_err(|error| format!("could not re-read {}: {error}", target.display()))?;
    let copy = fs::read(&backup)
        .map_err(|error| format!("could not read backup {}: {error}", backup.display()))?;

    if original != copy {
        return Err(format!("backup verification failed: {}", backup.display()));
    }

    Ok(backup)
}

fn available_backup_path(target: &Path) -> PathBuf {
    let default_backup = target.with_extension("bak");
    if !default_backup.exists() {
        return default_backup;
    }

    let mut timestamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs();

    loop {
        let backup = target.with_extension(format!("bak.{timestamp}"));
        if !backup.exists() {
            return backup;
        }
        timestamp += 1;
    }
}

fn verify_written_file(target: &Path) -> Result<(), String> {
    let data = fs::read(target)
        .map_err(|error| format!("could not verify {}: {error}", target.display()))?;

    for patch in PATCHES {
        if find_all(&data, patch.replacement).len() != 1 {
            return Err(format!(
                "verification failed for {}; restore from the backup",
                patch.name
            ));
        }
    }

    Ok(())
}

#[cfg(target_os = "windows")]
fn close_sublime() -> Result<(), String> {
    let _ = Command::new("taskkill")
        .args(["/IM", "sublime_text.exe"])
        .output();

    std::thread::sleep(Duration::from_millis(1500));

    let output = Command::new("tasklist")
        .args(["/FI", "IMAGENAME eq sublime_text.exe", "/NH"])
        .output()
        .map_err(|error| format!("could not check for running Sublime Text: {error}"))?;

    let text = String::from_utf8_lossy(&output.stdout).to_ascii_lowercase();
    if text.contains("sublime_text.exe") {
        return Err(
            "Sublime Text is still running; close it and retry, or pass --no-close after closing it yourself"
                .into(),
        );
    }

    println!("[+] Closed Sublime Text.");
    Ok(())
}

#[cfg(not(target_os = "windows"))]
fn close_sublime() -> Result<(), String> {
    let _ = Command::new("pkill").args(["-f", "sublime_text"]).output();
    std::thread::sleep(Duration::from_millis(1500));
    println!("[+] Requested Sublime Text shutdown.");
    Ok(())
}

fn restart_sublime(target: &Path) {
    #[cfg(target_os = "windows")]
    {
        let _ = Command::new(target).args(["--command", "about"]).spawn();
    }

    #[cfg(any(target_os = "linux", target_os = "macos"))]
    {
        let _ = Command::new("subl").args(["--command", "about"]).spawn();
    }
}

fn print_help() {
    println!(
        "Sublime Text {BUILD} patcher

Usage:
  subl-bypass [PATH] [OPTIONS]

Options:
  --dry-run       Report patch results without writing anything
  --no-close      Do not close Sublime Text before writing
  --no-restart    Do not launch Sublime Text after patching
  --help, -h      Show this help

Patches:
  1. UI/license flag (build {BUILD})
  2. Automatic update-check override (build {BUILD})"
    );
}

#[cfg(test)]
mod tests {
    use super::*;

    fn synthetic_image() -> Vec<u8> {
        let mut image = vec![0u8; 96];
        image[8..8 + PATCHES[0].original.len()].copy_from_slice(PATCHES[0].original);
        image[48..48 + PATCHES[1].original.len()].copy_from_slice(PATCHES[1].original);
        image
    }

    #[test]
    fn applies_both_patches() {
        let mut image = synthetic_image();

        assert_eq!(
            apply_patch(&mut image, &PATCHES[0]),
            PatchOutcome::Applied(8)
        );
        assert_eq!(
            apply_patch(&mut image, &PATCHES[1]),
            PatchOutcome::Applied(52)
        );
        assert_eq!(
            &image[8..8 + PATCHES[0].replacement.len()],
            PATCHES[0].replacement
        );
        assert_eq!(
            &image[48..48 + PATCHES[1].replacement.len()],
            PATCHES[1].replacement
        );
    }

    #[test]
    fn detects_already_applied_patches() {
        let mut image = synthetic_image();
        apply_patch(&mut image, &PATCHES[0]);
        apply_patch(&mut image, &PATCHES[1]);

        assert_eq!(
            apply_patch(&mut image, &PATCHES[0]),
            PatchOutcome::AlreadyApplied
        );
        assert_eq!(
            apply_patch(&mut image, &PATCHES[1]),
            PatchOutcome::AlreadyApplied
        );
    }

    #[test]
    fn reports_missing_signature() {
        let mut image = vec![0u8; 64];
        assert_eq!(apply_patch(&mut image, &PATCHES[1]), PatchOutcome::NotFound);
    }

    #[test]
    fn rejects_ambiguous_signature() {
        let mut image = vec![0u8; 128];
        image[0..PATCHES[1].original.len()].copy_from_slice(PATCHES[1].original);
        image[64..64 + PATCHES[1].original.len()].copy_from_slice(PATCHES[1].original);

        assert_eq!(
            apply_patch(&mut image, &PATCHES[1]),
            PatchOutcome::Ambiguous(2)
        );
    }

    #[test]
    fn backup_path_does_not_overwrite_existing_backup() {
        let directory = env::temp_dir().join(format!("subl-bypass-test-{}", std::process::id()));
        let _ = fs::remove_dir_all(&directory);
        fs::create_dir_all(&directory).unwrap();

        let target = directory.join("sublime_text.exe");
        fs::write(&target, b"patched").unwrap();
        fs::write(target.with_extension("bak"), b"pristine").unwrap();

        let backup = available_backup_path(&target);
        assert_ne!(backup, target.with_extension("bak"));
        assert!(backup.to_string_lossy().contains("sublime_text.bak."));

        fs::remove_dir_all(&directory).unwrap();
    }
}
