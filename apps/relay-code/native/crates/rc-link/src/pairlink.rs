//! What the phone scans or pastes: `relay://pair?v=1&code=…&host=…&id=…&instance=…&direct=…&via=…`
//! (crates/relay-remote/src/pairlink.rs), and the routes it names. A port of `PairLink.kt`.

use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, PartialEq)]
pub struct PairLink {
    pub code: String,
    pub host: String,
    pub host_id: String,
    pub instance: String,
    /// `ws://` addresses on the PC's networks.
    pub direct: Vec<String>,
    /// The join address of the person's own rendezvous, if any.
    pub via: Option<String>,
}

impl PairLink {
    /// Direct addresses first: they keep the lines on the person's own network.
    pub fn routes(&self) -> Vec<Route> {
        let mut r: Vec<Route> = self.direct.iter().map(|u| Route::new(u)).collect();
        if let Some(v) = &self.via {
            r.push(Route { url: v.clone(), kind: RouteKind::Rendezvous });
        }
        r
    }

    pub fn parse(text: &str) -> Option<PairLink> {
        let query = text.trim().strip_prefix("relay://pair?")?;
        let mut params = std::collections::HashMap::new();
        for part in query.split('&') {
            let Some(eq) = part.find('=') else { continue };
            if eq == 0 {
                continue;
            }
            // A link cut mid-escape when copied (`%3`) is no link.
            params.insert(&part[..eq], decode(&part[eq + 1..])?);
        }
        if params.get("v").map(String::as_str) != Some("1") {
            return None;
        }
        let code = params.get("code").filter(|c| !c.trim().is_empty())?.clone();
        let ws = |s: &str| s.starts_with("ws://") || s.starts_with("wss://");
        let direct: Vec<String> =
            params.get("direct").map(|d| d.split(',').map(|s| s.trim().to_string()).filter(|s| ws(s)).collect()).unwrap_or_default();
        let via = params.get("via").filter(|v| ws(v)).cloned();
        if direct.is_empty() && via.is_none() {
            return None;
        }
        let get = |k: &str| params.get(k).cloned().unwrap_or_default();
        Some(PairLink { code, host: get("host"), host_id: get("id"), instance: get("instance"), direct, via })
    }
}

/// Percent-decoding as `URLDecoder` reads it, with `+` kept as `+` (the Kotlin app's rule).
fn decode(s: &str) -> Option<String> {
    let b = s.as_bytes();
    let mut out = Vec::with_capacity(b.len());
    let mut i = 0;
    while i < b.len() {
        if b[i] == b'%' {
            let h = s.get(i + 1..i + 3)?;
            out.push(u8::from_str_radix(h, 16).ok()?);
            i += 3;
        } else {
            out.push(b[i]);
            i += 1;
        }
    }
    String::from_utf8(out).ok()
}

/// What kind of way to the PC a route is.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum RouteKind {
    /// A private LAN address: the phone is on the same network.
    Lan,
    /// A Tailscale address (100.64.0.0/10 or a `.ts.net` name).
    Tailscale,
    Other,
    /// The person's own rendezvous server, which can read the lines (docs/MOBILE.md §6).
    Rendezvous,
}

impl RouteKind {
    pub fn of(url: &str) -> RouteKind {
        let rest = url.split_once("://").map_or(url, |(_, r)| r);
        let authority = rest.split('/').next().unwrap_or("");
        let host = if let Some(h) = authority.strip_prefix('[') {
            h.split(']').next().unwrap_or("")
        } else {
            authority.rsplit_once(':').map_or(authority, |(h, _)| h)
        };
        if host.ends_with(".ts.net") {
            return RouteKind::Tailscale;
        }
        let parts: Vec<u32> = host.split('.').filter_map(|p| p.parse().ok()).collect();
        if parts.len() == 4 {
            let (a, b) = (parts[0], parts[1]);
            if a == 100 && (64..=127).contains(&b) {
                return RouteKind::Tailscale;
            }
            if a == 10 || (a == 172 && (16..=31).contains(&b)) || (a == 192 && b == 168) || a == 127 {
                return RouteKind::Lan;
            }
        }
        if host.ends_with(".local") || host == "localhost" {
            return RouteKind::Lan;
        }
        RouteKind::Other
    }

    fn rank(self) -> u8 {
        match self {
            RouteKind::Lan => 0,
            RouteKind::Other => 1,
            RouteKind::Tailscale => 2,
            RouteKind::Rendezvous => 3,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Route {
    pub url: String,
    pub kind: RouteKind,
}

impl Route {
    pub fn new(url: &str) -> Route {
        // A join address is always the server's, whatever its host looks like.
        let kind = if url.contains("/join/") { RouteKind::Rendezvous } else { RouteKind::of(url) };
        Route { url: url.to_string(), kind }
    }
}

/// The order routes are tried in: the home network first, Tailscale next, the server last; the
/// route that worked last time leads its own rank, so a phone at home does not wait on a stale
/// LAN address from another network.
pub fn order_routes(routes: &[Route], preferred: Option<&str>) -> Vec<Route> {
    let mut seen = std::collections::HashSet::new();
    let mut out: Vec<Route> = routes.iter().filter(|r| seen.insert(r.url.clone())).cloned().collect();
    out.sort_by_key(|r| (r.kind.rank(), if Some(r.url.as_str()) == preferred { 0 } else { 1 }));
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_link_reads_as_the_pc_writes_it() {
        let l = PairLink::parse(
            "relay://pair?v=1&code=AB12CD34&host=desk&id=h1&instance=stable&direct=ws%3A%2F%2F192.168.1.20%3A7420,ws%3A%2F%2F100.70.1.2%3A7420&via=wss%3A%2F%2Fbot.example.org%2Frelay%2Fjoin%2Froom1",
        )
        .unwrap();
        assert_eq!(l.code, "AB12CD34");
        assert_eq!(l.direct, ["ws://192.168.1.20:7420", "ws://100.70.1.2:7420"]);
        let kinds: Vec<_> = l.routes().iter().map(|r| r.kind).collect();
        assert_eq!(kinds, [RouteKind::Lan, RouteKind::Tailscale, RouteKind::Rendezvous]);
    }

    #[test]
    fn broken_links_are_no_links() {
        assert!(PairLink::parse("relay://pair?v=2&code=a&direct=ws%3A%2F%2Fx").is_none());
        assert!(PairLink::parse("relay://pair?v=1&code=a").is_none());
        assert!(PairLink::parse("relay://pair?v=1&code=a&direct=ws%3").is_none());
        assert!(PairLink::parse("https://example.org").is_none());
    }

    #[test]
    fn routes_go_home_first_and_last_good_leads_its_rank() {
        let r = [Route::new("wss://bot.example.org/relay/join/r"), Route::new("ws://100.70.1.2:7420"), Route::new("ws://192.168.1.20:7420"), Route::new("ws://10.0.0.5:7420")];
        let o = order_routes(&r, Some("ws://10.0.0.5:7420"));
        let urls: Vec<_> = o.iter().map(|r| r.url.as_str()).collect();
        assert_eq!(urls, ["ws://10.0.0.5:7420", "ws://192.168.1.20:7420", "ws://100.70.1.2:7420", "wss://bot.example.org/relay/join/r"]);
    }
}
