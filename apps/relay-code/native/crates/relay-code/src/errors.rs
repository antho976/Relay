//! Errors in words. The PC answers a refusal with a code (`git.diff_too_large`), a sentence that
//! may name an op (`design.list failed: …`) and, for git, git's own stderr (`fatal: not a git
//! repository …`). A page shows none of that as it comes: [`words`] turns any of it into a plain
//! sentence, and every error state and toast goes through it, so no op name or code is ever drawn.

use rc_link::BusError;

/// What went wrong, in words a person reads: a fixed sentence for the codes the phone knows, else
/// the PC's own sentence with its op names and git's prefixes taken off.
pub fn words(e: &BusError) -> String {
    match by_code(&e.code) {
        Some(w) => w.to_string(),
        None => plain(&e.message),
    }
}

/// The sentence for a code whose message would say nothing useful to a person.
fn by_code(code: &str) -> Option<&'static str> {
    Some(match code {
        "link.down" => "The PC is out of reach right now.",
        "link.timeout" | "bus.timeout" => "The PC took too long to answer.",
        "remote.op" => "The PC does not let the phone do that.",
        "bus.unknown_op" => "The PC does not know how to do that yet: it may need updating.",
        "git.gh_unavailable" | "github.gh_missing" => "GitHub's command-line tool (gh) is not set up on the PC.",
        "github.not_connected" => "GitHub is not connected on the PC.",
        "git.pr_list_timeout" | "git.pr_checks_timeout" | "github.list_timeout" => "GitHub took too long to answer.",
        "git.diff_too_large" => "Too large to show here: over 1 MB of text.",
        "git.diff_binary" => "A binary file: there is no text to show.",
        "git.open_failed" => "This folder is not a Git repository.",
        c if c.ends_with(".not_found") => "It is no longer there: it may have been deleted or moved.",
        _ => return None,
    })
}

/// A message as a person reads it: no leading op name ("design.list failed:"), none of git's
/// "fatal:", "error:" or hint lines, the common git failures in plain words, a capital and a full
/// stop.
pub fn plain(message: &str) -> String {
    // git's commonest failures, wherever in its output they are.
    if let Some(known) = git_failure(message) {
        return known.to_string();
    }
    // git writes hints and several lines: its "fatal:" or "error:" line says it, else the first.
    let lines: Vec<&str> = message.lines().map(str::trim).filter(|l| !l.is_empty() && !l.starts_with("hint:")).map(strip_op).collect();
    let said = lines.iter().find(|l| l.starts_with("fatal:") || l.starts_with("error:")).or(lines.first()).copied().unwrap_or_default();
    let line = strip_git(said);
    let mut out = line.trim().to_string();
    if out.is_empty() {
        return "Something went wrong on the PC.".into();
    }
    if let Some(c) = out.chars().next() {
        out.replace_range(..c.len_utf8(), &c.to_uppercase().to_string());
    }
    if !out.ends_with(['.', '!', '?']) {
        out.push('.');
    }
    out
}

/// Whether `token` reads as an op's name: lowercase words joined by dots (`git.pr.list`), no path.
fn is_op(token: &str) -> bool {
    let parts: Vec<&str> = token.split('.').collect();
    parts.len() >= 2 && parts.iter().all(|p| !p.is_empty() && p.bytes().all(|b| b.is_ascii_lowercase() || b == b'_'))
}

/// "design.list failed: x", "file.read refused: x", "git.status: x" → "x". Repeated, as the PC
/// sometimes wraps one in another.
fn strip_op(mut s: &str) -> &str {
    loop {
        let Some((head, rest)) = s.split_once(':') else { return s };
        let head = head.trim();
        let (op, said) = match head.strip_suffix(" failed").or_else(|| head.strip_suffix(" refused")) {
            Some(op) => (op, true),
            None => (head, false),
        };
        // Only an op's name goes: one followed by "failed" or "refused", or one of the PC's own
        // namespaces. "config.toml: missing field" keeps its file's name.
        if !is_op(op) || !(said || known_op(op)) {
            return s;
        }
        s = rest.trim_start();
    }
}

/// The PC's op namespaces: a name under one of them, alone before a colon, is an op's.
const OP_SPACES: &[&str] = &[
    "aide", "app", "arbiter", "avd", "bus", "clips", "dashboard", "design", "device", "file", "git", "github", "guardrail", "gym",
    "integration", "link", "mailbox", "module", "money", "notes", "notify", "phone", "plugin", "project", "remote", "session",
    "settings", "skill", "task", "thread", "ui", "usage", "video", "workspace", "worktree",
];

fn known_op(op: &str) -> bool {
    op.split('.').next().is_some_and(|space| OP_SPACES.contains(&space))
}

