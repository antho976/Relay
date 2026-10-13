//! `relay unreal-mcp` — the Unreal Engine plugin's MCP server (D159).
//!
//! Stdio JSON-RPC, like `relay mcp`, but it never touches the Relay bus: it works on the
//! Unreal project in the agent's checkout and on the Unreal editor running beside it. Offline
//! tools read the `.uproject`, run UnrealBuildTool and tail the editor log. Live tools talk to
//! the editor's Remote Control web server (plain HTTP on localhost, port 30010 by default) and
//! run editor Python through `PythonScriptLibrary`, which is how an agent edits binary assets
//! it cannot open as text.
//!
//! Environment: `UE_PROJECT` (a `.uproject` path) overrides discovery, `UE_ROOT` names the
//! engine directory (the one holding `Engine/`), `UE_REMOTE_CONTROL_URL` the editor endpoint
//! and `UE_REMOTE_CONTROL_PASSPHRASE` its passphrase when one is required.

use crate::mcp::tool;
use anyhow::{anyhow, bail, Context, Result};
use serde_json::{json, Map, Value};
use std::io::{Read, Write};
use std::net::{TcpStream, ToSocketAddrs};
use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::Duration;

const DEFAULT_REMOTE: &str = "http://127.0.0.1:30010";
const PYTHON_LIBRARY: &str = "/Script/PythonScriptPlugin.Default__PythonScriptLibrary";
/// Editor plugins the live tools depend on, by `.uproject` plugin name.
const BRIDGE_PLUGINS: [&str; 3] = ["RemoteControl", "PythonScriptPlugin", "EditorScriptingUtilities"];
/// Folders a project scan never enters: generated, cached or version-control state.
const SKIP_DIRS: [&str; 7] = ["Binaries", "Intermediate", "Saved", "DerivedDataCache", ".git", "node_modules", ".relay"];
const MAX_OUTPUT: usize = 60_000;
/// The largest reply read from the editor. A script printing in a loop over every asset is
/// refused here rather than held in memory several times over (RA-292).
const MAX_REPLY: u64 = 64 * 1024 * 1024;
/// Tools that act on the running editor. Each first checks the editor has this checkout's
/// project open: an agent in a worktree would otherwise edit assets in a different copy.
/// `ue_editor_lock` is not one: the lock is a file, and releasing it must work when the editor
/// has crashed or been closed.
const LIVE: [&str; 15] = ["ue_python", "ue_call", "ue_property", "ue_search_assets", "ue_level_actors", "ue_console",
    "ue_screenshot", "ue_anim_inspect", "ue_anim_preview", "ue_play", "ue_blueprint_info",
    "ue_asset_audit", "ue_asset_refs", "ue_data_table", "ue_profile"];
/// Live tools that change editor state, and so need the editor lock.
const MUTATING: [&str; 10] = ["ue_python", "ue_call", "ue_console", "ue_screenshot", "ue_anim_preview", "ue_property",
    "ue_play", "ue_profile", "ue_data_table", "ue_blueprint_info"];
/// A lock nobody has used for this long is free to take.
const LOCK_IDLE: Duration = Duration::from_secs(15 * 60);
/// How often a call in flight refreshes the lock's idle clock, well inside `LOCK_IDLE`.
const LOCK_HEARTBEAT: Duration = Duration::from_secs(60);
/// Log lines a tool returns: the first half and the last half of what matched.
const MAX_LOG_LINES: usize = 300;

// Every script gets the character frame and bone naming shared with the Blender tools.
const PY_COMMON: &str = concat!(include_str!("unreal_py/common.py"), "\n", include_str!("rig_frame.py"));
const PY_PROJECT_CHECK: &str = include_str!("unreal_py/project_check.py");
const PY_CAPTURE: &str = include_str!("unreal_py/capture.py");
// The checks are shared with blender_anim_inspect (`anim_rules.py`); the script only poses the skeleton.
const PY_ANIM_INSPECT: &str = concat!(include_str!("anim_rules.py"), "\n", include_str!("unreal_py/anim_inspect.py"));
const PY_ANIM_PREVIEW: &str = include_str!("unreal_py/anim_preview.py");
const PY_PLAY: &str = include_str!("unreal_py/play.py");
const PY_BLUEPRINT_INFO: &str = include_str!("unreal_py/blueprint_info.py");
const PY_ASSET_AUDIT: &str = include_str!("unreal_py/asset_audit.py");
const PY_ASSET_REFS: &str = include_str!("unreal_py/asset_refs.py");
const PY_DATA_TABLE: &str = include_str!("unreal_py/data_table.py");
const PY_IMPORT_FBX: &str = include_str!("unreal_py/import_fbx.py");
const PY_PREVIEW_ASSET: &str = include_str!("unreal_py/preview_asset.py");

pub fn serve() -> Result<u8> {
    crate::mcp::serve_sync(handle, lane)
}

/// Everything that drives the editor, builds or launches it shares one editor and waits its
/// turn (the editor lock names this server's holder, so it cannot keep two of its own calls
/// apart); reading the project, the log and crash reports runs beside a long build (RA-061).
fn lane(name: &str, args: &Value) -> crate::mcp::Lane {
    let reads = matches!(name, "ue_project_info" | "ue_log" | "ue_editor_status" | "ue_crash")
        || name == "ue_setup_check" && args["fix"] != true;
    if reads { crate::mcp::Lane::Parallel } else { crate::mcp::Lane::Serial }
}

const SERVER: crate::mcp::PluginServer = crate::mcp::PluginServer {
    name: "unreal",
    title: "Unreal Engine (Relay plugin)",
    instructions: "Unreal Engine bridge for the project in this checkout. Start with ue_project_info. ue_build and ue_log work without the editor; ue_python, ue_call, ue_property, ue_search_assets, ue_level_actors and ue_console need the editor open with its Remote Control web server running (check with ue_editor_status, diagnose with ue_setup_check). Load the unreal-editor-automation skill before editor scripting.",
    tools,
    call,
};

fn handle(message: &Value) -> Option<Value> {
    crate::mcp::plugin_reply(&SERVER, message)
}

fn tools() -> Vec<Value> {
    vec![
        tool("ue_project_info",
            "The Unreal project in this checkout: .uproject fields (engine association, modules, plugins), build targets, C++ modules, project plugins, config files, maps, asset counts, the resolved engine directory and log path. Works without the editor.",
            json!({}), &[], true),
        tool("ue_setup_check",
            "Whether the editor bridge is ready: the RemoteControl, PythonScriptPlugin and EditorScriptingUtilities plugins in the .uproject, whether the editor's Remote Control server answers, and whether remote Python runs. Also whether Use Less CPU when in Background is on. With fix=true, adds the missing plugins to the .uproject (takes effect after the editor restarts), writes the Remote Control settings, and turns the background throttle off in the running editor and in the project's config, so it stays off across restarts.",
            json!({"fix":{"type":"boolean","description":"Add missing bridge plugins to the .uproject"}}), &[], false),
        tool("ue_build",
            "Build with UnrealBuildTool through the engine's Build script. Defaults: the project's Editor target, the host platform, Development. Close the editor first (or use Live Coding in the editor instead). Returns success, the error lines and the tail of the output.",
            json!({
                "target":{"type":"string","description":"Target name, e.g. MyGameEditor, MyGame, MyGameServer"},
                "platform":{"type":"string","description":"Linux, Win64 or Mac; defaults to the host"},
                "configuration":{"type":"string","enum":["Debug","DebugGame","Development","Test","Shipping"]},
                "timeout_s":{"type":"integer","minimum":30,"maximum":7200,"description":"Default 3600"},
                "restart_editor":{"type":"boolean","description":"Quit a running editor first (saving), build, then relaunch it and wait until it answers. The way to apply C++ changes on Linux, which has no Live Coding. Refuses when the editor cannot be asked to save (see ue_editor_quit)."},
                "save":{"type":"boolean","description":"With restart_editor: save dirty packages before quitting (default true)"},
                "force":{"type":"boolean","description":"With restart_editor: terminate an editor that does not answer Remote Control, losing its unsaved work. Only when the human has said so."},
                "launch_timeout_s":{"type":"integer","minimum":30,"maximum":3600,"description":"With restart_editor: how long to wait for the relaunched editor to answer, default 900 (a first start compiles shaders)"},
                "allow_editor_open":{"type":"boolean","description":"Build even though the editor is running (hot-reload module; usually wrong)"},
                "keep_crash_reporters":{"type":"boolean","description":"Do not stop this project's leftover CrashReportClient processes before building (other projects' are never stopped)"}
            }), &[], false),
        tool("ue_log",
            "Tail the project's editor/game log (Saved/Logs/<Project>.log), optionally keeping only lines that match a regex such as 'Error|Warning' or 'LogBlueprint'.",
            json!({
                "lines":{"type":"integer","minimum":1,"maximum":5000,"description":"Lines to return, default 200"},
                "filter":{"type":"string","description":"Regex applied before taking the tail"},
                "file":{"type":"string","description":"Another file under Saved/Logs, e.g. a backup log"}
            }), &[], true),
        tool("ue_editor_status",
            "Whether the Unreal editor answers on its Remote Control web server, with the server's route information.",
            json!({}), &[], true),
        tool("ue_python",
            "Run Python in the running editor (import unreal). Returns printed output, log lines and the result. Wrap asset or level changes in `with unreal.ScopedEditorTransaction(\"...\"):` and save what you change.",
            json!({
                "code":{"type":"string","description":"Python source; multiple lines are fine"},
                "timeout_s":{"type":"integer","minimum":5,"maximum":1800,"description":"Default 120"}
            }), &["code"], false),
        tool("ue_call",
            "Call a UFUNCTION on an object through Remote Control (PUT /remote/object/call). Object paths look like /Game/Maps/Main.Main:PersistentLevel.Door_2 or, for static functions, /Script/Module.Default__Class.",
            json!({
                "object_path":{"type":"string"},
                "function":{"type":"string"},
                "parameters":{"type":"object","description":"Function parameters by name"},
                "transaction":{"type":"boolean","description":"Record an undoable transaction (default true)"}
            }), &["object_path","function"], false),
        tool("ue_property",
            "Read a property of an object through Remote Control, or write it when value is given (as an undoable transaction).",
            json!({
                "object_path":{"type":"string"},
                "property":{"type":"string"},
                "value":{"description":"New value; omit to read"}
            }), &["object_path","property"], false),
        tool("ue_search_assets",
            "Search the Asset Registry in the running editor by name fragment, class and folder.",
            json!({
                "query":{"type":"string","description":"Case-insensitive fragment of the asset name"},
                "class_names":{"type":"array","items":{"type":"string"},"description":"e.g. StaticMesh, Blueprint, Material, MaterialInstanceConstant, World"},
                "package_paths":{"type":"array","items":{"type":"string"},"description":"Folders to search, default /Game"},
                "limit":{"type":"integer","minimum":1,"maximum":2000,"description":"Default 100"}
            }), &[], true),
        tool("ue_level_actors",
            "List the actors in the level open in the editor: label, class, object path, location and folder.",
            json!({
                "class_filter":{"type":"string","description":"Case-insensitive fragment of the class name"},
                "name_filter":{"type":"string","description":"Case-insensitive fragment of the actor label"},
                "selected_only":{"type":"boolean"},
                "limit":{"type":"integer","minimum":1,"maximum":5000,"description":"Default 200"}
            }), &[], true),
        tool("ue_console",
            "Run a console command in the editor world, e.g. 'stat unit', 'r.ScreenPercentage 75', 'obj list class=StaticMesh'. Output goes to the log; read it with ue_log.",
            json!({"command":{"type":"string"}}), &["command"], false),
        tool("ue_screenshot",
            "Look at the level: render PNG images you can see. Frame one or more actors from named views (front/back/left/right/top/three_quarter, relative to the first actor's facing), or use an explicit camera, or the current editor viewport when neither is given. isolate=true hides the level actors around them. coverage=true also reports how much of each image the subject covers (0 = it did not render).",
            json!({
                "actors":{"type":"array","items":{"type":"string"},"description":"Actor labels or paths to frame"},
                "views":{"type":"array","items":{"type":"string","enum":["front","back","left","right","top","three_quarter","three_quarter_left"]},"description":"Default front, right, three_quarter"},
                "camera":{"type":"object","properties":{"location":{"type":"array","items":{"type":"number"}},"rotation":{"type":"array","items":{"type":"number"},"description":"[pitch, yaw, roll]"}}},
                "forward":{"type":"array","items":{"type":"number"},"description":"Override the facing used for named views"},
                "isolate":{"type":"boolean"},
                "coverage":{"type":"boolean"},
                "width":{"type":"integer","minimum":64,"maximum":1920},
                "height":{"type":"integer","minimum":64,"maximum":1080},
                "fov":{"type":"number","minimum":10,"maximum":120}
            }), &[], false),
        tool("ue_anim_inspect",
            "Measure an animation instead of eyeballing it. Poses a skeletal mesh at sample times straight from the animation data and reports, in the character's own frame ([forward, right, up] cm, left/right detected from the skeleton's bone pairs): where tracked bones, sockets and attached items are and which side they are on; grip distances between item sockets and hands; clearance of attached items to the body and of hands, feet, head and items to a partner character; expected contacts (touch or stay apart); feet below ground. Works for any skeleton and any item (weapon, tool, shield, prop, bag, instrument). Returns problems[] and passed.",
            json!({
                "mesh":{"type":"string","description":"Skeletal mesh asset path"},
                "animation":{"type":"string","description":"Animation sequence path; omit for the reference pose"},
                "times":{"type":"array","items":{"type":"number"}},
                "samples":{"type":"integer","minimum":2,"maximum":60,"description":"Evenly spaced samples when times is omitted, default 9"},
                "track":{"type":"array","items":{"type":"string"},"description":"Points to report: bone or socket, partner:<bone>, item:<name>:<socket|end_a|end_b|origin|center>"},
                "attachments":{"type":"array","items":{"type":"object","properties":{
                    "name":{"type":"string"},"mesh":{"type":"string","description":"Static or skeletal mesh of the item"},
                    "socket":{"type":"string","description":"Character bone or socket it hangs from"},
                    "location":{"type":"array","items":{"type":"number"}},"rotation":{"type":"array","items":{"type":"number"},"description":"[pitch, yaw, roll] offset"},
                    "grips":{"type":"array","items":{"type":"object","properties":{"socket":{"type":"string"},"bone":{"type":"string"},"tolerance":{"type":"number"}}}}
                },"required":["name","mesh","socket"]}},
                "partner":{"type":"object","properties":{
                    "mesh":{"type":"string"},"animation":{"type":"string"},
                    "location":{"type":"array","items":{"type":"number"},"description":"In this character's mesh space, cm"},
                    "yaw":{"type":"number","description":"Degrees, default 180 (facing back)"},
                    "time_offset":{"type":"number"}
                },"required":["mesh"]},
                "contacts":{"type":"array","items":{"type":"object","properties":{
                    "a":{"type":"string"},"b":{"type":"string"},"expect":{"type":"string","enum":["touch","apart"]},
                    "distance":{"type":"number"},"window":{"type":"array","items":{"type":"number"},"description":"[start, end] seconds"}
                },"required":["a","b"]}},
                "body_radius":{"type":"number","description":"Body thickness around bones for clipping, default 8 cm"},
                "touch_distance":{"type":"number","description":"Default 5 cm"}
            }), &["mesh"], true),
        tool("ue_anim_preview",
            "See an animation: spawns a temporary copy of the character (with attached items and an optional partner, same arguments as ue_anim_inspect) in the open level, poses it at each sample time and returns images from the chosen views. The preview actors are transient (never saved, and on engines that support it they leave the level unmodified) and are removed afterwards.",
            json!({
                "mesh":{"type":"string"},"animation":{"type":"string"},
                "times":{"type":"array","items":{"type":"number"}},
                "samples":{"type":"integer","minimum":1,"maximum":8,"description":"Default 4"},
                "attachments":{"type":"array","items":{"type":"object"}},
                "partner":{"type":"object"},
                "views":{"type":"array","items":{"type":"string"},"description":"Default front and right"},
                "location":{"type":"array","items":{"type":"number"},"description":"Where to spawn (the camera follows the preview wherever it is)"},
                "isolate":{"type":"boolean","description":"Hide level actors near the preview (default false: the preview spawns 500 m above the editor camera, clear of the level)"},
                "altitude":{"type":"number","description":"Height above the editor camera to spawn at when no location is given, default 50000 cm"},
                "settle_ms":{"type":"integer","minimum":50,"maximum":5000,"description":"Wait for the editor to apply each pose, default 400"},
                "width":{"type":"integer","minimum":64,"maximum":1280},"height":{"type":"integer","minimum":64,"maximum":1080}
            }), &["mesh"], false),
        tool("ue_play",
            "Play the game in the editor and watch it: starts Play In Editor (or Simulate), waits for it to run, takes in-game screenshots and runs your Python probe (with `world` = the game world) at checkpoints, then stops and returns the screenshots, probe output and the log lines (errors, warnings, ensures, 'Accessed None') produced while playing. Use it to check gameplay, runtime IK and anything that only happens at runtime.",
            json!({
                "mode":{"type":"string","enum":["pie","simulate"],"description":"Default pie"},
                "seconds":{"type":"number","minimum":1,"maximum":300,"description":"How long to play, default 5"},
                "checkpoints":{"type":"array","items":{"type":"number"},"description":"Seconds after start to screenshot and probe; default the end"},
                "screenshots":{"type":"boolean","description":"Default true"},
                "width":{"type":"integer"},"height":{"type":"integer"},
                "console":{"type":"array","items":{"type":"string"},"description":"Console commands to run right after play starts, e.g. cheats or 'slomo 0.5'"},
                "probe":{"type":"string","description":"Python run at each checkpoint with `unreal` and `world` (the game world); print what you want to see"},
                "log_filter":{"type":"string","description":"Regex over new log lines; default errors, warnings, ensures and Blueprint runtime errors"},
                "stop":{"type":"boolean","description":"Stop at the end (default true)"},
                "stop_existing":{"type":"boolean","description":"End a session that is already running first"},
                "outside":{"type":"object","description":"Also render the game from outside the player's camera at each checkpoint: first-person arms and guns seen from the side or front. A capture placed in the level before play follows the target; 'only owner see' parts are shown for the capture.","properties":{
                    "target":{"type":"string","description":"'player' (default: the player's pawn), or an actor label, name or class fragment"},
                    "views":{"type":"array","items":{"type":"string","enum":["front","back","left","right","top","three_quarter","three_quarter_left"]},"description":"Default right and front"},
                    "offset":{"type":"array","items":{"type":"number"},"description":"Camera at [forward, right, up] cm from the look-at point, in the target's frame"},
                    "look_at":{"type":"array","items":{"type":"number"},"description":"Point looked at, [forward, right, up] cm from the target's origin; default [30, 0, 50], where first-person hands and guns sit"},
                    "distance":{"type":"number","description":"For named views, default 150 cm"},
                    "fov":{"type":"number"},"width":{"type":"integer"},"height":{"type":"integer"}}}
            }), &[], false),
        tool("ue_blueprint_info",
            "Read Blueprints as text: parent class, interfaces, variables with default values, functions and events, components, and graph nodes where this engine version exposes them. compile=true compiles them and returns the compiler's log lines. Give paths, or a folder to read every Blueprint in it.",
            json!({
                "paths":{"type":"array","items":{"type":"string"}},
                "folder":{"type":"string"},
                "compile":{"type":"boolean"},
                "limit":{"type":"integer","minimum":1,"maximum":500,"description":"Default 50"}
            }), &[], false),
        tool("ue_asset_audit",
            "Find asset problems with facts: textures (not power of two, oversized, normal maps with the wrong compression, masks in sRGB, Never Stream, no mips), static meshes (no collision, dense without Nanite or LODs), references to missing assets, and redirectors. Each finding says how to fix it.",
            json!({
                "path":{"type":"string","description":"Folder, default /Game"},
                "checks":{"type":"array","items":{"type":"string","enum":["textures","meshes","references","redirectors"]}},
                "limit":{"type":"integer","minimum":1,"maximum":5000,"description":"Assets loaded per class, default 400"}
            }), &[], true),
        tool("ue_asset_refs",
            "What an asset depends on and what depends on it (up to 4 levels). Run it before renaming, moving or deleting anything.",
            json!({"path":{"type":"string"},"depth":{"type":"integer","minimum":1,"maximum":4}}), &["path"], true),
        tool("ue_data_table",
            "Data Tables as text. export writes the table to a CSV or JSON file in the checkout (and returns it); import fills the table from such a file and saves the asset. Keeps balancing, dialogue and loot data diffable.",
            json!({
                "action":{"type":"string","enum":["export","import"]},
                "path":{"type":"string","description":"Data Table asset path"},
                "file":{"type":"string","description":"File in the checkout, relative to the project root"},
                "format":{"type":"string","enum":["csv","json"]}
            }), &["action","path"], false),
        tool("ue_profile",
            "Measure performance: plays (or uses the running editor world with play=false), records Unreal's CSV profiler for the given seconds, and returns average, 95th percentile and worst frame, game thread, render thread and GPU times, plus the slowest frames.",
            json!({
                "seconds":{"type":"number","minimum":2,"maximum":120,"description":"Default 10"},
                "warmup":{"type":"number","minimum":0,"maximum":60,"description":"Seconds before recording, default 3"},
                "play":{"type":"boolean","description":"Start Play In Editor first, default true"},
                "console":{"type":"array","items":{"type":"string"}}
            }), &[], false),
        tool("ue_run_tests",
            "Run automation tests and return pass/fail per test with error messages. Headless by default (a separate editor process with no window; works while your editor is open); in_editor=true runs them in the open editor instead.",
            json!({
                "filter":{"type":"string","description":"Test path prefix, e.g. Project. or MyGame.Inventory"},
                "in_editor":{"type":"boolean"},
                "timeout_s":{"type":"integer","minimum":30,"maximum":7200}
            }), &["filter"], false),
        tool("ue_crash",
            "The most recent crash: error message, call stack and the last log lines, from Saved/Crashes.",
            json!({"index":{"type":"integer","minimum":0,"description":"0 = most recent"}}), &[], true),
        tool("ue_editor_launch",
            "Start the Unreal editor on this checkout's project with the Remote Control server enabled: waits until the port is free (a closed editor holds it for a while), launches, and waits until Remote Control answers, reporting a failed bind from the new log. Returns when the editor is ready.",
            json!({"timeout_s":{"type":"integer","minimum":30,"maximum":3600,"description":"Default 900; first starts compile shaders"},"extra_args":{"type":"array","items":{"type":"string"}},"keep_background_throttle":{"type":"boolean","description":"Leave 'Use Less CPU when in Background' as configured (default: off for this session)"}}), &[], false),
        tool("ue_editor_quit",
            "Quit the editor cleanly (saving dirty packages unless save=false), wait for the process to exit and for the Remote Control port to be released. Refuses, leaving the editor running, when a package cannot be saved or when the editor does not answer Remote Control (nothing could be saved); force=true then terminates it without saving. Use before builds; ue_build restart_editor=true does quit, build and relaunch in one call.",
            json!({"save":{"type":"boolean"},"force":{"type":"boolean","description":"Terminate an editor that does not answer Remote Control, losing its unsaved work. Only when the human has said so."}}), &[], false),
        tool("ue_editor_lock",
            "Who is driving the editor. Live tools that change the editor take this lock automatically, so two agents never script the one editor at once; it frees itself after 15 idle minutes. action=release gives it up when you are done.",
            json!({"action":{"type":"string","enum":["status","release"]}}), &[], false),
    ]
}

