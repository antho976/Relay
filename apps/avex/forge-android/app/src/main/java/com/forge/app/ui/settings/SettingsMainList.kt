package com.forge.app.ui.settings

import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Column
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
import androidx.compose.ui.text.AnnotatedString
import androidx.compose.ui.text.SpanStyle
import androidx.compose.ui.text.buildAnnotatedString
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.text.withStyle
import androidx.compose.ui.unit.dp
import com.forge.app.data.db.entities.VacationPeriod
import com.forge.app.ui.common.clickableLabeled
import java.time.LocalDate
import java.time.format.DateTimeFormatter

/**
 * The Settings root: an index of every page, in the app's own list voice (History's), not a phone's
 * settings app. Three groups under mono anchors, each row a page's name over its live value in the
 * mono reading voice, so the page answers "how is this set up" before a tap. Factory reset stands
 * apart, and the two pages about Avex itself close the page as buttons rather than rows.
 *
 * While a query is typed the groups give way to one ranked list of hits, each of which goes
 * straight to its page or action.
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
    storageUsed: String? = null
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
        IndexRow(p.title, rowSubtitle(p, state)) { onOpenPage(p) }
    }
    SettingsLazyScaffold(
        title = "Settings",
        subtitle = "Avex ${com.forge.app.BuildConfig.VERSION_NAME}",
        onBack = onBack,
        listState = listState
    ) {
        // The title already leaves a group's space under it, so the gaps are placed per item: an
        // arrangement would double the one above the search, and the hits sit 2dp apart.
        val group = Modifier.padding(top = KitGroupSpacing)
        item("search") { SettingsSearchBar(searchQuery, "Search settings", onSearchChange) }
        if (searchQuery.isBlank()) {
            // What shapes the training first: the plan, the coach that adjusts it, how a session
            // runs, which movements it draws on, and the weeks it should leave alone.
            item("training") {
                SettingsGroup("Training", group) {
                    page(SettingsPage.Program)
                    // Coach configuration only; the brief and its record live on the Coach tab.
                    // Freestyle has no plan to coach, so the row goes with the tab.
                    if (!state.freestyleMode) page(SettingsPage.Coach)
                    page(SettingsPage.Session)
                    page(SettingsPage.ExercisePrefs)
                    IndexRow(SettingsPage.CardioActivities.title, cardioReading(customCardioCount)) {
                        onOpenPage(SettingsPage.CardioActivities)
                    }
                    IndexRow(SettingsPage.Vacation.title, holidayStatus) { onOpenPage(SettingsPage.Vacation) }
                }
            }
            item("app") {
                SettingsGroup("App & devices", group) {
                    // The watch leads: it is the one setting that changes what the rest of the app
                    // can show.
                    page(SettingsPage.Recovery)
                    page(SettingsPage.Appearance)
                    page(SettingsPage.Format)
                    page(SettingsPage.Notifications)
                    page(SettingsPage.Security)
                }
            }
            item("data") {
                SettingsGroup("Data", group) {
                    // The backup's state, not what the page is for: the date of the last copy, or
                    // that it failed, is the one thing worth knowing before a phone gets lost.
                    IndexRow(SettingsPage.Backup.title, backupStatus ?: rowSubtitle(SettingsPage.Backup, state)) {
                        onOpenPage(SettingsPage.Backup)
                    }
                    IndexRow("Export data", "Sessions · weekly · full backup · PDF", onClick = onOpenDataDialog)
                    IndexRow("Import data", "Strong · Hevy · FitNotes · CSV", onClick = onImportData)
                    IndexRow(SettingsPage.Storage.title, storageUsed?.let { "$it used" } ?: rowSubtitle(SettingsPage.Storage, state)) {
                        onOpenPage(SettingsPage.Storage)
                    }
                    // One entry for the targeted resets; factory reset stands apart below.
                    IndexRow("Reset…", "Sessions · trophies · cardio · settings", onClick = onOpenResetMenu)
                }
            }
            item("factory") {
                // No reading: what it erases is the confirm dialog's to say, in a sentence.
                SettingsGroup(modifier = group) {
                    IndexRow(ResetTarget.FACTORY.label, null, danger = true) {
                        onResetTarget(ResetTarget.FACTORY)
                    }
                }
            }
            item("about") {
                // About Avex rather than about your training, so not one more group of rows.
                SettingsButtonBar(group) {
                    SettingsButton(SettingsPage.WhatsNew.title, kind = ButtonKind.Tonal) { onOpenPage(SettingsPage.WhatsNew) }
                    SettingsButton(SettingsPage.About.title, kind = ButtonKind.Tonal) { onOpenPage(SettingsPage.About) }
                }
            }
        } else {
            // One flat, ranked list (see [searchHits]), each hit tapping straight through to where
            // the setting lives.
            if (results.isEmpty()) {
                item("empty") {
                    SettingsGroup(modifier = group) {
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
                        group
                    )
                }
                itemsIndexed(results, key = { _, r -> r.key }, contentType = { _, _ -> "result" }) { i, r ->
                    KitLazyRow(i, results.size) {
                        IndexRow(highlightMatch(r.name, q), r.where, label = r.name) { openResult(r) }
                    }
                }
            }
        }
    }
}

/**
 * A root row: the name, and under it the page's live value in the app's mono reading voice (the
 * voice History's dates and Coach's readings use). The whole slab is the tap; there is no chevron,
 * since a filled row on this page is already a promise of one.
 */
