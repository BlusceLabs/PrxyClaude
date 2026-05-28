use std::collections::HashSet;
use std::net::{IpAddr, Ipv4Addr};
use thiserror::Error;

#[derive(Error, Debug)]
pub enum EgressError {
    #[error("URL scheme {0:?} is not allowed for web_fetch")]
    InvalidScheme(String),
    
    #[error("web_fetch URL must include a host")]
    NoHost,
    
    #[error("Could not resolve host {0}: {1}")]
    ResolutionError(String, String),
    
    #[error("Non-public IP host {0:?} is not allowed for web_fetch")]
    NonPublicIp(String),
    
    #[error("Host {0:?} resolves to a non-public address ({1})")]
    NonPublicResolvedAddress(String, String),
    
    #[error("web_fetch exceeded maximum redirects ({0})")]
    TooManyRedirects(usize),
    
    #[error("web_fetch redirect response missing Location header")]
    MissingLocationHeader,
}

#[derive(Debug, Clone)]
pub struct WebFetchEgressPolicy {
    pub allow_private_network_targets: bool,
    pub allowed_schemes: HashSet<String>,
}

impl Default for WebFetchEgressPolicy {
    fn default() -> Self {
        Self {
            allow_private_network_targets: false,
            allowed_schemes: HashSet::from(["https".to_string()]),
        }
    }
}

pub fn get_port_for_url(url: &url::Url) -> u16 {
    url.port()
        .unwrap_or_else(|| if url.scheme() == "https" { 443 } else { 80 })
}

pub fn resolve_host_to_addresses(host: &str) -> Result<Vec<std::net::SocketAddr>, EgressError> {
    use std::net::ToSocketAddrs;
    
    match (host, 0).to_socket_addrs() {
        Ok(addrs) => Ok(addrs.collect()),
        Err(e) => Err(EgressError::ResolutionError(host.to_string(), e.to_string())),
    }
}

pub fn validate_url_for_egress(url: &str, policy: &WebFetchEgressPolicy) -> Result<(), EgressError> {
    let parsed = url.parse::<url::Url>().map_err(|_| EgressError::NoHost)?;
    let scheme = parsed.scheme().to_lowercase();
    
    if !policy.allowed_schemes.contains(&scheme) {
        return Err(EgressError::InvalidScheme(scheme));
    }
    
    let host = parsed.host().ok_or(EgressError::NoHost)?;
    
    match host {
        url::Host::Ipv4(ip) => {
            if !policy.allow_private_network_targets {
                if ip == Ipv4Addr::LOCALHOST {
                    return Err(EgressError::NonPublicIp(ip.to_string()));
                }
                
                // Check for private networks
                if (ip.octets()[0] == 10) ||
                   (ip.octets()[0] == 172 && ip.octets()[1] >= 16 && ip.octets()[1] <= 31) ||
                   (ip.octets()[0] == 192 && ip.octets()[1] == 168) {
                    return Err(EgressError::NonPublicIp(ip.to_string()));
                }
            }
        },
        url::Host::Ipv6(ip) => {
            if !policy.allow_private_network_targets {
                // Check for IPv6 localhost
                if ip.is_loopback() {
                    return Err(EgressError::NonPublicIp(ip.to_string()));
                }
                
                // Check for private IPv6 networks (fc00::/7, etc.)
                // Simplified check - in production, use proper IPv6 private address detection
            }
        },
        url::Host::Domain(domain) => {
            if !policy.allow_private_network_targets {
                // Check for localhost variants
                let domain_lower = domain.to_lowercase();
                if domain_lower == "localhost" || domain_lower.ends_with(".localhost") {
                    return Err(EgressError::NonPublicIp(domain.to_string()));
                }
                
                if domain_lower.ends_with(".local") {
                    return Err(EgressError::NonPublicIp(domain.to_string()));
                }
                
                // Resolve to check if it's a private IP
                if !policy.allow_private_network_targets {
                    let addresses = resolve_host_to_addresses(domain)?;
                    for addr in addresses {
                        match addr.ip() {
                            IpAddr::V4(ipv4) => {
                                if (ipv4.octets()[0] == 10) ||
                                   (ipv4.octets()[0] == 172 && ipv4.octets()[1] >= 16 && ipv4.octets()[1] <= 31) ||
                                   (ipv4.octets()[0] == 192 && ipv4.octets()[1] == 168) {
                                    return Err(EgressError::NonPublicResolvedAddress(
                                        domain.to_string(),
                                        ipv4.to_string(),
                                    ));
                                }
                            },
                            IpAddr::V6(ipv6) => {
                                // Simplified IPv6 private address check
                                if ipv6.is_loopback() {
                                    return Err(EgressError::NonPublicResolvedAddress(
                                        domain.to_string(),
                                        ipv6.to_string(),
                                    ));
                                }
                            },
                        }
                    }
                }
            }
        },
    }
    
    Ok(())
}