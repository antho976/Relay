//! Files only this user may read. The store makes `store.db` private (RA-342); the databases
//! beside it hold as much — Threads' conversations, Arbiter's trades, Avex's training, Tally's
//! ledger — and were created with the umask's 0644 in a 0755 folder. These make them private too.
//! A failure (a file someone else owns) is logged, not fatal: an existing install must keep
//! opening.
//!
//! And files Relay reads where someone else may have put something else: [`read_regular`].

use std::io::Write;
use std::os::unix::fs::{MetadataExt, PermissionsExt};
use std::path::Path;

/// 0700 on the instance's data folder, when it is a real folder this user owns and others can
/// enter. The engine runs this at start, so an install from before keeps working and is closed.
pub fn private_data_dir(dir: &Path) {
    let Ok(meta) = std::fs::symlink_metadata(dir) else { return };
    if !meta.is_dir() || meta.uid() != unsafe { libc::getuid() } || meta.mode() & 0o077 == 0 {
        return;
    }
    if let Err(error) = std::fs::set_permissions(dir, std::fs::Permissions::from_mode(0o700)) {
        tracing::warn!(dir = %dir.display(), %error, "could not make the data folder private");
    }
}

/// 0600 on an SQLite database and the `-wal` and `-shm` files beside it, those that exist. Run it
/// after the database is open in WAL mode; later side files take the database file's mode.
pub fn private_db(path: &Path) {
    for side in ["", "-wal", "-shm"] {
        let mut name = path.as_os_str().to_owned();
        name.push(side);
        let name = Path::new(&name);
        if name.exists() {
            private_file(name);
        }
    }
}

/// 0600 on a file Relay wrote.
pub fn private_file(path: &Path) {
    if let Err(error) = std::fs::set_permissions(path, std::fs::Permissions::from_mode(0o600)) {
        tracing::warn!(path = %path.display(), %error, "could not make the file private");
    }
}

/// `bytes` at `path`, renamed in so a reader never finds half a file, and 0600 from the start
/// (the temporary file is created that way).
pub fn write_private_replace(path: &Path, bytes: &[u8]) -> std::io::Result<()> {
    let parent = path.parent().filter(|p| !p.as_os_str().is_empty()).unwrap_or(Path::new("."));
    let mut file = tempfile::Builder::new().prefix(".relay-").suffix(".part").tempfile_in(parent)?;
    file.write_all(bytes)?;
    file.as_file().sync_all()?;
    file.persist(path).map_err(|error| error.error)?;
    Ok(())
}

/// The file a symlink at `path` leads to, through every link in the chain, or `path` when it is
/// no symlink. A link to a file not made yet resolves too. [`write_private_replace`] at the result
/// writes through the link, where a rename over `path` would replace the link with a plain file:
/// a backup linked into a synced folder would stop syncing. Its own files (`gym-upload.json`)
/// Relay replaces, link or not.
pub fn resolve_links(path: &Path) -> std::io::Result<std::path::PathBuf> {
    let mut at = path.to_path_buf();
    for _ in 0..40 {
        match std::fs::symlink_metadata(&at) {
            Ok(meta) if meta.file_type().is_symlink() => {
                let target = std::fs::read_link(&at)?;
                at = match at.parent() {
                    Some(parent) if target.is_relative() => parent.join(target),
                    _ => target,
                };
            }
            _ => return Ok(at),
        }
    }
    Err(std::io::Error::from_raw_os_error(libc::ELOOP))
}

/// The most [`read_regular`] reads. Every file it serves is a few lines of config, JSON or a
/// git pointer.
pub const MAX_READ: u64 = 1 << 20;

/// Read `path` as text only if it is a regular file of at most [`MAX_READ`] bytes. For files
/// an agent can write — in its checkout, under `<repo>/.relay/hooks`, in the git dir its checkout
/// shares — read under a lock (`hooks::writes`, the store mutex, the exclude and skills locks):
/// a plain read of a FIFO planted there blocks forever, and every request waiting on that lock
/// with it. So the open does not wait on a FIFO (`O_NONBLOCK`) nor follow a final link
/// (`O_NOFOLLOW`), and the opened file must be regular. A missing file is still `NotFound`.
pub fn read_regular(path: &Path) -> std::io::Result<String> {
    read_capped(path, false)
}

/// [`read_regular`], following a link at `path` to the file it leads to, which must be regular.
/// For files git itself reads through links — a checkout's `.git` pointer, its slot's `gitdir`
/// and `commondir`, `info/exclude` — where refusing a link would change what Relay takes for a
/// worktree, not only refuse a FIFO.
pub fn read_regular_through_links(path: &Path) -> std::io::Result<String> {
    read_capped(path, true)
}

