//! The model pickers' shared rules, for Threads and the Dev assistant's drawer alike: Claude's
//! models, which model is one on this PC, and which picks go to the local server.

/// The Claude models a conversation may run on: (id, caption). An empty id is Claude's own
/// default. The models on this PC (`thread.models`) follow them in each picker.
pub const CLAUDE: [(&str, &str); 5] = [
    ("", "Default model"),
    ("claude-opus-5-5", "Opus 5.5"),
    ("claude-sonnet-5-5", "Sonnet 5.5"),
    ("claude-haiku-5-5", "Haiku 5.5"),
    ("claude-fable-5-1", "Fable 5.1"),
];

/// Whether `model` runs on this PC: one of `local` (the ids `thread.models` found), or any id that
/// is not Claude's, so one whose file was removed still reads as local (no effort, no pictures)
/// rather than as Claude's. Only such a pick goes to `thread.pick`, which loads it and stops the
/// local model running for any other: picking Claude in one place must not stop a local model
/// another conversation (Threads, or the assistant's drawer) is answering on.
pub fn is_local<'a>(model: &str, local: impl IntoIterator<Item = &'a str>) -> bool {
    !model.is_empty() && (local.into_iter().any(|id| id == model) || !model.starts_with("claude-"))
}

#[cfg(test)]
mod tests {
    use super::*;

    const HERE: [&str; 1] = ["qwen3.5-4b"];

    #[test]
    fn a_claude_pick_leaves_the_local_server_alone() {
        for (id, _) in CLAUDE {
            assert!(!is_local(id, HERE), "{id:?}");
        }
        assert!(is_local("qwen3.5-4b", HERE));
        assert!(is_local("qwen3.5-9b", HERE), "a local model whose file went is still not Claude's");
    }
}
