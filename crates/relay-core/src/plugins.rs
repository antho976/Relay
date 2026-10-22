//! Built-in plugins: a bundle of skills, always-on agent instructions, documentation and MCP
//! servers, switched on per project (D159).
//!
//! A plugin is compiled into the engine from the repository's `plugins/<id>/` folder (see
//! `build.rs`), so its content and the code that delivers it never drift apart. Switching one
//! on for a project stores one `plugin_projects` edge; from then on every agent launched in
//! that project gets the plugin's skills as real provider skill folders (through the same
//! materializer as installed skills), the plugin's instructions in its brief, and the plugin's
//! MCP servers in its provider configuration. Nothing is written for a project that has it off,
//! with one exception: Codex reads skills only from `$CODEX_HOME/skills`, one folder for the
//! whole machine, so a plugin on in any project registers its skills there (`skills::plan_user`).

use anyhow::Result;
use relay_bus::types::{Id, Plugin, PluginDoc, PluginMcpServer, PluginSkill};
use rusqlite::Connection;
use serde::Deserialize;
use std::collections::BTreeMap;
use std::path::Path;
use std::sync::OnceLock;

/// One plugin folder as the build script embedded it.
pub struct BundledPlugin {
    pub id: &'static str,
    /// A digest of every file, so a rebuilt engine with changed content re-materializes.
    pub digest: &'static str,
    /// `(path relative to the plugin root, contents)`, sorted by path.
    pub files: &'static [(&'static str, &'static [u8])],
}

include!(concat!(env!("OUT_DIR"), "/bundled_plugins.rs"));

#[derive(Debug, Clone, Deserialize)]
pub struct Manifest {
    pub id: String,
    pub name: String,
    pub version: String,
    #[serde(default)]
    pub category: String,
    #[serde(default)]
    pub summary: String,
    #[serde(default)]
    pub description: String,
    /// Markdown every agent of an enabled project receives in its brief.
    pub instructions: Option<String>,
    #[serde(default)]
    pub docs: Vec<String>,
    /// File-name suffixes at a checkout root that mark a project this plugin is for.
    #[serde(default)]
    pub detect: Vec<String>,
    #[serde(default)]
    pub mcp_servers: Vec<McpServer>,
    /// Where a new agent of an enabled project works when `session.create` names no checkout:
    /// `"primary"` for tools that act on one shared checkout (a running Unreal editor has
    /// exactly one project open). Absent means Relay's default, a new worktree.
    #[serde(default)]
    pub default_checkout: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct McpServer {
    pub name: String,
    #[serde(default)]
    pub description: String,
    /// `relay` means this Relay binary; anything else is looked up on `PATH` by the provider.
    pub command: String,
    #[serde(default)]
    pub args: Vec<String>,
    #[serde(default)]
    pub env: BTreeMap<String, String>,
    #[serde(default)]
    pub tools: Vec<String>,
}

/// One skill folder inside a plugin.
pub struct SkillEntry {
    /// Folder name, which is also the skill's provider-visible name.
    pub dir: String,
    pub description: String,
    pub body: String,
    /// Files relative to the skill folder.
    pub files: Vec<(&'static str, &'static [u8])>,
}

pub struct Loaded {
    pub bundle: &'static BundledPlugin,
    pub manifest: Manifest,
    pub skills: Vec<SkillEntry>,
}

impl Loaded {
    pub fn id(&self) -> &str {
        &self.manifest.id
    }

    pub fn file(&self, path: &str) -> Option<&'static str> {
        self.bundle
            .files
            .iter()
            .find(|(name, _)| *name == path)
            .and_then(|(_, bytes)| std::str::from_utf8(bytes).ok())
    }

    pub fn instructions(&self) -> String {
        self.manifest
            .instructions
            .as_deref()
            .and_then(|path| self.file(path))
            .unwrap_or("")
            .trim()
            .to_string()
    }
}

/// Every bundled plugin, parsed once. A folder with an unreadable manifest is skipped with a
/// warning rather than taking the engine down.
pub fn all() -> &'static [Loaded] {
    static LOADED: OnceLock<Vec<Loaded>> = OnceLock::new();
    LOADED.get_or_init(|| BUNDLED.iter().filter_map(|bundle| match load(bundle) {
        Ok(loaded) => Some(loaded),
        Err(error) => {
            tracing::warn!(plugin = bundle.id, error = %error, "bundled plugin manifest");
            None
        }
    }).collect())
}

