//! `relay blender-mcp` — the Blender plugin's MCP server (D161).
//!
//! Every tool runs Blender in background mode (`blender -b`) on a `.blend` in the agent's
//! checkout: no window, no running Blender, nothing to connect. Each call is one Blender process
//! with a Python script from `blender_py/`, arguments in a JSON file, and one `RELAY_JSON:` line
//! back. Renders come back as images the agent can see. `blender_to_unreal` hands the export to
//! the Unreal plugin's editor bridge and measures what arrived.
//!
//! Environment: `BLENDER_BIN` names the Blender executable when it is not on `PATH`.

use crate::mcp::tool;
use anyhow::{anyhow, bail, Context, Result};
use serde_json::{json, Value};
use std::io::Write;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::Duration;

/// Folders a file listing never enters.
const SKIP_DIRS: [&str; 9] = [".git", "node_modules", ".relay", "Binaries", "Intermediate", "Saved", "DerivedDataCache", "target", ".venv"];
const ASSET_EXTENSIONS: [&str; 7] = ["blend", "fbx", "obj", "glb", "gltf", "abc", "usd"];
/// The most art files one listing returns, and how many folders deep it looks.
const MAX_LISTED: usize = 500;
const MAX_DEPTH: usize = 8;
/// The most of a run's printed output (or failure text) a reply carries, like the Unreal server.
/// run.py keeps no more than this of what the agent's script prints, so the pipe stays small too.
const MAX_OUTPUT: usize = 60_000;

// Every script gets the character frame and bone naming shared with the Unreal tools.
const PY_COMMON: &str = concat!(include_str!("blender_py/common.py"), "\n", include_str!("rig_frame.py"));
const PY_INFO: &str = include_str!("blender_py/info.py");
const PY_RUN: &str = include_str!("blender_py/run.py");
const PY_RENDER: &str = include_str!("blender_py/render.py");
const PY_RIG_CHECK: &str = include_str!("blender_py/rig_check.py");
// The checks are shared with ue_anim_inspect (`anim_rules.py`); the script only poses the rig.
const PY_ANIM_INSPECT: &str = concat!(include_str!("anim_rules.py"), "\n", include_str!("blender_py/anim_inspect.py"));
const PY_EXPORT: &str = include_str!("blender_py/export.py");
const PY_MESH_CHECK: &str = include_str!("blender_py/mesh_check.py");

pub fn serve() -> Result<u8> {
    crate::mcp::serve_sync(handle, lane)
}

/// Each call is its own Blender process, so calls run side by side (RA-061); saves to one file
/// are kept apart by the save lock. Sending to Unreal drives the one editor, so that waits its turn.
fn lane(name: &str, _args: &Value) -> crate::mcp::Lane {
    if name == "blender_to_unreal" { crate::mcp::Lane::Serial } else { crate::mcp::Lane::Parallel }
}

const SERVER: crate::mcp::PluginServer = crate::mcp::PluginServer {
    name: "blender",
    title: "Blender (Relay plugin)",
    instructions: "Blender in background mode on .blend files in this checkout; no Blender window is needed. Start with blender_info (no file lists the art files). Look with blender_render, check rigs with blender_rig_check, measure animations with blender_anim_inspect, export with blender_export, and send to a running Unreal editor with blender_to_unreal. Load the blender-fundamentals skill first.",
    tools,
    call,
};

fn handle(message: &Value) -> Option<Value> {
    crate::mcp::plugin_reply(&SERVER, message)
}

fn file_arg() -> Value {
    json!({"type":"string","description":".blend file, relative to the checkout root"})
}

fn attachments_schema() -> Value {
    json!({"type":"array","items":{"type":"object","properties":{
        "object":{"type":"string","description":"An object already attached to the rig (bone parent or Child Of constraint)"},
        "name":{"type":"string"},
        "grips":{"type":"array","items":{"type":"object","properties":{
            "point":{"type":"string","description":"obj:<Empty> (a socket), item:<name>:<end_a|end_b|center|origin>, or a bone"},
            "bone":{"type":"string","description":"Bone, or bone:tail for the bone's tail (a hand bone's tail is usually the palm end)"},
            "tolerance":{"type":"number"}}}}
    },"required":["object"]}})
}

fn tools() -> Vec<Value> {
    vec![
        tool("blender_info",
            "What a .blend holds: units and frame rate, every object with type, parent and parent bone, world location and dimensions in cm, unapplied scale/rotation, modifiers; meshes (vertices, triangles, UV maps, vertex groups, materials, shape keys, Unreal collision/socket naming); armatures (bones, deform bones, roots, left/right pairs, current action); actions with frame ranges; images. Without file, lists the art files (.blend, .fbx, .obj, .glb, .gltf, .abc, .usd) in the checkout.",
            json!({"file": file_arg()}), &[], true),
        tool("blender_python",
            "Run Python (bpy) in Blender on a file (or an empty scene without file). save=true saves the file, save_as writes a new one (relative path). Print what you need to see. addons enables bundled add-ons first (e.g. rigify).",
            json!({
                "file": file_arg(),
                "code":{"type":"string"},
                "save":{"type":"boolean"},
                "save_as":{"type":"string"},
                "addons":{"type":"array","items":{"type":"string"}},
                "timeout_s":{"type":"integer","minimum":10,"maximum":3600,"description":"Default 300"}
            }), &["code"], false),
        tool("blender_render",
            "See a model or a pose: renders objects (an armature brings its skinned meshes) from named views relative to the character's facing - front, back, left, right, top, three_quarter, three_quarter_left - at chosen frames of an action. Returns images. Workbench (default, fast, shows shape and cavities), eevee or cycles (CPU, no denoiser). Other meshes are hidden unless isolate=false. The file is not changed.",
            json!({
                "file": file_arg(),
                "objects":{"type":"array","items":{"type":"string"}},
                "action":{"type":"string"},
                "frames":{"type":"array","items":{"type":"integer"}},
                "views":{"type":"array","items":{"type":"string"}},
                "engine":{"type":"string","enum":["workbench","eevee","cycles"]},
                "color":{"type":"string","enum":["MATERIAL","OBJECT","SINGLE","RANDOM","TEXTURE","VERTEX"],"description":"Workbench colouring, default MATERIAL"},
                "isolate":{"type":"boolean"},
                "width":{"type":"integer","minimum":64,"maximum":1920},"height":{"type":"integer","minimum":64,"maximum":1080},
                "timeout_s":{"type":"integer","minimum":10,"maximum":3600}
            }), &["file"], true),
        tool("blender_rig_check",
            "Check a rig and its skin before export: applied transforms, a single root bone, deform bones, leftover _end bones, left/right symmetry and naming, facing (-Y is Blender's front), skinned meshes with unweighted vertices, too many influences, weights on non-deform bones, stray vertex groups, missing UVs, non-manifold or inside-out geometry, and height in cm. Returns problems (must fix), warnings and passed.",
            json!({"file": file_arg(), "armature":{"type":"string"}, "meshes":{"type":"array","items":{"type":"string"}}}), &["file"], true),
        tool("blender_mesh_check",
            "Check meshes as they will be exported (modifiers applied): zero-area faces (e.g. a bevel wider than a thin part), zero-length edges, inside-out normals are problems; loose vertices, n-gons (the FBX exporter then skips tangents) and missing UVs are warnings. blender_export runs the same check and refuses to write a broken mesh.",
            json!({"file": file_arg(), "objects":{"type":"array","items":{"type":"string"}}}), &["file"], true),
        tool("blender_anim_inspect",
            "Measure an action before export, like ue_anim_inspect: at sampled frames, where tracked bones and objects are in the character's frame ([forward, right, up] cm, sides from the rig's .L/.R pairs); attached items (objects parented to a bone) with grip distances and clearance to the body; a partner armature's clearance; expected contacts (touch or apart, with a frame window); feet below the ground. Returns problems and passed.",
            json!({
                "file": file_arg(),
                "armature":{"type":"string"},
                "action":{"type":"string"},
                "frames":{"type":"array","items":{"type":"integer"}},
                "samples":{"type":"integer","minimum":2,"maximum":60},
                "track":{"type":"array","items":{"type":"string"},"description":"bone, bone:tail, obj:<name>, item:<name>:<end_a|end_b|center|origin>, partner:<bone>"},
                "attachments": attachments_schema(),
                "partner":{"type":"object","properties":{"armature":{"type":"string"},"action":{"type":"string"}},"required":["armature"]},
                "contacts":{"type":"array","items":{"type":"object","properties":{
                    "a":{"type":"string"},"b":{"type":"string"},"expect":{"type":"string","enum":["touch","apart"]},
                    "distance":{"type":"number"},"window":{"type":"array","items":{"type":"integer"}}},"required":["a","b"]}},
                "body_radius":{"type":"number","description":"cm, default 8"},
                "touch_distance":{"type":"number","description":"cm, default 5"}
            }), &["file"], true),
        tool("blender_export",
            "Export FBX for Unreal. kind static (a mesh with its SOCKET_ empties, UCX_/UBX_/USP_/UCP_ collision and _LODn children), skeletal (an armature and the meshes skinned to it; deform bones only, no leaf bones; animations=true bakes the action) or animation (the armature's action only). Settings are Unreal's usual ones; fbx_options overrides any exporter option except filepath, use_selection and check_existing (use path and objects). Reports the size in cm to compare after import.",
            json!({
                "file": file_arg(),
                "objects":{"type":"array","items":{"type":"string"}},
                "path":{"type":"string","description":"Output .fbx, relative to the checkout root"},
                "kind":{"type":"string","enum":["auto","static","skeletal","animation"]},
                "action":{"type":"string"},
                "animations":{"type":"boolean"},
                "all_actions":{"type":"boolean","description":"One take per action (fake-user actions included)"},
                "fbx_options":{"type":"object"},
                "allow_problems":{"type":"boolean","description":"Export even when the mesh check or armature scale finds problems"}
            }), &["file","path"], false),
        tool("blender_to_unreal",
            "Export from Blender and import into the running Unreal editor in one step, then measure the result: imported assets, their size against the Blender size (a 100x difference means a unit problem), a skeletal mesh's root bone scale and which way it faces (a mirrored rig shows there; hand_sides only reflects bone names). Needs the Unreal plugin on and the editor open.",
            json!({
                "file": file_arg(),
                "objects":{"type":"array","items":{"type":"string"}},
                "kind":{"type":"string","enum":["static","skeletal","animation"]},
                "action":{"type":"string"},
                "animations":{"type":"boolean"},
                "destination":{"type":"string","description":"Content folder, e.g. /Game/Characters/Hero"},
                "name":{"type":"string","description":"Asset name in Unreal"},
                "skeleton":{"type":"string","description":"Existing Skeleton asset, for animations and shared rigs"},
                "fbx_path":{"type":"string","description":"Where to keep the FBX, relative to the checkout; default Saved/Relay/Exports/<name>.fbx"},
                "materials":{"type":"boolean"},
                "allow_problems":{"type":"boolean","description":"Export even when the mesh check finds problems"},
                "socket_rotation":{"type":"string","enum":["match","zero","keep"],"description":"Sockets from SOCKET_ empties arrive with a -90 degree roll from the axis conversion, and at 100x scale (divided back unless keep). match (default): a socket whose empty had no rotation of its own gets none; zero: every socket; keep: as imported"},
                "importer":{"type":"string","enum":["legacy","interchange"],"description":"Default legacy: Interchange FBX produced empty meshes and transient materials on UE 5.8. The other is tried if the first fails."},
                "normals":{"type":"string","enum":["FBXNIM_IMPORT_NORMALS","FBXNIM_IMPORT_NORMALS_AND_TANGENTS","FBXNIM_COMPUTE_NORMALS"],"description":"Default FBXNIM_IMPORT_NORMALS (tangents computed): imported tangents from Blender gave a mesh that drew only its shadow"}
            }), &["file","kind","destination"], false),
    ]
}

