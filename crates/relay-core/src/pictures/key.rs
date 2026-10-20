//! The Pexels key at rest (pictures-SPEC-v2 §5.5, §7.4): `design.picture.key_set` checks a key
//! with Pexels and keeps it in the desktop keyring, `key_remove` forgets it, and [`load`] reads it
//! back the first time a Pexels call needs it ([`super::saved_key`]).
//!
//! The key goes to `secret-tool` on its stdin: no shell, no environment variable, nothing on a
//! command line another process could read. The test instance keeps it in a private file beside
//! its store instead, and an in-memory store keeps none (saving there is refused, not silently
//! dropped). No answer, error, log, event or audit row carries it: `key_set` is never audited,
//! and every refusal here names what went wrong, never the key.

use super::net::Req;
use super::{key_format, keyring, Bucket, Key, PictureSource, SecretKey};
use crate::engine::Engine;
use crate::Instance;
use relay_bus::BusError;
use std::path::{Path, PathBuf};
use std::process::{Command, Output};
use std::time::{Duration, Instant};

/// The longest key accepted; a Pexels key is 56 letters and digits.
pub const MAX_KEY: usize = 200;
/// How long a keyring lookup may take: room to answer an unlock prompt, while a Pexels search
/// (a lookup and an 8 s request) and an `add` (a lookup, a detail read and a 30 s download)
/// still answer within the desktop's 60 s (§8.5).
pub const LOOKUP_TIMEOUT: Duration = Duration::from_secs(20);
/// How long saving or forgetting may take, as Arbiter's and device signing's keyring writes.
pub const WRITE_TIMEOUT: Duration = Duration::from_secs(30);
/// How long agents are refused at once after a lookup failed, so they do not raise an unlock
/// prompt on every call; the person's calls always ask again.
pub const UNAVAILABLE_FOR: Duration = Duration::from_secs(60);
/// The test instance's key file, beside its store.
pub const TEST_FILE: &str = "pictures-pexels.key";

const LABEL: &str = "--label=Relay Pictures: Pexels key";
const ATTRIBUTES: [&str; 4] = ["application", "Relay", "purpose", "pictures-pexels"];

/// Where an engine keeps the Pexels key.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Keyring {
    /// An in-memory store: it keeps no key.
    Nowhere,
    /// The test instance: a private file beside its store.
    File(PathBuf),
    /// The desktop keyring, through this `secret-tool` (a test stands its own program in).
    Secret(PathBuf),
}

impl Keyring {
    pub fn of(engine: &Engine) -> Keyring {
        let store = engine.store.path();
        if store == Path::new(":memory:") {
            Keyring::Nowhere
        } else if engine.instance == Instance::Test {
            Keyring::File(store.with_file_name(TEST_FILE))
        } else {
            Keyring::Secret(PathBuf::from("secret-tool"))
        }
    }
}

/// The key as the person pasted it, with the whitespace around it dropped: 1 to [`MAX_KEY`]
/// printable ASCII characters (a Pexels key is letters and digits, and it travels as an HTTP
/// header); else `design.picture.key_format`, which never quotes it. Nothing has been sent yet.
pub fn check_format(raw: &str) -> Result<SecretKey, BusError> {
    let key = raw.trim();
    let chars = key.chars().count();
    if chars == 0 {
        return Err(key_format("is empty"));
    }
    if chars > MAX_KEY {
        return Err(key_format(&format!("is {chars} characters; a Pexels key is at most {MAX_KEY}")));
    }
    if key.chars().any(char::is_whitespace) {
        return Err(key_format("has a space or a line break inside it"));
    }
    if key.chars().any(char::is_control) {
        return Err(key_format("holds a control character"));
    }
    if !key.is_ascii() {
        return Err(key_format("holds a character outside ASCII, which a Pexels key never has"));
    }
    Ok(SecretKey::new(key))
}

/// The saved key as the keyring answers it: `Saved`, `Absent` (nothing saved, or the keyring
/// says no item matches), or `Unavailable` for [`UNAVAILABLE_FOR`] when it could not be asked
/// (no `secret-tool`, a locked or missing keyring, no answer within [`LOOKUP_TIMEOUT`]).
pub fn load(ring: &Keyring) -> Key {
    match ring {
        Keyring::Nowhere => Key::Absent,
        Keyring::File(path) => match std::fs::read_to_string(path) {
            Ok(text) => saved(&text),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => Key::Absent,
            Err(e) => {
                tracing::warn!(path = %path.display(), error = %e, "pictures: the test key file could not be read");
                unavailable()
            }
        },
        Keyring::Secret(program) => {
            let mut cmd = Command::new(program);
            cmd.arg("lookup").args(ATTRIBUTES);
            looked_up(crate::proc::output_with_timeout(&mut cmd, LOOKUP_TIMEOUT))
        }
    }
}

