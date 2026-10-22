@file:OptIn(androidx.compose.foundation.layout.ExperimentalLayoutApi::class)

package com.forge.app.ui.settings

import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.FlowRow
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.lazy.LazyListState
import androidx.compose.foundation.lazy.itemsIndexed
import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.rounded.SearchOff
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.runtime.remember
import androidx.compose.ui.Modifier
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.text.AnnotatedString
import androidx.compose.ui.text.SpanStyle
import androidx.compose.ui.text.buildAnnotatedString
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.text.withStyle
import androidx.compose.ui.unit.dp
import com.forge.app.data.db.entities.VacationPeriod
import com.forge.app.ui.common.OpenTopBarIcon
import com.forge.app.ui.common.clickableLabeled
import com.forge.app.ui.common.touchSlop
import com.forge.app.ui.theme.OpenIcons
import com.forge.app.ui.theme.OpenType
import com.forge.app.ui.theme.SettingsColors
import com.forge.app.ui.theme.SettingsType
import java.time.LocalDate
import java.time.format.DateTimeFormatter

/**
 * The Settings root (Relay design "st"): an index of every page. A back arrow and the search glyph
 * in the bar, the serif title and the version, then three sections under mono labels (Training, App,
 * Data), each row a page's name over its live value in the mono reading voice and a chevron, so the
 * page answers "how is this set up" before a tap. The rare things close the page: the pages about
 * Avex itself as outlined buttons, then Reset… and Factory reset as plain words.
 *
 * The search is compact, as drawn: the glyph opens a field in the bar itself, and while a query is
 * typed the sections give way to one ranked list of hits, each going straight to its page or action.
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
    /** A search hit inside Program opens its section, not the Program menu. */
    onOpenSection: (SettingsPage, ProgramSection?) -> Unit = { page, _ -> onOpenPage(page) },
    onOpenCoachBrief: () -> Unit,
    onOpenDataDialog: () -> Unit,
    onImportData: () -> Unit,
    onResetTarget: (ResetTarget) -> Unit,
    onOpenResetMenu: () -> Unit,
    /** The Backup row's live value (see [backupSubtitle]); null keeps the row's description. */
    backupStatus: String? = null,
    /** Readings that live on their own flows rather than [SettingsUiState]. */
    customCardioCount: Int = 0,
    holidayStatus: String = "None planned",
    /** Space used, once measured; null until then. */
    storageUsed: String? = null,
    /** The Watch & health row's value (see [watchReading]); null keeps the row's description. */
    watchStatus: String? = null,
    /** The active plan's split ("Upper · Lower", see [splitLabel]); blank leaves it out. */
    split: String = "",
    /** Whether the search field is open in the bar. A typed query keeps it open. */
    searchOpen: Boolean = false,
    onSearchOpenChange: (Boolean) -> Unit = {}
) {
    // Built once per query rather than on every recomposition, and holding targets rather than
    // lambdas so a fresh callback from the caller never invalidates it.
    val q = searchQuery.trim()
    val results = remember(q, state.freestyleMode) {
        if (q.isEmpty()) emptyList() else searchHits(q.lowercase(), state.freestyleMode)
    }
    val openResult: (SearchResult) -> Unit = { r ->
        when (r.action) {
            null -> r.page?.let { page -> onOpenSection(page, r.section) }
            SearchAction.DATA -> onOpenDataDialog()
            SearchAction.IMPORT -> onImportData()
            SearchAction.RESET -> onOpenResetMenu()
            SearchAction.FACTORY -> onResetTarget(ResetTarget.FACTORY)
            SearchAction.COACH -> onOpenCoachBrief()
        }
    }
    val page: @Composable (SettingsPage) -> Unit = { p ->
        SettingsNavigationRow(p.title, rowSubtitle(p, state, split)) { onOpenPage(p) }
    }
    val showSearch = searchOpen || searchQuery.isNotBlank()
    SettingsLazyScaffold(
        title = "Settings",
        subtitle = "Avex ${com.forge.app.BuildConfig.VERSION_NAME}",
        onBack = onBack,
        listState = listState,
        topBarTrailing = {
            OpenTopBarIcon(OpenIcons.Search, "Search settings", { onSearchOpenChange(true) })
        },
        topBarContent = if (showSearch) {
            {
                SettingsSearchField(
                    query = searchQuery,
                    placeholder = "Search settings",
                    onQueryChange = onSearchChange,
                    onBack = onBack,
                    onClose = { onSearchChange(""); onSearchOpenChange(false) }
                )
            }
        } else null
    ) {
        if (searchQuery.isBlank()) {
            // What shapes the training first: the plan, the coach that adjusts it, how a session
            // runs, which movements it draws on, and the weeks it should leave alone.
            item("training") {
                SettingsGroup("Training") {
                    page(SettingsPage.Program)
                    // Coach configuration only; the brief and its record live on the Coach tab.
                    // Freestyle has no plan to coach, so the row goes with the tab.
                    if (!state.freestyleMode) page(SettingsPage.Coach)
                    page(SettingsPage.Session)
                    page(SettingsPage.ExercisePrefs)
                    SettingsNavigationRow(SettingsPage.CardioActivities.title, cardioReading(customCardioCount)) {
                        onOpenPage(SettingsPage.CardioActivities)
                    }
                    SettingsNavigationRow(SettingsPage.Vacation.title, holidayStatus) { onOpenPage(SettingsPage.Vacation) }
                }
            }
            item("app") {
                SettingsGroup("App", Modifier.padding(top = RootSectionGap)) {
                    page(SettingsPage.Appearance)
                    page(SettingsPage.Format)
                    page(SettingsPage.Notifications)
                    page(SettingsPage.Security)
                    SettingsNavigationRow(SettingsPage.Recovery.title, watchStatus ?: rowSubtitle(SettingsPage.Recovery, state)) {
                        onOpenPage(SettingsPage.Recovery)
                    }
                }
            }
            item("data") {
                SettingsGroup("Data", Modifier.padding(top = RootSectionGap)) {
                    // The backup's state, not what the page is for: the date of the last copy, or
                    // that it failed, is the one thing worth knowing before a phone gets lost.
                    SettingsNavigationRow(SettingsPage.Backup.title, backupStatus ?: rowSubtitle(SettingsPage.Backup, state)) {
                        onOpenPage(SettingsPage.Backup)
                    }
                    SettingsNavigationRow("Export data", "Sessions, weekly, full backup, PDF", onClick = onOpenDataDialog)
                    SettingsNavigationRow("Import data", "Strong, Hevy, FitNotes, CSV", onClick = onImportData)
                    SettingsNavigationRow(SettingsPage.Storage.title, storageUsed?.let { "$it used" } ?: rowSubtitle(SettingsPage.Storage, state)) {
                        onOpenPage(SettingsPage.Storage)
                    }
                }
            }
            item("about") {
                // About Avex rather than about your training, so not one more section of rows.
                SettingsButtonBar(Modifier.padding(top = RootSectionGap)) {
                    listOf(SettingsPage.WhatsNew, SettingsPage.About, SettingsPage.PrivacyPolicy).forEach { p ->
                        SettingsCompactButton(p.title, height = 36.dp) { onOpenPage(p) }
                    }
                }
            }
            item("reset") {
                // The rare, erasing acts last, as words: the targeted resets behind one menu, and
                // factory reset in the soft danger red. What each erases is its confirm's to say.
                FlowRow(
                    Modifier.padding(top = 24.dp),
                    horizontalArrangement = Arrangement.spacedBy(24.dp),
                    verticalArrangement = Arrangement.spacedBy(20.dp)
                ) {
                    RootTextAction("Reset…", MaterialTheme.colorScheme.onSurfaceVariant, onOpenResetMenu)
                    RootTextAction(ResetTarget.FACTORY.label, SettingsColors.Danger) { onResetTarget(ResetTarget.FACTORY) }
                }
            }
        } else {
            // One flat, ranked list (see [searchHits]), each hit tapping straight through to where
            // the setting lives.
            if (results.isEmpty()) {
                item("empty") {
                    SettingsEmptyBlock(
                        Icons.Rounded.SearchOff,
                        "No settings match “$q”",
                        "Try a broader word, like units, backup or lock."
                    )
                }
            } else {
                // One lazy item per hit: a broad query can match dozens, and one item holding them
                // all composed every row at once.
                item("results", contentType = "results-header") {
                    SettingsGroupHeader("${results.size} ${if (results.size == 1) "result" else "results"}")
                }
                itemsIndexed(results, key = { _, r -> r.key }, contentType = { _, _ -> "result" }) { i, r ->
                    KitLazyRow(i, results.size) {
                        SearchHitRow(highlightMatch(r.name, q), r.where, r.name) { openResult(r) }
                    }
                }
            }
        }
    }
}

