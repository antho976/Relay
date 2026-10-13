//! Files only this user may read. The store makes `store.db` private (RA-342); the databases
//! beside it hold as much — Threads' conversations, Arbiter's trades, Avex's training, Tally's
//! ledger — and were created with the umask's 0644 in a 0755 folder. These make them private too.
//! A failure (a file someone else owns) is logged, not fatal: an existing install must keep
//! opening.

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
}
