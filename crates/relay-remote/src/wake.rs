//! What a phone needs to wake this PC from sleep: each physical network card's MAC address and
//! the broadcast address of the network it is on. The door sends them in its welcome; the phone
//! keeps them and, from the same network, sends a Wake-on-LAN packet when the PC does not answer.
//! Whether the PC wakes is up to its firmware and card (`ethtool -s <card> wol g`); nothing here
//! changes the machine's power settings.

use serde::{Deserialize, Serialize};
use std::path::Path;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct WakeTarget {
    pub mac: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub broadcast: Option<String>,
}

/// The cards a magic packet could reach: physical (`/sys/class/net/<card>/device` exists, so
/// not a bridge, tunnel or Tailscale), up, and on an IPv4 network that has a broadcast address.
pub fn targets() -> Vec<WakeTarget> {
    let mut out: Vec<WakeTarget> = Vec::new();
    for card in if_addrs::get_if_addrs().unwrap_or_default() {
        if card.is_loopback() {
            continue;
        }
        let if_addrs::IfAddr::V4(v4) = &card.addr else { continue };
        let sys = Path::new("/sys/class/net").join(&card.name);
        if !sys.join("device").exists() {
            continue;
        }
        let Some(mac) = std::fs::read_to_string(sys.join("address")).ok().map(|m| m.trim().to_string()) else { continue };
        if !valid_mac(&mac) || out.iter().any(|t| t.mac == mac) {
            continue;
        }
        out.push(WakeTarget { mac, broadcast: v4.broadcast.map(|b| b.to_string()) });
    }
    out
}

fn valid_mac(mac: &str) -> bool {
    let hex: String = mac.chars().filter(|c| *c != ':').collect();
    hex.len() == 12 && hex.chars().all(|c| c.is_ascii_hexdigit()) && hex != "000000000000"
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_mac_is_six_non_zero_octets() {
        assert!(valid_mac("a8:a1:59:12:34:56"));
        assert!(!valid_mac("00:00:00:00:00:00"));
        assert!(!valid_mac("a8:a1:59"));
        assert!(!valid_mac("zz:a1:59:12:34:56"));
    }

    #[test]
    fn targets_never_lists_a_card_twice() {
        let t = targets();
        let mut macs: Vec<_> = t.iter().map(|t| t.mac.clone()).collect();
        macs.sort();
        macs.dedup();
        assert_eq!(macs.len(), t.len());
    }
}