/** The air between the root's sections, as drawn: a little more than a sub-page's. */
private val RootSectionGap = 48.dp

/** Reset… and Factory reset: plain words in 14/600, each a 48dp target. */
@Composable
private fun RootTextAction(label: String, color: Color, onClick: () -> Unit) {
    Text(
        label,
        style = SettingsType.chip,
        color = color,
        modifier = Modifier
            .touchSlop(vertical = 15.dp, horizontal = 4.dp)
            .clickableLabeled(label, onClick = onClick)
            .padding(vertical = 15.dp, horizontal = 4.dp)
    )
}

/** A search hit: the name with its matched span in bold, where it lives, and a chevron. */
@Composable
private fun SearchHitRow(title: AnnotatedString, where: String, label: String, onClick: () -> Unit) {
    SettingsRowContainer(interaction = Modifier.clickableLabeled(label, onClick = onClick)) {
        Column(Modifier.weight(1f), verticalArrangement = Arrangement.spacedBy(3.dp)) {
            Text(title, style = OpenType.rowTitleL.copy(fontWeight = FontWeight.Normal), color = MaterialTheme.colorScheme.onBackground)
            SettingsReading(where)
        }
        SettingsChevron()
    }
}

/** The Cardio activities row's value: how many sports the user has added to the built-in list. */
internal fun cardioReading(count: Int): String = "$count of your own"

