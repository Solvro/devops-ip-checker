use axum::{Router, extract::State, http::header, response::IntoResponse, routing::get};

use crate::{
    config::AppConfig,
    get_ip::ExtractIp,
    responses::{
        auto::{AutoResponse, PreferredResponseType},
        html::html_response,
        json::json_response,
        text::plaintext_response,
    },
};

pub fn create_router(config: AppConfig) -> Router<()> {
    Router::new()
        .route("/", get(main_route))
        .route("/html", get(html_route))
        .route("/text", get(plaintext_route))
        .route("/json", get(json_route))
        .route("/ip", get(just_the_ip_route))
        .route("/style.css", get(styles))
        .route("/health", get(healthcheck))
        .with_state(config)
}

async fn main_route(
    State(config): State<AppConfig>,
    maybe_ip: Option<ExtractIp>,
    pref_type: PreferredResponseType,
) -> impl IntoResponse {
    match pref_type {
        PreferredResponseType::Html => AutoResponse::Html(html_response(config, maybe_ip)),
        PreferredResponseType::Json => AutoResponse::Json(json_response(config, maybe_ip)),
        PreferredResponseType::Text | PreferredResponseType::Unknown => {
            AutoResponse::Text(plaintext_response(config, maybe_ip))
        }
    }
}

async fn html_route(
    State(config): State<AppConfig>,
    maybe_ip: Option<ExtractIp>,
) -> impl IntoResponse {
    html_response(config, maybe_ip)
}

async fn plaintext_route(
    State(config): State<AppConfig>,
    maybe_ip: Option<ExtractIp>,
) -> impl IntoResponse {
    plaintext_response(config, maybe_ip)
}

async fn json_route(
    State(config): State<AppConfig>,
    maybe_ip: Option<ExtractIp>,
) -> impl IntoResponse {
    json_response(config, maybe_ip)
}

async fn just_the_ip_route(maybe_ip: Option<ExtractIp>) -> impl IntoResponse {
    maybe_ip.map_or_else(|| "unknown".to_owned(), |ip| ip.0.to_string())
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
