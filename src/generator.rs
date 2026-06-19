use crate::config::WeaverConfig;

pub fn generate_l4_config(config: &WeaverConfig) -> String {
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

pub fn generate_l7_config(config: &WeaverConfig) -> String {
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