private val HOLIDAY_FMT: DateTimeFormatter = DateTimeFormatter.ofPattern("MMM d")

/**
 * The Holidays row's value on [today]: the holiday under way and when it ends, else the next one to
 * start, else none. Periods are inclusive "yyyy-MM-dd" ranges; a malformed one is skipped.
 */
internal fun holidayReading(periods: List<VacationPeriod>, today: LocalDate): String {
    val ranges = periods.mapNotNull { p ->
        runCatching { LocalDate.parse(p.startDate) to LocalDate.parse(p.endDate) }.getOrNull()
    }
    ranges.filter { (s, e) -> !today.isBefore(s) && !today.isAfter(e) }.maxByOrNull { it.second }?.let { (_, e) ->
        return "Away until ${e.format(HOLIDAY_FMT)}"
    }
    ranges.filter { (s, _) -> s.isAfter(today) }.minByOrNull { it.first }?.let { (s, _) ->
        return "Next from ${s.format(HOLIDAY_FMT)}"
    }
    return "None planned"
}

/** One resolved search hit: its display fields and where it goes ([page], or [action] when it
 *  fires one). [key] is stable per entry across queries. */
private class SearchResult(
    val key: String,
    val name: String,
    val where: String,
    val rank: Int,
    val page: SettingsPage? = null,
    val action: SearchAction? = null,
    val section: ProgramSection? = null
)

