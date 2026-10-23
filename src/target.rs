use std::io::{self, Write};
use std::path::PathBuf;

use crate::patches::BUILD;

#[derive(Default)]
pub struct Options {
    pub path: Option<PathBuf>,
    pub dry_run: bool,
    pub no_close: bool,
    pub no_restart: bool,
}

impl Options {
    pub fn parse(args: impl Iterator<Item = String>) -> Result<Self, String> {
        let mut options = Self::default();

        for argument in args {
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

pub fn resolve_target(options: &Options) -> Result<PathBuf, String> {
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

pub fn default_sublime_path() -> &'static str {
    if cfg!(target_os = "windows") {
        r"C:\Program Files\Sublime Text\sublime_text.exe"
    } else if cfg!(target_os = "macos") {
        "/Applications/Sublime Text.app/Contents/MacOS/sublime_text"
    } else {
        "/opt/sublime_text/sublime_text"
    }
}

pub fn print_help() {
    println!(
        "Sublime Text {BUILD} patcher

Usage:
  sublime-patcher [PATH] [OPTIONS]

Options:
  --dry-run       Report patch results without writing anything
  --no-close      Do not close Sublime Text before writing
  --no-restart    Do not launch Sublime Text after patching
  --help, -h      Show this help

Patches:
  1. UI/license flag (build {BUILD})
  2. Automatic update-check override (build {BUILD})
  3. Update-check startup gate (build {BUILD})"
    );
}

#[cfg(test)]
mod tests {
    use super::*;

    fn args(values: &[&str]) -> impl Iterator<Item = String> {
        values.iter().map(|value| value.to_string())
    }

    #[test]
    fn parses_flags_and_positional_path() {
        let options = Options::parse(args(&[
            "--dry-run",
            "C:\\Program Files\\Sublime Text\\sublime_text.exe",
            "--no-restart",
        ]))
        .unwrap();

        assert!(options.dry_run);
        assert!(options.no_restart);
        assert!(!options.no_close);
        assert_eq!(
            options.path.unwrap().to_string_lossy(),
            r"C:\Program Files\Sublime Text\sublime_text.exe"
        );
    }

    #[test]
    fn parses_path_and_other_flags_before_path() {
        let options =
            Options::parse(args(&["--no-close", "--no-restart", "C:\\tmp\\s.exe"])).unwrap();
        assert!(options.no_close && options.no_restart);
        assert_eq!(options.path.unwrap().to_string_lossy(), r"C:\tmp\s.exe");
    }

    #[test]
    fn rejects_unknown_option() {
        assert!(Options::parse(args(&["--bogus"])).is_err());
    }

    #[test]
    fn rejects_multiple_paths() {
        assert!(Options::parse(args(&["a.exe", "b.exe"])).is_err());
    }

    #[test]
    fn supports_windows_and_unix_style_positional_values() {
        // Shim to make sure Windows path handling and POSIX style both resolve
        // to the same field when passed positionally.
        let windows_style = Options::parse(args(&["C:\\some\\path.exe"])).unwrap();
        let unix_style = Options::parse(args(&["/some/path"])).unwrap();
        assert!(windows_style.path.is_some());
        assert!(unix_style.path.is_some());
    }
}