fn call(name: &str, args: &Value) -> Result<Value> {
    let root = checkout_root()?;
    match name {
        "blender_info" => match args["file"].as_str().filter(|f| !f.is_empty()) {
            None => Ok(list_files(&root)),
            Some(file) => run(PY_INFO, args, Some(&resolve(&root, file)?), secs(args, 120)),
        },
        "blender_python" => {
            let mut a = args.clone();
            let save_as = args["save_as"].as_str().map(|save_as| resolve_new(&root, save_as, "blend")).transpose()?;
            let file = args["file"].as_str().filter(|f| !f.is_empty()).map(|f| resolve(&root, f)).transpose()?;
            // Two calls saving one .blend in a shared checkout: the second is refused up front by
            // the lock, and a save over a file someone changed since it was opened (run.py checks
            // this stamp, taken before Blender reads the file) is refused instead of losing work.
            let target = save_as.clone().or_else(|| file.clone().filter(|_| args["save"] == true));
            // Agent code meets the guardrail before it runs, and before anything is written for it
            // (RA-077; mcp::plugin_gate says what that can and cannot cover).
            crate::mcp::plugin_gate("blender_python", file.as_deref(), &target.as_deref().into_iter().collect::<Vec<_>>())?;
            if let Some(save_as) = &save_as {
                if let Some(parent) = save_as.parent() {
                    std::fs::create_dir_all(parent).with_context(|| format!("creating {}", parent.display()))?;
                }
                a["save_as"] = json!(save_as);
            }
            let _lock = target.as_deref().map(|t| SaveLock::take(t, secs(args, 300))).transpose()?;
            if let Some(file) = &file {
                a["_opened"] = opened_stamp(file)?;
            }
            run(PY_RUN, &a, file.as_deref(), secs(args, 300))
        }
        "blender_render" => {
            let file = resolve(&root, required(args, "file")?)?;
            let dir = scratch_dir(RENDER_PREFIX)?;
            let mut a = args.clone();
            a["out_dir"] = json!(dir);
            let result = run(PY_RENDER, &a, Some(&file), secs(args, 600));
            match result {
                Ok(mut value) => {
                    // Only the renders in this call's own folder are read back into the reply.
                    let files = value["files"].as_array().cloned().unwrap_or_default();
                    let files = files.iter().filter(|f| f["file"].as_str().is_some_and(|p| Path::new(p).starts_with(&dir)));
                    value["_images"] = json!(files.map(|f| json!({"label": f["view"], "path": f["file"]})).collect::<Vec<_>>());
                    value["_cleanup"] = json!(dir);
                    value.as_object_mut().unwrap().remove("files");
                    Ok(value)
                }
                Err(error) => {
                    let _ = std::fs::remove_dir_all(&dir);
                    Err(error)
                }
            }
        }
        "blender_mesh_check" => run(PY_MESH_CHECK, args, Some(&resolve(&root, required(args, "file")?)?), secs(args, 300)),
        "blender_rig_check" => run(PY_RIG_CHECK, args, Some(&resolve(&root, required(args, "file")?)?), secs(args, 300)),
        "blender_anim_inspect" => run(PY_ANIM_INSPECT, args, Some(&resolve(&root, required(args, "file")?)?), secs(args, 300)),
        "blender_export" => {
            let file = resolve(&root, required(args, "file")?)?;
            let mut a = args.clone();
            let path = resolve_new(&root, required(args, "path")?, "fbx")?;
            crate::mcp::plugin_gate("blender_export", Some(&file), &[&path])?;
            a["path"] = json!(path);
            run(PY_EXPORT, &a, Some(&file), secs(args, 600))
        }
        "blender_to_unreal" => to_unreal(&root, args),
        other => bail!("unknown tool {other}"),
    }
}

fn required<'a>(args: &'a Value, key: &str) -> Result<&'a str> {
    args[key].as_str().filter(|v| !v.trim().is_empty()).ok_or_else(|| anyhow!("{key} is required"))
}

fn secs(args: &Value, default: u64) -> Duration {
    Duration::from_secs(args["timeout_s"].as_u64().unwrap_or(default).clamp(10, 3600))
}

fn checkout_root() -> Result<PathBuf> {
    Ok(std::env::var_os("RELAY_WORKTREE").map(PathBuf::from).filter(|p| p.is_dir()).unwrap_or(std::env::current_dir()?))
}

/// An existing file inside the checkout. Absolute paths are accepted when they are inside it.
fn resolve(root: &Path, file: &str) -> Result<PathBuf> {
    let path = if Path::new(file).is_absolute() { PathBuf::from(file) } else { root.join(file) };
    let real = path.canonicalize().with_context(|| format!("{} does not exist; list the art files with blender_info and no file", path.display()))?;
    let root = root.canonicalize()?;
    anyhow::ensure!(real.starts_with(&root), "{} is outside the checkout {}", real.display(), root.display());
    Ok(real)
}

/// A file to be written inside the checkout, with the extension it must have.
fn resolve_new(root: &Path, file: &str, extension: &str) -> Result<PathBuf> {
    anyhow::ensure!(!Path::new(file).is_absolute() && !file.split(['/', '\\']).any(|part| part == ".."), "{file} must be a path inside the checkout, relative to its root");
    anyhow::ensure!(Path::new(file).extension().is_some_and(|e| e.eq_ignore_ascii_case(extension)), "{file} must end in .{extension}");
    Ok(root.join(file))
}

