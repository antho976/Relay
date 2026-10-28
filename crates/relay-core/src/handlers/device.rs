//! `device.*` (phase 10): request-driven ADB discovery, H.264 mirrors, Gradle deploys,
//! and bounded logcat streams. Nothing starts until a bus op asks for it.

use crate::device::{mirror_server, DeviceWatchRuntime, MirrorRuntime, MirrorRuntimeConfig, MirrorState, RunRuntime};
use crate::engine::{Ctx, Engine, IntoBus};
use crate::handlers::workspace::get_project;
use crate::mirror;
use crate::paths::Instance;
use crate::worktree;
use relay_bus::error::BusError;
use relay_bus::ops::device::*;
use relay_bus::types::{Avd, Device, DeviceKind, Id, Run};
use relay_bus::Empty;
use rusqlite::{params, OptionalExtension, Row};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use sha2::{Digest, Sha256};
use std::fs::{self, OpenOptions};
use std::io::{BufRead, BufReader, Read, Write};
use std::net::TcpStream;
use std::os::unix::fs::{OpenOptionsExt, PermissionsExt};
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::sync::atomic::Ordering;
use std::sync::{Arc, Mutex, PoisonError};
use std::time::{Duration, Instant};

pub fn register(e: &mut Engine) {
    // `adb devices` starts the adb server when it is not up: seconds of subprocess, and it used
    // to own the store mutex for all of it. One short read for the configured path, then adb with
    // nothing locked (D144).
    e.register_unlocked::<List>(|ctx, _| {
        let adb = ctx.read(adb_path)?;
        let mut devices = list_with_adb(&adb)?;
        // Whoever is using each device, so an agent knows before it tries (device_lease).
        let released = ctx.read(|conn| Ok(super::device_lease::prune(ctx.engine(), conn)))?;
        for lease in released { ctx.emit(super::device_lease::RELEASED, lease.event()); }
        for device in &mut devices {
            device.lease = ctx.engine().device_leases.holder_of(&device.serial).map(|lease| lease.view());
        }
        Ok(ListOut { devices })
    });

    e.register::<Watch>(|ctx: &mut Ctx, p| {
        if p.on {
            let adb = adb_path(ctx.tx())?;
            let runtime = {
                let mut watch = ctx.engine().device_watch.lock().unwrap();
                watch.clients += 1;
                if watch.runtime.is_none() {
                    let runtime = Arc::new(DeviceWatchRuntime::default());
                    watch.runtime = Some(runtime.clone());
                    Some(runtime)
                } else { None }
            };
            if let Some(runtime) = runtime {
                ctx.after_commit(move |engine| { let engine = Arc::downgrade(&engine); std::thread::spawn(move || device_watch_worker(engine, runtime, adb)); });
            }
        } else {
            // Stopped inside the lock, as `WatchLease::drop` does: `clients == 0` means stopped.
            let mut watch = ctx.engine().device_watch.lock().unwrap();
            watch.clients = watch.clients.saturating_sub(1);
            if watch.clients == 0 {
                if let Some(runtime) = watch.runtime.take() { runtime.stop(); }
            }
        }
        Ok(Empty {})
    });

    // `adb devices` and `wm size` are two subprocesses, seconds when the adb server is cold;
    // both run in the prepare phase with nothing locked, each under a deadline (D149), as does
    // hashing the server jar.
    e.register_staged::<MirrorStart, _>(|ctx, p| {
        if ctx.engine().instance != Instance::Test {
            mirror_server().map_err(|(code, message)| BusError::unavailable(code, message))?;
        }
        let adb = ctx.read(adb_path)?;
        let device = require_device(&adb, &p.device)?;
        if device.state != "device" {
            return Err(BusError::unavailable("device.not_ready", format!("{} is {}", device.model, device.state)));
        }
        let (physical_width, physical_height) = device_size(&adb, &p.device)?;
        Ok((adb, physical_width, physical_height))
    }, |ctx: &mut Ctx, p, (adb, physical_width, physical_height)| {
        let max_size = mirror::capture_size_for(p.max_size.unwrap_or(mirror::CAPTURE_MIN));
        // A caller's rate is held to the same 8 Mbps floor as the ladder (`MIN_BIT_RATE`).
        let bitrate = mirror::clamp_bit_rate(p.bitrate.unwrap_or_else(|| mirror::capture_bit_rate(max_size)));
        let (width, height) = mirror::fit_size(physical_width, physical_height, max_size);
        let id = ctx.engine().next_mirror.fetch_add(1, Ordering::SeqCst);
        let scid = new_scid(id);
        let runtime = MirrorRuntime::new(MirrorRuntimeConfig {
            id,
            device: p.device,
            width,
            height,
            max_size,
            bitrate,
            scid,
            adb: adb.display().to_string(),
        });
        ctx.engine().mirrors.lock().unwrap().insert(id, runtime.clone());
        ctx.emit("mirror.changed", json!({"mirror_id":id,"device":runtime.device,"state":"starting","width":width,"height":height}));
        if ctx.engine().instance != Instance::Test {
            ctx.after_commit(move |engine| { std::thread::spawn(move || mirror_worker(engine, runtime)); });
        }
        Ok(MirrorStartOut { mirror_id: id, width, height })
    });

    e.register::<MirrorStop>(|ctx: &mut Ctx, p| {
        let runtime = ctx
            .engine()
            .mirrors
            .lock()
            .unwrap()
            .remove(&p.mirror_id)
            .ok_or_else(|| {
                BusError::not_found(
                    "device.mirror_not_found",
                    format!("no mirror {}", p.mirror_id),
                )
            })?;
        runtime.stop();
        // A mirror that already ended (lost, failed) keeps the state it ended with, and its
        // stop was announced then: only the transition this call makes is news (RA-334).
        if runtime.finish(MirrorState::Stopped, None, None) {
            ctx.emit(
                "mirror.changed",
                json!({"mirror_id":p.mirror_id,"device":runtime.device,"state":"stopped"}),
            );
        }
        Ok(Empty {})
    });

    // The engine answers this from memory before it gets here (`Engine::mirror_fast_path`);
    // this handler is the fallback for the cases that path declines — an audited or
    // session-bound caller — and it touches nothing in the store either.
    e.register::<MirrorInput>(|ctx, p| {
        let runtime = mirror_by_id(ctx.engine(), p.mirror_id)?;
        send_input(&runtime, &p.event)?;
        Ok(Empty {})
    });

    // `adb devices` (up to 2 s, longer when the server is cold), the checkout and wrapper probes
    // and the branch lookup all run in the prepare phase with nothing locked (D149). The
    // transaction only records the run and takes the device lease.
    e.register_staged::<RunOp, RunPrepared>(|ctx, p| {
        let project = ctx.read(|conn| get_project(conn, p.project_id))?;
        // The device lease (device_lease): refuse before any adb or Gradle work when another
        // session is installing on this device, and name it.
        let requested_root = p.worktree.clone().unwrap_or_else(|| project.path.clone());
        let requested_root = std::fs::canonicalize(&requested_root).map(|path| path.display().to_string()).unwrap_or(requested_root);
        // `device.run` is user-only, so the lease is never the caller's own: it goes to the live
        // session that owns the checkout, else the user.
        let (holder, released, adb, sdk_root, integration_root) = ctx.read(|conn| {
            let holder = super::device_lease::worktree_holder(conn, project.id, &requested_root)?;
            let released = super::device_lease::prune(ctx.engine(), conn);
            let integration_root = p.integration_id.map(|id| integration_root(conn, project.id, id)).transpose()?;
            Ok((holder, released, adb_path(conn), android_sdk_root(conn), integration_root))
        })?;
        // Gone whatever this request's outcome: announced now, not with events a refusal drops.
        for lease in released { ctx.engine().emit_system(super::device_lease::RELEASED, lease.event()); }
        ctx.engine().device_leases.check(&p.device, &holder)?;
        let adb = adb?;
        let device = require_device(&adb, &p.device)?;
        if device.state != "device" {
            return Err(BusError::unavailable("device.not_ready", format!("{} is {}", device.model, device.state)));
        }
        let default_gradle_command = project.run_cmd.as_deref().is_none_or(|command| command.trim().is_empty());
        let gradle_init = default_gradle_command.then(gradle_init_script).transpose()?;
        let selected = integration_root.unwrap_or_else(|| PathBuf::from(p.worktree.as_deref().unwrap_or(&project.path)));
        let root = checked_run_root(&project.path, &selected)?;
        let project_root = std::fs::canonicalize(&project.path)
            .map_err(|error| BusError::invalid("project.path", error.to_string()))?;
        let sync_primary = p.integration_id.is_none() && root == project_root;
        let command = run_command(&root, project.run_cmd.as_deref(), p.variant.as_deref(), gradle_init.as_ref().map(|file| file.path()))?;
        let sdk_root = match sdk_root {
            Ok(path) => Some(path),
            Err(error) if default_gradle_command => return Err(error),
            Err(_) => None,
        };
        let source = run_source(&root, &project.path);
        Ok(RunPrepared { adb, requested_root, root, command, gradle_init, sdk_root, default_gradle_command, sync_primary, source })
    }, |ctx: &mut Ctx, p, prepared| {
        let RunPrepared { adb, requested_root, root, command, gradle_init, sdk_root, default_gradle_command, sync_primary, source } = prepared;
        let project = get_project(ctx.tx(), p.project_id)?;
        let holder = super::device_lease::worktree_holder(ctx.tx(), project.id, &requested_root)?;
        // Again under the lock: the device may have been taken while adb was answering.
        super::device_lease::check(ctx, &p.device, &holder)?;
        refuse_while_syncing(ctx.engine(), &root)?;
        // A run or build already live in this checkout is reading it: fast-forwarding under it
        // would build (or ship) a mix of two commits. This run uses the checkout as it is.
        let live = live_runs_in(ctx.tx(), &root)?;
        let sync_primary = sync_primary && live.is_empty();
        ctx.tx().execute(
            "INSERT INTO device_runs(project_id,device,worktree,state,started_at) VALUES (?1,?2,?3,'building',?4)",
            params![project.id,p.device,root.display().to_string(),ctx.now],
        ).bus()?;
        let id = ctx.tx().last_insert_rowid();
        let run = get_run(ctx.tx(), id)?;
        let runtime = RunRuntime::new(id);
        if sync_primary {
            runtime.set_syncing(Some(root.clone()));
        } else if let Some(other) = live.first() {
            runtime.push(format!("primary sync: skipped; run {other} is live in this checkout, so it is used as it is"));
        }
        let action = format!("device.run {} from {source}", p.variant.as_deref().unwrap_or("debug"));
        // From here the run exists in memory as well as in this transaction: the guard takes it
        // back out (runtime and lease) unless the transaction commits.
        let pending = PendingRun::register(ctx.engine(), runtime.clone())?;
        super::device_lease::acquire(ctx, crate::device_lease::Lease::new(&p.device, holder, crate::device_lease::Kind::Run(id), &action, None))?;
        let parent = ctx.req_id;
        let device_serial = p.device;
        let variant = p.variant.unwrap_or_else(|| "debug".to_string());
        let adb_string = adb.display().to_string();
        ctx.set_project(project.id);
        ctx.emit("run.changed", serde_json::to_value(&run).bus()?);
        let request = RunWorkerRequest {
            root,
            command,
            device: device_serial,
            adb: adb_string,
            sdk_root,
            launch_after_build: default_gradle_command,
            variant,
            _gradle_init: gradle_init,
            sync_primary,
        };
        ctx.after_commit(move |engine| {
            pending.keep();
            std::thread::spawn(move || run_worker(engine, runtime, parent, request));
        });
        Ok(run)
    });

    e.register::<Build>(|ctx: &mut Ctx, p| {
        let project = get_project(ctx.tx(), p.project_id)?;
        let root = resolve_run_root(ctx.tx(), project.id, &project.path, p.worktree.as_deref(), p.integration_id)?;
        // A release ships exactly what is checked out, so not while a run is moving it.
        refuse_while_syncing(ctx.engine(), &root)?;
        let variant = p.variant.unwrap_or_else(|| "release".to_string());
        let format = artifact_format(p.format.as_deref());
        let publish = p.publish.unwrap_or(false);
        let signing_profile = build_signing_profile(ctx.engine(), &project.path)?;
        let signing_init = signing_profile.as_ref().map(|_| signing_init_script()).transpose()?;
        // A release build is Gradle's alone: no adb, no device, and no `build_cmd` detour —
        // that one belongs to the integration verifier. Builds also skip the primary-checkout
        // sync a run does: a release ships exactly what is checked out (D152).
        let command = build_command(&root, &variant, &format, publish, signing_init.as_ref().map(|file| file.path()))?;
        let sdk_root = android_sdk_root(ctx.tx())?;
        ctx.tx().execute(
            "INSERT INTO device_runs(project_id,kind,device,worktree,state,variant,format,publish,started_at) VALUES (?1,'build','',?2,'building',?3,?4,?5,?6)",
            params![project.id, root.display().to_string(), variant, format, publish as i64, ctx.now],
        ).bus()?;
        let id = ctx.tx().last_insert_rowid();
        let run = get_run(ctx.tx(), id)?;
        let runtime = RunRuntime::new(id);
        let pending = PendingRun::register(ctx.engine(), runtime.clone())?;
        let parent = ctx.req_id;
        ctx.set_project(project.id);
        ctx.emit("run.changed", serde_json::to_value(&run).bus()?);
        let request = BuildWorkerRequest { root, command, sdk_root, variant, format, signing_profile, _signing_init: signing_init };
        ctx.after_commit(move |engine| {
            pending.keep();
            std::thread::spawn(move || build_worker(engine, runtime, parent, request));
        });
        Ok(run)
    });

    e.register_unlocked::<SigningGet>(|ctx, p| {
        let project = ctx.read(|conn| get_project(conn, p.project_id))?;
        signing_profile_out(ctx.engine(), &project.path)
            .map_err(|error| error.with_hint("switch Relay signing off, or create the profile again"))
    });

    // Key generation and Secret Service can both wait on external processes. The transaction
    // only attributes the completed profile and emits its state change (D149, D157).
    e.register_staged::<SigningCreate, _>(
        |ctx, p| {
            let project = ctx.read(|conn| get_project(conn, p.project_id))?;
            create_signing_profile(ctx.engine(), &project.path, &p.key_alias, &p.password)
        },
        |ctx: &mut Ctx, p, profile| {
            get_project(ctx.tx(), p.project_id)?;
            ctx.set_project(p.project_id);
            ctx.emit("device.signing.changed", json!({ "project_id": p.project_id }));
            Ok(profile)
        },
    );

    e.register::<SigningSetEnabled>(|ctx: &mut Ctx, p| {
        let project = get_project(ctx.tx(), p.project_id)?;
        let profile = set_signing_enabled(ctx.engine(), &project.path, p.enabled)?;
        ctx.set_project(p.project_id);
        ctx.emit("device.signing.changed", json!({ "project_id": p.project_id }));
        Ok(profile)
    });

    e.register::<RunStop>(|ctx: &mut Ctx, p| {
        let run = get_run(ctx.tx(), p.run_id)?;
        let runtime = ctx.engine().device_runs.lock().unwrap_or_else(PoisonError::into_inner).remove(&p.run_id);
        if let Some(runtime) = &runtime {
            runtime.stop();
        }
        // While it builds, the stop does not reach Gradle's daemon, which may still be running
        // `adb install`: the device stays the holder's a little longer. A running app is long
        // installed, and a build stopped before Gradle started (during its checkout's sync)
        // installs nothing: either device is free at once.
        let gradle_ran = runtime.as_ref().is_none_or(|runtime| runtime.started());
        let (released, grace) = if run.state == "building" && gradle_ran {
            ctx.engine().device_leases.stop_run(p.run_id, crate::device_lease::SHELL_GRACE)
        } else {
            (ctx.engine().device_leases.release_run(p.run_id), None)
        };
        for lease in released {
            ctx.emit(super::device_lease::RELEASED, lease.event());
        }
        if let Some(lease) = grace {
            ctx.emit(super::device_lease::ACQUIRED, lease.event());
        }
        if matches!(run.state.as_str(), "building" | "running") {
            ctx.tx()
                .execute(
                    "UPDATE device_runs SET state='stopped',finished_at=?1 WHERE id=?2",
                    params![ctx.now, p.run_id],
                )
                .bus()?;
            ctx.set_project(run.project_id);
            ctx.emit(
                "run.changed",
                serde_json::to_value(get_run(ctx.tx(), p.run_id)?).bus()?,
            );
        }
        Ok(Empty {})
    });

    e.register::<RunList>(|ctx, p| {
        get_project(ctx.tx(), p.project_id)?;
        let mut st = ctx
            .tx()
            .prepare_cached("SELECT * FROM device_runs WHERE project_id=?1 ORDER BY id DESC LIMIT 50")
            .bus()?;
        let runs = st
            .query_map([p.project_id], run_row)
            .bus()?
            .collect::<rusqlite::Result<Vec<_>>>()
            .bus()?;
        Ok(RunListOut { runs })
    });

    e.register_unlocked::<AvdList>(|ctx, _| {
        let (emulator, adb) = ctx.read(|conn| {
            Ok((sdk_tool(conn, "device.emulator_path", "emulator")?, adb_path(conn).ok()))
        })?;
        Ok(AvdListOut { avds: list_avds_with(&emulator, adb.as_deref())? })
    });
    e.register_unlocked::<AvdCatalog>(|ctx, _| {
        let (sdk, avdmanager) = ctx.read(|conn| {
            Ok((android_sdk_root(conn)?, sdk_tool(conn, "device.avdmanager_path", "avdmanager").ok()))
        })?;
        let mut system_images = installed_system_images(&sdk)?;
        system_images.sort();
        system_images.reverse();
        // avdmanager is a JVM: slow to start, and nothing here may wait on it forever (D144).
        let devices = avdmanager
            .and_then(|avdmanager| crate::proc::output_with_timeout(
                Command::new(avdmanager).args(["list", "device", "-c"]), AVDMANAGER_LIST_TIMEOUT).ok().flatten())
            .filter(|output| output.status.success())
            .map(|output| String::from_utf8_lossy(&output.stdout).lines().map(str::trim).filter(|line| !line.is_empty()).map(str::to_string).collect())
            .unwrap_or_default();
        Ok(AvdCatalogOut { system_images, devices })
    });
    // Creating runs avdmanager (a JVM) and then lists AVDs through the emulator binary, so
    // both happen before the transaction opens (D149); the transaction only announces it.
    e.register_staged::<AvdCreate, Avd>(|ctx, p| {
        let name = valid_avd_name(&p.name)?;
        if !p.package.starts_with("system-images;") {
            return Err(BusError::invalid("avd.package", "AVD package must be an installed system image"));
        }
        let (avdmanager, emulator) = ctx.read(|conn| Ok((
            sdk_tool(conn, "device.avdmanager_path", "avdmanager")?,
            sdk_tool(conn, "device.emulator_path", "emulator").ok(),
        )))?;
        let mut command = Command::new(avdmanager);
        command.args(["create", "avd", "--name", &name, "--package", &p.package]);
        if let Some(device) = p.device.as_deref().filter(|value| !value.trim().is_empty()) {
            command.args(["--device", device]);
        }
        // "no" answers "Do you wish to create a custom hardware profile?", which avdmanager
        // asks whenever no --device is given; an empty stdin makes it throw instead.
        let output = crate::proc::output_with_input(&mut command, b"no\n", AVD_CREATE_TIMEOUT)
            .map_err(|error| BusError::unavailable("avd.create_failed", error.to_string()))?
            .ok_or_else(|| BusError::unavailable("avd.create_timeout",
                format!("avdmanager did not finish within {} s", AVD_CREATE_TIMEOUT.as_secs())))?;
        if !output.status.success() {
            return Err(BusError::unavailable("avd.create_failed", String::from_utf8_lossy(&output.stderr).trim().to_string()));
        }
        // The AVD exists now whatever the listing says, so a listing that fails only costs the
        // details it would have filled in. Nothing just created is running: no adb probe.
        let listed = emulator.and_then(|emulator| list_avds_with(&emulator, None).ok())
            .and_then(|avds| avds.into_iter().find(|avd| avd.name == name));
        Ok(listed.unwrap_or(Avd { name, device: p.device.clone(), package: Some(p.package.clone()), path: None, running_serial: None }))
    }, |ctx: &mut Ctx, _p, avd| {
        ctx.emit("avd.changed", serde_json::to_value(&avd).bus()?);
        Ok(avd)
    });
    e.register_staged::<AvdBoot, PathBuf>(|ctx, p| {
        let name = valid_avd_name(&p.name)?;
        let (emulator, adb) = ctx.read(|conn| Ok((sdk_tool(conn, "device.emulator_path", "emulator")?, adb_path(conn).ok())))?;
        if !list_avds_with(&emulator, None)?.iter().any(|avd| avd.name == name) {
            return Err(BusError::not_found("avd.not_found", format!("no AVD named {name}")));
        }
        // A second emulator on a running AVD exits at once (the AVD is locked), headless and so
        // silently. Without adb nothing can be known, and the emulator's own refusal is reported.
        if let Some(serial) = adb.and_then(|adb| running_avds_with(&adb).ok()).and_then(|mut running| running.remove(&name)) {
            return Err(BusError::conflict("avd.already_running", format!("{name} is already running as {serial}")));
        }
        Ok(emulator)
    }, |ctx: &mut Ctx, p, emulator| {
        let name = valid_avd_name(&p.name)?;
        let cold = p.cold.unwrap_or(false);
        let event_name = name.clone();
        ctx.emit("avd.changed", json!({"name":name,"state":"booting"}));
        ctx.after_commit(move |engine| {
            let mut command = Command::new(emulator);
            // Headless: Relay's mirror is the emulator's only screen, the same window a phone
            // gets, rather than a second emulator UI beside it.
            command.arg(format!("@{event_name}")).arg("-no-window");
            if cold { command.arg("-no-snapshot-load"); }
            command.stdin(Stdio::null()).stdout(Stdio::piped()).stderr(Stdio::piped());
            match command.spawn() {
                Ok(child) => {
                    engine.emit_system("avd.changed", json!({"name":event_name,"state":"started"}));
                    // Weak: an emulator can outlive the engine that booted it.
                    let engine = Arc::downgrade(&engine);
                    let _ = std::thread::Builder::new().name("avd-reaper".into())
                        .spawn(move || reap_emulator(&engine, &event_name, child));
                }
                Err(error) => engine.emit_system("avd.changed", json!({"name":event_name,"state":"failed","message":error.to_string()})),
            }
        });
        Ok(Empty {})
    });
    // AVDs boot headless, so there is no emulator window to close: this is how one stops.
    e.register_staged::<AvdStop, _>(|ctx, p| {
        let name = valid_avd_name(&p.name)?;
        let adb = ctx.read(adb_path)?;
        let serial = running_avds_with(&adb)?.remove(&name)
            .ok_or_else(|| BusError::not_found("avd.not_running", format!("{name} is not running")))?;
        Ok((adb, name, serial))
    }, |ctx: &mut Ctx, _, (adb, name, serial)| {
        ctx.emit("avd.changed", json!({"name":name,"state":"stopping"}));
        ctx.after_commit(move |engine| {
            std::thread::spawn(move || {
                let mut command = Command::new(adb);
                command.args(["-s", &serial, "emu", "kill"]).stdin(Stdio::null());
                let stopped = matches!(crate::proc::output_with_timeout(&mut command, Duration::from_secs(10)), Ok(Some(output)) if output.status.success());
                engine.emit_system("avd.changed", json!({"name":name,"state":if stopped { "stopped" } else { "failed" }}));
            });
        });
        Ok(Empty {})
    });
}