/// git's own prefixes taken off: "fatal: ", "error: ", "warning: ", and "git pull failed: " the
/// PC puts before them.
fn strip_git(mut s: &str) -> &str {
    loop {
        let before = s;
        for p in ["fatal:", "error:", "warning:", "remote:"] {
            if let Some(rest) = s.strip_prefix(p) {
                s = rest.trim_start();
            }
        }
        for cmd in ["git ", "gh "] {
            if let Some(rest) = s.strip_prefix(cmd)
                && let Some((_, after)) = rest.split_once(" failed:")
                && rest.split_once(" failed:").is_some_and(|(c, _)| c.split_whitespace().count() <= 3)
            {
                s = after.trim_start();
            }
        }
        if s == before {
            return s;
        }
    }
}

/// git's commonest failures, said plainly, from anywhere in its output.
fn git_failure(output: &str) -> Option<&'static str> {
    let l = output.to_lowercase();
    Some(if l.contains("not a git repository") {
        "This folder is not a Git repository."
    } else if l.contains("could not resolve host") || l.contains("network is unreachable") || l.contains("unable to access") {
        "The PC could not reach the remote. Is it online?"
    } else if l.contains("permission denied (publickey)") || l.contains("could not read from remote repository") {
        "The remote refused the PC's key."
    } else if l.contains("authentication failed") || l.contains("bad credentials") || l.contains("http 401") {
        "The remote refused the PC's sign-in."
    } else if l.contains("[rejected]") || l.contains("non-fast-forward") || l.contains("fetch first") {
        "The remote has commits this branch does not. Pull them first."
    } else if l.contains("index.lock") && l.contains("exists") {
        "Another git command is running in this checkout. Try again in a moment."
    } else {
        return None;
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn err(code: &str, message: &str) -> BusError {
        BusError { kind: "unavailable".into(), code: code.into(), message: message.into(), hint: None }
    }

    /// What D19 found on screen, as the PC sends it, and what each page now shows.
    #[test]
    fn no_op_name_code_or_git_prefix_reaches_the_screen() {
        let cases = [
            (err("guardrail.list_failed", "guardrail.holds.list failed: the PC's store is busy; try again in a moment"), "The PC's store is busy; try again in a moment."),
            (err("design.list_failed", "design.list failed: database is locked"), "Database is locked."),
            (err("file.path", "file.read refused: the path is outside every worktree of this repo"), "The path is outside every worktree of this repo."),
            (err("git.status_failed", "fatal: not a git repository (or any of the parent directories): .git"), "This folder is not a Git repository."),
            (err("git.pull_failed", "git pull failed: fatal: unable to access 'https://github.com/a/b/': Could not resolve host: github.com"), "The PC could not reach the remote. Is it online?"),
            (err("git.push_failed", "git push failed: To github.com:a/b.git\n ! [rejected] relay/x -> relay/x (fetch first)\nerror: failed to push some refs\nhint: Updates were rejected"), "The remote has commits this branch does not. Pull them first."),
            (err("git.commit_failed", "git commit failed: error: pathspec 'x' did not match any file(s) known to git"), "Pathspec 'x' did not match any file(s) known to git."),
            (err("git.pr_merge_failed", "X Pull request antho976/Relay#209 is not mergeable: the merge commit cannot be cleanly created."), "X Pull request antho976/Relay#209 is not mergeable: the merge commit cannot be cleanly created."),
            (err("link.down", "The link to the PC closed"), "The PC is out of reach right now."),
            (err("git.diff_too_large", "This diff is larger than the 1 MiB editor limit."), "Too large to show here: over 1 MB of text."),
            (err("task.not_found", "task 41 not found"), "It is no longer there: it may have been deleted or moved."),
            (err("remote.op", "git.pull is not a phone op"), "The PC does not let the phone do that."),
        ];
        for (e, want) in cases {
            let got = words(&e);
            assert_eq!(got, want, "{} / {}", e.code, e.message);
            assert!(!got.split_whitespace().any(|w| is_op(w.trim_matches(|c: char| !c.is_ascii_alphanumeric() && c != '.' && c != '_'))), "an op name in {got:?}");
        }
    }

    #[test]
    fn a_plain_sentence_stays_as_it_is() {
        let s = "This branch and its upstream have both moved, so it cannot simply move forward. Merge or rebase it on the PC.";
        assert_eq!(words(&err("git.pull_not_fast_forward", s)), s);
        assert_eq!(plain(""), "Something went wrong on the PC.");
        // A file's name is not an op's: "app.rs" has no second lowercase-only word... and stays.
        assert_eq!(plain("src/app.rs could not be read"), "Src/app.rs could not be read.");
        assert!(!is_op("app.rs.bak/x") && is_op("git.pr.list") && !is_op("Git.pull"));
        // A file's name before a colon stays; an op's goes.
        assert_eq!(plain("config.toml: missing field `name`"), "Config.toml: missing field `name`.");
        assert_eq!(plain("git.status: index is locked"), "Index is locked.");
        assert_eq!(plain("foo.bar failed: it broke"), "It broke.");
    }
}