/// A `secret-tool lookup`'s outcome as the key's state.
fn looked_up(out: std::io::Result<Option<Output>>) -> Key {
    match out {
        Ok(Some(out)) if out.status.success() => saved(&String::from_utf8_lossy(&out.stdout)),
        // No item matches: secret-tool exits 1 and says nothing.
        Ok(Some(out)) if out.stderr.iter().all(u8::is_ascii_whitespace) => Key::Absent,
        Ok(Some(out)) => {
            tracing::warn!(stderr = %said(&out), "pictures: the desktop keyring could not be read");
            unavailable()
        }
        Ok(None) => {
            tracing::warn!("pictures: the desktop keyring did not answer within {} s", LOOKUP_TIMEOUT.as_secs());
            unavailable()
        }
        Err(e) => {
            tracing::warn!(error = %e, "pictures: secret-tool could not be started");
            unavailable()
        }
    }
}

fn unavailable() -> Key {
    Key::Unavailable(Instant::now() + UNAVAILABLE_FOR)
}

fn saved(text: &str) -> Key {
    match text.trim() {
        "" => Key::Absent,
        key => Key::Saved(SecretKey::new(key)),
    }
}

/// Keep `key`: in the desktop keyring, the key on `secret-tool`'s stdin; on the test instance in
/// a 0600 file. An in-memory store refuses with `design.picture.keyring`.
pub fn save(ring: &Keyring, key: &SecretKey) -> Result<(), BusError> {
    match ring {
        Keyring::Nowhere => Err(keyring("this store keeps no keys")),
        Keyring::File(path) => write_private(path, key.expose().as_bytes())
            .map_err(|e| keyring(format!("the key file could not be written: {e}"))),
        Keyring::Secret(program) => {
            let out = crate::proc::output_with_input(&mut store_command(program), key.expose().as_bytes(), WRITE_TIMEOUT);
            written(out, "keep")
        }
    }
}

/// `secret-tool store` as it is run: the label and the attributes, never the key, which goes on
/// stdin; no environment of its own.
pub fn store_command(program: &Path) -> Command {
    let mut cmd = Command::new(program);
    cmd.arg("store").arg(LABEL).args(ATTRIBUTES);
    cmd
}

/// Forget the saved key. Nothing saved is not an error.
pub fn forget(ring: &Keyring) -> Result<(), BusError> {
    match ring {
        Keyring::Nowhere => Ok(()),
        Keyring::File(path) => match std::fs::remove_file(path) {
            Err(e) if e.kind() != std::io::ErrorKind::NotFound => Err(keyring(format!("the key file could not be removed: {e}"))),
            _ => Ok(()),
        },
        Keyring::Secret(program) => {
            let mut cmd = Command::new(program);
            cmd.arg("clear").args(ATTRIBUTES);
            match crate::proc::output_with_timeout(&mut cmd, WRITE_TIMEOUT) {
                // Nothing to clear: it exits non-zero and says nothing.
                Ok(Some(out)) if !out.status.success() && out.stderr.iter().all(u8::is_ascii_whitespace) => Ok(()),
                out => written(out, "forget"),
            }
        }
    }
}

/// A keyring write's outcome as its refusal: `secret-tool` missing, silent past
/// [`WRITE_TIMEOUT`], or failing with what it said (which never holds the key: it was on stdin).
fn written(out: std::io::Result<Option<Output>>, what: &str) -> Result<(), BusError> {
    match out {
        Ok(Some(out)) if out.status.success() => Ok(()),
        Ok(Some(out)) => Err(keyring(format!("the desktop keyring did not {what} the key: {}", said(&out)))),
        Ok(None) => Err(keyring(format!("the desktop keyring did not answer within {} s", WRITE_TIMEOUT.as_secs()))),
        Err(e) => Err(keyring(format!("secret-tool could not be started ({e}); it comes with libsecret"))),
    }
}

/// What a child wrote on stderr, on one line and at most 200 characters.
fn said(out: &Output) -> String {
    let text = String::from_utf8_lossy(&out.stderr);
    let line = text.split_whitespace().collect::<Vec<_>>().join(" ");
    match line.as_str() {
        "" => format!("it exited with {}", out.status),
        _ => line.chars().filter(|c| !c.is_control()).take(200).collect(),
    }
}

/// Write `bytes` to `path` whole, readable by the person alone, even when a file was there.
fn write_private(path: &Path, bytes: &[u8]) -> std::io::Result<()> {
    use std::io::Write;
    use std::os::unix::fs::{OpenOptionsExt, PermissionsExt};
    let mut file = std::fs::OpenOptions::new().write(true).create(true).truncate(true).mode(0o600).open(path)?;
    // `mode` applies only to a file it creates.
    file.set_permissions(std::fs::Permissions::from_mode(0o600))?;
    file.write_all(bytes)?;
    file.sync_all()
}

