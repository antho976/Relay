# Sources

Primary sources behind the skills, read in October 2026. Where a skill quotes a number, it came from
one of these. Re-check version-specific details (tool flags, library versions, store thresholds)
against the live page before relying on them.

## Method and case studies

- How we made claude.ai faster (claude.dev blog, August 2026) — https://claude.dev/blog/how-we-made-claude-ai-faster/ — summarized in `case-study-claude-ai.md`
- Brendan Gregg, Performance analysis methodology — https://www.brendangregg.com/methodology.html
- USE method — https://www.brendangregg.com/usemethod.html
- TSA method — https://www.brendangregg.com/tsamethod.html
- Active benchmarking — https://www.brendangregg.com/activebenchmarking.html
- Off-CPU analysis — https://www.brendangregg.com/offcpuanalysis.html
- Flame graphs — https://www.brendangregg.com/flamegraphs.html
- Linux performance analysis in 60 seconds (Netflix) — https://www.brendangregg.com/Articles/Netflix_Linux_Perf_Analysis_60s.pdf
- The RED method (Tom Wilkie) — https://grafana.com/blog/the-red-method-how-to-instrument-your-services/
- Google SRE book, Monitoring distributed systems (four golden signals) — https://sre.google/sre-book/monitoring-distributed-systems/

## Perception and user-facing metrics

- Nielsen, Response times: the 3 important limits — https://www.nngroup.com/articles/response-times-3-important-limits/
- RAIL model — https://web.dev/articles/rail
- Core Web Vitals — https://web.dev/articles/vitals · INP — https://web.dev/articles/inp · CLS — https://web.dev/articles/cls · LCP — https://web.dev/articles/lcp
- Skeleton screens (NN/g) — https://www.nngroup.com/articles/skeleton-screens/
- Speculation rules / prerender — https://developer.chrome.com/docs/web-platform/prerender-pages
- Dan Luu, Computer latency: 1977–2017 — https://danluu.com/input-lag/
- Dan Luu, Terminal latency — https://danluu.com/term-latency/

## Statistics and regression detection

