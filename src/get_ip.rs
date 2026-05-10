use std::{
    convert::Infallible,
    net::{IpAddr, SocketAddr}, ops::Deref, str::FromStr,
};

use axum::{
    extract::{ConnectInfo, FromRequestParts, OptionalFromRequestParts},
    http::request::Parts,
};

use crate::config::AppConfig;

#[derive(Clone, Copy, Debug)]
pub struct ExtractIp(pub IpAddr);

impl OptionalFromRequestParts<AppConfig> for ExtractIp {
    type Rejection = Infallible;

    async fn from_request_parts(
        parts: &mut Parts,
        state: &AppConfig,
    ) -> Result<Option<Self>, Self::Rejection> {
        // check if we have an IP address and it's not a trusted one
        if let Some(info) = parts.extensions.get::<ConnectInfo<SocketAddr>>() {
            let addr = info.0.ip().to_canonical();
            if !state.trusted_proxies.iter().any(|cidr| cidr.contains(&addr)) {
                // return it as the user's IP
                return Ok(Some(Self(addr)));
            }
        }
        // we either have a proxy IP, or we don't have an IP (i.e. it's an unix socket connection)
        // extract from headers
        for key in state.forwarded_headers.iter() {
            let Some(val) = parts.headers.get(key.deref()) else {
                continue;
            };
            if let Some(addr) = val.to_str().ok().and_then(|val| IpAddr::from_str(val).ok()) {
                return Ok(Some(Self(addr.to_canonical())));
            }
        }
        Ok(None)
    }
}

impl FromRequestParts<AppConfig> for ExtractIp {
    type Rejection = &'static str;

    async fn from_request_parts(
        parts: &mut Parts,
        state: &AppConfig,
    ) -> Result<Self, Self::Rejection> {
        match <Self as OptionalFromRequestParts<AppConfig>>::from_request_parts(parts, state).await {
            Ok(Some(v)) => Ok(v),
            Ok(None) => Err("Failed to find your IP address"),
        }
    }
}
