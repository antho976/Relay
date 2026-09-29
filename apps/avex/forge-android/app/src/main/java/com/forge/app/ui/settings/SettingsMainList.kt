package com.forge.app.ui.settings

import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.lazy.LazyListState
import androidx.compose.foundation.lazy.itemsIndexed
import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.automirrored.rounded.KeyboardArrowRight
import androidx.compose.material.icons.rounded.DeleteForever
import androidx.compose.material.icons.rounded.Info
import androidx.compose.material.icons.rounded.RestartAlt
import androidx.compose.material.icons.rounded.SearchOff
import androidx.compose.material3.Icon
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.runtime.remember
import androidx.compose.ui.Modifier
import androidx.compose.ui.graphics.vector.ImageVector
import androidx.compose.ui.text.AnnotatedString
import androidx.compose.ui.text.SpanStyle
import androidx.compose.ui.text.buildAnnotatedString
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.text.withStyle
import androidx.compose.ui.unit.dp
import com.forge.app.ui.common.clickableLabeled
import com.forge.app.ui.nav.NavIcons

/**
 * The Settings root: a search pill over six groups, every row carrying its live value so the page
 * answers "how is this set up" before a tap. While a query is typed the groups give way to one
 * ranked list of hits, each of which goes straight to its page or action.
 */
@Composable
internal fun MainList(
    state: SettingsUiState,
    searchQuery: String,
    // Hoisted so the list keeps its scroll when a sub-page opens and closes over it.
    listState: LazyListState,
    onBack: () -> Unit,
    onSearchChange: (String) -> Unit,
    onOpenPage: (SettingsPage) -> Unit,
    onOpenCoachBrief: () -> Unit,
    onOpenDataDialog: () -> Unit,
    onImportData: () -> Unit,
    onResetTarget: (ResetTarget) -> Unit,
    onOpenResetMenu: () -> Unit
) {
    // Built once per query rather than on every recomposition, and holding targets rather than
    // lambdas so a fresh callback from the caller never invalidates it.
    val q = searchQuery.trim()
    val results = remember(q, state.freestyleMode) {
        if (q.isEmpty()) emptyList() else searchHits(q.lowercase(), state.freestyleMode)
    }
    val openResult: (SearchResult) -> Unit = { r ->
        when (r.action) {
            null -> r.page?.let(onOpenPage)
            SearchAction.DATA -> onOpenDataDialog()
            SearchAction.IMPORT -> onImportData()
            SearchAction.RESET -> onOpenResetMenu()
            SearchAction.FACTORY -> onResetTarget(ResetTarget.FACTORY)
            SearchAction.COACH -> onOpenCoachBrief()
        }
    }
    SettingsLazyScaffold(
        title = "Settings",
        onBack = onBack,
        listState = listState,
        // The hits are one lazy item each, 2dp apart like a group's rows, so while searching the
        // group spacing is placed by hand instead of between every item.
        verticalArrangement = if (searchQuery.isBlank()) Arrangement.spacedBy(KitGroupSpacing) else Arrangement.Top
    ) {
        item("search") { SettingsSearchBar(searchQuery, "Search settings", onSearchChange) }
        if (searchQuery.isBlank()) {
            item("general") {
                SettingsGroup("General") {
                    PageRow(SettingsPage.Appearance, state, onOpenPage)
                    PageRow(SettingsPage.Format, state, onOpenPage)
                    PageRow(SettingsPage.Notifications, state, onOpenPage)
                    PageRow(SettingsPage.Security, state, onOpenPage)
                }
            }
            item("training") {
                SettingsGroup("Training") {
                    PageRow(SettingsPage.Program, state, onOpenPage)
                    PageRow(SettingsPage.Session, state, onOpenPage)
                    PageRow(SettingsPage.ExercisePrefs, state, onOpenPage)
                    PageRow(SettingsPage.CardioActivities, state, onOpenPage)
                }
            }
            item("coach") {
                SettingsGroup("Coach & recovery") {
                    // Coach configuration only; the brief and its record live on the Coach tab.
                    // Freestyle has no plan to coach, so the row goes with the tab.
                    if (!state.freestyleMode) PageRow(SettingsPage.Coach, state, onOpenPage)
                    PageRow(SettingsPage.Recovery, state, onOpenPage)
                    PageRow(SettingsPage.Vacation, state, onOpenPage)
                }
            }
            item("data") {
                SettingsGroup("Data") {
                    PageRow(SettingsPage.Backup, state, onOpenPage)
                    SettingsNavigationRow("Export data", "Sessions · weekly · full backup · PDF", SettingsIcons.Export, onClick = onOpenDataDialog)
                    SettingsNavigationRow("Import data", "Strong · Hevy · FitNotes · CSV", SettingsIcons.Import, onClick = onImportData)
                    PageRow(SettingsPage.Storage, state, onOpenPage)
                }
            }
            item("reset") {
                SettingsGroup("Reset") {
                    // One entry for the targeted resets, and factory reset kept apart and marked.
                    SettingsNavigationRow("Reset…", "Sessions, trophies, cardio or settings", Icons.Rounded.RestartAlt, onClick = onOpenResetMenu)
                    SettingsNavigationRow(
                        ResetTarget.FACTORY.label,
                        "Erase everything Avex stores on this phone",
                        Icons.Rounded.DeleteForever,
                        tone = TileTone.Danger
                    ) { onResetTarget(ResetTarget.FACTORY) }
                }
            }
            item("about") {
                SettingsGroup("About") {
                    PageRow(SettingsPage.WhatsNew, state, onOpenPage)
                    PageRow(SettingsPage.About, state, onOpenPage)
                }
            }
        } else {
            // One flat, ranked list (see [searchHits]), each hit tapping straight through to where
            // the setting lives.
            if (results.isEmpty()) {
                item("empty") {
                    SettingsGroup(modifier = Modifier.padding(top = KitGroupSpacing)) {
                        SettingsEmptyBlock(
                            Icons.Rounded.SearchOff,
                            "No settings match “$q”",
                            "Try a broader word, like units, backup or lock."
                        )
                    }
                }
            } else {
                // One lazy item per hit, shaped by position like a group's rows: a broad query can
                // match dozens, and one item holding them all composed every row at once.
                item("results", contentType = "results-header") {
                    SettingsGroupHeader(
                        "${results.size} ${if (results.size == 1) "result" else "results"}",
                        Modifier.padding(top = KitGroupSpacing)
                    )
                }
                itemsIndexed(results, key = { _, r -> r.key }, contentType = { _, _ -> "result" }) { i, r ->
                    KitLazyRow(i, results.size) { SearchResultRow(r, q) { openResult(r) } }
                }
            }
        }
    }
}