/// `design.picture.key_set`'s slow phase: check the key's form (nothing sent when it is wrong),
/// that this store can keep a key, and that Pexels takes it (a 401 or 403 is
/// `design.picture.key_rejected`, and nothing is kept); then keep it, and make it this engine's
/// key, so a lookup that started earlier cannot overwrite it.
///
/// Pexels counts each key's requests apart, so a cooldown the saved key earned neither stops the
/// check of another key nor outlasts it once it is saved.
pub fn set(engine: &Engine, raw: &str) -> Result<(), BusError> {
    let key = check_format(raw)?;
    let ring = Keyring::of(engine);
    if ring == Keyring::Nowhere {
        return Err(keyring("this store keeps no keys"));
    }
    let desk = &engine.pictures;
    let fetch = desk.fetcher(engine.instance, PictureSource::Pexels)?;
    let check = Req::pexels_key_check(key.clone());
    let same = desk.key_state() == Key::Saved(key.clone());
    let checked = if same { desk.send(&*fetch, &check) } else { desk.send_past_cooldown(&*fetch, &check) };
    checked?;
    save(&ring, &key)?;
    if !same {
        desk.warm(Bucket::Pexels);
    }
    desk.set_key(Key::Saved(key));
    Ok(())
}

/// `design.picture.key_remove`'s slow phase: forget the key, then make "none" this engine's
/// answer, so a lookup that started earlier cannot bring it back.
pub fn remove(engine: &Engine) -> Result<(), BusError> {
    forget(&Keyring::of(engine))?;
    engine.pictures.set_key(Key::Absent);
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::os::unix::fs::PermissionsExt;

    const KEY: &str = "sekrit563kQ0wPexelsKeyFortyLettersAndDigitsXyZ0123456789";

    /// A stand-in for `secret-tool`: notes its arguments, environment and stdin in `dir`, writes
    /// `stderr`, prints `stdout` and exits with `exit`.
    fn fake_tool(dir: &Path, stdout: &str, stderr: &str, exit: i32) -> PathBuf {
        let path = dir.join("fake-keyring-tool");
        let script = format!(
            "#!/bin/sh\nprintf '%s\\n' \"$@\" > '{d}/args'\nenv > '{d}/env'\ncat > '{d}/stdin'\nprintf '%s' '{stdout}'\nprintf '%s' '{stderr}' >&2\nexit {exit}\n",
            d = dir.display()
        );
        std::fs::write(&path, script).unwrap();
        std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o755)).unwrap();
        path
    }

    fn noted(dir: &Path, what: &str) -> String {
        std::fs::read_to_string(dir.join(what)).unwrap_or_default()
    }

    #[test]
    fn a_key_is_checked_for_form_without_being_quoted() {
        assert_eq!(check_format(&format!("  {KEY}\n")).unwrap(), SecretKey::new(KEY), "the whitespace around it is dropped");
        assert!(check_format(&"k".repeat(MAX_KEY)).is_ok());
        for bad in ["", "   ", "abc def", "abc\tdef", "abc\u{0}def", "abc\u{7f}def", "abcédef", &"k".repeat(MAX_KEY + 1)] {
            let e = check_format(bad).unwrap_err();
            assert_eq!(e.code, "design.picture.key_format", "{bad:?}");
            assert_eq!(e.hint.as_deref(), Some("paste the whole key"));
        }
        let e = check_format(&format!("{KEY} {KEY}")).unwrap_err();
        assert!(!e.message.contains(KEY) && !e.message.contains("sekrit"), "{}", e.message);
        assert_eq!(format!("{:?}", check_format(KEY).unwrap()), "SecretKey(…)");
    }

    #[test]
    fn the_key_goes_to_secret_tool_on_stdin_and_nowhere_else() {
        let dir = tempfile::tempdir().unwrap();
        let tool = fake_tool(dir.path(), "", "", 0);
        let cmd = store_command(&tool);
        let argv: Vec<String> = cmd.get_args().map(|a| a.to_string_lossy().into_owned()).collect();
        assert_eq!(argv, ["store", "--label=Relay Pictures: Pexels key", "application", "Relay", "purpose", "pictures-pexels"]);
        assert_eq!(cmd.get_envs().count(), 0, "the command sets no environment");
        save(&Keyring::Secret(tool), &SecretKey::new(KEY)).unwrap();
        assert_eq!(noted(dir.path(), "stdin"), KEY, "the key, and nothing after it");
        assert_eq!(noted(dir.path(), "args").lines().collect::<Vec<_>>(), argv);
        assert!(!noted(dir.path(), "env").contains(KEY), "not in the child's environment");
    }

    // A program that cannot be run is fed to the outcome mappings below, never exec'd: a failed
    // exec here interrupts other threads' socket reads that have a timeout (EINTR), which made
    // net.rs's loopback deadline test read a timeout as "offline".
    fn not_started() -> std::io::Result<Option<Output>> {
        Err(std::io::Error::from(std::io::ErrorKind::NotFound))
    }

    #[test]
    fn a_keyring_that_does_not_keep_the_key_says_so_without_it() {
        let dir = tempfile::tempdir().unwrap();
        let tool = fake_tool(dir.path(), "", "Cannot create an item in a locked collection", 1);
        let e = save(&Keyring::Secret(tool), &SecretKey::new(KEY)).unwrap_err();
        assert_eq!(e.code, "design.picture.keyring");
        assert_eq!(e.message, "the desktop keyring did not keep the key: Cannot create an item in a locked collection");
        let missing = written(not_started(), "keep").unwrap_err();
        assert_eq!(missing.code, "design.picture.keyring");
        assert!(missing.message.starts_with("secret-tool could not be started"), "{}", missing.message);
        let silent = written(Ok(None), "keep").unwrap_err();
        assert_eq!((silent.code.as_str(), silent.message.as_str()), ("design.picture.keyring", "the desktop keyring did not answer within 30 s"));
        assert!(!format!("{e:?}").contains(KEY));
    }

    #[test]
    fn a_lookup_answers_saved_absent_or_unavailable() {
        let dir = tempfile::tempdir().unwrap();
        let ring = |stdout: &str, stderr: &str, exit: i32| Keyring::Secret(fake_tool(dir.path(), stdout, stderr, exit));
        assert_eq!(load(&ring(&format!("{KEY}\n"), "", 0)), Key::Saved(SecretKey::new(KEY)));
        assert_eq!(noted(dir.path(), "args").lines().collect::<Vec<_>>(), ["lookup", "application", "Relay", "purpose", "pictures-pexels"]);
        assert_eq!(load(&ring("", "", 1)), Key::Absent, "no item matches");
        assert_eq!(load(&ring("", "", 0)), Key::Absent, "an empty secret is none");
        let before = Instant::now();
        let Key::Unavailable(until) = load(&ring("", "Cannot get secret of a locked object", 1)) else { panic!("a locked keyring") };
        assert!(until >= before + UNAVAILABLE_FOR);
        assert!(matches!(looked_up(not_started()), Key::Unavailable(_)), "no secret-tool");
        assert!(matches!(looked_up(Ok(None)), Key::Unavailable(_)), "no answer within 20 s");
        assert_eq!(load(&Keyring::Nowhere), Key::Absent);
    }

    #[test]
    fn forgetting_clears_and_nothing_saved_is_no_error() {
        let dir = tempfile::tempdir().unwrap();
        forget(&Keyring::Secret(fake_tool(dir.path(), "", "", 0))).unwrap();
        assert_eq!(noted(dir.path(), "args").lines().collect::<Vec<_>>(), ["clear", "application", "Relay", "purpose", "pictures-pexels"]);
        forget(&Keyring::Secret(fake_tool(dir.path(), "", "", 1))).expect("nothing to clear");
        let e = forget(&Keyring::Secret(fake_tool(dir.path(), "", "The keyring is locked", 1))).unwrap_err();
        assert_eq!((e.code.as_str(), e.message.as_str()), ("design.picture.keyring", "the desktop keyring did not forget the key: The keyring is locked"));
        forget(&Keyring::Nowhere).unwrap();
    }

    #[test]
    fn the_test_instance_keeps_its_key_in_a_private_file() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join(TEST_FILE);
        let ring = Keyring::File(path.clone());
        assert_eq!(load(&ring), Key::Absent);
        std::fs::write(&path, "old").unwrap();
        std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o644)).unwrap();
        save(&ring, &SecretKey::new(KEY)).unwrap();
        assert_eq!(std::fs::metadata(&path).unwrap().permissions().mode() & 0o777, 0o600, "even over a file that was there");
        assert_eq!(std::fs::read_to_string(&path).unwrap(), KEY);
        assert_eq!(load(&ring), Key::Saved(SecretKey::new(KEY)));
        forget(&ring).unwrap();
        assert!(!path.exists());
        forget(&ring).expect("forgetting twice");
        assert_eq!(load(&ring), Key::Absent);
        assert_eq!(save(&Keyring::Nowhere, &SecretKey::new(KEY)).unwrap_err().code, "design.picture.keyring", "never silently dropped");
    }
}
