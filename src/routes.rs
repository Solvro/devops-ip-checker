use axum::{Router, extract::State, http::header, response::IntoResponse, routing::get};

use crate::{config::AppConfig, get_ip::ExtractIp, responses::html_response};

pub fn create_router(config: AppConfig) -> Router<()> {
    Router::new()
        .route("/", get(main_route))
        .route("/style.css", get(styles))
        .route("/health", get(healthcheck))
        .with_state(config)
}

async fn main_route(
    State(config): State<AppConfig>,
    maybe_ip: Option<ExtractIp>,
) -> impl IntoResponse {
    html_response(config, maybe_ip)
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
