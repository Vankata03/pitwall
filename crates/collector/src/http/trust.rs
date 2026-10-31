use std::{
    collections::HashSet,
    net::{IpAddr, SocketAddr},
};

use axum::{
    extract::{ConnectInfo, Request, State},
    http::{HeaderMap, Method, StatusCode, uri::Authority},
    middleware::Next,
    response::Response,
};

/// Known addresses of this machine, never addresses resolved from request headers.
/// Loopback, `localhost`, and the collector's `pitwall.local` name are also allowed.
#[derive(Clone)]
pub struct TrustedHosts {
    addresses: HashSet<IpAddr>,
}

impl TrustedHosts {
    /// Inventory local interfaces without resolving any request-supplied hostname.
    /// Rebuild the shell when the host's interface addresses change.
    pub fn discover() -> std::io::Result<Self> {
        Ok(Self::new(
            if_addrs::get_if_addrs()?
                .into_iter()
                .map(|interface| interface.ip()),
        ))
    }

    /// Supply addresses from a trusted interface inventory, not client input or DNS.
    pub fn new(addresses: impl IntoIterator<Item = IpAddr>) -> Self {
        Self {
            addresses: addresses
                .into_iter()
                .map(|ip| ip.to_canonical())
                .filter(|ip| !ip.is_unspecified())
                .collect(),
        }
    }

    fn contains(&self, authority: &Authority) -> bool {
        let host = authority.host();
        if host.eq_ignore_ascii_case("localhost") || host.eq_ignore_ascii_case("pitwall.local") {
            return true;
        }
        host.trim_start_matches('[')
            .trim_end_matches(']')
            .parse::<IpAddr>()
            .is_ok_and(|ip| {
                let ip = ip.to_canonical();
                ip.is_loopback() || self.addresses.contains(&ip)
            })
    }
}

fn single_header<'a>(headers: &'a HeaderMap, name: &str) -> Option<&'a str> {
    let mut values = headers.get_all(name).iter();
    let value = values.next()?.to_str().ok()?;
    values.next().is_none().then_some(value)
}

fn authority(value: &str) -> Option<Authority> {
    let authority = value.parse::<Authority>().ok()?;
    let suffix = value.get(authority.host().len()..)?;
    if value.contains('@') || (!suffix.is_empty() && authority.port_u16().is_none()) {
        return None;
    }
    Some(authority)
}

pub(super) async fn check_mutation(
    State(hosts): State<TrustedHosts>,
    request: Request,
    next: Next,
) -> Result<Response, StatusCode> {
    if !matches!(
        *request.method(),
        Method::GET | Method::HEAD | Method::OPTIONS
    ) {
        let headers = request.headers();
        let json = single_header(headers, "content-type").is_some_and(|value| {
            value
                .split(';')
                .next()
                .unwrap()
                .trim()
                .eq_ignore_ascii_case("application/json")
        });
        if !json {
            return Err(StatusCode::UNSUPPORTED_MEDIA_TYPE);
        }
        let host = single_header(headers, "host")
            .and_then(authority)
            .ok_or(StatusCode::FORBIDDEN)?;
        let origin = single_header(headers, "origin")
            .and_then(|value| value.strip_prefix("http://"))
            .and_then(authority)
            .ok_or(StatusCode::FORBIDDEN)?;
        if !hosts.contains(&host)
            || !host.host().eq_ignore_ascii_case(origin.host())
            || host.port_u16().unwrap_or(80) != origin.port_u16().unwrap_or(80)
        {
            return Err(StatusCode::FORBIDDEN);
        }
    }
    Ok(next.run(request).await)
}

pub(super) async fn check_loopback(request: Request, next: Next) -> Result<Response, StatusCode> {
    let loopback = request
        .extensions()
        .get::<ConnectInfo<SocketAddr>>()
        .is_some_and(|peer| peer.0.ip().to_canonical().is_loopback());
    if !loopback {
        return Err(StatusCode::FORBIDDEN);
    }
    Ok(next.run(request).await)
}
