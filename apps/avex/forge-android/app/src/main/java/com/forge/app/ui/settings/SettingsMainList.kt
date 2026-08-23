package com.forge.app.ui.settings

import androidx.compose.foundation.background
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.PaddingValues
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.lazy.LazyColumn
import androidx.compose.foundation.lazy.LazyListState
import androidx.compose.foundation.lazy.itemsIndexed
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.foundation.text.BasicTextField
import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.filled.Close
import androidx.compose.material.icons.filled.Search
import androidx.compose.material3.Icon
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.clip
import androidx.compose.ui.graphics.SolidColor
import androidx.compose.ui.graphics.vector.ImageVector
import androidx.compose.ui.text.AnnotatedString
import androidx.compose.ui.text.SpanStyle
import androidx.compose.ui.text.buildAnnotatedString
import androidx.compose.ui.text.font.FontStyle
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.text.style.TextAlign
import androidx.compose.ui.text.withStyle
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp
import com.forge.app.ui.common.clickableLabeled
import com.forge.app.ui.nav.NavIcons

@Composable
internal fun MainList(
    state: SettingsUiState,
    searchQuery: String,
    modifier: Modifier,
    // Hoisted so the root list's scroll position survives opening a sub-page and backing out — this
    // composable leaves composition while a sub-page is shown, so an internal state would reset to top.
    listState: LazyListState,
    onSearchChange: (String) -> Unit,
    onOpenPage: (SettingsPage) -> Unit,
    onOpenCoachBrief: () -> Unit,
    onOpenDataDialog: () -> Unit,
    onImportData: () -> Unit,
    onResetTarget: (ResetTarget) -> Unit,
    onOpenResetMenu: () -> Unit
) {
    LazyColumn(state = listState, modifier = modifier, contentPadding = PaddingValues(bottom = 56.dp)) {
        // Persistent search field at the top of the list (modern-phone-settings pattern) — replaces
        // the old top-bar magnifier toggle. Always visible, in both the grouped and results states.
        item("search") {
            SettingsSearchField(
                query = searchQuery,
                placeholder = "Search settings",
                onQueryChange = onSearchChange,
                modifier = Modifier.padding(start = SETTINGS_GUTTER, end = SETTINGS_GUTTER, top = 12.dp, bottom = 4.dp)
            )
        }
        if (searchQuery.isBlank()) {
            // Grouped under quiet mono anchors, separated by air alone — no per-row hairlines
            // (DESIGN §1/§7: lines mean data; sections separate by whitespace + their header). Each
            // row carries a muted leading glyph from the nav-bar family (SettingsIcons) for wayfinding.
            item("general") {
                SettingsSectionHeader("General", top = 8.dp)
                SettingsNavRow("Appearance", rowSubtitle(SettingsPage.Appearance, state), SettingsIcons.Appearance) { onOpenPage(SettingsPage.Appearance) }
                SettingsNavRow("Units & format", rowSubtitle(SettingsPage.Format, state), SettingsIcons.Units) { onOpenPage(SettingsPage.Format) }
                SettingsNavRow("Notifications", rowSubtitle(SettingsPage.Notifications, state), SettingsIcons.Notifications) { onOpenPage(SettingsPage.Notifications) }
                SettingsNavRow("Security", rowSubtitle(SettingsPage.Security, state), SettingsIcons.Security) { onOpenPage(SettingsPage.Security) }
            }
            item("training") {
                SettingsSectionHeader("Training")
                SettingsNavRow("Program & equipment", rowSubtitle(SettingsPage.Program, state), SettingsIcons.Program) { onOpenPage(SettingsPage.Program) }
                SettingsNavRow("Session", rowSubtitle(SettingsPage.Session, state), SettingsIcons.Session) { onOpenPage(SettingsPage.Session) }
                SettingsNavRow("Exercise likes", rowSubtitle(SettingsPage.ExercisePrefs, state), SettingsIcons.Likes) { onOpenPage(SettingsPage.ExercisePrefs) }
                SettingsNavRow("Cardio activities", "Your own cardio activities", NavIcons.Cardio) { onOpenPage(SettingsPage.CardioActivities) }
            }
            item("coach") {
                SettingsSectionHeader("Coach")
                // Coach CONFIGURATION only (on/off + mode) — the brief, trust and history live on
                // the Coach tab. Hidden for freestyle users — no plan to coach against (matches the
                // hub tab); Wearable + Holiday stay, they aren't coach-only.
                if (!state.freestyleMode) {
                    SettingsNavRow("Your coach", rowSubtitle(SettingsPage.Coach, state), NavIcons.Coach) { onOpenPage(SettingsPage.Coach) }
                }
                SettingsNavRow("Wearable", rowSubtitle(SettingsPage.Recovery, state), SettingsIcons.Wearable) { onOpenPage(SettingsPage.Recovery) }
                SettingsNavRow("Holiday / Vacation", "Pause your streak during a holiday", SettingsIcons.Holiday) { onOpenPage(SettingsPage.Vacation) }
            }
            item("data") {
                SettingsSectionHeader("Data")
                SettingsNavRow("Backup", rowSubtitle(SettingsPage.Backup, state), SettingsIcons.Backup) { onOpenPage(SettingsPage.Backup) }
                SettingsNavRow("Export data", "Sessions · weekly · full backup · PDF", SettingsIcons.Export) { onOpenDataDialog() }
                SettingsNavRow("Import data", "Strong · Hevy · FitNotes · CSV", SettingsIcons.Import) { onImportData() }
                SettingsNavRow("Storage", rowSubtitle(SettingsPage.Storage, state), SettingsIcons.Storage) { onOpenPage(SettingsPage.Storage) }
            }
            item("reset") {
                SettingsSectionHeader("Reset")
                // One umbrella entry opens a chooser for the targeted resets…
                DestructiveRow("Reset…") { onOpenResetMenu() }
                // …and a factory reset stays a separate, clearly more dangerous button.
                DestructiveRow(ResetTarget.FACTORY.label, isFactory = true) { onResetTarget(ResetTarget.FACTORY) }
            }
            item("about") {
                SettingsSectionHeader("About")
                // What's new (GYMAP-71) — subtitle carries the live app version; taps to the changelog.
                SettingsNavRow("What's new", rowSubtitle(SettingsPage.WhatsNew, state), SettingsIcons.WhatsNew) { onOpenPage(SettingsPage.WhatsNew) }
                Spacer(Modifier.height(20.dp))
                // About Avex stays a quiet footer link at the very bottom of the app.
                AboutLink { onOpenPage(SettingsPage.About) }
                Spacer(Modifier.height(8.dp))
            }
        } else {
            // Flat, ranked, individually-tappable results (modern phone-settings search): every
            // matched setting is its own row — page glyph + name (matched span emphasized) + its
            // location + tap straight to the destination. Name-prefix hits rank first, then substring,
            // then tag-only matches; the whole thing sorts by rank, then alphabetically.
            val q = searchQuery.trim()
            val ql = q.lowercase()
            val results = buildList {
                // Whole-page hits first ("app" → Appearance) …
                PAGE_ENTRIES.forEach { pe ->
                    if (ql in pe.page.title.lowercase() || ql in pe.tags) {
                        add(SearchResult(pe.page.title, pageSection(pe.page), pageGlyph(pe.page), "→", searchRank(pe.page.title, ql)) {
                            onOpenPage(pe.page)
                        })
                    }
                }
                // … then individual settings within a page.
                ALL_ITEMS.forEach { item ->
                    if (ql in item.name.lowercase() || ql in item.tags) {
                        add(SearchResult(item.name, item.page.title, pageGlyph(item.page), "→", searchRank(item.name, ql)) {
                            onOpenPage(item.page)
                        })
                    }
                }
                ACTION_ENTRIES.forEach { e ->
                    val hidden = e.action == SearchAction.COACH && state.freestyleMode
                    if (!hidden && (ql in e.name.lowercase() || ql in e.tags)) {
                        val onClick: () -> Unit = when (e.action) {
                            SearchAction.DATA -> onOpenDataDialog
                            SearchAction.IMPORT -> onImportData
                            SearchAction.RESET -> onOpenResetMenu
                            SearchAction.COACH -> onOpenCoachBrief
                        }
                        add(SearchResult(e.name, e.where, actionGlyph(e.action), "↗", searchRank(e.name, ql), onClick))
                    }
                }
            }.sortedWith(compareBy({ it.rank }, { it.name.lowercase() }))

            if (results.isEmpty()) {
                item("empty") { NoSearchResults(q) }
            } else {
                item("count") { SearchCount(results.size) }
                itemsIndexed(results, key = { i, _ -> "res-$i" }) { _, r ->
                    SearchResultRow(r, q)
                }
            }
        }
    }
}