/** A root row for a sub-page: its glyph, its name, its live value. */
@Composable
private fun PageRow(page: SettingsPage, state: SettingsUiState, onOpenPage: (SettingsPage) -> Unit) {
    SettingsNavigationRow(page.title, rowSubtitle(page, state), pageGlyph(page)) { onOpenPage(page) }
}

/** One resolved search hit: its display fields, a leading glyph and where it goes ([page], or
 *  [action] when it fires one). [key] is stable per entry across queries. */
private class SearchResult(
    val key: String,
    val name: String,
    val where: String,
    val glyph: ImageVector,
    val rank: Int,
    val page: SettingsPage? = null,
    val action: SearchAction? = null
)

/** Every hit for [ql] (lowercased), name-prefix hits first, then name substrings, then tag-only. */
private fun searchHits(ql: String, freestyleMode: Boolean): List<SearchResult> = buildList {
    PAGE_ENTRIES.forEachIndexed { i, pe ->
        if (ql in pe.page.title.lowercase() || ql in pe.tags) {
            add(SearchResult("page-$i", pe.page.title, pageSection(pe.page), pageGlyph(pe.page), searchRank(pe.page.title, ql), page = pe.page))
        }
    }
    ALL_ITEMS.forEachIndexed { i, item ->
        if (ql in item.name.lowercase() || ql in item.tags) {
            add(SearchResult("item-$i", item.name, item.page.title, pageGlyph(item.page), searchRank(item.name, ql), page = item.page))
        }
    }
    ACTION_ENTRIES.forEachIndexed { i, e ->
        val hidden = e.action == SearchAction.COACH && freestyleMode
        if (!hidden && (ql in e.name.lowercase() || ql in e.tags)) {
            add(SearchResult("action-$i", e.name, e.where, actionGlyph(e.action), searchRank(e.name, ql), action = e.action))
        }
    }
}.sortedWith(compareBy({ it.rank }, { it.name.lowercase() }))

/** Relevance: name-prefix (0) beats a name substring (1) beats a tag-only match (2). */
private fun searchRank(name: String, ql: String): Int {
    val n = name.lowercase()
    return when {
        n.startsWith(ql) -> 0
        ql in n -> 1
        else -> 2
    }
}

/** The group a page sits in on the root list, a page hit's breadcrumb. */
private fun pageSection(page: SettingsPage): String = when (page) {
    SettingsPage.Appearance, SettingsPage.Format, SettingsPage.Notifications, SettingsPage.Security -> "General"
    SettingsPage.Program, SettingsPage.Session, SettingsPage.ExercisePrefs, SettingsPage.CardioActivities -> "Training"
    SettingsPage.Coach, SettingsPage.Recovery, SettingsPage.Vacation -> "Coach & recovery"
    SettingsPage.Backup, SettingsPage.Storage -> "Data"
    SettingsPage.WhatsNew, SettingsPage.PrivacyPolicy, SettingsPage.About -> "About"
}

/** A page's glyph, shared by its root row and its search hits. */
internal fun pageGlyph(page: SettingsPage): ImageVector = when (page) {
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
    SettingsPage.PrivacyPolicy, SettingsPage.About -> Icons.Rounded.Info
}

