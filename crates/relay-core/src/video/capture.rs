//! An Android device's screen recorded for a video (`video.capture.android`): Android's demo
//! mode for a clean status bar, `screenrecord` running while the steps tap, swipe and type, the
//! recording stopped with SIGINT (so its file is finished), pulled, written again at a constant
//! frame rate (a screen recording's frames come only when the screen changes), and added to the
//! video's assets with where each step fell and where the picture changes.
//!
//! The device is held for the whole capture with a lease of its own (`device_lease`), as an
//! agent's shell command would take, so a peer's install or run waits; it is let go however the
//! capture ends. Every adb call is `proc::output_with_timeout`, but `screenrecord`, which runs
//! while the steps do, on a thread of its own with its own deadline.

use super::{assets, inspect, media};
use crate::device_lease::{Holder, Kind, Lease};
use crate::engine::Engine;
use relay_bus::error::BusError;
use relay_bus::ops::video::{CaptureMark, CaptureStep, VideoAsset};
use serde_json::json;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::{Duration, Instant};

/// screenrecord stops itself at 180 s.
pub const MOST_SECONDS: f64 = 170.0;
const ADB_WITHIN: Duration = Duration::from_secs(20);
/// How long screenrecord takes to start drawing, about: steps wait this before the first.
const WARM_UP: Duration = Duration::from_millis(900);
const ON_DEVICE: &str = "/sdcard/relay-capture.mp4";

/// One step as the shell command that does it, and how it reads.
pub fn command(step: &CaptureStep) -> Result<(Option<Vec<String>>, String), BusError> {
    let given = [step.tap.is_some(), step.swipe.is_some(), step.text.is_some(), step.key.is_some(), step.wait.is_some(), step.launch.is_some()]
        .iter().filter(|g| **g).count();
    if given != 1 {
        return Err(BusError::invalid("video.capture", "each step is one of tap, swipe, text, key, wait or launch"));
    }
    let px = |v: f64| format!("{}", v.round().max(0.0) as i64);
    if let Some([x, y]) = step.tap {
        return Ok((Some(vec!["input".into(), "tap".into(), px(x), px(y)]), format!("tap {} {}", px(x), px(y))));
    }
    if let Some([x1, y1, x2, y2]) = step.swipe {
        let ms = step.ms.unwrap_or(300).clamp(10, 10_000);
        return Ok((Some(vec!["input".into(), "swipe".into(), px(x1), px(y1), px(x2), px(y2), ms.to_string()]), format!("swipe {} {} to {} {}", px(x1), px(y1), px(x2), px(y2))));
    }
    if let Some(text) = &step.text {
        // `input text` takes %s for a space and refuses the shell's own marks.
        if text.chars().any(|c| "\"'`\\$&|;<>(){}*?!#~".contains(c) || c.is_control()) {
            return Err(BusError::invalid("video.capture", "typed text is letters, digits, spaces and plain punctuation"));
        }
        return Ok((Some(vec!["input".into(), "text".into(), text.replace(' ', "%s")]), format!("type {text:?}")));
    }
    if let Some(key) = &step.key {
        let code = match key.trim().to_ascii_uppercase().as_str() {
            "BACK" => "4".to_string(),
            "HOME" => "3".into(),
            "ENTER" => "66".into(),
            "APP_SWITCH" | "RECENTS" => "187".into(),
            "TAB" => "61".into(),
            "DEL" | "BACKSPACE" => "67".into(),
            n if n.parse::<u32>().is_ok() => n.into(),
            n => return Err(BusError::invalid("video.capture", format!("no key {n}: BACK, HOME, ENTER, APP_SWITCH, TAB, DEL or a keycode"))),
        };
        return Ok((Some(vec!["input".into(), "keyevent".into(), code]), format!("key {}", key.trim())));
    }
    if let Some(seconds) = step.wait {
        if !(0.0..=60.0).contains(&seconds) {
            return Err(BusError::invalid("video.capture", "a wait is 0 to 60 seconds"));
        }
        return Ok((None, format!("wait {seconds} s")));
    }
    let app = step.launch.as_deref().unwrap_or_default().trim();
    Ok((Some(launch(app)?), format!("open {app}")))
}

