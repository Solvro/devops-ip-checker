use std::convert::Infallible;
use std::str::FromStr;

use axum::{
    Json,
    extract::FromRequestParts,
    http::header,
    http::request::Parts,
    response::{Html, IntoResponse},
};

use crate::responses::json::JsonPayload;

pub enum AutoResponse {
    Text(String),
    Json(Json<JsonPayload>),
    Html(Html<String>),
}

impl IntoResponse for AutoResponse {
    fn into_response(self) -> axum::response::Response {
        match self {
            Self::Text(text) => text.into_response(),
            Self::Json(json) => json.into_response(),
            Self::Html(html) => html.into_response(),
        }
    }
}

#[derive(Clone, Copy, Default)]
pub enum PreferredResponseType {
    Text,
    Json,
    Html,
    #[default]
    Unknown,
}

impl<S: Send + Sync> FromRequestParts<S> for PreferredResponseType {
    type Rejection = Infallible;

    async fn from_request_parts(parts: &mut Parts, _state: &S) -> Result<Self, Self::Rejection> {
        let Some(accept_header) = parts.headers.get(header::ACCEPT) else {
            return Ok(Self::Unknown);
        };
        let Ok(accept_header) = accept_header.to_str() else {
            return Ok(Self::Unknown);
        };
        let mut format_preference: [(PreferredResponseType, Option<f32>); 3] = [
            (PreferredResponseType::Html, None),
            (PreferredResponseType::Json, None),
            (PreferredResponseType::Text, None),
        ];

        // iterate over the specified formats
        for format in accept_header.split(',') {
            // split the format entry further into parts
            let mut format_part_iter = format.split(';').map(str::trim);
            let Some(format_name) = format_part_iter.next() else {
                continue;
            };
            // look for the q param, parse as f32, default to 1.0
            let quality = format_part_iter
                .find_map(|p| p.strip_prefix("q=").map(f32::from_str).and_then(Result::ok))
                .unwrap_or(1.);

            if quality <= 0. {
                continue;
            }
            let quality = quality.min(1.);

            // match the format and update the format preference ranking
            match format_name {
                "text/html" => {
                    let entry = &mut format_preference[0];
                    entry.1 = Some(quality.max(entry.1.unwrap_or(0.)));
                }
                "application/json" | "application/*" => {
                    let entry = &mut format_preference[1];
                    entry.1 = Some(quality.max(entry.1.unwrap_or(0.)));
                }
                "text/plain" => {
                    let entry = &mut format_preference[2];
                    entry.1 = Some(quality.max(entry.1.unwrap_or(0.)));
                }
                "text/*" => {
                    // matches both plaintext and html
                    let entry = &mut format_preference[0];
                    entry.1 = Some(quality.max(entry.1.unwrap_or(0.)));
                    let entry = &mut format_preference[2];
                    entry.1 = Some(quality.max(entry.1.unwrap_or(0.)));
                }
                _ => {}
            }
        }

        // find the highest score
        let Some(best_score) = format_preference
            .iter()
            .filter_map(|x| x.1)
            .reduce(f32::max)
        else {
            // no formats matched
            return Ok(Self::Unknown);
        };

        // then find which format got it
        Ok(format_preference
            .iter()
            .find(|x| x.1 == Some(best_score))
            .map_or(Self::Unknown, |x| x.0))
    }
}