fn call(name: &str, args: &Value) -> Result<Value> {
    // Tools that run agent-supplied code or commands meet the guardrail first (RA-077; see
    // mcp::plugin_gate for what that can and cannot cover).
    if matches!(name, "ue_python" | "ue_console" | "ue_call" | "ue_play" | "ue_profile") {
        crate::mcp::plugin_gate(name, None, &[])?;
    }
    // Held, and kept fresh, until the call returns.
    let mut _hold = None;
    if LIVE.contains(&name) {
        let project = Project::find()?;
        guard_project(&project)?;
        let writes = MUTATING.contains(&name)
            && (name != "ue_property" || args.get("value").is_some())
            && (name != "ue_data_table" || args["action"] == "import")
            && (name != "ue_blueprint_info" || args["compile"] == true);
        if writes {
            _hold = Some(LockHold::take(&project, &holder_id())?);
        }
    }
    match name {
        "ue_project_info" => project_info(&Project::find()?),
        "ue_setup_check" => setup_check(&Project::find()?, args["fix"].as_bool().unwrap_or(false)),
        "ue_build" => build(&Project::find()?, args),
        "ue_log" => log(&Project::find()?, args),
        "ue_editor_status" => editor_status(),
        "ue_python" => {
            let code = args["code"].as_str().ok_or_else(|| anyhow!("code is required"))?;
            python(code, secs(args, "timeout_s", 120))
        }
        "ue_call" => {
            let mut body = json!({
                "objectPath": required(args, "object_path")?,
                "functionName": required(args, "function")?,
                "generateTransaction": args["transaction"].as_bool().unwrap_or(true),
            });
            if let Some(parameters) = args.get("parameters").filter(|p| p.is_object()) {
                body["parameters"] = parameters.clone();
            }
            remote("PUT", "/remote/object/call", Some(&body), Duration::from_secs(120)).map(capped)
        }
        "ue_property" => {
            let property = required(args, "property")?;
            let mut body = json!({"objectPath": required(args, "object_path")?, "propertyName": property});
            match args.get("value") {
                Some(value) => {
                    body["access"] = json!("WRITE_TRANSACTION_ACCESS");
                    body["propertyValue"] = json!({ property: value });
                }
                None => body["access"] = json!("READ_ACCESS"),
            }
            remote("PUT", "/remote/object/property", Some(&body), Duration::from_secs(30)).map(capped)
        }
        "ue_search_assets" => python_json(&search_assets_script(args), Duration::from_secs(120)),
        "ue_level_actors" => python_json(&level_actors_script(args), Duration::from_secs(60)),
        "ue_console" => {
            let command = required(args, "command")?;
            let code = format!(
                "import unreal\nworld = unreal.get_editor_subsystem(unreal.UnrealEditorSubsystem).get_editor_world()\nunreal.SystemLibrary.execute_console_command(world, {})\nprint('ran', {})\n",
                py_str(&command), py_str(&command)
            );
            python(&code, Duration::from_secs(60))
        }
        "ue_screenshot" => {
            let project = Project::find()?;
            let dir = capture_dir(&project)?;
            let mut script_args = args.clone();
            script_args["out_dir"] = json!(dir);
            script_args["prefix"] = json!("shot");
            let result = python_json(&script(PY_CAPTURE, &script_args), Duration::from_secs(120));
            with_images(result, &dir)
        }
        "ue_anim_inspect" => python_json(&script(PY_ANIM_INSPECT, args), Duration::from_secs(300)),
        "ue_anim_preview" => anim_preview(&Project::find()?, args),
        "ue_play" => play(&Project::find()?, args),
        "ue_editor_launch" => launch_editor(&Project::find()?, args),
        "ue_editor_quit" => quit_editor(&Project::find()?, args["save"].as_bool().unwrap_or(true), args["force"] == true),
        "ue_blueprint_info" => {
            let project = Project::find()?;
            let start = log_len(&project);
            let mut value = python_json(&script(PY_BLUEPRINT_INFO, args), Duration::from_secs(600))?;
            if args["compile"] == true {
                value["compiler_log"] = json!(cap_lines(log_since(&project, start, Some(r"LogBlueprint|Error|Warning")), MAX_LOG_LINES));
            }
            Ok(value)
        }
        "ue_asset_audit" => python_json(&script(PY_ASSET_AUDIT, args), Duration::from_secs(1800)),
        "ue_asset_refs" => python_json(&script(PY_ASSET_REFS, args), Duration::from_secs(120)),
        "ue_data_table" => data_table(&Project::find()?, args),
        "ue_profile" => profile(&Project::find()?, args),
        "ue_run_tests" => run_tests(&Project::find()?, args),
        "ue_crash" => crash(&Project::find()?, args["index"].as_u64().unwrap_or(0) as usize),
        "ue_editor_lock" => {
            let project = Project::find()?;
            if args["action"].as_str() == Some("release") {
                release_lock(&project, &holder_id())?;
            }
            Ok(json!({"lock": read_lock(&project), "you": holder_id()}))
        }
        other => bail!("unknown tool {other}"),
    }
}

/// What `import_fbx` will need, asked before the Blender export that precedes it, which can take
/// minutes (RA-588): this checkout's project, the editor open on it, and no other agent holding
/// the editor. Only advisory for the lock, which can change during the export; `import_fbx`
/// still takes it.
pub(crate) fn import_preflight() -> Result<()> {
    let project = Project::find()?;
    guard_project(&project)?;
    lock_free_for(&read_lock(&project), &holder_id())
}

/// Import an FBX into the editor's project and measure what arrived (used by the Blender
/// plugin's `blender_to_unreal`). Same project guard and editor lock as every live tool.
pub(crate) fn import_fbx(fbx: &Path, mut args: Value) -> Result<Value> {
    let project = Project::find()?;
    guard_project(&project)?;
    let _hold = LockHold::take(&project, &holder_id())?;
    args["fbx"] = json!(fbx);
    let start = log_len(&project);
    let import_log = |project: &Project| cap_lines(log_since(project, start, Some(r"LogFbx|Interchange|Error|Warning")), MAX_LOG_LINES);
    let mut value = match python_json(&script(PY_IMPORT_FBX, &args), Duration::from_secs(900)) {
        Ok(value) => value,
        Err(error) => bail!("{error:#}\n{}", import_log(&project).join("\n")),
    };
    // Does it draw? An invisible mesh (it happened: only its shadow rendered) passes every
    // size and axis check.
    let mut renders = Vec::new();
    let dir = capture_dir(&project)?;
    for (n, asset) in value["imported"].as_array().cloned().unwrap_or_default().into_iter().enumerate() {
        if !matches!(asset["class"].as_str(), Some("StaticMesh") | Some("SkeletalMesh")) {
            continue;
        }
        let shot = (|| -> Result<Value> {
            let spawned = python_json(&script(PY_PREVIEW_ASSET, &json!({"action": "spawn", "path": asset["path"]})), Duration::from_secs(60))?;
            std::thread::sleep(Duration::from_millis(300));
            python_json(&script(PY_CAPTURE, &json!({
                "actors": [spawned["actor"]], "center": spawned["center"], "radius": spawned["radius"],
                "forward": [0.0, 1.0, 0.0], "views": ["front", "three_quarter"], "coverage": true,
                "width": 320, "height": 320, "out_dir": dir, "prefix": format!("import{n}"),
            })), Duration::from_secs(120))
        })();
        let _ = python_json(&script(PY_PREVIEW_ASSET, &json!({"action": "cleanup"})), Duration::from_secs(60));
        match shot {
            Ok(shot) => {
                let files = shot["files"].as_array().cloned().unwrap_or_default();
                let coverage: Vec<Value> = files.iter().map(|x| x["coverage"].clone()).collect();
                let mut check = json!({"path": asset["path"], "coverage": coverage, "renders": renders_from_coverage(&coverage), "files": shot["files"]});
                if check["renders"].is_null() {
                    let why: Vec<Value> = files.iter().filter_map(|x| x.get("coverage_error").cloned()).collect();
                    check["not_verified"] = json!(format!("the render check could not read the captures back, so whether the mesh draws is unknown: {why:?}"));
                }
                renders.push(check);
            }
            Err(error) => renders.push(json!({"path": asset["path"], "error": format!("{error:#}")})),
        }
    }
    value["_images"] = json!(renders.iter().flat_map(|r| r["files"].as_array().cloned().unwrap_or_default())
        .map(|f| json!({"label": format!("imported: {}", f["view"].as_str().unwrap_or("")), "path": f["file"]})).collect::<Vec<_>>());
    value["_cleanup"] = json!(dir);
    value["render_check"] = json!(renders);
    value["import_log"] = json!(import_log(&project));
    Ok(value)
}

/// `ARGS_JSON` first, then the shared helpers, then the tool's own script.
fn script(body: &str, args: &Value) -> String {
    format!("ARGS_JSON = {}\n{PY_COMMON}\n{body}", py_str(&args.to_string()))
}

/// Whether a mesh draws, from the coverage of its views: true when any view shows it, false when
/// every view was measured and none does, null when no view could be measured. A failed read
/// is not evidence that it draws.
fn renders_from_coverage(coverage: &[Value]) -> Value {
    let known: Vec<f64> = coverage.iter().filter_map(Value::as_f64).collect();
    if known.iter().any(|c| *c > 0.002) {
        json!(true)
    } else if !known.is_empty() && known.len() == coverage.len() {
        json!(false)
    } else {
        Value::Null
    }
}

// ---------------------------------------------------------------- which editor, and who drives it

/// Refuse live tools when the editor has another project open. The common case is an agent in
/// a git worktree while the human's editor is on the main checkout: its C++ would land in one
/// copy and its asset edits in the other.
fn guard_project(project: &Project) -> Result<()> {
    if std::env::var("UE_ALLOW_PROJECT_MISMATCH").is_ok_and(|v| v == "1") {
        return Ok(());
    }
    let open = editor_project()?;
    if same_file(&open, &project.uproject) {
        return Ok(());
    }
    bail!(
        "the editor has {} open, but this agent works in {}. Changes made through the editor would land in a different copy of the project than your files. Ask the human to start this agent in the project's main checkout (the Unreal plugin does that by default for new agents), or to open this checkout's .uproject in the editor. UE_ALLOW_PROJECT_MISMATCH=1 overrides this check.",
        open.display(), project.uproject.display()
    )
}

fn editor_project() -> Result<PathBuf> {
    use std::sync::Mutex;
    static CACHE: Mutex<Option<(std::time::Instant, PathBuf)>> = Mutex::new(None);
    if let Some((at, path)) = CACHE.lock().unwrap().clone() {
        if at.elapsed() < Duration::from_secs(30) {
            return Ok(path);
        }
    }
    let result = python_json(&script(PY_PROJECT_CHECK, &json!({})), Duration::from_secs(20))?;
    let path = PathBuf::from(result["project"].as_str().ok_or_else(|| anyhow!("the editor reported no project"))?);
    *CACHE.lock().unwrap() = Some((std::time::Instant::now(), path.clone()));
    Ok(path)
}

fn same_file(a: &Path, b: &Path) -> bool {
    match (std::fs::canonicalize(a), std::fs::canonicalize(b)) {
        (Ok(a), Ok(b)) => a == b,
        _ => a == b,
    }
}

fn holder_id() -> String {
    std::env::var("RELAY_SESSION").ok().filter(|s| !s.is_empty()).unwrap_or_else(|| format!("pid {}", std::os::unix::process::parent_id()))
}

fn lock_path(project: &Project) -> PathBuf {
    project.root.join("Saved/Relay/editor-lock.json")
}

fn now_secs() -> u64 {
    std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_secs()).unwrap_or(0)
}

fn read_lock(project: &Project) -> Value {
    read_lock_at(&lock_path(project))
}

/// Every read-modify-write of the lock file runs holding an OS lock on a file beside it, so two
/// agents that both find the editor free cannot both take it (RA-285).
fn with_lock_file<T>(path: &Path, f: impl FnOnce() -> Result<T>) -> Result<T> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    let guard = std::fs::OpenOptions::new().create(true).truncate(false).write(true).open(path.with_extension("lock"))?;
    guard.lock()?;
    f()
}

/// Written beside it and renamed into place: a reader never sees a half-written lock as free.
fn write_lock_at(path: &Path, lock: &Value) -> Result<()> {
    let tmp = path.with_extension("json.tmp");
    std::fs::write(&tmp, lock.to_string())?;
    std::fs::rename(&tmp, path)?;
    Ok(())
}

fn read_lock_at(path: &Path) -> Value {
    let Ok(raw) = std::fs::read_to_string(path) else { return Value::Null };
    let mut lock: Value = serde_json::from_str(&raw).unwrap_or(Value::Null);
    if let Some(last) = lock["last_used"].as_u64() {
        let idle = now_secs().saturating_sub(last);
        lock["idle_s"] = json!(idle);
        lock["expired"] = json!(idle >= LOCK_IDLE.as_secs());
    }
    lock
}

/// One agent drives the editor at a time. The lock is a file in the project's `Saved/`, which
/// every agent sharing the checkout sees and git ignores.
fn acquire_lock(project: &Project, me: &str) -> Result<()> {
    with_lock_file(&lock_path(project), || acquire_lock_locked(project, me))
}

/// Refuses when someone other than `me` holds `lock` and it has not expired.
fn lock_free_for(lock: &Value, me: &str) -> Result<()> {
    if let Some(holder) = lock["holder"].as_str() {
        if holder != me && lock["expired"] != true {
            bail!(
                "the editor is being driven by {holder} (idle {}s). Do offline work (C++, config, data files) or wait; the lock frees itself after {} idle minutes, or when {holder} runs ue_editor_lock with action=release.",
                lock["idle_s"].as_u64().unwrap_or(0), LOCK_IDLE.as_secs() / 60
            );
        }
    }
    Ok(())
}