/// An emulator that exits this soon after its boot never came up: a missing system image, no
/// KVM, an AVD locked by another emulator.
const AVD_EARLY_EXIT: Duration = Duration::from_secs(10);
/// How much of a booting emulator's output is kept to explain an exit.
const AVD_TAIL_LINES: usize = 40;

/// Wait on a booted emulator, so it never lingers as a zombie, and say so when it goes on its
/// own: `failed` with its last error when it never came up, `stopped` when a running one died.
/// A clean exit later is avd.stop's `emu kill`, which announces itself.
fn reap_emulator(engine: &std::sync::Weak<Engine>, name: &str, mut child: std::process::Child) {
    let started = Instant::now();
    let tail = Arc::new(Mutex::new(std::collections::VecDeque::new()));
    let pipes: [Option<Box<dyn Read + Send>>; 2] = [
        child.stdout.take().map(|pipe| Box::new(pipe) as Box<dyn Read + Send>),
        child.stderr.take().map(|pipe| Box::new(pipe) as Box<dyn Read + Send>),
    ];
    // Drained on their own threads: a child the emulator forks may hold them open past its exit.
    let drains: Vec<_> = pipes.into_iter().flatten().map(|pipe| {
        let tail = tail.clone();
        std::thread::spawn(move || {
            // Lossy and to the end: a drain that stopped early would leave the emulator blocked
            // on a full pipe.
            let (mut reader, mut buffer) = (BufReader::new(pipe), Vec::new());
            while let Ok(Some(line)) = read_lossy_line(&mut reader, &mut buffer) {
                let mut tail = tail.lock().unwrap_or_else(|poison| poison.into_inner());
                if tail.len() == AVD_TAIL_LINES { tail.pop_front(); }
                tail.push_back(line);
            }
        })
    }).collect();
    let status = child.wait();
    let settle = Instant::now() + Duration::from_millis(500);
    while drains.iter().any(|drain| !drain.is_finished()) && Instant::now() < settle { std::thread::sleep(Duration::from_millis(20)); }
    let early = started.elapsed() < AVD_EARLY_EXIT;
    if !early && status.as_ref().is_ok_and(|status| status.success()) { return; }
    let lines: Vec<String> = tail.lock().unwrap_or_else(|poison| poison.into_inner()).iter().cloned().collect();
    let message = emulator_exit_message(&lines).unwrap_or_else(|| match &status {
        Ok(status) => format!("emulator exited ({status})"),
        Err(error) => format!("emulator could not be waited on: {error}"),
    });
    let Some(engine) = engine.upgrade().filter(|engine| !engine.is_quitting()) else { return };
    engine.emit_system("avd.changed", json!({"name":name,"state":if early { "failed" } else { "stopped" },"message":message}));
}

/// The line of an emulator's output that says why it quit: its last error, else its last line.
fn emulator_exit_message(lines: &[String]) -> Option<String> {
    let error = lines.iter().rev().find(|line| ["ERROR", "PANIC", "FATAL"].iter().any(|word| line.contains(word)));
    error.or_else(|| lines.iter().rev().find(|line| !line.trim().is_empty())).map(|line| line.trim().to_string())
}

/// The first wait before `adb track-devices` is started again, doubling to the second.
const WATCH_RETRY_MIN: Duration = Duration::from_millis(500);
const WATCH_RETRY_MAX: Duration = Duration::from_secs(10);

/// Follow `adb track-devices` for as long as any client holds a watch. The stream ends when the
/// adb server restarts (another adb of a different version, `adb kill-server`, a crash); the
/// worker then starts it again, which also starts the server, until the last client lets go.
/// Holds the engine weakly: a strong hold kept an engine with a live watch from ever being
/// dropped, and `adb track-devices` running after it.
fn device_watch_worker(engine: std::sync::Weak<Engine>, runtime: Arc<DeviceWatchRuntime>, adb: PathBuf) {
    let mut retry = WATCH_RETRY_MIN;
    while !runtime.stopped() {
        let started = Instant::now();
        let child = Command::new(&adb).args(["track-devices", "-l"]).stdin(Stdio::null()).stdout(Stdio::piped()).stderr(Stdio::null()).spawn();
        if let Ok(mut child) = child {
            match child.stdout.take() {
                Some(stdout) => {
                    if !runtime.install(child) { break; }
                    // Every frame is the whole device list, the first one included: a server that
                    // came back with a different list is reported by that first frame.
                    let mut reader = BufReader::new(stdout);
                    while !runtime.stopped() {
                        match read_track_frame(&mut reader) {
                            Ok(Some(_)) => match engine.upgrade() {
                                Some(engine) => engine.emit_system("device.changed", json!({"reason":"system"})),
                                None => runtime.stop(),
                            },
                            Ok(None) | Err(_) => break,
                        }
                    }
                    runtime.finish();
                }
                None => { let _ = child.kill(); let _ = child.wait(); }
            }
        }
        // A stream that ran for a while was healthy; one that died at once backs off.
        if started.elapsed() > WATCH_RETRY_MAX { retry = WATCH_RETRY_MIN; }
        let until = Instant::now() + retry;
        while !runtime.stopped() && Instant::now() < until { std::thread::sleep(Duration::from_millis(50)); }
        retry = (retry * 2).min(WATCH_RETRY_MAX);
    }
    if let Some(engine) = engine.upgrade() {
        clear_device_watch(&engine, &runtime);
    }
}

/// One message of `adb track-devices`: a `%04x` byte count, then that many bytes of device
/// list (one device per line, none at all once the last one is unplugged — which is why the
/// stream cannot be read by lines). `Ok(None)` when the stream ends between messages.
fn read_track_frame(reader: &mut impl Read) -> std::io::Result<Option<Vec<u8>>> {
    let mut header = [0u8; 4];
    if let Err(error) = reader.read_exact(&mut header) {
        return if error.kind() == std::io::ErrorKind::UnexpectedEof { Ok(None) } else { Err(error) };
    }
    let length = std::str::from_utf8(&header).ok()
        .filter(|header| header.bytes().all(|byte| byte.is_ascii_hexdigit()))
        .and_then(|header| usize::from_str_radix(header, 16).ok())
        .ok_or_else(|| std::io::Error::new(std::io::ErrorKind::InvalidData, format!("not an adb length prefix: {:?}", String::from_utf8_lossy(&header))))?;
    let mut payload = vec![0u8; length];
    reader.read_exact(&mut payload)?;
    Ok(Some(payload))
}

fn clear_device_watch(engine: &Engine, runtime: &Arc<DeviceWatchRuntime>) {
    let mut current = engine.device_watch.lock().unwrap();
    if current.runtime.as_ref().is_some_and(|value| Arc::ptr_eq(value, runtime)) { current.runtime.take(); }
}

fn valid_avd_name(value: &str) -> Result<String, BusError> {
    let value = value.trim();
    if value.is_empty() || value.len() > 80 || !value.chars().all(|character| character.is_ascii_alphanumeric() || matches!(character, '_' | '-' | '.')) {
        return Err(BusError::invalid("avd.name", "AVD name must use 1..=80 letters, digits, dots, dashes, or underscores"));
    }
    Ok(value.to_string())
}

fn sdk_tool(conn: &rusqlite::Connection, setting: &str, name: &str) -> Result<PathBuf, BusError> {
    let configured: Option<String> = conn.prepare_cached("SELECT value FROM settings WHERE path=?1").bus()?
        .query_row([setting], |row| row.get(0)).optional().bus()?;
    if let Some(value) = configured.and_then(|value| serde_json::from_str::<String>(&value).ok()).filter(|value| !value.is_empty()) {
        let path = PathBuf::from(value);
        if path.is_file() { return Ok(path); }
        return Err(BusError::unavailable("avd.tool_missing", format!("configured {name} does not exist: {}", path.display())));
    }
    if let Ok(path) = which::which(name) { return Ok(path); }
    if let Ok(sdk) = android_sdk_root(conn) {
        let direct = match name {
            "emulator" => Some(sdk.join("emulator/emulator")),
            "avdmanager" => find_avdmanager(&sdk),
            _ => None,
        };
        if let Some(path) = direct.filter(|path| path.is_file()) { return Ok(path); }
    }
    Err(BusError::unavailable("avd.tool_missing", format!("{name} is not installed or could not be detected")))
}

fn android_sdk_root(conn: &rusqlite::Connection) -> Result<PathBuf, BusError> {
    if let Some(root) = located_sdk_root(conn)? { return Ok(root); }
    if let Ok(adb) = adb_path(conn) {
        if adb.parent().and_then(Path::file_name).is_some_and(|name| name == "platform-tools") {
            if let Some(root) = adb.parent().and_then(Path::parent).filter(|path| path.is_dir()) { return Ok(root.to_path_buf()); }
        }
    }
    Err(BusError::unavailable("avd.sdk_missing", "Android SDK root could not be detected").with_hint("set the Android SDK path in Settings"))
}

/// The SDK root from Settings, the environment or the usual install location — everything
/// [`android_sdk_root`] tries except deriving it from adb, so [`adb_path`] can look there too.
fn located_sdk_root(conn: &rusqlite::Connection) -> Result<Option<PathBuf>, BusError> {
    let configured: Option<String> = conn.prepare_cached("SELECT value FROM settings WHERE path='device.sdk_path'").bus()?
        .query_row([], |row| row.get(0)).optional().bus()?;
    if let Some(value) = configured.and_then(|value| serde_json::from_str::<String>(&value).ok()).filter(|value| !value.is_empty()) {
        let path = PathBuf::from(value);
        if path.is_dir() { return Ok(Some(path)); }
    }
    for key in ["ANDROID_SDK_ROOT", "ANDROID_HOME"] {
        if let Some(path) = std::env::var_os(key).map(PathBuf::from).filter(|path| path.is_dir()) { return Ok(Some(path)); }
    }
    if let Some(home) = directories::BaseDirs::new().map(|dirs| dirs.home_dir().to_path_buf()) {
        for path in [home.join("Android/Sdk"), home.join("Android/sdk")] {
            if path.is_dir() { return Ok(Some(path)); }
        }
    }
    Ok(None)
}

/// The adb an Android SDK ships, when it has one.
fn sdk_adb(sdk: &Path) -> Option<PathBuf> {
    Some(sdk.join("platform-tools/adb")).filter(|path| path.is_file())
}

fn find_avdmanager(sdk: &Path) -> Option<PathBuf> {
    let latest = sdk.join("cmdline-tools/latest/bin/avdmanager");
    if latest.is_file() { return Some(latest); }
    let mut candidates = std::fs::read_dir(sdk.join("cmdline-tools")).ok()?.flatten()
        .map(|entry| (numeric_version(&entry.file_name().to_string_lossy()), entry.path().join("bin/avdmanager")))
        .filter(|(_, path)| path.is_file()).collect::<Vec<_>>();
    // Newest version last: 19.0 after 9.0, which plain path order gets backwards.
    candidates.sort();
    candidates.pop().map(|(_, path)| path).or_else(|| {
        let legacy = sdk.join("tools/bin/avdmanager");
        legacy.is_file().then_some(legacy)
    })
}

/// `19.0` → `[19, 0]`, for ordering versioned SDK directories; `None` for any other name.
fn numeric_version(name: &str) -> Option<Vec<u64>> {
    name.split('.').map(|part| part.parse().ok()).collect()
}

fn installed_system_images(sdk: &Path) -> Result<Vec<String>, BusError> {
    let root = sdk.join("system-images");
    if !root.is_dir() { return Ok(Vec::new()); }
    let mut images = Vec::new();
    for api in std::fs::read_dir(&root).map_err(|error| BusError::unavailable("avd.catalog_failed", error.to_string()))? {
        let api = api.map_err(|error| BusError::unavailable("avd.catalog_failed", error.to_string()))?;
        if !api.path().is_dir() { continue; }
        for variant in std::fs::read_dir(api.path()).into_iter().flatten().flatten() {
            if !variant.path().is_dir() { continue; }
            for arch in std::fs::read_dir(variant.path()).into_iter().flatten().flatten() {
                if arch.path().is_dir() {
                    images.push(format!("system-images;{};{};{}", api.file_name().to_string_lossy(), variant.file_name().to_string_lossy(), arch.file_name().to_string_lossy()));
                }
            }
        }
    }
    Ok(images)
}

/// The subprocess half of [`list_avds`], with both tool paths already read out of the store so
/// the emulator and adb calls happen with nothing locked (D144).
fn list_avds_with(emulator: &Path, adb: Option<&Path>) -> Result<Vec<Avd>, BusError> {
    let output = crate::proc::output_with_timeout(Command::new(emulator).arg("-list-avds"), AVD_LIST_TIMEOUT)
        .map_err(|error| BusError::unavailable("avd.list_failed", error.to_string()))?
        .ok_or_else(|| BusError::unavailable("avd.list_failed", "emulator -list-avds did not answer"))?;
    if !output.status.success() {
        return Err(BusError::unavailable("avd.list_failed", String::from_utf8_lossy(&output.stderr).trim().to_string()));
    }
    let running = adb.map(running_avds_with).transpose().ok().flatten().unwrap_or_default();
    let base = std::env::var_os("ANDROID_AVD_HOME").map(PathBuf::from).or_else(|| directories::BaseDirs::new().map(|dirs| dirs.home_dir().join(".android/avd")));
    let mut avds = Vec::new();
    for name in String::from_utf8_lossy(&output.stdout).lines().map(str::trim).filter(|line| !line.is_empty()) {
        let ini = base.as_ref().map(|base| base.join(format!("{name}.ini")));
        let path = ini.as_ref().and_then(|ini| read_property(ini, "path")).map(PathBuf::from)
            .or_else(|| base.as_ref().map(|base| base.join(format!("{name}.avd"))));
        let config = path.as_ref().map(|path| path.join("config.ini"));
        avds.push(Avd {
            name: name.to_string(),
            device: config.as_ref().and_then(|path| read_property(path, "hw.device.name")),
            package: config.as_ref().and_then(|path| read_property(path, "image.sysdir.1")).map(|value| image_package(&value)),
            path: path.map(|path| path.display().to_string()),
            running_serial: running.get(name).cloned(),
        });
    }
    Ok(avds)
}

fn running_avds_with(adb: &Path) -> Result<std::collections::HashMap<String, String>, BusError> {
    let mut running = std::collections::HashMap::new();
    for device in list_with_adb(adb)?.into_iter().filter(|device| device.kind == DeviceKind::Avd && device.state == "device") {
        // A wedged emulator console never answers this; give each one a few seconds.
        if let Ok(Some(output)) = crate::proc::output_with_timeout(
            Command::new(adb).args(["-s", &device.serial, "emu", "avd", "name"]), AVD_NAME_TIMEOUT) {
            let stdout = String::from_utf8_lossy(&output.stdout);
            let name = stdout.lines().map(str::trim).find(|line| !line.is_empty() && *line != "OK");
            if let Some(name) = name { running.insert(name.to_string(), device.serial); }
        }
    }
    Ok(running)
}

fn read_property(path: &Path, key: &str) -> Option<String> {
    std::fs::read_to_string(path).ok()?.lines().find_map(|line| line.strip_prefix(&format!("{key}=")).map(str::trim).map(str::to_string))
}

fn image_package(value: &str) -> String {
    let normalized = value.trim().trim_matches('/').replace('\\', "/");
    if let Some(rest) = normalized.split("system-images/").nth(1) {
        format!("system-images;{}", rest.trim_matches('/').replace('/', ";"))
    } else { normalized.replace('/', ";") }
}

pub fn mirror_by_id(engine: &Engine, id: Id) -> Result<Arc<MirrorRuntime>, BusError> {
    engine
        .mirrors
        .lock()
        .unwrap()
        .get(&id)
        .cloned()
        .ok_or_else(|| BusError::not_found("device.mirror_not_found", format!("no mirror {id}")))
}

pub fn run_by_id(engine: &Engine, id: Id) -> Result<Arc<RunRuntime>, BusError> {
    engine
        .device_runs
        .lock()
        .unwrap()
        .get(&id)
        .cloned()
        .ok_or_else(|| {
            BusError::not_found(
                "device.run_not_live",
                format!("run {id} has no live stream"),
            )
        })
}

/// adb from Settings, else PATH, else the discovered Android SDK's `platform-tools` — the
/// order `sdk_tool` uses for the emulator and avdmanager. Android Studio installs adb only
/// there, and a desktop session's PATH rarely includes it.
pub(crate) fn adb_path(conn: &rusqlite::Connection) -> Result<PathBuf, BusError> {
    let configured: Option<String> = conn
        .prepare_cached("SELECT value FROM settings WHERE path='device.adb_path'")
        .bus()?
        .query_row([], |row| row.get(0))
        .optional()
        .bus()?;
    if let Some(value) = configured.and_then(|value| serde_json::from_str::<String>(&value).ok()) {
        let path = PathBuf::from(value);
        if path.is_file() {
            return Ok(path);
        }
        return Err(BusError::unavailable(
            "device.adb_missing",
            format!("configured adb does not exist: {}", path.display()),
        ));
    }
    if let Ok(path) = which::which("adb") {
        return Ok(path);
    }
    located_sdk_root(conn)?.as_deref().and_then(sdk_adb).ok_or_else(|| {
        BusError::unavailable("device.adb_missing", "adb is not on PATH or in the Android SDK")
            .with_hint("install the Android SDK platform-tools, or set the adb path in Settings")
    })
}

/// `adb devices` starts the adb server when it is not already up, which can take seconds. This
/// runs inside the request transaction, so an unbounded wait would freeze every other bus op —
/// keystrokes included (D144). Bound it and refuse instead.
const ADB_LIST_TIMEOUT: Duration = Duration::from_secs(2);
/// `emulator -list-avds` reads a directory; it answers in milliseconds when it answers at all.
const AVD_LIST_TIMEOUT: Duration = Duration::from_secs(10);
/// `adb emu avd name` talks to the emulator's console, which a frozen emulator never answers.
const AVD_NAME_TIMEOUT: Duration = Duration::from_secs(3);
/// avdmanager is a JVM; listing device profiles is quick once it has started.
const AVDMANAGER_LIST_TIMEOUT: Duration = Duration::from_secs(60);
/// Creating an AVD copies a system image's userdata; generous, but never unbounded.
const AVD_CREATE_TIMEOUT: Duration = Duration::from_secs(120);

fn list_with_adb(adb: &Path) -> Result<Vec<Device>, BusError> {
    let mut command = Command::new(adb);
    command.args(["devices", "-l"]);
    let output = crate::proc::output_with_timeout(&mut command, ADB_LIST_TIMEOUT)
        .map_err(|e| BusError::unavailable("device.adb_failed", e.to_string()))?
        .ok_or_else(|| {
            BusError::unavailable(
                "device.adb_timeout",
                format!("adb did not answer within {}s", ADB_LIST_TIMEOUT.as_secs()),
            )
            .with_hint("the adb server may be starting; try again in a moment")
        })?;
    if !output.status.success() {
        return Err(BusError::unavailable(
            "device.adb_failed",
            String::from_utf8_lossy(&output.stderr).trim().to_string(),
        ));
    }
    Ok(String::from_utf8_lossy(&output.stdout).lines().skip(1).filter_map(parse_device_line).collect())
}

/// One line of `adb devices -l`. The state is every word before the first `key:value` extra,
/// so `no permissions (user in plugdev group; ...); see [http://...]` stays whole rather than
/// becoming `no`.
fn parse_device_line(line: &str) -> Option<Device> {
    let mut fields = line.split_whitespace().peekable();
    let serial = fields.next()?;
    if serial.starts_with('*') {
        return None;
    }
    let extra = |field: &str| field.split_once(':').is_some_and(|(key, _)| !key.is_empty() && key.chars().all(|c| c.is_ascii_lowercase() || c == '_'));
    let mut state = Vec::new();
    while let Some(field) = fields.next_if(|field| !extra(field)) {
        state.push(field);
    }
    if state.is_empty() {
        return None;
    }
    let extras: Vec<_> = fields.collect();
    let model = extras
        .iter()
        .find_map(|field| field.strip_prefix("model:"))
        .unwrap_or(serial)
        .replace('_', " ");
    Some(Device {
        serial: serial.to_string(),
        model,
        kind: if serial.starts_with("emulator-") {
            DeviceKind::Avd
        } else {
            DeviceKind::Usb
        },
        state: state.join(" "),
        lease: None,
    })
}

fn require_device(adb: &Path, serial: &str) -> Result<Device, BusError> {
    list_with_adb(adb)?
        .into_iter()
        .find(|device| device.serial == serial)
        .ok_or_else(|| {
            BusError::unavailable("device.none", format!("device {serial} is not connected"))
        })
}

const ADB_SIZE_TIMEOUT: Duration = Duration::from_secs(5);

