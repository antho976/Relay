//! Helpers every smoke part shares (RA-720): one widget lookup, one bounded wait, one
//! error contract, and a click a person could have made.
use gtk::prelude::*;
use gtk4 as gtk;
use std::time::{Duration, Instant};

/// How long a part waits for the UI to reach the state its next step needs.
pub(crate) const WAIT: Duration = Duration::from_secs(5);

pub(crate) fn named(root: &impl IsA<gtk::Widget>, name: &str) -> Option<gtk::Widget> {
    let root = root.as_ref();
    if root.widget_name() == name {
        return Some(root.clone());
    }
    let mut child = root.first_child();
    while let Some(widget) = child {
        if let Some(found) = named(&widget, name) {
            return Some(found);
        }
        child = widget.next_sibling();
    }
    None
}

/// The first widget under `root`, depth first, that passes `test`.
pub(crate) fn find(root: &gtk::Widget, test: &impl Fn(&gtk::Widget) -> bool) -> Option<gtk::Widget> {
    if test(root) {
        return Some(root.clone());
    }
    let mut child = root.first_child();
    while let Some(widget) = child {
        if let Some(found) = find(&widget, test) {
            return Some(found);
        }
        child = widget.next_sibling();
    }
    None
}

pub(crate) fn require(condition: bool, reason: &str) -> Result<(), String> {
    if condition {
        Ok(())
    } else {
        Err(reason.to_string())
    }
}

/// Polls `ready` until it holds or [`WAIT`] passes. The budget is wall time, so a loaded
/// machine that stretches each poll does not stretch the budget with it.
pub(crate) async fn wait_for(ready: impl FnMut() -> bool, reason: &str) -> Result<(), String> {
    wait_within(WAIT, ready, reason).await
}

pub(crate) async fn wait_within(
    budget: Duration,
    mut ready: impl FnMut() -> bool,
    reason: &str,
) -> Result<(), String> {
    let deadline = Instant::now() + budget;
    loop {
        if ready() {
            return Ok(());
        }
        if Instant::now() >= deadline {
            return Err(format!("Timed out after {budget:?}: {reason}"));
        }
        glib::timeout_future(Duration::from_millis(25)).await;
    }
}

/// True once `name` is a button a person could press: present, shown and sensitive.
pub(crate) fn clickable(root: &impl IsA<gtk::Widget>, name: &str) -> bool {
    named(root, name).is_some_and(|w| (w.is::<gtk::Button>() || w.is::<gtk::MenuButton>()) && w.is_mapped() && w.is_sensitive())
}

/// Clicks the button `name`. `emit_clicked` runs the handlers of a disabled or hidden
/// button too, so this refuses one a person could not press (RA-716).
pub(crate) fn click(root: &impl IsA<gtk::Widget>, name: &str) -> Result<(), String> {
    let found = named(root, name).ok_or_else(|| format!("Missing control: {name}"))?;
    // A menu button (a picker) opens its menu, as a press would.
    if let Some(menu) = found.downcast_ref::<gtk::MenuButton>() {
        require(menu.is_mapped() && menu.is_sensitive(), &format!("{name} cannot be pressed"))?;
        menu.popup();
        return Ok(());
    }
    let key = found
        .downcast::<gtk::Button>()
        .map_err(|_| format!("Not a button: {name}"))?;
    press(&key, name)
}

pub(crate) fn press(key: &gtk::Button, name: &str) -> Result<(), String> {
    require(key.is_mapped(), &format!("{name} is not shown, so it cannot be clicked"))?;
    require(key.is_sensitive(), &format!("{name} is disabled, so it cannot be clicked"))?;
    key.emit_clicked();
    Ok(())
}

/// The grouped toggle keys of a choice row (launch.rs `Choice`), in order.
fn choice_keys(root: &impl IsA<gtk::Widget>, row: &str) -> Result<Vec<gtk::ToggleButton>, String> {
    let row = named(root, row).ok_or_else(|| format!("Missing choice row: {row}"))?;
    let mut keys = Vec::new();
    let mut child = row.first_child();
    while let Some(widget) = child {
        child = widget.next_sibling();
        if let Ok(key) = widget.downcast::<gtk::ToggleButton>() {
            keys.push(key);
        }
    }
    Ok(keys)
}

/// Presses key `index` of the choice row `row`, refusing one a person could not press.
pub(crate) fn choose(root: &impl IsA<gtk::Widget>, row: &str, index: usize) -> Result<(), String> {
    let keys = choice_keys(root, row)?;
    let key = keys.get(index).ok_or_else(|| format!("{row} has no key {index}"))?;
    press(key.upcast_ref(), &format!("{row} key {index}"))?;
    require(key.is_active(), &format!("{row} key {index} did not take the choice"))
}

/// Which key of the choice row `row` is chosen.
pub(crate) fn chosen(root: &impl IsA<gtk::Widget>, row: &str) -> Result<usize, String> {
    let keys = choice_keys(root, row)?;
    let mut active = keys.iter().enumerate().filter(|(_, key)| key.is_active()).map(|(i, _)| i);
    match (active.next(), active.next()) {
        (Some(index), None) => Ok(index),
        _ => Err(format!("{row} must have exactly one key chosen")),
    }
}

/// What the production `DragSource` on `row` hands a drop target: its `prepare` handler
/// runs, so a change to the payload format reaches the drop under test (RA-722).
pub(crate) fn drag_payload(row: &gtk::Widget) -> Result<glib::Value, String> {
    let controllers = row.observe_controllers();
    let source = (0..controllers.n_items())
        .filter_map(|i| controllers.item(i))
        .find_map(|c| c.downcast::<gtk::DragSource>().ok())
        .ok_or_else(|| format!("{} has no drag source", row.widget_name()))?;
    let content = source
        .emit_by_name::<Option<gtk::gdk::ContentProvider>>("prepare", &[&0.0_f64, &0.0_f64])
        .ok_or_else(|| format!("{} refused to start a drag", row.widget_name()))?;
    content
        .value(String::static_type())
        .map_err(|e| format!("{} drag content: {e}", row.widget_name()))
}

/// Drops `payload` on the `DropTarget` of `row`, returning whether the target accepted it.
pub(crate) fn drop_on(row: &gtk::Widget, payload: glib::Value) -> Result<bool, String> {
    let controllers = row.observe_controllers();
    let target = (0..controllers.n_items())
        .filter_map(|i| controllers.item(i))
        .find_map(|c| c.downcast::<gtk::DropTarget>().ok())
        .ok_or_else(|| format!("{} has no drop target", row.widget_name()))?;
    Ok(target.emit_by_name::<bool>("drop", &[&glib::BoxedValue(payload), &0.0_f64, &0.0_f64]))
}