fn acquire_lock_locked(project: &Project, me: &str) -> Result<()> {
    let lock = read_lock(project);
    lock_free_for(&lock, me)?;
    let since = if lock["holder"].as_str() == Some(me) { lock["since"].as_u64().unwrap_or(now_secs()) } else { now_secs() };
    write_lock_at(&lock_path(project), &json!({"holder": me, "since": since, "last_used": now_secs()}))
}

/// Move the idle clock to now, if `me` still holds the lock. A lock released or taken over in
/// the meantime is left alone.
fn touch_lock(path: &Path, me: &str) {
    let _ = with_lock_file(path, || {
        let lock = read_lock_at(path);
        if lock["holder"].as_str() == Some(me) {
            let since = lock["since"].as_u64().unwrap_or(now_secs());
            write_lock_at(path, &json!({"holder": me, "since": since, "last_used": now_secs()}))?;
        }
        Ok(())
    });
}

/// The editor lock, held for the length of one call. The idle clock otherwise moves only when a
/// call starts, so an in-editor test run or a restart build longer than `LOCK_IDLE` would look
/// abandoned while it is still running. A thread refreshes it until the hold is dropped, and
/// the drop refreshes it once more, so idleness counts from the end of the call.
struct LockHold {
    stop: Option<std::sync::mpsc::Sender<()>>,
    beat: Option<std::thread::JoinHandle<()>>,
    path: PathBuf,
    me: String,
}

impl LockHold {
    fn take(project: &Project, me: &str) -> Result<LockHold> {
        LockHold::take_every(project, me, LOCK_HEARTBEAT)
    }

    fn take_every(project: &Project, me: &str, every: Duration) -> Result<LockHold> {
        acquire_lock(project, me)?;
        let (stop, stopped) = std::sync::mpsc::channel::<()>();
        let (path, holder) = (lock_path(project), me.to_string());
        let beat = std::thread::spawn(move || {
            while let Err(std::sync::mpsc::RecvTimeoutError::Timeout) = stopped.recv_timeout(every) {
                touch_lock(&path, &holder);
            }
        });
        Ok(LockHold { stop: Some(stop), beat: Some(beat), path: lock_path(project), me: me.to_string() })
    }
}

impl Drop for LockHold {
    fn drop(&mut self) {
        drop(self.stop.take());
        if let Some(beat) = self.beat.take() {
            let _ = beat.join();
        }
        touch_lock(&self.path, &self.me);
    }
}

fn release_lock(project: &Project, me: &str) -> Result<()> {
    with_lock_file(&lock_path(project), || {
        let lock = read_lock(project);
        match lock["holder"].as_str() {
            Some(holder) if holder != me && lock["expired"] != true => bail!("the lock belongs to {holder}, not to you"),
            Some(_) => {
                std::fs::remove_file(lock_path(project))?;
                Ok(())
            }
            None => Ok(()),
        }
    })
}

// ---------------------------------------------------------------- playing, testing, measuring

// ---------------------------------------------------------------- the editor process

/// Save (optionally), ask the editor to quit, and wait until the process has gone and the
/// Remote Control port is free again, so the next launch can bind it. An editor that cannot be
/// asked is never signalled without `force`: whatever it has not saved would go with it.
fn quit_editor(project: &Project, save: bool, force: bool) -> Result<Value> {
    let hold = LockHold::take(project, &holder_id())?;
    let result = quit_editor_locked(project, save, force);
    drop(hold);
    let _ = release_lock(project, &holder_id());
    result
}

fn quit_editor_locked(project: &Project, save: bool, force: bool) -> Result<Value> {
    let started = std::time::Instant::now();
    let round = |d: Duration| (d.as_secs_f64() * 10.0).round() / 10.0;
    let port = crate::unreal_process::port_of(&remote_base());
    let before: Vec<u32> = crate::unreal_process::editors_for(&project.uproject).iter().map(|p| p.pid).collect();
    if before.is_empty() {
        return Ok(json!({"stopped": [], "saved": false, "port_free": crate::unreal_process::port_free(port),
            "notes": ["no editor is running this project"], "seconds": round(started.elapsed())}));
    }
    // A game thread busy loading a map or compiling misses a single short probe.
    let reachable = (0..3).any(|attempt| {
        if attempt > 0 {
            std::thread::sleep(Duration::from_secs(2));
        }
        remote("GET", "/remote/info", None, Duration::from_secs(3)).is_ok()
    });
    let mut notes = Vec::new();
    let mut saved = json!(false);
    // A lingering editor may be signalled only once it has saved and been asked to quit, or
    // when the caller accepted losing its work.
    let mut may_signal = force;
    if reachable {
        guard_project(project)?;
        let code = if save {
            // save_dirty_packages runs without a dialog and silently skips what it cannot save
            // (an untitled map, a read-only file), so ask again what is still dirty.
            // Maps ue_play's capture camera dirtied (play.py notes the ones that were clean
            // before) are named, so a rewrite of an unchanged map can be reverted.
            "import unreal, sys\n\
             L = unreal.EditorLoadingAndSavingUtils\n\
             maps = [p.get_name() for p in list(getattr(L, 'get_dirty_map_packages', list)())]\n\
             helper = sorted(set(maps) & set(getattr(sys.modules.get('_relay_state'), 'maps_clean_before_relay', ())))\n\
             ok = L.save_dirty_packages(True, True)\n\
             dirty = [p.get_name() for p in list(getattr(L, 'get_dirty_map_packages', list)()) + list(getattr(L, 'get_dirty_content_packages', list)())]\n\
             if ok and not dirty:\n    if helper:\n        print('RELAY_HELPER_MAPS:' + ', '.join(helper))\n    unreal.SystemLibrary.quit_editor()\n    print('quitting')\n\
             else:\n    print('RELAY_UNSAVED:' + (', '.join(dirty) or 'packages the editor would not save'))\n"
        } else {
            "import unreal\nunreal.SystemLibrary.quit_editor()\nprint('quitting')\n"
        };
        match python_tx(code, Duration::from_secs(20), false) {
            Ok(result) => {
                let output = result["output"].as_str().unwrap_or("");
                if let Some(dirty) = output.lines().find_map(|l| l.trim().strip_prefix("RELAY_UNSAVED:")) {
                    bail!("the editor could not save {dirty}, so it was not asked to quit and is still running. Ask the human to save or discard those in the editor, or quit with save=false to lose them.");
                }
                if let Some(maps) = output.lines().find_map(|l| l.trim().strip_prefix("RELAY_HELPER_MAPS:")) {
                    notes.push(format!("saved {maps}, which had no unsaved changes before ue_play placed its outside-capture camera there. If nobody edited them since, the save only rewrote them: check git status and restore them if the diff is unintended."));
                }
                saved = json!(save);
                may_signal = true;
            }
            // The editor answered and the script failed before quitting: nothing is closing.
            Err(error) if format!("{error:#}").starts_with("Python failed") => {
                bail!("the quit script failed, so the editor was left running: {error:#}");
            }
            // It stops answering as it shuts down, and a long save outlasts the reply timeout.
            // Either way, wait for it rather than signal it.
            Err(error) => {
                saved = Value::Null;
                notes.push(format!("quit request: {error:#}; waiting for the editor to exit"));
            }
        }
    } else if force {
        notes.push("the editor did not answer Remote Control; terminating it as asked with force=true (unsaved changes are lost)".to_string());
    } else {
        bail!(
            "the editor (pid {before:?}) did not answer Remote Control for about 10 s, so nothing could be saved, and it was left running rather than terminated with its unsaved work. It may be busy (loading, compiling) or its web server may be down: try again shortly, check ue_editor_status, or ask the human to save and close it. force=true terminates it without saving."
        );
    }
    let requested = started.elapsed();
    let deadline = std::time::Instant::now() + Duration::from_secs(90);
    let mut signalled = None;
    loop {
        // Re-read each time: a PID only stays a target while it is still this project's editor.
        let left: Vec<u32> = crate::unreal_process::editors_for(&project.uproject).iter().map(|p| p.pid).filter(|pid| before.contains(pid)).collect();
        if left.is_empty() {
            break;
        }
        if std::time::Instant::now() >= deadline {
            if may_signal {
                bail!("the editor is still running after 90 s; close it by hand");
            }
            bail!("the editor has not exited after 90 s and may still be saving; it was not signalled. Check on it, or ask the human to close it.");
        }
        // Saved and asked: an editor that lingers in shutdown gets a terminate signal (a normal
        // shutdown request) rather than a long wait. Forced and unreachable: at once.
        let lingering = !reachable || started.elapsed() > requested + Duration::from_secs(20);
        if signalled.is_none() && may_signal && lingering {
            for pid in &left {
                if crate::unreal_process::still_editor_for(*pid, &project.uproject) {
                    crate::unreal_process::kill(*pid);
                }
            }
            signalled = Some(round(started.elapsed()));
        }
        std::thread::sleep(Duration::from_millis(250));
    }
    let exited = started.elapsed();
    let port_free = crate::unreal_process::wait_port_free(port, Duration::from_secs(90));
    Ok(json!({
        "stopped": before, "saved": saved, "port_free": port_free, "notes": notes,
        "seconds": round(started.elapsed()),
        // Where the time went, so a slow quit can be told apart from a slow port.
        "timing": {"save_and_request_s": round(requested), "terminate_signal_at_s": signalled, "exited_at_s": round(exited), "port_free_at_s": round(started.elapsed())},
    }))
}

/// Launch the editor on this checkout's project once the port is free, then wait until Remote
/// Control answers, watching the new log for a failed bind.
fn launch_editor(project: &Project, args: &Value) -> Result<Value> {
    let running = crate::unreal_process::editors_for(&project.uproject);
    if !running.is_empty() {
        return Ok(json!({"already_running": running.iter().map(|p| p.pid).collect::<Vec<_>>(), "status": editor_status()?}));
    }
    let _hold = LockHold::take(project, &holder_id())?;
    let engine = engine_root(project)?;
    let port = crate::unreal_process::port_of(&remote_base());
    let started = std::time::Instant::now();
    let extra: Vec<String> = args["extra_args"].as_array().map(|a| a.iter().filter_map(|v| v.as_str().map(str::to_string)).collect()).unwrap_or_default();
    let mut extra = extra;
    // "Use Less CPU when in Background" comes back on every start, and a throttled editor runs
    // agent play sessions at a few frames per second. Write it off in the saved per-project user
    // settings (the editor is not running, so nothing overwrites it) and override it on the
    // command line as well.
    if args["keep_background_throttle"] != true {
        let platform_dir = if cfg!(target_os = "windows") { "WindowsEditor" } else if cfg!(target_os = "macos") { "MacEditor" } else { "LinuxEditor" };
        let ini = project.root.join("Saved/Config").join(platform_dir).join("EditorPerProjectUserSettings.ini");
        // A file it cannot read is left alone; the command line below still turns it off.
        let _ = merge_ini_file(&ini, PERF_SECTION, &[(PERF_KEY.to_string(), "False".to_string())]);
        extra.push("-ini:EditorPerProjectUserSettings:[/Script/UnrealEd.EditorPerformanceSettings]:bThrottleCPUWhenNotForeground=False".to_string());
    }
    // The log as it stood before the launch: until the new editor starts its own file, what is
    // on disk is the previous session's, and its bind failures are not this one's.
    let mark = log_mark(project);
    let pid = crate::unreal_process::launch(&engine, &project.uproject, port, &extra)?;
    let timeout = Duration::from_secs(args["timeout_s"].as_u64().unwrap_or(900).clamp(30, 3600));
    loop {
        if remote("GET", "/remote/info", None, Duration::from_secs(3)).is_ok() {
            return Ok(json!({"pid": pid, "reachable": true, "seconds": started.elapsed().as_secs(), "engine": engine, "args": extra}));
        }
        // Every line since the launch that could be one, not a tail: the failure is logged
        // once, early, and later start-up logging would push it out of any window.
        let lines = log_since(project, new_session_start(project, mark), Some(BIND_PREFILTER));
        let bind = crate::unreal_process::bind_failures(&lines);
        if !bind.is_empty() {
            bail!("the editor started (pid {pid}) but its web server could not bind port {port}: {bind:?}. In the editor console run `WebControl.StopServer` then `WebControl.StartServer`.");
        }
        if crate::unreal_process::editors_for(&project.uproject).is_empty() && started.elapsed() > Duration::from_secs(20) {
            bail!("the editor exited during start-up; read ue_log and ue_crash");
        }
        if started.elapsed() > timeout {
            bail!("the editor (pid {pid}) did not answer on Remote Control within {} s (first starts compile shaders and can take long; raise timeout_s). Check ue_setup_check.", timeout.as_secs());
        }
        std::thread::sleep(Duration::from_secs(2));
    }
}

/// `Config/DefaultRemoteControl.ini` settings the bridge needs, merged into the project's file.
/// Keys are checked against the engine's RemoteControlSettings.h when it can be found, so a
/// key an engine version does not have is reported instead of written blindly. Remote console
/// commands are not among them: ue_console runs its command through Python (RA-550).
const RC_SECTION: &str = "[/Script/RemoteControlCommon.RemoteControlSettings]";
const RC_KEYS: [(&str, &str); 3] = [
    ("bAutoStartWebServer", "True"),
    ("bEnableRemotePythonExecution", "True"),
    ("bAllowAnyRemoteFunctionCall", "True"),
];

fn rc_header(engine: &Path) -> Option<String> {
    let direct = engine.join("Engine/Plugins/VirtualProduction/RemoteControl/Source/RemoteControlCommon/Public/RemoteControlSettings.h");
    if let Ok(text) = std::fs::read_to_string(&direct) {
        return Some(text);
    }
    fn find(dir: &Path, depth: usize) -> Option<PathBuf> {
        if depth > 7 { return None; }
        for e in std::fs::read_dir(dir).ok()?.flatten() {
            let p = e.path();
            if p.is_dir() {
                if let Some(found) = find(&p, depth + 1) { return Some(found); }
            } else if e.file_name() == "RemoteControlSettings.h" {
                return Some(p);
            }
        }
        None
    }
    find(&engine.join("Engine/Plugins"), 0).and_then(|p| std::fs::read_to_string(p).ok())
}

fn write_remote_control_ini(project: &Project) -> Result<Value> {
    let header = engine_root(project).ok().and_then(|e| rc_header(&e));
    let (keys, unknown): (Vec<_>, Vec<_>) = RC_KEYS.iter().partition(|(k, _)| header.as_ref().is_none_or(|h| h.contains(k)));
    let path = project.root.join("Config/DefaultRemoteControl.ini");
    let changed = merge_ini_file(&path, RC_SECTION, &keys.iter().map(|(k, v)| (k.to_string(), v.to_string())).collect::<Vec<_>>())?;
    Ok(json!({
        "file": path, "changed": changed,
        "written": keys.iter().map(|(k, v)| format!("{k}={v}")).collect::<Vec<_>>(),
        "not_in_this_engine": unknown.iter().map(|(k, _)| k.to_string()).collect::<Vec<_>>(),
        "checked_against_engine_source": header.is_some(),
    }))
}

/// "Use Less CPU when in Background", off. Changed only in memory (as `ue_play` does), it comes
/// back on at the next start, so it goes into the project's default per-user settings, and into
/// any saved per-user file that already holds it, since that file wins over the default.
const PERF_SECTION: &str = "[/Script/UnrealEd.EditorPerformanceSettings]";
const PERF_KEY: &str = "bThrottleCPUWhenNotForeground";

fn write_editor_settings_ini(project: &Project) -> Result<Value> {
    let keys = [(PERF_KEY.to_string(), "False".to_string())];
    let mut files = vec![project.root.join("Config/DefaultEditorPerProjectUserSettings.ini")];
    if let Ok(dirs) = std::fs::read_dir(project.root.join("Saved/Config")) {
        for dir in dirs.flatten() {
            let saved = dir.path().join("EditorPerProjectUserSettings.ini");
            if std::fs::read_to_string(&saved).is_ok_and(|t| t.lines().any(|l| l.split_once('=').is_some_and(|(k, _)| k.trim() == PERF_KEY))) {
                files.push(saved);
            }
        }
    }
    let mut changed = Vec::new();
    let mut skipped = Vec::new();
    for path in &files {
        match merge_ini_file(path, PERF_SECTION, &keys) {
            Ok(true) => changed.push(path.clone()),
            Ok(false) => {}
            Err(error) => skipped.push(format!("{error:#}")),
        }
    }
    Ok(json!({"files": files, "changed": changed, "skipped": skipped, "written": format!("{PERF_KEY}=False")}))
}

/// `merge_ini` on a file. Only a missing file counts as empty: one that cannot be read (not
/// UTF-8, no permission) is left as it is with an error rather than replaced by one section,
/// and the new text is renamed into place, so a crash never leaves it half written (RA-286).
/// Whether the file changed.
fn merge_ini_file(path: &Path, section: &str, keys: &[(String, String)]) -> Result<bool> {
    let text = match std::fs::read_to_string(path) {
        Ok(text) => text,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => String::new(),
        Err(error) => return Err(error).with_context(|| format!("reading {}; left unchanged", path.display())),
    };
    let merged = merge_ini(&text, section, keys);
    if merged == text {
        return Ok(false);
    }
    if let Some(parent) = path.parent() { std::fs::create_dir_all(parent)?; }
    let tmp = path.with_extension("ini.relay-tmp");
    std::fs::write(&tmp, &merged).with_context(|| format!("writing {}", tmp.display()))?;
    std::fs::rename(&tmp, path).with_context(|| format!("replacing {}", path.display()))?;
    Ok(true)
}

/// Set keys inside one ini section, keeping every other line and section as it was.
fn merge_ini(text: &str, section: &str, keys: &[(String, String)]) -> String {
    let mut lines: Vec<String> = text.lines().map(str::to_string).collect();
    let start = lines.iter().position(|l| l.trim() == section);
    let start = match start {
        Some(i) => i,
        None => {
            if lines.last().is_some_and(|l| !l.trim().is_empty()) { lines.push(String::new()); }
            lines.push(section.to_string());
            lines.len() - 1
        }
    };
    let end = lines.iter().enumerate().skip(start + 1).find(|(_, l)| l.trim_start().starts_with('[')).map(|(i, _)| i).unwrap_or(lines.len());
    let mut insert_at = end;
    for (key, value) in keys {
        let found = (start + 1..end).find(|i| lines[*i].split_once('=').is_some_and(|(k, _)| k.trim() == key));
        match found {
            Some(i) => lines[i] = format!("{key}={value}"),
            None => {
                // Keep the section's block together, before any trailing blank lines.
                while insert_at > start + 1 && lines[insert_at - 1].trim().is_empty() { insert_at -= 1; }
                lines.insert(insert_at, format!("{key}={value}"));
                insert_at += 1;
            }
        }
    }
    let mut out = lines.join("\n");
    out.push('\n');
    out
}