fn device_size(adb: &Path, serial: &str) -> Result<(u32, u32), BusError> {
    let mut command = Command::new(adb);
    command.args(["-s", serial, "shell", "wm", "size"]);
    let output = crate::proc::output_with_timeout(&mut command, ADB_SIZE_TIMEOUT)
        .map_err(|e| BusError::unavailable("device.size_failed", e.to_string()))?
        .ok_or_else(|| {
            BusError::unavailable(
                "device.size_timeout",
                format!("`adb shell wm size` did not answer within {}s", ADB_SIZE_TIMEOUT.as_secs()),
            )
            .with_hint("the device may be locked up or still booting; replug it and try again")
        })?;
    if !output.status.success() {
        return Err(BusError::unavailable(
            "device.size_failed",
            String::from_utf8_lossy(&output.stderr).trim().to_string(),
        ));
    }
    let text = String::from_utf8_lossy(&output.stdout);
    text.lines()
        .rev()
        .find_map(|line| line.split_whitespace().last())
        .and_then(|size| size.split_once('x'))
        .and_then(|(w, h)| Some((w.parse().ok()?, h.parse().ok()?)))
        .ok_or_else(|| {
            BusError::unavailable(
                "device.size_failed",
                format!("unrecognized wm size output: {}", text.trim()),
            )
        })
}

fn new_scid(id: Id) -> u32 {
    let nanos = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|duration| duration.subsec_nanos())
        .unwrap_or(0);
    ((nanos ^ std::process::id().rotate_left(16) ^ (id as u32).rotate_left(7)) & 0x7fff_ffff).max(1)
}

/// One short adb step of a mirror's setup or teardown, under a deadline. The worker is off the
/// bus, but a push that never returns would still leave the window on "Connecting" forever.
fn mirror_adb(runtime: &MirrorRuntime, args: Vec<String>, timeout: Duration) -> Result<String, String> {
    let mut command = Command::new(&runtime.adb);
    command.args(&args);
    let output = crate::proc::output_with_timeout(&mut command, timeout)
        .map_err(|error| format!("adb: {error}"))?
        .ok_or_else(|| format!("adb {} did not finish within {}s", args.join(" "), timeout.as_secs()))?;
    if !output.status.success() {
        return Err(format!(
            "adb {} failed: {}",
            args.join(" "),
            String::from_utf8_lossy(&output.stderr).trim()
        ));
    }
    Ok(String::from_utf8_lossy(&output.stdout).into_owned())
}

fn connect_mirror_video(port: u16, deadline: Instant) -> Result<TcpStream, String> {
    loop {
        if let Ok(mut stream) = TcpStream::connect(("127.0.0.1", port)) {
            let _ = stream.set_read_timeout(Some(Duration::from_secs(1)));
            let mut dummy = [0u8; 1];
            if stream.read_exact(&mut dummy).is_ok() {
                return Ok(stream);
            }
        }
        if Instant::now() >= deadline {
            return Err("mirror server did not come up".into());
        }
        std::thread::sleep(Duration::from_millis(100));
    }
}

fn drain_server_lines(pipe: Option<impl Read + Send + 'static>, ring: Arc<Mutex<Vec<String>>>) {
    let Some(pipe) = pipe else { return };
    std::thread::spawn(move || {
        let (mut reader, mut buffer) = (BufReader::new(pipe), Vec::new());
        while let Ok(Some(line)) = read_lossy_line(&mut reader, &mut buffer) {
            let mut lines = ring.lock().unwrap();
            lines.push(line);
            let overflow = lines.len().saturating_sub(30);
            if overflow > 0 {
                lines.drain(..overflow);
            }
        }
    });
}

const MIRROR_PUSH_TIMEOUT: Duration = Duration::from_secs(60);
const MIRROR_FORWARD_TIMEOUT: Duration = Duration::from_secs(10);
/// How long the server gets between the handshake and its first session packet. An encoder
/// that never starts is a failure to report, not a window to leave on "Connecting".
const MIRROR_FIRST_UNIT_TIMEOUT: Duration = Duration::from_secs(10);

/// Report a mirror that ended without being asked to: the `mirror.changed` event for every
/// listener, and the runtime's terminal status, which is what the socket door forwards to
/// the window that owns it — the window does not have to be subscribed to anything.
fn mirror_failure(engine: &Engine, runtime: &MirrorRuntime, state: MirrorState, code: &str, message: String) {
    if !runtime.finish(state, Some(code.to_string()), Some(message.clone())) {
        return;
    }
    let state = if state == MirrorState::Lost { "lost" } else { "failed" };
    engine.emit_system(
        "mirror.changed",
        json!({"mirror_id":runtime.id,"device":runtime.device,"state":state,"code":code,"message":message}),
    );
}

/// Whether adb still sees the device. Asked once, after the stream broke, to tell "unplugged"
/// from "the server died" — the two need different words and a different next step.
fn device_present(runtime: &MirrorRuntime) -> bool {
    mirror_adb(runtime, vec!["-s".into(), runtime.device.clone(), "get-state".into()], Duration::from_secs(3))
        .is_ok_and(|state| state.trim() == "device")
}

/// Where this engine lists the adb forwards its mirrors hold open, one `serial port scid` per
/// line, so a forward an engine that died (or quit faster than adb) left behind is removed by
/// the next one — and only such a forward: scrcpy itself names its sockets the same way.
fn forwards_file(engine: &Engine) -> Option<PathBuf> {
    engine.store.path().parent().map(|dir| dir.join("mirror-forwards"))
}

/// Edits of the forwards file, one at a time across this engine's mirrors.
static FORWARDS_FILE: Mutex<()> = Mutex::new(());

fn record_forward(engine: &Engine, serial: &str, port: u16, scid: u32) {
    let Some(path) = forwards_file(engine) else { return };
    let _guard = FORWARDS_FILE.lock().unwrap_or_else(|poison| poison.into_inner());
    if let Ok(mut file) = OpenOptions::new().create(true).append(true).mode(0o600).open(&path) {
        let _ = writeln!(file, "{serial} {port} {scid}");
    }
}

fn unrecord_forward(engine: &Engine, serial: &str, port: u16) {
    let Some(path) = forwards_file(engine) else { return };
    let _guard = FORWARDS_FILE.lock().unwrap_or_else(|poison| poison.into_inner());
    let Ok(text) = fs::read_to_string(&path) else { return };
    let kept: String = text
        .lines()
        .filter(|line| parse_forward_line(line).is_some_and(|(s, p, _)| !(s == serial && p == port)))
        .map(|line| format!("{line}\n"))
        .collect();
    let _ = if kept.is_empty() { fs::remove_file(&path) } else { fs::write(&path, kept) };
}

fn parse_forward_line(line: &str) -> Option<(&str, u16, u32)> {
    let mut words = line.split_whitespace();
    let (serial, port, scid) = (words.next()?, words.next()?.parse().ok()?, words.next()?.parse().ok()?);
    words.next().is_none().then_some((serial, port, scid))
}

/// Remove the mirror's forward if nobody has yet, under `timeout`. Exactly once, whoever gets
/// there first: the worker, or the engine's shutdown.
fn remove_forward(engine: &Engine, runtime: &MirrorRuntime, timeout: Duration) {
    let Some(port) = runtime.take_forward() else { return };
    if mirror_adb(runtime, mirror::forward_remove_args(&runtime.device, port), timeout).is_ok() {
        unrecord_forward(engine, &runtime.device, port);
    }
}

/// How long the engine's shutdown waits for adb to drop the open mirrors' forwards.
const SHUTDOWN_FORWARD_TIMEOUT: Duration = Duration::from_secs(2);

/// The engine is quitting with mirrors open: remove their forwards now, all at once and under
/// a short deadline, rather than leave them to workers the process exit will not wait for.
/// `mirrors` is taken before `Engine::shutdown` empties the map. Refresh-rate pins those
/// mirrors hold are put back alongside, under the same deadline per adb call.
pub fn remove_mirror_forwards(engine: &Engine, mirrors: &[Arc<MirrorRuntime>]) {
    std::thread::scope(|scope| {
        scope.spawn(|| restore_all_refresh(engine, mirrors));
        for runtime in mirrors {
            scope.spawn(move || remove_forward(engine, runtime, SHUTDOWN_FORWARD_TIMEOUT));
        }
    });
}

// ---------------------------------------------------------------------------
// what a mirror start reads: Settings, and the device's display
// ---------------------------------------------------------------------------

/// `device.mirror.*` (defaults in `settings.rs`), read once per mirror start.
#[derive(Debug, Default)]
struct MirrorSettings {
    /// `max_fps`: a fixed frame-rate cap instead of the device's refresh rate.
    max_fps: Option<u32>,
    /// `video_codec_options`: extra encoder keys, as typed (checked before use).
    video_codec_options: Option<String>,
    /// `pin_refresh_rate`: hold the panel at its top refresh rate while mirroring.
    pin_refresh_rate: bool,
}

/// One short read of three leaf rows; the worker is off the bus, and this is the only time it
/// takes the store.
fn mirror_settings(engine: &Engine) -> MirrorSettings {
    let conn = engine.store.lock();
    let leaf = |path: &str| -> Option<Value> {
        let value: String = conn
            .prepare_cached("SELECT value FROM settings WHERE path = ?1")
            .ok()?
            .query_row([path], |row| row.get(0))
            .ok()?;
        serde_json::from_str(&value).ok()
    };
    MirrorSettings {
        max_fps: leaf("device.mirror.max_fps")
            .and_then(|value| value.as_u64())
            .filter(|&fps| fps > 0)
            .map(|fps| fps.min(u64::from(mirror::MAX_FPS_CAP)) as u32),
        video_codec_options: leaf("device.mirror.video_codec_options")
            .and_then(|value| value.as_str().map(str::trim).filter(|value| !value.is_empty()).map(String::from)),
        pin_refresh_rate: leaf("device.mirror.pin_refresh_rate").and_then(|value| value.as_bool()).unwrap_or(false),
    }
}

/// `dumpsys display` answers in well under a second; a device that does not is mirrored at the
/// default cap rather than waited on.
const DISPLAY_DUMP_TIMEOUT: Duration = Duration::from_secs(3);

/// The device's top refresh rate ([`mirror::display_refresh_rate`]), or `None` when adb or the
/// output gave nothing usable.
fn device_refresh_rate(adb: &str, serial: &str) -> Option<f32> {
    let mut command = Command::new(adb);
    command.args(["-s", serial, "shell", "dumpsys", "display"]);
    let output = crate::proc::output_with_timeout(&mut command, DISPLAY_DUMP_TIMEOUT).ok()??;
    if !output.status.success() {
        return None;
    }
    mirror::display_refresh_rate(&String::from_utf8_lossy(&output.stdout))
}

// ---------------------------------------------------------------------------
// refresh-rate pin (Settings `device.mirror.pin_refresh_rate`, off by default)
// ---------------------------------------------------------------------------
//
// An adaptive (LTPO, Samsung "Adaptive" motion smoothness) panel idles at a low rate and only
// ramps up once something moves, so the first frame after a tap can wait out a slow vsync. While
// a mirror runs, `settings put system min_refresh_rate <top rate>` holds it up; afterwards the
// previous value goes back. Only `min_refresh_rate` is touched — `peak_refresh_rate` (the user's
// own 60/120 choice) is left as it is.
//
// Putting it back has to survive more than a clean stop, so before anything is changed the
// previous value is written to `mirror-refresh` beside the store (`serial previous`, `null` for
// unset). It is restored, and the line removed, when the last mirror on the device ends, when the
// engine shuts down, at the next engine start, or at the next mirror start on that device —
// whichever first finds the device connected. No file (an in-memory store), or a previous value
// that cannot be written back verbatim, means no pin. A device that never comes back while Relay
// runs keeps the pin until it does; `adb shell settings delete system min_refresh_rate` clears it
// by hand (or puts back the value the file names).

/// Mirrors on each device holding the pin, keyed by record file and serial (one engine per
/// process in use, but the tests run several): the first pins, the last to end restores. The
/// lock also serializes every edit of the record files.
static REFRESH_PINS: Mutex<std::collections::BTreeMap<(PathBuf, String), usize>> = Mutex::new(std::collections::BTreeMap::new());

const REFRESH_SETTING: &str = "min_refresh_rate";
/// What `settings get` prints for a key that is not set; restored with `settings delete`.
const SETTING_UNSET: &str = "null";
const REFRESH_ADB_TIMEOUT: Duration = Duration::from_secs(3);

fn refresh_file(engine: &Engine) -> Option<PathBuf> {
    let store = engine.store.path();
    if store == Path::new(":memory:") {
        return None;
    }
    store.parent().map(|dir| dir.join("mirror-refresh"))
}

/// A `settings get` answer that can go back through `adb shell` verbatim: `null`, or a short
/// number-like word (`120.0`, `Infinity`). Nothing a shell would interpret.
fn restorable_setting(value: &str) -> bool {
    !value.is_empty() && value.len() <= 32 && value.chars().all(|c| c.is_ascii_alphanumeric() || c == '.' || c == '-')
}

fn read_refresh_records(path: &Path) -> Vec<(String, String)> {
    fs::read_to_string(path)
        .unwrap_or_default()
        .lines()
        .filter_map(|line| {
            let (serial, previous) = line.trim().rsplit_once(' ')?;
            (!serial.is_empty() && restorable_setting(previous)).then(|| (serial.to_string(), previous.to_string()))
        })
        .collect()
}

/// Replace the record file whole (written aside, synced, renamed), or remove it when empty. A
/// pin is only made once this has succeeded.
fn write_refresh_records(path: &Path, records: &[(String, String)]) -> std::io::Result<()> {
    if records.is_empty() {
        return match fs::remove_file(path) {
            Err(error) if error.kind() != std::io::ErrorKind::NotFound => Err(error),
            _ => Ok(()),
        };
    }
    let aside = path.with_extension("new");
    let mut file = OpenOptions::new().create(true).write(true).truncate(true).mode(0o600).open(&aside)?;
    for (serial, previous) in records {
        writeln!(file, "{serial} {previous}")?;
    }
    file.sync_all()?;
    fs::rename(&aside, path)
}

fn refresh_adb(adb: &Path, serial: &str, action: &[&str], timeout: Duration) -> Option<String> {
    let mut command = Command::new(adb);
    command.args(["-s", serial, "shell", "settings"]).args(action);
    let output = crate::proc::output_with_timeout(&mut command, timeout).ok()??;
    output.status.success().then(|| String::from_utf8_lossy(&output.stdout).trim().to_string())
}

fn read_refresh(adb: &Path, serial: &str, timeout: Duration) -> Option<String> {
    refresh_adb(adb, serial, &["get", "system", REFRESH_SETTING], timeout)
}

/// Put `serial`'s recorded value back and drop its record, if it has one. Caller holds
/// [`REFRESH_PINS`]. True when nothing is left to restore. Success is read back from the
/// device rather than taken from adb's exit status, which older adb always reports as 0.
fn restore_refresh_locked(path: &Path, adb: &Path, serial: &str, timeout: Duration) -> bool {
    let mut records = read_refresh_records(path);
    let Some(at) = records.iter().position(|(recorded, _)| recorded == serial) else { return true };
    let previous = records[at].1.clone();
    if previous == SETTING_UNSET {
        refresh_adb(adb, serial, &["delete", "system", REFRESH_SETTING], timeout);
    } else {
        refresh_adb(adb, serial, &["put", "system", REFRESH_SETTING, &previous], timeout);
    }
    if read_refresh(adb, serial, timeout).as_deref() != Some(previous.as_str()) {
        tracing::warn!(device = serial, previous = %previous, "could not restore the device's min_refresh_rate; retried at the next start");
        return false;
    }
    records.remove(at);
    let _ = write_refresh_records(path, &records);
    true
}

/// Hold `serial`'s panel at `rate` Hz for one mirror. True when this mirror now holds a pin
/// (and must [`unpin_refresh`]); false when it could not be made safely, which leaves the
/// device as it was.
fn pin_refresh(engine: &Engine, adb: &Path, serial: &str, rate: f32) -> bool {
    let Some(path) = refresh_file(engine) else { return false };
    let mut pins = REFRESH_PINS.lock().unwrap_or_else(|poison| poison.into_inner());
    let pin = (path.clone(), serial.to_string());
    if let Some(held) = pins.get_mut(&pin) {
        *held += 1;
        return true;
    }
    let mut records = read_refresh_records(&path);
    // A record a previous run left holds the true previous value: what the device has now is
    // that run's pin, so it must not become the value restored.
    if !records.iter().any(|(recorded, _)| recorded == serial) {
        let Some(previous) = read_refresh(adb, serial, REFRESH_ADB_TIMEOUT).filter(|value| restorable_setting(value)) else {
            return false;
        };
        records.push((serial.to_string(), previous));
        if write_refresh_records(&path, &records).is_err() {
            return false;
        }
    }
    // Floored: 120.00001 pins 120, and 59.94 must not pin above the mode it came from.
    let value = (rate.floor() as u32).to_string();
    refresh_adb(adb, serial, &["put", "system", REFRESH_SETTING, &value], REFRESH_ADB_TIMEOUT);
    if read_refresh(adb, serial, REFRESH_ADB_TIMEOUT).as_deref() != Some(value.as_str()) {
        restore_refresh_locked(&path, adb, serial, REFRESH_ADB_TIMEOUT);
        return false;
    }
    pins.insert(pin, 1);
    true
}

/// One mirror on `serial` let go of its pin; the last one restores.
fn unpin_refresh(engine: &Engine, adb: &Path, serial: &str) {
    let Some(path) = refresh_file(engine) else { return };
    let mut pins = REFRESH_PINS.lock().unwrap_or_else(|poison| poison.into_inner());
    let pin = (path.clone(), serial.to_string());
    match pins.get_mut(&pin) {
        Some(held) if *held > 1 => *held -= 1,
        Some(_) => {
            pins.remove(&pin);
            restore_refresh_locked(&path, adb, serial, REFRESH_ADB_TIMEOUT);
        }
        // Already put back by the engine's shutdown.
        None => {}
    }
}

/// A mirror on `serial` that does not pin: a record a crashed run left for it is put back now
/// rather than at the next engine start, unless another mirror here holds the pin.
fn settle_refresh(engine: &Engine, adb: &Path, serial: &str) {
    let Some(path) = refresh_file(engine) else { return };
    let pins = REFRESH_PINS.lock().unwrap_or_else(|poison| poison.into_inner());
    if !pins.contains_key(&(path.clone(), serial.to_string())) && read_refresh_records(&path).iter().any(|(recorded, _)| recorded == serial) {
        restore_refresh_locked(&path, adb, serial, REFRESH_ADB_TIMEOUT);
    }
}

/// Held by the worker for as long as its pin: dropping it (the worker's every way out, a panic
/// included) lets go.
struct RefreshPin {
    engine: Arc<Engine>,
    adb: PathBuf,
    serial: String,
}

impl Drop for RefreshPin {
    fn drop(&mut self) {
        unpin_refresh(&self.engine, &self.adb, &self.serial);
    }
}

/// The engine is shutting down: put back every pin its mirrors hold, whatever their count.
fn restore_all_refresh(engine: &Engine, mirrors: &[Arc<MirrorRuntime>]) {
    let Some(path) = refresh_file(engine) else { return };
    let mut pins = REFRESH_PINS.lock().unwrap_or_else(|poison| poison.into_inner());
    for runtime in mirrors {
        if pins.remove(&(path.clone(), runtime.device.clone())).is_some() {
            restore_refresh_locked(&path, Path::new(&runtime.adb), &runtime.device, SHUTDOWN_FORWARD_TIMEOUT);
        }
    }
}

/// Engine start: put back the pins a previous run recorded and did not restore, on every
/// recorded device that is connected now. The rest stay recorded for later. On its own thread.
fn restore_stale_refresh(engine: &Arc<Engine>) {
    let Some(path) = refresh_file(engine) else { return };
    if read_refresh_records(&path).is_empty() {
        return;
    }
    let engine = engine.clone();
    let _ = std::thread::Builder::new().name("mirror-refresh".into()).spawn(move || {
        let Ok(adb) = adb_path(&engine.store.lock()) else { return };
        let pins = REFRESH_PINS.lock().unwrap_or_else(|poison| poison.into_inner());
        for (serial, _) in read_refresh_records(&path) {
            let mut state = Command::new(&adb);
            state.args(["-s", &serial, "get-state"]);
            let present = crate::proc::output_with_timeout(&mut state, REFRESH_ADB_TIMEOUT)
                .ok()
                .flatten()
                .is_some_and(|output| String::from_utf8_lossy(&output.stdout).trim() == "device");
            if present && !pins.contains_key(&(path.clone(), serial.clone())) {
                restore_refresh_locked(&path, &adb, &serial, REFRESH_ADB_TIMEOUT);
            }
        }
    });
}

/// At start, before any mirror of this engine exists: remove the forwards a previous run of it
/// recorded and never removed (it crashed, or quit faster than adb). Each must still be in
/// `adb forward --list` exactly as recorded — serial, port and Relay's scrcpy socket — or it is
/// left alone: a port adb has since handed to someone else is theirs. On its own thread.
///
/// Also puts back any refresh-rate pin such a run left on a device (see [`pin_refresh`]).
pub fn clear_stale_forwards(engine: &Arc<Engine>) {
    restore_stale_refresh(engine);
    let Some(path) = forwards_file(engine) else { return };
    let recorded = {
        let _guard = FORWARDS_FILE.lock().unwrap_or_else(|poison| poison.into_inner());
        let text = fs::read_to_string(&path).unwrap_or_default();
        let _ = fs::remove_file(&path);
        text
    };
    let stale: Vec<(String, u16, u32)> = recorded
        .lines()
        .filter_map(parse_forward_line)
        .map(|(serial, port, scid)| (serial.to_string(), port, scid))
        .collect();
    if stale.is_empty() {
        return;
    }
    let engine = engine.clone();
    let _ = std::thread::Builder::new().name("mirror-forwards".into()).spawn(move || {
        let Ok(adb) = adb_path(&engine.store.lock()) else { return };
        let mut list = Command::new(&adb);
        list.args(["forward", "--list"]);
        let Ok(Some(output)) = crate::proc::output_with_timeout(&mut list, MIRROR_FORWARD_TIMEOUT) else { return };
        let listed = String::from_utf8_lossy(&output.stdout).into_owned();
        for (serial, port, scid) in stale {
            // `forward_args` ends with the device socket a mirror with this scid listens on.
            let socket = mirror::forward_args(&serial, scid).pop().unwrap_or_default();
            let line = format!("{serial} tcp:{port} {socket}");
            if listed.lines().any(|listed| listed.trim() == line) {
                let mut remove = Command::new(&adb);
                remove.args(mirror::forward_remove_args(&serial, port));
                let _ = crate::proc::output_with_timeout(&mut remove, MIRROR_FORWARD_TIMEOUT);
            }
        }
    });
}

