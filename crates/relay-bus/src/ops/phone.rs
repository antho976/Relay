//! `phone.*` — BUS.md §10.21: what the PC serves its paired phone app. Relay Code updates from
//! the PC it is linked to: `apps/relay-code/tools/publish-apk.sh` puts an APK and its
//! `latest.json` in the instance's data folder (`phone/`), and the phone reads them here.
use crate::registry::{OpMeta, Scope};
use crate::{op, Empty};

/// The largest piece of the APK one `phone.app.apk` answers, decoded: 3 MiB, 4 MiB in base64,
/// well under the remote door's 8 MiB message cap.
pub const APK_PIECE: u64 = 3 * 1024 * 1024;

result!(#[schemars(rename = "PhoneAppLatest")] AppLatest {
    /// The build: the commit count the APK was built at (Gradle's `versionCode`).
    pub build: u32,
    /// What the build is called (`0.312`).
    pub version: String,
    /// The APK's length in bytes.
    pub size: u64,
    /// Its SHA-256, lowercase hex: the phone checks the download against it.
    pub sha256: String,
    /// When it was published, ms since the epoch.
    pub made_at: i64,
});
op!(Latest, "phone.app.latest", Empty => Option<AppLatest>,
    OpMeta::query(Scope::Global, 12, "The phone app build the PC holds for its paired phone, or null when none is published"));

payload!(#[schemars(rename = "PhoneAppApkIn")] ApkIn {
    /// Where this piece starts, in bytes (default 0).
    pub offset: Option<u64>,
    /// At most this many bytes (default and most 3 MiB).
    pub len: Option<u64>,
});
result!(#[schemars(rename = "PhoneAppApkOut")] ApkOut {
    /// The whole APK's length in bytes.
    pub size: u64,
    /// This piece, base64.
    pub bytes_b64: String,
    /// This piece ends the file.
    pub eof: bool,
});
op!(Apk, "phone.app.apk", ApkIn => ApkOut,
    OpMeta::query(Scope::Global, 12, "The published phone app's APK, in pieces of at most 3 MiB from offset"));

entries!(Latest, Apk);