pub fn get(id: &str) -> Option<&'static Loaded> {
    all().iter().find(|plugin| plugin.id() == id)
}

fn load(bundle: &'static BundledPlugin) -> Result<Loaded> {
    let raw = bundle
        .files
        .iter()
        .find(|(name, _)| *name == "plugin.json")
        .map(|(_, bytes)| *bytes)
        .ok_or_else(|| anyhow::anyhow!("no plugin.json"))?;
    let manifest: Manifest = serde_json::from_slice(raw)?;
    anyhow::ensure!(manifest.id == bundle.id, "manifest id {:?} is not its folder {:?}", manifest.id, bundle.id);
    let mut skills: BTreeMap<String, SkillEntry> = BTreeMap::new();
    for (path, bytes) in bundle.files {
        let Some(rest) = path.strip_prefix("skills/") else { continue };
        let Some((dir, inner)) = rest.split_once('/') else { continue };
        let entry = skills.entry(dir.to_string()).or_insert_with(|| SkillEntry {
            dir: dir.to_string(),
            description: String::new(),
            body: String::new(),
            files: Vec::new(),
        });
        entry.files.push((inner, bytes));
        if inner == "SKILL.md" {
            entry.body = String::from_utf8_lossy(bytes).into_owned();
            entry.description = skill_description(&entry.body);
        }
    }
    Ok(Loaded {
        bundle,
        manifest,
        skills: skills.into_values().filter(|skill| !skill.body.is_empty()).collect(),
    })
}

/// One value from a `SKILL.md` YAML front matter block, the one reader of it for plugin skills,
/// installed skills' names (`github.rs`) and every skill's bus `description` (RA-709). The block
/// opens the body, blank lines before it aside, and `key` sits at column 0: a key nested under
/// another one belongs to that one. A plain value has its quotes trimmed; a `>` or `|` block is
/// folded from the indented lines under it into one line.
pub fn frontmatter(body: &str, key: &str) -> Option<String> {
    let mut lines = body.lines().skip_while(|line| line.trim().is_empty()).peekable();
    if lines.next()?.trim() != "---" {
        return None;
    }
    while let Some(line) = lines.next() {
        let line = line.trim_end();
        if line.trim() == "---" {
            return None;
        }
        let Some(value) = line.strip_prefix(key).and_then(|rest| rest.strip_prefix(':')) else {
            continue;
        };
        let value = value.trim();
        let block = value.starts_with(['>', '|']) && value[1..].chars().all(|c| matches!(c, '-' | '+' | '1'..='9'));
        if !block {
            return Some(value.trim_matches('"').trim_matches('\'').to_string());
        }
        let mut folded = Vec::new();
        while let Some(line) = lines.next_if(|line| line.starts_with([' ', '\t']) || line.trim().is_empty()) {
            folded.push(line.trim());
        }
        return Some(folded.into_iter().filter(|line| !line.is_empty()).collect::<Vec<_>>().join(" "));
    }
    None
}

/// What a skill is for: its front matter's `description`, else the first line of prose after
/// the block (a heading is not prose); empty when there is neither.
pub fn skill_description(body: &str) -> String {
    if let Some(description) = frontmatter(body, "description").filter(|value| !value.is_empty()) {
        return description;
    }
    let mut lines = body.lines().skip_while(|line| line.trim().is_empty()).peekable();
    if lines.next_if(|line| line.trim() == "---").is_some() {
        lines.by_ref().find(|line| line.trim() == "---");
    }
    lines
        .map(str::trim)
        .find(|line| !line.is_empty() && !line.starts_with('#') && !line.starts_with("---"))
        .unwrap_or_default()
        .to_string()
}

/// Ids of the bundled plugins switched on for a project, in id order.
pub fn enabled_ids(conn: &Connection, project_id: Id) -> Result<Vec<String>> {
    let mut stmt = conn.prepare_cached(
        "SELECT plugin_id FROM plugin_projects WHERE project_id=?1 ORDER BY plugin_id",
    )?;
    let ids = stmt.query_map([project_id], |row| row.get(0))?.collect::<rusqlite::Result<Vec<String>>>()?;
    Ok(ids)
}