- Dean & Barroso, The Tail at Scale (CACM 2013) — https://research.google/pubs/the-tail-at-scale/
- Gil Tene, How NOT to measure latency / coordinated omission — https://groups.google.com/g/mechanical-sympathy/c/icNZJejUHfE/m/BfDekfBEs_sJ · https://bravenewgeek.com/everything-you-know-about-latency-is-wrong/
- HdrHistogram — https://github.com/HdrHistogram/HdrHistogram
- Google Benchmark comparison tooling (Mann–Whitney U) — https://github.com/google/benchmark/blob/main/docs/tools.md
- Criterion.rs analysis — https://bheisler.github.io/criterion.rs/book/analysis.html
- Fleming & Wallace, the correct way to summarize benchmark results (geometric mean) — https://cacm.acm.org/research/how-not-to-lie-with-statistics-the-correct-way-to-summarize-benchmark-results
- Change-point detection in CI (MongoDB, E-divisive) — https://arxiv.org/abs/2003.00584
- Sequential testing for canaries (Netflix) — https://arxiv.org/abs/2205.14762
- Meta ServiceLab (OSDI '24) — https://www.usenix.org/system/files/osdi24-chow.pdf
- Meta FBDetect (SOSP '24) — https://tangchq74.github.io/FBDetect-SOSP24.pdf
- Meta Strobelight — https://engineering.fb.com/2025/01/21/production-engineering/strobelight-a-profiling-service-built-on-open-source-technology/
- Google-Wide Profiling — https://research.google/pubs/pub36575/ · Profiling a warehouse-scale computer — https://research.google/pubs/profiling-a-warehouse-scale-computer/ · AutoFDO — https://research.google/pubs/autofdo-automatic-feedback-directed-optimization-for-warehouse-scale-applications/
- Chromium perf regression sheriffing — https://chromium.googlesource.com/chromium/src/+/71.0.3578.2/docs/speed/perf_regression_sheriffing.md
- Mozilla perf sheriffing (Perfherder) — https://firefox-source-docs.mozilla.org/testing/perfdocs/perf-sheriffing.html
- rustc-perf (instruction counts as the headline metric) — https://github.com/rust-lang/rustc-perf

## Budgets, laws, estimation

- Performance budgets 101 — https://web.dev/articles/performance-budgets-101
- Amdahl's law — https://en.wikipedia.org/wiki/Amdahl%27s_law · Little's law — https://en.wikipedia.org/wiki/Little%27s_law
- Latency numbers every programmer should know — https://gist.github.com/jboner/2841832 · interactive, by year — https://colin-scott.github.io/personal_website/research/interactive_latency.html

## Linux and native tooling

- perf examples (Gregg) — https://www.brendangregg.com/perf.html · `perf stat` — https://man7.org/linux/man-pages/man1/perf-stat.1.html
- samply — https://github.com/mstange/samply · cargo-flamegraph — https://github.com/flamegraph-rs/flamegraph
- Valgrind callgrind manual — https://valgrind.org/docs/manual/cl-manual.html
- heaptrack — https://github.com/KDE/heaptrack · dhat-rs — https://docs.rs/dhat/latest/dhat/
- bpftrace one-liners — https://github.com/bpftrace/bpftrace/blob/master/docs/tutorial_one_liners.md
- LLVM benchmarking tips (noise reduction) — https://llvm.org/docs/Benchmarking.html · Google Benchmark, reducing variance — https://google.github.io/benchmark/reducing_variance.html
- hyperfine — https://github.com/sharkdp/hyperfine
- powertop — https://github.com/fenrus75/powertop · timer slack — https://man7.org/linux/man-pages/man2/pr_set_timerslack.2const.html · NO_HZ — https://docs.kernel.org/timers/no_hz.html
- posix_spawn — https://man7.org/linux/man-pages/man3/posix_spawn.3.html · Rust std process spawning — https://github.com/rust-lang/rust/blob/master/library/std/src/sys/process/unix/unix.rs
- IPC latency data — https://kamalmarhubi.com/blog/2015/06/10/some-early-linux-ipc-latency-data · https://github.com/goldsborough/ipc-bench

## Rust

- The Rust Performance Book (Nethercote) — https://nnethercote.github.io/perf-book/ (build configuration, heap allocations, hashing, type sizes, bounds checks, I/O, profiling, benchmarking)
- gungraun (formerly iai-callgrind) — https://github.com/gungraun/gungraun · regressions and baselines — https://gungraun.github.io/gungraun/latest/html/regressions.html
- criterion — https://github.com/criterion-rs/criterion.rs · divan — https://github.com/nvzqz/divan
- CodSpeed — https://codspeed.io/docs/instruments · Bencher thresholds — https://bencher.dev/docs/explanation/thresholds/
- Alice Ryhl, Async: what is blocking? — https://ryhl.io/blog/async-what-is-blocking/
- tokio `spawn_blocking` — https://docs.rs/tokio/latest/tokio/task/fn.spawn_blocking.html

## SQLite

- WAL — https://www.sqlite.org/wal.html · pragmas — https://www.sqlite.org/pragma.html · mmap — https://www.sqlite.org/mmap.html
- EXPLAIN QUERY PLAN — https://www.sqlite.org/eqp.html · transactions — https://www.sqlite.org/lang_transaction.html · FAQ — https://www.sqlite.org/faq.html
- Many small queries are efficient in SQLite — https://www.sqlite.org/np1queryprob.html
- rusqlite `Connection` — https://docs.rs/rusqlite/latest/rusqlite/struct.Connection.html

## GTK 4

- Running GTK apps (debug variables) — https://docs.gtk.org/gtk4/running.html · GdkFrameClock — https://docs.gtk.org/gdk4/class.FrameClock.html · `GDK_PRIORITY_REDRAW` — https://docs.gtk.org/gdk4/const.PRIORITY_REDRAW.html
- Frame clock source — https://gitlab.gnome.org/GNOME/gtk/-/raw/main/gdk/gdkframeclockidle.c · Inspector visual options — https://gitlab.gnome.org/GNOME/gtk/-/raw/main/gtk/inspector/visual.ui · GTK NEWS — https://gitlab.gnome.org/GNOME/gtk/-/raw/main/NEWS
- New renderers for GTK — https://blog.gtk.org/2024/01/28/new-renderers-for-gtk/ · Graphics offload — https://blog.gtk.org/2023/11/15/introducing-graphics-offload/
- GtkWidget — https://docs.gtk.org/gtk4/class.Widget.html · GtkLabel — https://docs.gtk.org/gtk4/class.Label.html · list widgets — https://docs.gtk.org/gtk4/section-list-widget.html · GtkSnapshot — https://docs.gtk.org/gtk4/class.Snapshot.html
- CSS overview — https://docs.gtk.org/gtk4/css-overview.html · GtkCssProvider — https://docs.gtk.org/gtk4/class.CssProvider.html · CSS revalidation (issue 55) — https://gitlab.gnome.org/GNOME/gtk/-/issues/55
- gtk-rs book, main event loop — https://gtk-rs.org/gtk4-rs/git/book/main_event_loop.html · memory management — https://gtk-rs.org/gtk4-rs/stable/latest/book/g_object_memory_management.html · `glib::MainContext` — https://gtk-rs.org/gtk-rs-core/stable/latest/docs/glib/struct.MainContext.html · `clone!` — https://gtk-rs.org/gtk-rs-core/stable/latest/docs/glib/macro.clone.html
- GLib main loop — https://docs.gtk.org/glib/main-loop.html · `add_tick_callback` — https://docs.gtk.org/gtk4/method.Widget.add_tick_callback.html · Use timeout_add_seconds — https://wiki.gnome.org/Initiatives/GnomeGoals/UseTimeoutAddSeconds · GNOME Sleep initiative — https://wiki.gnome.org/Initiatives/GnomePerformance/Sleep
- Startup cost reports — https://gitlab.gnome.org/GNOME/gtk/-/issues/5263 · https://gitlab.gnome.org/GNOME/gtk/-/issues/5362 · https://gitlab.gnome.org/GNOME/gtk/-/issues/4831
- Christian Hergert: GNOME 45/46 retrospective (VTE and GtkSourceView) — https://blogs.gnome.org/chergert/2024/03/25/gnome-45-46-retrospective/ · poll timeout precision — https://blogs.gnome.org/chergert/2024/03/03/improving-poll-timeout-precision/ · debug builds and GPUs — https://blogs.gnome.org/chergert/2024/03/24/debug-builds-and-gpus/ · testing keyboard input latency (Keyspeed) — https://blogs.gnome.org/chergert/2026/06/13/testing-keyboard-input-latency/
- Marks and counters in sysprof — https://feaneron.com/2025/09/08/marks-and-counters-in-sysprof/
- How much faster are the GNOME 46 terminals — https://bxt.rs/blog/just-how-much-faster-are-the-gnome-46-terminals/ · VTE news — https://discourse.gnome.org/t/terminal-and-vte-news/20030 · VteTerminal (GTK 4) — https://gnome.pages.gitlab.gnome.org/vte/gtk4/class.Terminal.html
- GtkSourceView buffer — https://gnome.pages.gitlab.gnome.org/gtksourceview/gtksourceview5/class.Buffer.html

## Android

- App startup time — https://developer.android.com/topic/performance/vitals/launch-time · Play vitals launch time — https://developer.android.com/google/play/vitals/launch-time
- App Startup library — https://developer.android.com/topic/libraries/app-startup
- Baseline Profiles — https://developer.android.com/topic/performance/baselineprofiles/overview · DEX layout and Startup Profiles — https://developer.android.com/topic/performance/baselineprofiles/dex-layout-optimizations
- Macrobenchmark — https://developer.android.com/topic/performance/benchmarking/macrobenchmark-overview · metrics — https://developer.android.com/topic/performance/benchmarking/macrobenchmark-metrics · in CI — https://developer.android.com/topic/performance/benchmarking/benchmarking-in-ci · Microbenchmark — https://developer.android.com/topic/performance/benchmarking/microbenchmark-overview
- Slow rendering — https://developer.android.com/topic/performance/vitals/render · JankStats — https://developer.android.com/topic/performance/jankstats · Android vitals — https://developer.android.com/topic/performance/vitals · Play Console vitals thresholds — https://support.google.com/googleplay/android-developer/answer/9844486
- Compose performance — https://developer.android.com/develop/ui/compose/performance · best practices — https://developer.android.com/develop/ui/compose/performance/bestpractices · strong skipping — https://developer.android.com/develop/ui/compose/performance/stability/strongskipping · stability diagnose/fix — https://developer.android.com/develop/ui/compose/performance/stability/diagnose · composition tracing — https://developer.android.com/develop/ui/compose/tooling/tracing
- R8 / app optimization — https://developer.android.com/topic/performance/app-optimization/enable-app-optimization
- SQLite on Android — https://developer.android.com/topic/performance/sqlite-performance-best-practices · Room async queries — https://developer.android.com/training/data-storage/room/async-queries · Room 3 — https://developer.android.com/jetpack/androidx/releases/room3 · Paging 3 — https://developer.android.com/topic/libraries/architecture/paging/v3-overview
- WorkManager battery — https://developer.android.com/develop/background-work/background-tasks/persistent/getting-started/define-work · wake locks — https://developer.android.com/develop/background-work/background-tasks/awake/wakelock/best-practices
- Android performance leveling guide (2025) — https://android-developers.googleblog.com/2025/11/leveling-guide-for-your-performance.html
