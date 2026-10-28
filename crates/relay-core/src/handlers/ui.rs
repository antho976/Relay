//! Phase 9 shell state and durable named layouts.
//!
//! The shell state is the engine's own in-memory model (`Engine::ui`), not a mirror of any
//! window: no client executes `ui.*` ops, so they succeed headless too. The native client reads
//! only the page and a focused pane's `target.session` from `ui.changed`, and shows `ui.toast`;
//! every other pane and window op changes this model alone (BUS.md §6.5).

use crate::engine::{Ctx, Engine, IntoBus};
use relay_bus::ops::ui::*;
use relay_bus::types::{PaneInfo, PaneTarget, WindowInfo};
use relay_bus::Empty;
use rusqlite::{params, OptionalExtension};
use serde_json::{json, Value};
use std::sync::PoisonError;

fn state(engine: &Engine) -> StateOut {
    let ui = engine.ui.lock().unwrap_or_else(PoisonError::into_inner);
    StateOut {
        project_id: ui.project_id,
        page: ui.page,
        panes: ui.panes.clone(),
        focused: ui.focused.clone(),
        windows: ui.windows.clone(),
    }
}

fn valid_name(name: &str) -> Result<&str, relay_bus::BusError> {
    let name = name.trim();
    if name.is_empty() || name.chars().count() > 64 {
        return Err(relay_bus::BusError::invalid(
            "ui.layout_name",
            "layout name must be 1 to 64 characters",
        ));
    }
    Ok(name)
}

/// The arrangement `ui.layout.save` stores when it is given none: the one the native client
/// last saved for the project (`native.layout.current.<id>`), then the older engine key, then a
/// plain grid. Reading only the older key, which nothing writes now, saved the grid every time
/// (RA-419).
fn fallback_state(ctx: &Ctx, project_id: i64) -> Result<Value, relay_bus::BusError> {
    for path in [format!("native.layout.current.{project_id}"), format!("layout.current.{project_id}")] {
        let current = crate::handlers::settings::get(ctx.tx(), Some(&path))?;
        if !current.is_null() {
            return Ok(current);
        }
    }
    Ok(json!({"page":"agents","agent_layout":"grid"}))
}

/// A popout window whose last pane left it is gone; only `main` may stand empty.
fn drop_empty_windows(windows: &mut Vec<WindowInfo>) {
    windows.retain(|window| window.main || !window.panes.is_empty());
}

/// `xdg-open` returns as soon as it has handed the target over; wait for it on a thread of its
/// own so it is reaped rather than left defunct for the engine's lifetime (RA-421).
fn xdg_open(target: &std::ffi::OsStr) {
    match std::process::Command::new("xdg-open").arg(target).spawn() {
        Ok(mut child) => {
            std::thread::spawn(move || {
                let _ = child.wait();
            });
        }
        Err(error) => tracing::warn!(error = %error, "xdg-open"),
    }
}