/// The art files in the checkout, at most `MAX_LISTED` of them. A listing that stopped at the
/// cap or did not look below `MAX_DEPTH` says so, rather than passing for complete (RA-281).
fn list_files(root: &Path) -> Value {
    #[derive(Default)]
    struct Listing { files: Vec<Value>, truncated: bool, too_deep: usize }
    fn walk(dir: &Path, root: &Path, depth: usize, out: &mut Listing) {
        let Ok(entries) = std::fs::read_dir(dir) else { return };
        let mut entries: Vec<_> = entries.flatten().collect();
        entries.sort_by_key(|e| e.file_name());
        for entry in entries {
            if out.truncated {
                return;
            }
            let path = entry.path();
            let name = entry.file_name().to_string_lossy().into_owned();
            if path.is_dir() {
                if SKIP_DIRS.contains(&name.as_str()) || name.starts_with('.') {
                    continue;
                }
                if depth >= MAX_DEPTH {
                    out.too_deep += 1;
                } else {
                    walk(&path, root, depth + 1, out);
                }
            } else if path.extension().is_some_and(|e| ASSET_EXTENSIONS.contains(&e.to_string_lossy().to_lowercase().as_str())) {
                if out.files.len() >= MAX_LISTED {
                    out.truncated = true;
                    return;
                }
                let size = entry.metadata().map(|m| m.len()).unwrap_or(0);
                out.files.push(json!({"file": path.strip_prefix(root).unwrap_or(&path), "bytes": size}));
            }
        }
    }
    let mut listing = Listing::default();
    walk(root, root, 0, &mut listing);
    let blender = find_blender().map(|p| p.display().to_string()).map_err(|e| e.to_string());
    let mut value = json!({"root": root, "files": listing.files, "blender": blender.as_ref().ok(), "blender_problem": blender.err()});
    if listing.truncated {
        value["truncated"] = json!(format!("only the first {MAX_LISTED} art files are listed; open a file by name with blender_info file"));
    }
    if listing.too_deep > 0 {
        value["not_searched"] = json!(format!("folders more than {MAX_DEPTH} levels deep were not searched: {}", listing.too_deep));
    }
    value
}

/// The name a render's folder in the temp dir starts with; `mcp::tool_result` deletes a
/// `_cleanup` folder only when it is one of these (or an Unreal capture folder).
pub(crate) const RENDER_PREFIX: &str = "relay-blender-";

/// A new folder in the temp dir for one call, readable only by the user (the script, its
/// arguments and the renders), under a name no one can guess or claim first.
fn scratch_dir(prefix: &str) -> Result<PathBuf> {
    use std::os::unix::fs::DirBuilderExt;
    let dir = std::env::temp_dir().join(format!("{prefix}{}", uuid::Uuid::new_v4()));
    std::fs::DirBuilder::new().mode(0o700).create(&dir).with_context(|| format!("creating {}", dir.display()))?;
    Ok(dir)
}

fn find_blender() -> Result<PathBuf> {
    if let Some(path) = std::env::var_os("BLENDER_BIN").map(PathBuf::from) {
        anyhow::ensure!(path.is_file(), "BLENDER_BIN={} is not a file", path.display());
        return Ok(path);
    }
    let exe = if cfg!(target_os = "windows") { "blender.exe" } else { "blender" };
    if let Some(paths) = std::env::var_os("PATH") {
        for dir in std::env::split_paths(&paths) {
            let candidate = dir.join(exe);
            if candidate.is_file() {
                return Ok(candidate);
            }
        }
    }
    let mut candidates = vec![
        PathBuf::from("/Applications/Blender.app/Contents/MacOS/Blender"),
        PathBuf::from("/snap/bin/blender"),
        PathBuf::from("/usr/local/bin/blender"),
    ];
    // Windows installs one folder per version; take the newest.
    if let Ok(entries) = std::fs::read_dir("C:/Program Files/Blender Foundation") {
        let mut versions: Vec<PathBuf> = entries.flatten().map(|e| e.path().join("blender.exe")).filter(|p| p.is_file()).collect();
        versions.sort();
        candidates.extend(versions.into_iter().rev());
    }
    candidates.into_iter().find(|p| p.is_file()).ok_or_else(|| anyhow!("Blender not found: install it, put `blender` on PATH, or set BLENDER_BIN to the executable"))
}

/// One background Blender run: the shared helpers and the tool's script, the arguments as a
/// JSON file, the last `RELAY_JSON:` line as the result. A Python error comes back with its
/// traceback.
fn run(body: &str, args: &Value, file: Option<&Path>, timeout: Duration) -> Result<Value> {
    let blender = find_blender()?;
    let dir = scratch_dir("relay-blender-job-")?;
    let script = dir.join("job.py");
    let args_file = dir.join("args.json");
    std::fs::write(&script, format!("{PY_COMMON}\n{body}"))?;
    std::fs::write(&args_file, args.to_string())?;
    let mut command = Command::new(&blender);
    command.arg("-b");
    if let Some(file) = file {
        command.arg(file);
    }
    command
        .args(["--factory-startup", "--python-exit-code", "1", "--python"])
        .arg(&script)
        .arg("--")
        .arg(&args_file);
    if let Some(file) = file.and_then(Path::parent) {
        command.current_dir(file);
    }
    // Confined to the session's write roots where bwrap can (RA-077; blender_sandbox).
    let sandbox = crate::blender_sandbox::Sandbox::current(&checkout_root()?);
    let out_dir: Vec<PathBuf> = args["out_dir"].as_str().map(PathBuf::from).into_iter().collect();
    let mut command = sandbox.wrap(command, &dir, &out_dir).inspect_err(|_| { let _ = std::fs::remove_dir_all(&dir); })?;
    let output = relay_core::proc::output_with_timeout(&mut command, timeout);
    let _ = std::fs::remove_dir_all(&dir);
    let output = output.with_context(|| format!("running {}", blender.display()))?
        .ok_or_else(|| anyhow!("Blender did not finish within {} s and was stopped", timeout.as_secs()))?;
    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);
    let printed = printed(&stdout);
    let result = stdout.lines().rev().find_map(|l| l.strip_prefix("RELAY_JSON:"));
    if !output.status.success() || (result.is_none() && body != PY_RUN) {
        bail!("Blender failed:\n{}\n(sandbox {})", failure_text(&stdout, &stderr), sandbox.describe());
    }
    let mut value = match result {
        Some(json_text) => serde_json::from_str(json_text).context("the script's RELAY_JSON line is not JSON")?,
        None => json!({}),
    };
    // Fields are added below; indexing anything but an object would panic (RA-301).
    anyhow::ensure!(value.is_object(), "the script's result is not a JSON object: {}", tail(&value.to_string(), 5));
    strip_reserved(&mut value);
    if body == PY_RUN {
        // The agent's own script: its printed output is the result.
        value["output"] = json!(tail(&printed.join("\n"), 300));
        value["saved"] = stdout.lines().find_map(|l| l.strip_prefix("RELAY_SAVED:")).and_then(|s| serde_json::from_str(s).ok()).unwrap_or(Value::Null);
    }
    value["sandbox"] = json!(sandbox.describe());
    Ok(value)
}

/// Keys starting with `_` are the MCP layer's instructions (`_images` are read into the reply,
/// `_cleanup` is deleted), set only here in Rust. A script's result never carries them: agent
/// code can `emit()` or print any `RELAY_JSON:` line, and `{"_cleanup": "/home/me"}` would
/// otherwise delete a tree outside the sandbox.
fn strip_reserved(value: &mut Value) {
    if let Some(object) = value.as_object_mut() {
        object.retain(|key, _| !key.starts_with('_'));
    }
}

/// What the agent's script printed: run.py puts it between markers, so Blender's own start-up
/// lines stay out.
fn printed(stdout: &str) -> Vec<&str> {
    stdout.lines()
        .skip_while(|l| *l != "RELAY_OUT_BEGIN").skip(1)
        .take_while(|l| *l != "RELAY_OUT_END")
        .collect()
}

/// The traceback and error lines of a failed run, without Blender's start-up noise, after the
/// end of what the script printed. The traceback reaches stderr after everything printed to
/// stdout, so starting at it dropped the output that shows how far the script got (RA-282).
fn failure_text(stdout: &str, stderr: &str) -> String {
    let all: Vec<&str> = stdout.lines().chain(stderr.lines()).collect();
    let start = all.iter().position(|l| l.starts_with("Traceback")).unwrap_or(all.len().saturating_sub(40));
    let lines: Vec<&str> = all[start..].iter().copied().filter(|l| !l.starts_with("EGL Error") && !l.trim().is_empty()).collect();
    let printed = printed(stdout);
    match all.iter().position(|l| *l == "RELAY_OUT_END") {
        Some(end) if end < start && !printed.is_empty() => {
            tail(&format!("printed before it failed:\n{}\n\n{}", tail(&printed.join("\n"), 40), tail(&lines.join("\n"), 60)), 110)
        }
        _ => tail(&lines.join("\n"), 60),
    }
}