pub fn enabled_for(conn: &Connection, project_id: Id) -> Result<Vec<&'static Loaded>> {
    Ok(enabled_ids(conn, project_id)?.iter().filter_map(|id| get(id)).collect())
}

/// Plugins on for at least one project — what the machine-wide Codex skill folder carries.
pub fn enabled_anywhere(conn: &Connection) -> Result<Vec<&'static Loaded>> {
    let mut stmt = conn.prepare_cached("SELECT DISTINCT plugin_id FROM plugin_projects ORDER BY plugin_id")?;
    let ids = stmt.query_map([], |row| row.get(0))?.collect::<rusqlite::Result<Vec<String>>>()?;
    Ok(ids.iter().filter_map(|id| get(id)).collect())
}

pub fn projects_for(conn: &Connection, plugin_id: &str) -> Result<Vec<Id>> {
    let mut stmt = conn.prepare_cached(
        "SELECT project_id FROM plugin_projects WHERE plugin_id=?1 ORDER BY project_id",
    )?;
    let ids = stmt.query_map([plugin_id], |row| row.get(0))?.collect::<rusqlite::Result<Vec<Id>>>()?;
    Ok(ids)
}

/// The checkout a new session of this project gets when the request names none: `"primary"`
/// when an enabled plugin asks for it (D160), otherwise a new worktree.
pub fn default_checkout(conn: &Connection, project_id: Id) -> Result<&'static str> {
    let primary = enabled_for(conn, project_id)?
        .iter()
        .any(|plugin| plugin.manifest.default_checkout.as_deref() == Some("primary"));
    Ok(if primary { "primary" } else { "new" })
}

/// Whether a checkout root looks like this plugin's kind of project: one directory listing,
/// never a tree walk.
pub fn detect(plugin: &Loaded, root: &Path) -> bool {
    if plugin.manifest.detect.is_empty() {
        return false;
    }
    let Ok(entries) = std::fs::read_dir(root) else { return false };
    entries.flatten().any(|entry| {
        let name = entry.file_name();
        let name = name.to_string_lossy();
        plugin.manifest.detect.iter().any(|suffix| name.ends_with(suffix.as_str()))
    })
}

pub fn to_bus(plugin: &Loaded, enabled_in: Vec<Id>, suggested_for: Vec<Id>) -> Plugin {
    let manifest = &plugin.manifest;
    Plugin {
        id: manifest.id.clone(),
        name: manifest.name.clone(),
        version: manifest.version.clone(),
        category: manifest.category.clone(),
        summary: manifest.summary.clone(),
        description: manifest.description.clone(),
        skills: plugin
            .skills
            .iter()
            .map(|skill| PluginSkill {
                name: skill.dir.clone(),
                description: skill.description.clone(),
                files: skill.files.len() as u32,
            })
            .collect(),
        mcp_servers: manifest
            .mcp_servers
            .iter()
            .map(|server| PluginMcpServer {
                name: server.name.clone(),
                description: server.description.clone(),
                tools: server.tools.clone(),
            })
            .collect(),
        docs: manifest.docs.clone(),
        enabled_in,
        suggested_for,
    }
}

pub fn docs(plugin: &Loaded) -> Vec<PluginDoc> {
    plugin
        .manifest
        .docs
        .iter()
        .filter_map(|path| plugin.file(path).map(|body| PluginDoc { path: path.clone(), body: body.to_string() }))
        .collect()
}

/// One plugin MCP server ready for a provider configuration: name, command, args, env.
pub type LaunchServer = (String, String, Vec<String>, BTreeMap<String, String>);

/// The MCP servers every enabled plugin contributes to a project, as `(name, command, args,
/// env)` ready for a provider configuration. `relay` resolves to this Relay binary.
pub fn mcp_servers(
    conn: &Connection,
    project_id: Id,
    relay: &Path,
) -> Result<Vec<LaunchServer>> {
    let mut out: Vec<LaunchServer> = Vec::new();
    for plugin in enabled_for(conn, project_id)? {
        for server in &plugin.manifest.mcp_servers {
            // `relay` is reserved for the bus's own server; a plugin cannot shadow it.
            if server.name == "relay" || out.iter().any(|(name, ..)| name == &server.name) {
                continue;
            }
            let command = if server.command == "relay" {
                relay.display().to_string()
            } else {
                server.command.clone()
            };
            out.push((server.name.clone(), command, server.args.clone(), server.env.clone()));
        }
    }
    Ok(out)
}

