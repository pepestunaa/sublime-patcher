use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use crate::patches::PATCHES;

pub fn make_backup(target: &Path) -> Result<PathBuf, String> {
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

pub fn available_backup_path(target: &Path) -> PathBuf {
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

pub fn verify_written_file(target: &Path) -> Result<(), String> {
    let data = fs::read(target)
        .map_err(|error| format!("could not verify {}: {error}", target.display()))?;

    for patch in PATCHES {
        if crate::bytepatch::find_all(&data, patch.replacement).len() != 1 {
            return Err(format!(
                "verification failed for {}; restore from the backup",
                patch.name
            ));
        }
    }

    Ok(())
}

#[cfg(target_os = "windows")]
pub fn close_sublime() -> Result<(), String> {
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
pub fn close_sublime() -> Result<(), String> {
    let _ = Command::new("pkill").args(["-f", "sublime_text"]).output();
    std::thread::sleep(Duration::from_millis(1500));
    println!("[+] Requested Sublime Text shutdown.");
    Ok(())
}

pub fn restart_sublime(target: &Path) {
    #[cfg(target_os = "windows")]
    {
        let _ = Command::new(target).args(["--command", "about"]).spawn();
    }

    #[cfg(any(target_os = "linux", target_os = "macos"))]
    {
        let _ = Command::new("subl").args(["--command", "about"]).spawn();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::env;
    use std::fs;

    #[test]
    fn backup_path_does_not_overwrite_existing_backup() {
        let directory =
            env::temp_dir().join(format!("sublime-patcher-test-{}", std::process::id()));
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