/// The last `lines` lines, and at most `MAX_OUTPUT` bytes of them (cut on a char boundary).
fn tail(text: &str, lines: usize) -> String {
    let all: Vec<&str> = text.lines().collect();
    let out = all[all.len().saturating_sub(lines)..].join("\n");
    let mut cut = out.len().saturating_sub(MAX_OUTPUT);
    while !out.is_char_boundary(cut) {
        cut += 1;
    }
    out[cut..].to_string()
}

/// The file as this run found it, before Blender reads it: run.py refuses to save over it once
/// its mtime or size differ.
fn opened_stamp(file: &Path) -> Result<Value> {
    let meta = std::fs::metadata(file).with_context(|| format!("reading {}", file.display()))?;
    let mtime_ns = meta.modified()?.duration_since(std::time::UNIX_EPOCH).map(|d| d.as_nanos() as u64).unwrap_or(0);
    Ok(json!({"path": file, "mtime_ns": mtime_ns, "size": meta.len()}))
}

/// `<file>.relay-lock` beside a .blend while a `blender_python` call that saves it runs; removed
/// on drop. It names the time it expires, so one left by a killed server does not block forever.
struct SaveLock(PathBuf);

impl SaveLock {
    fn take(target: &Path, timeout: Duration) -> Result<SaveLock> {
        let mut path = target.as_os_str().to_owned();
        path.push(".relay-lock");
        let path = PathBuf::from(path);
        let now = || std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_secs()).unwrap_or(0);
        for _ in 0..2 {
            match std::fs::OpenOptions::new().write(true).create_new(true).open(&path) {
                Ok(mut lock) => {
                    let until = now() + timeout.as_secs() + 60;
                    let _ = write!(lock, "{}", json!({"pid": std::process::id(), "until": until}));
                    return Ok(SaveLock(path));
                }
                Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => {
                    // Unreadable (or still being written): held for the longest a call can run.
                    let until = std::fs::read_to_string(&path).ok()
                        .and_then(|t| serde_json::from_str::<Value>(&t).ok()).and_then(|v| v["until"].as_u64())
                        .or_else(|| {
                            let modified = std::fs::metadata(&path).and_then(|m| m.modified()).ok()?;
                            Some(modified.duration_since(std::time::UNIX_EPOCH).ok()?.as_secs() + 3660)
                        });
                    if until.is_some_and(|until| until > now()) {
                        bail!("{} is being changed by another blender_python call that saves it (lock {}); wait for it to finish and run again on the saved file", target.display(), path.display());
                    }
                    let _ = std::fs::remove_file(&path);
                }
                Err(error) => return Err(error).with_context(|| format!("creating {}", path.display())),
            }
        }
        bail!("could not lock {}", target.display())
    }
}

impl Drop for SaveLock {
    fn drop(&mut self) {
        let _ = std::fs::remove_file(&self.0);
    }
}

fn to_unreal(root: &Path, args: &Value) -> Result<Value> {
    let file = resolve(root, required(args, "file")?)?;
    let kind = required(args, "kind")?;
    // Asked before an export that can take minutes, not after it (RA-588).
    let destination = required(args, "destination")?;
    let stem = args["name"].as_str().map(str::to_string)
        .or_else(|| args["objects"][0].as_str().map(str::to_string))
        .unwrap_or_else(|| file.file_stem().map(|s| s.to_string_lossy().into_owned()).unwrap_or_else(|| "Export".into()));
    let fbx_rel = args["fbx_path"].as_str().map(str::to_string).unwrap_or_else(|| format!("Saved/Relay/Exports/{stem}.fbx"));
    let fbx = resolve_new(root, &fbx_rel, "fbx")?;
    crate::mcp::plugin_gate("blender_to_unreal", Some(&file), &[&fbx])?;
    // The project, the editor on it and its lock, before the export rather than after it.
    crate::unreal::import_preflight()?;
    let exported = run(PY_EXPORT, &json!({
        "objects": args["objects"], "path": fbx, "kind": kind, "action": args["action"],
        "animations": args["animations"].as_bool().unwrap_or(kind == "animation"),
        "allow_problems": args["allow_problems"],
    }), Some(&file), Duration::from_secs(600))?;
    let mut import_args = json!({
        "kind": kind, "destination": destination, "name": args["name"],
        "skeleton": args["skeleton"], "animations": args["animations"], "materials": args["materials"],
    });
    import_args["sockets"] = exported["socket_details"].clone();
    for key in ["importer", "normals", "socket_rotation"] {
        if let Some(v) = args.get(key).filter(|v| v.is_string()) {
            import_args[key] = v.clone();
        }
    }
    let imported = crate::unreal::import_fbx(&fbx, import_args)?;
    let checks = compare(&exported, &imported);
    Ok(json!({"fbx": fbx, "exported": exported, "imported": imported["imported"], "importer": imported["importer"],
        "attempts": imported["attempts"], "render_check": imported["render_check"], "sockets": imported["sockets"], "import_log": imported["import_log"],
        "problems": checks, "passed": checks.is_empty(),
        "_images": imported["_images"], "_cleanup": imported["_cleanup"]}))
}