fn log_len(project: &Project) -> u64 {
    std::fs::metadata(project.log_path()).map(|m| m.len()).unwrap_or(0)
}

/// Lines that may report a failed bind; `unreal_process::bind_failures` decides.
const BIND_PREFILTER: &str = r"(?i)bind|address already in use";

/// The log file's identity and length, or None when there is none yet.
fn log_mark(project: &Project) -> Option<(u64, u64)> {
    use std::os::unix::fs::MetadataExt;
    std::fs::metadata(project.log_path()).ok().map(|m| (m.ino(), m.len()))
}

/// Where a session started after `mark` begins in the log on disk: past the old end while the
/// file is still the one marked, and at 0 once the editor has moved it aside for a new one.
fn new_session_start(project: &Project, mark: Option<(u64, u64)>) -> u64 {
    use std::os::unix::fs::MetadataExt;
    match (std::fs::metadata(project.log_path()), mark) {
        (Ok(now), Some((ino, len))) if now.ino() == ino && now.len() >= len => len,
        _ => 0,
    }
}

/// Every log line written since byte offset `start` that `filter` matches (all lines without
/// one). Callers that parse the lines get all of them; what goes back to the agent is capped
/// with `cap_lines`.
fn log_since(project: &Project, start: u64, filter: Option<&str>) -> Vec<String> {
    use std::io::Seek;
    // Only the bytes past `start`: callers poll a log that can run to hundreds of MB (RA-287).
    let Ok(mut file) = std::fs::File::open(project.log_path()) else { return Vec::new() };
    let mut bytes = Vec::new();
    if file.seek(std::io::SeekFrom::Start(start)).is_err() || file.read_to_end(&mut bytes).is_err() {
        return Vec::new();
    }
    let text = String::from_utf8_lossy(&bytes);
    let re = filter.and_then(|f| regex::Regex::new(f).ok());
    text.lines().filter(|l| re.as_ref().is_none_or(|re| re.is_match(l))).map(str::to_string).collect()
}

/// At most `max` lines: the first half (the first errors are usually the cause) and the last
/// half, with a line in between saying how many were left out.
fn cap_lines(mut lines: Vec<String>, max: usize) -> Vec<String> {
    if lines.len() <= max {
        return lines;
    }
    let head = max / 2;
    let tail = max - head;
    let omitted = lines.len() - head - tail;
    let rest = lines.split_off(lines.len() - tail);
    lines.truncate(head);
    lines.push(format!("[relay: {omitted} matching lines omitted here; narrow the filter, or read them with ue_log]"));
    lines.extend(rest);
    lines
}

fn play_step(action: &str, extra: Value) -> Result<Value> {
    let mut args = extra;
    args["action"] = json!(action);
    python_json_tx(&script(PY_PLAY, &args), Duration::from_secs(60), false)
}

/// Ask the editor to start playing. Afterwards `requested` says whether a session may be running
/// because of this call, so cleanup can stop it on every path: true once the request may have
/// reached the editor (a reply lost to a timeout included), false when play.py refused before
/// asking, which it does when a session that is not this call's is already running.
fn request_play(mode: &str, requested: &mut bool) -> Result<Value> {
    let result = play_step("start", json!({"mode": mode}));
    *requested = match &result {
        Ok(_) => true,
        Err(error) => !format!("{error:#}").starts_with("Python failed"),
    };
    result
}

fn wait_for_play(on: bool) -> Result<()> {
    for _ in 0..120 {
        if play_step("status", json!({}))?["in_play"] == on {
            return Ok(());
        }
        std::thread::sleep(Duration::from_millis(250));
    }
    bail!("the play session did not {} within 30 s", if on { "start" } else { "stop" })
}

fn pngs_under(dir: &Path) -> Vec<PathBuf> {
    let mut out = Vec::new();
    let Ok(entries) = std::fs::read_dir(dir) else { return out };
    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_dir() {
            out.extend(pngs_under(&path));
        } else if path.extension().is_some_and(|e| e.eq_ignore_ascii_case("png")) {
            out.push(path);
        }
    }
    out
}

const DEFAULT_PLAY_LOG: &str = r"Error|Warning|[Ee]nsure|Accessed None|Script Msg|LogBlueprintUserMessages|Assertion";

/// Files written under `dir` that were not in `seen`, oldest first.
fn new_pngs(dir: &Path, seen: &std::collections::HashSet<PathBuf>) -> Vec<PathBuf> {
    let mut files: Vec<(std::time::SystemTime, PathBuf)> = pngs_under(dir)
        .into_iter()
        .filter(|p| !seen.contains(p))
        .filter_map(|p| Some((std::fs::metadata(&p).ok()?.modified().ok()?, p)))
        .collect();
    files.sort();
    files.into_iter().map(|(_, p)| p).collect()
}

fn play(project: &Project, args: &Value) -> Result<Value> {
    let seconds = args["seconds"].as_f64().unwrap_or(5.0).clamp(1.0, 300.0);
    let mut checkpoints: Vec<f64> = args["checkpoints"].as_array()
        .map(|a| a.iter().filter_map(Value::as_f64).filter(|t| *t >= 0.0 && *t <= seconds).collect())
        .unwrap_or_else(|| vec![seconds]);
    checkpoints.sort_by(|a, b| a.partial_cmp(b).unwrap());
    checkpoints.truncate(12);
    let screenshots = args["screenshots"].as_bool().unwrap_or(true);
    let shots_dir = project.root.join("Saved/Screenshots");
    let mut seen: std::collections::HashSet<PathBuf> = pngs_under(&shots_dir).into_iter().collect();
    let outside = args.get("outside").filter(|o| o.is_object()).cloned();
    // Every step that can fail runs in the closure below, and what it may leave behind (a
    // capture camera in the level, a running session, a scratch folder) is undone after it on
    // every path. These say what there is to undo.
    let mut prepared = false;
    let mut requested = false;
    let mut outside_dir: Option<PathBuf> = None;
    let mut started = Value::Null;
    let mut log_start = log_len(project);
    let mut t0 = std::time::Instant::now();
    let mut first = None;
    let mut shots: Vec<Value> = Vec::new();
    let mut probes: Vec<Value> = Vec::new();
    let outcome = (|| -> Result<()> {
        if args["stop_existing"] == true {
            play_step("stop", json!({}))?;
            wait_for_play(false)?;
        }
        if outside.is_some() {
            // Placed in the level now so the game world, a copy of the level, contains it.
            // Marked first: a step that fails half way may still have placed it.
            prepared = true;
            play_step("prepare_outside", json!({}))?;
            outside_dir = Some(capture_dir(project)?);
        }
        log_start = log_len(project);
        started = request_play(args["mode"].as_str().unwrap_or("pie"), &mut requested)?;
        wait_for_play(true)?;
        t0 = std::time::Instant::now();
        first = play_step("status", json!({})).ok();
        if let Some(commands) = args["console"].as_array() {
            play_step("console", json!({"commands": commands}))?;
        }
        for at in &checkpoints {
            std::thread::sleep(Duration::from_secs_f64(*at).saturating_sub(t0.elapsed()));
            // Probe first, then capture, then wait for this capture's file: a screenshot is
            // written a frame or more after the request, and pairing by order let images fall
            // one checkpoint behind the probes.
            if let Some(code) = args["probe"].as_str() {
                let out = python_tx(&format!("ARGS_JSON = {}\n{PY_COMMON}\nworld = unreal.get_editor_subsystem(unreal.UnrealEditorSubsystem).get_game_world()\n{code}", py_str("{}")), Duration::from_secs(60), false);
                probes.push(match out {
                    Ok(v) => json!({"at": at, "output": v["output"]}),
                    Err(e) => json!({"at": at, "error": format!("{e:#}")}),
                });
            }
            if screenshots {
                play_step("shot", json!({"width": args["width"].as_u64().unwrap_or(1280), "height": args["height"].as_u64().unwrap_or(720)}))?;
                let deadline = std::time::Instant::now() + Duration::from_secs(5);
                loop {
                    let fresh = new_pngs(&shots_dir, &seen);
                    if let Some(file) = fresh.first() {
                        // Give the writer a moment to finish the file.
                        std::thread::sleep(Duration::from_millis(150));
                        seen.insert(file.clone());
                        shots.push(json!({"view": format!("checkpoint {at}s"), "file": file}));
                        break;
                    }
                    if std::time::Instant::now() >= deadline {
                        shots.push(json!({"view": format!("checkpoint {at}s"), "missing": true}));
                        break;
                    }
                    std::thread::sleep(Duration::from_millis(100));
                }
            }
            if let (Some(spec), Some(dir)) = (&outside, &outside_dir) {
                let mut a = spec.clone();
                a["out_dir"] = json!(dir);
                a["prefix"] = json!(format!("outside_{at}s"));
                match play_step("outside_capture", a) {
                    Ok(v) => {
                        for f in v["files"].as_array().cloned().unwrap_or_default() {
                            shots.push(json!({"view": format!("checkpoint {at}s, {}", f["view"].as_str().unwrap_or("outside")), "file": f["file"]}));
                        }
                    }
                    Err(e) => shots.push(json!({"view": format!("checkpoint {at}s outside"), "error": format!("{e:#}")})),
                }
            }
        }
        std::thread::sleep(Duration::from_secs_f64(seconds).saturating_sub(t0.elapsed()));
        Ok(())
    })();
    let last = if requested { play_step("status", json!({})).ok() } else { None };
    // The session's length ends here: stopping and cleaning up are not frames played (RA-288).
    let played = t0.elapsed();
    let stopped = if requested && args["stop"].as_bool().unwrap_or(true) {
        play_step("stop", json!({"restore_throttle": started["was_throttled"]})).and_then(|_| wait_for_play(false)).err().map(|e| format!("{e:#}"))
    } else {
        None
    };
    let cleanup_error = if prepared { play_step("cleanup_outside", json!({})).err().map(|e| format!("{e:#}")) } else { None };
    let filter = args["log_filter"].as_str().unwrap_or(DEFAULT_PLAY_LOG);
    // Harmless: Remote Control wraps calls in a transaction that play start cancels.
    let log: Vec<String> = log_since(project, log_start, Some(filter)).into_iter().filter(|l| !l.contains("Remote Call Transaction Wrap")).collect();
    let errors = log.iter().filter(|l| l.contains("Error") || l.contains("Accessed None") || l.to_lowercase().contains("ensure")).count();
    let log = cap_lines(log, MAX_LOG_LINES);
    if let Err(error) = outcome {
        // What the session produced is often what explains the failure (a Blueprint error
        // that ended play, say), so it goes back with the error.
        if let Some(dir) = &outside_dir {
            let _ = std::fs::remove_dir_all(dir);
        }
        let kept: Vec<&Value> = shots.iter().filter(|s| !outside_dir.as_ref().is_some_and(|d| s["file"].as_str().is_some_and(|f| Path::new(f).starts_with(d)))).collect();
        bail!("{error:#}\n\nThe session so far: {}", serde_json::to_string_pretty(&json!({
            "probes": probes, "screenshots": kept, "log": log, "error_lines": errors, "stop_error": stopped, "cleanup_error": cleanup_error,
        }))?);
    }
    // Frames per wall-clock second over the session: a throttled or overloaded editor shows
    // here before it shows as a flaky test.
    let fps = match (first.as_ref().and_then(|v| v["frame"].as_u64()), last.as_ref().and_then(|v| v["frame"].as_u64())) {
        (Some(a), Some(b)) if b > a => Some(((b - a) as f64 / played.as_secs_f64() * 10.0).round() / 10.0),
        _ => None,
    };
    let images: Vec<Value> = shots.iter().filter(|s| s["file"].is_string()).map(|s| json!({"label": s["view"], "path": s["file"]})).collect();
    Ok(json!({
        "mode": started["requested"],
        "played_s": played.as_secs_f64().min(seconds + 5.0),
        "average_fps": fps,
        "fps_warning": if fps.is_some_and(|f| f < 20.0) { json!("Under 20 fps: timings, physics and animation in this session are not representative. If the editor window was in the background, check Editor Preferences > Performance > Use Less CPU when in Background.") } else { Value::Null },
        "checkpoints": checkpoints,
        "probes": probes,
        "screenshots": shots,
        "log": log,
        "error_lines": errors,
        "stop_error": stopped,
        "cleanup_error": cleanup_error,
        // Screenshots live in the project's own folder; show them but leave them in place.
        // Outside captures live in a scratch folder that goes once they are read.
        "_images": images,
        "_cleanup": outside_dir,
    }))
}

fn data_table(project: &Project, args: &Value) -> Result<Value> {
    let format = args["format"].as_str().unwrap_or("csv");
    let file = args["file"].as_str().map(|f| {
        anyhow::ensure!(!f.contains("..") && !Path::new(f).is_absolute(), "file must be relative to the project root");
        Ok(project.root.join(f))
    }).transpose()?;
    match args["action"].as_str() {
        Some("export") => {
            let mut value = python_json(&script(PY_DATA_TABLE, &json!({"action":"export","path":args["path"],"format":format})), Duration::from_secs(120))?;
            if let Some(file) = &file {
                if let Some(parent) = file.parent() { std::fs::create_dir_all(parent)?; }
                std::fs::write(file, value["text"].as_str().unwrap_or(""))?;
                value["written"] = json!(file);
            }
            Ok(value)
        }
        Some("import") => {
            let file = file.ok_or_else(|| anyhow!("import needs file"))?;
            let text = std::fs::read_to_string(&file).with_context(|| format!("reading {}", file.display()))?;
            let start = log_len(project);
            let result = python_json(&script(PY_DATA_TABLE, &json!({"action":"import","path":args["path"],"format":format,"text":text})), Duration::from_secs(120));
            match result {
                Ok(v) => Ok(v),
                Err(e) => bail!("{e:#}\n{}", cap_lines(log_since(project, start, Some("LogDataTable|Error|Warning")), MAX_LOG_LINES).join("\n")),
            }
        }
        _ => bail!("action must be export or import"),
    }
}

/// Averages, 95th percentiles and maxima of the timing columns of an Unreal CSV profile.
fn summarize_csv(text: &str) -> Value {
    let mut lines = text.lines();
    let Some(header) = lines.next() else { return json!({"error":"empty profile"}) };
    let columns: Vec<&str> = header.split(',').map(str::trim).collect();
    let wanted: Vec<usize> = columns.iter().enumerate()
        .filter(|(_, c)| ["FrameTime", "GameThreadTime", "RenderThreadTime", "GPUTime", "RHIThreadTime"].contains(c) || c.starts_with("GPU/") && c.len() < 40)
        .map(|(i, _)| i).collect();
    let mut rows: Vec<Vec<f64>> = Vec::new();
    for line in lines {
        let cells: Vec<&str> = line.split(',').collect();
        if cells.len() != columns.len() {
            continue;
        }
        let values: Option<Vec<f64>> = wanted.iter().map(|i| cells[*i].trim().parse().ok()).collect();
        if let Some(values) = values { rows.push(values); }
    }
    let mut stats = Map::new();
    for (k, i) in wanted.iter().enumerate() {
        let mut v: Vec<f64> = rows.iter().map(|r| r[k]).collect();
        if v.is_empty() { continue; }
        let avg = v.iter().sum::<f64>() / v.len() as f64;
        v.sort_by(|a, b| a.partial_cmp(b).unwrap());
        let p95 = v[((v.len() as f64 * 0.95) as usize).min(v.len() - 1)];
        stats.insert(columns[*i].to_string(), json!({"avg_ms": (avg * 100.0).round() / 100.0, "p95_ms": (p95 * 100.0).round() / 100.0, "max_ms": (v[v.len()-1] * 100.0).round() / 100.0}));
    }
    let frame = columns.iter().position(|c| *c == "FrameTime").and_then(|i| wanted.iter().position(|w| *w == i));
    let mut worst: Vec<(usize, f64)> = frame.map(|k| rows.iter().enumerate().map(|(n, r)| (n, r[k])).collect()).unwrap_or_default();
    worst.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap());
    let fps = stats.get("FrameTime").and_then(|f| f["avg_ms"].as_f64()).filter(|ms| *ms > 0.0).map(|ms| (1000.0 / ms * 10.0).round() / 10.0);
    json!({"frames": rows.len(), "avg_fps": fps, "stats": stats,
        "worst_frames": worst.iter().take(5).map(|(n, ms)| json!({"frame": n, "ms": ms})).collect::<Vec<_>>()})
}

fn profile(project: &Project, args: &Value) -> Result<Value> {
    let seconds = args["seconds"].as_f64().unwrap_or(10.0).clamp(2.0, 120.0);
    let warmup = args["warmup"].as_f64().unwrap_or(3.0).clamp(0.0, 60.0);
    let play = args["play"].as_bool().unwrap_or(true);
    let dir = project.root.join("Saved/Profiling/CSV");
    let before: std::collections::HashSet<PathBuf> = std::fs::read_dir(&dir).map(|e| e.flatten().map(|e| e.path()).collect()).unwrap_or_default();
    // As in ue_play: the steps run in one closure, and the session and the profiler are
    // stopped after it on every path.
    let mut throttle = Value::Null;
    let mut requested = false;
    let mut recording = false;
    let run = (|| -> Result<()> {
        if play {
            throttle = request_play("pie", &mut requested)?["was_throttled"].clone();
            wait_for_play(true)?;
        }
        if let Some(commands) = args["console"].as_array() {
            play_step("console", json!({"commands": commands}))?;
        }
        std::thread::sleep(Duration::from_secs_f64(warmup));
        recording = true;
        play_step("console", json!({"commands": ["csvprofile start"]}))?;
        std::thread::sleep(Duration::from_secs_f64(seconds));
        play_step("console", json!({"commands": ["csvprofile stop"]}))?;
        recording = false;
        Ok(())
    })();
    if recording {
        let _ = play_step("console", json!({"commands": ["csvprofile stop"]}));
    }
    let stop_error = if requested {
        play_step("stop", json!({"restore_throttle": throttle})).and_then(|_| wait_for_play(false)).err().map(|e| format!("{e:#}"))
    } else {
        None
    };
    if let Err(error) = run {
        bail!("{error:#}{}", stop_error.map(|s| format!("\nStopping the play session also failed: {s}")).unwrap_or_default());
    }
    let mut file = None;
    for _ in 0..40 {
        file = std::fs::read_dir(&dir).ok().and_then(|e| e.flatten().map(|e| e.path())
            .filter(|p| !before.contains(p) && p.extension().is_some_and(|x| x == "csv")).max());
        if file.is_some() { break; }
        std::thread::sleep(Duration::from_millis(250));
    }
    let file = file.ok_or_else(|| anyhow!("no CSV profile appeared in {} (csvprofile needs a Development or Test build of the editor)", dir.display()))?;
    // The profiler finishes writing on a worker thread.
    std::thread::sleep(Duration::from_millis(750));
    let text = std::fs::read_to_string(&file)?;
    let mut summary = summarize_csv(&text);
    summary["file"] = json!(file);
    summary["recorded_s"] = json!(seconds);
    summary["stop_error"] = json!(stop_error);
    summary["budget_hint"] = json!("60 fps is 16.7 ms per frame, 30 fps is 33.3 ms. The largest of game thread, render thread and GPU is what limits the frame.");
    Ok(summary)
}

