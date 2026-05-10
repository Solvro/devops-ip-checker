use std::{env, fs::File, hash::Hash, mem, sync::Arc};

use base64::Engine;
use cidr::{
    IpCidr,
    errors::NetworkParseError,
    parsers::{parse_cidr_full_ignore_hostbits, parse_loose_ip, parse_short_ip_address_as_cidr},
};
use cloneable_errors::{ErrorContext, ResContext};
use indexmap::IndexMap;
use serde::{
    Deserialize, Deserializer,
    de::{Error, Visitor},
};
use tokio::net::TcpStream;
use tokio_rustls::{
    TlsConnector,
    rustls::{ClientConfig, RootCertStore, pki_types::ServerName},
};
use tracing::{info, warn};
use webpki::EndEntityCert;

#[derive(Default, Debug)]
pub struct Config {
    pub app: AppConfig,
    pub listen: ListenConfig,
}

#[derive(Clone, Debug, Default)]
pub struct AppConfig {
    /// this server's name to be used in responses
    pub server_name: Option<Arc<str>>,
    /// mapping of client address cidrs to pretty names to be displayed
    pub ip_ranges: Arc<IndexMap<IpCidr, Box<str>>>,
    /// list of trusted proxy ips, for extraction of ip from headers
    pub trusted_proxies: Arc<[IpCidr]>,
    /// list of possible header names with IPs
    pub forwarded_headers: Arc<[Box<str>]>,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
#[repr(transparent)]
pub struct DeserializableCidr(IpCidr);

impl Hash for DeserializableCidr {
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        self.0.hash(state);
    }
}

#[allow(clippy::unsafe_derive_deserialize)]
#[derive(Deserialize, Default, Debug)]
#[serde(default)]
pub struct FileConfig {
    /// mapping of client address cidrs to pretty names to be displayed
    pub ip_ranges: Arc<IndexMap<DeserializableCidr, Box<str>>>,

    /// list of trusted proxy ips, for extraction of ip from headers
    pub trusted_proxies: Arc<[DeserializableCidr]>,

    /// list of possible header names with IPs
    pub forwarded_headers: Arc<[Box<str>]>,

    /// where should we get this server's name from?
    pub server_name: ServerNameSource,

    /// where should the server listen?
    pub listen: ListenConfig,
}

#[derive(Deserialize, Default, Debug)]
#[serde(tag = "source", rename_all = "snake_case")]
pub enum ServerNameSource {
    Static {
        name: Arc<str>,
    },
    TlsCertificate {
        /// the domain name/ip address with port to connect to
        target: Box<str>,
        /// domain name to ask for in SNI
        hostname: Box<str>,
        /// mapping for dns name in cert -> server name,
        /// first match wins
        mappings: IndexMap<Box<str>, Arc<str>>,
    },
    #[default]
    None,
}

#[derive(Debug, Deserialize)]
pub struct ListenConfig {
    /// Listen on a TCP socket
    #[serde(default)]
    pub tcp: Option<Box<str>>,
    /// Listen on a unix socket
    #[serde(default)]
    pub unix: Option<Box<str>>,
    /// Mode for the unix socket
    #[serde(default = "default_unix_mode")]
    pub unix_mode: u32,
}

impl Default for ListenConfig {
    fn default() -> Self {
        Self {
            tcp: Some("[::]:8080".into()),
            unix: None,
            unix_mode: 0,
        }
    }
}

fn default_unix_mode() -> u32 {
    0o666
}

impl Config {
    pub async fn get() -> Result<Config, ErrorContext> {
        if let Some(config_text) = env::var_os("IP_CHECKER_CONFIG") {
            // config file in the envvar
            let file_config: FileConfig = serde_json::from_slice(config_text.as_encoded_bytes())
                .context(
                    "Failed to parse the contents of IP_CHECKER_CONFIG envvar as FileConfig",
                )?;
            file_config
                .resolve()
                .await
                .context("Failed to resolve parsed IP_CHECKER_CONFIG config")
        } else if let Some(config_b64) = env::var_os("IP_CHECKER_CONFIG_B64") {
            // config file base64-encoded in the envvar
            let decoded = base64::engine::general_purpose::URL_SAFE
                .decode(config_b64.as_encoded_bytes())
                .context("Failed to base64-decode the contents of IP_CHECKER_CONFIG_B64 envvar")?;

            let file_config: FileConfig = serde_json::from_slice(&decoded)
                .context("Failed to parse the base64-decoded contents of IP_CHECKER_CONFIG_B64 envvar as FileConfig")?;

            file_config
                .resolve()
                .await
                .context("Failed to resolve parsed IP_CHECKER_CONFIG_B64 config")
        } else if let Some(config_path) = env::var_os("IP_CHECKER_CONFIG_FILE") {
            // path to config file in the envvar
            let file_config: FileConfig =
                serde_json::from_reader(File::open(&config_path).with_context(|| {
                    format!(
                        "Failed to open {} (IP_CHECKER_CONFIG_FILE) for reading",
                        config_path.display()
                    )
                })?)
                .with_context(|| {
                    format!(
                        "Failed to parse the contents of {} (IP_CHECKER_CONFIG_FILE) as FileConfig",
                        config_path.display()
                    )
                })?;
            file_config.resolve().await.with_context(|| {
                format!(
                    "Failed to resolve parsed {} (IP_CHECKER_CONFIG_FILE) config",
                    config_path.display()
                )
            })
        } else {
            warn!(
                "Neither IP_CHECKER_CONFIG nor IP_CHECKER_CONFIG_FILE envvars were present - using default configuration"
            );
            Ok(Config::default())
        }
    }
}

