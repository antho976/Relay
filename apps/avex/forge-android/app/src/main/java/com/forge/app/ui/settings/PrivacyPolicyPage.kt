package com.forge.app.ui.settings

import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.rememberScrollState
import androidx.compose.foundation.verticalScroll
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.ui.Modifier
import androidx.compose.ui.unit.dp

@Composable
internal fun PrivacyPolicyPage(modifier: Modifier = Modifier) {
    Column(
        modifier = modifier
            .fillMaxSize()
            .verticalScroll(rememberScrollState())
            .padding(bottom = 56.dp)
    ) {
        SettingsSectionHeader("Privacy policy", top = 12.dp)
        SettingsExplainer(
            "Last updated September 26, 2026",
            Modifier.padding(horizontal = SETTINGS_GUTTER)
        )

        PolicySection(
            "Offline by design",
            "Avex is an offline, single-user fitness tracker. It has no account, servers, analytics, advertising, or Internet permission. Your data stays on your devices unless you export or share it."
        )
        PolicySection(
            "Data stored on your device",
            "Avex stores workouts, sets, cardio, programs, goals, notes, moods, trophies, personal records, body measurements, settings, coach history, Academy progress, crash logs, your profile, avatar, and progress photos in private app storage.",
            "With auto-backup on, Avex also keeps a weekly backup copy of this data, including progress photos, in the same private storage. Files you export are written there too before you choose where to send them."
        )
        PolicySection(
            "Health Connect",
            "If you grant access, Avex may read sleep, resting heart rate, weight, body fat, steps, exercise sessions, heart rate, distance, total calories, heart-rate variability, and lean body mass. If you also allow history access, the one-time weight import can read weight older than 30 days. These readings support recovery, cardio, body, and training-history features.",
            "Avex may write weight, body fat, active calories, exercise sessions, and heart rate when you enable the matching feature. Access is optional and granular. You can revoke it in Health Connect at any time, and Avex continues to work without it. Health Connect data is processed only on your device."
        )
        PolicySection(
            "Other permissions",
            "Camera access is used only when you take a progress photo. Biometrics protect the app or photo gallery through Android's system prompt. Notifications, vibration, and foreground services support reminders, rest timers, active workouts, and watch heart-rate capture. Wear body-sensor access is used only during a workout session."
        )
        PolicySection(
            "Sharing and retention",
            "Avex does not sell or share your data. Exports, backups, PDFs, images, and support files leave the app only when you choose a destination through Android. Files you share are then governed by that destination's privacy practices.",
            // Kept in step with .claude/PRIVACY.md. This said Settings could "erase all Avex data"
            // while factory reset kept the backup ZIP, exports and crash logs (2026-09-26 audit, D2);
            // the reset now erases them, and what it cannot reach is named.
            "Data remains on your device until you delete it, reset Avex, clear app storage, or uninstall. You can delete individual records in the app. Factory reset in Settings erases all Avex data stored in the app, including its backup copy, exports, and crash logs.",
            "Backups or exports you saved to a folder you picked, such as Downloads, a cloud drive, or the auto-backup folder, are outside the app and are not erased by a factory reset or an uninstall. Delete them there."
        )
        PolicySection(
            "Children and contact",
            "Avex is a personal fitness tool and is not directed at children. Questions about this policy can be sent to anthonybacon419@gmail.com. Material policy changes will update the date shown here and in the public policy."
        )
        Spacer(Modifier.height(8.dp))
    }
}

@Composable
private fun PolicySection(title: String, vararg paragraphs: String) {
    SettingsSectionHeader(title)
    Column(
        modifier = Modifier.padding(horizontal = SETTINGS_GUTTER),
        verticalArrangement = Arrangement.spacedBy(8.dp)
    ) {
        paragraphs.forEach { paragraph ->
            Text(
                paragraph,
                style = MaterialTheme.typography.bodyMedium,
                color = MaterialTheme.colorScheme.onSurfaceVariant
            )
        }
    }
}