/** A quiet, centered footer link to the About page — sits at the very bottom of the settings list. */
@Composable
private fun AboutLink(onClick: () -> Unit) {
    Text(
        "About Avex",
        style = MaterialTheme.typography.labelSmall,
        color = MaterialTheme.colorScheme.onSurfaceVariant,
        textAlign = TextAlign.Center,
        modifier = Modifier
            .fillMaxWidth()
            .clickableLabeled("About Avex", onClick = onClick)
            .padding(vertical = SETTINGS_ROW_PAD)
    )
}

/** One resolved search hit: its display fields, a leading glyph, a trailing marker and where it goes. */
private class SearchResult(
    val name: String,
    val where: String,
    val glyph: ImageVector?,
    val trailing: String,
    val rank: Int,
    val onClick: () -> Unit
)

/** Relevance: name-prefix (0) beats a name substring (1) beats a tag-only match (2). */
private fun searchRank(name: String, ql: String): Int {
    val n = name.lowercase()
    return when {
        n.startsWith(ql) -> 0
        ql in n -> 1
        else -> 2
    }
}

/** The section a page sits under in the main list — a page result's breadcrumb. */
private fun pageSection(page: SettingsPage): String = when (page) {
    SettingsPage.Appearance, SettingsPage.Format, SettingsPage.Notifications, SettingsPage.Security -> "General"
    SettingsPage.Program, SettingsPage.Session, SettingsPage.ExercisePrefs, SettingsPage.CardioActivities -> "Training"
    SettingsPage.Coach, SettingsPage.Recovery, SettingsPage.Vacation -> "Coach"
    SettingsPage.Backup, SettingsPage.Storage -> "Data"
    SettingsPage.WhatsNew, SettingsPage.About -> "About"
}

