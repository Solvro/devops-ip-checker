use std::{net::IpAddr, sync::LazyLock};

use axum::{
    Router,
    extract::State,
    http::header,
    response::{Html, IntoResponse},
    routing::get,
};
use cidr::IpCidr;
use indexmap::IndexMap;

use crate::{config::AppConfig, get_ip::ExtractIp, metadata::{BUILD_TIME, GIT_HASH, GIT_REF}};

static METADATA_FOOTER: LazyLock<&'static str> = LazyLock::new(||
    if let Some(hash) = GIT_HASH {
        format!(
            r#"<h5 id="git"><a href="https://github.com/Solvro/devops-ip-checker/commit/{hash}">devops-ip-checker {}{} {BUILD_TIME}</a><h5>"#,
            &hash[..8],
            if let Some(gref) = GIT_REF {
                format!(" ({gref})")
            } else {
                String::new()
            },
        ).leak()
    } else {
        format!(
            r#"<h5 id="git"><a href="https://github.com/Solvro/devops-ip-checker">devops-ip-checker {BUILD_TIME}</a><h5>"#,
        ).leak()
    }
);


pub fn create_router(config: AppConfig) -> Router<()> {
    Router::new()
        .route("/", get(main_route))
        .route("/style.css", get(styles))
        .route("/health", get(healthcheck))
        .with_state(config)
}

fn classify_ip(addr: IpAddr, ranges: &IndexMap<IpCidr, Box<str>>) -> Option<&str> {
    ranges
        .iter()
        .find(|(cidr, ..)| cidr.contains(&addr))
        .map(|(_, name)| name)
        .map(|s| &**s)
}

async fn main_route(
    State(config): State<AppConfig>,
    maybe_ip: Option<ExtractIp>,
) -> impl IntoResponse {
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

async fn styles() -> impl IntoResponse {
    (
        [(header::CONTENT_TYPE, "text/css; charset=utf-8")],
        include_str!("./style.css"),
    )
}

async fn healthcheck() -> impl IntoResponse {
    "elo żelo!!!"
}
