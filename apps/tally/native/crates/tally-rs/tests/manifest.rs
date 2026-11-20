//! The Android manifest: every way in to the app names the native library, or GameActivity dies
//! before Rust runs (0.3 shipped with launcher aliases that did not, and crashed from the icon).

#[test]
fn every_launcher_entry_names_the_library() {
    let manifest = include_str!("../../../android/src/main/AndroidManifest.xml");
    let mut entries = 0;
    for tag in ["<activity\n", "<activity-alias"] {
        for block in manifest.split(tag).skip(1) {
            let end = block.find("</activity").expect("the entry closes");
            let body = &block[..end];
            if body.contains("android.intent.category.LAUNCHER") || tag == "<activity\n" {
                entries += 1;
                assert!(body.contains(r#"android:name="android.app.lib_name" android:value="tally""#), "an entry without lib_name:\n{body}");
            }
        }
    }
    assert!(entries >= 3, "the activity and both launcher icons, found {entries}");
}
