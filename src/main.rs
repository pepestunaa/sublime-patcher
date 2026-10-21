mod bytepatch;
mod ops;
mod patches;
mod target;

use std::env;
use std::fs;
use std::process::ExitCode;

use bytepatch::{PatchOutcome, apply_patch};
use ops::{close_sublime, make_backup, restart_sublime, verify_written_file};
use patches::{BUILD, PATCHES};
use target::{Options, resolve_target};

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
    let options = Options::parse(env::args().skip(1))?;
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
                println!(
                    "[+] {}: patched byte at 0x{offset:X} ({})",
                    patch.name, patch.description
                );
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
            "could not write {}: {error} (run as Administrator, or re-launch from sublime-patcher.ps1 which asks for elevation automatically)",
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
