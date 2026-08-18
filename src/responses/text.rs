use std::sync::LazyLock;

use crate::{
    config::AppConfig,
    get_ip::{ExtractIp, classify_ip},
    metadata::{BUILD_TIME, GIT_HASH, GIT_REF},
};

pub(super) static TEXT_METADATA_FOOTER: LazyLock<&'static str> = LazyLock::new(|| {
    if let Some(hash) = GIT_HASH {
        format!(
            r"devops-ip-checker {}{} {}",
            &hash[..8],
            if let Some(gref) = GIT_REF {
                format!(" ({gref})")
            } else {
                String::new()
            },
            BUILD_TIME,
        )
    } else {
        format!(r"devops-ip-checker {BUILD_TIME}")
    }
    .leak()
});

pub fn plaintext_response(config: AppConfig, maybe_ip: Option<ExtractIp>) -> String {
    format!(
        "Twoje IP: {}{}{}\n{}",
        maybe_ip.map_or_else(|| "nieznane".to_string(), |ExtractIp(ip)| ip.to_string()),
        if let Some(class) = maybe_ip.and_then(|ExtractIp(ip)| classify_ip(ip, &config.ip_ranges)) {
            format!(" ({})", html_escape::encode_safe(class))
        } else {
            String::new()
        },
        if let Some(server) = config.server_name {
            format!("\nserwer: {}", html_escape::encode_safe(&server))
        } else {
            String::new()
        },
        *TEXT_METADATA_FOOTER
    )
}
