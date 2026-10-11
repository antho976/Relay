//! What the phone scans: one `relay://pair` link carrying the code and every way to reach the
//! engine, direct addresses first. Rendered as a QR code in the terminal for the camera and
//! printed as text for typing.

use crate::registry::Registry;
use std::net::Ipv4Addr;

/// Percent-encode anything outside the unreserved set. Small enough not to be a dependency.
pub fn encode(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    for b in s.bytes() {
        match b {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' => out.push(b as char),
            _ => out.push_str(&format!("%{b:02X}")),
        }
    }
    out
}

pub struct PairLink {
    pub code: String,
    pub host: String,
    pub host_id: String,
    pub instance: String,
    pub direct: Vec<String>,
    pub via: Option<String>,
}

impl PairLink {
    pub fn build(registry: &Registry, instance: &str, code: &str, port: u16, lan: &[Ipv4Addr]) -> PairLink {
        PairLink {
            code: code.to_string(),
            host: registry.host_name.clone(),
            host_id: registry.host_id.clone(),
            instance: instance.to_string(),
            direct: lan.iter().map(|ip| format!("ws://{ip}:{port}")).collect(),
            via: registry.rendezvous.as_ref().map(crate::tunnel::join_url),
        }
    }

    pub fn to_url(&self) -> String {
        let mut url = format!(
            "relay://pair?v=1&code={}&host={}&id={}&instance={}",
            encode(&self.code),
            encode(&self.host),
            encode(&self.host_id),
            encode(&self.instance)
        );
        if !self.direct.is_empty() {
            url.push_str("&direct=");
            url.push_str(&encode(&self.direct.join(",")));
        }
        if let Some(via) = &self.via {
            url.push_str("&via=");
            url.push_str(&encode(via));
        }
        url
    }

    /// A QR code drawn with half-block characters: light modules on the terminal's dark
    /// background, which is what phone cameras expect to see inverted back.
    pub fn qr(&self) -> Option<String> {
        use qrcode::render::unicode;
        let code = qrcode::QrCode::new(self.to_url().as_bytes()).ok()?;
        Some(
            code.render::<unicode::Dense1x2>()
                .dark_color(unicode::Dense1x2::Light)
                .light_color(unicode::Dense1x2::Dark)
                .quiet_zone(true)
                .build(),
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_link_carries_every_route_direct_first() {
        let mut reg = Registry::fresh();
        reg.host_name = "antho desktop".into();
        reg.host_id = "abc".into();
        reg.set_rendezvous("wss://relay.example.org");
        let link = PairLink::build(&reg, "dev", "ABCD-EFGH", 7420, &[Ipv4Addr::new(192, 168, 1, 20)]);
        let url = link.to_url();
        assert!(url.starts_with("relay://pair?v=1&code=ABCD-EFGH&host=antho%20desktop&id=abc&instance=dev"));
        assert!(url.contains("&direct=ws%3A%2F%2F192.168.1.20%3A7420"));
        let room = &reg.rendezvous.as_ref().unwrap().room;
        assert!(url.contains(&format!("&via=wss%3A%2F%2Frelay.example.org%2Fjoin%2F{room}")));
        assert!(link.qr().unwrap().contains('█'));
    }

    #[test]
    fn an_engine_behind_its_rendezvous_hands_out_the_public_address() {
        let mut reg = Registry::fresh();
        reg.set_rendezvous("ws://127.0.0.1:7430");
        reg.rendezvous.as_mut().unwrap().public = Some("wss://bot.example.org/relay".into());
        let url = PairLink::build(&reg, "stable", "X", 7420, &[]).to_url();
        let room = &reg.rendezvous.as_ref().unwrap().room;
        assert!(url.contains(&format!("&via=wss%3A%2F%2Fbot.example.org%2Frelay%2Fjoin%2F{room}")), "{url}");
        assert!(!url.contains("127.0.0.1"));
        // The engine itself still dials the loopback address.
        assert!(crate::tunnel::host_url(reg.rendezvous.as_ref().unwrap()).starts_with("ws://127.0.0.1:7430/host/"));
    }

    #[test]
    fn a_link_without_a_rendezvous_has_no_via() {
        let reg = Registry::fresh();
        let link = PairLink::build(&reg, "stable", "X", 7420, &[]);
        assert!(!link.to_url().contains("via="));
        assert!(!link.to_url().contains("direct="));
    }
}
