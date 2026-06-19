use std::fs;
use std::os::unix::fs::PermissionsExt;
use std::path::Path;
use std::process::Stdio;
use tokio::process::Command;

pub async fn ensure_binaries() {
    let rpxy_exists = is_command_available("rpxy") || Path::new("./.weaver-bin/rpxy").exists();
    let rpxy_l4_exists = is_command_available("rpxy-l4") || Path::new("./.weaver-bin/rpxy-l4").exists();

    if rpxy_exists && rpxy_l4_exists {
        return;
    }

    println!(">>> Proxies not found in $PATH. Initializing auto-downloader...");
    fs::create_dir_all("./.weaver-bin").expect("Failed to create local bin directory");

    let is_nix = Path::new("/etc/nixos").exists() || Path::new("/nix/store").exists();
    let mut preferred_target = if is_nix {
        "x86_64-unknown-linux-musl"
    } else {
        "x86_64-unknown-linux-gnu"
    };

    if !rpxy_exists {
        println!(">>> Resolving latest release for rpxy via GitHub API...");
        let url = match get_download_url("junkurihara/rust-rpxy", preferred_target).await {
            Some(u) => u,
            None => {
                println!("!!! No standard {} build found for rpxy. Falling back to gnu...", preferred_target);
                preferred_target = "x86_64-unknown-linux-gnu";
                get_download_url("junkurihara/rust-rpxy", preferred_target).await
                    .expect("Failed to find ANY compatible rpxy release asset on GitHub.")
            }
        };
        println!(">>> Downloading: {}", url);
        download_and_extract(&url, "rpxy").await;
    }

    if !rpxy_l4_exists {
        println!(">>> Resolving latest release for rpxy-l4 via GitHub API...");
        let url = match get_download_url("junkurihara/rust-rpxy-l4", preferred_target).await {
            Some(u) => u,
            None => {
                println!("!!! No standard {} build found for rpxy-l4. Falling back to gnu...", preferred_target);
                get_download_url("junkurihara/rust-rpxy-l4", "x86_64-unknown-linux-gnu").await
                    .expect("Failed to find ANY compatible rpxy-l4 release asset on GitHub.")
            }
        };
        println!(">>> Downloading: {}", url);
        download_and_extract(&url, "rpxy-l4").await;
    }
}

async fn get_download_url(repo: &str, target_triple: &str) -> Option<String> {
    let api_url = format!("https://api.github.com/repos/{}/releases/latest", repo);
    
    let output = Command::new("curl")
        .args(&["-sSL", "-H", "User-Agent: rpxy-weaver", &api_url])
        .output()
        .await
        .expect("Failed to fetch GitHub API");
        
    let json_str = String::from_utf8_lossy(&output.stdout);
    let mut fallback_url = None;
    
    for line in json_str.split(',') {
        if line.contains("\"browser_download_url\"") && line.contains(target_triple) && line.contains(".tar.gz") {
            let parts: Vec<&str> = line.split('"').collect();
            for part in parts {
                if part.starts_with("https://") {
                    // Try to avoid feature-specific builds like s2n or webpki-roots if a standard one exists
                    if part.contains("webpki") || part.contains("s2n") {
                        fallback_url = Some(part.to_string());
                    } else {
                        return Some(part.to_string());
                    }
                }
            }
        }
    }
    fallback_url
}

async fn download_and_extract(url: &str, bin_name: &str) {
    let tmp_dir = format!("./.weaver-bin/tmp_{}", bin_name);
    let _ = fs::remove_dir_all(&tmp_dir); // Ensure clean extraction state
    fs::create_dir_all(&tmp_dir).expect("Failed to create tmp dir");

    let mut curl = Command::new("curl")
        .args(&["-sSLf", url])
        .stdout(Stdio::piped())
        .spawn()
        .expect("Failed to spawn curl.");

    let curl_stdout = curl.stdout.take().expect("Failed to open curl stdout");
    let tar_stdin: std::process::Stdio = curl_stdout
        .try_into()
        .expect("Failed to convert Tokio stdout to standard Stdio");

    let tar = Command::new("tar")
        .args(&["-xz", "-C", &tmp_dir])
        .stdin(tar_stdin)
        .status()
        .await
        .expect("Failed to extract tar archive");

    let curl_status = curl.wait().await.expect("Failed to wait for curl");

    if !curl_status.success() || !tar.success() {
        panic!("Failed to download or extract {}. The URL might be invalid: {}", bin_name, url);
    }

    // Bypass naming conventions entirely by just grabbing the largest non-doc file
    let extracted_bin = find_largest_file(&tmp_dir)
        .expect(&format!("Could not find any executable file in the downloaded archive for {}", bin_name));
        
    let final_path = format!("./.weaver-bin/{}", bin_name);
    fs::rename(&extracted_bin, &final_path).expect("Failed to move binary");
    
    // Explicitly chmod +x the binary to prevent execution panics
    if let Ok(mut perms) = fs::metadata(&final_path).map(|m| m.permissions()) {
        perms.set_mode(0o755);
        let _ = fs::set_permissions(&final_path, perms);
    }

    fs::remove_dir_all(&tmp_dir).expect("Failed to clean up tmp dir");
    
    println!(">>> Successfully installed {}", bin_name);
}

fn find_largest_file(dir: &str) -> Option<std::path::PathBuf> {
    let mut largest_file = None;
    let mut max_size = 0;

    fn search(current_dir: &str, largest_file: &mut Option<std::path::PathBuf>, max_size: &mut u64) {
        if let Ok(entries) = fs::read_dir(current_dir) {
            for entry in entries.flatten() {
                let path = entry.path();
                if path.is_dir() {
                    search(path.to_str().unwrap(), largest_file, max_size);
                } else {
                    let file_name = path.file_name().unwrap().to_string_lossy().to_lowercase();
                    // Ignore typical text, config, and markdown files
                    if !file_name.ends_with(".md") && !file_name.ends_with(".txt") && !file_name.contains("license") {
                        if let Ok(metadata) = fs::metadata(&path) {
                            if metadata.len() > *max_size {
                                *max_size = metadata.len();
                                *largest_file = Some(path.clone());
                            }
                        }
                    }
                }
            }
        }
    }

    search(dir, &mut largest_file, &mut max_size);
    largest_file
}

fn is_command_available(cmd: &str) -> bool {
    std::process::Command::new("which")
        .arg(cmd)
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
        .map(|status| status.success())
        .unwrap_or(false)
}