/// The shell command that opens `app`: an activity with `am start`, a package by its launcher.
pub fn launch(app: &str) -> Result<Vec<String>, BusError> {
    if app.is_empty() || !app.chars().all(|c| c.is_ascii_alphanumeric() || matches!(c, '.' | '_' | '/' | '$')) {
        return Err(BusError::invalid("video.capture", format!("{app:?} is not an app: a package (com.example.app) or an activity (com.example.app/.Main)")));
    }
    Ok(if app.contains('/') {
        vec!["am".into(), "start".into(), "-n".into(), app.into()]
    } else {
        vec!["monkey".into(), "-p".into(), app.into(), "-c".into(), "android.intent.category.LAUNCHER".into(), "1".into()]
    })
}

/// Android's demo mode: the commands that enter it with a clean status bar, and the one that
/// leaves it.
pub fn demo_mode() -> (Vec<Vec<String>>, Vec<String>) {
    let broadcast = |extra: &[&str]| {
        let mut cmd: Vec<String> = ["am", "broadcast", "-a", "com.android.systemui.demo"].iter().map(|s| s.to_string()).collect();
        cmd.extend(extra.iter().map(|s| s.to_string()));
        cmd
    };
    let enter = vec![
        vec!["settings".into(), "put".into(), "global".into(), "sysui_demo_allowed".into(), "1".into()],
        broadcast(&["-e", "command", "enter"]),
        broadcast(&["-e", "command", "clock", "-e", "hhmm", "1230"]),
        broadcast(&["-e", "command", "battery", "-e", "level", "100", "-e", "plugged", "false"]),
        broadcast(&["-e", "command", "network", "-e", "wifi", "show", "-e", "level", "4"]),
        broadcast(&["-e", "command", "network", "-e", "mobile", "show", "-e", "datatype", "none", "-e", "level", "4"]),
        broadcast(&["-e", "command", "notifications", "-e", "visible", "false"]),
    ];
    (enter, broadcast(&["-e", "command", "exit"]))
}

struct Adb {
    path: PathBuf,
    serial: String,
}

impl Adb {
    fn run(&self, args: &[String], within: Duration) -> Result<std::process::Output, BusError> {
        let mut cmd = Command::new(&self.path);
        cmd.arg("-s").arg(&self.serial).args(args);
        crate::proc::output_with_timeout(&mut cmd, within)
            .map_err(|e| BusError::unavailable("video.capture", format!("adb: {e}")))?
            .ok_or_else(|| BusError::unavailable("video.capture", format!("adb {} did not answer within {} s", args.first().map_or("", |s| s), within.as_secs())))
    }

    fn shell(&self, args: &[String]) -> Result<(), BusError> {
        let mut all = vec!["shell".to_string()];
        all.extend(args.iter().cloned());
        let out = self.run(&all, ADB_WITHIN)?;
        if !out.status.success() {
            return Err(BusError::unavailable("video.capture", format!("adb shell {} failed: {}", args.join(" "), String::from_utf8_lossy(&out.stderr).trim())));
        }
        Ok(())
    }
}

/// The device to use: `asked`, or the only one adb lists.
fn device(adb: &Path, asked: Option<&str>) -> Result<String, BusError> {
    let out = crate::proc::output_with_timeout(Command::new(adb).arg("devices"), ADB_WITHIN)
        .map_err(|e| BusError::unavailable("video.capture", format!("adb: {e}")))?
        .ok_or_else(|| BusError::unavailable("video.capture", "adb devices did not answer"))?;
    let ready: Vec<String> = String::from_utf8_lossy(&out.stdout).lines().skip(1)
        .filter_map(|l| l.split_once('\t')).filter(|(_, state)| state.trim() == "device").map(|(s, _)| s.trim().to_string()).collect();
    match asked {
        Some(serial) if ready.iter().any(|r| r == serial) => Ok(serial.to_string()),
        Some(serial) => Err(BusError::invalid("video.capture", format!("no device {serial} is connected and ready")).with_details(json!({"devices": ready}))),
        None => match ready.as_slice() {
            [one] => Ok(one.clone()),
            [] => Err(BusError::unavailable("video.capture", "no Android device is connected").with_hint("connect one (adb devices lists it), or start an emulator")),
            many => Err(BusError::invalid("video.capture", "more than one device is connected: name one").with_details(json!({"devices": many}))),
        },
    }
}

pub struct Captured {
    pub asset: VideoAsset,
    pub seconds: f64,
    pub frames: u32,
    pub marks: Vec<CaptureMark>,
    pub motion: Vec<relay_bus::ops::video::MotionEvent>,
    pub device: String,
}

