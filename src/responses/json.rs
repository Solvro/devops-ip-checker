use std::{net::IpAddr, sync::Arc};

use axum::Json;
use serde::Serialize;

use crate::{
    config::AppConfig,
    get_ip::{ExtractIp, classify_ip},
    metadata::{BUILD_TIME, GIT_HASH, GIT_REF, VERSION},
};

#[derive(Serialize)]
pub struct JsonPayload {
    address: Option<IpAddr>,
    classification: Option<String>,
    meta: MetaPayload,
}

#[derive(Serialize)]
struct MetaPayload {
    server_name: Option<Arc<str>>,
    version: &'static str,
    git_ref: Option<&'static str>,
    git_hash: Option<&'static str>,
    build_timestamp: &'static str,
}

impl MetaPayload {
    const fn new(server_name: Option<Arc<str>>) -> Self {
        Self {
            server_name,
            version: VERSION,
            git_ref: GIT_REF,
            git_hash: GIT_HASH,
            build_timestamp: BUILD_TIME,
        }
    }
}

pub fn json_response(config: AppConfig, maybe_ip: Option<ExtractIp>) -> Json<JsonPayload> {
    Json(JsonPayload {
        address: maybe_ip.map(|ExtractIp(ip)| ip),
        classification: maybe_ip
            .and_then(|ExtractIp(ip)| classify_ip(ip, &config.ip_ranges).map(str::to_owned)),
        meta: MetaPayload::new(config.server_name),
    })
}
