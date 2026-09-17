use std::fs::{self, File};
use std::io::{self, Read, Write};
use std::path::PathBuf;
use std::process::Command;

fn main() -> io::Result<()> {
    close_sublime();
    let default_path = if cfg!(target_os = "windows") {
        r"C:\Program Files\Sublime Text\sublime_text.exe"
    } else if cfg!(target_os = "macos") {
        "/Applications/Sublime Text.app/Contents/MacOS/sublime_text"
    } else {
        "/opt/sublime_text/sublime_text"
    };

    println!("[*] Default path for your OS: {}", default_path);
    println!("[*] Press ENTER to use default, or paste a custom path below:");
    print!("> ");
    io::stdout().flush()?; 

    let mut user_input = String::new();
    io::stdin().read_line(&mut user_input)?;

    let cleaned_input = user_input.trim();

    let final_path = if cleaned_input.is_empty() {
        PathBuf::from(default_path)
    } else {
        let unquoted = cleaned_input.trim_matches(|c| c == '"' || c == '\'');
        PathBuf::from(unquoted)
    };

    if !final_path.exists() {
        println!("\n[-] Error: Could not find the file at: {:?}", final_path);
        println!("    Please restart the program and double-check your path input.");
        return Ok(());
    }

    let backup_path = final_path.with_extension("bak");
    println!("\n[+] Creating a safety backup at: {:?}", backup_path);
    fs::copy(&final_path, &backup_path)?;

    let mut file = File::open(&final_path)?;
    let mut buffer = Vec::new();
    file.read_to_end(&mut buffer)?;

    let search_pattern: [u8; 7] = [0x0F, 0xB6, 0x51, 0x05, 0x83, 0xF2, 0x01];
    let patch_bytes: [u8; 7]     = [0xC6, 0x41, 0x05, 0x01, 0xB2, 0x00, 0x90];

    let mut found = false;
    
    for i in 0..=(buffer.len() - search_pattern.len()) {
        if buffer[i..(i + search_pattern.len())] == search_pattern {
            println!("[+] Target sequence found at byte offset: 0x{:X}", i);
            
            buffer[i..(i + patch_bytes.len())].copy_from_slice(&patch_bytes);
            found = true;
            break; 
        }
    }

    if found {
        let mut out_file = File::create(&final_path)?;
        out_file.write_all(&buffer)?;
        println!("[+] Success: File patched successfully!");
    } else {
        println!("[-] Error: Could not find the correct Build 4200 signature.");
        println!("    The file might already be patched or it is a different version.");
    }

    #[cfg(target_os = "windows")]
    let _ = Command::new(r"C:\Program Files\Sublime Text\sublime_text.exe")
        .arg("--command")
        .arg("about")
        .spawn();

    #[cfg(any(target_os = "linux", target_os = "macos"))]
    let _ = Command::new("subl")
        .arg("--command")
        .arg("about")
        .spawn();


    Ok(())
}

fn close_sublime() {
    // Windows: Forcefully kill the process by image name
    #[cfg(target_os = "windows")]
    let _ = Command::new("taskkill")
        .args(["/F", "/IM", "sublime_text.exe"])
        .output();

    // macOS/Linux: Kill the process by matching the full command line
    #[cfg(any(target_os = "linux", target_os = "macos"))]
    let _ = Command::new("pkill")
        .args(["-f", "sublime_text"])
        .output();
    
    println!("[+] Attempted to close Sublime Text to avoid file locks.");
}


// just testing a sublime merge commit to trigered popup from license
