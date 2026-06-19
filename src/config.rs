use serde::Deserialize;

#[derive(Deserialize, Debug)]
pub struct WeaverConfig {
    pub rpxy_bin: String,
    pub rpxy_l4_bin: String,
    pub admin_email: String,
    pub public_http_port: u16,
    pub public_https_port: u16,
    pub internal_http_port: u16,
    pub internal_https_port: u16,
    pub apps: Vec<AppConfig>,
}

#[derive(Deserialize, Debug)]
pub struct AppConfig {
    pub domain: String,
    pub backend: String,
    pub tls: bool,
}