/** The leading glyph for a page's results — the same family the nav rows use. */
private fun pageGlyph(page: SettingsPage): ImageVector? = when (page) {
    SettingsPage.Appearance -> SettingsIcons.Appearance
    SettingsPage.Format -> SettingsIcons.Units
    SettingsPage.Session -> SettingsIcons.Session
    SettingsPage.Notifications -> SettingsIcons.Notifications
    SettingsPage.Security -> SettingsIcons.Security
    SettingsPage.Program -> SettingsIcons.Program
    SettingsPage.Coach -> NavIcons.Coach
    SettingsPage.Recovery -> SettingsIcons.Wearable
    SettingsPage.ExercisePrefs -> SettingsIcons.Likes
    SettingsPage.CardioActivities -> NavIcons.Cardio
    SettingsPage.Vacation -> SettingsIcons.Holiday
    SettingsPage.Backup -> SettingsIcons.Backup
    SettingsPage.Storage -> SettingsIcons.Storage
    SettingsPage.WhatsNew -> SettingsIcons.WhatsNew
    SettingsPage.About -> null
}

private fun actionGlyph(action: SearchAction): ImageVector? = when (action) {
    SearchAction.DATA -> SettingsIcons.Export
    SearchAction.IMPORT -> SettingsIcons.Import
    SearchAction.COACH -> NavIcons.Coach
    SearchAction.RESET -> null
}

/** Bold the matched span within a result name so it's clear WHY the row surfaced. */
private fun highlightMatch(name: String, query: String): AnnotatedString = buildAnnotatedString {
    val idx = if (query.isEmpty()) -1 else name.indexOf(query, ignoreCase = true)
    if (idx < 0) {
        append(name)
    } else {
        append(name.substring(0, idx))
        withStyle(SpanStyle(fontWeight = FontWeight.Bold)) { append(name.substring(idx, idx + query.length)) }
        append(name.substring(idx + query.length))
    }
}

/** A single search result: leading glyph · name (matched span bold) + location · trailing marker. */
@Composable
private fun SearchResultRow(result: SearchResult, query: String) {
    val onBg = MaterialTheme.colorScheme.onBackground
    val muted = MaterialTheme.colorScheme.onSurfaceVariant
    Row(
        modifier = Modifier
            .fillMaxWidth()
            .clickableLabeled(result.name, onClick = result.onClick)
            .padding(horizontal = SETTINGS_GUTTER, vertical = SETTINGS_ROW_PAD),
        verticalAlignment = Alignment.CenterVertically,
        horizontalArrangement = Arrangement.spacedBy(16.dp)
    ) {
        // Keep the text column aligned whether or not this hit has a glyph (Reset actions don't).
        if (result.glyph != null) {
            Icon(result.glyph, contentDescription = null, tint = muted, modifier = Modifier.size(20.dp))
        } else {
            Spacer(Modifier.size(20.dp))
        }
        Column(Modifier.weight(1f), verticalArrangement = Arrangement.spacedBy(2.dp)) {
            Text(highlightMatch(result.name, query), style = MaterialTheme.typography.bodyMedium, color = onBg)
            SettingsExplainer(result.where)
        }
        Text(result.trailing, style = MaterialTheme.typography.bodyMedium, color = muted)
    }
}