/// The transport half of a mirror: push, forward, boot the server, pump the stream, and on the
/// way out report how it ended. Public so the integration tests can drive it against a fake
/// adb and a fake server; the engine itself only ever starts it from `device.mirror.start`.
#[doc(hidden)]
pub fn mirror_worker(engine: Arc<Engine>, runtime: Arc<MirrorRuntime>) {
    let fail = |code: &str, message: String| {
        mirror_failure(&engine, &runtime, MirrorState::Failed, code, message);
        engine.mirrors.lock().unwrap().remove(&runtime.id);
    };
    // Checked again here, right before the push: the start request's check ran before adb did.
    let server_jar = match mirror_server() {
        Ok(path) => path,
        Err((code, message)) => {
            fail(code, message);
            return;
        }
    };
    // The panel's refresh rate sets the frame-rate cap. Asked alongside the push and forward
    // rather than before them, so it adds no time to a start; a failure that returns early
    // leaves this thread to finish on its own under its deadline.
    let refresh_probe = {
        let (adb, serial) = (runtime.adb.clone(), runtime.device.clone());
        std::thread::spawn(move || device_refresh_rate(&adb, &serial))
    };

    if let Err(error) = mirror_adb(&runtime, mirror::push_args(&runtime.device, &server_jar.to_string_lossy()), MIRROR_PUSH_TIMEOUT) {
        fail("device.mirror_push_failed", error);
        return;
    }
    let port = match mirror_adb(&runtime, mirror::forward_args(&runtime.device, runtime.scid), MIRROR_FORWARD_TIMEOUT)
        .and_then(|value| value.trim().parse::<u16>().map_err(|_| format!("adb forward returned no port ({value:?})")))
    {
        Ok(port) => port,
        Err(error) => {
            fail("device.mirror_forward_failed", error);
            return;
        }
    };
    runtime.set_forward(port);
    record_forward(&engine, &runtime.device, port, runtime.scid);
    if runtime.stopped() {
        remove_forward(&engine, &runtime, MIRROR_FORWARD_TIMEOUT);
        engine.mirrors.lock().unwrap().remove(&runtime.id);
        return;
    }

    let settings = mirror_settings(&engine);
    let refresh = refresh_probe.join().ok().flatten();
    let video_codec_options = match settings.video_codec_options {
        Some(extra) => match mirror::merge_codec_options(mirror::BASE_CODEC_OPTIONS, &extra) {
            Ok(_) => extra,
            Err(error) => {
                tracing::warn!(mirror_id = runtime.id, %error, "device.mirror.video_codec_options ignored");
                String::new()
            }
        },
        None => String::new(),
    };
    let launch = mirror::ServerLaunch {
        max_size: runtime.max_size,
        video_bit_rate: runtime.bitrate,
        max_fps: settings.max_fps.unwrap_or_else(|| mirror::max_fps_for(refresh)),
        video_codec_options,
    };
    // Held until the worker returns, by any path; its drop puts the device's setting back.
    let refresh_pin = match refresh {
        Some(rate) if settings.pin_refresh_rate => pin_refresh(&engine, Path::new(&runtime.adb), &runtime.device, rate)
            .then(|| RefreshPin { engine: engine.clone(), adb: PathBuf::from(&runtime.adb), serial: runtime.device.clone() }),
        _ => {
            settle_refresh(&engine, Path::new(&runtime.adb), &runtime.device);
            None
        }
    };
    let args = mirror::server_shell_args(&runtime.device, runtime.scid, &launch);
    tracing::info!(
        mirror_id = runtime.id,
        device = %runtime.device,
        refresh_hz = refresh.unwrap_or(0.0),
        refresh_pinned = refresh_pin.is_some(),
        launch = %args.join(" "),
        "mirror server launch"
    );
    let mut command = Command::new(&runtime.adb);
    command
        .args(&args)
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    let mut server = match command.spawn() {
        Ok(server) => server,
        Err(error) => {
            remove_forward(&engine, &runtime, MIRROR_FORWARD_TIMEOUT);
            fail("device.mirror_spawn_failed", error.to_string());
            return;
        }
    };
    let logs: Arc<Mutex<Vec<String>>> = Arc::default();
    drain_server_lines(server.stdout.take(), logs.clone());
    drain_server_lines(server.stderr.take(), logs.clone());
    runtime.set_child(server);

    let result = (|| -> Result<(), String> {
        let mut video = connect_mirror_video(port, Instant::now() + Duration::from_secs(10))?;
        let control = TcpStream::connect(("127.0.0.1", port))
            .map_err(|error| format!("mirror control socket: {error}"))?;
        video.set_read_timeout(Some(Duration::from_secs(5))).map_err(|error| error.to_string())?;
        let mut name = [0u8; 64];
        video.read_exact(&mut name).map_err(|error| format!("mirror handshake (device name): {error}"))?;
        let mut codec = [0u8; 4];
        video.read_exact(&mut codec).map_err(|error| format!("mirror handshake (codec id): {error}"))?;
        if mirror::parse_codec_id(&codec) != mirror::CODEC_ID_H264 {
            return Err("mirror device did not provide H.264 video".into());
        }
        let _ = video.set_nodelay(true);
        let _ = control.set_nodelay(true);
        let keepalive = video.try_clone().map_err(|error| error.to_string())?;
        let mut drain = control.try_clone().map_err(|error| error.to_string())?;
        runtime.set_video(keepalive);
        runtime.install_control(control).map_err(|error| format!("mirror control socket: {error}"))?;
        // Device→host messages (clipboard pushes) have no consumer; read and drop them so the
        // device never blocks on a full socket.
        std::thread::spawn(move || {
            let mut bytes = [0u8; 4096];
            while drain.read(&mut bytes).is_ok_and(|count| count > 0) {}
        });
        let device_name = String::from_utf8_lossy(&name).trim_end_matches('\0').trim().to_string();
        // "Running" waits for the first unit — the session packet that carries the real
        // picture size — so the window and tap/swipe never work from the `wm size` estimate.
        video.set_read_timeout(Some(MIRROR_FIRST_UNIT_TIMEOUT)).map_err(|error| error.to_string())?;
        let mut announced = false;
        let trace = mirror::trace_enabled();
        let mut pushed: u64 = 0;
        let announce = |announced: &mut bool| {
            if *announced {
                return;
            }
            *announced = true;
            runtime.set_running(device_name.clone());
            let (width, height) = runtime.size();
            engine.emit_system(
                "mirror.changed",
                json!({"mirror_id":runtime.id,"device":runtime.device,"name":device_name,"state":"running","width":width,"height":height}),
            );
        };
        while !runtime.stopped() {
            let mut header = [0u8; 12];
            video.read_exact(&mut header).map_err(|error| {
                if announced { format!("mirror stream ended: {error}") } else { format!("mirror server sent no video: {error}") }
            })?;
            if !announced {
                // The first-unit deadline covers the handshake only; once frames flow, a
                // static screen can legitimately send nothing for minutes.
                video.set_read_timeout(None).map_err(|error| error.to_string())?;
            }
            match mirror::parse_stream_unit(&header) {
                mirror::StreamUnit::Session { width, height } => {
                    runtime.set_size(width, height);
                    if announced {
                        engine.emit_system(
                            "mirror.changed",
                            json!({"mirror_id":runtime.id,"device":runtime.device,"state":"running","width":width,"height":height}),
                        );
                    }
                    announce(&mut announced);
                }
                mirror::StreamUnit::Media { config, key, pts, len } => {
                    let header_ns = trace.then(mirror::monotonic_ns);
                    announce(&mut announced);
                    if len == 0 || len > 16 * 1024 * 1024 {
                        return Err("mirror stream desynced".into());
                    }
                    let mut packet = vec![0u8; 1 + len as usize];
                    packet[0] = u8::from(config) | (u8::from(key) << 1);
                    video.read_exact(&mut packet[1..]).map_err(|error| format!("mirror stream ended: {error}"))?;
                    runtime.push(packet);
                    // The worker is the runtime's only writer, so this is the seq the packet
                    // went out under — the key that joins it to the door's and client's lines.
                    pushed += 1;
                    if let Some(header_ns) = header_ns {
                        tracing::info!(
                            target: "mirror_trace",
                            stage = "read",
                            mono_ns = mirror::monotonic_ns(),
                            header_ns,
                            mirror_id = runtime.id,
                            seq = pushed,
                            pts_us = pts,
                            len,
                            key,
                            config,
                        );
                    }
                }
            }
        }
        Ok(())
    })();

    let requested_stop = runtime.stopped();
    runtime.stop();
    if let Some(mut child) = runtime.take_child() {
        let _ = child.wait();
    }
    remove_forward(&engine, &runtime, MIRROR_FORWARD_TIMEOUT);
    match result {
        // The video went because input could not reach the device: say that, not "stream ended".
        Err(error) if !requested_stop && runtime.control_lost().is_some() => {
            let lost = runtime.control_lost().unwrap_or(error);
            mirror_failure(
                &engine,
                &runtime,
                MirrorState::Failed,
                "device.mirror_control_lost",
                format!("input could no longer reach {} ({lost}); the mirror was ended", runtime.device),
            );
        }
        Err(error) if !requested_stop => {
            if device_present(&runtime) {
                let tail = logs.lock().unwrap().iter().rev().take(5).cloned().collect::<Vec<_>>();
                let message = if tail.is_empty() { error } else { format!("{error}\n{}", tail.into_iter().rev().collect::<Vec<_>>().join("\n")) };
                mirror_failure(&engine, &runtime, MirrorState::Failed, "device.mirror_stream_failed", message);
            } else {
                mirror_failure(
                    &engine,
                    &runtime,
                    MirrorState::Lost,
                    "device.mirror_device_lost",
                    format!("{} is no longer connected", runtime.device),
                );
            }
        }
        _ => {
            runtime.finish(MirrorState::Stopped, None, None);
        }
    }
    engine.mirrors.lock().unwrap().remove(&runtime.id);
}

fn event_coordinate(event: &Value, name: &str, limit: u32) -> Result<i32, BusError> {
    let value = event.get(name).and_then(Value::as_i64).ok_or_else(|| {
        BusError::invalid("device.input", format!("event.{name} must be an integer"))
    })?;
    if !(0..limit as i64).contains(&value) {
        return Err(BusError::invalid("device.input", format!("event.{name} is outside the mirrored display ({limit} px)")));
    }
    Ok(value as i32)
}

/// Only the named fields: a `tap` or `swipe` carrying anything else (a `duration_ms`, say) is
/// refused rather than run without it.
fn event_fields(event: &Value, kind: &str, fields: &[&str]) -> Result<(), BusError> {
    let extra = event.as_object().into_iter().flatten().map(|(key, _)| key.as_str()).find(|key| *key != "type" && !fields.contains(key));
    match extra {
        Some(key) => Err(BusError::invalid("device.input", format!("{kind} takes only {}; event.{key} is not supported", fields.join(", ")))),
        None => Ok(()),
    }
}

/// The most a `text` input event may carry (see `send_input`).
const TEXT_INPUT_MAX: usize = 64 * 1024;

/// Encode and write one `device.mirror.input` event. Pure memory and one socket write — no
/// store access — which is what lets the engine answer it on the fast path. `tap`/`swipe`
/// coordinates are in the stream's *current* size, rotation included.
///
/// A `swipe` is instant: DOWN, one MOVE and UP in a single write, so the view sees a fling, not
/// a timed drag. A caller that needs a slow drag sends its own `touch` stream (DOWN, MOVEs, UP)
/// at the pace it wants, as the native client does.
pub fn send_input(runtime: &MirrorRuntime, event: &Value) -> Result<(), BusError> {
    let kind = event.get("type").and_then(Value::as_str)
        .ok_or_else(|| BusError::invalid("device.input", "event.type is required"))?;
    let (width, height) = runtime.size();
    let (w, h) = (width as u16, height as u16);
    let message = match kind {
        "tap" => {
            event_fields(event, kind, &["x", "y"])?;
            let x = event_coordinate(event, "x", width)?;
            let y = event_coordinate(event, "y", height)?;
            let mut bytes = mirror::touch(mirror::ACTION_DOWN, x, y, w, h, 1.0).to_vec();
            bytes.extend_from_slice(&mirror::touch(mirror::ACTION_UP, x, y, w, h, 0.0));
            bytes
        }
        "swipe" => {
            event_fields(event, kind, &["x1", "y1", "x2", "y2"])?;
            let x1 = event_coordinate(event, "x1", width)?;
            let y1 = event_coordinate(event, "y1", height)?;
            let x2 = event_coordinate(event, "x2", width)?;
            let y2 = event_coordinate(event, "y2", height)?;
            let mut bytes = mirror::touch(mirror::ACTION_DOWN, x1, y1, w, h, 1.0).to_vec();
            bytes.extend_from_slice(&mirror::touch(mirror::ACTION_MOVE, x2, y2, w, h, 1.0));
            bytes.extend_from_slice(&mirror::touch(mirror::ACTION_UP, x2, y2, w, h, 0.0));
            bytes
        }
        _ => {
            let input = serde_json::from_value::<mirror::InputMsg>(event.clone())
                .map_err(|error| BusError::invalid("device.input", error.to_string()))?;
            // One clipboard message cannot be split; cut short, the phone would paste a prefix
            // and the caller would never know.
            if let mirror::InputMsg::SetClipboard { text, .. } = &input {
                if text.len() > mirror::SET_CLIPBOARD_MAX_LENGTH {
                    return Err(BusError::invalid("device.input", format!(
                        "clipboard text is {} bytes; one setclipboard carries at most {}",
                        text.len(), mirror::SET_CLIPBOARD_MAX_LENGTH,
                    )).with_hint("split it, and send each part as its own setclipboard with paste on"));
                }
            }
            // Typed text goes as one inject message per 300 bytes, all written under the control
            // lock: a 64 MiB line was some 220,000 of them, with every other input queued behind.
            if let mirror::InputMsg::Text { text } = &input {
                if text.len() > TEXT_INPUT_MAX {
                    return Err(BusError::invalid("device.input", format!(
                        "text is {} bytes; type \"text\" takes at most {TEXT_INPUT_MAX}",
                        text.len(),
                    )).with_hint(format!(
                        "send it as {{\"type\":\"setclipboard\",\"text\":…,\"paste\":true}}: one message, up to {} bytes",
                        mirror::SET_CLIPBOARD_MAX_LENGTH,
                    )));
                }
            }
            mirror::encode_input(&input)
        }
    };
    runtime.send_control(message)
        .map_err(|error| BusError::unavailable("device.input_failed", error.to_string()))
}

/// What `device.run` settled with nothing locked (D149): the device answered, the checkout,
/// its command and its SDK are resolved. The transaction only records the run.
struct RunPrepared {
    adb: PathBuf,
    requested_root: String,
    root: PathBuf,
    command: String,
    gradle_init: Option<tempfile::NamedTempFile>,
    sdk_root: Option<PathBuf>,
    default_gradle_command: bool,
    sync_primary: bool,
    source: String,
}

/// The live runs and builds recorded in the checkout `root`.
fn live_runs_in(conn: &rusqlite::Connection, root: &Path) -> Result<Vec<Id>, BusError> {
    conn.prepare_cached("SELECT id FROM device_runs WHERE worktree=?1 AND state IN ('building','running') ORDER BY id").bus()?
        .query_map([root.display().to_string()], |row| row.get(0)).bus()?
        .collect::<rusqlite::Result<Vec<Id>>>().bus()
}

/// Refuse to start in `root` while a run is fetching and fast-forwarding it.
fn refuse_while_syncing(engine: &Engine, root: &Path) -> Result<(), BusError> {
    let syncing = engine.device_runs.lock().unwrap_or_else(PoisonError::into_inner).values().find(|run| run.syncing(root)).map(|run| run.id);
    match syncing {
        Some(id) => Err(BusError::conflict(
            "device.checkout_syncing",
            format!("run {id} is updating {} to its upstream", root.display()),
        ).with_hint("try again once its log shows the primary sync line (seconds, or as long as the fetch takes)")),
        None => Ok(()),
    }
}

/// A run registered in memory — its live runtime and, for a device run, its lease — by a
/// request whose transaction has not committed yet. Neither is part of the transaction, so a
/// rollback alone would leave a phantom run holding the device forever. The guard rides in
/// the request's after-commit closure: committed, the closure runs and [`PendingRun::keep`]
/// disarms it; rolled back, the closure is dropped unrun and the guard takes both back out.
struct PendingRun {
    engine: Option<Arc<Engine>>,
    id: Id,
}

impl PendingRun {
    fn register(engine: &Engine, runtime: Arc<RunRuntime>) -> Result<Self, BusError> {
        let engine = engine.arc().ok_or_else(|| BusError::internal("the engine is shutting down"))?;
        let id = runtime.id;
        engine.device_runs.lock().unwrap_or_else(PoisonError::into_inner).insert(id, runtime);
        Ok(Self { engine: Some(engine), id })
    }
    fn keep(mut self) {
        self.engine = None;
    }
}

impl Drop for PendingRun {
    fn drop(&mut self) {
        let Some(engine) = self.engine.take() else { return };
        engine.device_runs.lock().unwrap_or_else(PoisonError::into_inner).remove(&self.id);
        // Silently: the acquisition's event was never sent either.
        engine.device_leases.release_run(self.id);
    }
}

/// `relay/brisk-otter` for a pooled checkout, `the primary checkout` otherwise: what a refused
/// caller needs to recognize whose build is on the phone.
fn run_source(root: &Path, project_path: &str) -> String {
    let primary = std::fs::canonicalize(project_path).ok();
    if primary.as_deref() == Some(root) {
        return "the primary checkout".into();
    }
    gix::open(root).ok()
        .and_then(|repo| repo.head_name().ok().flatten().map(|name| name.shorten().to_string()))
        .unwrap_or_else(|| root.display().to_string())
}

fn resolve_run_root(
    conn: &rusqlite::Connection,
    project_id: Id,
    project_path: &str,
    requested: Option<&str>,
    integration_id: Option<Id>,
) -> Result<PathBuf, BusError> {
    let selected = match integration_id {
        Some(integration_id) => integration_root(conn, project_id, integration_id)?,
        None => requested
            .map(PathBuf::from)
            .unwrap_or_else(|| PathBuf::from(project_path)),
    };
    checked_run_root(project_path, &selected)
}

/// The worktree a passed integration left behind: the store half of [`resolve_run_root`].
fn integration_root(conn: &rusqlite::Connection, project_id: Id, integration_id: Id) -> Result<PathBuf, BusError> {
    let row: Option<(Id, Option<String>, String)> = conn
        .prepare_cached("SELECT project_id,worktree,state FROM integrations WHERE id=?1")
        .bus()?
        .query_row([integration_id], |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)))
        .optional()
        .bus()?;
    let (owner, path, state) = row.ok_or_else(|| {
        BusError::not_found(
            "integration.not_found",
            format!("no integration {integration_id}"),
        )
    })?;
    if owner != project_id {
        return Err(BusError::invalid(
            "device.integration_project",
            "integration belongs to another project",
        ));
    }
    if state != "passed" {
        return Err(BusError::conflict(
            "device.integration_not_ready",
            format!("integration {integration_id} is {state}"),
        ));
    }
    Ok(PathBuf::from(path.ok_or_else(|| {
        BusError::conflict(
            "device.integration_not_ready",
            "integration has no worktree",
        )
    })?))
}

/// The filesystem half of [`resolve_run_root`]: `selected`, canonical, and one of the project's
/// checkouts.
fn checked_run_root(project_path: &str, selected: &Path) -> Result<PathBuf, BusError> {
    let selected = std::fs::canonicalize(selected)
        .map_err(|e| BusError::invalid("device.worktree", e.to_string()))?;
    // Membership only. Listing the worktrees with a dirty check ran a `git status` per checkout
    // before a build had even looked for its Gradle wrapper (PERF §1.4).
    if !worktree::contains(Path::new(project_path), &selected)
        .map_err(|e| BusError::unavailable("worktree.list_failed", e.to_string()))?
    {
        return Err(BusError::invalid(
            "device.worktree",
            "worktree does not belong to project",
        ));
    }
    Ok(selected)
}

fn run_command(
    root: &Path,
    configured: Option<&str>,
    variant: Option<&str>,
    init_script: Option<&Path>,
) -> Result<String, BusError> {
    if let Some(command) = configured.filter(|command| !command.trim().is_empty()) {
        return Ok(command.to_string());
    }
    let wrapper = gradle_wrapper(root).ok_or_else(|| {
        BusError::unavailable(
            "device.gradle_missing",
            "no run command or Gradle wrapper is configured",
        )
    })?;
    let task = format!("install{}", gradle_variant(variant.unwrap_or("debug"))?);
    let init = init_script.map(|path| format!(" --init-script '{}'", path.display())).unwrap_or_default();
    Ok(format!("{wrapper}{init} {task}"))
}

/// `./gradlew` as the run command spells it, from the project root, or else from the one
/// subdirectory that has a wrapper (an Android app nested in a larger repository); `None` when
/// the checkout has none. Commands run from the root, and Gradle looks for the build where it
/// is started rather than beside the wrapper, so a nested one is pointed at its own directory.
fn gradle_wrapper(root: &Path) -> Option<String> {
    if root.join("gradlew").is_file() {
        return Some("./gradlew".into());
    }
    let mut nested = std::fs::read_dir(root).ok()?.flatten()
        .map(|entry| entry.file_name().to_string_lossy().into_owned())
        .filter(|name| !name.starts_with('.') && name.chars().all(|c| c.is_ascii_alphanumeric() || matches!(c, '_' | '-' | '.')))
        .filter(|name| root.join(name).join("gradlew").is_file());
    let dir = nested.next()?;
    nested.next().is_none().then(|| format!("./{dir}/gradlew -p {dir}"))
}

/// `release` → `Release`: the variant as Gradle spells it inside a task name. Rejects
/// anything that would not survive being pasted into a shell command.
fn gradle_variant(variant: &str) -> Result<String, BusError> {
    if variant.is_empty() || !variant.chars().all(|c| c.is_ascii_alphanumeric() || c == '_') {
        return Err(BusError::invalid(
            "device.variant",
            "variant may contain only letters, numbers, and underscores",
        ));
    }
    let mut chars = variant.chars();
    Ok(format!("{}{}", chars.next().unwrap().to_ascii_uppercase(), chars.as_str()))
}

