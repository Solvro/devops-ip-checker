use std::{
    env,
    fs::{self, Permissions},
    io::ErrorKind,
    net::SocketAddr,
    os::unix::fs::{MetadataExt, PermissionsExt},
    time::{Duration, Instant},
};

use async_signal::{Signal, Signals};
use cloneable_errors::{ErrContext, ErrorContext, ResContext, anyhow, bail};
use futures_lite::StreamExt;
use reqwest::{ClientBuilder, StatusCode};
use tokio::{
    net::{TcpListener, UnixListener},
    select,
    task::JoinSet,
};
use tracing::{debug, info};

use crate::{
    config::{FileConfig, ListenConfig},
    routes::create_router,
};

mod config;
mod get_ip;
mod responses;
mod routes;

mod metadata {
    include!(concat!(env!("OUT_DIR"), "/metadata.rs"));
}

#[tokio::main]
async fn main() -> Result<(), ErrorContext> {
    tracing_subscriber::fmt::init();
    let config = FileConfig::get().context("Failed to read config")?;

    if env::args().nth(1).is_some_and(|x| x == "health") {
        health(config.listen).await
    } else {
        server(config).await
    }
}

async fn health(listen_config: ListenConfig) -> Result<(), ErrorContext> {
    info!("Running in healthcheck mode");

    if let Some(unix_path) = listen_config.unix {
        let start = Instant::now();
        info!("Checking {unix_path} over unix sockets");
        let client = ClientBuilder::new()
            .unix_socket(&*unix_path)
            .timeout(Duration::from_secs(1))
            .build()
            .context("Failed to build new reqwest client")?;

        let response = client
            .get("http://localhost/health")
            .send()
            .await
            .with_context(|| format!("/health request over unix socket at {unix_path} failed"))?;
        let status = response.status();

        if status != StatusCode::OK {
            bail!("Expected /health to return code 200, got {status}",)
        }

        info!("Got 200 response in {}ms", start.elapsed().as_millis());
    }
    if let Some(target) = listen_config.tcp {
        let start = Instant::now();
        info!("Checking {target} over TCP");
        let client = ClientBuilder::new()
            .timeout(Duration::from_secs(1))
            .build()
            .context("Failed to build new reqwest client")?;

        let url = format!("http://{target}/health");
        let response = client
            .get(&url)
            .send()
            .await
            .with_context(|| format!("Request to {url} failed"))?;
        let status = response.status();

        if status != StatusCode::OK {
            bail!("Expected /health to return code 200, got {status}",)
        }

        info!("Got 200 response in {}ms", start.elapsed().as_millis());
    }

    info!("Healthchecks OK");
    Ok(())
}

async fn server(config: FileConfig) -> Result<(), ErrorContext> {
    let config = config
        .resolve()
        .await
        .context("Failed to resolve service config")?;

    debug!("Parsed config: {config:?}");

    let router = create_router(config.app);
    let mut join_set = JoinSet::<Result<(), ErrorContext>>::new();

    if let Some(addr) = config.listen.tcp {
        let router = router.clone();
        join_set.spawn(async move {
            let listener = TcpListener::bind(&*addr)
                .await
                .with_context(|| format!("Failed to bind a TcpListener to {addr}"))?;
            info!("Listening on TCP {addr}!");
            axum::serve(
                listener,
                router.into_make_service_with_connect_info::<SocketAddr>(),
            )
            .await
            .with_context(|| format!("Error while serving on a TCP socket at {addr}"))
        });
    }
    if let Some(path) = config.listen.unix {
        join_set.spawn(async move {
            // clear existing socket
            match fs::metadata(&*path) {
                Err(e) if e.kind() == ErrorKind::NotFound => (),
                Err(e) => return Err(e.context(format!("Failed to stat {path}"))),
                Ok(m) => {
                    if m.mode() & 0o140_000 == 0 {
                        bail!("Non-socket file found at Unix socket listen location {path}",);
                    }
                    fs::remove_file(&*path)
                        .with_context(|| format!("Failed to delete existing socket at {path}"))?;
                }
            }

            let listener = UnixListener::bind(&*path)
                .with_context(|| format!("Failed to bind an UnixListener to {path}"))?;

            // set new perms
            fs::set_permissions(&*path, Permissions::from_mode(config.listen.unix_mode))
                .with_context(|| {
                    format!(
                        "Failed to set mode of socket at {path} to {:o}",
                        config.listen.unix_mode
                    )
                })?;

            info!("Listening on a Unix socket at {path}!");
            axum::serve(listener, router)
                .await
                .with_context(|| format!("Error while serving on an Unix socket at {path}"))
        });
    } else {
        drop(router);
    }

    let joiner = async {
        Err(match join_set.join_next().await {
            None => anyhow!("No listen targets specified"),
            Some(Err(e)) => e.context("One of the listen tasks panicked"),
            Some(Ok(Err(e))) => e.context("One of the listen tasks returned an error"),
            Some(Ok(Ok(()))) => anyhow!("One of the listen tasks returned with no error"),
        })
    };

    let mut signals = Signals::new([Signal::Term, Signal::Int, Signal::Quit, Signal::Hup])
        .context("Failed to set up signal hooks")?;

    select! {
        biased;
        res = joiner => res,
        _ = signals.next() => Ok(()),
    }
}
