use serde::Deserialize;
use std::fs;
use std::process::Stdio;
use tokio::process::Command;

#[derive(Deserialize, Debug)]
struct WeaverConfig {
    rpxy_bin: String,
    rpxy_l4_bin: String,
    admin_email: String,
    public_http_port: u16,
    public_https_port: u16,
    internal_http_port: u16,
    internal_https_port: u16,
    apps: Vec<AppConfig>,
}

#[derive(Deserialize, Debug)]
struct AppConfig {
    domain: String,
    backend: String,
    tls: bool,
}

#[tokio::main]
async fn main() {
    println!(">>> Starting rpxy-weaver: The Double-Proxy Automator");

    // 1. Read Unified Config
    let config_str = fs::read_to_string("weaver.toml").expect("Failed to read weaver.toml");
    let config: WeaverConfig = toml::from_str(&config_str).expect("Failed to parse weaver.toml");

    // 2. Generate configuration for rpxy-l4
    let rpxy_l4_toml = generate_l4_config(&config);
    fs::write("rpxy-l4-generated.toml", rpxy_l4_toml).expect("Failed to write rpxy-l4 config");
    println!(">>> Generated rpxy-l4-generated.toml");

    // 3. Generate configuration for rpxy
    let rpxy_toml = generate_l7_config(&config);
    fs::write("rpxy-generated.toml", rpxy_toml).expect("Failed to write rpxy config");
    println!(">>> Generated rpxy-generated.toml");

    // 4. Spawn both processes
    println!(">>> Booting proxies on bare metal...");
    
    let mut l4_process = Command::new(&config.rpxy_l4_bin)
        .arg("--config")
        .arg("rpxy-l4-generated.toml")
        .stdout(Stdio::inherit())
        .stderr(Stdio::inherit())
        .spawn()
        .unwrap_or_else(|e| panic!("Failed to start '{}'. Is it installed and in your $PATH? Error: {}", config.rpxy_l4_bin, e));

    let mut l7_process = Command::new(&config.rpxy_bin)
        .arg("--config")
        .arg("rpxy-generated.toml")
        .stdout(Stdio::inherit())
        .stderr(Stdio::inherit())
        .spawn()
        .unwrap_or_else(|e| panic!("Failed to start '{}'. Is it installed and in your $PATH? Error: {}", config.rpxy_bin, e));

    // 5. Monitor and keep alive
    tokio::select! {
        status = l4_process.wait() => {
            println!("!!! rpxy-l4 exited with status: {:?}", status);
        }
        status = l7_process.wait() => {
            println!("!!! rpxy exited with status: {:?}", status);
        }
    }
}

fn generate_l4_config(config: &WeaverConfig) -> String {
    format!(
        r#"
[listen]
port = {public_https_port}
port_quic = {public_https_port}
bind_addresses = ["0.0.0.0", "[::]"]

[[upstreams]]
name = "rpxy_l7_https"
backend = ["127.0.0.1:{internal_https_port}"]
proxy_protocol = true

[[upstreams]]
name = "rpxy_l7_http"
backend = ["127.0.0.1:{internal_http_port}"]
proxy_protocol = true

[routing.tcp]
{public_http_port} = "rpxy_l7_http"

[routing.tls]
default = "rpxy_l7_https"
"#,
        public_http_port = config.public_http_port,
        public_https_port = config.public_https_port,
        internal_http_port = config.internal_http_port,
        internal_https_port = config.internal_https_port
    )
}

fn generate_l7_config(config: &WeaverConfig) -> String {
    let mut toml = format!(
        r#"
[listen]
port = {internal_http_port}
tls_port = {internal_https_port}
bind_addresses = ["127.0.0.1"]

[proxy_protocol]
trusted_proxies = ["127.0.0.1/32"]
timeout = 50

[acme]
email = "{email}"
"#,
        internal_http_port = config.internal_http_port,
        internal_https_port = config.internal_https_port,
        email = config.admin_email
    );

    for app in &config.apps {
        let tls_flag = if app.tls { "true" } else { "false" };
        let app_block = format!(
            r#"
[[reverse_proxy]]
server_name = ["{domain}"]
upstream = ["http://{backend}"]
tls = {tls}
"#,
            domain = app.domain,
            backend = app.backend,
            tls = tls_flag
        );
        toml.push_str(&app_block);
    }

    toml
}
