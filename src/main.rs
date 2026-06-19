mod config;
mod downloader;
mod generator;

use std::fs;
use std::path::Path;
use std::process::Stdio;
use tokio::process::Command;
use config::WeaverConfig;

#[tokio::main]
async fn main() {
    println!(">>> Starting rpxy-weaver: The Double-Proxy Automator");

    // 1. Ensure binaries are installed (Auto-download if missing)
    downloader::ensure_binaries().await;

    // 2. Read Unified Config
    let config_str = fs::read_to_string("weaver.toml").expect("Failed to read weaver.toml");
    let mut config: WeaverConfig = toml::from_str(&config_str).expect("Failed to parse weaver.toml");

    // Override bin paths if they were downloaded locally
    if Path::new("./.weaver-bin/rpxy").exists() {
        config.rpxy_bin = "./.weaver-bin/rpxy".to_string();
    }
    if Path::new("./.weaver-bin/rpxy-l4").exists() {
        config.rpxy_l4_bin = "./.weaver-bin/rpxy-l4".to_string();
    }

    // 3. Generate configuration for rpxy-l4
    let rpxy_l4_toml = generator::generate_l4_config(&config);
    fs::write("rpxy-l4-generated.toml", rpxy_l4_toml).expect("Failed to write rpxy-l4 config");
    println!(">>> Generated rpxy-l4-generated.toml");

    // 4. Generate configuration for rpxy
    let rpxy_toml = generator::generate_l7_config(&config);
    fs::write("rpxy-generated.toml", rpxy_toml).expect("Failed to write rpxy config");
    println!(">>> Generated rpxy-generated.toml");

    // 5. Spawn both processes
    println!(">>> Booting proxies on bare metal...");
    
    let mut l4_process = Command::new(&config.rpxy_l4_bin)
        .arg("--config")
        .arg("rpxy-l4-generated.toml")
        .stdout(Stdio::inherit())
        .stderr(Stdio::inherit())
        .spawn()
        .unwrap_or_else(|e| panic!("Failed to start '{}'. Error: {}", config.rpxy_l4_bin, e));

    let mut l7_process = Command::new(&config.rpxy_bin)
        .arg("--config")
        .arg("rpxy-generated.toml")
        .stdout(Stdio::inherit())
        .stderr(Stdio::inherit())
        .spawn()
        .unwrap_or_else(|e| panic!("Failed to start '{}'. Error: {}", config.rpxy_bin, e));

    // 6. Monitor and keep alive
    tokio::select! {
        status = l4_process.wait() => {
            println!("!!! rpxy-l4 exited with status: {:?}", status);
        }
        status = l7_process.wait() => {
            println!("!!! rpxy exited with status: {:?}", status);
        }
    }
}