/** Every hit for [ql] (lowercased), name-prefix hits first, then name substrings, then tag-only. */
private fun searchHits(ql: String, freestyleMode: Boolean): List<SearchResult> = buildList {
    PAGE_ENTRIES.forEachIndexed { i, pe ->
        if (ql in pe.page.title.lowercase() || ql in pe.tags) {
            add(SearchResult("page-$i", pe.page.title, pageSection(pe.page), searchRank(pe.page.title, ql), page = pe.page))
        }
    }
    ALL_ITEMS.forEachIndexed { i, item ->
        if (ql in item.name.lowercase() || ql in item.tags) {
            add(SearchResult(
                "item-$i", item.name, item.section?.let { "${item.page.title} · ${it.title}" } ?: item.page.title,
                searchRank(item.name, ql), page = item.page, section = item.section
            ))
        }
    }
    ACTION_ENTRIES.forEachIndexed { i, e ->
        val hidden = e.action == SearchAction.COACH && freestyleMode
        if (!hidden && (ql in e.name.lowercase() || ql in e.tags)) {
            add(SearchResult("action-$i", e.name, e.where, searchRank(e.name, ql), action = e.action))
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

/** The section a page sits in on the root list, a page hit's breadcrumb. */
private fun pageSection(page: SettingsPage): String = when (page) {
    SettingsPage.Program, SettingsPage.Coach, SettingsPage.Session, SettingsPage.ExercisePrefs,
    SettingsPage.CardioActivities, SettingsPage.Vacation -> "Training"
    SettingsPage.Recovery, SettingsPage.Appearance, SettingsPage.Format, SettingsPage.Notifications,
    SettingsPage.Security -> "App"
    SettingsPage.Backup, SettingsPage.Storage -> "Data"
    SettingsPage.WhatsNew, SettingsPage.PrivacyPolicy, SettingsPage.About -> "About"
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

/**
 * The split a plan's days spell, from their names: the letter or number that tells two versions of
 * a day apart is dropped and repeats fold, so "Upper A, Lower A, Upper B, Lower B" reads
 * "Upper · Lower". Blank for no days.
 */
internal fun splitLabel(dayNames: List<String>): String =
    dayNames
        .map { it.trim().replace(Regex("""\s+([A-Za-z]|\d+)$"""), "") }
        .filter { it.isNotBlank() }
        .distinct()
        .joinToString(" · ")

/** "Upper · Lower, 4 days" — the plan in one line; "4 days a week" when the split has no names. */
internal fun planLine(split: String, days: Int): String =
    if (split.isBlank()) "$days days a week" else "$split, $days days"

/** The equipment in a word: the active setup's name, "full gym" for everything, else a count. */
internal fun gearReading(s: SettingsUiState): String {
    val preset = activePreset(s)
    return when {
        s.availableEquipment.isEmpty() || s.availableEquipment.size >= com.forge.app.program.Equipment.entries.size -> "full gym"
        preset != null -> preset.label.lowercase()
        s.availableEquipment.size == 1 -> "1 piece of gear"
        else -> "${s.availableEquipment.size} pieces of gear"
    }
}

/** How many of the Notifications page's switches are on: the phone alerts and the in-app kinds. */
internal fun notificationsOn(s: SettingsUiState): Int =
    listOf(s.trainingReminderEnabled, s.weeklyRecapEnabled, s.restTimerAlertEnabled).count { it } +
        com.forge.app.data.repo.NoticeKind.entries.count { it.key !in s.disabledNoticeKinds }

/** The locks in one line: "App lock on", "App and photo locks on", "Privacy mode on", or "Off". */
internal fun securityReading(s: SettingsUiState): String = when {
    s.appLockEnabled && s.galleryLockEnabled -> "App and photo locks on"
    s.appLockEnabled -> "App lock on"
    s.galleryLockEnabled -> "Photo lock on"
    s.privacyMode -> "Privacy mode on"
    else -> "Off"
}

/**
 * The Watch & health row's value: Health Connect and how many of its readings are on, or why it is
 * not working yet.
 */
internal fun watchReading(available: Boolean, needsUpdate: Boolean, on: Int): String = when {
    needsUpdate -> "Health Connect needs an update"
    !available -> "Health Connect not installed"
    on == 0 -> "Health Connect · not connected"
    on == 1 -> "Health Connect · 1 reading"
    else -> "Health Connect · $on readings"
}

internal fun rowSubtitle(page: SettingsPage, s: SettingsUiState, split: String = ""): String = when (page) {
    SettingsPage.Appearance -> {
        val accent = when {
            !s.accentEnabled -> "Monochrome"
            s.accentFromIcon && com.forge.app.appicon.AppIcon.fromKey(s.appIconKey).accentHex != null -> "Icon accent"
            else -> accentName(s.accentColorHex)
        }
        "$accent · ${if (s.amoledMode) "AMOLED" else "Pearl"}"
    }
    SettingsPage.Format -> "${s.weightUnit.label} · ${if (s.useMiles) "mi" else "km"} · ${if (s.useCm) "cm" else "in"} · " +
        "${if (s.timeFormat24h) "24h" else "12h"} · week starts ${if (s.firstDayMonday) "Monday" else "Sunday"}"
    SettingsPage.Session -> "Rest ${restLabel(s.restCompoundSeconds)} · " +
        if (s.hapticStrength == "off") "haptics off" else "haptics ${s.hapticStrength}"
    SettingsPage.Notifications -> {
        val on = notificationsOn(s)
        val head = if (on == 0) "All off" else "$on on"
        if (s.quietHoursEnabled) {
            // One window shared by every day reads as a time range; a per-day schedule reads "per day".
            val w = s.quietHoursSchedule.windows[0]
            val quiet =
                if (s.quietHoursSchedule.isUniform && !w.isOff) "quiet ${windowLabel(w, s.timeFormat24h)}"
                else "quiet per day"
            "$head · $quiet"
        } else head
    }
    SettingsPage.Program ->
        gearReading(s).let { gear -> if (s.freestyleMode) "Freestyle · $gear" else "${planLine(split, s.daysPerWeek)} · $gear" }
    SettingsPage.Coach -> when {
        !s.coachEnabled -> "Off"
        s.coachMode == "auto" -> "On · earning auto-apply"
        else -> "On · asks before changing"
    }
    SettingsPage.Security -> securityReading(s)
    SettingsPage.Recovery -> "Health Connect · watch app"
    SettingsPage.ExercisePrefs -> "${s.liked.size} liked · ${s.disliked.size} avoided"
    // The count lives on its own StateFlow, not SettingsUiState, so the row shows what it is for.
    SettingsPage.CardioActivities -> "Sports the built-in list misses"
    SettingsPage.Vacation -> "Pause your streak while you're away"
    SettingsPage.Backup -> "Weekly copy · back up now"
    SettingsPage.Storage -> "Space used · clear cache"
    SettingsPage.WhatsNew -> "Version ${com.forge.app.BuildConfig.VERSION_NAME}"
    SettingsPage.PrivacyPolicy -> "Offline use · permissions · deletion"
    SettingsPage.About -> "Privacy, gestures, licenses"
}

/**
 * The Backup row's value: whether the weekly copy is on, and when the last one landed. A failure
 * leads, since it is the line that needs a tap.
 */
internal fun backupSubtitle(enabled: Boolean, savedAt: String?, failed: Boolean): String = when {
    failed -> "Last backup failed · back up now"
    !enabled -> if (savedAt != null) "Weekly copy off · last $savedAt" else "Weekly copy off"
    savedAt != null -> "Weekly · last copy $savedAt"
    else -> "Weekly · no copy yet"
}

/** "2:30" for a rest time in seconds. */
internal fun restLabel(seconds: Int): String = "${seconds / 60}:${(seconds % 60).toString().padStart(2, '0')}"
