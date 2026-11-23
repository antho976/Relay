#!/usr/bin/env bash
# Turns the collections' pictures (one PNG per tag, named by its slug: the tag lower-cased, spaces
# as "-", "martial-arts.png") into the WebP files leaf-server serves (GET /v1/recs/tagart/{slug}):
# 640 × 360, quality 80, into apps/leaf/server/tagart/ (TAGART_OUT changes where).
#
#   apps/leaf/tools/tagart.sh <src_dir>
#
# Uses cwebp when it is installed, else Python's Pillow. Then, at deploy, copy them to the server's
# data folder (LEAF_TAGART, by default <LEAF_DATA>/tagart, /data/tagart in the container):
#
#   ssh server1 'mkdir -p apps/leaf/server/data/leaf/tagart'
#   scp apps/leaf/server/tagart/*.webp server1:apps/leaf/server/data/leaf/tagart/
set -euo pipefail

if [[ $# -ne 1 || ! -d $1 ]]; then
    echo "usage: $0 <folder of <slug>.png>" >&2
    exit 2
fi
src=$1
out=${TAGART_OUT:-$(cd "$(dirname "$0")/.." && pwd)/server/tagart}
mkdir -p "$out"

shopt -s nullglob
pngs=("$src"/*.png)
if [[ ${#pngs[@]} -eq 0 ]]; then
    echo "$src: no .png files" >&2
    exit 2
fi

n=0
for f in "${pngs[@]}"; do
    slug=$(basename "$f" .png)
    if [[ ! $slug =~ ^[a-z0-9][a-z0-9-]*$ ]]; then
        echo "skipped $f: a slug is lower-case letters, digits and - only" >&2
        continue
    fi
    to="$out/$slug.webp"
    if command -v cwebp >/dev/null; then
        cwebp -quiet -q 80 -resize 640 360 "$f" -o "$to.part"
    else
        # Isolated (-I): nothing from the source folder is imported.
        python3 -I - "$f" "$to.part" <<'PY'
import sys
from PIL import Image, ImageOps
src, to = sys.argv[1], sys.argv[2]
with Image.open(src) as im:
    im = ImageOps.fit(im.convert("RGB"), (640, 360), Image.Resampling.LANCZOS)
    im.save(to, "WEBP", quality=80, method=6)
PY
    fi
    mv "$to.part" "$to"
    n=$((n + 1))
done
echo "$n pictures in $out ($(du -sh "$out" | cut -f1))"
