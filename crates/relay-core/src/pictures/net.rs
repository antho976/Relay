//! The client for the Pictures providers (pictures-SPEC-v2 §8): three fixed hosts, https only,
//! never following a redirect, every body capped and every request under one deadline.
//!
//! No caller passes a URL. Every request is built here from constants and a checked id, except a
//! Pexels picture link, which [`super::parse`] kept only when it is https on images.pexels.com.
//! [`allowed_host`] is checked again before every send. The Pexels key goes only to
//! api.pexels.com, as `Authorization`. The seam is [`Fetch`]: tests and the smoke run's fixtures
//! install their own, and the test instance never builds the real client ([`super::Desk::fetcher`]).

pub const OPENVERSE_HOST: &str = "api.openverse.org";
pub const PEXELS_API_HOST: &str = "api.pexels.com";
pub const PEXELS_IMAGES_HOST: &str = "images.pexels.com";
const HOSTS: [&str; 3] = [OPENVERSE_HOST, PEXELS_API_HOST, PEXELS_IMAGES_HOST];

/// Which of the three hosts `url` is on: https, the host exactly, no port but 443, no user.
pub fn allowed_host(url: &str) -> Option<&'static str> {
    let uri: ureq::http::Uri = url.parse().ok()?;
    if uri.scheme_str() != Some("https") {
        return None;
    }
    let authority = uri.authority()?;
    if authority.as_str().contains('@') || !matches!(authority.port_u16(), None | Some(443)) {
        return None;
    }
    HOSTS.into_iter().find(|host| *host == authority.host())
}

/// Whether `url` may be sent at all.
pub fn host_allowed(url: &str) -> bool {
    allowed_host(url).is_some()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn only_the_three_hosts_over_https_are_allowed() {
        for url in ["https://api.openverse.org/v1/images/", "https://api.pexels.com/v1/search",
            "https://images.pexels.com/photos/1/pexels-photo-1.jpeg?h=350", "https://api.openverse.org:443/v1/images/"] {
            assert!(host_allowed(url), "{url}");
        }
        for url in ["http://api.openverse.org/v1/images/", "https://api.openverse.org.evil.example/v1/",
            "https://evil.api.openverse.org/v1/", "https://127.0.0.1/v1/", "https://api.openverse.org:8443/v1/",
            "https://user@api.openverse.org/v1/", "https://user:pw@api.pexels.com/v1/", "https://API.openverse.org/v1/",
            "api.openverse.org/v1/images/", "ftp://api.pexels.com/", "https://api.openverse.org./v1/", ""] {
            assert!(!host_allowed(url), "{url}");
        }
        assert_eq!(allowed_host("https://images.pexels.com/p.jpeg"), Some(PEXELS_IMAGES_HOST));
    }
}