fn editor_cmd(engine: &Path) -> PathBuf {
    let bin = engine.join("Engine/Binaries");
    if cfg!(target_os = "windows") {
        bin.join("Win64/UnrealEditor-Cmd.exe")
    } else if cfg!(target_os = "macos") {
        bin.join("Mac/UnrealEditor-Cmd")
    } else {
        bin.join("Linux/UnrealEditor-Cmd")
    }
}

fn run_tests(project: &Project, args: &Value) -> Result<Value> {
    let filter = required(args, "filter")?;
    // -ExecCmds splits its commands on ',' and the automation command on ';': either would cut
    // off the Quit and leave the headless editor idling until the timeout (RA-551).
    anyhow::ensure!(!filter.contains([';', '"', ',']), "filter must be a test path prefix; separate several with '+', e.g. MyGame.A+MyGame.B");
    if args["in_editor"] == true {
        guard_project(project)?;
        // Refreshed while the tests run: a long suite must not look abandoned.
        let _hold = LockHold::take(project, &holder_id())?;
        let start = log_len(project);
        play_step("console", json!({"commands": [format!("Automation RunTests {filter}")]}))?;
        let deadline = std::time::Instant::now() + secs(args, "timeout_s", 1800);
        loop {
            // Every result line, however many tests ran.
            let lines = log_since(project, start, Some("LogAutomation"));
            if lines.iter().any(|l| l.contains("Automation Test Queue Empty") || l.contains("No automation tests matched")) {
                return Ok(test_lines(&lines));
            }
            if std::time::Instant::now() > deadline {
                bail!("tests did not finish before the timeout; partial results:\n{}", cap_lines(lines, MAX_LOG_LINES).join("\n"));
            }
            std::thread::sleep(Duration::from_secs(1));
        }
    }
    let engine = engine_root(project)?;
    let cmd = editor_cmd(&engine);
    anyhow::ensure!(cmd.is_file(), "{} does not exist (build the editor, or use in_editor=true)", cmd.display());
    let report = project.root.join("Saved/Relay/TestReport").join(uuid::Uuid::new_v4().to_string());
    std::fs::create_dir_all(&report)?;
    let result = run_tests_headless(project, args, &cmd, &filter, &report);
    // Gone on every path, a timeout included.
    let _ = std::fs::remove_dir_all(&report);
    result
}

fn run_tests_headless(project: &Project, args: &Value, cmd: &Path, filter: &str, report: &Path) -> Result<Value> {
    let mut command = Command::new(cmd);
    command
        .arg(&project.uproject)
        .arg(format!("-ExecCmds=Automation RunTests {filter};Quit"))
        // A backstop: the editor exits once the queue is empty even if Quit never ran.
        .arg("-TestExit=Automation Test Queue Empty")
        .arg(format!("-ReportExportPath={}", report.display()))
        .args(["-unattended", "-nopause", "-nosplash", "-nullrhi", "-NoSound", "-stdout", "-FullStdOutLogOutput"]);
    let started = std::time::Instant::now();
    let output = relay_core::proc::output_with_timeout(&mut command, secs(args, "timeout_s", 1800))?
        .ok_or_else(|| anyhow!("the test run did not finish within its timeout and was stopped"))?;
    let stdout = String::from_utf8_lossy(&output.stdout);
    let index = std::fs::read_to_string(report.join("index.json")).ok()
        .and_then(|raw| serde_json::from_str::<Value>(raw.trim_start_matches('\u{feff}')).ok());
    let mut value = match index {
        Some(index) => {
            let tests: Vec<Value> = index["tests"].as_array().cloned().unwrap_or_default().iter().map(|t| {
                let errors: Vec<Value> = t["entries"].as_array().cloned().unwrap_or_default().iter()
                    .filter(|e| e["event"]["type"] == "Error" || e["event"]["type"] == "Warning")
                    .map(|e| json!(format!("{}: {}", e["event"]["type"].as_str().unwrap_or(""), e["event"]["message"].as_str().unwrap_or(""))))
                    .take(20).collect();
                json!({"test": t["fullTestPath"], "state": t["state"], "messages": errors})
            }).collect();
            json!({"succeeded": index["succeeded"], "failed": index["failed"], "tests": tests})
        }
        None => test_lines(&stdout.lines().filter(|l| l.contains("LogAutomation")).map(str::to_string).collect::<Vec<_>>()),
    };
    value["exit_code"] = json!(output.status.code());
    value["seconds"] = json!(started.elapsed().as_secs());
    Ok(value)
}

/// Results from `LogAutomationController` lines: `Test Completed. Result={Success} Name={..} Path={..}`.
fn test_lines(lines: &[String]) -> Value {
    let re = regex::Regex::new(r"Result=\{(\w+)\}\s+Name=\{([^}]*)\}\s+Path=\{([^}]*)\}").unwrap();
    let tests: Vec<Value> = lines.iter().filter_map(|l| re.captures(l)).map(|c| json!({"test": &c[3], "name": &c[2], "state": &c[1]})).collect();
    let failed = tests.iter().filter(|t| t["state"] != "Success").count();
    json!({"succeeded": tests.len() - failed, "failed": failed, "tests": tests,
        "errors": lines.iter().filter(|l| l.contains("Error")).take(50).collect::<Vec<_>>()})
}

fn crash(project: &Project, index: usize) -> Result<Value> {
    let dir = project.root.join("Saved/Crashes");
    let mut crashes: Vec<(std::time::SystemTime, PathBuf)> = std::fs::read_dir(&dir)
        .with_context(|| format!("no crash reports in {}", dir.display()))?
        .flatten()
        .filter(|e| e.path().is_dir())
        .filter_map(|e| Some((e.metadata().ok()?.modified().ok()?, e.path())))
        .collect();
    crashes.sort_by_key(|c| std::cmp::Reverse(c.0));
    let (_, folder) = crashes.get(index).cloned().ok_or_else(|| anyhow!("there are {} crash reports", crashes.len()))?;
    let context = std::fs::read_to_string(folder.join("CrashContext.runtime-xml")).unwrap_or_default();
    let field = |name: &str| -> Option<String> {
        let re = regex::Regex::new(&format!(r"(?s)<{name}>(.*?)</{name}>")).ok()?;
        re.captures(&context).map(|c| c[1].trim().replace("&lt;", "<").replace("&gt;", ">").replace("&amp;", "&").replace("&quot;", "\"").replace("&apos;", "'"))
    };
    let log = std::fs::read_dir(&folder).ok().and_then(|e| e.flatten().map(|e| e.path()).find(|p| p.extension().is_some_and(|x| x == "log")));
    let log_tail = log.and_then(|p| std::fs::read_to_string(p).ok()).map(|t| tail(&t, 80));
    Ok(json!({
        "folder": folder,
        "of": crashes.len(),
        "error": field("ErrorMessage"),
        "crash_type": field("CrashType"),
        "callstack": field("CallStack").map(|c| tail(&c, 60)),
        "engine_version": field("EngineVersion"),
        "build_configuration": field("BuildConfiguration"),
        "log_tail": log_tail,
        "hint": "Read the first frames of the call stack that are in your project's modules; engine frames below them are usually the consequence.",
    }))
}

// ---------------------------------------------------------------- seeing

fn capture_dir(project: &Project) -> Result<PathBuf> {
    let dir = project.root.join("Saved/Relay/Captures").join(uuid::Uuid::new_v4().to_string());
    std::fs::create_dir_all(&dir).with_context(|| format!("creating {}", dir.display()))?;
    Ok(dir)
}

/// Attach the PNGs a capture script wrote as `_images`, which the MCP layer turns into image
/// content, then delete the folder.
fn with_images(result: Result<Value>, dir: &Path) -> Result<Value> {
    let mut value = match result {
        Ok(value) => value,
        Err(error) => {
            let _ = std::fs::remove_dir_all(dir);
            return Err(error);
        }
    };
    let mut images = Vec::new();
    for file in value["files"].as_array().cloned().unwrap_or_default() {
        let path = PathBuf::from(file["file"].as_str().unwrap_or(""));
        // Some engine versions append the extension themselves.
        let found = [path.clone(), path.with_extension("png.png")].into_iter().find(|p| p.is_file());
        match found {
            Some(found) => images.push(json!({"label": file["view"], "path": found})),
            None => images.push(json!({"label": file["view"], "missing": path})),
        }
    }
    value["_images"] = json!(images);
    value["_cleanup"] = json!(dir);
    Ok(value)
}

fn anim_preview(project: &Project, args: &Value) -> Result<Value> {
    let dir = capture_dir(project)?;
    let mut base = args.clone();
    if base.get("samples").is_none() && base.get("times").is_none() {
        base["samples"] = json!(4);
    }
    let run = |action: &str, extra: Value| -> Result<Value> {
        let mut a = base.clone();
        a["action"] = json!(action);
        if let (Some(target), Some(extra)) = (a.as_object_mut(), extra.as_object()) {
            for (k, v) in extra {
                target.insert(k.clone(), v.clone());
            }
        }
        python_json(&script(PY_ANIM_PREVIEW, &a), Duration::from_secs(120))
    };
    let outcome = (|| -> Result<Value> {
        let setup = run("setup", json!({}))?;
        let times: Vec<f64> = setup["times"].as_array().cloned().unwrap_or_default().iter().filter_map(Value::as_f64).take(8).collect();
        let settle = Duration::from_millis(args["settle_ms"].as_u64().unwrap_or(400).clamp(50, 5000));
        let mut files = Vec::new();
        let mut poses = Vec::new();
        for (i, t) in times.iter().enumerate() {
            run("pose", json!({"time": t}))?;
            // The editor applies a pose on its next tick; check before taking the picture.
            let mut check = Value::Null;
            for _ in 0..8 {
                std::thread::sleep(settle);
                check = run("check", json!({"time": t}))?;
                if check["off_by_cm"].as_f64().unwrap_or(f64::MAX) <= 1.5 {
                    break;
                }
            }
            poses.push(json!({"time": t, "pose_applied": check["off_by_cm"].as_f64().unwrap_or(f64::MAX) <= 1.5, "off_by_cm": check["off_by_cm"]}));
            let capture_args = json!({
                "actors": setup["actors"], "forward": setup["forward"],
                "center": check["center"], "radius": check["radius"],
                "views": args.get("views").cloned().unwrap_or(json!(["front", "right"])),
                // The preview is spawned away from level geometry, so isolation is rarely needed.
                "isolate": args["isolate"].as_bool().unwrap_or(false),
                "width": args.get("width").cloned().unwrap_or(json!(480)),
                "height": args.get("height").cloned().unwrap_or(json!(480)),
                "out_dir": dir, "prefix": format!("t{i}_{t:.3}s"),
            });
            let shot = python_json(&script(PY_CAPTURE, &capture_args), Duration::from_secs(120))?;
            files.extend(shot["files"].as_array().cloned().unwrap_or_default());
        }
        let stale = poses.iter().any(|p| p["pose_applied"] == false);
        Ok(json!({
            "files": files,
            "poses": poses,
            "warning": if stale { json!("Some poses had not been applied when captured: the editor may be throttled in the background (Editor Preferences > General > Performance > Use Less CPU when in Background). Raise settle_ms or keep the editor focused, and trust ue_anim_inspect's numbers over these images.") } else { Value::Null },
        }))
    })();
    let cleanup = run("cleanup", json!({}));
    let mut value = with_images(outcome, &dir)?;
    if let Err(error) = cleanup {
        value["cleanup_error"] = json!(format!("{error:#}"));
    }
    Ok(value)
}

fn required(args: &Value, key: &str) -> Result<String> {
    args[key].as_str().filter(|v| !v.trim().is_empty()).map(str::to_string).ok_or_else(|| anyhow!("{key} is required"))
}

fn secs(args: &Value, key: &str, default: u64) -> Duration {
    Duration::from_secs(args[key].as_u64().unwrap_or(default).clamp(5, 7200))
}

/// A JSON string literal is also a valid Python string literal.
fn py_str(text: &str) -> String {
    serde_json::to_string(text).expect("a string serializes")
}

// ---------------------------------------------------------------- the project on disk

struct Project {
    uproject: PathBuf,
    root: PathBuf,
    name: String,
    descriptor: Value,
}

impl Project {
    fn find() -> Result<Project> {
        let uproject = match std::env::var_os("UE_PROJECT") {
            Some(path) => PathBuf::from(path),
            None => {
                let start = std::env::var_os("RELAY_WORKTREE").map(PathBuf::from)
                    .filter(|p| p.is_dir())
                    .unwrap_or(std::env::current_dir()?);
                find_uproject(&start).ok_or_else(|| anyhow!(
                    "no .uproject within three folders of {} — set UE_PROJECT to the project file", start.display()
                ))?
            }
        };
        let raw = std::fs::read_to_string(&uproject).with_context(|| format!("reading {}", uproject.display()))?;
        let descriptor: Value = serde_json::from_str(&raw).with_context(|| format!("parsing {}", uproject.display()))?;
        let root = uproject.parent().map(Path::to_path_buf).unwrap_or_default();
        let name = uproject.file_stem().map(|s| s.to_string_lossy().into_owned()).unwrap_or_default();
        Ok(Project { uproject, root, name, descriptor })
    }

    fn log_path(&self) -> PathBuf {
        self.root.join("Saved/Logs").join(format!("{}.log", self.name))
    }
}

fn find_uproject(start: &Path) -> Option<PathBuf> {
    let mut level = vec![start.to_path_buf()];
    for _ in 0..=3 {
        let mut next = Vec::new();
        for dir in &level {
            let Ok(entries) = std::fs::read_dir(dir) else { continue };
            let mut entries: Vec<_> = entries.flatten().collect();
            entries.sort_by_key(|e| e.file_name());
            for entry in &entries {
                let path = entry.path();
                if path.extension().is_some_and(|e| e == "uproject") && path.is_file() {
                    return Some(path);
                }
            }
            for entry in entries {
                let name = entry.file_name().to_string_lossy().into_owned();
                if entry.path().is_dir() && !name.starts_with('.') && !SKIP_DIRS.contains(&name.as_str()) {
                    next.push(entry.path());
                }
            }
        }
        level = next;
    }
    None
}

/// The engine directory (the one holding `Engine/`) this project builds with.
fn engine_root(project: &Project) -> Result<PathBuf> {
    let association = project.descriptor["EngineAssociation"].as_str().unwrap_or("").trim().to_string();
    crate::unreal_process::engine_root(&project.uproject, &project.root, &association, &project.log_path())
}

fn host_platform() -> &'static str {
    if cfg!(target_os = "windows") { "Win64" } else if cfg!(target_os = "macos") { "Mac" } else { "Linux" }
}

fn build_script(engine: &Path) -> PathBuf {
    let batch = engine.join("Engine/Build/BatchFiles");
    if cfg!(target_os = "windows") {
        batch.join("Build.bat")
    } else if cfg!(target_os = "macos") {
        batch.join("Mac/Build.sh")
    } else {
        batch.join("Linux/Build.sh")
    }
}

fn names_with_suffix(dir: &Path, suffix: &str, depth: usize) -> Vec<String> {
    let mut out = Vec::new();
    let Ok(entries) = std::fs::read_dir(dir) else { return out };
    for entry in entries.flatten() {
        let path = entry.path();
        let name = entry.file_name().to_string_lossy().into_owned();
        if path.is_dir() && depth > 0 && !SKIP_DIRS.contains(&name.as_str()) {
            out.extend(names_with_suffix(&path, suffix, depth - 1));
        } else if let Some(stem) = name.strip_suffix(suffix) {
            out.push(stem.to_string());
        }
    }
    out.sort();
    out
}

fn project_info(project: &Project) -> Result<Value> {
    let source = project.root.join("Source");
    let targets = names_with_suffix(&source, ".Target.cs", 0);
    let modules = names_with_suffix(&source, ".Build.cs", 2);
    let plugins = names_with_suffix(&project.root.join("Plugins"), ".uplugin", 2);
    let configs = names_with_suffix(&project.root.join("Config"), ".ini", 1);
    let mut counts = Map::new();
    let mut maps = Vec::new();
    let mut seen = 0usize;
    let content = project.root.join("Content");
    count_content(&content, &content, &mut counts, &mut maps, &mut seen);
    let engine = engine_root(project);
    Ok(json!({
        "name": project.name,
        "uproject": project.uproject,
        "root": project.root,
        "engine_association": project.descriptor["EngineAssociation"],
        "engine_root": engine.as_ref().ok(),
        "engine_problem": engine.as_ref().err().map(|e| e.to_string()),
        "descriptor_modules": project.descriptor["Modules"],
        "descriptor_plugins": project.descriptor["Plugins"],
        "target_platforms": project.descriptor["TargetPlatforms"],
        "targets": targets,
        "cpp_modules": modules,
        "blueprint_only": targets.is_empty(),
        "project_plugins": plugins,
        "config_files": configs,
        "content_files_by_extension": counts,
        "content_files_scanned": seen,
        "maps": maps,
        "log": project.log_path(),
        "log_exists": project.log_path().is_file(),
    }))
}

fn count_content(base: &Path, dir: &Path, counts: &mut Map<String, Value>, maps: &mut Vec<String>, seen: &mut usize) {
    let Ok(entries) = std::fs::read_dir(dir) else { return };
    for entry in entries.flatten() {
        if *seen >= 200_000 {
            return;
        }
        let path = entry.path();
        if path.is_dir() {
            count_content(base, &path, counts, maps, seen);
            continue;
        }
        *seen += 1;
        let ext = path.extension().map(|e| e.to_string_lossy().to_lowercase()).unwrap_or_default();
        let slot = counts.entry(ext.clone()).or_insert(json!(0));
        *slot = json!(slot.as_u64().unwrap_or(0) + 1);
        if ext == "umap" && maps.len() < 200 {
            if let Ok(relative) = path.with_extension("").strip_prefix(base) {
                maps.push(format!("/Game/{}", relative.display()));
            }
        }
    }
}