/// The format a build is recorded and run with: `apk` when none is asked for, and `bundle` for
/// `aab`, the file extension, which is what the desktop and phone pickers send. Anything else is
/// passed through for [`build_command`] to refuse.
fn artifact_format(requested: Option<&str>) -> String {
    match requested.map(str::trim) {
        None | Some("") => "apk".into(),
        Some(format) if format.eq_ignore_ascii_case("aab") => "bundle".into(),
        Some(format) => format.to_string(),
    }
}

/// The Gradle task a release build runs. `publish` hands the artifact to Gradle Play
/// Publisher, which owns the Play credentials. An optional Relay signing init script changes
/// only this invocation and never writes into the selected Android checkout (D152, D157).
fn build_command(
    root: &Path,
    variant: &str,
    format: &str,
    publish: bool,
    signing_init: Option<&Path>,
) -> Result<String, BusError> {
    let wrapper = gradle_wrapper(root).ok_or_else(|| {
        BusError::unavailable(
            "device.gradle_missing",
            "no Gradle wrapper in the selected worktree",
        )
    })?;
    let variant = gradle_variant(variant)?;
    let task = match (publish, format) {
        (false, "apk") => format!("assemble{variant}"),
        (false, "bundle") => format!("bundle{variant}"),
        (true, "apk") => format!("publish{variant}Apk"),
        (true, "bundle") => format!("publish{variant}Bundle"),
        _ => {
            return Err(BusError::invalid(
                "device.format",
                "format must be \"apk\" or \"bundle\"",
            ))
        }
    };
    let signing = signing_init
        .map(|path| format!(" --init-script '{}'", path.display()))
        .unwrap_or_default();
    Ok(format!("{wrapper} {task}{signing}"))
}

const GRADLE_RELAY_SIGNING_INIT: &str = r#"gradle.beforeProject { target ->
    target.plugins.withId("com.android.application") {
        def androidComponents = target.extensions.getByName("androidComponents")
        androidComponents.finalizeDsl { android ->
            def relaySigning = android.signingConfigs.findByName("relayRelease")
            if (relaySigning == null) relaySigning = android.signingConfigs.create("relayRelease")
            relaySigning.storeFile = target.file(System.getenv("RELAY_SIGNING_STORE_FILE"))
            relaySigning.storePassword = System.getenv("RELAY_SIGNING_STORE_PASSWORD")
            relaySigning.keyAlias = System.getenv("RELAY_SIGNING_KEY_ALIAS")
            relaySigning.keyPassword = System.getenv("RELAY_SIGNING_KEY_PASSWORD")
            def release = android.buildTypes.findByName("release")
            if (release == null) throw new GradleException("Relay signing needs an Android release build type")
            release.signingConfig = relaySigning
        }
    }
}
"#;

fn signing_init_script() -> Result<tempfile::NamedTempFile, BusError> {
    let mut file = tempfile::Builder::new()
        .prefix("relay-signing-")
        .suffix(".gradle")
        .tempfile()
        .map_err(|error| BusError::unavailable("device.signing_init_failed", error.to_string()))?;
    file.write_all(GRADLE_RELAY_SIGNING_INIT.as_bytes())
        .map_err(|error| BusError::unavailable("device.signing_init_failed", error.to_string()))?;
    Ok(file)
}

const SIGNING_PROFILE_FILE: &str = "profile.json";
const SIGNING_KEYSTORE_FILE: &str = "release.p12";
const TEST_SIGNING_SECRET_FILE: &str = ".test-secret";

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct SigningProfileDisk {
    project: String,
    key_alias: String,
    keystore: String,
    #[serde(default = "enabled_by_default")]
    enabled: bool,
}

#[derive(Debug, Clone)]
struct SigningRuntimeProfile {
    enabled: bool,
    key_alias: String,
    keystore: PathBuf,
    secret_id: String,
    directory: PathBuf,
}

fn enabled_by_default() -> bool {
    true
}

fn validate_signing_alias(alias: &str) -> Result<(), BusError> {
    if alias.is_empty()
        || alias.len() > 64
        || !alias
            .chars()
            .all(|character| character.is_ascii_alphanumeric() || matches!(character, '.' | '_' | '-'))
    {
        return Err(BusError::invalid(
            "device.signing_alias",
            "key alias must be 1-64 letters, numbers, dots, underscores, or hyphens",
        ));
    }
    Ok(())
}

fn validate_signing_input(alias: &str, password: &str) -> Result<(), BusError> {
    validate_signing_alias(alias)?;
    let password_len = password.chars().count();
    if !(6..=256).contains(&password_len) || password.contains(['\0', '\r', '\n']) {
        return Err(BusError::invalid(
            "device.signing_password",
            "password must be 6-256 characters with no line breaks",
        ));
    }
    Ok(())
}

fn signing_root(engine: &Engine) -> PathBuf {
    engine
        .store
        .path()
        .parent()
        .filter(|path| !path.as_os_str().is_empty())
        .map(|path| path.join("signing"))
        .unwrap_or_else(|| std::env::temp_dir().join(format!("relay-signing-test-{}", std::process::id())))
}

fn signing_project_key(project: &str) -> String {
    let mut hash = Sha256::new();
    hash.update(project.as_bytes());
    hash.finalize().iter().map(|byte| format!("{byte:02x}")).collect()
}

fn signing_directory(engine: &Engine, project: &str) -> PathBuf {
    signing_root(engine).join(signing_project_key(project))
}

fn load_signing_profile_disk(
    engine: &Engine,
    project: &str,
) -> Result<Option<(PathBuf, SigningProfileDisk)>, BusError> {
    let directory = signing_directory(engine, project);
    let profile_path = directory.join(SIGNING_PROFILE_FILE);
    if !profile_path.is_file() {
        return Ok(None);
    }
    let text = fs::read_to_string(&profile_path)
        .map_err(|error| BusError::unavailable("device.signing_profile_unreadable", error.to_string()))?;
    let profile: SigningProfileDisk = serde_json::from_str(&text)
        .map_err(|error| BusError::unavailable("device.signing_profile_invalid", error.to_string()))?;
    if profile.project != project || profile.keystore != SIGNING_KEYSTORE_FILE {
        return Err(BusError::unavailable(
            "device.signing_profile_invalid",
            "the saved signing profile does not match this project",
        ));
    }
    validate_signing_alias(&profile.key_alias)?;
    let keystore = directory.join(&profile.keystore);
    if !keystore.is_file() {
        return Err(BusError::unavailable(
            "device.signing_keystore_missing",
            format!("saved keystore is missing: {}", keystore.display()),
        ));
    }
    Ok(Some((directory, profile)))
}

fn load_signing_profile(engine: &Engine, project: &str) -> Result<Option<SigningRuntimeProfile>, BusError> {
    let Some((directory, profile)) = load_signing_profile_disk(engine, project)? else {
        return Ok(None);
    };
    Ok(Some(SigningRuntimeProfile {
        enabled: profile.enabled,
        key_alias: profile.key_alias,
        keystore: directory.join(&profile.keystore),
        secret_id: signing_project_key(project),
        directory,
    }))
}

/// The profile a release build signs with, if any. A profile that cannot be loaded refuses
/// the build only while it is still enabled: one switched off is ignored whatever state its
/// keystore is in, so a broken profile never holds every release build hostage.
fn build_signing_profile(engine: &Engine, project: &str) -> Result<Option<SigningRuntimeProfile>, BusError> {
    match load_signing_profile(engine, project) {
        Ok(profile) => Ok(profile.filter(|profile| profile.enabled)),
        Err(_) if signing_profile_switched_off(&signing_directory(engine, project)) => Ok(None),
        Err(error) => Err(error.with_hint("switch Relay signing off, or create the profile again")),
    }
}

/// Whether the saved profile, read leniently, says `"enabled": false`.
fn signing_profile_switched_off(directory: &Path) -> bool {
    fs::read_to_string(directory.join(SIGNING_PROFILE_FILE)).ok()
        .and_then(|text| serde_json::from_str::<Value>(&text).ok())
        .and_then(|value| value.get("enabled").and_then(Value::as_bool))
        == Some(false)
}

/// Move a profile directory that cannot be used out of the way, keeping it: a broken profile
/// may still hold the only copy of an upload key. Gone already (a racing create) is fine.
fn quarantine_signing_directory(directory: &Path) -> Result<(), BusError> {
    let stamp = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|elapsed| elapsed.as_millis()).unwrap_or(0);
    let name = directory.file_name().map(|name| name.to_string_lossy().into_owned()).unwrap_or_default();
    let aside = (0..).map(|n| directory.with_file_name(format!("{name}.broken-{stamp}-{n}")))
        .find(|path| !path.exists()).expect("an unused name");
    match fs::rename(directory, aside) {
        Err(error) if error.kind() != std::io::ErrorKind::NotFound => {
            Err(BusError::unavailable("device.signing_store_failed", error.to_string()))
        }
        _ => Ok(()),
    }
}

fn signing_profile_out(engine: &Engine, project: &str) -> Result<SigningProfileOut, BusError> {
    let Some(profile) = load_signing_profile(engine, project)? else {
        return Ok(SigningProfileOut { configured: false, enabled: false, key_alias: None, keystore: None });
    };
    Ok(SigningProfileOut {
        configured: true,
        enabled: profile.enabled,
        key_alias: Some(profile.key_alias),
        keystore: Some(profile.keystore.display().to_string()),
    })
}

fn create_signing_profile(
    engine: &Engine,
    project: &str,
    alias: &str,
    password: &str,
) -> Result<SigningProfileOut, BusError> {
    validate_signing_input(alias, password)?;
    let root = signing_root(engine);
    fs::create_dir_all(&root)
        .map_err(|error| BusError::unavailable("device.signing_store_failed", error.to_string()))?;
    fs::set_permissions(&root, fs::Permissions::from_mode(0o700))
        .map_err(|error| BusError::unavailable("device.signing_store_failed", error.to_string()))?;
    let directory = signing_directory(engine, project);
    let _creating = CreatingSigning::claim(&directory)?;
    // Only a usable profile blocks a new one. A directory left half-created by a crash, or a
    // profile whose file or keystore broke, is set aside so the project can start over.
    if directory.exists() {
        if let Ok(Some(_)) = load_signing_profile_disk(engine, project) {
            return Err(BusError::conflict("device.signing_exists", "this project already has a Relay signing profile"));
        }
        quarantine_signing_directory(&directory)?;
    }
    fs::create_dir(&directory).map_err(|error| {
        if error.kind() == std::io::ErrorKind::AlreadyExists {
            BusError::conflict("device.signing_exists", "this project already has a Relay signing profile")
        } else {
            BusError::unavailable("device.signing_store_failed", error.to_string())
        }
    })?;

    let keystore = directory.join(SIGNING_KEYSTORE_FILE);
    let result = (|| {
        fs::set_permissions(&directory, fs::Permissions::from_mode(0o700))
            .map_err(|error| BusError::unavailable("device.signing_store_failed", error.to_string()))?;
        generate_signing_keystore(engine.instance, &keystore, alias, password)?;
        let profile = SigningProfileDisk {
            project: project.to_string(),
            key_alias: alias.to_string(),
            keystore: SIGNING_KEYSTORE_FILE.to_string(),
            enabled: true,
        };
        let bytes = serde_json::to_vec_pretty(&profile)
            .map_err(|error| BusError::internal(format!("serializing signing profile: {error}")))?;
        write_private_new(&directory.join(SIGNING_PROFILE_FILE), &bytes)
            .map_err(|error| BusError::unavailable("device.signing_store_failed", error.to_string()))?;
        store_signing_secret(engine.instance, &directory, &signing_project_key(project), project, password)?;
        Ok(SigningProfileOut {
            configured: true,
            enabled: true,
            key_alias: Some(alias.to_string()),
            keystore: Some(keystore.display().to_string()),
        })
    })();
    if result.is_err() {
        let _ = fs::remove_dir_all(&directory);
    }
    result
}

/// Profile directories a `device.signing.create` is filling in right now. A directory with no
/// usable profile is abandoned — safe to set aside — only when no create in this process owns it.
static SIGNING_CREATING: Mutex<Vec<PathBuf>> = Mutex::new(Vec::new());

struct CreatingSigning(PathBuf);

impl CreatingSigning {
    fn claim(directory: &Path) -> Result<Self, BusError> {
        let mut creating = SIGNING_CREATING.lock().unwrap_or_else(|poison| poison.into_inner());
        if creating.iter().any(|path| path == directory) {
            return Err(BusError::conflict("device.signing_exists", "a Relay signing profile is already being created for this project"));
        }
        creating.push(directory.to_path_buf());
        Ok(Self(directory.to_path_buf()))
    }
}

impl Drop for CreatingSigning {
    fn drop(&mut self) {
        SIGNING_CREATING.lock().unwrap_or_else(|poison| poison.into_inner()).retain(|path| path != &self.0);
    }
}

fn set_signing_enabled(engine: &Engine, project: &str, enabled: bool) -> Result<SigningProfileOut, BusError> {
    let loaded = match load_signing_profile_disk(engine, project) {
        // Switching off a profile that cannot be loaded sets it aside: it could never sign
        // anything, and it must not keep refusing builds or a fresh `device.signing.create`.
        Err(_) if !enabled => {
            quarantine_signing_directory(&signing_directory(engine, project))?;
            return signing_profile_out(engine, project);
        }
        Err(error) => return Err(error.with_hint("create the Relay signing profile again")),
        Ok(loaded) => loaded,
    };
    let Some((directory, mut profile)) = loaded else {
        return Err(BusError::not_found(
            "device.signing_missing",
            "this project has no Relay signing profile",
        ));
    };
    profile.enabled = enabled;
    let bytes = serde_json::to_vec_pretty(&profile)
        .map_err(|error| BusError::internal(format!("serializing signing profile: {error}")))?;
    write_private_replace(&directory.join(SIGNING_PROFILE_FILE), &bytes)
        .map_err(|error| BusError::unavailable("device.signing_store_failed", error.to_string()))?;
    signing_profile_out(engine, project)
}

fn write_private_new(path: &Path, bytes: &[u8]) -> std::io::Result<()> {
    let mut file = OpenOptions::new().write(true).create_new(true).mode(0o600).open(path)?;
    file.write_all(bytes)?;
    file.sync_all()
}

fn write_private_replace(path: &Path, bytes: &[u8]) -> std::io::Result<()> {
    let mut file = tempfile::Builder::new().prefix(".relay-signing-profile-").tempfile_in(
        path.parent().ok_or_else(|| std::io::Error::other("signing profile has no parent directory"))?,
    )?;
    file.write_all(bytes)?;
    file.as_file().sync_all()?;
    file.persist(path).map_err(|error| error.error)?;
    Ok(())
}

fn keytool_path() -> PathBuf {
    std::env::var_os("JAVA_HOME")
        .map(PathBuf::from)
        .map(|home| home.join("bin/keytool"))
        .filter(|path| path.is_file())
        .unwrap_or_else(|| PathBuf::from("keytool"))
}

fn generate_signing_keystore(instance: Instance, path: &Path, alias: &str, password: &str) -> Result<(), BusError> {
    if instance == Instance::Test {
        return write_private_new(path, b"Relay test keystore")
            .map_err(|error| BusError::unavailable("device.keytool_failed", error.to_string()));
    }
    let mut command = Command::new(keytool_path());
    command
        .args([
            "-genkeypair",
            "-keystore",
            path.to_string_lossy().as_ref(),
            "-storetype",
            "PKCS12",
            "-alias",
            alias,
            "-keyalg",
            "RSA",
            "-keysize",
            "4096",
            "-validity",
            "10000",
            "-dname",
            "CN=Relay Android Release",
            "-storepass:env",
            "RELAY_SIGNING_PASSWORD",
            "-keypass:env",
            "RELAY_SIGNING_PASSWORD",
        ])
        .env("RELAY_SIGNING_PASSWORD", password);
    let output = crate::proc::output_with_timeout(&mut command, Duration::from_secs(90))
        .map_err(|error| BusError::unavailable("device.keytool_missing", error.to_string()))?
        .ok_or_else(|| BusError::unavailable("device.keytool_timeout", "keytool did not finish within 90 seconds"))?;
    if !output.status.success() {
        return Err(BusError::conflict("device.keytool_failed", keytool_error(&output.stdout, &output.stderr)));
    }
    fs::set_permissions(path, fs::Permissions::from_mode(0o600))
        .map_err(|error| BusError::unavailable("device.signing_store_failed", error.to_string()))
}

/// Why keytool failed. It prints `keytool error: ...` on stdout; stderr has only its progress
/// line (`Generating 4096-bit RSA key pair ...`), which is reported only when stdout is empty.
fn keytool_error(stdout: &[u8], stderr: &[u8]) -> String {
    let stdout = String::from_utf8_lossy(stdout);
    let stderr = String::from_utf8_lossy(stderr);
    stdout.lines().map(str::trim).find(|line| line.starts_with("keytool error"))
        .map(str::to_string)
        .or_else(|| Some(stdout.trim().to_string()).filter(|text| !text.is_empty()))
        .unwrap_or_else(|| stderr.trim().to_string())
}

fn store_signing_secret(
    instance: Instance,
    directory: &Path,
    secret_id: &str,
    project: &str,
    password: &str,
) -> Result<(), BusError> {
    if instance == Instance::Test {
        return write_private_new(&directory.join(TEST_SIGNING_SECRET_FILE), password.as_bytes())
            .map_err(|error| BusError::unavailable("device.signing_secret_failed", error.to_string()));
    }
    const STORE: &str = r#"printf %s "$RELAY_SIGNING_PASSWORD" | secret-tool store --label="$RELAY_SIGNING_LABEL" application Relay purpose android-signing project "$RELAY_SIGNING_PROJECT""#;
    let label = format!(
        "Relay Android signing: {}",
        Path::new(project).file_name().and_then(|name| name.to_str()).unwrap_or(project)
    );
    let mut command = Command::new("sh");
    command
        .args(["-c", STORE])
        .env("RELAY_SIGNING_PASSWORD", password)
        .env("RELAY_SIGNING_LABEL", label)
        .env("RELAY_SIGNING_PROJECT", secret_id);
    let output = crate::proc::output_with_timeout(&mut command, Duration::from_secs(30))
        .map_err(|error| BusError::unavailable("device.secret_service_missing", error.to_string()))?
        .ok_or_else(|| BusError::unavailable("device.signing_secret_timeout", "Linux Secret Service did not answer within 30 seconds"))?;
    if output.status.success() {
        Ok(())
    } else {
        Err(BusError::unavailable(
            "device.signing_secret_failed",
            String::from_utf8_lossy(&output.stderr).trim().to_string(),
        )
        .with_hint("install libsecret and unlock your desktop keyring"))
    }
}

fn load_signing_secret(instance: Instance, profile: &SigningRuntimeProfile) -> Result<String, BusError> {
    if instance == Instance::Test {
        return fs::read_to_string(profile.directory.join(TEST_SIGNING_SECRET_FILE))
            .map_err(|error| BusError::unavailable("device.signing_secret_missing", error.to_string()));
    }
    let mut command = Command::new("secret-tool");
    command.args([
        "lookup",
        "application",
        "Relay",
        "purpose",
        "android-signing",
        "project",
        &profile.secret_id,
    ]);
    let output = crate::proc::output_with_timeout(&mut command, Duration::from_secs(15))
        .map_err(|error| BusError::unavailable("device.secret_service_missing", error.to_string()))?
        .ok_or_else(|| BusError::unavailable("device.signing_secret_timeout", "Linux Secret Service did not answer within 15 seconds"))?;
    let password = String::from_utf8_lossy(&output.stdout).trim_end_matches(['\r', '\n']).to_string();
    if !output.status.success() || password.is_empty() {
        return Err(BusError::unavailable(
            "device.signing_secret_missing",
            "the password for this Relay signing profile is not in Linux Secret Service",
        )
        .with_hint("unlock your desktop keyring, then rebuild"));
    }
    Ok(password)
}

const GRADLE_ALLOW_DOWNGRADE_INIT: &str = r#"allprojects {
    plugins.withId("com.android.application") {
        def android = extensions.findByName("android")
        if (android.hasProperty("installation")) {
            android.installation.installOptions.add("-d")
        } else if (android.hasProperty("adbOptions")) {
            android.adbOptions.installOptions.add("-d")
        }
    }
}
"#;

fn gradle_init_script() -> Result<tempfile::NamedTempFile, BusError> {
    let mut file = tempfile::Builder::new().prefix("relay-gradle-").suffix(".gradle").tempfile()
        .map_err(|error| BusError::unavailable("device.gradle_init_failed", error.to_string()))?;
    file.write_all(GRADLE_ALLOW_DOWNGRADE_INIT.as_bytes())
        .map_err(|error| BusError::unavailable("device.gradle_init_failed", error.to_string()))?;
    Ok(file)
}

#[derive(Debug)]
struct PrimarySyncError {
    code: &'static str,
    message: String,
}

impl PrimarySyncError {
    fn new(code: &'static str, message: impl Into<String>) -> Self {
        Self { code, message: message.into() }
    }
}

/// A local git query (rev-parse, config, merge-base, diff) on the primary checkout.
const SYNC_GIT_TIMEOUT: Duration = Duration::from_secs(30);
/// The one network step of the primary sync. A remote that stops answering fails the run
/// instead of leaving it in 'building' with its device lease held.
const SYNC_FETCH_TIMEOUT: Duration = Duration::from_secs(120);

/// `git -C root args` under a deadline. `Err` carries what to tell the run.
fn git_output(root: &Path, args: &[&str]) -> Result<std::process::Output, String> {
    let mut command = Command::new("git");
    command.arg("-C").arg(root).args(args);
    crate::proc::output_with_timeout(&mut command, SYNC_GIT_TIMEOUT)
        .map_err(|error| format!("could not run git: {error}"))?
        .ok_or_else(|| format!("git {} did not finish within {} s", args.join(" "), SYNC_GIT_TIMEOUT.as_secs()))
}

fn git_capture(root: &Path, args: &[&str]) -> Result<String, PrimarySyncError> {
    let output = git_output(root, args).map_err(|message| PrimarySyncError::new("device.sync_failed", message))?;
    if !output.status.success() {
        let detail = String::from_utf8_lossy(&output.stderr).trim().to_string();
        return Err(PrimarySyncError::new(
            "device.sync_failed",
            if detail.is_empty() { format!("git {} failed", args.join(" ")) } else { detail },
        ));
    }
    Ok(String::from_utf8_lossy(&output.stdout).trim().to_string())
}

