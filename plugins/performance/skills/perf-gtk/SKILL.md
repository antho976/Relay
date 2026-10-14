---
name: perf-gtk
description: GTK 4 desktop client performance, especially from Rust (gtk4-rs) - the frame clock and its phases, GLib main-loop priorities (events, resize 110, redraw 120, idle 200) and how a flood of default-priority futures starves painting, queue_draw vs queue_resize, layout cascades from changing labels, snapshot and render-node cost, the GL/Vulkan renderers and Cairo fallbacks, CSS invalidation and provider reloads, GtkListView/ColumnView recycling instead of widget-per-row boxes, Pango text cost, VTE and GtkSourceView knobs, async patterns (spawn_future_local, gio::spawn_blocking, async-channel, tokio off the UI thread), tick callbacks vs timeouts, startup (icon themes, CSS parse, building hidden pages), reference-cycle leaks with clone!, idle wakeups, and the measuring tools (GDK_DEBUG=frames, GSK_DEBUG, the Inspector's frame timings, recorder and statistics, sysprof marks, FrameClock timings). Use for any GTK UI that janks, starts slowly, uses CPU at rest, leaks, or lags on input.
---

# GTK 4 performance

A GTK app is one thread running one GLib main loop. Input, your callbacks, layout and painting all
take turns on it. Every rule here comes back to: **keep each turn short, and do not ask for work
the frame does not need.**

## 1. The frame and the main loop

- The **frame clock** is idle until something asks for a frame (`queue_draw`, `queue_resize`, a tick
  callback, an animation). Then it runs one frame through its phases: flush-events → before-paint →
  **update** (tick callbacks, animations) → **layout** (measure + allocate) → **paint** (each widget's
  `snapshot()` builds a render-node tree; the renderer diffs it against the last frame and draws the
  damage) → after-paint.
- **Main-loop priorities** (lower runs first): events and `DEFAULT` (0, where
  `glib::spawn_future_local`, channel wake-ups and `timeout_add_local` run) → `HIGH_IDLE` (100) →
  GTK resize (110) → **redraw (120)** → `DEFAULT_IDLE` (200, `idle_add`).
  Consequence: **a steady stream of default-priority work starves painting.** A future that drains
  a busy channel (terminal output, events, log lines) without yielding can keep the frame from
  ever running. Drain at most N items or a time slice per wake-up, coalesce, or run bulk UI updates
  with `MainContext::spawn_local_with_priority(Priority::DEFAULT_IDLE, …)`.
- **Budget**: 16.7 ms at 60 Hz, 8.3 ms at 120 Hz for everything in a frame — your callbacks,
  layout, snapshot and the renderer's CPU work (diffing, batching, uploads). A GPU renderer does
  not make the CPU side free.

## 2. Asking for less

- `queue_draw` repaints; `queue_allocate` reruns allocation only; **`queue_resize` renegotiates
  size and propagates up the tree** — the expensive one. Setting a label's text, changing a
  widget's visibility, or swapping children queues a resize.
- Labels updated often (counters, timers, status): fix their size (`width-chars`,
  `max-width-chars`, ellipsize, a size request) so a text change does not relayout the window.
  `GtkLabel` is for short text; long text belongs in a `GtkTextView`.
- A **tick callback** runs every frame while installed and keeps the frame clock running at refresh
  rate. Return `ControlFlow::Break` when the animation ends; never leave an endless one installed
  for something that changes every few seconds. It does not redraw by itself — call `queue_draw`.
- **Do not rebuild what did not change.** Compare a cheap key (a version, a hash of the inputs)
  before `remove`-all-and-rebuild; update the changed rows in place. Rebuilding a box of N
  widgets is N constructions, N style resolutions and a full layout.
- Custom drawing: cache the `PangoLayout` (or glyph strings) between frames and
  `snapshot.append_layout()`; build render nodes once and reuse them when inputs are unchanged.
  Avoid Cairo nodes in hot widgets (they rasterize on the CPU and upload a texture);
  blur, shadows and offscreen effects are costly per frame.

## 3. Lists: recycle, do not build

`GtkListView`, `GtkGridView` and `GtkColumnView` create widgets only for visible rows and **recycle**
them as you scroll; a `GtkBox`/`GtkListBox` with one widget tree per item is fine for tens of rows
and impractical for thousands — every row is built, styled, measured and kept in memory.

- Model: `gio::ListStore` (balanced tree, fine to ~1 M items) or `GtkStringList`; make bulk
  changes with one `splice`/`items-changed`, not N appends.
- Factory: build the row's widgets in `setup`, keep `bind` cheap (set text, toggle a class),
  undo connections in `unbind`. State lives in the item, never in the row widget — rows are rebound
  at any time.
- Filters and sorters: `FilterListModel`/`SortListModel` with incremental mode for large models.
- The list view must be the direct child of a `ScrolledWindow`.

## 4. CSS

- Style is recomputed when a widget's CSS node changes state (hover, focus, classes, `:backdrop`)
  — and state changes propagate: hover and `:focus-within` reach ancestors, so a selector that
  depends on ancestor state invalidates more than it looks. Keep selectors short; avoid toggling
  classes on the window or other high ancestors during interaction; avoid broad `*` and long
  descendant chains on large trees.
- Load the app's CSS once; give runtime-variable parts (theme colours) their own small provider
  and reload only it, only when its text actually changed. A reload restyles every widget that
  provider applies to.
- Transitions and infinite animations keep the frame clock running; animate only what is visible
  and honour `gtk-enable-animations`.
- Measure before changing CSS: sysprof's CSS marks or `GTK_DEBUG=no-css-cache` (debug only) to
  see how much style resolution costs in a slow frame.

## 5. Async on the main loop (gtk4-rs)

```rust
glib::spawn_future_local(clone!(#[weak] label, async move {
    let text = gio::spawn_blocking(move || std::fs::read_to_string(path)) // off the UI thread
        .await.expect("task").unwrap_or_default();
    label.set_text(&text);                                               // back on it
}));
```

- Disk, network, subprocesses, decoding, parsing, highlighting, hashing, canonicalizing paths:
  never in a signal handler or a future's synchronous part. `gio::spawn_blocking` for one-off
  blocking calls (closures must be `Send` — no GObjects inside); your own threads or rayon for
  sustained CPU work; a tokio runtime held in a `OnceLock` for async I/O, with results sent back
  over `async_channel` and awaited in `spawn_future_local`. Never `block_on` on the UI thread.
- Streams: `async_channel::bounded(n)`; worker `send_blocking`, UI
  `while let Ok(x) = rx.recv().await` — with a per-wake-up budget (§1).
- Timers: `add_tick_callback` for anything visual (frame-aligned, removed with the widget);
  `timeout_add_local` for precise non-visual work; **`timeout_add_seconds_local` for background
  periodic work** (GLib groups these wake-ups). Keep the `SourceId` and remove it when the widget
  is disposed or the window hides.
- `glib::timeout_future_seconds` instead of `thread::sleep` in futures.

## 6. Startup

- Measure launch → first frame → first useful frame → interactive (sysprof's "Launch Application",
  GTK's `gtk_init` marks, `strace -f -tt -T -e trace=openat,statx,connect` for file and socket
  stalls).
- Known costs: icon-theme loading (hundreds of ms with large themes; `GTK_DEBUG=icontheme`), CSS
  parsing of a big stylesheet, GPU renderer initialization (`GSK_RENDERER=cairo` A/B shows it),
  font registration, accessibility bus lookups (`GTK_A11Y=none` A/B), and building every page.
- **Present the window first; build hidden pages on first show** (or at idle priority after the
  first frame). Do not open secondary windows at startup unless they are needed.
- Batch the reads issued on connect; show cached state before the fresh state arrives.

## 7. Terminals and source views

- **VTE** (0.76+) draws with GSK text nodes on the frame clock and processes input ~2× faster than
  older releases; keep it current. Costly knobs: very large `scrollback-lines` (memory, search),
  regex matchers (`match_add_regex`, evaluated on pointer motion), `enable-a11y`, BiDi/shaping on
  heavy output. `get_text_range` scans the whole scrollback — never on a hot path.
- Feed output in frame-sized slices so input and painting stay responsive during a flood; measure
  keystroke latency *during* a flood (p99.9) and Ctrl-C response, not only at rest.
- **GtkSourceView**: very long lines are a known weak spot — gate huge or minified files (open
  read-only, disable highlighting with `set_highlight_syntax(false)`, or truncate);
  `ensure_highlight()` is synchronous.

## 8. Memory and leaks

- The classic leak is a **reference cycle through a closure**: a signal handler or timeout that
  strongly captures its own widget, its parent or the window. Capture with
  `clone!(#[weak] widget, …)` (the closure becomes a no-op once the widget is gone); keep exactly
  one strong owner; `glib::WeakRef` inside imp structs.
- Detect: run with `GOBJECT_DEBUG=instance-count`, open the Inspector's Statistics tab, open and
  close a page N times — object counts should return to baseline. heaptrack for allocation growth.

## 9. Idle

At rest the app should produce **no frames and no wake-ups**:

- `GDK_DEBUG=frames` (or the Inspector's "Show Framerate"/"Show Graphic Updates") — frames while
  idle mean a tick callback, animation, transition or redraw loop is alive.
- Stop timers, polls and animations when the window is hidden, minimized or the page unmapped
  (`notify::is-active`, `map`/`unmap`, the toplevel's `suspended` state). Replace polling with
  `gio::FileMonitor`, D-Bus signals or a pushed event.
- Cursor blink: respect `gtk-cursor-blink` and stop blinking when unfocused.
- Measure wakeups: `/proc/<pid>/status` context switches over 30 s, `perf stat -p`, `powertop`,
  `strace -f -tt -e trace=ppoll,poll,epoll_wait,futex,timerfd_settime -p <pid>`.

## 10. Measuring tools

| Tool | What it shows |
|---|---|
| `GDK_DEBUG=frames` | frame-clock log per frame |
| Inspector (`GTK_DEBUG=interactive`, Ctrl+Shift+D) → Visual | Show Framerate, Show Frame Times, Show Graphic Updates, Show Cairo Rendering (finds fallbacks) |
| Inspector → Recorder | the render-node tree of each frame (memory-heavy; record short) |
| Inspector → Statistics (`GOBJECT_DEBUG=instance-count`) | live object counts per type |
| `GSK_DEBUG=fallback,renderer,cache` / `full-redraw` / `profile` | fallbacks, renderer choice, worst-case frames, GPU profiling heat maps |
| `GSK_RENDERER=help`, `GDK_DEBUG=opengl,vulkan`, `GDK_DISABLE=…` | which renderer runs; A/B a feature |
| **sysprof** (`sysprof-cli -- ./app`) | CPU samples with GTK's frame-phase, layout, CSS and renderer marks on one timeline; select a slow frame's span and read its flame graph. Needs frame pointers and symbols |
| `FrameClock::current_timings()` / `timings()` | per-frame timings in code (fields fill in late; 0 means pending) |
| Keyspeed (uinput injection → sysprof marks), a camera or a light-sensor rig | keystroke latency; only hardware sees scan-out |

`GDK_DEBUG=no-vsync` repaints as fast as possible at 100 % CPU — only for throughput tests.
Open windows for measurement inside a virtual compositor (for example
`kwin_wayland --virtual --no-lockscreen`) rather than on someone's desktop; a locked virtual
session never delivers frame callbacks.
