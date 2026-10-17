//! The icons an `icon` node draws: outline geometry on a 24-unit grid as SVG path data, drawn
//! with round caps and joins in the node's stroke. Names are `relay_bus::design::ICONS`; the
//! geometry is `relay_bus::design::icon_path`, which the engine's `design.code` draws too.

/// An icon's path data, or `None` for a name not in the set.
pub fn path(name: &str) -> Option<&'static str> {
    relay_bus::design::icon_path(name)
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