fn git_optional(root: &Path, args: &[&str]) -> Option<String> {
    git_output(root, args)
        .ok()
        .filter(|output| output.status.success())
        .map(|output| String::from_utf8_lossy(&output.stdout).trim().to_string())
}

fn git_is_ancestor(root: &Path, ancestor: &str, descendant: &str) -> Result<bool, PrimarySyncError> {
    let output = git_output(root, &["merge-base", "--is-ancestor", ancestor, descendant])
        .map_err(|message| PrimarySyncError::new("device.sync_failed", format!("could not compare revisions: {message}")))?;
    match output.status.code() {
        Some(0) => Ok(true),
        Some(1) => Ok(false),
        _ => Err(PrimarySyncError::new(
            "device.sync_failed",
            "git could not compare the primary branch with its upstream",
        )),
    }
}

/// Fetch `remote` without ever waiting on a person (no credential prompt) or a stalled link.
fn git_fetch(root: &Path, remote: &str) -> Result<(), String> {
    let mut command = Command::new("git");
    command.arg("-C").arg(root);
    crate::proc::quiet_network_git(&mut command);
    command.args(["fetch", "--quiet", "--no-tags", remote]);
    let output = crate::proc::output_with_timeout(&mut command, SYNC_FETCH_TIMEOUT)
        .map_err(|error| format!("could not run git: {error}"))?
        .ok_or_else(|| format!("git fetch did not finish within {} s", SYNC_FETCH_TIMEOUT.as_secs()))?;
    if output.status.success() {
        Ok(())
    } else {
        Err(String::from_utf8_lossy(&output.stderr).trim().to_string())
    }
}

fn short_oid(oid: &str) -> &str {
    oid.get(..oid.len().min(8)).unwrap_or(oid)
}

/// `stopped` is asked once more after the fetch, which can take minutes: a run stopped by then
/// leaves the checkout as it was rather than fast-forwarding it for a build that will not run.
fn sync_primary_checkout(root: &Path, stopped: &dyn Fn() -> bool) -> Result<String, PrimarySyncError> {
    let Some(branch) = git_optional(root, &["symbolic-ref", "--quiet", "--short", "HEAD"])
        .filter(|branch| !branch.is_empty())
    else {
        return Ok("primary sync: detached HEAD has no upstream; using local commit".into());
    };
    let Some(upstream) = git_optional(
        root,
        &["rev-parse", "--abbrev-ref", "--symbolic-full-name", "@{upstream}"],
    )
    .filter(|upstream| !upstream.is_empty())
    else {
        return Ok(format!("primary sync: {branch} has no upstream; using local commit"));
    };

    let repo = gix::open(root)
        .map_err(|error| PrimarySyncError::new("device.sync_failed", format!("could not inspect primary checkout: {error}")))?;
    if worktree::is_dirty(&repo) {
        return Err(PrimarySyncError::new(
            "device.sync_dirty",
            "primary checkout has uncommitted changes; Relay will not fetch or overwrite it",
        ));
    }

    let remote = git_capture(root, &["config", "--get", &format!("branch.{branch}.remote")])?;
    if remote != "." {
        git_fetch(root, &remote).map_err(|error| {
            PrimarySyncError::new(
                "device.sync_fetch_failed",
                format!("could not refresh {upstream}: {error}"),
            )
        })?;
    }

    let head = git_capture(root, &["rev-parse", "HEAD"])?;
    let upstream_head = git_capture(root, &["rev-parse", "@{upstream}"])?;
    if head == upstream_head {
        return Ok(format!(
            "primary sync: {branch} is current with {upstream} ({})",
            short_oid(&head)
        ));
    }
    if git_is_ancestor(root, &upstream_head, &head)? {
        return Ok(format!(
            "primary sync: local {branch} is ahead of {upstream}; using {}",
            short_oid(&head)
        ));
    }
    if !git_is_ancestor(root, &head, &upstream_head)? {
        return Err(PrimarySyncError::new(
            "device.sync_diverged",
            format!("{branch} and {upstream} have diverged; Relay will not merge or overwrite either side"),
        ));
    }

    if stopped() {
        return Err(PrimarySyncError::new("device.run_stopped", "the run was stopped before the primary checkout was updated"));
    }
    let removed = git_capture(root, &["diff", "--diff-filter=DR", "--name-only", "HEAD..@{upstream}"])?;
    let safety_ref = if removed.is_empty() {
        None
    } else {
        let reference = format!("refs/relay/snapshots/device-sync-{}", short_oid(&head));
        worktree::git_mutate(root, &["update-ref", &reference, &head]).map_err(|error| {
            PrimarySyncError::new(
                "device.sync_snapshot_failed",
                format!("could not preserve {} before updating: {error}", short_oid(&head)),
            )
        })?;
        Some((reference, removed.lines().count()))
    };

    worktree::git_mutate(root, &["merge", "--ff-only", "--quiet", "@{upstream}"]).map_err(|error| {
        PrimarySyncError::new(
            "device.sync_fast_forward_failed",
            format!("could not safely fast-forward {branch}: {error}"),
        )
    })?;
    let updated = git_capture(root, &["rev-parse", "HEAD"])?;
    Ok(if let Some((reference, count)) = safety_ref {
        format!(
            "primary sync: fast-forwarded {branch} {} -> {}; preserved {count} removed or renamed path(s) at {reference}",
            short_oid(&head),
            short_oid(&updated)
        )
    } else {
        format!(
            "primary sync: fast-forwarded {branch} {} -> {}",
            short_oid(&head),
            short_oid(&updated)
        )
    })
}

struct RunWorkerRequest {
    root: PathBuf,
    command: String,
    device: String,
    adb: String,
    sdk_root: Option<PathBuf>,
    launch_after_build: bool,
    variant: String,
    _gradle_init: Option<tempfile::NamedTempFile>,
    sync_primary: bool,
}

/// `adb shell` probes the worker makes — `pidof`, `resolve-activity`, `logcat -c` / `-d` — and
/// a deadline on each, so a device that stops answering fails the run instead of leaving it
/// in 'building' with its lease held.
const ADB_SHELL_TIMEOUT: Duration = Duration::from_secs(15);
/// `am start -W` waits for the activity to draw its first frame.
const ADB_LAUNCH_TIMEOUT: Duration = Duration::from_secs(60);
/// One `pidof` while waiting for the launched app to appear.
const ADB_PIDOF_TIMEOUT: Duration = Duration::from_secs(5);
/// How often a running app is checked for having exited.
const APP_EXIT_POLL: Duration = Duration::from_secs(2);
/// apksigner and jarsigner are JVMs.
const SIGNING_VERIFY_TIMEOUT: Duration = Duration::from_secs(60);

/// `adb args` under `timeout`; `Err` says what to put in the run log.
fn adb_output(adb: &str, args: &[&str], timeout: Duration) -> Result<std::process::Output, String> {
    let mut command = Command::new(adb);
    command.args(args);
    crate::proc::output_with_timeout(&mut command, timeout)
        .map_err(|error| format!("could not run adb: {error}"))?
        .ok_or_else(|| format!("adb {} did not answer within {} s", args.join(" "), timeout.as_secs()))
}

fn run_worker(engine: Arc<Engine>, runtime: Arc<RunRuntime>, parent: uuid::Uuid, request: RunWorkerRequest) {
    let RunWorkerRequest {
        root,
        command,
        device,
        adb,
        sdk_root,
        launch_after_build,
        variant,
        _gradle_init,
        sync_primary,
    } = request;
    if sync_primary {
        // A stop that landed before the worker started must not fetch or fast-forward anything.
        if runtime.stopped() {
            runtime.set_syncing(None);
            return end_run(&engine, &runtime);
        }
        let synced = sync_primary_checkout(&root, &|| runtime.stopped());
        runtime.set_syncing(None);
        match synced {
            Ok(message) => runtime.push(message),
            Err(_) if runtime.stopped() => return end_run(&engine, &runtime),
            Err(error) => {
                fail_run(&engine, &runtime, parent, error.code, &error.message);
                return;
            }
        }
    }
    let env = [("RELAY_DEVICE", device.as_str()), ("ANDROID_SERIAL", device.as_str())];
    match stream_command(
        &engine,
        &runtime,
        parent,
        &root,
        &command,
        &env,
        sdk_root.as_deref(),
        "build or install command failed",
    ) {
        Streamed::Ok => {}
        Streamed::Stopped | Streamed::Failed => return,
    }
    // From here on every step checks for a stop: `device.run.stop` between the install and the
    // launch must not see the app opened and its row turned back to 'running' afterwards.
    if runtime.stopped() {
        return end_run(&engine, &runtime);
    }
    let _ = adb_output(&adb, &["-s", &device, "logcat", "-c"], ADB_SHELL_TIMEOUT);
    let launched = if launch_after_build {
        match launch_installed_app(&adb, &device, &root, &variant, &|| runtime.stopped()) {
            Ok(app) => {
                runtime.push(format!("app launched: {}", app.component));
                Some(app)
            }
            Err(_) if runtime.stopped() => return end_run(&engine, &runtime),
            Err(message) => {
                runtime.push(message);
                fail_run(
                    &engine,
                    &runtime,
                    parent,
                    "device.launch_failed",
                    "installed app could not be opened",
                );
                return;
            }
        }
    } else {
        variant_application_ids(&root, &variant)
            .into_iter()
            .next()
            .map(|package| LaunchedApp { component: package.clone(), package })
    };
    let Some(launched) = launched else {
        fail_run(
            &engine,
            &runtime,
            parent,
            "device.package_missing",
            "no applicationId was found for the installed variant",
        );
        return;
    };
    let Some(pid) = wait_for_app_pid(&adb, &device, &launched.package, Duration::from_secs(10), &runtime) else {
        if runtime.stopped() {
            return end_run(&engine, &runtime);
        }
        // No process usually means it already died. The crash buffer says why; `logcat -c`
        // emptied it before the launch, so whatever is there for this package is this launch.
        let dump = adb_output(&adb, &["-s", &device, "logcat", "-d", "-v", "brief", "-b", "crash"], ADB_SHELL_TIMEOUT)
            .map(|output| String::from_utf8_lossy(&output.stdout).into_owned())
            .unwrap_or_default();
        if let Some(crash) = startup_crash(&dump, &launched.package) {
            let headline = crash.iter().find(|line| line.contains("FATAL EXCEPTION")).unwrap_or(&crash[0]).clone();
            report_crash(&engine, runtime.id, parent, &headline);
            for line in crash {
                runtime.push(line);
            }
            fail_run(&engine, &runtime, parent, "device.app_crashed", "the app crashed during startup");
        } else {
            fail_run(
                &engine,
                &runtime,
                parent,
                "device.pid_missing",
                "the installed app did not expose a process after launch",
            );
        }
        return;
    };
    // Refused when the row already left 'building' (a stop won the race): nothing to attach.
    if advance_run(&engine, runtime.id, parent, "running", false).is_err() {
        return end_run(&engine, &runtime);
    }
    runtime.push(format!("logcat attached pid={pid}"));
    let mut logcat = Command::new(&adb);
    logcat
        .args([
            "-s",
            &device,
            "logcat",
            "-v",
            "brief",
            "-b",
            "main,system,crash",
            &format!("--pid={pid}"),
        ])
        .stdout(Stdio::piped())
        .stderr(Stdio::null());
    let Ok(mut child) = logcat.spawn() else {
        fail_run(
            &engine,
            &runtime,
            parent,
            "device.logcat_failed",
            "could not start logcat",
        );
        return;
    };
    let Some(stdout) = child.stdout.take() else {
        let _ = child.kill();
        let _ = child.wait();
        fail_run(
            &engine,
            &runtime,
            parent,
            "device.logcat_failed",
            "logcat had no output stream",
        );
        return;
    };
    let logcat_pid = child.id();
    runtime.set_child(child);
    // `logcat --pid` outlives the process it follows, so the run would never end on its own:
    // watch the app and end the stream once it is gone.
    let exited = Arc::new(std::sync::atomic::AtomicBool::new(false));
    let (done, finished) = std::sync::mpsc::channel::<()>();
    let watcher = {
        let (adb, device, package, exited) = (adb.clone(), device.clone(), launched.package.clone(), exited.clone());
        std::thread::spawn(move || loop {
            match finished.recv_timeout(APP_EXIT_POLL) {
                Err(std::sync::mpsc::RecvTimeoutError::Timeout) => {}
                _ => return,
            }
            let output = adb_output(&adb, &["-s", &device, "shell", "pidof", "-s", &package], ADB_PIDOF_TIMEOUT).ok();
            let alive = output.as_ref().and_then(|output| {
                pid_alive(&String::from_utf8_lossy(&output.stdout), &String::from_utf8_lossy(&output.stderr), pid)
            });
            if alive == Some(false) {
                exited.store(true, Ordering::SeqCst);
                // The crash lines a dying app writes are already on their way; let them land.
                if !matches!(finished.recv_timeout(Duration::from_millis(500)), Err(std::sync::mpsc::RecvTimeoutError::Timeout)) { return; }
                // Not reaped yet: the worker joins this thread before it waits on logcat.
                unsafe { libc::kill(logcat_pid as i32, libc::SIGTERM); }
                return;
            }
        })
    };
    let mut crash_reported = false;
    let mut reader = BufReader::new(stdout);
    let mut buffer = Vec::new();
    while !runtime.stopped() {
        // Lossy: one line of binary or mis-encoded log must not end the stream.
        let Ok(Some(line)) = read_lossy_line(&mut reader, &mut buffer) else { break };
        if !crash_reported && crash_line(&line) {
            crash_reported = true;
            report_crash(&engine, runtime.id, parent, &line);
        }
        runtime.push(line);
    }
    drop(done);
    let _ = watcher.join();
    if let Some(mut child) = runtime.take_child() {
        let _ = child.kill();
        let _ = child.wait();
    }
    let stopped = runtime.stopped();
    if !stopped && exited.load(Ordering::SeqCst) {
        runtime.push(format!("app exited (pid {pid})"));
    }
    // Lease first, as in `fail_run`: a client that reads the run as finished can take the device.
    end_run(&engine, &runtime);
    if !stopped {
        let _ = advance_run(&engine, runtime.id, parent, "finished", true);
    }
}

/// A line of the app's own log that says it crashed: Java's `FATAL EXCEPTION`, or libc's
/// `Fatal signal 11 (SIGSEGV)` for a native one (its tombstone comes from another pid).
fn crash_line(line: &str) -> bool {
    line.contains("FATAL EXCEPTION") || line.contains("Fatal signal")
}

/// The most of one log line kept: a run command that prints a huge blob with no newline would
/// otherwise grow the engine without end, and the 512-line ring hold and broadcast it whole.
const MAX_LOG_LINE: usize = 64 * 1024;

/// Read one line, invalid UTF-8 replaced rather than refused, cut at [`MAX_LOG_LINE`] bytes
/// (the rest of it read and dropped). `Ok(None)` at end of stream.
fn read_lossy_line(reader: &mut impl BufRead, buffer: &mut Vec<u8>) -> std::io::Result<Option<String>> {
    buffer.clear();
    let mut read = 0;
    loop {
        let available = match reader.fill_buf() {
            Ok(available) => available,
            Err(error) if error.kind() == std::io::ErrorKind::Interrupted => continue,
            Err(error) => return Err(error),
        };
        if available.is_empty() {
            break;
        }
        let (taken, ended) = match available.iter().position(|b| *b == b'\n') {
            Some(at) => (at + 1, true),
            None => (available.len(), false),
        };
        let room = MAX_LOG_LINE.saturating_sub(buffer.len());
        buffer.extend_from_slice(&available[..taken.min(room)]);
        reader.consume(taken);
        read += taken;
        if ended {
            break;
        }
    }
    if read == 0 {
        return Ok(None);
    }
    while matches!(buffer.last(), Some(b'\n' | b'\r')) {
        buffer.pop();
    }
    Ok(Some(String::from_utf8_lossy(buffer).into_owned()))
}

/// Whether `pidof -s` output says `pid` is still the app's process. `None` when adb itself
/// failed (device gone, server restarting) and the answer is unknown; a different pid means
/// the app died and was started again, which ends this run's stream too.
fn pid_alive(stdout: &str, stderr: &str, pid: u32) -> Option<bool> {
    let pids: Vec<u32> = stdout.split_whitespace().filter_map(|value| value.parse().ok()).collect();
    if pids.contains(&pid) {
        Some(true)
    } else if pids.is_empty() && !stderr.trim().is_empty() {
        None
    } else {
        Some(false)
    }
}

/// The crash in a `logcat -b crash` dump that belongs to `package`: from its `FATAL EXCEPTION`
/// line to the next one, at most [`STARTUP_CRASH_LINES`] lines. `None` when it has none.
fn startup_crash(dump: &str, package: &str) -> Option<Vec<String>> {
    let lines: Vec<&str> = dump.lines().collect();
    let process = lines.iter().position(|line| {
        line.split_once("Process: ").is_some_and(|(_, rest)| rest.split([',', ' ']).next() == Some(package))
    })?;
    let start = lines[..process].iter().rposition(|line| line.contains("FATAL EXCEPTION")).unwrap_or(process);
    let end = lines[process + 1..].iter().position(|line| line.contains("FATAL EXCEPTION")).map_or(lines.len(), |offset| process + 1 + offset);
    Some(lines[start..end].iter().take(STARTUP_CRASH_LINES).map(|line| line.to_string()).collect())
}

const STARTUP_CRASH_LINES: usize = 200;

/// Outcome of one streamed shell command. `Failed` has already failed the run; `Stopped`
/// means `device.run.stop` won the race and the worker owns nothing more.
enum Streamed {
    Ok,
    Stopped,
    Failed,
}

/// Run one shell command inside `root`, streaming its output into the run buffer. Shared by
/// device runs and release builds so both stop, fail and stream identically.
#[allow(clippy::too_many_arguments)]
fn stream_command(
    engine: &Arc<Engine>,
    runtime: &Arc<RunRuntime>,
    parent: uuid::Uuid,
    root: &Path,
    command: &str,
    env: &[(&str, &str)],
    sdk_root: Option<&Path>,
    failure: &str,
) -> Streamed {
    runtime.push(format!("$ {command}"));
    if runtime.stopped() {
        return Streamed::Stopped;
    }
    // stdout and stderr share one pipe, so every command of a chained run_cmd reaches the log
    // with its errors in order; a ` 2>&1` suffix would cover only the last one.
    let Ok((output, stdout, stderr)) = std::io::pipe().and_then(|(output, writer)| Ok((output, writer.try_clone()?, writer))) else {
        fail_run(engine, runtime, parent, "device.run_spawn_failed", "run command had no output stream");
        return Streamed::Failed;
    };
    let shell = shell_args(engine.instance, command, sdk_root);
    let mut spawn = Command::new(&shell[0]);
    // Its own process group, so a stop reaches the Gradle client the shell forked, not
    // only the shell (which would leave the build installing onto a released device).
    std::os::unix::process::CommandExt::process_group(&mut spawn, 0);
    spawn
        .current_dir(root)
        .args(&shell[1..])
        .envs(env.iter().copied())
        .stdout(stdout)
        .stderr(stderr);
    if let Some(sdk_root) = sdk_root {
        spawn.env("ANDROID_HOME", sdk_root).env("ANDROID_SDK_ROOT", sdk_root);
    }
    runtime.mark_started();
    let spawned = spawn.spawn();
    // The parent's copies of the write end go with the Command, or the log never reaches EOF.
    drop(spawn);
    let Ok(child) = spawned else {
        fail_run(engine, runtime, parent, "device.run_spawn_failed", "could not start the run command");
        return Streamed::Failed;
    };
    runtime.set_group_child(child);
    let mut reader = BufReader::new(output);
    let mut buffer = Vec::new();
    loop {
        if runtime.stopped() {
            if let Some(mut child) = runtime.take_child() {
                let _ = child.wait();
            }
            return Streamed::Stopped;
        }
        // Lossy, so a tool printing Latin-1 or raw bytes cannot cut the build log short.
        match read_lossy_line(&mut reader, &mut buffer) {
            Ok(Some(line)) => runtime.push(line),
            Ok(None) => break,
            Err(error) => {
                runtime.push(format!("output: {error}"));
                break;
            }
        }
    }
    let status = runtime.take_child().and_then(|mut child| child.wait().ok());
    if runtime.stopped() {
        return Streamed::Stopped;
    }
    if !status.is_some_and(|status| status.success()) {
        fail_run(engine, runtime, parent, "device.build_failed", failure);
        return Streamed::Failed;
    }
    Streamed::Ok
}

/// The shell and arguments a run command executes under. Production runs use a Fish login
/// shell, so the build sees the user's PATH and JDK; its start-up files run *after* the
/// environment is set, though, and a `set -gx ANDROID_HOME` in config.fish would beat the
/// configured SDK for Gradle while adb and apksigner kept the configured one. So the SDK is
/// exported again inside the command, after start-up. The test instance runs plain `sh -c`,
/// with no profile at all: bus tests must not depend on /etc/profile or ~/.profile.
fn shell_args(instance: Instance, command: &str, sdk_root: Option<&Path>) -> Vec<String> {
    if instance == Instance::Test {
        return vec!["sh".into(), "-c".into(), command.into()];
    }
    let exports = sdk_root.map(|root| {
        let root = fish_quote(&root.display().to_string());
        format!("set -gx ANDROID_HOME {root}; set -gx ANDROID_SDK_ROOT {root}; ")
    });
    vec!["fish".into(), "-lc".into(), format!("{}{command}", exports.unwrap_or_default())]
}

/// One Fish word: single quotes, inside which only `\\` and `\'` are escapes.
fn fish_quote(value: &str) -> String {
    format!("'{}'", value.replace('\\', "\\\\").replace('\'', "\\'"))
}

struct BuildWorkerRequest {
    root: PathBuf,
    command: String,
    sdk_root: PathBuf,
    variant: String,
    format: String,
    signing_profile: Option<SigningRuntimeProfile>,
    _signing_init: Option<tempfile::NamedTempFile>,
}