/// The brief section that makes an enabled plugin's rules part of every agent's standing
/// context: its instructions verbatim, then what it installed and where.
pub fn brief_section(conn: &Connection, project_id: Id) -> Result<String> {
    let plugins = enabled_for(conn, project_id)?;
    if plugins.is_empty() {
        return Ok("No plugins are on for this project.".into());
    }
    let mut out = String::new();
    for plugin in plugins {
        if !out.is_empty() {
            out.push_str("\n\n");
        }
        out.push_str(&format!("### {} plugin (on for this project)\n\n", plugin.manifest.name));
        let instructions = plugin.instructions();
        if !instructions.is_empty() {
            out.push_str(&instructions);
            out.push_str("\n\n");
        }
        if !plugin.skills.is_empty() {
            out.push_str(&format!(
                "Skills from this plugin, installed as folders under {}/ and {}/ — load the matching one before the work it covers:\n",
                crate::skills::TARGETS[0],
                crate::skills::TARGETS[1]
            ));
            for skill in &plugin.skills {
                out.push_str(&format!("- {} — {}\n", skill.dir, skill.description));
            }
        }
        for server in &plugin.manifest.mcp_servers {
            out.push_str(&format!(
                "\nMCP server `{}`: {}\nTools: {}\n",
                server.name,
                server.description,
                server.tools.join(", ")
            ));
        }
    }
    Ok(out.trim_end().to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_unreal_engine_plugin_is_bundled_and_complete() {
        let plugin = get("unreal-engine").expect("unreal-engine is bundled");
        assert_eq!(plugin.manifest.name, "Unreal Engine");
        assert!(!plugin.instructions().is_empty(), "no always-on instructions");
        for doc in &plugin.manifest.docs {
            assert!(plugin.file(doc).is_some(), "manifest names a missing doc {doc}");
        }
        assert!(plugin.skills.len() >= 10, "only {} skills", plugin.skills.len());
        for skill in &plugin.skills {
            assert_eq!(
                frontmatter(&skill.body, "name").as_deref(),
                Some(skill.dir.as_str()),
                "{}: front matter name must equal its folder",
                skill.dir
            );
            assert!(!skill.description.is_empty(), "{} has no description", skill.dir);
            assert_eq!(crate::skills::folder_name(&skill.dir), skill.dir, "{} is not provider-safe", skill.dir);
        }
        let server = &plugin.manifest.mcp_servers[0];
        assert_eq!(server.name, "unreal");
        assert_eq!(server.command, "relay");
    }

    #[test]
    fn the_blender_plugin_is_bundled_and_complete() {
        let plugin = get("blender").expect("blender is bundled");
        assert!(!plugin.instructions().is_empty());
        for doc in &plugin.manifest.docs {
            assert!(plugin.file(doc).is_some(), "manifest names a missing doc {doc}");
        }
        assert_eq!(plugin.manifest.mcp_servers[0].args, vec!["blender-mcp".to_string()]);
        for skill in &plugin.skills {
            assert_eq!(frontmatter(&skill.body, "name").as_deref(), Some(skill.dir.as_str()));
            assert!(!skill.description.is_empty(), "{} has no description", skill.dir);
        }
        // Two plugins on at once must not collide on skill folders or server names.
        let unreal = get("unreal-engine").unwrap();
        assert!(plugin.skills.iter().all(|s| unreal.skills.iter().all(|u| u.dir != s.dir)));
        assert_ne!(plugin.manifest.mcp_servers[0].name, unreal.manifest.mcp_servers[0].name);
    }

    #[test]
    fn the_performance_plugin_is_bundled_and_complete() {
        let plugin = get("performance").expect("performance is bundled");
        assert!(!plugin.instructions().is_empty());
        for doc in &plugin.manifest.docs {
            assert!(plugin.file(doc).is_some(), "manifest names a missing doc {doc}");
        }
        assert!(plugin.manifest.mcp_servers.is_empty(), "the performance plugin runs no server");
        assert!(plugin.manifest.detect.is_empty(), "it is for any project, so it suggests itself for none");
        assert_eq!(plugin.skills.len(), 10);
        for skill in &plugin.skills {
            assert!(skill.dir.starts_with("perf-"), "{} is not prefixed", skill.dir);
            assert_eq!(frontmatter(&skill.body, "name").as_deref(), Some(skill.dir.as_str()));
            assert!(!skill.description.is_empty(), "{} has no description", skill.dir);
            assert_eq!(crate::skills::folder_name(&skill.dir), skill.dir, "{} is not provider-safe", skill.dir);
            // The rules name every skill, so an agent can find it from its brief alone.
            assert!(plugin.instructions().contains(&format!("`{}`", skill.dir)), "instructions do not name {}", skill.dir);
        }
    }

    #[test]
    fn the_pictures_plugin_is_bundled_and_complete() {
        let plugin = get(relay_bus::ops::picture::PLUGIN).expect("pictures is bundled");
        assert_eq!(plugin.manifest.name, "Pictures");
        assert!(!plugin.instructions().is_empty());
        assert!(!plugin.manifest.docs.is_empty());
        for doc in &plugin.manifest.docs {
            assert!(plugin.file(doc).is_some(), "manifest names a missing doc {doc}");
        }
        // Its tools are engine ops, gated by the switch: no server, no suggestion, no checkout rule.
        assert!(plugin.manifest.mcp_servers.is_empty());
        assert!(plugin.manifest.detect.is_empty());
        assert_eq!(plugin.manifest.default_checkout, None);
        let skills: Vec<&str> = plugin.skills.iter().map(|skill| skill.dir.as_str()).collect();
        assert_eq!(skills, ["design-pictures"]);
        let skill = &plugin.skills[0];
        assert_eq!(frontmatter(&skill.body, "name").as_deref(), Some(skill.dir.as_str()));
        assert!(!skill.description.is_empty());
        assert_eq!(crate::skills::folder_name(&skill.dir), skill.dir, "not provider-safe");
        assert!(plugin.instructions().contains("`design-pictures`"), "the rules name the skill");
        // The skill teaches every op the plugin brings, and what to do where it is off (Codex
        // registers a plugin's skills for the whole machine).
        let brought: Vec<&str> = relay_bus::registry::Registry::global().entries().iter()
            .filter(|entry| entry.meta.plugin == Some(plugin.id()))
            .map(|entry| entry.name)
            .collect();
        assert_eq!(brought.len(), 3, "{brought:?}");
        for op in brought {
            assert!(skill.body.contains(&format!("`{op} ")), "the skill does not show {op}");
        }
        assert!(skill.body.contains("`plugin.off`") && skill.body.contains("can_call"));
    }

    #[test]
    fn the_video_plugin_is_bundled_and_complete() {
        let plugin = get(relay_bus::ops::video::PLUGIN).expect("video is bundled");
        assert_eq!(plugin.manifest.name, "Video");
        assert!(!plugin.instructions().is_empty());
        for doc in &plugin.manifest.docs {
            assert!(plugin.file(doc).is_some(), "manifest names a missing doc {doc}");
        }
        assert!(plugin.manifest.mcp_servers.is_empty());
        assert_eq!(plugin.manifest.default_checkout, None);
        let skills: Vec<&str> = plugin.skills.iter().map(|skill| skill.dir.as_str()).collect();
        assert_eq!(skills, ["video-code"]);
        let skill = &plugin.skills[0];
        assert_eq!(frontmatter(&skill.body, "name").as_deref(), Some(skill.dir.as_str()));
        assert_eq!(crate::skills::folder_name(&skill.dir), skill.dir, "not provider-safe");
        assert!(plugin.instructions().contains("`video-code`"), "the rules name the skill");
        // The runtime the engine writes beside the store comes with it, and is no skill's.
        assert!(plugin.file("runtime/worker.mjs").is_some() && plugin.file("runtime/kit/index.ts").is_some());
        let brought: Vec<&str> = relay_bus::registry::Registry::global().entries().iter()
            .filter(|entry| entry.meta.plugin == Some(plugin.id()))
            .map(|entry| entry.name)
            .collect();
        assert_eq!(brought.len(), 29, "{brought:?}");
        for op in brought {
            assert!(skill.body.contains(&format!("`{op} ")), "the skill does not show {op}");
        }
        assert!(skill.body.contains("`plugin.off`") && skill.body.contains("can_call"));
    }

    /// pictures-SPEC-v2 §4: the plugin layer gates an op on the plugin its registry entry names,
    /// so that name must be one `plugin.enable` takes, or the op could never be switched on.
    #[test]
    fn every_plugin_op_names_a_bundled_plugin() {
        let mut brought = 0;
        for entry in relay_bus::registry::Registry::global().entries() {
            if let Some(plugin) = entry.meta.plugin {
                assert!(get(plugin).is_some(), "{} comes with {plugin}, which is not bundled", entry.name);
                brought += 1;
            }
        }
        assert!(brought > 0);
    }

    #[test]
    fn no_two_bundled_plugins_share_a_skill_folder() {
        let mut seen = std::collections::BTreeMap::new();
        for plugin in all() {
            for skill in &plugin.skills {
                if let Some(other) = seen.insert(skill.dir.as_str(), plugin.id()) {
                    panic!("{} is in both {other} and {}", skill.dir, plugin.id());
                }
            }
        }
    }

    #[test]
    fn frontmatter_reads_single_line_values_only_inside_the_block() {
        let body = "---\nname: unreal-ai\ndescription: \"Behavior trees, EQS\"\n---\n\ndescription: not this\n";
        assert_eq!(frontmatter(body, "name").as_deref(), Some("unreal-ai"));
        assert_eq!(frontmatter(body, "description").as_deref(), Some("Behavior trees, EQS"));
        assert_eq!(frontmatter("# no block\ndescription: x", "description"), None);
        // A blank line before the block used to hide it from the client's own reader.
        assert_eq!(frontmatter("\n---\nname: foo\n---\n", "name").as_deref(), Some("foo"));
        // Nested keys belong to something else.
        assert_eq!(frontmatter("---\nmetadata:\n  description: nested\ndescription: top\n---\n", "description").as_deref(), Some("top"));
        assert_eq!(frontmatter("---\nmetadata:\n  name: nested\n---\n", "name"), None);
        assert_eq!(frontmatter("---\ndescription: >\n  Folded over\n\n  two lines.\nname: foo\n---\n", "description").as_deref(), Some("Folded over two lines."));
        assert_eq!(frontmatter("---\ndescription: |-\n  Kept\n  apart\n---\n", "description").as_deref(), Some("Kept apart"));
        assert_eq!(frontmatter("---\ndescription: >= 2 of them\n---\n", "description").as_deref(), Some(">= 2 of them"));
    }

    #[test]
    fn a_skill_without_a_description_is_described_by_its_first_prose() {
        assert_eq!(skill_description("---\nname: foo\ndescription: \"Does foo\"\n---\n# Foo\nProse."), "Does foo");
        assert_eq!(skill_description("\n---\nname: foo\n---\nFirst prose.\n"), "First prose.");
        assert_eq!(skill_description("---\nname: foo\ndescription:\n---\n\n# Foo\n\nFirst prose.\n"), "First prose.");
        assert_eq!(skill_description("# Title\n\nNo front matter here.\n"), "No front matter here.");
        assert_eq!(skill_description("---\nname: foo\n"), "", "an unclosed block has no prose after it");
        assert_eq!(skill_description(""), "");
    }

    #[test]
    fn detection_reads_one_listing() {
        let plugin = get("unreal-engine").unwrap();
        let root = tempfile::tempdir().unwrap();
        assert!(!detect(plugin, root.path()));
        std::fs::create_dir_all(root.path().join("nested")).unwrap();
        std::fs::write(root.path().join("nested/Deep.uproject"), "{}").unwrap();
        assert!(!detect(plugin, root.path()), "detection must not walk the tree");
        std::fs::write(root.path().join("Shooter.uproject"), "{}").unwrap();
        assert!(detect(plugin, root.path()));
    }
}