private fun actionGlyph(action: SearchAction): ImageVector = when (action) {
    SearchAction.DATA -> SettingsIcons.Export
    SearchAction.IMPORT -> SettingsIcons.Import
    SearchAction.COACH -> NavIcons.Coach
    SearchAction.RESET -> Icons.Rounded.RestartAlt
    SearchAction.FACTORY -> Icons.Rounded.DeleteForever
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

/** A search hit: the page glyph, the name with its matched span bold, and where it lives. */
@Composable
private fun SearchResultRow(result: SearchResult, query: String, onClick: () -> Unit) {
    SettingsRowContainer(interaction = Modifier.clickableLabeled(result.name, onClick = onClick)) {
        SettingsIconTile(result.glyph)
        Column(Modifier.weight(1f), verticalArrangement = Arrangement.spacedBy(2.dp)) {
            Text(highlightMatch(result.name, query), style = MaterialTheme.typography.bodyLarge, color = MaterialTheme.colorScheme.onSurface)
            Text("In ${result.where}", style = MaterialTheme.typography.bodyMedium, color = MaterialTheme.colorScheme.onSurfaceVariant)
        }
        Icon(
            Icons.AutoMirrored.Rounded.KeyboardArrowRight,
            contentDescription = null,
            tint = MaterialTheme.colorScheme.onSurfaceVariant
        )
    }
}

internal fun rowSubtitle(page: SettingsPage, s: SettingsUiState): String = when (page) {
    SettingsPage.Appearance -> buildList {
        add(if (s.amoledMode) "Pure black" else "Warm dark")
        add(
            when {
                !s.accentEnabled -> "Monochrome"
                s.accentFromIcon && com.forge.app.appicon.AppIcon.fromKey(s.appIconKey).accentHex != null -> "Icon accent"
                else -> "${accentName(s.accentColorHex)} accent"
            }
        )
    }.joinToString(" · ")
    SettingsPage.Format -> "${s.weightUnit.label} · ${if (s.useMiles) "mi" else "km"} · ${if (s.useCm) "cm" else "in"} · ${if (s.timeFormat24h) "24h" else "12h"} · week from ${if (s.firstDayMonday) "Mon" else "Sun"}"
    SettingsPage.Session -> "${s.hapticStrength.replaceFirstChar { it.uppercase() }} haptics · ${restLabel(s.restCompoundSeconds)} rest"
    SettingsPage.Notifications -> {
        val on = buildList {
            if (s.trainingReminderEnabled) add("Reminders")
            if (s.weeklyRecapEnabled) add("recap")
            if (s.restTimerAlertEnabled) add("timer")
        }.joinToString(" · ").replaceFirstChar { it.uppercase() }.ifEmpty { "All off" }
        if (s.quietHoursEnabled) {
            // One window shared by every day reads as a time range; a per-day schedule reads "per day".
            val w = s.quietHoursSchedule.windows[0]
            val quiet =
                if (s.quietHoursSchedule.isUniform && !w.isOff) "quiet ${windowLabel(w, s.timeFormat24h)}"
                else "quiet per day"
            "$on · $quiet"
        } else on
    }
    SettingsPage.Program -> "${s.daysPerWeek} days a week · ${if (s.availableEquipment.isEmpty()) "all equipment" else "${s.availableEquipment.size} pieces of gear"}"
    SettingsPage.Coach -> when {
        !s.coachEnabled -> "Off"
        s.coachMode == "auto" -> "On · earning auto-apply"
        else -> "On · suggests every change"
    }
    SettingsPage.Security -> buildList {
        if (s.appLockEnabled) add("App lock")
        if (s.galleryLockEnabled) add("gallery lock")
        if (s.privacyMode) add("privacy mode")
    }.joinToString(" · ").replaceFirstChar { it.uppercase() }.ifEmpty { "No lock set" }
    SettingsPage.Recovery -> "Health Connect · sleep, weight, workouts"
    SettingsPage.ExercisePrefs -> when {
        s.liked.isEmpty() && s.disliked.isEmpty() -> "Prefer or hide any movement"
        else -> "${s.liked.size} preferred · ${s.disliked.size} hidden"
    }
    // The count lives on its own StateFlow, not SettingsUiState, so the row shows what it is for.
    SettingsPage.CardioActivities -> "Sports the built-in list misses"
    SettingsPage.Vacation -> "Pause your streak while you're away"
    SettingsPage.Backup -> "Weekly copy · back up now"
    SettingsPage.Storage -> "Space used · clear cache"
    SettingsPage.WhatsNew -> "Version ${com.forge.app.BuildConfig.VERSION_NAME}"
    SettingsPage.PrivacyPolicy -> "Offline use · permissions · deletion"
    SettingsPage.About -> "Privacy, gestures, licenses"
}

/** "2:30" for a rest time in seconds. */
internal fun restLabel(seconds: Int): String = "${seconds / 60}:${(seconds % 60).toString().padStart(2, '0')}"