/** A quiet count line above the results. */
@Composable
private fun SearchCount(n: Int) {
    Text(
        "$n ${if (n == 1) "result" else "results"}",
        style = MaterialTheme.typography.labelSmall,
        color = MaterialTheme.colorScheme.onSurfaceVariant,
        letterSpacing = 1.sp,
        modifier = Modifier.padding(start = SETTINGS_GUTTER, end = SETTINGS_GUTTER, top = 16.dp, bottom = 4.dp)
    )
}

@Composable
private fun NoSearchResults(query: String) {
    Text(
        "No settings match “$query”",
        style = MaterialTheme.typography.bodyMedium,
        color = MaterialTheme.colorScheme.onSurfaceVariant,
        modifier = Modifier.padding(horizontal = SETTINGS_GUTTER, vertical = 28.dp)
    )
}

internal fun rowSubtitle(page: SettingsPage, s: SettingsUiState): String = when (page) {
    SettingsPage.Appearance -> "AMOLED ${if (s.amoledMode) "on" else "off"} · compact ${if (s.compactSetLogging) "on" else "off"}"
    SettingsPage.Format -> "${s.weightUnit.label} · ${if (s.useMiles) "mi" else "km"} · ${if (s.useCm) "cm" else "in"} · ${dateShort(s.dateFormat)} · ${if (s.timeFormat24h) "24h" else "12h"} · ${tzShort(s.timezone)}"
    SettingsPage.Session -> "Haptic: ${s.hapticStrength}"
    SettingsPage.Notifications -> {
        // Live preview of which notification types are on (was just quiet-hours / "Off").
        val on = buildList {
            if (s.trainingReminderEnabled) add("reminders")
            if (s.weeklyRecapEnabled) add("recap")
            if (s.restTimerAlertEnabled) add("timer")
        }.joinToString(" · ").ifEmpty { "Off" }
        if (s.quietHoursEnabled) {
            // One window shared by every day reads as a time range; a per-day schedule just reads "per day".
            val w = s.quietHoursSchedule.windows[0]
            val quiet =
                if (s.quietHoursSchedule.isUniform && !w.isOff)
                    "quiet ${w.start.toString().padStart(2, '0')}:00–${w.end.toString().padStart(2, '0')}:00"
                else "quiet per day"
            "$on · $quiet"
        } else on
    }
    SettingsPage.Program -> "${s.daysPerWeek} days/week · ${if (s.availableEquipment.isEmpty()) "all equipment" else "${s.availableEquipment.size} equipment"}"
    SettingsPage.Coach -> when {
        !s.coachEnabled -> "Off"
        s.coachMode == "auto" -> "On · earning auto-apply"
        else -> "On · suggest mode"
    }
    SettingsPage.Security -> "App lock ${if (s.appLockEnabled) "on" else "off"} · gallery ${if (s.galleryLockEnabled) "on" else "off"}"
    SettingsPage.Recovery -> "Health Connect · sleep & resting HR"
    SettingsPage.ExercisePrefs -> "${s.liked.size} liked · ${s.disliked.size} disliked"
    // Count lives on its own StateFlow (not SettingsUiState), so search shows a static subtitle.
    SettingsPage.CardioActivities -> "Your own cardio activities"
    // Reached via a dedicated MainList row, not the search/nav grid — subtitle unused.
    SettingsPage.Vacation -> "Pause your streak during a holiday"
    // Live status (last-run date, on/off) lives on its own StateFlow — the row shows a static hint.
    SettingsPage.Backup -> "Weekly copy · back up now"
    // Live sizes live on their own StateFlow (not SettingsUiState) — the row shows a static hint.
    SettingsPage.Storage -> "Space used · clear cache"
    // The live app version — the built VERSION_NAME, so the row reads current at a glance.
    SettingsPage.WhatsNew -> "Version ${com.forge.app.BuildConfig.VERSION_NAME}"
    SettingsPage.About -> "Version · privacy · what's stored"
}

internal fun dateShort(f: String) = when (f) {
    "dd/MM/yyyy" -> "05/01"
    "MM/dd/yyyy" -> "01/05"
    else -> "Jan 5"
}

internal fun tzShort(id: String) = when (id) {
    "America/Los_Angeles" -> "PST"; "America/Denver" -> "MST"; "America/Chicago" -> "CST"
    "America/New_York" -> "EST"; "America/Sao_Paulo" -> "BRT"; "UTC" -> "UTC"
    "Europe/London" -> "GMT"; "Europe/Paris" -> "CET"; "Europe/Moscow" -> "MSK"
    "Asia/Kolkata" -> "IST"; "Asia/Tokyo" -> "JST"; "Australia/Sydney" -> "AEST"
    else -> id.substringAfterLast("/")
}