/// A release build is one Gradle task and the artifact it left behind. No device is
/// involved, so the run finishes where a device run would start logcat (D152).
fn build_worker(engine: Arc<Engine>, runtime: Arc<RunRuntime>, parent: uuid::Uuid, request: BuildWorkerRequest) {
    let BuildWorkerRequest { root, command, sdk_root, variant, format, signing_profile, _signing_init } = request;
    let signing_password = match signing_profile.as_ref() {
        Some(profile) => match load_signing_secret(engine.instance, profile) {
            Ok(password) => Some(password),
            Err(error) => {
                fail_run(&engine, &runtime, parent, &error.code, &error.message);
                return;
            }
        },
        None => None,
    };
    let keystore = signing_profile.as_ref().map(|profile| profile.keystore.display().to_string());
    let env = match (&signing_profile, &signing_password, &keystore) {
        (Some(profile), Some(password), Some(keystore)) => vec![
            ("RELAY_SIGNING_STORE_FILE", keystore.as_str()),
            ("RELAY_SIGNING_STORE_PASSWORD", password.as_str()),
            ("RELAY_SIGNING_KEY_ALIAS", profile.key_alias.as_str()),
            ("RELAY_SIGNING_KEY_PASSWORD", password.as_str()),
        ],
        _ => Vec::new(),
    };
    if let Some(profile) = &signing_profile {
        runtime.push(format!("Relay signing: {} ({})", profile.key_alias, profile.keystore.display()));
    }
    match stream_command(
        &engine,
        &runtime,
        parent,
        &root,
        &command,
        &env,
        Some(&sdk_root),
        "the Gradle build failed",
    ) {
        Streamed::Ok => {}
        Streamed::Stopped | Streamed::Failed => return,
    }
    let Some(artifact) = variant_artifact(&root, &variant, &format) else {
        fail_run(
            &engine,
            &runtime,
            parent,
            "device.artifact_missing",
            &format!("the build wrote no {format} under build/outputs for variant {variant}"),
        );
        return;
    };
    let signing = artifact_signing(&sdk_root, &artifact, &format);
    runtime.push(format!("signing: {signing}"));
    runtime.push(format!("artifact: {}", artifact.display()));
    let _ = finish_build(&engine, runtime.id, parent, &artifact, signing);
    end_run(&engine, &runtime);
}

/// Verify the artifact after Gradle writes it. Relay never asks for a keystore or password:
/// the target project's release signing config owns those, and this only checks the result.
fn artifact_signing(sdk_root: &Path, artifact: &Path, format: &str) -> &'static str {
    let output = if format == "apk" {
        let Some(apksigner) = latest_build_tool(sdk_root, "apksigner") else { return "unverified" };
        crate::proc::output_with_timeout(Command::new(apksigner).args(["verify", artifact.to_string_lossy().as_ref()]).env("LC_ALL", "C"), SIGNING_VERIFY_TIMEOUT)
    } else {
        let jarsigner = std::env::var_os("JAVA_HOME")
            .map(PathBuf::from)
            .map(|home| home.join("bin/jarsigner"))
            .filter(|path| path.is_file())
            .unwrap_or_else(|| PathBuf::from("jarsigner"));
        crate::proc::output_with_timeout(Command::new(jarsigner).args(["-verify", artifact.to_string_lossy().as_ref()]).env("LC_ALL", "C"), SIGNING_VERIFY_TIMEOUT)
    };
    let Ok(Some(output)) = output else { return "unverified" };
    let detail = format!("{}\n{}", String::from_utf8_lossy(&output.stdout), String::from_utf8_lossy(&output.stderr)).to_ascii_lowercase();
    if (format == "apk" && output.status.success()) || (format == "bundle" && detail.contains("jar verified")) {
        "signed"
    } else if detail.contains("no jar signatures") || detail.contains("not signed") || detail.contains("jar is unsigned") {
        "unsigned"
    } else {
        "unverified"
    }
}

fn latest_build_tool(sdk_root: &Path, name: &str) -> Option<PathBuf> {
    std::fs::read_dir(sdk_root.join("build-tools"))
        .ok()?
        .flatten()
        .map(|entry| entry.path().join(name))
        .filter(|path| path.is_file())
        .max()
}

/// The artifact a finished build left behind: the newest `.apk` / `.aab` under any module's
/// `build/outputs/{apk,bundle}/<variant>`. A flavoured project has no such directory — its APKs
/// sit in `apk/free/release`, its bundles in `bundle/freeRelease` — so the outputs tree is
/// searched instead, but only the directories of this variant: an up-to-date release task
/// rewrites nothing, and the debug APK a run installed since is newer than it.
fn variant_artifact(root: &Path, variant: &str, format: &str) -> Option<PathBuf> {
    let (outputs, extension) = if format == "bundle" {
        ("outputs/bundle", "aab")
    } else {
        ("outputs/apk", "apk")
    };
    let mut found = Vec::new();
    for_each_build_dir(root, 0, &mut |build| {
        let outputs = build.join(outputs);
        let scoped = outputs.join(variant);
        let wanted = |path: &Path| path.extension().is_some_and(|found| found == extension);
        if scoped.is_dir() {
            collect_under(&scoped, 0, 3, &wanted, &mut found);
        } else {
            let of_variant = |path: &Path| {
                wanted(path) && path.parent().and_then(|dir| dir.strip_prefix(&outputs).ok()).is_some_and(|dir| variant_directory(dir, variant))
            };
            collect_under(&outputs, 0, 3, &of_variant, &mut found);
        }
    });
    found
        .into_iter()
        .max_by_key(|path| std::fs::metadata(path).and_then(|data| data.modified()).ok())
}

/// Whether `dir`, relative to an outputs directory, holds `variant`'s artifacts: its parts
/// spelled as one variant name (`free/release` → `freeRelease`) are the variant, or end with
/// it as a whole word — `release` is every flavour's release, never `freePrerelease`.
fn variant_directory(dir: &Path, variant: &str) -> bool {
    let mut name = String::new();
    for part in dir.iter().map(|part| part.to_string_lossy()) {
        let mut chars = part.chars();
        if name.is_empty() {
            name.push_str(&part);
        } else if let Some(first) = chars.next() {
            name.push(first.to_ascii_uppercase());
            name.push_str(chars.as_str());
        }
    }
    let (name, variant) = (name.as_bytes(), variant.as_bytes());
    let Some(start) = name.len().checked_sub(variant.len()) else { return false };
    name[start..].eq_ignore_ascii_case(variant) && (start == 0 || name[start].is_ascii_uppercase())
}

/// Every `build` directory under `dir`, at most five levels down, skipping trees that never
/// hold an Android module's outputs. The one walk behind both the artifact and the APK
/// metadata lookups, so the skip list and depth live once.
fn for_each_build_dir(dir: &Path, depth: usize, visit: &mut dyn FnMut(&Path)) {
    if depth > 5 {
        return;
    }
    let Ok(entries) = std::fs::read_dir(dir) else { return };
    for entry in entries.flatten() {
        if !entry.file_type().is_ok_and(|kind| kind.is_dir()) {
            continue;
        }
        let path = entry.path();
        let name = entry.file_name();
        let name = name.to_string_lossy();
        if name == "build" {
            visit(&path);
        } else if !matches!(name.as_ref(), ".git" | ".relay" | "node_modules" | "target") {
            for_each_build_dir(&path, depth + 1, visit);
        }
    }
}

/// Files under `dir`, `max_depth` levels down at most, that `wanted` accepts.
fn collect_under(dir: &Path, depth: usize, max_depth: usize, wanted: &dyn Fn(&Path) -> bool, found: &mut Vec<PathBuf>) {
    if depth > max_depth {
        return;
    }
    let Ok(entries) = std::fs::read_dir(dir) else { return };
    for entry in entries.flatten() {
        let path = entry.path();
        if entry.file_type().is_ok_and(|kind| kind.is_dir()) {
            collect_under(&path, depth + 1, max_depth, wanted, found);
        } else if wanted(&path) {
            found.push(path);
        }
    }
}

struct LaunchedApp {
    component: String,
    package: String,
}

/// `stopped` is asked right before `am start`: resolving the launcher takes up to
/// [`ADB_SHELL_TIMEOUT`], and a run stopped meanwhile must not open the app.
fn launch_installed_app(adb: &str, device: &str, root: &Path, variant: &str, stopped: &dyn Fn() -> bool) -> Result<LaunchedApp, String> {
    let packages = variant_application_ids(root, variant);
    if packages.is_empty() {
        return Err(format!(
            "no applicationId found in Gradle output metadata for variant {variant}"
        ));
    }
    let mut resolution_errors = Vec::new();
    for package in packages {
        let resolved = adb_output(
            adb,
            &[
                "-s",
                device,
                "shell",
                "cmd",
                "package",
                "resolve-activity",
                "--brief",
                "-a",
                "android.intent.action.MAIN",
                "-c",
                "android.intent.category.LAUNCHER",
                &package,
            ],
            ADB_SHELL_TIMEOUT,
        )
        .map_err(|error| format!("could not resolve launcher for {package}: {error}"))?;
        let stdout = String::from_utf8_lossy(&resolved.stdout);
        let component = stdout
            .lines()
            .map(str::trim)
            .find(|line| line.starts_with(&format!("{package}/")))
            .filter(|line| line[package.len() + 1..].chars().all(|c| c.is_ascii_alphanumeric() || matches!(c, '_' | '.' | '$')))
            .map(str::to_string);
        let Some(component) = component.filter(|_| resolved.status.success()) else {
            let detail = String::from_utf8_lossy(&resolved.stderr).trim().to_string();
            resolution_errors.push(if detail.is_empty() { package } else { format!("{package}: {detail}") });
            continue;
        };
        if stopped() {
            return Err("the run was stopped before the app was opened".into());
        }
        let launched = adb_output(adb, &["-s", device, "shell", "am", "start", "-W", "-n", &component], ADB_LAUNCH_TIMEOUT)
            .map_err(|error| format!("could not launch {component}: {error}"))?;
        let stdout = String::from_utf8_lossy(&launched.stdout);
        let stderr = String::from_utf8_lossy(&launched.stderr);
        let refused = !launched.status.success()
            || stdout.contains("Error:")
            || stderr.contains("Error:")
            || stdout.contains("unable to resolve Intent")
            || stderr.contains("unable to resolve Intent");
        if refused {
            let detail = format!("{}\n{}", stdout.trim(), stderr.trim()).trim().to_string();
            return Err(format!("launch refused for {component}: {detail}"));
        }
        return Ok(LaunchedApp { component, package });
    }
    Err(format!(
        "no launcher activity resolved for installed package(s): {}",
        resolution_errors.join(", ")
    ))
}

fn wait_for_app_pid(
    adb: &str,
    device: &str,
    package: &str,
    timeout: Duration,
    runtime: &RunRuntime,
) -> Option<u32> {
    let deadline = Instant::now() + timeout;
    loop {
        if runtime.stopped() {
            return None;
        }
        let output = adb_output(adb, &["-s", device, "shell", "pidof", "-s", package], ADB_PIDOF_TIMEOUT).ok();
        if let Some(pid) = output
            .filter(|output| output.status.success())
            .and_then(|output| String::from_utf8(output.stdout).ok())
            .and_then(|text| text.split_whitespace().find_map(|value| value.parse::<u32>().ok()))
        {
            return Some(pid);
        }
        if Instant::now() >= deadline {
            return None;
        }
        std::thread::sleep(Duration::from_millis(200));
    }
}

fn variant_application_ids(root: &Path, variant: &str) -> Vec<String> {
    let mut metadata = Vec::new();
    let wanted = |path: &Path| path.file_name().is_some_and(|name| name == "output-metadata.json");
    for_each_build_dir(root, 0, &mut |build| collect_under(&build.join("outputs/apk"), 0, 4, &wanted, &mut metadata));
    let mut packages = metadata
        .into_iter()
        .filter_map(|path| std::fs::read_to_string(path).ok())
        .filter_map(|text| serde_json::from_str::<Value>(&text).ok())
        .filter(|value| value.get("variantName").and_then(Value::as_str).is_some_and(|name| name.eq_ignore_ascii_case(variant)))
        .filter_map(|value| value.get("applicationId").and_then(Value::as_str).map(str::to_string))
        // Each goes into an `adb shell` line the device's shell parses: only a real package name.
        .filter(|package| valid_application_id(package))
        .collect::<Vec<_>>();
    packages.sort();
    packages.dedup();
    packages
}

/// A Java package name, as Android requires of an applicationId: two or more dot-separated
/// segments of letters, digits and underscores, each starting with a letter.
fn valid_application_id(value: &str) -> bool {
    value.split('.').count() >= 2
        && value.split('.').all(|segment| {
            segment.chars().next().is_some_and(|c| c.is_ascii_alphabetic()) && segment.chars().all(|c| c.is_ascii_alphanumeric() || c == '_')
        })
}

/// The device is released before the row says `failed`, never after: otherwise a client that
/// already reads the run as failed could still be refused with `device.busy` naming it.
fn fail_run(engine: &Engine, runtime: &RunRuntime, parent: uuid::Uuid, code: &str, message: &str) {
    runtime.push(format!("{code}: {message}"));
    end_run(engine, runtime);
    let _ = advance_run(engine, runtime.id, parent, "failed", true);
}

/// The worker is done with the run: no live stream, no lease. Idempotent, since
/// `device.run.stop` has usually done both already.
fn end_run(engine: &Engine, runtime: &RunRuntime) {
    engine.device_runs.lock().unwrap_or_else(PoisonError::into_inner).remove(&runtime.id);
    super::device_lease::release_run(engine, runtime.id);
}

/// A worker's write to a run that is no longer live — stopped by `device.run.stop`, or already
/// ended — is refused here, inside its transaction, so it can never turn the row back.
fn require_live_run(written: usize, id: Id) -> Result<(), BusError> {
    if written == 0 {
        return Err(BusError::conflict("device.run_ended", format!("run {id} has already ended")));
    }
    Ok(())
}

/// A finished build records its artifact in the same write that finishes the run, so
/// `device.run.list` never shows a finished build without the file it produced.
fn finish_build(engine: &Engine, id: Id, parent: uuid::Uuid, artifact: &Path, signing: &str) -> Result<(), BusError> {
    let project_id = run_project(engine, id).bus()?;
    let artifact = artifact.display().to_string();
    engine.system_write("device.build.finish",Some(parent),Some(project_id),None,json!({"run_id":id,"artifact":artifact,"signing":signing}),|tx,now|{
        require_live_run(tx.execute("UPDATE device_runs SET state='finished',finished_at=?1,artifact=?2,signing=?3 WHERE id=?4 AND state IN ('building','running')",params![now,artifact,signing,id]).bus()?,id)?;
        let run=get_run(tx,id)?;
        Ok(((),vec![("run.changed".into(),serde_json::to_value(run).bus()?)]))
    })
}

fn advance_run(
    engine: &Engine,
    id: Id,
    parent: uuid::Uuid,
    state: &str,
    finished: bool,
) -> Result<(), BusError> {
    let project_id = run_project(engine, id).bus()?;
    engine.system_write("device.run.advance",Some(parent),Some(project_id),None,json!({"run_id":id,"state":state}),|tx,now|{
        require_live_run(tx.execute("UPDATE device_runs SET state=?1,finished_at=CASE WHEN ?2 THEN ?3 ELSE finished_at END WHERE id=?4 AND state IN ('building','running')",params![state,finished as i64,now,id]).bus()?,id)?;
        let run=get_run(tx,id)?;
        Ok(((),vec![("run.changed".into(),serde_json::to_value(run).bus()?)]))
    })
}

fn report_crash(engine: &Engine, id: Id, parent: uuid::Uuid, line: &str) {
    let Ok(project_id) = run_project(engine, id) else { return };
    let _=engine.system_write("device.run.crash",Some(parent),Some(project_id),None,json!({"run_id":id}),|tx,now|{
        tx.execute("INSERT INTO notifications(project_id,category,title,body,link,read,created_at) VALUES (?1,'system','App crashed on device',?2,NULL,0,?3)",params![project_id,line,now]).bus()?;
        Ok(((),vec![("run.crash".into(),json!({"run_id":id,"line":line})),("notify.new".into(),json!({"category":"system","project_id":project_id,"run_id":id}))]))
    });
}

/// The project a run belongs to, for a worker's write (no request open, so its own short read).
fn run_project(engine: &Engine, id: Id) -> rusqlite::Result<Id> {
    engine.store.lock().prepare_cached("SELECT project_id FROM device_runs WHERE id=?1")?.query_row([id], |row| row.get(0))
}