impl FileConfig {
    pub async fn resolve(self) -> Result<Config, ErrorContext> {
        Ok(Config {
            app: AppConfig {
                server_name: self
                    .server_name
                    .resolve()
                    .await
                    .context("Failed to resolve server name")?,
                ip_ranges: unsafe {
                    // SAFETY: this transmute changes the key type from DeserializableCidr to IpCidr.
                    //         DeserializableCidr is a newtype of IpCidr, with #[repr(transparent)],
                    //         and Hash implemented by delegating down to IpCidr.
                    mem::transmute::<
                        Arc<IndexMap<DeserializableCidr, Box<str>>>,
                        Arc<IndexMap<IpCidr, Box<str>>>,
                    >(self.ip_ranges)
                },
                trusted_proxies: unsafe {
                    // SAFETY: same as above, except this is just a ref-counted slice
                    mem::transmute::<Arc<[DeserializableCidr]>, Arc<[IpCidr]>>(self.trusted_proxies)
                },
                forwarded_headers: self.forwarded_headers,
            },
            listen: self.listen,
        })
    }
}

fn parse_cidr(input: &str) -> Result<IpCidr, NetworkParseError> {
    parse_cidr_full_ignore_hostbits(input, parse_loose_ip, parse_short_ip_address_as_cidr)
}

impl<'de> Deserialize<'de> for DeserializableCidr {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        struct CidrVisitor;
        impl Visitor<'_> for CidrVisitor {
            type Value = IpCidr;

            fn expecting(&self, formatter: &mut std::fmt::Formatter) -> std::fmt::Result {
                formatter.write_str("an IPv4 or v6 network in CIDR format")
            }

            fn visit_str<E: Error>(self, v: &str) -> Result<Self::Value, E> {
                parse_cidr(v).map_err(E::custom)
            }
        }

        deserializer
            .deserialize_str(CidrVisitor)
            .map(DeserializableCidr)
    }
}

impl ServerNameSource {
    pub async fn resolve(self) -> Result<Option<Arc<str>>, ErrorContext> {
        match self {
            ServerNameSource::None => Ok(None),
            ServerNameSource::Static { name } => Ok(Some(name)),
            ServerNameSource::TlsCertificate {
                target,
                hostname,
                mappings,
            } => {
                info!(
                    "TLS Certificate specified as server name source: Attempting to get valid DNS names for {target}'s certificate (using SNI hostname {hostname})"
                );
                let root_certs = RootCertStore {
                    roots: webpki_roots::TLS_SERVER_ROOTS.to_vec(),
                };

                let config = ClientConfig::builder()
                    .with_root_certificates(root_certs)
                    .with_no_client_auth();
                let connector = TlsConnector::from(Arc::new(config));
                let server_name = ServerName::DnsName(
                    hostname
                        .to_string()
                        .try_into()
                        .with_context(|| format!("{hostname} is not a valid dns name"))?,
                );

                let stream = TcpStream::connect(&*target)
                    .await
                    .with_context(|| format!("Failed to connect to {target}"))?;
                let stream = connector
                    .connect(server_name, stream)
                    .await
                    .with_context(|| format!("Failed to upgrade TCP to TLS for {target}"))?;

                let certificate = stream
                    .get_ref()
                    .1
                    .peer_certificates()
                    .context("No certificates found?")?
                    .first()
                    .context("Certificate chain empty?")?;
                let parsed_cert = EndEntityCert::try_from(certificate)
                    .context("Failed to parse peer certificate")?;
                let dns_names = parsed_cert.valid_dns_names().collect::<Vec<_>>();

                info!(
                    "Valid DNS names on {target}'s cert: {}",
                    dns_names.join(", ")
                );

                let result = mappings
                    .iter()
                    .find(|(target_domain, ..)| dns_names.contains(&&***target_domain))
                    .map(|(_, name)| name)
                    .cloned();

                info!("Server name resolved to {result:?}");
                Ok(result)
            }
        }
    }
}