fn setup_check(project: &Project, fix: bool) -> Result<Value> {
    let listed = project.descriptor["Plugins"].as_array().cloned().unwrap_or_default();
    let state = |name: &str| listed.iter().find(|p| p["Name"] == name).map(|p| p["Enabled"].as_bool().unwrap_or(false));
    let missing: Vec<&str> = BRIDGE_PLUGINS.iter().copied().filter(|name| state(name) != Some(true)).collect();
    let mut fixed = Vec::new();
    if fix && !missing.is_empty() {
        let mut descriptor = project.descriptor.clone();
        if !descriptor["Plugins"].is_array() {
            descriptor["Plugins"] = json!([]);
        }
        let plugins = descriptor["Plugins"].as_array_mut().expect("just made an array");
        for name in &missing {
            match plugins.iter_mut().find(|p| p["Name"] == *name) {
                Some(entry) => entry["Enabled"] = json!(true),
                None => plugins.push(json!({"Name": name, "Enabled": true})),
            }
            fixed.push(*name);
        }
        // Unreal writes descriptors with tabs; keeping that keeps the diff to the added lines.
        let mut out = Vec::new();
        let formatter = serde_json::ser::PrettyFormatter::with_indent(b"\t");
        let mut serializer = serde_json::Serializer::with_formatter(&mut out, formatter);
        serde::Serialize::serialize(&descriptor, &mut serializer)?;
        out.extend_from_slice(b"\n");
        std::fs::write(&project.uproject, out).with_context(|| format!("writing {}", project.uproject.display()))?;
    }
    let ini = if fix { Some(write_remote_control_ini(project)?) } else { None };
    let editor_ini = if fix { Some(write_editor_settings_ini(project)?) } else { None };
    let editor = remote("GET", "/remote/info", None, Duration::from_secs(3));
    let python_ok = match &editor {
        Ok(_) => python("print('relay-python-ok')", Duration::from_secs(20))
            .map(|v| v.to_string().contains("relay-python-ok"))
            .map_err(|e| e.to_string()),
        Err(_) => Err("editor not reachable".into()),
    };
    // The running editor's value; with fix, also switched off now, so the editor writes the
    // same value back when it quits.
    let throttled = match &python_ok {
        Ok(true) => python_json(&format!(
            "import json, unreal\ns = unreal.get_default_object(unreal.EditorPerformanceSettings)\nwas = bool(s.get_editor_property('throttle_cpu_when_not_foreground'))\n{}print('RELAY_JSON:' + json.dumps(was))\n",
            if fix { "s.set_editor_property('throttle_cpu_when_not_foreground', False)\n" } else { "" }
        ), Duration::from_secs(20)).ok().and_then(|v| v.as_bool()),
        _ => None,
    };
    let mut advice = Vec::new();
    if !missing.is_empty() && fixed.is_empty() {
        advice.push(format!("Enable {} (Edit > Plugins, or run ue_setup_check with fix=true) and restart the editor.", missing.join(", ")));
    }
    if !fixed.is_empty() {
        advice.push("The .uproject now lists the bridge plugins; the editor must be restarted to load them.".to_string());
    }
    if ini.as_ref().is_some_and(|i| i["changed"] == true) {
        advice.push("Config/DefaultRemoteControl.ini now enables the web server at start-up, remote Python and remote function calls; restart the editor (ue_editor_quit, then ue_editor_launch) to apply it. That file is shared project config: once committed, every teammate's editor serves unauthenticated remote Python on localhost. Commit it only with the team's agreement (or set a passphrase and UE_REMOTE_CONTROL_PASSPHRASE).".to_string());
    }
    if throttled == Some(true) && !fix {
        advice.push("Use Less CPU when in Background is on: the editor barely ticks while another window has focus. Run ue_setup_check with fix=true to turn it off for good (it writes Config/DefaultEditorPerProjectUserSettings.ini).".to_string());
    }
    if editor.is_err() {
        advice.push("Open the project in the editor and start the Remote Control web server: run `WebControl.StartServer` in the editor console, or turn on auto-start under Project Settings > Plugins > Remote Control. Set UE_REMOTE_CONTROL_URL if it is not on 127.0.0.1:30010.".to_string());
    } else if let Err(error) = &python_ok {
        advice.push(format!("Remote Python failed ({error}). Run ue_setup_check with fix=true (writes Config/DefaultRemoteControl.ini) and restart the editor; make sure the Python Editor Script Plugin is enabled."));
    }
    Ok(json!({
        "uproject": project.uproject,
        "bridge_plugins": BRIDGE_PLUGINS.iter().map(|name| json!({"name": name, "enabled": state(name) == Some(true) || fixed.contains(name)})).collect::<Vec<_>>(),
        "added_to_uproject": fixed,
        "remote_control_ini": ini,
        "background_throttle": {"was_on": throttled, "ini": editor_ini},
        "remote_control_url": remote_base(),
        "editor_reachable": editor.is_ok(),
        "editor_error": editor.err().map(|e| e.to_string()),
        "remote_python": python_ok.is_ok_and(|ok| ok),
        "ready": missing.is_empty() && advice.is_empty(),
        "advice": advice,
    }))
}

fn build(project: &Project, args: &Value) -> Result<Value> {
    let engine = engine_root(project)?;
    // A running editor (or a crash reporter left from one) makes UnrealBuildTool build a
    // numbered hot-reload module that the editor never loads: old code keeps running.
    let restart = args["restart_editor"] == true;
    // A restart keeps the lock from the quit through the build to the relaunch: released in
    // between, another agent could launch the editor while the build tool writes its binaries.
    let _hold = if restart { Some(LockHold::take(project, &holder_id())?) } else { None };
    let mut quit = Value::Null;
    if restart && !crate::unreal_process::editors_for(&project.uproject).is_empty() {
        quit = quit_editor_locked(project, args["save"].as_bool().unwrap_or(true), args["force"] == true)?;
    }
    let pre = crate::unreal_process::prebuild(&project.uproject, args["keep_crash_reporters"] != true);
    if pre["editor_running"].as_array().is_some_and(|a| !a.is_empty()) && args["allow_editor_open"] != true {
        bail!(
            "the editor is running this project (pid {}). Building now makes a hot-reload module the editor may never load. Use restart_editor=true (quits the editor, builds, relaunches), or quit it with ue_editor_quit first. There is no Live Coding on Linux.",
            pre["editor_running"]
        );
    }
    let script = build_script(&engine);
    anyhow::ensure!(script.is_file(), "{} does not exist", script.display());
    let targets = names_with_suffix(&project.root.join("Source"), ".Target.cs", 0);
    let target = match args["target"].as_str().filter(|t| !t.is_empty()) {
        Some(target) => target.to_string(),
        None => targets.iter().find(|t| t.ends_with("Editor")).cloned().unwrap_or_else(|| format!("{}Editor", project.name)),
    };
    let platform = args["platform"].as_str().filter(|p| !p.is_empty()).unwrap_or(host_platform()).to_string();
    let configuration = args["configuration"].as_str().filter(|c| !c.is_empty()).unwrap_or("Development").to_string();
    let mut command = Command::new(&script);
    command
        .arg(&target)
        .arg(&platform)
        .arg(&configuration)
        .arg(format!("-Project={}", project.uproject.display()))
        .arg("-WaitMutex")
        .arg("-FromMsBuild")
        .current_dir(&engine);
    let started = std::time::Instant::now();
    let output = relay_core::proc::output_with_timeout(&mut command, secs(args, "timeout_s", 3600))
        .with_context(|| format!("running {}", script.display()))?;
    let Some(output) = output else {
        bail!("the build did not finish within its timeout and was stopped");
    };
    let text = format!("{}{}", String::from_utf8_lossy(&output.stdout), String::from_utf8_lossy(&output.stderr));
    let errors: Vec<&str> = text
        .lines()
        .filter(|line| {
            let lower = line.to_lowercase();
            lower.contains("error") && !lower.contains("0 error") || lower.contains("undefined reference") || lower.contains("fatal")
        })
        .take(200)
        .collect();
    let manifest = crate::unreal_process::module_manifest(&project.root, &platform);
    let mut warnings: Vec<String> = Vec::new();
    if let Some(killed) = pre["killed_crash_reporters"].as_array().filter(|a| !a.is_empty()) {
        warnings.push(format!("stopped leftover crash reporter(s) {killed:?} that made the build tool think an editor was running"));
    }
    if let Some(others) = pre["other_crash_reporters"].as_array().filter(|a| !a.is_empty()) {
        warnings.push(format!("left crash reporter(s) {others:?} running: their command lines name no path in this project, so they may be another project's open crash dialog. If this build still made a numbered hot-reload module, close them and build again."));
    }
    if let Some(numbered) = manifest["hot_reload_modules"].as_array().filter(|a| !a.is_empty()) {
        warnings.push(format!(
            "UnrealEditor.modules points at numbered hot-reload modules {numbered:?}: the editor may run old code. Quit the editor, delete the numbered files from Binaries/{platform}, and build again."
        ));
    }
    // A failed relaunch is reported beside the build's result, not instead of it (RA-290).
    let (relaunched, relaunch_error) = if restart && output.status.success() {
        match launch_editor(project, &json!({"timeout_s": args["launch_timeout_s"]})) {
            Ok(value) => (Some(value), None),
            Err(error) => (None, Some(format!("{error:#}"))),
        }
    } else {
        (None, None)
    };
    Ok(json!({
        "success": output.status.success(),
        "warnings": warnings,
        "quit_editor": quit,
        "relaunched_editor": relaunched,
        "relaunch_error": relaunch_error,
        "exit_code": output.status.code(),
        "command": format!("{} {target} {platform} {configuration} -Project=\"{}\" -WaitMutex -FromMsBuild", script.display(), project.uproject.display()),
        "seconds": started.elapsed().as_secs(),
        "errors": errors,
        "tail": tail(&text, 150),
        "hint": if output.status.success() { Value::Null } else { json!("Fix the first error first; later ones often cascade. A 'Unable to build while Live Coding is active' error means the editor is open: build with restart_editor=true.") },
    }))
}

fn tail(text: &str, lines: usize) -> String {
    let all: Vec<&str> = text.lines().collect();
    let mut out = all[all.len().saturating_sub(lines)..].join("\n");
    if out.len() > MAX_OUTPUT {
        let mut cut = out.len() - MAX_OUTPUT;
        while !out.is_char_boundary(cut) {
            cut += 1;
        }
        out = out[cut..].to_string();
    }
    out
}

fn log(project: &Project, args: &Value) -> Result<Value> {
    let path = match args["file"].as_str().filter(|f| !f.is_empty()) {
        Some(file) => {
            anyhow::ensure!(!file.contains("..") && !file.starts_with('/'), "file must be a name under Saved/Logs");
            project.root.join("Saved/Logs").join(file)
        }
        None => project.log_path(),
    };
    let bytes = std::fs::read(&path).with_context(|| format!("reading {} — has the editor or game run yet?", path.display()))?;
    let text = String::from_utf8_lossy(&bytes);
    let lines = args["lines"].as_u64().unwrap_or(200).clamp(1, 5000) as usize;
    let filter = match args["filter"].as_str().filter(|f| !f.is_empty()) {
        Some(pattern) => Some(regex::Regex::new(pattern).with_context(|| format!("bad filter regex {pattern:?}"))?),
        None => None,
    };
    let kept: Vec<&str> = text.lines().filter(|line| filter.as_ref().is_none_or(|re| re.is_match(line))).collect();
    let total = kept.len();
    Ok(json!({
        "file": path,
        "matching_lines": total,
        // Only the tail is joined, not every match (RA-287).
        "lines": tail(&kept[total.saturating_sub(lines)..].join("\n"), lines),
    }))
}

// ---------------------------------------------------------------- the running editor

fn remote_base() -> String {
    std::env::var("UE_REMOTE_CONTROL_URL").ok().filter(|v| !v.trim().is_empty()).unwrap_or_else(|| DEFAULT_REMOTE.to_string())
}

fn editor_status() -> Result<Value> {
    match remote("GET", "/remote/info", None, Duration::from_secs(3)) {
        Ok(info) => {
            let routes = info["HttpRoutes"].as_array().map(|r| r.len());
            // Which project is open matters as much as whether the editor answers.
            let open = editor_project().ok();
            let ours = Project::find().ok().map(|p| p.uproject);
            let matches = match (&open, &ours) {
                (Some(open), Some(ours)) => Some(same_file(open, ours)),
                _ => None,
            };
            let lock = Project::find().map(|p| read_lock(&p)).unwrap_or(Value::Null);
            Ok(json!({"reachable": true, "url": remote_base(), "editor_project": open, "this_checkout": ours,
                "same_project": matches, "editor_lock": lock, "routes": routes, "info": info}))
        }
        Err(error) => {
            // Unreachable has three different causes with three different fixes.
            let project = Project::find().ok();
            let editors: Vec<u32> = project.as_ref().map(|p| crate::unreal_process::editors_for(&p.uproject).iter().map(|e| e.pid).collect()).unwrap_or_default();
            let port = crate::unreal_process::port_of(&remote_base());
            let port_free = crate::unreal_process::port_free(port);
            // The log on disk is the running editor's own; with no editor running it is an
            // old session's, whose bind failures say nothing about now.
            let candidates: Vec<String> = match &project {
                Some(p) if !editors.is_empty() => log_since(p, 0, Some(BIND_PREFILTER)),
                _ => Vec::new(),
            };
            let bind = crate::unreal_process::bind_failures(&candidates);
            let advice = if !bind.is_empty() {
                format!("The editor is running but its web server could not bind port {port} (a previous editor still held it). In the editor console run `WebControl.StopServer` and then `WebControl.StartServer` (StartServer alone does nothing), or ask the human to save and restart the editor. ue_editor_quit cannot save through a server it cannot reach, so it refuses rather than terminate the editor.")
            } else if !editors.is_empty() && port_free {
                "The editor is running but no web server is listening: run `WebControl.StartServer` in its console, or turn on auto-start in Project Settings > Plugins > Remote Control (ue_setup_check with fix=true writes that setting).".to_string()
            } else if editors.is_empty() && !port_free {
                format!("No editor is running for this project but port {port} is still held (a closed editor releases it after a while, or another program uses it). ue_editor_launch waits for it to free up.")
            } else if editors.is_empty() {
                "No editor is running for this project: start it with ue_editor_launch.".to_string()
            } else {
                "Run ue_setup_check for the full picture.".to_string()
            };
            Ok(json!({
                "reachable": false,
                "url": remote_base(),
                "error": format!("{error:#}"),
                "editor_processes": editors,
                "port_free": port_free,
                "bind_failures": bind,
                "advice": advice,
            }))
        }
    }
}

/// Run editor Python through `ExecutePythonCommandEx`, which returns the command's log output
/// alongside its result instead of only a success flag.
fn python(code: &str, timeout: Duration) -> Result<Value> {
    python_tx(code, timeout, true)
}

/// `transaction: false` for calls around play sessions: starting Play In Editor inside a
/// Remote Control transaction logs "Cancelling Open Transaction" on every start.
fn python_tx(code: &str, timeout: Duration, transaction: bool) -> Result<Value> {
    python_run(code, timeout, transaction).map(|(result, _)| result)
}

/// The result `python_tx` returns, with its output cut to a readable tail, and the whole output
/// beside it: a tool's `RELAY_JSON:` line is often longer than that tail.
fn python_run(code: &str, timeout: Duration, transaction: bool) -> Result<(Value, String)> {
    let body = json!({
        "objectPath": PYTHON_LIBRARY,
        "functionName": "ExecutePythonCommandEx",
        "parameters": {"PythonCommand": code, "ExecutionMode": "ExecuteFile", "FileExecutionScope": "Private"},
        "generateTransaction": transaction,
    });
    let response = remote("PUT", "/remote/object/call", Some(&body), timeout)?;
    let output = python_output(&response);
    let ok = response["ReturnValue"].as_bool().unwrap_or(false);
    let result = json!({
        "ok": ok,
        "output": tail(&output, 2000),
        "result": capped(response["CommandResult"].clone()),
    });
    if !ok {
        bail!("Python failed:\n{}", serde_json::to_string_pretty(&result)?);
    }
    Ok((result, output))
}

/// Everything an `ExecutePythonCommandEx` reply printed, in order and uncut.
fn python_output(response: &Value) -> String {
    let lines: Vec<String> = response["LogOutput"]
        .as_array()
        .map(|entries| entries.iter().map(|e| {
            let kind = e["Type"].as_str().unwrap_or("Info");
            let text = e["Output"].as_str().unwrap_or("").trim_end();
            if kind == "Info" { text.to_string() } else { format!("[{kind}] {text}") }
        }).collect())
        .unwrap_or_default();
    lines.join("\n")
}

/// The JSON of the last `RELAY_JSON:` line in a script's output. Searched line by line, since
/// one log entry can hold several lines.
fn relay_json(output: &str) -> Option<&str> {
    output.lines().rev().find_map(|line| line.trim().strip_prefix("RELAY_JSON:"))
}

/// Run a script that prints one line `RELAY_JSON:<json>` and return that JSON.
fn python_json(code: &str, timeout: Duration) -> Result<Value> {
    python_json_tx(code, timeout, true)
}

fn python_json_tx(code: &str, timeout: Duration, transaction: bool) -> Result<Value> {
    // Found in the whole output: cutting it first lost every result over the tail's size.
    let (result, output) = python_run(code, timeout, transaction)?;
    let line = relay_json(&output)
        .ok_or_else(|| anyhow!("the editor script printed no result:\n{}", result["output"].as_str().unwrap_or("")))?;
    Ok(serde_json::from_str(line)?)
}