fn get_run(conn: &rusqlite::Connection, id: Id) -> Result<Run, BusError> {
    conn.prepare_cached("SELECT * FROM device_runs WHERE id=?1").bus()?
        .query_row([id], run_row)
        .optional()
        .bus()?
        .ok_or_else(|| BusError::not_found("device.run_not_found", format!("no run {id}")))
}
fn run_row(row: &Row) -> rusqlite::Result<Run> {
    Ok(Run {
        id: row.get("id")?,
        project_id: row.get("project_id")?,
        kind: row.get("kind")?,
        device: row.get("device")?,
        worktree: row.get("worktree")?,
        state: row.get("state")?,
        artifact: row.get("artifact")?,
        variant: row.get("variant")?,
        format: row.get("format")?,
        publish: row.get::<_, i64>("publish")? != 0,
        signing: row.get("signing")?,
        started_at: row.get("started_at")?,
        finished_at: row.get("finished_at")?,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A line past the cap is cut there, the rest of it dropped, and the next line read whole;
    /// invalid UTF-8 is replaced, a last line without a newline still read.
    #[test]
    fn a_log_line_is_cut_at_the_cap() {
        let mut text = vec![b'x'; MAX_LOG_LINE * 3];
        text.extend_from_slice(b"\r\nnext \xff\nlast");
        let (mut reader, mut buffer) = (std::io::BufReader::with_capacity(4096, &text[..]), Vec::new());
        assert_eq!(read_lossy_line(&mut reader, &mut buffer).unwrap().unwrap().len(), MAX_LOG_LINE);
        assert_eq!(read_lossy_line(&mut reader, &mut buffer).unwrap().unwrap(), "next \u{fffd}");
        assert_eq!(read_lossy_line(&mut reader, &mut buffer).unwrap().unwrap(), "last");
        assert_eq!(read_lossy_line(&mut reader, &mut buffer).unwrap(), None);
    }

    fn git(root: &Path, args: &[&str]) -> String {
        let output = Command::new("git")
            .arg("-C")
            .arg(root)
            .args(args)
            .output()
            .unwrap();
        assert!(
            output.status.success(),
            "git {} failed: {}",
            args.join(" "),
            String::from_utf8_lossy(&output.stderr)
        );
        String::from_utf8_lossy(&output.stdout).trim().to_string()
    }

    fn commit(root: &Path, message: &str) {
        git(root, &["add", "-A"]);
        git(
            root,
            &[
                "-c",
                "user.name=Relay Test",
                "-c",
                "user.email=relay@example.invalid",
                "commit",
                "-m",
                message,
            ],
        );
    }

    fn upstream_fixture() -> (tempfile::TempDir, PathBuf, PathBuf) {
        let fixture = tempfile::tempdir().unwrap();
        let remote = fixture.path().join("remote.git");
        let seed = fixture.path().join("seed");
        let checkout = fixture.path().join("checkout");
        let init = Command::new("git")
            .args(["init", "--bare", "--initial-branch=main"])
            .arg(&remote)
            .output()
            .unwrap();
        assert!(init.status.success());
        let init = Command::new("git")
            .args(["init", "--initial-branch=main"])
            .arg(&seed)
            .output()
            .unwrap();
        assert!(init.status.success());
        std::fs::write(seed.join("keep.txt"), "base\n").unwrap();
        commit(&seed, "base");
        git(&seed, &["remote", "add", "origin", remote.to_str().unwrap()]);
        git(&seed, &["push", "-u", "origin", "main"]);
        let cloned = Command::new("git")
            .args(["clone", remote.to_str().unwrap(), checkout.to_str().unwrap()])
            .output()
            .unwrap();
        assert!(cloned.status.success());
        (fixture, seed, checkout)
    }

    fn push_file(seed: &Path, path: &str, contents: &str, message: &str) {
        std::fs::write(seed.join(path), contents).unwrap();
        commit(seed, message);
        git(seed, &["push"]);
    }

    /// A huge `text` would be hundreds of thousands of inject messages under the control lock;
    /// it is refused, pointing at the one-message clipboard paste. Clipboard text over its own
    /// limit points at splitting, not back at `text`.
    #[test]
    fn oversized_text_input_is_refused_with_the_way_that_works() {
        let runtime = MirrorRuntime::new(MirrorRuntimeConfig {
            id: 1, device: "S".into(), width: 1080, height: 2400,
            max_size: 1600, bitrate: 8_000_000, scid: 1, adb: "adb".into(),
        });
        assert!(send_input(&runtime, &json!({"type": "text", "text": "a".repeat(TEXT_INPUT_MAX)})).is_ok());
        let refused = send_input(&runtime, &json!({"type": "text", "text": "a".repeat(TEXT_INPUT_MAX + 1)})).unwrap_err();
        assert_eq!(refused.code, "device.input");
        assert!(refused.hint.unwrap().contains("setclipboard"));
        let refused = send_input(&runtime, &json!({"type": "setclipboard", "text": "a".repeat(mirror::SET_CLIPBOARD_MAX_LENGTH + 1), "paste": true})).unwrap_err();
        assert!(!refused.hint.unwrap().contains("\"text\""));
    }

    #[test]
    fn sizes_preserve_aspect_and_round_like_the_server() {
        // scrcpy rounds the short side to a multiple of 8, not 2 (486 was never what it sent).
        assert_eq!(mirror::fit_size(1080, 2400, 1080), (488, 1080));
        assert_eq!(mirror::fit_size(800, 600, 1080), (800, 600));
    }

    #[test]
    fn primary_sync_fast_forwards_a_clean_checkout() {
        let (_fixture, seed, checkout) = upstream_fixture();
        push_file(&seed, "new.txt", "remote\n", "remote update");

        let message = sync_primary_checkout(&checkout, &|| false).unwrap();

        assert!(message.contains("fast-forwarded main"), "{message}");
        assert_eq!(std::fs::read_to_string(checkout.join("new.txt")).unwrap(), "remote\n");
        assert_eq!(git(&checkout, &["rev-parse", "HEAD"]), git(&seed, &["rev-parse", "HEAD"]));
    }

    #[test]
    fn primary_sync_refuses_dirty_or_diverged_checkouts() {
        let (_fixture, seed, checkout) = upstream_fixture();
        std::fs::write(checkout.join("keep.txt"), "local dirty\n").unwrap();
        let dirty = sync_primary_checkout(&checkout, &|| false).unwrap_err();
        assert_eq!(dirty.code, "device.sync_dirty");
        assert_eq!(std::fs::read_to_string(checkout.join("keep.txt")).unwrap(), "local dirty\n");

        git(&checkout, &["checkout", "--", "keep.txt"]);
        std::fs::write(checkout.join("local.txt"), "local\n").unwrap();
        commit(&checkout, "local update");
        push_file(&seed, "remote.txt", "remote\n", "remote update");
        let diverged = sync_primary_checkout(&checkout, &|| false).unwrap_err();
        assert_eq!(diverged.code, "device.sync_diverged");
        assert!(checkout.join("local.txt").is_file());
        assert!(!checkout.join("remote.txt").exists());
    }

    #[test]
    fn primary_sync_snapshots_before_remote_deletions() {
        let (_fixture, seed, checkout) = upstream_fixture();
        let old_head = git(&checkout, &["rev-parse", "HEAD"]);
        git(&seed, &["rm", "keep.txt"]);
        commit(&seed, "remove tracked file");
        git(&seed, &["push"]);

        let message = sync_primary_checkout(&checkout, &|| false).unwrap();

        assert!(message.contains("preserved 1 removed or renamed path"), "{message}");
        assert!(!checkout.join("keep.txt").exists());
        let reference = format!("refs/relay/snapshots/device-sync-{}", short_oid(&old_head));
        assert_eq!(git(&checkout, &["rev-parse", &reference]), old_head);
        assert_eq!(git(&checkout, &["show", &format!("{reference}:keep.txt")]), "base");
    }

    #[test]
    fn primary_sync_never_fast_forwards_for_a_stopped_run() {
        let (_fixture, seed, checkout) = upstream_fixture();
        let head = git(&checkout, &["rev-parse", "HEAD"]);
        push_file(&seed, "new.txt", "remote\n", "remote update");

        let error = sync_primary_checkout(&checkout, &|| true).unwrap_err();

        assert_eq!(error.code, "device.run_stopped");
        assert_eq!(git(&checkout, &["rev-parse", "HEAD"]), head);
        assert!(!checkout.join("new.txt").exists());
    }

    #[test]
    fn primary_sync_reports_fetch_failures_without_changing_head() {
        let (_fixture, _seed, checkout) = upstream_fixture();
        let head = git(&checkout, &["rev-parse", "HEAD"]);
        git(&checkout, &["remote", "set-url", "origin", "/definitely/missing/relay.git"]);

        let error = sync_primary_checkout(&checkout, &|| false).unwrap_err();

        assert_eq!(error.code, "device.sync_fetch_failed");
        assert_eq!(git(&checkout, &["rev-parse", "HEAD"]), head);
        assert!(checkout.join("keep.txt").is_file());
    }

    #[test]
    fn track_devices_frames_parse_including_the_empty_list() {
        // Two devices, then the last one unplugged: an empty frame with no newline anywhere.
        let list = "emulator-5554\tdevice\nR58M\tdevice\n";
        let stream = format!("{:04x}{list}0000{:04x}R58M\toffline\n", list.len(), "R58M\toffline\n".len());
        let mut reader = std::io::Cursor::new(stream.into_bytes());
        assert_eq!(read_track_frame(&mut reader).unwrap().unwrap(), list.as_bytes());
        assert_eq!(read_track_frame(&mut reader).unwrap().unwrap(), b"");
        assert_eq!(read_track_frame(&mut reader).unwrap().unwrap(), b"R58M\toffline\n");
        assert!(read_track_frame(&mut reader).unwrap().is_none(), "a clean end between frames");
    }

    #[test]
    fn track_devices_rejects_a_stream_that_is_not_framed() {
        let mut unframed = std::io::Cursor::new(b"relay-phone device\n".to_vec());
        assert!(read_track_frame(&mut unframed).is_err());
        let mut signed = std::io::Cursor::new(b"+00a".to_vec());
        assert!(read_track_frame(&mut signed).is_err());
        let mut truncated = std::io::Cursor::new(b"0010short".to_vec());
        assert!(read_track_frame(&mut truncated).is_err());
    }

    #[test]
    fn lossy_lines_survive_invalid_utf8_and_crlf() {
        let mut reader = std::io::Cursor::new(b"ok\r\nbad \xff\xfe byte\nlast".to_vec());
        let mut buffer = Vec::new();
        assert_eq!(read_lossy_line(&mut reader, &mut buffer).unwrap().as_deref(), Some("ok"));
        assert_eq!(read_lossy_line(&mut reader, &mut buffer).unwrap().as_deref(), Some("bad \u{fffd}\u{fffd} byte"));
        assert_eq!(read_lossy_line(&mut reader, &mut buffer).unwrap().as_deref(), Some("last"));
        assert_eq!(read_lossy_line(&mut reader, &mut buffer).unwrap(), None);
    }

    #[test]
    fn an_app_is_alive_only_under_its_own_pid() {
        assert_eq!(pid_alive("1000\n", "", 1000), Some(true));
        assert_eq!(pid_alive("", "", 1000), Some(false), "pidof found nothing: it exited");
        assert_eq!(pid_alive("2044\n", "", 1000), Some(false), "restarted under another pid");
        assert_eq!(pid_alive("", "error: device 'x' not found\n", 1000), None, "adb failed: unknown");
    }

    #[test]
    fn the_configured_sdk_wins_over_shell_start_up_files() {
        // Tests read no profile at all.
        assert_eq!(shell_args(Instance::Test, "./gradlew", Some(Path::new("/sdk"))), ["sh", "-c", "./gradlew"]);
        // Fish reads config.fish before the command, so the SDK is exported inside it, after.
        let args = shell_args(Instance::Dev, "./gradlew installDebug", Some(Path::new("/opt/it's \\ sdk")));
        assert_eq!(&args[..2], ["fish", "-lc"]);
        assert_eq!(
            args[2],
            r"set -gx ANDROID_HOME '/opt/it\'s \\ sdk'; set -gx ANDROID_SDK_ROOT '/opt/it\'s \\ sdk'; ./gradlew installDebug"
        );
        assert_eq!(shell_args(Instance::Dev, "make", None)[2], "make");
        if let Ok(fish) = which::which("fish") {
            let output = Command::new(fish).args(["--no-config", "-c", &format!("printf %s {}", fish_quote(r"a'b\c d"))]).output().unwrap();
            assert_eq!(String::from_utf8_lossy(&output.stdout), r"a'b\c d");
        }
    }

    #[test]
    fn a_startup_crash_is_cut_out_of_the_crash_buffer() {
        let dump = "\
E/AndroidRuntime( 900): FATAL EXCEPTION: main
E/AndroidRuntime( 900): Process: com.other.app, PID: 900
E/AndroidRuntime( 900): java.lang.Error: not ours
E/AndroidRuntime( 1234): FATAL EXCEPTION: main
E/AndroidRuntime( 1234): Process: com.example.app, PID: 1234
E/AndroidRuntime( 1234): java.lang.IllegalStateException: boom
E/AndroidRuntime( 1234): \tat com.example.app.MainActivity.onCreate(MainActivity.kt:12)
";
        let crash = startup_crash(dump, "com.example.app").unwrap();
        assert_eq!(crash.len(), 4, "{crash:?}");
        assert!(crash[0].contains("FATAL EXCEPTION"));
        assert!(crash[2].contains("IllegalStateException: boom"));
        assert!(crash[3].contains("MainActivity.kt:12"));
        // A prefix of another package's name is not this package.
        assert!(startup_crash(dump, "com.example").is_none());
        assert!(startup_crash("", "com.example.app").is_none());
    }

    #[test]
    fn adb_is_found_in_the_sdk_platform_tools() {
        let sdk = tempfile::tempdir().unwrap();
        assert_eq!(sdk_adb(sdk.path()), None);
        std::fs::create_dir_all(sdk.path().join("platform-tools")).unwrap();
        std::fs::write(sdk.path().join("platform-tools/adb"), "").unwrap();
        assert_eq!(sdk_adb(sdk.path()), Some(sdk.path().join("platform-tools/adb")));
    }

    #[test]
    fn a_broken_signing_profile_is_set_aside_not_deleted() {
        let root = tempfile::tempdir().unwrap();
        let directory = root.path().join("abc123");
        std::fs::create_dir(&directory).unwrap();
        std::fs::write(directory.join(SIGNING_KEYSTORE_FILE), "key").unwrap();
        std::fs::write(directory.join(SIGNING_PROFILE_FILE), r#"{"enabled": false, "project": 3}"#).unwrap();
        assert!(signing_profile_switched_off(&directory), "read leniently, even when invalid");
        quarantine_signing_directory(&directory).unwrap();
        assert!(!directory.exists());
        let kept: Vec<_> = std::fs::read_dir(root.path()).unwrap().flatten().map(|entry| entry.path()).collect();
        assert_eq!(kept.len(), 1);
        assert!(kept[0].file_name().unwrap().to_string_lossy().starts_with("abc123.broken-"));
        assert!(kept[0].join(SIGNING_KEYSTORE_FILE).is_file(), "the key survives");
        // Already gone (a racing create moved it first) is not an error.
        quarantine_signing_directory(&directory).unwrap();
        assert!(!signing_profile_switched_off(&directory));
    }

    fn run_engine() -> (Arc<Engine>, tempfile::TempDir, Id) {
        use relay_bus::{Actor, Request};
        let engine = Engine::new(Instance::Test, crate::Store::open_memory().unwrap());
        let workspace = tempfile::tempdir().unwrap();
        let repo = workspace.path().join("repo");
        std::fs::create_dir_all(repo.join(".git")).unwrap();
        std::fs::write(repo.join(".git/HEAD"), "ref: refs/heads/trunk\n").unwrap();
        let dispatch = |op: &str, payload: Value| {
            engine.dispatch(Request::new(Actor::User, op, payload), crate::engine::Door::InProcess).into_result().unwrap()
        };
        let path = std::fs::canonicalize(workspace.path()).unwrap();
        let workspace_id = dispatch("workspace.create", json!({"path": path}))["id"].as_i64().unwrap();
        let project = dispatch("project.add", json!({"workspace_id": workspace_id, "path": path.join("repo")}));
        let project_id = project["id"].as_i64().unwrap();
        (engine, workspace, project_id)
    }

    #[test]
    fn a_lease_pruned_by_a_refused_run_is_still_announced() {
        use relay_bus::{Actor, Request};
        let (engine, _workspace, project_id) = run_engine();
        let mut events = engine.subscribe();
        // Held by a session that does not exist: the next request that prunes drops it.
        let gone = crate::device_lease::Holder::Session { id: 9_999, name: "gone".into() };
        let lease = crate::device_lease::Lease::new("phone-b", gone, crate::device_lease::Kind::Claim, "testing", None);
        engine.device_leases.acquire(lease, &mut Vec::new()).unwrap();
        let refused = engine.dispatch(Request::new(Actor::User, "device.run", json!({"project_id": project_id, "device": "no-such-phone"})), crate::engine::Door::InProcess);
        assert!(refused.error.is_some(), "no such device: the run is refused");
        assert!(engine.device_leases.is_empty());
        let mut announced = Vec::new();
        while let Ok(event) = events.try_recv() { if event.ev == super::super::device_lease::RELEASED { announced.push(event.payload); } }
        assert_eq!(announced.len(), 1, "{announced:?}");
        assert_eq!(announced[0]["device"], "phone-b");
    }

    /// A run that will fast-forward a checkout holds builds in it off until it has, and a run
    /// or build live in a checkout is what a new run's sync looks for before moving it.
    #[test]
    fn a_checkout_being_synced_is_not_built_from_and_a_busy_one_is_not_synced() {
        let (engine, workspace, project_id) = run_engine();
        let root = std::fs::canonicalize(workspace.path()).unwrap().join("repo");
        let run = RunRuntime::new(41);
        run.set_syncing(Some(root.clone()));
        engine.device_runs.lock().unwrap().insert(41, run.clone());
        let refused = refuse_while_syncing(&engine, &root).unwrap_err();
        assert_eq!(refused.code, "device.checkout_syncing");
        assert!(refused.message.contains("run 41"), "{}", refused.message);
        assert!(refuse_while_syncing(&engine, &root.join("elsewhere")).is_ok());
        run.set_syncing(None);
        assert!(refuse_while_syncing(&engine, &root).is_ok(), "a finished sync holds nothing");

        let conn = engine.store.lock();
        assert!(live_runs_in(&conn, &root).unwrap().is_empty());
        conn.execute(
            "INSERT INTO device_runs(project_id,kind,device,worktree,state,variant,format,publish,started_at) VALUES (?1,'build','',?2,'building','release','apk',0,'now')",
            params![project_id, root.display().to_string()],
        ).unwrap();
        let build = conn.last_insert_rowid();
        conn.execute(
            "INSERT INTO device_runs(project_id,device,worktree,state,started_at) VALUES (?1,'x',?2,'finished','now')",
            params![project_id, root.display().to_string()],
        ).unwrap();
        assert_eq!(live_runs_in(&conn, &root).unwrap(), vec![build], "only a live row counts");
    }

    #[test]
    fn adb_device_lines_keep_a_multi_word_state() {
        let device = parse_device_line("R5CT1234ABC            no permissions (user in plugdev group; are your udev rules wrong?); see [http://developer.android.com/tools/device.html] usb:1-1 transport_id:2").unwrap();
        assert_eq!(device.serial, "R5CT1234ABC");
        assert_eq!(device.state, "no permissions (user in plugdev group; are your udev rules wrong?); see [http://developer.android.com/tools/device.html]");
        let device = parse_device_line("emulator-5554          device product:sdk_gphone64 model:sdk_gphone64_x86_64 device:emu64x transport_id:1").unwrap();
        assert_eq!((device.state.as_str(), device.model.as_str(), device.kind), ("device", "sdk gphone64 x86 64", DeviceKind::Avd));
        assert_eq!(parse_device_line("0123456789ABCDEF\tunauthorized usb:2-1 transport_id:3").map(|device| device.state), Some("unauthorized".into()));
        assert!(parse_device_line("").is_none());
        assert!(parse_device_line("* daemon started successfully").is_none());
    }

    #[test]
    fn the_newest_cmdline_tools_wins_by_version_not_by_spelling() {
        let sdk = tempfile::tempdir().unwrap();
        for version in ["9.0", "19.0", "custom"] {
            let bin = sdk.path().join("cmdline-tools").join(version).join("bin");
            std::fs::create_dir_all(&bin).unwrap();
            std::fs::write(bin.join("avdmanager"), "").unwrap();
        }
        assert_eq!(find_avdmanager(sdk.path()), Some(sdk.path().join("cmdline-tools/19.0/bin/avdmanager")));
        assert!(numeric_version("10.0") > numeric_version("9.0"));
        assert_eq!(numeric_version("latest-2"), None);
    }

    #[test]
    fn a_nested_gradle_wrapper_is_pointed_at_its_own_project() {
        let root = tempfile::tempdir().unwrap();
        assert_eq!(gradle_wrapper(root.path()), None);
        std::fs::create_dir_all(root.path().join("android")).unwrap();
        std::fs::write(root.path().join("android/gradlew"), "").unwrap();
        assert_eq!(gradle_wrapper(root.path()).as_deref(), Some("./android/gradlew -p android"));
        std::fs::create_dir_all(root.path().join("other")).unwrap();
        std::fs::write(root.path().join("other/gradlew"), "").unwrap();
        assert_eq!(gradle_wrapper(root.path()), None, "two nested wrappers: neither is guessed");
        std::fs::write(root.path().join("gradlew"), "").unwrap();
        assert_eq!(gradle_wrapper(root.path()).as_deref(), Some("./gradlew"));
    }

    #[test]
    fn a_flavoured_release_never_records_another_variants_artifact() {
        let root = tempfile::tempdir().unwrap();
        let outputs = root.path().join("app/build/outputs");
        let artifact = |path: &str, age: u64| {
            let path = outputs.join(path);
            std::fs::create_dir_all(path.parent().unwrap()).unwrap();
            std::fs::write(&path, "").unwrap();
            let modified = std::time::SystemTime::now() - Duration::from_secs(age);
            std::fs::File::options().write(true).open(&path).unwrap().set_modified(modified).unwrap();
            path
        };
        // An up-to-date release from an hour ago; the debug APKs a run installed just now.
        let release = artifact("apk/free/release/app-free-release.apk", 3600);
        artifact("apk/free/debug/app-free-debug.apk", 10);
        artifact("apk/paid/prerelease/app-paid-prerelease.apk", 5);
        let bundle = artifact("bundle/freeRelease/app-free-release.aab", 3600);
        artifact("bundle/freeDebug/app-free-debug.aab", 10);
        assert_eq!(variant_artifact(root.path(), "release", "apk"), Some(release.clone()));
        assert_eq!(variant_artifact(root.path(), "freeRelease", "apk"), Some(release));
        assert_eq!(variant_artifact(root.path(), "release", "bundle"), Some(bundle));
        assert_eq!(variant_artifact(root.path(), "staging", "apk"), None);
        // Unflavoured: the variant's own directory, as before.
        let plain = artifact("apk/release/app-release.apk", 3600);
        assert_eq!(variant_artifact(root.path(), "release", "apk"), Some(plain));
    }

    #[test]
    fn an_aab_is_built_as_a_bundle() {
        assert_eq!(artifact_format(None), "apk");
        assert_eq!(artifact_format(Some("aab")), "bundle");
        assert_eq!(artifact_format(Some("bundle")), "bundle");
        assert_eq!(artifact_format(Some("apk")), "apk");
        let root = tempfile::tempdir().unwrap();
        std::fs::write(root.path().join("gradlew"), "").unwrap();
        assert_eq!(build_command(root.path(), "release", &artifact_format(Some("aab")), false, None).unwrap(), "./gradlew bundleRelease");
        assert_eq!(build_command(root.path(), "release", &artifact_format(Some("ipa")), false, None).unwrap_err().code, "device.format");
    }

    #[test]
    fn keytool_failures_report_keytools_own_error() {
        let stderr = b"Generating 4096-bit RSA key pair and self-signed certificate (SHA384withRSA) with a validity of 10000 days\n";
        assert_eq!(keytool_error(b"keytool error: java.io.FileNotFoundException: /x (No such file or directory)\n", stderr),
            "keytool error: java.io.FileNotFoundException: /x (No such file or directory)");
        assert_eq!(keytool_error(b"", b"something broke\n"), "something broke");
    }

    #[test]
    fn native_crashes_count_as_crashes() {
        assert!(crash_line("F libc    : Fatal signal 11 (SIGSEGV), code 1 (SEGV_MAPERR), fault addr 0x0 in tid 4242 (main), pid 4242 (com.example)"));
        assert!(crash_line("E AndroidRuntime: FATAL EXCEPTION: main"));
        assert!(!crash_line("I ActivityManager: Displayed com.example/.Main"));
    }

    #[test]
    fn only_a_package_name_is_taken_as_an_application_id() {
        assert!(valid_application_id("com.example.app"));
        assert!(valid_application_id("com.example_2.App"));
        for bad in ["x; pm uninstall com.other", "com.example$(id)", "single", "com..example", "com.1example", ""] {
            assert!(!valid_application_id(bad), "{bad}");
        }
    }

    #[test]
    fn an_emulator_exit_is_explained_by_its_last_error() {
        let lines: Vec<String> = ["INFO    | Android emulator version 35.1", "ERROR   | x86_64 emulation currently requires hardware acceleration!", "INFO    | exiting"]
            .into_iter().map(str::to_string).collect();
        assert_eq!(emulator_exit_message(&lines).as_deref(), Some("ERROR   | x86_64 emulation currently requires hardware acceleration!"));
        assert_eq!(emulator_exit_message(&["only line".to_string()]).as_deref(), Some("only line"));
        assert_eq!(emulator_exit_message(&[]), None);
    }

    #[test]
    fn a_run_registered_by_a_transaction_that_never_commits_is_taken_back() {
        let (engine, _workspace, _) = run_engine();
        let lease = |id| crate::device_lease::Lease::new("relay-phone", crate::device_lease::Holder::User, crate::device_lease::Kind::Run(id), "device.run", None);
        // Rolled back: the after-commit closure holding the guard is dropped unrun.
        let pending = PendingRun::register(&engine, RunRuntime::new(7)).unwrap();
        assert!(engine.device_leases.acquire(lease(7), &mut Vec::new()).unwrap());
        assert!(run_by_id(&engine, 7).is_ok());
        drop(pending);
        assert!(run_by_id(&engine, 7).is_err(), "no phantom live run");
        assert!(engine.device_leases.holder_of("relay-phone").is_none(), "the device is free again");
        // Committed: the closure ran and kept it.
        PendingRun::register(&engine, RunRuntime::new(8)).unwrap().keep();
        assert!(engine.device_leases.acquire(lease(8), &mut Vec::new()).unwrap());
        assert!(run_by_id(&engine, 8).is_ok());
        assert!(engine.device_leases.holder_of("relay-phone").is_some());
    }

    #[test]
    fn a_worker_never_rewrites_a_stopped_run() {
        let (engine, _workspace, project_id) = run_engine();
        let id = {
            let conn = engine.store.lock();
            conn.execute(
                "INSERT INTO device_runs(project_id,device,worktree,state,started_at) VALUES (?1,'relay-phone','/tmp','building','t')",
                [project_id],
            ).unwrap();
            conn.last_insert_rowid()
        };
        let parent = uuid::Uuid::new_v4();
        advance_run(&engine, id, parent, "running", false).unwrap();
        engine.store.lock().execute("UPDATE device_runs SET state='stopped' WHERE id=?1", [id]).unwrap();
        // The install finished after the stop; the worker's late writes are all refused.
        assert_eq!(advance_run(&engine, id, parent, "running", false).unwrap_err().code, "device.run_ended");
        assert!(advance_run(&engine, id, parent, "failed", true).is_err());
        assert!(finish_build(&engine, id, parent, Path::new("/tmp/app.apk"), "signed").is_err());
        let state: String = engine.store.lock().query_row("SELECT state FROM device_runs WHERE id=?1", [id], |row| row.get(0)).unwrap();
        assert_eq!(state, "stopped");
    }
}