@Composable
private fun IndexRow(
    title: AnnotatedString,
    reading: String?,
    label: String = title.text,
    danger: Boolean = false,
    onClick: () -> Unit
) {
    SettingsRowContainer(interaction = Modifier.clickableLabeled(label, onClick = onClick)) {
        Column(Modifier.weight(1f), verticalArrangement = Arrangement.spacedBy(4.dp)) {
            Text(
                title,
                style = MaterialTheme.typography.bodyLarge,
                color = if (danger) MaterialTheme.colorScheme.error else MaterialTheme.colorScheme.onSurface
            )
            if (reading != null) {
                Text(reading, style = appTypography().labelMedium, color = MaterialTheme.colorScheme.onSurfaceVariant)
            }
        }
    }
}

@Composable
private fun IndexRow(title: String, reading: String?, danger: Boolean = false, onClick: () -> Unit) =
    IndexRow(AnnotatedString(title), reading, danger = danger, onClick = onClick)

/** The Cardio activities row's value: how many sports the user has added to the built-in list. */
internal fun cardioReading(count: Int): String = when (count) {
    0 -> "None added"
    1 -> "1 added"
    else -> "$count added"
}

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

/** The group a page sits in on the root list, a page hit's breadcrumb. */
private fun pageSection(page: SettingsPage): String = when (page) {
    SettingsPage.Program, SettingsPage.Coach, SettingsPage.Session, SettingsPage.ExercisePrefs,
    SettingsPage.CardioActivities, SettingsPage.Vacation -> "Training"
    SettingsPage.Recovery, SettingsPage.Appearance, SettingsPage.Format, SettingsPage.Notifications,
    SettingsPage.Security -> "App & devices"
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
    SettingsPage.Session -> "${if (s.hapticStrength == "off") "Haptics off" else "${s.hapticStrength.replaceFirstChar { it.uppercase() }} haptics"} · ${restLabel(s.restCompoundSeconds)} rest"
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
    SettingsPage.Program -> {
        // "1 pieces of gear" and a day count for a plan a freestyle user doesn't have, before.
        val gear = when {
            s.availableEquipment.isEmpty() || s.availableEquipment.size >= com.forge.app.program.Equipment.entries.size -> "all equipment"
            s.availableEquipment.size == 1 -> "1 piece of gear"
            else -> "${s.availableEquipment.size} pieces of gear"
        }
        if (s.freestyleMode) "Freestyle · $gear" else "${s.daysPerWeek} days a week · $gear"
    }
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
    SettingsPage.Recovery -> "Health Connect · watch app"
    SettingsPage.ExercisePrefs -> when {
        s.liked.isEmpty() && s.disliked.isEmpty() -> "None set"
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
