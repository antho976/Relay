package com.forge.app.ui.settings

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
import androidx.compose.runtime.remember
import androidx.compose.ui.Modifier
import androidx.compose.ui.platform.LocalContext
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp
import com.forge.app.ui.theme.emphasized

/**
 * Settings → About. Shows the real installed version (read from PackageManager, so no BuildConfig
 * feature needed) and the app's privacy stance: Forge is fully offline and holds no INTERNET
 * permission, so every claim below is verifiable from the manifest.
 */
@Composable
internal fun AboutPage(modifier: Modifier = Modifier) {
    val onBg = MaterialTheme.colorScheme.onBackground
    val muted = MaterialTheme.colorScheme.onSurfaceVariant
    val context = LocalContext.current
    val version = remember {
        runCatching {
            context.packageManager.getPackageInfo(context.packageName, 0).versionName
        }.getOrNull().orEmpty()
    }

    Column(
        modifier = modifier
            .fillMaxSize()
            .verticalScroll(rememberScrollState())
            .padding(bottom = 56.dp)
    ) {
        Column(Modifier.padding(start = 24.dp, end = 24.dp, top = 20.dp, bottom = 16.dp)) {
            Text("FORGE", style = MaterialTheme.typography.headlineSmall, color = emphasized(onBg))
            Text(
                if (version.isBlank()) "Offline strength tracker" else "Version $version · Offline strength tracker",
                style = MaterialTheme.typography.labelSmall,
                color = muted,
                fontSize = 11.sp
            )
        }
        SectionDivider()

        SectionLabel("YOUR DATA STAYS ON THIS DEVICE")
        AboutParagraph(
            "Everything you log lives in a private database on this phone. There's no account and no " +
                "sign-in — Forge doesn't even request the Internet permission, so it physically can't " +
                "upload your data anywhere."
        )
        AboutParagraph(
            "No servers, no analytics, no tracking. Your data only moves when you choose to move it — " +
                "exporting or sharing a file, or saving a backup — and it goes exactly where you point it " +
                "through Android's own share / file picker."
        )
        AboutParagraph(
            "Keep your own backups from Settings → Export data → full backup; restoring on a new phone " +
                "brings everything back. Turn on Privacy to block screenshots and hide the app preview in " +
                "recent apps."
        )
        SectionDivider()

        SectionLabel("ABOUT")
        AboutParagraph(
            "A personal gym companion — auto-generated programs, an adaptive coach, progress stats, " +
                "trophies and a rank ladder. Built for lifting, not for the cloud."
        )
        Text(
            "Forge · a solo-built, offline-first project.",
            style = MaterialTheme.typography.labelSmall,
            color = muted.copy(alpha = 0.6f),
            modifier = Modifier.padding(horizontal = 24.dp, vertical = 8.dp)
        )
        Spacer(Modifier.height(8.dp))
    }
}

@Composable
private fun AboutParagraph(text: String) {
    Text(
        text,
        style = MaterialTheme.typography.bodySmall,
        color = MaterialTheme.colorScheme.onSurfaceVariant,
        modifier = Modifier.padding(horizontal = 24.dp, vertical = 8.dp)
    )
}
