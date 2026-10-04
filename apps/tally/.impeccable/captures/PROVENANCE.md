# Device captures

Emulator screenshots of Home with its real system bars, for the design finish review: the status
bar, the raised navigation bar and its indicator pill, the add FAB and the system insets, which
the Robolectric goldens in `app/src/test/screenshots` do not draw.

No captures yet. Run the **Device captures** workflow (`.github/workflows/device-captures.yml`)
on the branch to review. It replaces this file with the run, the commit and the device profiles
the PNGs came from, and commits them here:

| File | Device profile | Image | Font scale |
| --- | --- | --- | --- |
| phone.png | pixel_7 | API 34 google_apis x86_64 | 1.0 |
| phone-200.png | pixel_7 | API 34 google_apis x86_64 | 2.0 |
| tablet.png | pixel_tablet | API 34 google_apis x86_64 | 1.0 |
