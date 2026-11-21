package com.forge.app.ui.settings

/**
 * The app's release notes, powering Settings → What's new. Hand-authored and shipped IN the binary
 * (the app holds no INTERNET permission, so there's nothing to fetch) — when you cut a build, add a
 * new [ReleaseNote] at the TOP of [CHANGELOG]. Newest first; each line is dry and grounded in a real
 * change (DESIGN §11), ordered New → Improved → Fixed so the read flows the same way every release.
 *
 * NOTE: the versions/dates below are a starting draft grouped by the git minor versions — reconcile
 * them with what actually shipped in each tester build before release.
 */

/** A single changelog line's kind — becomes its quiet mono tag in the What's new page. */
internal enum class ChangeKind(val tag: String) {
    New("New"),
    Improved("Improved"),
    Fixed("Fixed")
}

/** One line of a release: what changed, tagged by [kind]. */
internal data class ChangeNote(val kind: ChangeKind, val text: String)

/** One release: a version + a human date + its notes (already ordered New → Improved → Fixed). */
internal data class ReleaseNote(val version: String, val date: String, val notes: List<ChangeNote>)

internal val CHANGELOG: List<ReleaseNote> = listOf(
    ReleaseNote(
        version = "0.9",
        date = "Aug 2026",
        notes = listOf(
            ChangeNote(ChangeKind.New, "Train from the Wear OS companion with live heart rate, tiles and complications"),
            ChangeNote(ChangeKind.New, "Browse the rebuilt Academy and read the full training library offline"),
            ChangeNote(ChangeKind.New, "See monthly activity, streaks and body trends together on your profile"),
            ChangeNote(ChangeKind.Improved, "Onboarding now builds your plan first and keeps optional settings for the end"),
            ChangeNote(ChangeKind.Improved, "Settings, notifications and app icons now use the finished Avex design"),
            ChangeNote(ChangeKind.Fixed, "Health Connect privacy details now open directly inside Avex"),
        )
    ),
    ReleaseNote(
        version = "0.8.8",
        date = "Jul 2026",
        notes = listOf(
            ChangeNote(ChangeKind.New, "Track waist, chest, arm, thigh and hip measurements from your profile"),
            ChangeNote(ChangeKind.New, "See your whole training year as a day grid on your profile"),
            ChangeNote(ChangeKind.New, "Share a before-and-after progress photo card"),
            ChangeNote(ChangeKind.Improved, "Log your bodyweight for a past day, and add a note"),
            ChangeNote(ChangeKind.Fixed, "Cardio pace now rounds correctly"),
        )
    ),
    ReleaseNote(
        version = "0.8.7",
        date = "Jun 2026",
        notes = listOf(
            ChangeNote(ChangeKind.New, "Add your own cardio activities with a name and icon"),
            ChangeNote(ChangeKind.New, "Log a freestyle workout and reorder exercises as you go"),
            ChangeNote(ChangeKind.New, "Note the weather conditions on a cardio session"),
            ChangeNote(ChangeKind.Improved, "Cardio fields now match the activity: incline, laps or elevation gain"),
            ChangeNote(ChangeKind.Improved, "Export cardio to CSV, and enter duration in hours and minutes"),
        )
    ),
    ReleaseNote(
        version = "0.8.6",
        date = "Jun 2026",
        notes = listOf(
            ChangeNote(ChangeKind.New, "Choose your app icon, with a matching startup animation"),
            ChangeNote(ChangeKind.New, "Rebuilt progress photo gallery with before-and-after compare"),
            ChangeNote(ChangeKind.New, "Timed holds like planks and dead hangs now log a duration"),
            ChangeNote(ChangeKind.Improved, "Freestyle sets carry RPE, warmup, drop, failure and AMRAP tags"),
        )
    ),
)
