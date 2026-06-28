use std::sync::LazyLock;

use axum::response::Html;

use crate::{
    config::AppConfig,
    get_ip::{ExtractIp, classify_ip},
    metadata::{BUILD_TIME, GIT_HASH, GIT_REF},
};

static METADATA_FOOTER: LazyLock<&'static str> = LazyLock::new(|| {
    if let Some(hash) = GIT_HASH {
        format!(
            r#"<h5 id="git"><a href="https://github.com/Solvro/devops-ip-checker/commit/{}">devops-ip-checker {}{} {}</a><h5>"#,
            html_escape::encode_safe(hash),
            html_escape::encode_safe(&hash[..8]),
            if let Some(gref) = GIT_REF {
                format!(" ({})", html_escape::encode_safe(gref))
            } else {
                String::new()
            },
            html_escape::encode_safe(BUILD_TIME),
        ).leak()
    } else {
        format!(
            r#"<h5 id="git"><a href="https://github.com/Solvro/devops-ip-checker">devops-ip-checker {}</a><h5>"#,
            html_escape::encode_safe(BUILD_TIME),
        ).leak()
    }
});

pub fn html_response(config: AppConfig, maybe_ip: Option<ExtractIp>) -> Html<String> {
    Html(format!(
        r#"<!DOCTYPE html>
<html>
    <head>
        <title>sprawdzarka ip</title>
        <meta charset="utf-8">
        <link rel="stylesheet" href="/style.css">
    </head>
    <body>
        <h2 id="ip-header">Twoje IP:</h2>
        <h3 id="ip"{}>{}</h3>
        {}
        {}
        {}
    </body>
</html>"#,
        if maybe_ip.is_some_and(|ExtractIp(ip)| ip.is_ipv6()) {
            " data-v6"
        } else {
            ""
        },
        html_escape::encode_safe(
            &maybe_ip.map_or_else(|| "nieznane".to_string(), |ExtractIp(ip)| ip.to_string())
        ),
        if let Some(class) = maybe_ip.and_then(|ExtractIp(ip)| classify_ip(ip, &config.ip_ranges)) {
            format!(
                r#"<h4 id="ip-class">({})</h4>"#,
                html_escape::encode_safe(class)
            )
        } else {
            String::new()
        },
        if let Some(server) = config.server_name {
            format!(
                r#"<h4 id="server-name">serwer: {}</h4>"#,
                html_escape::encode_safe(&server)
            )
        } else {
            String::new()
        },
        *METADATA_FOOTER
    ))
}
