//! The icons an `icon` node draws: outline geometry on a 24-unit grid as SVG path data, drawn
//! with round caps and joins in the node's stroke. Names are `relay_bus::design::ICONS`.
//!
//! The phone draws the same set from a copy
//! (`apps/relay-android/core/src/main/resources/design/icons.json`, read by `Icons.kt` in
//! `apps/relay-android/core/src/main/kotlin/com/quietsoftware/relay/core/design/`). A change here
//! changes it there in the same piece of work.

/// An icon's path data, or `None` for a name not in the set.
pub fn path(name: &str) -> Option<&'static str> {
    Some(match name {
        "home" => "M3 10.5 12 3l9 7.5M5 9v11h5v-6h4v6h5V9",
        "search" => "M4 11a7 7 0 1 0 14 0a7 7 0 1 0-14 0M21 21l-5-5",
        "settings" => "M9 12a3 3 0 1 0 6 0a3 3 0 1 0-6 0M5 12a7 7 0 1 0 14 0a7 7 0 1 0-14 0M12 2v3M12 19v3M2 12h3M19 12h3M4.9 4.9l2.2 2.2M16.9 16.9l2.2 2.2M4.9 19.1l2.2-2.2M16.9 7.1l2.2-2.2",
        "user" => "M8 7a4 4 0 1 0 8 0a4 4 0 1 0-8 0M4 21v-1a6 6 0 0 1 6-6h4a6 6 0 0 1 6 6v1",
        "users" => "M5 8a3.5 3.5 0 1 0 7 0a3.5 3.5 0 1 0-7 0M2 20v-1a5 5 0 0 1 5-5h3a5 5 0 0 1 5 5v1M16 4.5a3.5 3.5 0 0 1 0 7M19 14a5 5 0 0 1 3 5v1",
        "bell" => "M6 16v-5a6 6 0 0 1 12 0v5l2 2H4zM10 21h4",
        "mail" => "M3 6h18v12H3zM3 7l9 6 9-6",
        "heart" => "M12 20s-8-4.5-8-10a4.5 4.5 0 0 1 8-2.8A4.5 4.5 0 0 1 20 10c0 5.5-8 10-8 10z",
        "star" => "M12 3l2.8 5.7 6.2.9-4.5 4.4 1.1 6.2L12 17.3l-5.6 2.9 1.1-6.2L3 9.6l6.2-.9z",
        "plus" => "M12 5v14M5 12h14",
        "minus" => "M5 12h14",
        "x" => "M6 6l12 12M18 6 6 18",
        "check" => "M5 12.5l4.5 4.5L19 7",
        "chevron-left" => "M15 6l-6 6 6 6",
        "chevron-right" => "M9 6l6 6-6 6",
        "chevron-up" => "M6 15l6-6 6 6",
        "chevron-down" => "M6 9l6 6 6-6",
        "arrow-left" => "M19 12H5M11 6l-6 6 6 6",
        "arrow-right" => "M5 12h14M13 6l6 6-6 6",
        "arrow-up" => "M12 19V5M6 11l6-6 6 6",
        "arrow-down" => "M12 5v14M6 13l6 6 6-6",
        "menu" => "M4 6h16M4 12h16M4 18h16",
        "more-horizontal" => "M5 12a1 1 0 1 0 2 0a1 1 0 1 0-2 0M11 12a1 1 0 1 0 2 0a1 1 0 1 0-2 0M17 12a1 1 0 1 0 2 0a1 1 0 1 0-2 0",
        "more-vertical" => "M11 5a1 1 0 1 0 2 0a1 1 0 1 0-2 0M11 12a1 1 0 1 0 2 0a1 1 0 1 0-2 0M11 19a1 1 0 1 0 2 0a1 1 0 1 0-2 0",
        "trash" => "M4 7h16M10 11v6M14 11v6M6 7l1 13h10l1-13M9 7V4h6v3",
        "edit" => "M4 20h4L19 9l-4-4L4 16zM13.5 6.5l4 4",
        "copy" => "M8 8h12v12H8zM16 8V4H4v12h4",
        "download" => "M12 4v11M7 10l5 5 5-5M4 20h16",
        "upload" => "M12 16V5M7 10l5-5 5 5M4 20h16",
        "share" => "M15.5 5a2.5 2.5 0 1 0 5 0a2.5 2.5 0 1 0-5 0M3.5 12a2.5 2.5 0 1 0 5 0a2.5 2.5 0 1 0-5 0M15.5 19a2.5 2.5 0 1 0 5 0a2.5 2.5 0 1 0-5 0M8.2 10.8l7.6-4.6M8.2 13.2l7.6 4.6",
        "link" => "M10 14a4 4 0 0 0 5.7 0l3-3a4 4 0 0 0-5.7-5.7l-1 1M14 10a4 4 0 0 0-5.7 0l-3 3a4 4 0 0 0 5.7 5.7l1-1",
        "lock" => "M5 11h14v10H5zM8 11V7a4 4 0 0 1 8 0v4",
        "unlock" => "M5 11h14v10H5zM8 11V7a4 4 0 0 1 7.5-2",
        "eye" => "M2 12s3.5-7 10-7 10 7 10 7-3.5 7-10 7S2 12 2 12zM9 12a3 3 0 1 0 6 0a3 3 0 1 0-6 0",
        "eye-off" => "M3 3l18 18M10.6 5.1A10 10 0 0 1 12 5c6.5 0 10 7 10 7a17 17 0 0 1-3 3.9M6.6 6.6C3.7 8.4 2 12 2 12s3.5 7 10 7a9.6 9.6 0 0 0 5.4-1.6M9.9 9.9a3 3 0 0 0 4.2 4.2",
        "calendar" => "M4 6h16v15H4zM4 10h16M8 3v4M16 3v4",
        "clock" => "M3 12a9 9 0 1 0 18 0a9 9 0 1 0-18 0M12 7v5l3 2",
        "camera" => "M3 8h4l2-3h6l2 3h4v12H3zM8.5 13.5a3.5 3.5 0 1 0 7 0a3.5 3.5 0 1 0-7 0",
        "image" => "M3 5h18v14H3zM3 16l5-5 4 4 3-3 6 6M14.5 9a1.5 1.5 0 1 0 3 0a1.5 1.5 0 1 0-3 0",
        "file" => "M6 3h8l5 5v13H6zM14 3v5h5",
        "folder" => "M3 6h6l2 2h10v11H3z",
        "shopping-cart" => "M3 4h3l2.5 11h10L21 8H7M9 20a1 1 0 1 0 2 0a1 1 0 1 0-2 0M16 20a1 1 0 1 0 2 0a1 1 0 1 0-2 0",
        "credit-card" => "M3 6h18v12H3zM3 10h18M7 15h4",
        "globe" => "M3 12a9 9 0 1 0 18 0a9 9 0 1 0-18 0M3 12h18M12 3c3 3 3 15 0 18M12 3c-3 3-3 15 0 18",
        "map-pin" => "M12 21s-7-6.5-7-12a7 7 0 0 1 14 0c0 5.5-7 12-7 12zM9.5 9a2.5 2.5 0 1 0 5 0a2.5 2.5 0 1 0-5 0",
        "phone" => "M5 4h4l2 5-2.5 1.5a11 11 0 0 0 5 5L15 13l5 2v4a2 2 0 0 1-2 2A16 16 0 0 1 3 6a2 2 0 0 1 2-2z",
        "message" => "M4 5h16v11H9l-5 4z",
        "send" => "M21 3 10 14M21 3l-7 18-4-7-7-4z",
        "filter" => "M3 5h18l-7 8v6l-4 2v-8z",
        "sliders" => "M4 6h10M18 6h2M4 12h4M12 12h8M4 18h12M20 18h.01M14 4v4M8 10v4M16 16v4",
        "grid" => "M4 4h7v7H4zM13 4h7v7h-7zM4 13h7v7H4zM13 13h7v7h-7z",
        "list" => "M9 6h11M9 12h11M9 18h11M4 6h1M4 12h1M4 18h1",
        "play" => "M7 4l13 8-13 8z",
        "pause" => "M7 4h3v16H7zM14 4h3v16h-3z",
        "refresh" => "M20 11a8 8 0 0 0-14.7-4.3L4 9M4 4v5h5M4 13a8 8 0 0 0 14.7 4.3L20 15M20 20v-5h-5",
        "log-out" => "M9 21H5V3h4M16 17l5-5-5-5M21 12H9",
        "bookmark" => "M6 3h12v18l-6-4-6 4z",
        "info" => "M3 12a9 9 0 1 0 18 0a9 9 0 1 0-18 0M12 11v6M12 7.5v.5",
        "alert" => "M12 3 2 20h20zM12 10v4M12 17v.5",
        "help" => "M3 12a9 9 0 1 0 18 0a9 9 0 1 0-18 0M9.5 9a2.5 2.5 0 0 1 5 .5c0 1.5-2.5 2-2.5 3.5M12 17v.5",
        "sun" => "M8 12a4 4 0 1 0 8 0a4 4 0 1 0-8 0M12 2v2M12 20v2M2 12h2M20 12h2M4.9 4.9l1.4 1.4M17.7 17.7l1.4 1.4M4.9 19.1l1.4-1.4M17.7 6.3l1.4-1.4",
        "moon" => "M20 14.5A8 8 0 1 1 9.5 4a6.5 6.5 0 0 0 10.5 10.5z",
        "wifi" => "M2 9a15 15 0 0 1 20 0M5 12.5a10 10 0 0 1 14 0M8.5 16a5 5 0 0 1 7 0M12 19.5v.5",
        "battery" => "M3 7h15v10H3zM21 10v4M6 10v4M9 10v4",
        "terminal" => "M3 4h18v16H3zM7 9l3 3-3 3M12 15h5",
        "code" => "M8 7l-5 5 5 5M16 7l5 5-5 5M14 4l-4 16",
        "zap" => "M13 2 4 14h7l-1 8 9-12h-7z",
        "cloud" => "M7 19h10a4.5 4.5 0 0 0 .6-9A6 6 0 0 0 6 9.5 4.8 4.8 0 0 0 7 19z",
        "git-branch" => "M4 19a2 2 0 1 0 4 0a2 2 0 1 0-4 0M16 7a2 2 0 1 0 4 0a2 2 0 1 0-4 0M6 3v14M18 9a9 9 0 0 1-9 9H8",
        "bar-chart" => "M4 20h16M7 16v-5M12 16V6M17 16v-8",
        _ => return None,
    })
}

#[cfg(test)]
mod tests {
    use relay_bus::design::ICONS;

    #[test]
    fn every_icon_name_has_geometry() {
        for name in ICONS {
            assert!(super::path(name).is_some(), "{name} has no path");
        }
        assert!(super::path("no-such-icon").is_none());
    }
}
