//! Two-phase port probe implementation.
//!
//! Phase 1: Fast local loopback TCP probe (< 1ms gatekeeper) verifying that the local
//! proxy client (e.g. Clash, Mihomo, v2ray, Sing-box) is active and listening.
//! Phase 2: Upstream HTTP probe through proxy to external health endpoints.

use std::fmt;
use std::io::{self, Read, Write};
use std::net::{IpAddr, SocketAddr, TcpStream, ToSocketAddrs};
use std::time::Duration;

#[derive(Debug)]
pub enum ProbeError {
    PortClosed {
        host: String,
        port: u16,
    },
    Timeout {
        host: String,
        port: u16,
        duration: Duration,
    },
    ResolutionFailed {
        host: String,
        port: u16,
        details: String,
    },
    HttpError {
        status: u16,
        target: String,
    },
    InvalidUrl(String),
    Io(io::Error),
}

impl fmt::Display for ProbeError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ProbeError::PortClosed { host, port } => {
                write!(f, "Proxy port is closed or not listening: {}:{}", host, port)
            }
            ProbeError::Timeout {
                host,
                port,
                duration,
            } => write!(
                f,
                "Connection to {}:{} timed out after {:?}",
                host, port, duration
            ),
            ProbeError::ResolutionFailed {
                host,
                port,
                details,
            } => write!(
                f,
                "Failed to resolve address for {}:{}: {}",
                host, port, details
            ),
            ProbeError::HttpError { status, target } => {
                write!(f, "HTTP probe to {} failed with status {}", target, status)
            }
            ProbeError::InvalidUrl(url) => write!(f, "Invalid target URL for probe: {}", url),
            ProbeError::Io(e) => write!(f, "I/O error during port probe: {}", e),
        }
    }
}

impl std::error::Error for ProbeError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            ProbeError::Io(e) => Some(e),
            _ => None,
        }
    }
}

impl From<io::Error> for ProbeError {
    fn from(e: io::Error) -> Self {
        ProbeError::Io(e)
    }
}

/// Resolves a host and port to a concrete `SocketAddr`.
/// Optimizes `localhost` and `127.0.0.1` to bypass DNS lookups entirely.
fn resolve_target(host: &str, port: u16) -> Result<SocketAddr, ProbeError> {
    let trimmed = host.trim();
    if trimmed.eq_ignore_ascii_case("localhost") || trimmed == "127.0.0.1" {
        return Ok(SocketAddr::from(([127, 0, 0, 1], port)));
    }
    if let Ok(ip) = trimmed.parse::<IpAddr>() {
        return Ok(SocketAddr::new(ip, port));
    }
    let addrs: Vec<SocketAddr> = (trimmed, port)
        .to_socket_addrs()
        .map_err(|e| ProbeError::ResolutionFailed {
            host: host.to_string(),
            port,
            details: e.to_string(),
        })?
        .collect();

    addrs.into_iter().next().ok_or_else(|| ProbeError::ResolutionFailed {
        host: host.to_string(),
        port,
        details: "No IP addresses found".to_string(),
    })
}

/// Phase 1: Fast local TCP probe verifying that the proxy daemon is listening.
///
/// Designed to execute in < 1ms for local loopback connections.
pub fn probe_local_port(host: &str, port: u16, timeout: Duration) -> Result<(), ProbeError> {
    let addr = resolve_target(host, port)?;
    match TcpStream::connect_timeout(&addr, timeout) {
        Ok(stream) => {
            drop(stream);
            Ok(())
        }
        Err(e) => match e.kind() {
            io::ErrorKind::ConnectionRefused => Err(ProbeError::PortClosed {
                host: host.to_string(),
                port,
            }),
            io::ErrorKind::TimedOut => Err(ProbeError::Timeout {
                host: host.to_string(),
                port,
                duration: timeout,
            }),
            _ => Err(ProbeError::Io(e)),
        },
    }
}

/// Parses target URL into host and full request target for HTTP proxy request.
fn parse_target_host(url: &str) -> Result<(&str, &str), ProbeError> {
    let stripped = if let Some(s) = url.strip_prefix("http://") {
        s
    } else if let Some(s) = url.strip_prefix("https://") {
        s
    } else {
        return Err(ProbeError::InvalidUrl(url.to_string()));
    };

    let host = match stripped.find('/') {
        Some(pos) => &stripped[..pos],
        None => stripped,
    };

    if host.is_empty() {
        return Err(ProbeError::InvalidUrl(url.to_string()));
    }

    Ok((host, url))
}

/// Phase 2: Upstream HTTP probe through proxy to verify external connectivity.
///
/// Sends a standard HTTP proxy GET request and parses the returned HTTP status code.
pub fn probe_upstream_http(
    proxy_host: &str,
    proxy_port: u16,
    target_url: &str,
    timeout: Duration,
) -> Result<u16, ProbeError> {
    let (target_host, request_uri) = parse_target_host(target_url)?;
    let proxy_addr = resolve_target(proxy_host, proxy_port)?;

    let mut stream = TcpStream::connect_timeout(&proxy_addr, timeout)?;
    stream.set_read_timeout(Some(timeout))?;
    stream.set_write_timeout(Some(timeout))?;

    let request = format!(
        "GET {} HTTP/1.1\r\nHost: {}\r\nUser-Agent: proxy-launcher/0.1\r\nConnection: close\r\n\r\n",
        request_uri, target_host
    );

    stream.write_all(request.as_bytes())?;
    stream.flush()?;

    let mut buffer = [0u8; 512];
    let bytes_read = stream.read(&mut buffer)?;
    if bytes_read == 0 {
        return Err(ProbeError::Io(io::Error::new(
            io::ErrorKind::UnexpectedEof,
            "Proxy server closed connection without response",
        )));
    }

    let response_text = String::from_utf8_lossy(&buffer[..bytes_read]);
    let first_line = response_text.lines().next().unwrap_or_default();

    // Parse status line: HTTP/1.1 204 No Content
    let mut parts = first_line.split_whitespace();
    let _version = parts.next();
    let status_str = parts.next().ok_or_else(|| {
        ProbeError::Io(io::Error::new(
            io::ErrorKind::InvalidData,
            format!("Invalid HTTP response status line: {}", first_line),
        ))
    })?;

    let status_code: u16 = status_str.parse().map_err(|_| {
        ProbeError::Io(io::Error::new(
            io::ErrorKind::InvalidData,
            format!("Failed to parse status code: {}", status_str),
        ))
    })?;

    if (200..=399).contains(&status_code) {
        Ok(status_code)
    } else {
        Err(ProbeError::HttpError {
            status: status_code,
            target: target_url.to_string(),
        })
    }
}