/// What a capture is asked to do.
pub struct Plan<'a> {
    pub device: Option<&'a str>,
    pub launch: Option<&'a str>,
    pub steps: &'a [CaptureStep],
    pub tail: f64,
    pub fps: f64,
    pub demo: bool,
    pub name: Option<&'a str>,
    pub overwrite: bool,
}

/// Record, for the video at `dir`, as `holder`.
pub fn capture(engine: &Engine, adb_path: &Path, dir: &Path, holder: Holder, plan: &Plan) -> Result<Captured, BusError> {
    let commands: Vec<(Option<Vec<String>>, String)> = plan.steps.iter().map(command).collect::<Result<_, _>>()?;
    let opening = plan.launch.map(launch).transpose()?;
    let planned: f64 = plan.steps.iter().map(|s| s.wait.unwrap_or(0.0) + 0.3).sum::<f64>() + plan.tail + 2.0;
    if planned > MOST_SECONDS {
        return Err(BusError::invalid("video.capture", format!("about {planned:.0} s of steps: a capture is at most {MOST_SECONDS} s (screenrecord's own limit)")));
    }
    let serial = device(adb_path, plan.device)?;
    // The device for the whole capture, and back however it ends.
    let action = "video.capture.android";
    let lease = Lease::new(&serial, holder.clone(), Kind::Claim, action, Some(Duration::from_secs_f64(planned + 120.0)));
    let mut pruned = Vec::new();
    engine.device_leases.acquire(lease.clone(), &mut pruned)?;
    for gone in pruned {
        engine.emit_system(crate::handlers::device_lease::RELEASED, gone.event());
    }
    engine.emit_system(crate::handlers::device_lease::ACQUIRED, lease.event());
    let adb = Adb { path: adb_path.to_path_buf(), serial: serial.clone() };
    let result = record(engine, &adb, dir, plan, opening.as_deref(), &commands);
    if plan.demo {
        let _ = adb.shell(&demo_mode().1);
    }
    for gone in engine.device_leases.release_where(|l| l.device == serial && l.holder == holder && l.kind == Kind::Claim && l.action == action) {
        engine.emit_system(crate::handlers::device_lease::RELEASED, gone.event());
    }
    result
}

fn record(engine: &Engine, adb: &Adb, dir: &Path, plan: &Plan, opening: Option<&[String]>, commands: &[(Option<Vec<String>>, String)]) -> Result<Captured, BusError> {
    if plan.demo {
        for step in demo_mode().0 {
            // A device that will not take demo mode still records.
            let _ = adb.shell(&step);
        }
    }
    if let Some(app) = opening {
        adb.shell(app)?;
        std::thread::sleep(Duration::from_millis(1500));
    }
    let _ = adb.shell(&["rm".into(), "-f".into(), ON_DEVICE.into()]);
    // screenrecord runs beside the steps, on its own thread, until SIGINT finishes its file.
    let (path, serial) = (adb.path.clone(), adb.serial.clone());
    let recorder = std::thread::Builder::new().name("video-capture".into()).spawn(move || {
        let mut cmd = Command::new(path);
        cmd.args(["-s", &serial, "shell", "screenrecord", "--bit-rate", "20000000", "--time-limit", "180", ON_DEVICE]);
        crate::proc::output_with_timeout(&mut cmd, Duration::from_secs(200))
    }).map_err(|e| BusError::internal(format!("capture thread: {e}")))?;
    let started = Instant::now();
    std::thread::sleep(WARM_UP);
    let mut marks = Vec::new();
    let outcome = (|| -> Result<(), BusError> {
        for (i, (cmd, did)) in commands.iter().enumerate() {
            let at = started.elapsed().saturating_sub(WARM_UP / 2).as_secs_f64();
            marks.push(CaptureMark { step: i as u32, did: did.clone(), at_s: (at * 1000.0).round() / 1000.0, frame: (at * plan.fps).round() as u32 });
            match cmd {
                Some(cmd) => adb.shell(cmd)?,
                None => std::thread::sleep(Duration::from_secs_f64(plan.steps[i].wait.unwrap_or(0.0))),
            }
        }
        std::thread::sleep(Duration::from_secs_f64(plan.tail));
        Ok(())
    })();
    let _ = adb.shell(&["pkill".into(), "-INT".into(), "screenrecord".into()]);
    let _ = recorder.join();
    outcome?;
    // The file is finished a moment after the recorder stops.
    std::thread::sleep(Duration::from_millis(800));
    let raw_dir = dir.join("public").join(format!(".capture-{}", uuid::Uuid::new_v4().simple()));
    std::fs::create_dir_all(&raw_dir).map_err(|e| BusError::unavailable("video.folder", format!("{}: {e}", raw_dir.display())))?;
    let raw = raw_dir.join("screen.mp4");
    let pulled = adb.run(&["pull".into(), ON_DEVICE.into(), raw.display().to_string()], Duration::from_secs(120));
    let _ = adb.shell(&["rm".into(), "-f".into(), ON_DEVICE.into()]);
    let finish = || -> Result<Captured, BusError> {
        let pulled = pulled?;
        if !pulled.status.success() || !raw.is_file() {
            return Err(BusError::unavailable("video.capture", format!("the recording could not be pulled: {}", String::from_utf8_lossy(&pulled.stderr).trim())));
        }
        let name = plan.name.map(str::to_string).unwrap_or_else(|| format!("capture-{}", crate::time::now().get(..19).unwrap_or("now").replace([':', 'T'], "-")));
        let even = raw_dir.join(format!("{}.mp4", assets::safe_name(&name).trim_end_matches(".mp4")));
        media::constant_rate(engine, &raw, &even, plan.fps)?;
        let made = json!({"made_by": "video.capture.android", "device": adb.serial, "launch": plan.launch, "steps": marks, "fps": plan.fps,
            "demo": plan.demo, "made_at": crate::time::now()});
        let asset = assets::add(dir, &assets::Add { source: &even, folder: "assets", name: None, overwrite: plan.overwrite, link: false })?;
        let at = PathBuf::from(&asset.path);
        let _ = std::fs::write(assets::sidecar(&at), serde_json::to_vec_pretty(&made).unwrap_or_default());
        let info = inspect::inspect(engine, dir, &at, Some(true))?;
        Ok(Captured {
            asset: assets::describe(dir, &at, asset.renamed_from, asset.replaced, false),
            seconds: info.duration_s.unwrap_or(0.0),
            frames: info.frames.unwrap_or(0),
            marks: marks.clone(),
            motion: info.motion,
            device: adb.serial.clone(),
        })
    };
    let result = finish();
    let _ = std::fs::remove_dir_all(&raw_dir);
    result
}

