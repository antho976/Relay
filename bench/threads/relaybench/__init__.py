"""Relay's Threads model bench (bench/threads/README.md).

A throwaway Relay engine at a fixed date, a fixed ledger and training history, and a local model
behind the product's own path (Claude Code, Relay's prompt, the MCP door). Standard library only.
"""

import os
from pathlib import Path

#: bench/threads/
BENCH_DIR = Path(__file__).resolve().parent.parent
#: The repository root.
REPO = BENCH_DIR.parent.parent
TASKS_DIR = BENCH_DIR / "tasks"
RESULTS_DIR = BENCH_DIR / "results"

#: The fixed world's "now" (contract: everything runs at this local time under libfaketime).
ANCHOR = "2026-09-18 12:00:00"
TODAY = ANCHOR[:10]
#: The zone the anchor is local to; set for the engine, Claude Code and the MCP server alike.
ZONE = "America/Toronto"
#: The thread model id the engine runs locally (crates/relay-core/src/local_models.rs).
MODEL_ID = "qwen35-4b"
#: The file name the engine looks for in `threads.local.dir` for MODEL_ID.
MODEL_FILE = "Qwen3.5-4B-Q8_0.gguf"
#: Every llama-server the bench starts holds this lock (one GPU, several people running benches).
GPU_LOCK = Path("/tmp/relay-bench-gpu.lock")
#: Throwaway engines live under here (short: a unix socket path is at most 108 bytes).
WORK_ROOT = Path("/tmp/relay-bench")


def log(*parts):
    """Progress on stderr, so stdout stays for results."""
    print(*parts, file=__import__("sys").stderr, flush=True)


def default_relay_bin():
    return REPO / "target" / "debug" / "relay"


def env_flag(name):
    return os.environ.get(name, "") not in ("", "0", "false", "no")
