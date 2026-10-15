import json, sys
# python3 show.py before|after <this directory>: the table in ATTRIBUTION-2026-10-14.md, "Fixed since, part 2".
tag = sys.argv[1]
rows = {}
for f in ['c1', 'c2', 'n1', 'n2']:
    for l in open(f'{sys.argv[2]}/{tag}-{f}.jsonl'):
        r = json.loads(l); rows.setdefault(r['name'], {})[f] = r
for n, d in rows.items():
    ir = [d[f]['counters']['instructions']['p50'] for f in ('c1', 'c2')]
    p50 = [d[f]['wall_ns']['p50'] / 1000 for f in ('n1', 'n2')]
    print(f"{n:26} Ir {ir[0]:>10} {ir[1]:>10}   p50 µs {p50[0]:8.1f} {p50[1]:8.1f}   allocs {d['n1']['allocs_per_iter']:>7}  bytes {d['n1']['alloc_bytes_per_iter']:>9}")