#[cfg(test)]
mod tests {
    use super::*;

    fn step(f: impl FnOnce(&mut CaptureStep)) -> CaptureStep {
        let mut s = CaptureStep { tap: None, swipe: None, ms: None, text: None, key: None, wait: None, launch: None };
        f(&mut s);
        s
    }

    #[test]
    fn steps_are_the_shell_commands_that_do_them() {
        let (cmd, did) = command(&step(|s| s.tap = Some([540.4, 1200.0]))).unwrap();
        assert_eq!((cmd.unwrap().join(" "), did.as_str()), ("input tap 540 1200".into(), "tap 540 1200"));
        let (cmd, _) = command(&step(|s| { s.swipe = Some([540.0, 1800.0, 540.0, 600.0]); s.ms = Some(450); })).unwrap();
        assert_eq!(cmd.unwrap().join(" "), "input swipe 540 1800 540 600 450");
        assert_eq!(command(&step(|s| s.text = Some("bench 135".into()))).unwrap().0.unwrap().join(" "), "input text bench%s135");
        assert_eq!(command(&step(|s| s.key = Some("back".into()))).unwrap().0.unwrap().join(" "), "input keyevent 4");
        assert_eq!(command(&step(|s| s.wait = Some(1.5))).unwrap().0, None);
        assert_eq!(command(&step(|s| s.launch = Some("com.example.app/.Main".into()))).unwrap().0.unwrap().join(" "), "am start -n com.example.app/.Main");
        assert!(launch("com.example.app").unwrap().contains(&"monkey".to_string()));
        // Nothing reaches the device's shell that its shell would read as its own.
        assert_eq!(command(&step(|s| s.text = Some("a; rm -rf /".into()))).unwrap_err().code, "video.capture");
        assert_eq!(launch("com.x; reboot").unwrap_err().code, "video.capture");
        assert_eq!(command(&step(|s| { s.tap = Some([1.0, 2.0]); s.wait = Some(1.0); })).unwrap_err().code, "video.capture");
        let (enter, exit) = demo_mode();
        assert!(enter[2].join(" ").contains("clock -e hhmm 1230"));
        assert!(exit.join(" ").ends_with("-e command exit"));
    }
}