/// What should survive the trip: height (Z is up on both sides), a root bone scale of 1, and the
/// character's facing. Sides are worked out from bone names on both sides, so a mirrored rig
/// would still look right-handed; the facing is what changes. Blender's (x, y, z) arrives in
/// Unreal as (x, -y, z) with the export's default axes.
fn compare(exported: &Value, imported: &Value) -> Vec<String> {
    let mut problems = Vec::new();
    if imported["failed"] == true {
        // Whether each broken asset is gone is what the Asset Registry said after the delete
        // (`cleaned_up[].deleted`), not assumed (RA-553).
        let left: Vec<&str> = imported["attempts"].as_array().into_iter().flatten()
            .flat_map(|attempt| attempt["cleaned_up"].as_array().into_iter().flatten())
            .filter(|asset| asset["deleted"] == false)
            .filter_map(|asset| asset["path"].as_str())
            .collect();
        let cleanup = if left.is_empty() {
            "the broken assets were deleted again".to_string()
        } else {
            format!("these broken assets could not be deleted and are still in the project: {}", left.join(", "))
        };
        problems.push(format!(
            "the import failed with every importer tried; {cleanup}: {}",
            serde_json::to_string(&imported["attempts"]).unwrap_or_default()
        ));
    }
    for check in imported["render_check"].as_array().cloned().unwrap_or_default() {
        if check["renders"] == false {
            problems.push(format!(
                "{} does not render (coverage {}): typically broken normals or tangents - re-export after blender_mesh_check, and import with normals=FBXNIM_IMPORT_NORMALS",
                check["path"].as_str().unwrap_or("?"), check["coverage"]
            ));
        }
    }
    let blender_height = exported["size_cm"][2].as_f64();
    let expected_forward = exported["forward_world"].as_array()
        .and_then(|f| Some([f.first()?.as_f64()?, -f.get(1)?.as_f64()?, f.get(2)?.as_f64()?]));
    for asset in imported["imported"].as_array().cloned().unwrap_or_default() {
        let path = asset["path"].as_str().unwrap_or("?");
        if let (Some(b), Some(u)) = (blender_height, asset["size_cm"][2].as_f64()) {
            if b > 0.1 {
                let ratio = u / b;
                if !(0.9..=1.1).contains(&ratio) {
                    problems.push(format!(
                        "{path} is {u:.1} cm tall in Unreal but {b:.1} cm in Blender (x{ratio:.3}); {}",
                        if (80.0..120.0).contains(&ratio) || (0.008..0.012).contains(&ratio) {
                            "a unit mismatch: check the scene's unit scale and the export's apply_scale_options"
                        } else {
                            "check the object's scale and the import's scale settings"
                        }
                    ));
                }
            }
        }
        if let Some(mats) = asset["transient_materials"].as_array().filter(|m| !m.is_empty()) {
            problems.push(format!("{path}: materials {mats:?} are not saved assets, so the mesh cannot be saved; import with materials=false and assign project materials"));
        }
        if let Some(scale) = asset["root_bone_scale"].as_array() {
            if scale.iter().filter_map(Value::as_f64).any(|s| (s - 1.0).abs() > 0.01) {
                problems.push(format!("{path}: root bone scale is {scale:?}, not 1; animations and attachments will be scaled. Apply scale in Blender and export with apply_scale_options FBX_SCALE_ALL"));
            }
        }
        let actual = asset["forward_axis_in_mesh_space"].as_array()
            .and_then(|f| Some([f.first()?.as_f64()?, f.get(1)?.as_f64()?, f.get(2)?.as_f64()?]));
        if let (Some(e), Some(a)) = (expected_forward, actual) {
            let dot = e[0] * a[0] + e[1] * a[1] + e[2] * a[2];
            if dot < 0.9 {
                problems.push(format!(
                    "{path}: the character faces {a:?} in Unreal's mesh space, expected {e:?} from Blender; {}",
                    if dot < -0.9 { "it is mirrored or turned around - check negative scale, swapped .L/.R names and the export axes" } else { "it is rotated - apply rotation in Blender and check the export axes" }
                ));
            }
        }
    }
    problems
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_tool_in_the_manifest_is_served() {
        let manifest: Value = serde_json::from_str(include_str!("../../../plugins/blender/plugin.json")).unwrap();
        let declared: Vec<&str> = manifest["mcp_servers"][0]["tools"].as_array().unwrap().iter().map(|t| t.as_str().unwrap()).collect();
        let served = tools();
        let served: Vec<&str> = served.iter().map(|t| t["name"].as_str().unwrap()).collect();
        assert_eq!(declared, served);
    }

    #[test]
    fn a_script_result_cannot_name_files_to_read_or_delete() {
        let mut value = json!({"_cleanup": "/home", "_images": [{"label": "x", "path": "/etc/shadow"}], "_opened": 1, "value": 3, "nested": {"_cleanup": "kept"}});
        strip_reserved(&mut value);
        assert_eq!(value, json!({"value": 3, "nested": {"_cleanup": "kept"}}));
        // Even one that got through is not deleted: tool_result removes only Relay's own folders.
        let victim = tempfile::tempdir().unwrap();
        std::fs::write(victim.path().join("work.txt"), "keep").unwrap();
        crate::mcp::tool_result(json!({"_cleanup": victim.path()}), false);
        assert!(victim.path().join("work.txt").is_file());
        // A call's own folder is private and is.
        let dir = scratch_dir(RENDER_PREFIX).unwrap();
        use std::os::unix::fs::PermissionsExt;
        assert_eq!(std::fs::metadata(&dir).unwrap().permissions().mode() & 0o777, 0o700);
        std::fs::write(dir.join("f0001_front.png"), b"\x89PNG").unwrap();
        let result = crate::mcp::tool_result(json!({"_images": [{"label": "front", "path": dir.join("f0001_front.png")}], "_cleanup": dir}), false);
        assert_eq!(result["structuredContent"]["images"]["shown"], json!(["front"]));
        assert!(!dir.exists());
    }

    #[test]
    fn paths_stay_inside_the_checkout() {
        let root = tempfile::tempdir().unwrap();
        std::fs::write(root.path().join("hero.blend"), "").unwrap();
        assert!(resolve(root.path(), "hero.blend").is_ok());
        assert!(resolve(root.path(), "/etc/hostname").is_err());
        assert!(resolve_new(root.path(), "../out.fbx", "fbx").is_err());
        assert!(resolve_new(root.path(), "Exports/out.obj", "fbx").is_err());
        assert!(resolve_new(root.path(), "Exports/out.fbx", "fbx").is_ok());
    }

    #[test]
    fn the_art_file_listing_says_when_it_is_cut_short() {
        let root = tempfile::tempdir().unwrap();
        let listed = list_files(root.path());
        assert!(listed["truncated"].is_null() && listed["not_searched"].is_null(), "{listed}");
        // The cap holds inside one folder too, after a subfolder already filled the list.
        std::fs::create_dir_all(root.path().join("a")).unwrap();
        for n in 0..MAX_LISTED {
            std::fs::write(root.path().join(format!("a/{n:03}.fbx")), "").unwrap();
        }
        std::fs::write(root.path().join("b.obj"), "").unwrap();
        // Named to be walked first, before the cap ends the walk.
        let mut deep = root.path().to_path_buf();
        for _ in 0..=MAX_DEPTH {
            deep.push("0");
        }
        std::fs::create_dir_all(&deep).unwrap();
        let listed = list_files(root.path());
        assert_eq!(listed["files"].as_array().unwrap().len(), MAX_LISTED);
        assert!(listed["truncated"].as_str().unwrap().contains("first 500"), "{listed}");
        assert!(listed["not_searched"].as_str().unwrap().ends_with("searched: 1"), "{listed}");
    }

    #[test]
    fn a_failure_keeps_what_the_script_printed_before_it() {
        let stdout = "Blender 5.2\nRELAY_OUT_BEGIN\nstep 1 done\nstep 2 done\nRELAY_OUT_END\n";
        let stderr = "Traceback (most recent call last):\n  File \"script\", line 3\nValueError: nope\n";
        let text = failure_text(stdout, stderr);
        assert!(text.starts_with("printed before it failed:\nstep 1 done\nstep 2 done\n\nTraceback"), "{text}");
        assert!(text.ends_with("ValueError: nope") && !text.contains("Blender 5.2"), "{text}");
        // Other scripts print no markers, and a traceback the script printed itself is not repeated.
        assert_eq!(failure_text("Blender 5.2\n", stderr), failure_text("", stderr));
        let caught = "RELAY_OUT_BEGIN\nTraceback (most recent call last):\nKeyError: 'x'\nRELAY_OUT_END\n";
        assert!(!failure_text(caught, stderr).contains("printed before"));
    }

    #[test]
    fn a_round_trip_flags_units_root_scale_and_facing() {
        let exported = json!({"size_cm": [60.0, 20.0, 178.0], "forward_world": [0.0, -1.0, 0.0]});
        let good = json!({"imported": [{"path": "/Game/Hero", "size_cm": [60.0, 20.0, 178.2], "root_bone_scale": [1.0, 1.0, 1.0], "forward_axis_in_mesh_space": [0.0, 1.0, 0.0]}]});
        assert!(compare(&exported, &good).is_empty(), "{:?}", compare(&exported, &good));
        let bad = json!({"imported": [{"path": "/Game/Hero", "size_cm": [6000.0, 2000.0, 17800.0], "root_bone_scale": [100.0, 100.0, 100.0], "forward_axis_in_mesh_space": [0.0, -1.0, 0.0]}]});
        let problems = compare(&exported, &bad);
        assert!(problems[0].contains("unit mismatch"), "{problems:?}");
        assert!(problems.iter().any(|p| p.contains("root bone scale")));
        assert!(problems.iter().any(|p| p.contains("mirrored or turned around")));
        let turned = json!({"imported": [{"path": "/Game/Hero", "size_cm": [60.0, 20.0, 178.0], "forward_axis_in_mesh_space": [1.0, 0.0, 0.0]}]});
        assert!(compare(&exported, &turned)[0].contains("rotated"));

        // A failed import names what the registry still holds, rather than claiming it is gone.
        let attempt = |deleted: bool| json!({"importer": "legacy", "cleaned_up": [{"path": "/Game/Hero", "deleted": deleted}]});
        let gone = compare(&exported, &json!({"failed": true, "attempts": [attempt(true)]}));
        assert!(gone[0].contains("deleted again"), "{gone:?}");
        let stuck = compare(&exported, &json!({"failed": true, "attempts": [attempt(true), attempt(false)]}));
        assert!(stuck[0].contains("still in the project: /Game/Hero") && !stuck[0].contains("deleted again"), "{stuck:?}");
    }

    #[test]
    fn output_is_capped_in_bytes_and_a_save_lock_excludes_a_second_saver() {
        let long = format!("{}\n{}", "x".repeat(10), "é".repeat(MAX_OUTPUT));
        let cut = tail(&long, 300);
        assert!(cut.len() <= MAX_OUTPUT && cut.chars().all(|c| c == 'é'), "{}", cut.len());
        assert_eq!(tail("a\nb\nc", 2), "b\nc");

        let dir = tempfile::tempdir().unwrap();
        let blend = dir.path().join("hero.blend");
        let held = SaveLock::take(&blend, Duration::from_secs(60)).unwrap();
        let refused = SaveLock::take(&blend, Duration::from_secs(60)).err().unwrap();
        assert!(format!("{refused:#}").contains("another blender_python call"), "{refused:#}");
        drop(held);
        assert!(!dir.path().join("hero.blend.relay-lock").exists());
        // One left by a killed server expires.
        std::fs::write(dir.path().join("hero.blend.relay-lock"), r#"{"pid":1,"until":1}"#).unwrap();
        assert!(SaveLock::take(&blend, Duration::from_secs(60)).is_ok());
    }

    /// RA-077: agent code in a sandboxed run cannot write outside the write roots, and can inside.
    #[test]
    fn sandboxed_scripts_write_only_inside_the_write_roots() {
        if find_blender().is_err() || !crate::blender_sandbox::available() {
            eprintln!("Blender or a working bwrap missing; skipping");
            return;
        }
        let allowed = tempfile::tempdir().unwrap();
        let outside = tempfile::tempdir().unwrap();
        let _sandbox = crate::blender_sandbox::test_roots(&[allowed.path()]);
        let t = Duration::from_secs(120);
        let write = |dir: &Path| json!({"code": format!("open({:?}, 'w').write('x')\nprint('wrote')", dir.join("probe.txt"))});

        let inside = run(PY_RUN, &write(allowed.path()), None, t).unwrap();
        assert_eq!(inside["output"].as_str().unwrap().trim(), "wrote", "{inside}");
        assert!(inside["sandbox"].as_str().unwrap().starts_with("bwrap:"), "{inside}");
        assert!(allowed.path().join("probe.txt").is_file());

        let error = format!("{:#}", run(PY_RUN, &write(outside.path()), None, t).unwrap_err());
        assert!(error.contains("Read-only file system"), "{error}");
        assert!(error.contains("sandbox bwrap:"), "a failure says it ran sandboxed: {error}");
        assert!(!outside.path().join("probe.txt").exists());
        let home = std::env::var("HOME").unwrap();
        let error = format!("{:#}", run(PY_RUN, &json!({"code": format!("open({:?}, 'w')", format!("{home}/.relay-sandbox-probe"))}), None, t).unwrap_err());
        assert!(error.contains("Read-only file system"), "{error}");
        // No network either.
        let net = run(PY_RUN, &json!({"code": "import socket\ntry:\n    socket.create_connection(('1.1.1.1', 80), timeout=2)\n    print('online')\nexcept OSError:\n    print('offline')"}), None, t).unwrap();
        assert_eq!(net["output"].as_str().unwrap().trim(), "offline");
        // Nor the user's sockets: the session bus and the service manager would run anything
        // outside the sandbox. The environment does not name them, the folder holding them is
        // empty, and the session is the sandbox's own (one led from outside its pids reads 0).
        let sockets = run(PY_RUN, &json!({"code": "import os\nrun = '/run/user/%d' % os.getuid()\nprint(sorted(k for k in os.environ if 'DBUS' in k or 'SOCK' in k or k.startswith('RELAY') or k == 'XDG_RUNTIME_DIR'), os.listdir(run) if os.path.isdir(run) else [], os.getsid(0) != 0)"}), None, t).unwrap();
        assert_eq!(sockets["output"].as_str().unwrap().trim(), "[] [] True", "{sockets}");
    }

    /// Runs every Blender script against a fixture built by `blender_py/tests/make_fixture.py`,
    /// with real Blender. Skipped where Blender is not installed (CI runners do not carry it).
    #[test]
    fn the_blender_tools_work_on_a_real_rig() {
        let Ok(blender) = find_blender() else {
            eprintln!("Blender not installed; skipping");
            return;
        };
        let dir = tempfile::tempdir().unwrap();
        // Under the sandbox where bwrap works, so the real tools are known to work confined.
        let _sandbox = crate::blender_sandbox::test_roots(&[dir.path()]);
        let fixture = dir.path().join("fixture.blend");
        let maker = Path::new(env!("CARGO_MANIFEST_DIR")).join("src/blender_py/tests/make_fixture.py");
        let status = Command::new(&blender).args(["-b", "--factory-startup", "--python"]).arg(&maker).arg("--").arg(&fixture)
            .output().unwrap();
        assert!(fixture.is_file(), "fixture not built: {}", String::from_utf8_lossy(&status.stdout));
        let t = Duration::from_secs(300);

        let info = run(PY_INFO, &json!({}), Some(&fixture), t).unwrap();
        assert!(!crate::blender_sandbox::available() || info["sandbox"].as_str().unwrap().starts_with("bwrap:"), "{}", info["sandbox"]);
        assert!(info["objects"].as_array().unwrap().iter().any(|o| o["name"] == "Hero" && o["armature"]["left_right_pairs"] == 6));
        // Blender 5 keeps F-curves in slot channelbags; the count must not read zero there.
        let swing = info["actions"].as_array().unwrap().iter().find(|a| a["name"] == "Swing").unwrap();
        assert!(swing["fcurves"].as_u64().unwrap() > 0 && swing["bones_animated"].as_u64().unwrap() > 0, "{} {swing}", info["blender"]);

        let rig = run(PY_RIG_CHECK, &json!({"armature": "Hero"}), Some(&fixture), t).unwrap();
        assert_eq!(rig["passed"], true, "{rig}");
        assert_eq!(rig["faces"], "-Y (Blender front)");
        assert_eq!(rig["height_cm"], 178.0);

        let anim = run(PY_ANIM_INSPECT, &json!({
            "armature": "Hero", "action": "Swing", "samples": 3, "track": ["hand.R", "hand.L"],
            "attachments": [{"object": "Sword", "grips": [{"point": "obj:Grip", "bone": "hand.R:tail"}]}]
        }), Some(&fixture), t).unwrap();
        assert_eq!(anim["attachments"]["Sword"]["side"], "right", "{anim}");
        assert_eq!(anim["attachments"]["Sword"]["held_by"], "hand.R");
        assert_eq!(anim["samples"][0]["points"]["hand.L"]["side"], "left");
        assert_eq!(anim["samples"][0]["checks"][0]["distance"], 0.0);

        let out = dir.path().join("Hero.fbx");
        let exported = run(PY_EXPORT, &json!({"path": out, "objects": ["Hero"], "animations": true, "action": "Swing"}), Some(&fixture), t);
        match exported {
            Ok(exported) => {
                assert_eq!(exported["kind"], "skeletal");
                assert!(!exported["objects"].as_array().unwrap().iter().any(|o| o == "Sword"), "a bone-parented prop went into the character");
                assert!(out.is_file());
            }
            // Distribution builds of Blender without numpy cannot run the FBX exporter.
            Err(error) => assert!(format!("{error:#}").contains("numpy"), "{error:#}"),
        }

        let rendered = dir.path().join("renders");
        std::fs::create_dir_all(&rendered).unwrap();
        let shots = run(PY_RENDER, &json!({"out_dir": rendered, "objects": ["Hero"], "views": ["front"], "width": 96, "height": 96}), Some(&fixture), t).unwrap();
        assert!(Path::new(shots["files"][0]["file"].as_str().unwrap()).is_file());

        let script = run(PY_RUN, &json!({"code": "print(len(bpy.data.objects))"}), Some(&fixture), t).unwrap();
        assert_eq!(script["output"].as_str().unwrap().lines().last(), Some("6"));

        // A bevel wider than a thin plate collapses faces: the check sees it on the evaluated mesh
        // and export refuses to write it.
        let plate = dir.path().join("sub/dir/plate.blend");
        let made = run(PY_RUN, &json!({
            "code": "import bmesh\nbpy.ops.wm.read_factory_settings(use_empty=True)\nme = bpy.data.meshes.new('Plate')\nbm = bmesh.new()\nbmesh.ops.create_cube(bm, size=1.0)\nfor v in bm.verts:\n    v.co.z *= 0.002\nbm.to_mesh(me)\nbm.free()\no = bpy.data.objects.new('Plate', me)\nbpy.context.scene.collection.objects.link(o)\nm = o.modifiers.new('Bevel', 'BEVEL')\nm.width = 0.3\nm.segments = 3\nm.limit_method = 'NONE'\nprint('made')",
            "save_as": plate,
        }), None, t).unwrap();
        assert_eq!(made["output"].as_str().unwrap().trim(), "made");
        assert!(plate.is_file(), "save_as did not create its folders");
        let check = run(PY_MESH_CHECK, &json!({}), Some(&plate), t).unwrap();
        assert_eq!(check["passed"], false, "{check}");
        assert!(check["meshes"][0]["degenerate_faces"].as_u64().unwrap() > 0 || check["meshes"][0]["zero_length_edges"].as_u64().unwrap() > 0, "{check}");
        let refused = run(PY_EXPORT, &json!({"path": dir.path().join("plate.fbx"), "kind": "static"}), Some(&plate), t).unwrap_err();
        assert!(format!("{refused:#}").contains("not exported"), "{refused:#}");

        // Socket empties report how they are turned relative to their mesh, which the import uses
        // to take the axis conversion's roll back off.
        let crate_file = dir.path().join("crate.blend");
        run(PY_RUN, &json!({
            "code": "import bmesh\nbpy.ops.wm.read_factory_settings(use_empty=True)\nme = bpy.data.meshes.new('SM_Crate')\nbm = bmesh.new()\nbmesh.ops.create_cube(bm, size=1.0)\nbm.to_mesh(me)\nbm.free()\nme.uv_layers.new(name='UVMap')\no = bpy.data.objects.new('SM_Crate', me)\nbpy.context.scene.collection.objects.link(o)\nfor name, yaw in (('SOCKET_Top', 0.0), ('SOCKET_Side', 1.5708)):\n    e = bpy.data.objects.new(name, None)\n    bpy.context.scene.collection.objects.link(e)\n    e.parent = o\n    e.location = (0, 0, 0.5)\n    e.rotation_euler = (0, 0, yaw)",
            "save_as": crate_file,
        }), None, t).unwrap();
        match run(PY_EXPORT, &json!({"path": dir.path().join("crate.fbx"), "objects": ["SM_Crate"], "kind": "static"}), Some(&crate_file), t) {
            Ok(exported) => {
                let details = exported["socket_details"].as_array().unwrap();
                let top = details.iter().find(|d| d["name"] == "Top").unwrap();
                let side = details.iter().find(|d| d["name"] == "Side").unwrap();
                assert_eq!(top["identity"], true, "{exported}");
                assert_eq!(side["identity"], false);
                assert_eq!(side["rotation_deg"][2], 90.0);
            }
            Err(error) => assert!(format!("{error:#}").contains("numpy"), "{error:#}"),
        }

        // Every way of hiding an object still exports it, and an excluded collection is reported
        // rather than failing the export or vanishing from it.
        let hidden_file = dir.path().join("hidden.blend");
        run(PY_RUN, &json!({
            "code": "import bmesh\nbpy.ops.wm.read_factory_settings(use_empty=True)\nscene = bpy.context.scene\ndef box(name, coll):\n    me = bpy.data.meshes.new(name)\n    bm = bmesh.new()\n    bmesh.ops.create_cube(bm, size=1.0)\n    bm.to_mesh(me)\n    bm.free()\n    me.uv_layers.new(name='UVMap')\n    o = bpy.data.objects.new(name, me)\n    c = bpy.data.collections.new(coll)\n    scene.collection.children.link(c)\n    c.objects.link(o)\n    return o, c\nbox('SM_Eye', 'Eye')\n_, monitor = box('SM_Monitor', 'Monitor')\nmonitor.hide_viewport = True\n_, unpickable = box('SM_Unpickable', 'Unpickable')\nunpickable.hide_select = True\no, _ = box('SM_Locked', 'Locked')\no.hide_select = True\nbox('RefBox', 'Reference')\nlayers = bpy.context.view_layer.layer_collection.children\nlayers['Eye'].hide_viewport = True\nlayers['Reference'].exclude = True",
            "save_as": hidden_file,
        }), None, t).unwrap();
        let hidden_fbx = dir.path().join("hidden.fbx");
        match run(PY_EXPORT, &json!({"path": hidden_fbx, "kind": "static"}), Some(&hidden_file), t) {
            Ok(exported) => {
                assert_eq!(exported["objects"], json!(["SM_Eye", "SM_Locked", "SM_Monitor", "SM_Unpickable"]), "{exported}");
                assert_eq!(exported["excluded"], json!(["RefBox"]));
                let back = run(PY_RUN, &json!({"code": format!(
                    "bpy.ops.wm.read_factory_settings(use_empty=True)\nbpy.ops.import_scene.fbx(filepath={:?})\nprint(sorted(o.name for o in bpy.data.objects if o.type == 'MESH'))",
                    hidden_fbx.display().to_string())}), None, t).unwrap();
                assert_eq!(back["output"].as_str().unwrap().lines().last(), Some("['SM_Eye', 'SM_Locked', 'SM_Monitor', 'SM_Unpickable']"), "{back}");
                // Named, an excluded object is brought in.
                let named = run(PY_EXPORT, &json!({"path": dir.path().join("ref.fbx"), "objects": ["RefBox"], "kind": "static"}), Some(&hidden_file), t).unwrap();
                assert_eq!(named["objects"], json!(["RefBox"]), "{named}");
            }
            Err(error) => assert!(format!("{error:#}").contains("numpy"), "{error:#}"),
        }

        // Another rig's action plays: Swing was made on Hero, so its slot does not auto-assign to
        // Partner, and the pose stayed at rest. The slot is picked by the curves that resolve.
        let partner = run(PY_RUN, &json!({"code": "a = bpy.data.objects['Partner']\nemit.__globals__['set_action'](a, 'Swing')\nbpy.context.scene.frame_set(20)\nprint(a.animation_data.action_slot.identifier, round(a.pose.bones['upper_arm.R'].rotation_euler.x, 2))"}), Some(&fixture), t).unwrap();
        assert_eq!(partner["output"].as_str().unwrap().lines().last(), Some("OBHero 1.57"), "{partner}");
        let foreign = run(PY_RUN, &json!({"code": "e = bpy.data.objects.new('Tmp', None)\nbpy.context.scene.collection.objects.link(e)\ne['foo'] = 1.0\ne.keyframe_insert('[\"foo\"]', frame=1)\nemit.__globals__['set_action'](bpy.data.objects['Partner'], e.animation_data.action.name)"}), Some(&fixture), t).unwrap_err();
        assert!(format!("{foreign:#}").contains("animates nothing on Partner"), "{foreign:#}");

        // A posed, unkeyed rig still measures its bind pose, which is what Unreal measures.
        let posed = dir.path().join("posed.blend");
        run(PY_RUN, &json!({"code": "h = bpy.data.objects['Hero']\nh.animation_data.action = None\nh.pose.bones['root'].scale = (0.5, 0.5, 0.5)", "save_as": posed}), Some(&fixture), t).unwrap();
        assert_eq!(run(PY_RIG_CHECK, &json!({"armature": "Hero"}), Some(&posed), t).unwrap()["height_cm"], 178.0);
        match run(PY_EXPORT, &json!({"path": dir.path().join("posed.fbx"), "objects": ["Hero"]}), Some(&posed), t) {
            Ok(exported) => assert_eq!(exported["size_cm"][2], 178.0, "{exported}"),
            Err(error) => assert!(format!("{error:#}").contains("numpy"), "{error:#}"),
        }

        // A rotation in quaternion (glTF imports) or axis-angle mode is a rotation too.
        let turned = dir.path().join("turned.blend");
        run(PY_RUN, &json!({"code": "import bmesh\nbpy.ops.wm.read_factory_settings(use_empty=True)\nfor name, mode in (('Quat', 'QUATERNION'), ('AxisAngle', 'AXIS_ANGLE'), ('Straight', 'QUATERNION')):\n    me = bpy.data.meshes.new(name)\n    bm = bmesh.new()\n    bmesh.ops.create_cube(bm, size=1.0)\n    bm.to_mesh(me)\n    bm.free()\n    o = bpy.data.objects.new(name, me)\n    bpy.context.scene.collection.objects.link(o)\n    o.rotation_mode = mode\n    if name != 'Straight':\n        o.rotation_euler = (0, 0, 0)\n        o.rotation_mode = 'XYZ'\n        o.rotation_euler = (0, 0, 1.5708)\n        o.rotation_mode = mode", "save_as": turned}), None, t).unwrap();
        let info = run(PY_INFO, &json!({}), Some(&turned), t).unwrap();
        let flags = |name: &str| info["objects"].as_array().unwrap().iter().find(|o| o["name"] == name).unwrap()["flags"].to_string();
        assert!(flags("Quat").contains("rotation not applied") && flags("AxisAngle").contains("rotation not applied"), "{info}");
        assert_eq!(flags("Straight"), "[]");

        // An action reaches the rig through its mesh, and a tall subject fits a landscape image.
        let shots = run(PY_RENDER, &json!({"out_dir": rendered, "objects": ["HeroBody"], "action": "Swing", "frames": [20], "views": ["front"], "width": 64, "height": 48}), Some(&fixture), t).unwrap();
        assert_eq!((shots["armature"].as_str(), shots["action_slot"].as_str()), (Some("Hero"), Some("OBHero")), "{shots}");
        let pillar = dir.path().join("pillar.blend");
        run(PY_RUN, &json!({"code": "import bmesh\nbpy.ops.wm.read_factory_settings(use_empty=True)\nme = bpy.data.meshes.new('Pillar')\nbm = bmesh.new()\nbmesh.ops.create_cube(bm, size=1.0)\nfor v in bm.verts:\n    v.co.x *= 0.3\n    v.co.y *= 0.25\n    v.co.z *= 1.8\nbm.to_mesh(me)\nbm.free()\nbpy.context.scene.collection.objects.link(bpy.data.objects.new('Pillar', me))", "save_as": pillar}), None, t).unwrap();
        let shots = run(PY_RENDER, &json!({"out_dir": rendered, "views": ["front"], "width": 64, "height": 48}), Some(&pillar), t).unwrap();
        let png = shots["files"][0]["file"].as_str().unwrap();
        let rows = run(PY_RUN, &json!({"code": format!("im = bpy.data.images.load({png:?})\nw, h = im.size\npx = list(im.pixels)\nrow = lambda y: [tuple(round(c, 3) for c in px[(y * w + x) * 4:(y * w + x) * 4 + 3]) for x in range(w)]\nprint(max(max(p) for p in row(0) + row(h - 1)) < 0.05, max(max(p) for p in row(h // 2)) > 0.3)")}), None, t).unwrap();
        // The background is black (with dither); the pillar is grey.
        assert_eq!(rows["output"].as_str().unwrap().lines().last(), Some("True True"), "the pillar is cut off at the top or bottom: {rows}");
        let action_without_rig = run(PY_RENDER, &json!({"out_dir": rendered, "action": "Swing", "views": ["front"], "width": 64, "height": 48}), Some(&pillar), t).unwrap_err();
        assert!(format!("{action_without_rig:#}").contains("needs one armature"), "{action_without_rig:#}");
        let eevee = run(PY_RENDER, &json!({"out_dir": rendered, "views": ["front"], "width": 64, "height": 48, "engine": "eevee"}), Some(&pillar), t).unwrap();
        assert!(eevee["engine"].as_str().unwrap().starts_with("BLENDER_EEVEE"), "{eevee}");

        // Saving leaves no .blend1, and a save over a file that changed after it was opened is
        // refused rather than undoing the other change.
        let copy = dir.path().join("copy.blend");
        std::fs::copy(&pillar, &copy).unwrap();
        let saved = run(PY_RUN, &json!({"code": "bpy.data.objects['Pillar'].location.x = 1", "save": true, "_opened": opened_stamp(&copy).unwrap()}), Some(&copy), t).unwrap();
        assert_eq!(saved["saved"], json!(copy.display().to_string()), "{saved}");
        assert!(!dir.path().join("copy.blend1").exists(), "save left a .blend1 backup");
        let lost = run(PY_RUN, &json!({"code": "import os\nos.utime(bpy.data.filepath, ns=(1, 1))", "save": true, "_opened": opened_stamp(&copy).unwrap()}), Some(&copy), t).unwrap_err();
        assert!(format!("{lost:#}").contains("changed on disk after this run opened it"), "{lost:#}");

        // A print loop is kept to its tail, in bytes as well as lines.
        let chatty = run(PY_RUN, &json!({"code": "for i in range(200000):\n    print(i)\nprint('y' * 1000000)"}), None, t).unwrap();
        let output = chatty["output"].as_str().unwrap();
        assert!(output.len() <= MAX_OUTPUT && output.ends_with("yyy"), "{}", output.len());

        let broken = run(PY_RUN, &json!({"code": "print('got this far')\nraise ValueError('nope')"}), None, t).unwrap_err();
        assert!(format!("{broken:#}").contains("ValueError: nope"), "{broken:#}");
        assert!(format!("{broken:#}").contains("got this far"), "what the script printed is kept: {broken:#}");
    }

    /// The anim_rules fixes (shared with ue_anim_inspect), on a variant of the fixture: a bar at
    /// the left hand held by hand.R, the partner 55 cm ahead, a forward "Reach" with the right
    /// arm and a "Sink" that drops the whole rig 1 cm by frame 10 and 5 cm by frame 20.
    #[test]
    fn anim_inspect_rules_hold_on_a_real_rig() {
        let Ok(blender) = find_blender() else {
            eprintln!("Blender not installed; skipping");
            return;
        };
        let dir = tempfile::tempdir().unwrap();
        // Under the sandbox where bwrap works, so the real tools are known to work confined.
        let _sandbox = crate::blender_sandbox::test_roots(&[dir.path()]);
        let fixture = dir.path().join("fixture.blend");
        let maker = Path::new(env!("CARGO_MANIFEST_DIR")).join("src/blender_py/tests/make_fixture.py");
        Command::new(&blender).args(["-b", "--factory-startup", "--python"]).arg(&maker).arg("--").arg(&fixture).output().unwrap();
        let t = Duration::from_secs(300);
        let variant = dir.path().join("variant.blend");
        run(PY_RUN, &json!({"code": "import bmesh\nfrom mathutils import Vector, Matrix\nhero = bpy.data.objects['Hero']\nbpy.data.objects['Partner'].location.y = -0.55\nme = bpy.data.meshes.new('Bar')\nbm = bmesh.new()\nbmesh.ops.create_cube(bm, size=1.0)\nfor v in bm.verts:\n    v.co = Vector((v.co.x * 0.1, v.co.y * 0.04, v.co.z * 0.04))\nbm.to_mesh(me)\nbm.free()\nbar = bpy.data.objects.new('Bar', me)\nbpy.context.scene.collection.objects.link(bar)\nbar.parent, bar.parent_type, bar.parent_bone = hero, 'BONE', 'hand.R'\nbar.matrix_world = Matrix.Translation((0.8, 0, 1.35))\nfor name, bone, path, keys in (('Sink', 'root', 'location', ((1, (0, 0, 0)), (10, (0, 0, -0.01)), (20, (0, 0, -0.05)))), ('Reach', 'upper_arm.R', 'rotation_euler', ((1, (0, 0, 0)), (20, (0, 0, 1.5708))))):\n    act = bpy.data.actions.new(name)\n    act.use_fake_user = True\n    hero.animation_data.action = act\n    pb = hero.pose.bones[bone]\n    pb.rotation_mode = 'XYZ'\n    for f, v in keys:\n        setattr(pb, path, v)\n        pb.keyframe_insert(path, frame=f)\n    setattr(pb, path, (0, 0, 0))",
            "save_as": variant}), Some(&fixture), t).unwrap();
        let kinds = |r: &Value, kind: &str| -> Vec<Value> { r["problems"].as_array().unwrap().iter().filter(|p| p["kind"] == kind).cloned().collect() };

        // A grip written `bone:tail` exempts that hand's chain from the item's clearance.
        let bar = |grips: Value| run(PY_ANIM_INSPECT, &json!({"armature": "Hero", "frames": [1], "attachments": [{"object": "Bar", "grips": grips}]}), Some(&variant), t).unwrap();
        let gripped = bar(json!([{"point": "item:Bar:center", "bone": "hand.L:tail"}]));
        assert_eq!(gripped["passed"], true, "{gripped}");
        assert!(!kinds(&bar(json!([])), "clipping").is_empty(), "without the grip the bar clips the left hand");

        // A touch excuses partner clipping only inside its window and only against its bone.
        let reach = |bone: &str, window: [u32; 2]| run(PY_ANIM_INSPECT, &json!({"armature": "Hero", "action": "Reach", "frames": [1, 20],
            "partner": {"armature": "Partner"},
            "contacts": [{"a": "hand.R:tail", "b": format!("partner:{bone}"), "expect": "touch", "distance": 100, "window": window}]}), Some(&variant), t).unwrap();
        let inside = reach("upper_arm.L", [20, 20]);
        assert_eq!(inside["passed"], true, "{inside}");
        let outside = kinds(&reach("upper_arm.L", [1, 1]), "partner_clipping");
        assert!(outside.len() == 1 && outside[0]["frame"] == 20, "{outside:?}");
        let elsewhere = kinds(&reach("head", [20, 20]), "partner_clipping");
        assert!(elsewhere.len() == 1 && elsewhere[0]["detail"].as_str().unwrap().contains("upper_arm.L"), "{elsewhere:?}");

        // Ground: the lowest point of each foot against the rest-pose floor, not the ankle.
        let sink = run(PY_ANIM_INSPECT, &json!({"armature": "Hero", "action": "Sink", "frames": [1, 10, 20]}), Some(&variant), t).unwrap();
        let ground = kinds(&sink, "ground");
        assert!(!ground.is_empty() && ground.iter().all(|p| p["frame"] == 20), "{sink}");
        assert_eq!(sink["samples"][0]["feet_height"]["foot.L"], 0.0, "{sink}");
        assert_eq!(sink["samples"][1]["feet_height"]["foot.L"], -1.0);
    }
}
