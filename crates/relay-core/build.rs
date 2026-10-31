//! Bundles every plugin under the repository's `plugins/` into the engine binary (D159).
//!
//! A built-in plugin is a folder of text: a `plugin.json` manifest, its always-on agent
//! instructions, its skill folders and its documentation. Compiling the files in keeps a
//! plugin's content and the engine that materializes it at one revision, with nothing to
//! install beside the binary.

use std::fmt::Write as _;
use std::path::{Path, PathBuf};
use std::{env, fs};

fn main() {
    let manifest = PathBuf::from(env::var("CARGO_MANIFEST_DIR").unwrap());
    let root = manifest.join("../../plugins");
    println!("cargo:rerun-if-changed={}", root.display());
    let mut out = String::from("pub static BUNDLED: &[BundledPlugin] = &[\n");
    let mut plugins: Vec<PathBuf> = fs::read_dir(&root)
        .map(|entries| entries.flatten().map(|e| e.path()).filter(|p| p.join("plugin.json").is_file()).collect())
        .unwrap_or_default();
    plugins.sort();
    for plugin in plugins {
        let mut files = Vec::new();
        collect(&plugin, &plugin, &mut files);
        files.sort();
        let mut hash: u64 = 0xcbf2_9ce4_8422_2325;
        let mut entries = String::new();
        for (relative, path) in &files {
            for byte in relative.bytes().chain(fs::read(path).unwrap()) {
                hash = (hash ^ u64::from(byte)).wrapping_mul(0x0100_0000_01b3);
            }
            let absolute = path.canonicalize().unwrap();
            writeln!(entries, "        ({relative:?}, include_bytes!({:?})),", absolute.display().to_string()).unwrap();
        }
        let id = plugin.file_name().unwrap().to_string_lossy().to_string();
        writeln!(out, "    BundledPlugin {{\n        id: {id:?},\n        digest: \"{hash:016x}\",\n        files: &[\n{entries}        ],\n    }},").unwrap();
    }
    out.push_str("];\n");
    let dest = PathBuf::from(env::var("OUT_DIR").unwrap()).join("bundled_plugins.rs");
    fs::write(dest, out).unwrap();
}

fn collect(base: &Path, dir: &Path, files: &mut Vec<(String, PathBuf)>) {
    let Ok(entries) = fs::read_dir(dir) else { return };
    for entry in entries.flatten() {
        let path = entry.path();
        let name = entry.file_name().to_string_lossy().to_string();
        // Hidden files, and Python's byte-code cache beside a plugin's script, are not the plugin's.
        if name.starts_with('.') || name == "__pycache__" {
            continue;
        }
        if path.is_dir() {
            collect(base, &path, files);
        } else if path.is_file() {
            let relative = path.strip_prefix(base).unwrap().to_string_lossy().replace('\\', "/");
            files.push((relative, path));
        }
    }
}
