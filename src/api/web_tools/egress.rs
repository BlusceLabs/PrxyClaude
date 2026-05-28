use std::net::{IpAddr, ToSocketAddrs};
use std::sync::Arc;
use thiserror::Error;
use url::Url;

#[derive(Error, Debug)]
pub enum WebFetchEgressViolation {
    #[error("URL scheme {0:?} is not allowed for web_fetch")]
    SchemeNotAllowed(String),
    #[error("web_fetch URL must include a host")]
    NoHost,
    #[error("localhost targets are not allowed for web_fetch")]
    Localhost,
    #[error(".local hostnames are not allowed for web_fetch")]
    LocalHostname,
    #[error("Non-public IP host {0:?} is not allowed for web_fetch")]
    NonPublicIp(String),
    #[error("Could not resolve host {0:?}: {1}")]
    ResolutionFailed(String, String),
    #[error("Host {0:?} resolves to a non-public address ({1})")]
    ResolvedToNonPublic(String, String),
    #[error("web_fetch exceeded maximum redirects ({0})")]
    MaxRedirects(usize),
    #[error("web_fetch redirect response missing Location header")]
    MissingLocation,
}

#[derive(Debug, Clone)]
pub struct WebFetchEgressPolicy {
    pub allow_private_network_targets: bool,
    pub allowed_schemes: Arc<Vec<String>>,
}

impl WebFetchEgressPolicy {
    pub fn new(allow_private_network_targets: bool, allowed_schemes: Vec<String>) -> Self {
        Self {
            allow_private_network_targets,
            allowed_schemes: Arc::new(allowed_schemes),
        }
    }

    pub fn allowed_scheme_set(&self) -> Vec<String> {
        self.allowed_schemes
            .iter()
            .map(|s| s.to_lowercase())
            .collect()
    }
}

fn port_for_url(url: &Url) -> u16 {
    url.port()
        .unwrap_or_else(|| if url.scheme() == "https" { 443 } else { 80 })
}

fn is_global_ip(addr: &IpAddr) -> bool {
    match addr {
        IpAddr::V4(v4) => !v4.is_private()
            && !v4.is_loopback()
            && !v4.is_link_local()
            && !v4.is_multicast()
            && !v4.is_broadcast()
            && !v4.is_documentation()
            && !v4.is_unspecified(),
        IpAddr::V6(v6) => !v6.is_loopback()
            && !v6.is_multicast()
            && !v6.is_unspecified()
            && !v6.is_unique_local()
            && !v6.is_unicast_link_local(),
    }
}

pub fn get_validated_addrs_for_egress(
    url_str: &str,
    policy: &WebFetchEgressPolicy,
) -> Result<Vec<std::net::SocketAddr>, WebFetchEgressViolation> {
    let parsed = Url::parse(url_str).map_err(|_| {
        WebFetchEgressViolation::SchemeNotAllowed("invalid url".to_string())
    })?;

    let scheme = parsed.scheme().to_lowercase();
    let allowed = policy.allowed_scheme_set();
    if !allowed.contains(&scheme) {
        return Err(WebFetchEgressViolation::SchemeNotAllowed(scheme));
    }

    let host = parsed.host_str().ok_or(WebFetchEgressViolation::NoHost)?;
    if host.is_empty() {
        return Err(WebFetchEgressViolation::NoHost);
    }

    if policy.allow_private_network_targets {
        return resolve_host(host, port_for_url(&parsed));
    }

    let host_lower = host.to_lowercase();
    if host_lower == "localhost" || host_lower.ends_with(".localhost") {
        return Err(WebFetchEgressViolation::Localhost);
    }
    if host_lower.ends_with(".local") {
        return Err(WebFetchEgressViolation::LocalHostname);
    }

    if let Ok(parsed_ip) = host.parse::<IpAddr>() {
        if !is_global_ip(&parsed_ip) {
            return Err(WebFetchEgressViolation::NonPublicIp(host.to_string()));
        }
        return resolve_host(host, port_for_url(&parsed));
    }

    let addrs = resolve_host(host, port_for_url(&parsed))?;
    for addr in &addrs {
        if !is_global_ip(&addr.ip()) {
            return Err(WebFetchEgressViolation::ResolvedToNonPublic(
                host.to_string(),
                addr.ip().to_string(),
            ));
        }
    }
    Ok(addrs)
}

fn resolve_host(host: &str, port: u16) -> Result<Vec<std::net::SocketAddr>, WebFetchEgressViolation> {
    let addr_str = format!("{host}:{port}");
    let addrs: Vec<std::net::SocketAddr> = (addr_str, 0)
        .to_socket_addrs()
        .map_err(|e| {
            WebFetchEgressViolation::ResolutionFailed(host.to_string(), e.to_string())
        })?
        .collect();

    if addrs.is_empty() {
        return Err(WebFetchEgressViolation::ResolutionFailed(
            host.to_string(),
            "no addresses resolved".to_string(),
        ));
    }
    Ok(addrs)
}

pub fn enforce_web_fetch_egress(
    url: &str,
    policy: &WebFetchEgressPolicy,
) -> Result<(), WebFetchEgressViolation> {
    get_validated_addrs_for_egress(url, policy)?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_scheme_not_allowed() {
        let policy = WebFetchEgressPolicy::new(true, vec!["https".to_string()]);
        let result = enforce_web_fetch_egress("http://example.com", &policy);
        assert!(result.is_err());
        assert!(matches!(result, Err(WebFetchEgressViolation::SchemeNotAllowed(_))));
    }

    #[test]
    fn test_localhost_blocked() {
        let policy = WebFetchEgressPolicy::new(false, vec!["https".to_string()]);
        let result = enforce_web_fetch_egress("https://localhost:8080/test", &policy);
        assert!(result.is_err());
        assert!(matches!(result, Err(WebFetchEgressViolation::Localhost)));
    }
}
