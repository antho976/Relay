#!/usr/bin/env bash
# Leaf's server data copied to this PC (the owner, 2026-11-25): the server's backups and books
# would otherwise live on one disk. Run nightly by a systemd user timer (install-offsite.sh).
#
# Each night is one archive, $LEAF_OFFSITE/leaf-YYYY-MM-DD.tar.gz (default ~/backups/leaf), the
# last 14 kept. In it: leaf-server's data (leaf.db, its nightly JSON backups, the phone's books
# and shelf) and Suwayomi's database, settings and own backups (its backups are the copy to
# restore from: the live database may be mid-write). Not copied: chapters (fetched again),
# chapter pictures and caches (made again). The server is told when (offsite_at), and Settings ›
# Server shows it. The server has no rsync: tar over ssh.
set -euo pipefail

HOST=${LEAF_HOST:-server1}
DEST=${LEAF_OFFSITE:-$HOME/backups/leaf}
KEEP=14
DATA=apps/leaf/server/data

mkdir -p "$DEST"
out="$DEST/leaf-$(date +%F).tar.gz"
# GNU tar answers 1 when a file changed while read (Suwayomi's live database): still a copy.
ssh "$HOST" "tar -C $DATA -czf - --warning=no-file-changed --exclude=leaf/thumbs --exclude=leaf/tagart --exclude=leaf/offsite_at leaf suwayomi/database.mv.db suwayomi/server.conf suwayomi/backups suwayomi/settings" > "$out.part" || [ $? -eq 1 ]
# A copy that isn't a whole archive is not kept.
tar -tzf "$out.part" > /dev/null
mv "$out.part" "$out"
ssh "$HOST" "touch $DATA/leaf/offsite_at"

# The oldest nights go once there are more than KEEP.
find "$DEST" -maxdepth 1 -type f -name 'leaf-20*.tar.gz' | sort | head -n -"$KEEP" | while read -r old; do
    rm -- "$old"
done
echo "leaf offsite: $out ($(du -h "$out" | cut -f1))"