pub fn register(engine: &mut Engine) {
    engine.register::<State>(|ctx, _| Ok(state(ctx.engine())));
    engine.register::<PageSwitch>(|ctx: &mut Ctx, payload| {
        if let Some(project_id) = payload.project_id {
            crate::handlers::workspace::get_project(ctx.tx(), project_id)?;
        }
        let before = state(ctx.engine());
        {
            let mut ui = ctx.engine().ui.lock().unwrap_or_else(PoisonError::into_inner);
            ui.page = payload.page;
            if payload.project_id.is_some() {
                ui.project_id = payload.project_id;
            }
        }
        ctx.set_undo(
            "ui.page.switch",
            json!({"page":before.page,"project_id":before.project_id}),
            None,
        );
        ctx.emit(
            "ui.changed",
            serde_json::to_value(state(ctx.engine())).bus()?,
        );
        Ok(Empty {})
    });
    engine.register::<PaneOpen>(|ctx: &mut Ctx, payload| {
        let pane = {
            let mut ui = ctx.engine().ui.lock().unwrap_or_else(PoisonError::into_inner);
            let pane = format!("pane-{}", ui.next_pane);
            ui.next_pane += 1;
            for item in &mut ui.panes {
                item.focused = false;
            }
            ui.panes.push(PaneInfo {
                pane: pane.clone(),
                kind: payload.kind,
                target: payload.target.unwrap_or(PaneTarget {
                    session: None,
                    path: None,
                    sha: None,
                    note_id: None,
                    run_id: None,
                    mirror_id: None,
                }),
                window_id: "main".into(),
                focused: true,
            });
            ui.focused = Some(pane.clone());
            if let Some(main) = ui.windows.iter_mut().find(|window| window.main) {
                main.panes.push(pane.clone());
            }
            pane
        };
        ctx.emit(
            "ui.changed",
            serde_json::to_value(state(ctx.engine())).bus()?,
        );
        Ok(PaneOpenOut { pane })
    });
    engine.register::<PaneClose>(|ctx: &mut Ctx, payload| {
        let mut ui = ctx.engine().ui.lock().unwrap_or_else(PoisonError::into_inner);
        let before = ui.panes.len();
        ui.panes.retain(|pane| pane.pane != payload.pane);
        if before == ui.panes.len() {
            return Err(relay_bus::BusError::not_found(
                "ui.pane_not_found",
                format!("no pane {}", payload.pane),
            ));
        }
        for window in &mut ui.windows {
            window.panes.retain(|pane| pane != &payload.pane);
        }
        drop_empty_windows(&mut ui.windows);
        if ui.focused.as_deref() == Some(payload.pane.as_str()) {
            ui.focused = ui.panes.last().map(|pane| pane.pane.clone());
            if let Some(last) = ui.panes.last_mut() {
                last.focused = true;
            }
        }
        drop(ui);
        ctx.emit(
            "ui.changed",
            serde_json::to_value(state(ctx.engine())).bus()?,
        );
        Ok(Empty {})
    });
    engine.register::<PaneFocus>(|ctx: &mut Ctx, payload| {
        let mut ui = ctx.engine().ui.lock().unwrap_or_else(PoisonError::into_inner);
        if !ui.panes.iter().any(|pane| pane.pane == payload.pane) {
            return Err(relay_bus::BusError::not_found(
                "ui.pane_not_found",
                format!("no pane {}", payload.pane),
            ));
        }
        for pane in &mut ui.panes {
            pane.focused = pane.pane == payload.pane;
        }
        ui.focused = Some(payload.pane);
        drop(ui);
        ctx.emit(
            "ui.changed",
            serde_json::to_value(state(ctx.engine())).bus()?,
        );
        Ok(Empty {})
    });
    engine.register::<PaneMove>(|ctx: &mut Ctx, payload| {
        if !matches!(
            payload.edge.as_str(),
            "top" | "bottom" | "left" | "right" | "center"
        ) {
            return Err(relay_bus::BusError::invalid(
                "ui.pane_edge",
                "edge must be top, bottom, left, right, or center",
            ));
        }
        let mut ui = ctx.engine().ui.lock().unwrap_or_else(PoisonError::into_inner);
        let from = ui
            .panes
            .iter()
            .position(|pane| pane.pane == payload.pane)
            .ok_or_else(|| {
                relay_bus::BusError::not_found(
                    "ui.pane_not_found",
                    format!("no pane {}", payload.pane),
                )
            })?;
        if !ui.panes.iter().any(|pane| pane.pane == payload.to) {
            return Err(relay_bus::BusError::not_found(
                "ui.pane_not_found",
                format!("no pane {}", payload.to),
            ));
        }
        if payload.pane == payload.to {
            return Err(relay_bus::BusError::invalid(
                "ui.pane_move",
                "a pane cannot be moved relative to itself",
            ));
        }
        // The model is an ordered list per window, so an edge says which side of the target
        // the pane lands on: before it for top and left, after it for bottom, right and center
        // (stacked onto it). The pane joins the target's window (RA-420).
        let after = matches!(payload.edge.as_str(), "bottom" | "right" | "center");
        let mut pane = ui.panes.remove(from);
        let (to, window_id) = ui.panes.iter().enumerate()
            .find(|(_, item)| item.pane == payload.to)
            .map(|(at, item)| (at, item.window_id.clone()))
            .unwrap_or((ui.panes.len(), pane.window_id.clone()));
        pane.window_id = window_id.clone();
        let at = (to + usize::from(after)).min(ui.panes.len());
        ui.panes.insert(at, pane);
        for window in &mut ui.windows {
            window.panes.retain(|item| item != &payload.pane);
            if window.window_id == window_id {
                let at = window.panes.iter().position(|item| item == &payload.to).map_or(window.panes.len(), |at| at + usize::from(after));
                window.panes.insert(at, payload.pane.clone());
            }
        }
        drop_empty_windows(&mut ui.windows);
        drop(ui);
        // The whole model, as every other ui op sends it, and the move itself as D40 named it.
        let mut changed = serde_json::to_value(state(ctx.engine())).bus()?;
        changed["move"] = json!({"pane":payload.pane,"to":payload.to,"edge":payload.edge});
        ctx.emit("ui.changed", changed);
        Ok(Empty {})
    });
    engine.register::<LayoutList>(|ctx, payload| {
        crate::handlers::workspace::get_project(ctx.tx(), payload.project_id)?;
        let mut stmt = ctx
            .tx()
            .prepare_cached("SELECT name FROM ui_layouts WHERE project_id=?1 ORDER BY name COLLATE NOCASE")
            .bus()?;
        let layouts = stmt
            .query_map([payload.project_id], |row| row.get(0))
            .bus()?
            .collect::<rusqlite::Result<Vec<_>>>()
            .bus()?;
        Ok(LayoutListOut { layouts })
    });
    engine.register::<LayoutSave>(|ctx: &mut Ctx, payload| {
        crate::handlers::workspace::get_project(ctx.tx(), payload.project_id)?;
        let name = valid_name(&payload.name)?.to_string();
        let layout = match payload.state {
            Some(value) => value,
            None => fallback_state(ctx, payload.project_id)?,
        };
        let before = ctx
            .tx()
            .query_row(
                "SELECT state FROM ui_layouts WHERE project_id=?1 AND name=?2",
                params![payload.project_id, name],
                |row| row.get::<_, String>(0),
            )
            .optional()
            .bus()?;
        ctx.tx().execute(
            "INSERT INTO ui_layouts(project_id,name,state,created_at,updated_at) VALUES (?1,?2,?3,?4,?4)
             ON CONFLICT(project_id,name) DO UPDATE SET state=excluded.state,updated_at=excluded.updated_at",
            params![payload.project_id,name,serde_json::to_string(&layout).bus()?,ctx.now],
        ).bus()?;
        if let Some(before) = before {
            ctx.set_undo("ui.layout.save", json!({"project_id":payload.project_id,"name":name,"state":serde_json::from_str::<Value>(&before).unwrap_or(Value::Null)}), None);
        } else {
            ctx.set_undo("ui.layout.delete", json!({"project_id":payload.project_id,"name":name}), None);
        }
        ctx.set_project(payload.project_id);
        ctx.emit("layout.changed", json!({"action":"saved","project_id":payload.project_id,"name":name,"state":layout}));
        Ok(Empty {})
    });
    engine.register::<LayoutApply>(|ctx: &mut Ctx, payload| {
        crate::handlers::workspace::get_project(ctx.tx(), payload.project_id)?;
        let name = valid_name(&payload.name)?;
        let raw = ctx
            .tx()
            .query_row(
                "SELECT state FROM ui_layouts WHERE project_id=?1 AND name=?2",
                params![payload.project_id, name],
                |row| row.get::<_, String>(0),
            )
            .optional()
            .bus()?
            .ok_or_else(|| {
                relay_bus::BusError::not_found("ui.layout_not_found", format!("no layout {name:?}"))
            })?;
        let layout: Value = serde_json::from_str(&raw).bus()?;
        ctx.set_project(payload.project_id);
        ctx.emit(
            "layout.changed",
            json!({"action":"applied","project_id":payload.project_id,"name":name,"state":layout}),
        );
        Ok(Empty {})
    });
    engine.register::<LayoutDelete>(|ctx: &mut Ctx, payload| {
        let name = valid_name(&payload.name)?.to_string();
        let before = ctx.tx().query_row(
            "SELECT state FROM ui_layouts WHERE project_id=?1 AND name=?2",
            params![payload.project_id,name], |row| row.get::<_, String>(0),
        ).optional().bus()?.ok_or_else(|| relay_bus::BusError::not_found("ui.layout_not_found", format!("no layout {name:?}")))?;
        ctx.tx().execute("DELETE FROM ui_layouts WHERE project_id=?1 AND name=?2", params![payload.project_id,name]).bus()?;
        ctx.set_undo("ui.layout.save", json!({"project_id":payload.project_id,"name":name,"state":serde_json::from_str::<Value>(&before).unwrap_or(Value::Null)}), None);
        ctx.set_project(payload.project_id);
        ctx.emit("layout.changed", json!({"action":"deleted","project_id":payload.project_id,"name":name}));
        Ok(Empty {})
    });
    engine.register::<WindowPopout>(|ctx: &mut Ctx, payload| {
        let mut ui = ctx.engine().ui.lock().unwrap_or_else(PoisonError::into_inner);
        let position = ui
            .panes
            .iter()
            .position(|pane| pane.pane == payload.pane)
            .ok_or_else(|| {
                relay_bus::BusError::not_found(
                    "ui.pane_not_found",
                    format!("no pane {}", payload.pane),
                )
            })?;
        let window_id = format!("popout-{}", ui.next_window);
        ui.next_window += 1;
        ui.panes[position].window_id = window_id.clone();
        for window in &mut ui.windows {
            window.panes.retain(|pane| pane != &payload.pane);
        }
        drop_empty_windows(&mut ui.windows);
        ui.windows.push(WindowInfo {
            window_id: window_id.clone(),
            main: false,
            panes: vec![payload.pane],
        });
        drop(ui);
        ctx.emit(
            "ui.changed",
            serde_json::to_value(state(ctx.engine())).bus()?,
        );
        Ok(PopoutOut { window_id })
    });
    engine.register::<WindowClose>(|ctx: &mut Ctx, payload| {
        if payload.window_id == "main" {
            return Err(relay_bus::BusError::conflict(
                "ui.main_window",
                "the main window cannot be closed through ui.window.close",
            ));
        }
        let mut ui = ctx.engine().ui.lock().unwrap_or_else(PoisonError::into_inner);
        let index = ui
            .windows
            .iter()
            .position(|window| window.window_id == payload.window_id)
            .ok_or_else(|| {
                relay_bus::BusError::not_found(
                    "ui.window_not_found",
                    format!("no window {}", payload.window_id),
                )
            })?;
        let panes = ui.windows.remove(index).panes;
        for pane in &mut ui.panes {
            if panes.contains(&pane.pane) {
                pane.window_id = "main".into();
            }
        }
        if let Some(main) = ui.windows.iter_mut().find(|window| window.main) {
            main.panes.extend(panes);
        }
        drop(ui);
        ctx.emit(
            "ui.changed",
            serde_json::to_value(state(ctx.engine())).bus()?,
        );
        Ok(Empty {})
    });
    engine.register::<WindowList>(|ctx, _| {
        Ok(WindowListOut {
            windows: ctx.engine().ui.lock().unwrap_or_else(PoisonError::into_inner).windows.clone(),
        })
    });
    engine.register::<Toast>(|ctx: &mut Ctx, payload| {
        ctx.emit("ui.toast", json!({"text":payload.text,"level":payload.level.unwrap_or_else(||"info".into()),"ttl_ms":payload.ttl_ms.unwrap_or(4000)}));
        Ok(Empty {})
    });
    engine.register::<OsReveal>(|ctx: &mut Ctx, payload| {
        let path = std::path::PathBuf::from(payload.path);
        if !path.is_absolute() {
            return Err(relay_bus::BusError::invalid(
                "os.path",
                "path must be absolute",
            ));
        }
        // Reveal shows a folder: the path itself when it is one, else the folder holding it.
        // Opening `parent()` blindly handed a file (`/x/app.jar` for `/x/app.jar/y`) to its
        // desktop handler instead (RA-421).
        let target = if path.is_dir() {
            path
        } else {
            match path.parent() {
                Some(parent) if parent.is_dir() => parent.to_path_buf(),
                _ => {
                    return Err(relay_bus::BusError::not_found(
                        "os.path",
                        format!("no folder to reveal for {}", path.display()),
                    ))
                }
            }
        };
        ctx.after_commit(move |_| xdg_open(target.as_os_str()));
        Ok(Empty {})
    });
    engine.register::<OsOpenUrl>(|ctx: &mut Ctx, payload| {
        if !(payload.url.starts_with("https://") || payload.url.starts_with("http://")) {
            return Err(relay_bus::BusError::invalid(
                "os.url",
                "only http and https URLs are supported",
            ));
        }
        ctx.after_commit(move |_| xdg_open(payload.url.as_ref()));
        Ok(Empty {})
    });
}