fn read_capped(path: &Path, follow: bool) -> std::io::Result<String> {
    use std::io::Read as _;
    use std::os::unix::fs::OpenOptionsExt as _;
    let refuse = |why: &str| {
        tracing::warn!(path = %path.display(), "not reading {why}");
        std::io::Error::new(std::io::ErrorKind::InvalidData, format!("{} is {why}", path.display()))
    };
    let flags = if follow { libc::O_NONBLOCK } else { libc::O_NOFOLLOW | libc::O_NONBLOCK };
    let file = match std::fs::OpenOptions::new().read(true).custom_flags(flags).open(path) {
        Ok(file) => file,
        Err(error) if !follow && error.raw_os_error() == Some(libc::ELOOP) => return Err(refuse("a symlink")),
        Err(error) => return Err(error),
    };
    let meta = file.metadata()?;
    if !meta.is_file() {
        return Err(refuse("not a regular file"));
    }
    if meta.len() > MAX_READ {
        return Err(refuse("larger than 1 MiB"));
    }
    let mut text = String::new();
    file.take(MAX_READ + 1).read_to_string(&mut text)?;
    if text.len() as u64 > MAX_READ {
        return Err(refuse("larger than 1 MiB"));
    }
    Ok(text)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn mode(path: &Path) -> u32 {
        std::fs::metadata(path).unwrap().permissions().mode() & 0o777
    }

    #[test]
    fn databases_their_side_files_and_the_data_folder_become_private() {
        let dir = tempfile::tempdir().unwrap();
        let data = dir.path().join("dev");
        std::fs::create_dir(&data).unwrap();
        std::fs::set_permissions(&data, std::fs::Permissions::from_mode(0o755)).unwrap();
        let db = data.join("money.db");
        let conn = rusqlite::Connection::open(&db).unwrap();
        conn.pragma_update(None, "journal_mode", "WAL").unwrap();
        conn.execute_batch("CREATE TABLE t (x); INSERT INTO t VALUES (1);").unwrap();
        for side in ["", "-wal", "-shm"] {
            let mut name = db.as_os_str().to_owned();
            name.push(side);
            std::fs::set_permissions(Path::new(&name), std::fs::Permissions::from_mode(0o644)).unwrap();
        }
        private_db(&db);
        private_data_dir(&data);
        assert_eq!(mode(&data), 0o700);
        for side in ["money.db", "money.db-wal", "money.db-shm"] {
            assert_eq!(mode(&data.join(side)), 0o600, "{side}");
        }
        // A symlink to a folder is not followed.
        let elsewhere = dir.path().join("elsewhere");
        std::fs::create_dir(&elsewhere).unwrap();
        std::fs::set_permissions(&elsewhere, std::fs::Permissions::from_mode(0o755)).unwrap();
        std::os::unix::fs::symlink(&elsewhere, dir.path().join("link")).unwrap();
        private_data_dir(&dir.path().join("link"));
        assert_eq!(mode(&elsewhere), 0o755);

        let upload = data.join("gym-upload.json");
        std::fs::write(&upload, "old").unwrap();
        std::fs::set_permissions(&upload, std::fs::Permissions::from_mode(0o644)).unwrap();
        write_private_replace(&upload, b"{}").unwrap();
        assert_eq!((std::fs::read_to_string(&upload).unwrap().as_str(), mode(&upload)), ("{}", 0o600));
    }

    #[test]
    fn links_resolve_to_the_file_they_lead_to() {
        let dir = tempfile::tempdir().unwrap();
        let real = dir.path().join("sync").join("tally.json");
        std::fs::create_dir(real.parent().unwrap()).unwrap();
        let (relative, absolute) = (dir.path().join("relative.json"), dir.path().join("absolute.json"));
        std::os::unix::fs::symlink("sync/tally.json", &relative).unwrap();
        std::os::unix::fs::symlink(&relative, &absolute).unwrap();
        // Not made yet, then made: either way the chain ends at the real file.
        assert_eq!(resolve_links(&absolute).unwrap(), real);
        std::fs::write(&real, "old").unwrap();
        assert_eq!(resolve_links(&absolute).unwrap(), real);
        assert_eq!(resolve_links(&real).unwrap(), real);
        let (a, b) = (dir.path().join("a"), dir.path().join("b"));
        std::os::unix::fs::symlink(&b, &a).unwrap();
        std::os::unix::fs::symlink(&a, &b).unwrap();
        assert_eq!(resolve_links(&a).unwrap_err().raw_os_error(), Some(libc::ELOOP));
    }
}