fn search_assets_script(args: &Value) -> String {
    let query = args["query"].as_str().unwrap_or("");
    let classes: Vec<String> = args["class_names"].as_array().map(|a| a.iter().filter_map(|v| v.as_str().map(str::to_string)).collect()).unwrap_or_default();
    let mut paths: Vec<String> = args["package_paths"].as_array().map(|a| a.iter().filter_map(|v| v.as_str().map(str::to_string)).collect()).unwrap_or_default();
    if paths.is_empty() {
        paths.push("/Game".into());
    }
    let limit = args["limit"].as_u64().unwrap_or(100).clamp(1, 2000);
    format!(r#"import unreal, json
query = {query}.lower()
classes = set(c.lower() for c in {classes})
registry = unreal.AssetRegistryHelpers.get_asset_registry()
found = []
for root in {paths}:
    for data in registry.get_assets_by_path(root, recursive=True):
        cls_path = getattr(data, "asset_class_path", None)
        cls = str(cls_path.asset_name) if cls_path is not None else str(data.asset_class)
        name = str(data.asset_name)
        if query and query not in name.lower():
            continue
        if classes and cls.lower() not in classes:
            continue
        found.append({{"name": name, "class": cls, "package": str(data.package_name), "object_path": str(data.package_name) + "." + name}})
        if len(found) >= {limit}:
            break
    if len(found) >= {limit}:
        break
print("RELAY_JSON:" + json.dumps({{"count": len(found), "limit": {limit}, "assets": found}}))
"#, query = py_str(query), classes = serde_json::to_string(&classes).unwrap(), paths = serde_json::to_string(&paths).unwrap())
}

fn level_actors_script(args: &Value) -> String {
    let class_filter = args["class_filter"].as_str().unwrap_or("");
    let name_filter = args["name_filter"].as_str().unwrap_or("");
    let selected = if args["selected_only"].as_bool().unwrap_or(false) { "True" } else { "False" };
    let limit = args["limit"].as_u64().unwrap_or(200).clamp(1, 5000);
    format!(r#"import unreal, json
actors_sub = unreal.get_editor_subsystem(unreal.EditorActorSubsystem)
actors = actors_sub.get_selected_level_actors() if {selected} else actors_sub.get_all_level_actors()
cf = {class_filter}.lower()
nf = {name_filter}.lower()
out = []
for actor in actors:
    cls = actor.get_class().get_name()
    label = actor.get_actor_label()
    if cf and cf not in cls.lower():
        continue
    if nf and nf not in label.lower():
        continue
    loc = actor.get_actor_location()
    out.append({{"label": label, "class": cls, "path": actor.get_path_name(), "folder": str(actor.get_folder_path()), "location": [round(loc.x, 2), round(loc.y, 2), round(loc.z, 2)]}})
    if len(out) >= {limit}:
        break
world = unreal.get_editor_subsystem(unreal.UnrealEditorSubsystem).get_editor_world()
print("RELAY_JSON:" + json.dumps({{"level": world.get_path_name() if world else None, "total_in_level": len(actors), "count": len(out), "actors": out}}))
"#, class_filter = py_str(class_filter), name_filter = py_str(name_filter))
}

/// One HTTP/1.1 request to the Remote Control server. It is plain HTTP on localhost, so a
/// small client is all this needs; the reply is JSON.
fn remote(method: &str, path: &str, body: Option<&Value>, timeout: Duration) -> Result<Value> {
    remote_at(&remote_base(), method, path, body, timeout)
}

fn remote_at(base: &str, method: &str, path: &str, body: Option<&Value>, timeout: Duration) -> Result<Value> {
    let (host, name, port) = crate::unreal_process::remote_addr(base)
        .ok_or_else(|| anyhow!("UE_REMOTE_CONTROL_URL must be an http:// URL, got {base}"))?;
    let sockets: Vec<_> = (name.as_str(), port).to_socket_addrs().with_context(|| format!("resolving {host}"))?.collect();
    anyhow::ensure!(!sockets.is_empty(), "{host} resolves to nothing");
    // Every address in turn: `localhost` resolves to ::1 first while the editor listens on
    // 127.0.0.1 (RA-291).
    let mut connected = Err(std::io::Error::other("no address tried"));
    for socket in &sockets {
        connected = TcpStream::connect_timeout(socket, Duration::from_secs(3));
        if connected.is_ok() {
            break;
        }
    }
    let mut stream = connected
        .with_context(|| format!("the Unreal editor is not answering at {base} (Remote Control web server not running?)"))?;
    stream.set_read_timeout(Some(timeout))?;
    stream.set_write_timeout(Some(Duration::from_secs(10)))?;
    let payload = body.map(serde_json::to_vec).transpose()?.unwrap_or_default();
    let mut request = format!(
        "{method} {path} HTTP/1.1\r\nHost: {host}\r\nAccept: application/json\r\nConnection: close\r\nContent-Length: {}\r\n",
        payload.len()
    );
    if body.is_some() {
        request.push_str("Content-Type: application/json\r\n");
    }
    if let Ok(passphrase) = std::env::var("UE_REMOTE_CONTROL_PASSPHRASE") {
        request.push_str(&format!("Passphrase: {passphrase}\r\n"));
    }
    request.push_str("\r\n");
    stream.write_all(request.as_bytes())?;
    stream.write_all(&payload)?;
    let mut raw = Vec::new();
    (&mut stream).take(MAX_REPLY + 1).read_to_end(&mut raw).context("reading the editor's reply (a long editor operation may need a larger timeout)")?;
    anyhow::ensure!(
        raw.len() as u64 <= MAX_REPLY,
        "the editor's reply is larger than {} MB and was not read; print less (a summary, or the first rows), or write the data to a file in the project and read that",
        MAX_REPLY / (1024 * 1024)
    );
    let (status, body) = parse_response(&raw)?;
    let text = String::from_utf8_lossy(&body).into_owned();
    let value: Value = if text.trim().is_empty() { json!({}) } else { serde_json::from_str(&text).unwrap_or(json!({"body": text})) };
    if !(200..300).contains(&status) {
        bail!("Remote Control answered HTTP {status}: {}", serde_json::to_string(&value)?);
    }
    Ok(value)
}

/// A Remote Control value passed straight back to the agent, cut to its first `MAX_OUTPUT`
/// bytes of JSON when longer: a large array or map property would fill its context (RA-292).
fn capped(value: Value) -> Value {
    let text = value.to_string();
    if text.len() <= MAX_OUTPUT {
        return value;
    }
    let mut cut = MAX_OUTPUT;
    while !text.is_char_boundary(cut) {
        cut -= 1;
    }
    json!({"truncated": true, "bytes": text.len(), "json_head": &text[..cut],
        "note": "the editor's reply was too long to return whole; read a narrower property, or use ue_python to print what you need"})
}

fn parse_response(raw: &[u8]) -> Result<(u16, Vec<u8>)> {
    let split = raw.windows(4).position(|w| w == b"\r\n\r\n").ok_or_else(|| anyhow!("malformed HTTP reply"))?;
    let head = String::from_utf8_lossy(&raw[..split]);
    let mut body = raw[split + 4..].to_vec();
    let status = head
        .lines()
        .next()
        .and_then(|line| line.split_whitespace().nth(1))
        .and_then(|code| code.parse().ok())
        .ok_or_else(|| anyhow!("malformed HTTP status line"))?;
    let chunked = head.lines().any(|line| {
        let lower = line.to_ascii_lowercase();
        lower.starts_with("transfer-encoding:") && lower.contains("chunked")
    });
    if chunked {
        body = dechunk(&body)?;
    }
    Ok((status, body))
}

fn dechunk(mut raw: &[u8]) -> Result<Vec<u8>> {
    let mut out = Vec::new();
    loop {
        let end = raw.windows(2).position(|w| w == b"\r\n").ok_or_else(|| anyhow!("malformed chunk"))?;
        let size_text = String::from_utf8_lossy(&raw[..end]);
        let size = usize::from_str_radix(size_text.split(';').next().unwrap_or("").trim(), 16)?;
        raw = &raw[end + 2..];
        if size == 0 {
            return Ok(out);
        }
        anyhow::ensure!(raw.len() >= size, "truncated chunk");
        out.extend_from_slice(&raw[..size]);
        raw = raw.get(size + 2..).unwrap_or(&[]);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::net::TcpListener;
    use std::os::unix::process::ExitStatusExt;
    use std::process::Stdio;

    fn project(dir: &Path, name: &str, descriptor: Value) {
        std::fs::write(dir.join(format!("{name}.uproject")), serde_json::to_string_pretty(&descriptor).unwrap()).unwrap();
    }

    #[test]
    fn every_tool_in_the_manifest_is_served_and_nothing_else() {
        let manifest: Value = serde_json::from_str(include_str!("../../../plugins/unreal-engine/plugin.json")).unwrap();
        let declared: Vec<&str> = manifest["mcp_servers"][0]["tools"].as_array().unwrap().iter().map(|t| t.as_str().unwrap()).collect();
        let served: Vec<Value> = tools();
        let served: Vec<&str> = served.iter().map(|t| t["name"].as_str().unwrap()).collect();
        assert_eq!(declared, served);
    }

    #[test]
    fn initialize_and_list_speak_mcp() {
        let init = handle(&json!({"jsonrpc":"2.0","id":1,"method":"initialize","params":{"protocolVersion":"2025-06-18"}})).unwrap();
        assert_eq!(init["result"]["serverInfo"]["name"], "unreal");
        let listed = handle(&json!({"jsonrpc":"2.0","id":2,"method":"tools/list"})).unwrap();
        assert_eq!(listed["result"]["tools"].as_array().unwrap().len(), 25);
        assert!(handle(&json!({"jsonrpc":"2.0","method":"notifications/initialized"})).is_none());
        let unknown = handle(&json!({"jsonrpc":"2.0","id":3,"method":"tools/call","params":{"name":"nope"}})).unwrap();
        assert_eq!(unknown["error"]["code"], -32602);
    }

    #[test]
    fn project_discovery_skips_generated_folders_and_reads_the_layout() {
        let root = tempfile::tempdir().unwrap();
        std::fs::create_dir_all(root.path().join("Intermediate/Stale")).unwrap();
        project(&root.path().join("Intermediate/Stale"), "Wrong", json!({}));
        let game = root.path().join("game");
        std::fs::create_dir_all(game.join("Source/Shooter")).unwrap();
        std::fs::create_dir_all(game.join("Content/Maps")).unwrap();
        std::fs::create_dir_all(game.join("Config")).unwrap();
        project(&game, "Shooter", json!({"FileVersion":3,"EngineAssociation":"5.4","Modules":[{"Name":"Shooter","Type":"Runtime"}]}));
        std::fs::write(game.join("Source/ShooterEditor.Target.cs"), "").unwrap();
        std::fs::write(game.join("Source/Shooter.Target.cs"), "").unwrap();
        std::fs::write(game.join("Source/Shooter/Shooter.Build.cs"), "").unwrap();
        std::fs::write(game.join("Content/Maps/Arena.umap"), "").unwrap();
        std::fs::write(game.join("Config/DefaultEngine.ini"), "").unwrap();
        assert_eq!(find_uproject(root.path()).unwrap(), game.join("Shooter.uproject"));

        let found = Project { uproject: game.join("Shooter.uproject"), root: game.clone(), name: "Shooter".into(), descriptor: json!({"EngineAssociation":"5.4"}) };
        let info = project_info(&found).unwrap();
        assert_eq!(info["targets"], json!(["Shooter", "ShooterEditor"]));
        assert_eq!(info["cpp_modules"], json!(["Shooter"]));
        assert_eq!(info["maps"], json!(["/Game/Maps/Arena"]));
        assert_eq!(info["config_files"], json!(["DefaultEngine"]));
        assert_eq!(info["blueprint_only"], false);
    }

    #[test]
    fn setup_fix_adds_only_the_missing_bridge_plugins() {
        let root = tempfile::tempdir().unwrap();
        project(root.path(), "Game", json!({"FileVersion":3,"Plugins":[{"Name":"PythonScriptPlugin","Enabled":true},{"Name":"RemoteControl","Enabled":false}]}));
        let found = Project {
            uproject: root.path().join("Game.uproject"),
            root: root.path().to_path_buf(),
            name: "Game".into(),
            descriptor: serde_json::from_str(&std::fs::read_to_string(root.path().join("Game.uproject")).unwrap()).unwrap(),
        };
        // SAFETY: every test that sets this variable sets it to the same closed port, so the
        // live checks never reach a real editor whichever runs first.
        unsafe { std::env::set_var("UE_REMOTE_CONTROL_URL", CLOSED_REMOTE) };
        let report = setup_check(&found, true).unwrap();
        assert_eq!(report["added_to_uproject"], json!(["RemoteControl", "EditorScriptingUtilities"]));
        assert_eq!(report["editor_reachable"], false);
        let written: Value = serde_json::from_str(&std::fs::read_to_string(root.path().join("Game.uproject")).unwrap()).unwrap();
        let plugins = written["Plugins"].as_array().unwrap();
        assert_eq!(plugins.len(), 3);
        assert!(plugins.iter().all(|p| p["Enabled"] == true));
        assert_eq!(written["FileVersion"], 3, "other fields survive");
    }

    #[test]
    fn remote_calls_reach_a_remote_control_server_and_decode_chunked_replies() {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let port = listener.local_addr().unwrap().port();
        let server = std::thread::spawn(move || {
            let (mut stream, _) = listener.accept().unwrap();
            let mut buffer = vec![0u8; 8192];
            let mut request = Vec::new();
            loop {
                let n = stream.read(&mut buffer).unwrap();
                request.extend_from_slice(&buffer[..n]);
                let text = String::from_utf8_lossy(&request);
                if let Some(split) = text.find("\r\n\r\n") {
                    let length: usize = text.lines().find_map(|l| l.strip_prefix("Content-Length: ")).unwrap().trim().parse().unwrap();
                    if request.len() >= split + 4 + length { break; }
                }
            }
            let body = r#"{"ReturnValue":true,"CommandResult":"None","LogOutput":[{"Type":"Info","Output":"RELAY_JSON:{\"count\":1}"}]}"#;
            let reply = format!("HTTP/1.1 200 OK\r\nTransfer-Encoding: chunked\r\n\r\n{:x}\r\n{body}\r\n0\r\n\r\n", body.len());
            stream.write_all(reply.as_bytes()).unwrap();
            String::from_utf8(request).unwrap()
        });
        let url = format!("http://127.0.0.1:{port}");
        let body = json!({"objectPath": PYTHON_LIBRARY, "functionName": "ExecutePythonCommandEx", "parameters": {"PythonCommand": "print(1)"}});
        let result = remote_at(&url, "PUT", "/remote/object/call", Some(&body), Duration::from_secs(5)).unwrap();
        let request = server.join().unwrap();
        assert!(request.starts_with("PUT /remote/object/call HTTP/1.1\r\n"));
        assert!(request.contains("ExecutePythonCommandEx"));
        assert_eq!(result["ReturnValue"], true);
        assert_eq!(result["LogOutput"][0]["Output"], "RELAY_JSON:{\"count\":1}");
    }

    /// Nothing listens here, and it is above 1024 so the quit's port check can bind it.
    const CLOSED_REMOTE: &str = "http://127.0.0.1:39917";

    #[test]
    fn an_editor_that_cannot_be_asked_to_save_is_left_running() {
        let root = tempfile::tempdir().unwrap();
        let project = bare_project(root.path());
        std::fs::write(&project.uproject, "{}").unwrap();
        // A stand-in editor: any long-running binary named UnrealEditor with this project open.
        let binary = root.path().join("UnrealEditor");
        std::fs::copy("/usr/bin/tail", &binary).unwrap();
        // A test forking at the same moment can hold the fresh copy open for writing a moment.
        let mut editor = (0..50)
            .find_map(|_| match Command::new(&binary).arg("-f").arg(&project.uproject).stdout(Stdio::null()).spawn() {
                Err(e) if e.kind() == std::io::ErrorKind::ExecutableFileBusy => { std::thread::sleep(Duration::from_millis(20)); None }
                other => Some(other.unwrap()),
            })
            .expect("the stand-in editor stayed busy");
        // SAFETY: see setup_fix_adds_only_the_missing_bridge_plugins.
        unsafe { std::env::set_var("UE_REMOTE_CONTROL_URL", CLOSED_REMOTE) };
        let deadline = std::time::Instant::now() + Duration::from_secs(5);
        while crate::unreal_process::editors_for(&project.uproject).is_empty() && std::time::Instant::now() < deadline {
            std::thread::sleep(Duration::from_millis(50));
        }

        let refused = quit_editor(&project, true, false).unwrap_err().to_string();
        assert!(refused.contains("left running"), "{refused}");
        assert!(editor.try_wait().unwrap().is_none(), "the editor was signalled");
        assert!(read_lock(&project)["holder"].is_null(), "a refusal leaves the editor lock behind");
        let refused = quit_editor(&project, false, false).unwrap_err().to_string();
        assert!(refused.contains("force=true"), "{refused}");
        assert!(editor.try_wait().unwrap().is_none());

        let forced = quit_editor(&project, true, true).unwrap();
        assert_eq!(forced["stopped"], json!([editor.id()]), "{forced}");
        assert_eq!(forced["saved"], false);
        assert!(editor.wait().unwrap().signal().is_some());
    }

    fn bare_project(root: &Path) -> Project {
        Project { uproject: root.join("Game.uproject"), root: root.to_path_buf(), name: "Game".into(), descriptor: json!({}) }
    }

    #[test]
    fn one_agent_drives_the_editor_at_a_time() {
        let root = tempfile::tempdir().unwrap();
        let project = bare_project(root.path());
        acquire_lock(&project, "calm-otter").unwrap();
        acquire_lock(&project, "calm-otter").expect("the holder keeps its own lock");
        let refused = acquire_lock(&project, "brisk-fox").unwrap_err().to_string();
        assert!(refused.contains("calm-otter"), "{refused}");
        // blender_to_unreal's preflight asks the same question without taking the lock.
        assert!(lock_free_for(&read_lock(&project), "brisk-fox").is_err());
        assert!(lock_free_for(&read_lock(&project), "calm-otter").is_ok());
        assert!(release_lock(&project, "brisk-fox").is_err(), "only the holder releases");
        release_lock(&project, "calm-otter").unwrap();
        acquire_lock(&project, "brisk-fox").unwrap();

        // An abandoned lock frees itself.
        std::fs::write(lock_path(&project), json!({"holder":"brisk-fox","since":1,"last_used":1}).to_string()).unwrap();
        assert_eq!(read_lock(&project)["expired"], true);
        acquire_lock(&project, "calm-otter").unwrap();
        assert_eq!(read_lock(&project)["holder"], "calm-otter");
    }

    #[test]
    fn two_agents_never_both_take_a_free_editor() {
        let root = tempfile::tempdir().unwrap();
        let project = std::sync::Arc::new(bare_project(root.path()));
        let start = std::sync::Arc::new(std::sync::Barrier::new(8));
        let agents: Vec<_> = (0..8).map(|n| {
            let (project, start) = (project.clone(), start.clone());
            std::thread::spawn(move || {
                start.wait();
                acquire_lock(&project, &format!("agent-{n}")).is_ok()
            })
        }).collect();
        let took = agents.into_iter().map(|a| a.join().unwrap()).filter(|ok| *ok).count();
        assert_eq!(took, 1, "exactly one agent holds the editor");
        assert!(read_lock(&project)["holder"].as_str().unwrap().starts_with("agent-"));
    }

    #[test]
    fn an_ini_that_cannot_be_read_is_left_as_it_was() {
        let root = tempfile::tempdir().unwrap();
        let project = bare_project(root.path());
        // Latin-1, as another tool may have saved it: not UTF-8.
        let latin1 = b"[/Script/Other]\nTitle=Caf\xe9\n".to_vec();
        let config = root.path().join("Config");
        std::fs::create_dir_all(&config).unwrap();
        std::fs::write(config.join("DefaultEditorPerProjectUserSettings.ini"), &latin1).unwrap();
        let keys = [(PERF_KEY.to_string(), "False".to_string())];
        assert!(merge_ini_file(&config.join("DefaultEditorPerProjectUserSettings.ini"), PERF_SECTION, &keys).is_err());
        let result = write_editor_settings_ini(&project).unwrap();
        assert_eq!(result["changed"], json!([]), "{result}");
        assert_eq!(result["skipped"].as_array().unwrap().len(), 1, "{result}");
        assert_eq!(std::fs::read(config.join("DefaultEditorPerProjectUserSettings.ini")).unwrap(), latin1, "the file was replaced");
        // A missing file is simply created.
        assert!(merge_ini_file(&config.join("New.ini"), PERF_SECTION, &keys).unwrap());
        assert!(!merge_ini_file(&config.join("New.ini"), PERF_SECTION, &keys).unwrap());
        assert!(!config.join("New.ini.relay-tmp").exists());
    }

    #[test]
    fn a_long_remote_value_is_cut_before_it_reaches_the_agent() {
        assert_eq!(capped(json!({"a": 1})), json!({"a": 1}));
        let long = json!({"Items": (0..20_000).map(|n| format!("é{n}")).collect::<Vec<_>>()});
        let cut = capped(long.clone());
        assert_eq!(cut["truncated"], true);
        assert_eq!(cut["bytes"], long.to_string().len());
        assert!(cut["json_head"].as_str().unwrap().len() <= MAX_OUTPUT);
    }

    #[test]
    fn a_host_name_is_tried_at_every_address_it_resolves_to() {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let port = listener.local_addr().unwrap().port();
        let server = std::thread::spawn(move || {
            let (mut stream, _) = listener.accept().unwrap();
            let mut request = Vec::new();
            let mut buffer = [0u8; 4096];
            while !request.windows(4).any(|w| w == b"\r\n\r\n") {
                let n = stream.read(&mut buffer).unwrap();
                request.extend_from_slice(&buffer[..n]);
            }
            stream.write_all(b"HTTP/1.1 200 OK\r\nContent-Length: 11\r\n\r\n{\"ok\":true}").unwrap();
        });
        // `localhost` may resolve to ::1 first, where nothing listens.
        let reply = remote_at(&format!("http://localhost:{port}/"), "GET", "/remote/info", None, Duration::from_secs(5)).unwrap();
        server.join().unwrap();
        assert_eq!(reply["ok"], true);
    }

    #[test]
    fn captured_images_become_image_content_and_the_folder_is_removed() {
        let root = tempfile::tempdir().unwrap();
        // Where capture_dir puts it: mcp::tool_result deletes no other folder.
        let dir = root.path().join("Saved/Relay/Captures").join(uuid::Uuid::new_v4().to_string());
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(dir.join("shot_front.png"), b"\x89PNG fake").unwrap();
        let value = with_images(Ok(json!({"files":[
            {"view":"front","file":dir.join("shot_front.png")},
            {"view":"right","file":dir.join("shot_right.png")}
        ]})), &dir).unwrap();
        let result = crate::mcp::tool_result(value, false);
        let content = result["content"].as_array().unwrap();
        let images: Vec<&Value> = content.iter().filter(|c| c["type"] == "image").collect();
        assert_eq!(images.len(), 1);
        assert_eq!(images[0]["mimeType"], "image/png");
        assert_eq!(result["structuredContent"]["images"]["shown"], json!(["front"]));
        assert!(result["structuredContent"].get("_images").is_none());
        assert!(!dir.exists(), "capture folder left behind");
    }

    #[test]
    fn scripts_carry_their_arguments_and_the_shared_helpers() {
        let code = script(PY_ANIM_INSPECT, &json!({"mesh":"/Game/It's"}));
        assert!(code.starts_with("ARGS_JSON = \"{\\\"mesh\\\":\\\"/Game/It's\\\"}\"\n"), "{}", &code[..80]);
        assert!(code.contains("def body_frame(skel):"));
        assert!(code.trim_end().ends_with("})"));
    }

    /// The pose maths is Python that runs inside the editor; `tests/run_inspect.py` runs it
    /// against a stand-in `unreal` module. Skipped only where no python3 exists.
    #[test]
    fn animation_checks_hold_against_a_stand_in_editor() {
        let runner = Path::new(env!("CARGO_MANIFEST_DIR")).join("src/unreal_py/tests/run_inspect.py");
        let Ok(output) = Command::new("python3").arg(&runner).output() else {
            eprintln!("python3 not found; skipping");
            return;
        };
        assert!(output.status.success(), "{}{}", String::from_utf8_lossy(&output.stdout), String::from_utf8_lossy(&output.stderr));
    }

    #[test]
    fn remote_control_settings_merge_into_the_projects_ini() {
        let keys = vec![("bEnableRemotePythonExecution".to_string(), "True".to_string()), ("bAutoStartWebServer".to_string(), "True".to_string())];
        let fresh = merge_ini("", RC_SECTION, &keys);
        assert_eq!(fresh, format!("{RC_SECTION}\nbEnableRemotePythonExecution=True\nbAutoStartWebServer=True\n"));
        let existing = format!("[Other]\nA=1\n\n{RC_SECTION}\nbEnableRemotePythonExecution=False\nRemoteControlHttpServerPort=30010\n\n[Later]\nB=2\n");
        let merged = merge_ini(&existing, RC_SECTION, &keys);
        assert!(merged.contains("bEnableRemotePythonExecution=True\n"));
        assert!(!merged.contains("=False"));
        assert!(merged.contains("RemoteControlHttpServerPort=30010\nbAutoStartWebServer=True\n\n[Later]\nB=2"), "{merged}");
        assert!(merged.starts_with("[Other]\nA=1\n"));
        assert_eq!(merge_ini(&merged, RC_SECTION, &keys), merged, "merging twice changes nothing");
    }

    #[test]
    fn the_background_throttle_stays_off_across_restarts() {
        let root = tempfile::tempdir().unwrap();
        let project = bare_project(root.path());
        let saved = root.path().join("Saved/Config/LinuxEditor");
        std::fs::create_dir_all(&saved).unwrap();
        std::fs::write(saved.join("EditorPerProjectUserSettings.ini"), format!("{PERF_SECTION}\n{PERF_KEY}=True\nbMonitorEditorPerformance=True\n")).unwrap();
        let other = root.path().join("Saved/Config/WindowsEditor");
        std::fs::create_dir_all(&other).unwrap();
        std::fs::write(other.join("EditorPerProjectUserSettings.ini"), "[Other]\nA=1\n").unwrap();
        let result = write_editor_settings_ini(&project).unwrap();
        assert_eq!(result["changed"].as_array().unwrap().len(), 2, "{result}");
        let default = std::fs::read_to_string(root.path().join("Config/DefaultEditorPerProjectUserSettings.ini")).unwrap();
        assert_eq!(default, format!("{PERF_SECTION}\n{PERF_KEY}=False\n"));
        let user = std::fs::read_to_string(saved.join("EditorPerProjectUserSettings.ini")).unwrap();
        assert!(user.contains(&format!("{PERF_KEY}=False\nbMonitorEditorPerformance=True")), "{user}");
        assert_eq!(std::fs::read_to_string(other.join("EditorPerProjectUserSettings.ini")).unwrap(), "[Other]\nA=1\n", "a file without the key is left to the default");
        assert_eq!(write_editor_settings_ini(&project).unwrap()["changed"], json!([]), "a second run changes nothing");
    }

    #[test]
    fn profiles_summarize_timing_columns_and_skip_metadata_rows() {
        let csv = "FrameTime,GameThreadTime,RenderThreadTime,GPUTime,Other\n\
                   16.0,10.0,8.0,15.0,1\n\
                   18.0,12.0,8.0,17.0,1\n\
                   40.0,30.0,9.0,20.0,1\n\
                   [HasHeaderRowAtEnd],1\n\
                   FrameTime,GameThreadTime,RenderThreadTime,GPUTime,Other\n";
        let s = summarize_csv(csv);
        assert_eq!(s["frames"], 3);
        assert_eq!(s["stats"]["FrameTime"]["max_ms"], 40.0);
        assert_eq!(s["stats"]["GameThreadTime"]["avg_ms"], 17.33);
        assert!(s["stats"].get("Other").is_none());
        assert_eq!(s["worst_frames"][0]["ms"], 40.0);
    }

    #[test]
    fn in_editor_test_results_are_read_from_the_log() {
        let lines = vec![
            "LogAutomationController: Display: Test Completed. Result={Success} Name={Adds} Path={Project.Inventory.Adds}".to_string(),
            "LogAutomationController: Error: Test Completed. Result={Fail} Name={Stacks} Path={Project.Inventory.Stacks}".to_string(),
            "LogAutomationController: Display: ...Automation Test Queue Empty 2 tests performed.".to_string(),
        ];
        let v = test_lines(&lines);
        assert_eq!(v["succeeded"], 1);
        assert_eq!(v["failed"], 1);
        assert_eq!(v["tests"][1]["test"], "Project.Inventory.Stacks");
    }

    #[test]
    fn crashes_are_read_newest_first() {
        let root = tempfile::tempdir().unwrap();
        let project = bare_project(root.path());
        let old = root.path().join("Saved/Crashes/UECC-old");
        let new = root.path().join("Saved/Crashes/UECC-new");
        std::fs::create_dir_all(&old).unwrap();
        std::thread::sleep(Duration::from_millis(20));
        std::fs::create_dir_all(&new).unwrap();
        std::fs::write(new.join("CrashContext.runtime-xml"), "<Root><ErrorMessage>Assertion failed: Health &gt; 0</ErrorMessage><CallStack>MyGame!AHero::Tick()\nUnrealEditor-Engine!AActor::Tick()</CallStack></Root>").unwrap();
        std::fs::write(new.join("Game.log"), "line 1\nFatal error\n").unwrap();
        let c = crash(&project, 0).unwrap();
        assert_eq!(c["error"], "Assertion failed: Health > 0");
        assert!(c["callstack"].as_str().unwrap().starts_with("MyGame!AHero::Tick()"));
        assert!(c["log_tail"].as_str().unwrap().contains("Fatal error"));
        assert_eq!(c["of"], 2);
    }

    #[test]
    fn python_scripts_embed_arguments_as_literals() {
        let script = level_actors_script(&json!({"name_filter":"it's \"quoted\"\n","limit":5}));
        assert!(script.contains(r#"nf = "it's \"quoted\"\n".lower()"#));
        assert!(script.contains("if len(out) >= 5:"));
        let search = search_assets_script(&json!({"class_names":["StaticMesh"]}));
        assert!(search.contains(r#"for root in ["/Game"]:"#));
        assert!(search.contains(r#"set(c.lower() for c in ["StaticMesh"])"#));
    }

    #[test]
    fn the_editor_lock_works_without_an_editor() {
        // Releasing a lock must work after the editor has crashed, so the lock tool needs none.
        assert!(!LIVE.contains(&"ue_editor_lock"));
        let served: Vec<Value> = tools();
        let served: Vec<&str> = served.iter().map(|t| t["name"].as_str().unwrap()).collect();
        for name in LIVE.iter().chain(MUTATING.iter()) {
            assert!(served.contains(name), "{name} is not a tool");
        }
        assert!(MUTATING.iter().all(|m| LIVE.contains(m)), "a mutating tool skips the project guard");
    }

    #[test]
    fn a_held_lock_stays_fresh_until_the_call_ends() {
        let root = tempfile::tempdir().unwrap();
        let project = bare_project(root.path());
        let stale = |holder: &str| std::fs::write(lock_path(&project), json!({"holder":holder,"since":1,"last_used":1}).to_string()).unwrap();
        let hold = LockHold::take_every(&project, "calm-otter", Duration::from_millis(20)).unwrap();
        // As if the call had been running for longer than LOCK_IDLE.
        stale("calm-otter");
        std::thread::sleep(Duration::from_millis(200));
        let lock = read_lock(&project);
        assert_eq!(lock["expired"], false, "{lock}");
        assert_eq!(lock["since"], 1, "the hold keeps when it was taken");
        assert!(acquire_lock(&project, "brisk-fox").is_err(), "taken over mid-call");
        drop(hold);
        assert!(read_lock(&project)["idle_s"].as_u64().unwrap() < 5, "idleness counts from the end of the call");
        // Once dropped, nothing refreshes it.
        stale("calm-otter");
        std::thread::sleep(Duration::from_millis(100));
        assert_eq!(read_lock(&project)["expired"], true);

        // A lock that changed hands is not written back.
        let hold = LockHold::take_every(&project, "calm-otter", Duration::from_millis(20)).unwrap();
        stale("brisk-fox");
        std::thread::sleep(Duration::from_millis(100));
        drop(hold);
        assert_eq!(read_lock(&project)["holder"], "brisk-fox");
        assert_eq!(read_lock(&project)["last_used"], 1);
    }

    fn write_log(project: &Project, text: &str) {
        std::fs::create_dir_all(project.log_path().parent().unwrap()).unwrap();
        std::fs::write(project.log_path(), text).unwrap();
    }

    #[test]
    fn log_lines_are_filtered_before_they_are_capped() {
        let root = tempfile::tempdir().unwrap();
        let project = bare_project(root.path());
        let mut text = String::from("LogTemp: old session\n");
        let start = text.len() as u64;
        text.push_str("LogBlueprint: Error: the first error, the cause\n");
        for n in 0..2000 {
            text.push_str(&format!("LogTemp: Display: noise {n}\nLogScript: Warning: repeated {n}\n"));
        }
        write_log(&project, &text);
        let all = log_since(&project, start, Some("Error|Warning"));
        assert_eq!(all.len(), 2001, "every match, not a tail");
        assert!(all[0].contains("the first error"));
        let capped = cap_lines(all, MAX_LOG_LINES);
        assert_eq!(capped.len(), MAX_LOG_LINES + 1);
        assert!(capped[0].contains("the first error"), "the first errors are kept");
        assert!(capped[MAX_LOG_LINES / 2].contains("1701 matching lines omitted"), "{}", capped[MAX_LOG_LINES / 2]);
        assert!(capped.last().unwrap().contains("repeated 1999"));
        assert_eq!(cap_lines(vec!["a".into()], MAX_LOG_LINES), vec!["a".to_string()]);
    }

    #[test]
    fn in_editor_results_count_every_test() {
        let root = tempfile::tempdir().unwrap();
        let project = bare_project(root.path());
        let mut text = String::new();
        for n in 0..400 {
            let result = if n == 0 { "Fail" } else { "Success" };
            text.push_str(&format!("LogAutomationController: Display: Test Started. Name={{T{n}}}\n"));
            text.push_str(&format!("LogAutomationController: Display: Test Completed. Result={{{result}}} Name={{T{n}}} Path={{Project.T{n}}}\n"));
        }
        text.push_str("LogAutomationController: Display: ...Automation Test Queue Empty 400 tests performed.\n");
        write_log(&project, &text);
        let report = test_lines(&log_since(&project, 0, Some("LogAutomation")));
        assert_eq!(report["failed"], 1, "the first test's failure is not dropped");
        assert_eq!(report["succeeded"], 399);
    }

    #[test]
    fn a_launch_reads_only_the_new_sessions_log() {
        let root = tempfile::tempdir().unwrap();
        let project = bare_project(root.path());
        let bind = "LogHttpListener: Error: HttpListener unable to bind to 127.0.0.1:30010\n";
        // The previous session failed to bind; a fresh launch must not report that.
        write_log(&project, &format!("LogInit: start\n{bind}LogExit: Exiting.\n"));
        let mark = log_mark(&project);
        let old = new_session_start(&project, mark);
        assert!(crate::unreal_process::bind_failures(&log_since(&project, old, Some(BIND_PREFILTER))).is_empty());
        // The new editor moves the old log aside and starts its own; its failure comes early
        // and is followed by far more than 300 lines of start-up.
        std::fs::rename(project.log_path(), root.path().join("Saved/Logs/Game-backup.log")).unwrap();
        let mut text = format!("LogInit: start\n{bind}");
        for n in 0..1000 {
            text.push_str(&format!("LogShaderCompilers: Display: compiling {n}\n"));
        }
        write_log(&project, &text);
        let start = new_session_start(&project, mark);
        assert_eq!(start, 0);
        assert_eq!(crate::unreal_process::bind_failures(&log_since(&project, start, Some(BIND_PREFILTER))).len(), 1);
        // A file that only grew is the same session: read past the mark.
        let grown = log_mark(&project);
        std::fs::write(project.log_path(), format!("{text}LogTemp: more\n")).unwrap();
        assert_eq!(new_session_start(&project, grown), text.len() as u64);
    }

    #[test]
    fn a_large_result_is_found_before_the_output_is_cut() {
        let rows: Vec<Value> = (0..2000).map(|n| json!({"label": format!("Actor_{n}"), "path": format!("/Game/Maps/Main.Main:PersistentLevel.StaticMeshActor_UAID_{n:040}")})).collect();
        let payload = json!({"count": rows.len(), "actors": rows}).to_string();
        assert!(payload.len() > 2 * MAX_OUTPUT);
        let response = json!({"ReturnValue": true, "LogOutput": [
            {"Type": "Info", "Output": "LogPython: starting\nsecond line\n"},
            {"Type": "Info", "Output": format!("RELAY_JSON:{payload}\n")},
            {"Type": "Warning", "Output": "a warning printed after the result"},
        ]});
        let output = python_output(&response);
        assert!(relay_json(&tail(&output, 2000)).is_none(), "the cut output has lost the result");
        let parsed: Value = serde_json::from_str(relay_json(&output).unwrap()).unwrap();
        assert_eq!(parsed["count"], 2000);
        assert_eq!(relay_json("a\nRELAY_JSON:1\nRELAY_JSON:2\n[Warning] w"), Some("2"), "the last result wins");
    }

    #[test]
    fn a_render_check_that_could_not_read_pixels_is_not_a_pass() {
        assert_eq!(renders_from_coverage(&[json!(0.0), json!(0.05)]), json!(true));
        assert_eq!(renders_from_coverage(&[json!(0.0), json!(0.001)]), json!(false));
        assert!(renders_from_coverage(&[Value::Null, Value::Null]).is_null(), "unknown, not visible");
        assert!(renders_from_coverage(&[]).is_null());
        assert!(renders_from_coverage(&[json!(0.0), Value::Null]).is_null(), "one view unmeasured");
    }
}
