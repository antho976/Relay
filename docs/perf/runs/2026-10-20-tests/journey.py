#!/usr/bin/env python3
"""Edit relay-core, rebuild its tests, with its integration tests as 67 executables (base, the
manifest at HEAD~ with no tests/all.rs) and as tests/all.rs plus 7 (merged): wall and CPU time,
peak RSS of compilers and linkers (../2026-10-18-disk/rss-sampler.sh), bytes linked. Alternates
the two, settling each switch with an untimed build. Restores the merged layout at the end.

    python3 docs/perf/runs/2026-10-20-tests/journey.py <base Cargo.toml> <rounds>
"""
import glob, os, resource, subprocess, sys, time

root = os.path.abspath(os.path.join(os.path.dirname(__file__), "../../../.."))
here = os.path.dirname(os.path.abspath(__file__))
sampler = os.path.join(here, "../2026-10-18-disk/rss-sampler.sh")
core = os.path.join(root, "crates/relay-core")
base, rounds = open(sys.argv[1]).read(), int(sys.argv[2])
merged, allrs = open(f"{core}/Cargo.toml").read(), open(f"{core}/tests/all.rs").read()
build = ["cargo", "test", "--no-run", "-q", "-p", "relay-core"]


def setup(kind):
    open(f"{core}/Cargo.toml", "w").write(base if kind == "base" else merged)
    if kind == "base" and os.path.exists(f"{core}/tests/all.rs"):
        os.remove(f"{core}/tests/all.rs")
    if kind == "merged":
        open(f"{core}/tests/all.rs", "w").write(allrs)


try:
    for r in range(1, rounds + 1):
        for kind in ("base", "merged"):
            setup(kind)
            subprocess.run(build, cwd=root, capture_output=True)
            stop = os.path.join(here, "stop")
            if os.path.exists(stop):
                os.remove(stop)
            rss = subprocess.Popen(["bash", sampler, root, stop], stdout=subprocess.PIPE, text=True)
            os.utime(f"{core}/src/lib.rs")
            r0, t0 = resource.getrusage(resource.RUSAGE_CHILDREN), time.time()
            p = subprocess.run(build, cwd=root, capture_output=True)
            t1, r1 = time.time(), resource.getrusage(resource.RUSAGE_CHILDREN)
            open(stop, "w").close()
            peak = rss.communicate()[0].split()
            os.remove(stop)
            exes = [f for f in glob.glob(f"{root}/target/debug/deps/*") if os.access(f, os.X_OK)
                    and os.path.isfile(f) and os.path.getmtime(f) >= t0]
            cpu = (r1.ru_utime - r0.ru_utime) + (r1.ru_stime - r0.ru_stime)
            print(f"{kind} round{r}: exit {p.returncode}, wall {t1 - t0:.1f} s, cpu {cpu:.1f} s, "
                  f"peak rss sum/single MB {peak[0]} {peak[1]}, linked {len(exes)} exes "
                  f"{sum(map(os.path.getsize, exes)) / 1e9:.1f} GB, load {os.getloadavg()[0]:.1f}", flush=True)
finally:
    setup("merged")
