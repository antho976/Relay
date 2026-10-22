package com.forge.app.ui.gym.history

import androidx.compose.runtime.getValue
import androidx.compose.runtime.setValue
import androidx.compose.foundation.background
import androidx.compose.foundation.border
import androidx.compose.foundation.horizontalScroll
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.PaddingValues
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.heightIn
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.layout.statusBarsPadding
import androidx.compose.foundation.layout.width
import androidx.compose.foundation.lazy.LazyColumn
import androidx.compose.foundation.lazy.items
import androidx.compose.foundation.rememberScrollState
import androidx.compose.foundation.selection.selectable
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.foundation.text.BasicTextField
import androidx.compose.foundation.text.KeyboardActions
import androidx.compose.foundation.text.KeyboardOptions
import androidx.compose.material3.Icon
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.saveable.rememberSaveable
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.focus.FocusRequester
import androidx.compose.ui.focus.focusRequester
import androidx.compose.ui.graphics.SolidColor
import androidx.compose.ui.platform.LocalFocusManager
import androidx.compose.ui.semantics.Role
import androidx.compose.ui.semantics.contentDescription
import androidx.compose.ui.semantics.heading
import androidx.compose.ui.semantics.semantics
import androidx.compose.ui.text.input.ImeAction
import androidx.compose.ui.unit.dp
import androidx.hilt.navigation.compose.hiltViewModel
import androidx.lifecycle.compose.collectAsStateWithLifecycle
import com.forge.app.data.db.entities.CardioEntry
import com.forge.app.data.db.entities.Session
import com.forge.app.data.db.entities.durationMinutes
import com.forge.app.domain.cardio.CardioActivity
import com.forge.app.domain.cardio.pacePerUnit
import com.forge.app.domain.session.SessionType
import com.forge.app.domain.units.distanceUnitLabel
import com.forge.app.domain.units.formatDistance
import com.forge.app.program.Program
import com.forge.app.ui.common.OpenDeltaBars
import com.forge.app.ui.common.OpenLabel
import com.forge.app.ui.common.OpenPrTag
import com.forge.app.ui.common.OpenTabs
import com.forge.app.ui.common.OpenTopBarIcon
import com.forge.app.ui.common.bounceCombinedClick
import com.forge.app.ui.common.currentLocale
import com.forge.app.ui.common.touchSlop
import com.forge.app.ui.theme.HistoryType
import com.forge.app.ui.theme.LocalForgeSettings
import com.forge.app.ui.theme.OpenColors
import com.forge.app.ui.theme.OpenIcons
import com.forge.app.ui.theme.OpenType
import java.time.Instant
import java.time.ZoneId
import java.time.format.TextStyle

/**
 * History (2026-10-21 "Open panel" redesign, design `history_hs`): the "View all" behind Home's and
 * Stats' recent sessions. Search sits in the top bar, then the title and a line reading what the
 * filters leave, a type filter, and every session grouped by month: a date column, the day's name
 * with its PR tag, duration and heaviest set, and each lift against its own last time as bars.
 *
 * The older filters (Heavy, Short, Long, the user's own #tags) are kept after the four drawn ones in
 * the same scrolling row, so nothing the page could narrow by before is lost.
 */
@Composable
fun SessionHistoryScreen(
    onBack: () -> Unit,
    onOpenSession: (Long) -> Unit,
    onOpenCardio: (Long) -> Unit = {},
    viewModel: SessionHistoryViewModel = hiltViewModel()
) {
    val state by viewModel.state.collectAsStateWithLifecycle()
    SessionHistoryPage(
        state = state,
        query = viewModel.queryText,
        actions = HistoryActions(
            back = onBack,
            openSession = onOpenSession,
            openCardio = onOpenCardio,
            query = viewModel::setQuery,
            kind = viewModel::setKind,
            clearPills = viewModel::clearPillFilters,
            heavy = { viewModel.setVolumeFilter(if (it) SessionHistoryFilter.HIGH_VOLUME else null) },
            duration = viewModel::setDurationFilter,
            tag = viewModel::setTagFilter,
            requestDetail = viewModel::requestDetail
        ),
        detail = viewModel::detailFor
    )
}

/** Everything the History page can do, so the page itself is a pure function of its state. */
class HistoryActions(
    val back: () -> Unit = {},
    val openSession: (Long) -> Unit = {},
    val openCardio: (Long) -> Unit = {},
    val query: (String) -> Unit = {},
    val kind: (HistoryKind) -> Unit = {},
    val clearPills: () -> Unit = {},
    val heavy: (Boolean) -> Unit = {},
    val duration: (SessionHistoryFilter?) -> Unit = {},
    val tag: (String?) -> Unit = {},
    val requestDetail: (Long) -> Unit = {}
)

@Composable
fun SessionHistoryPage(
    state: SessionHistoryUiState,
    query: String,
    actions: HistoryActions,
    detail: (Long) -> HistoryRowDetail?,
    initialSearchOpen: Boolean = false
) {
    val ink = MaterialTheme.colorScheme.onBackground
    val muted = MaterialTheme.colorScheme.onSurfaceVariant
    // The field stays open while it holds a query, so a search is never hidden behind its icon.
    var searchOpen by rememberSaveable { mutableStateOf(initialSearchOpen || query.isNotEmpty()) }

    LazyColumn(
        Modifier.fillMaxSize().statusBarsPadding(),
        contentPadding = PaddingValues(start = 24.dp, end = 24.dp, top = 4.dp, bottom = 56.dp)
    ) {
        item("top", contentType = "top") {
            Row(Modifier.fillMaxWidth().height(48.dp), verticalAlignment = Alignment.CenterVertically) {
                OpenTopBarIcon(OpenIcons.ArrowLeft, "Back", actions.back)
                Spacer(Modifier.weight(1f))
                if (!searchOpen) OpenTopBarIcon(OpenIcons.Search, "Search history", { searchOpen = true })
            }
        }
        if (searchOpen) {
            item("search", contentType = "search") {
                HistorySearchField(
                    query = query,
                    onQuery = actions.query,
                    onClose = { actions.query(""); searchOpen = false },
                    focusOnOpen = !initialSearchOpen && query.isEmpty()
                )
            }
        }
        item("head", contentType = "head") {
            Column {
                Spacer(Modifier.height(20.dp))
                Text("History", style = OpenType.title, color = ink, modifier = Modifier.semantics { heading() })
                Spacer(Modifier.height(8.dp))
                OpenLabel(state.meta, style = OpenType.monoXS)
                Spacer(Modifier.height(28.dp))
                FilterRow(state, actions)
            }
        }

        if (!state.loaded) {
            // Nothing until the first read lands, so "no sessions" never flashes on open.
        } else if (state.months.isEmpty()) {
            item("empty", contentType = "empty") {
                Column {
                    Spacer(Modifier.height(40.dp))
                    OpenLabel(
                        if (state.anyFilterActive) "No sessions match. Clear a filter or the search."
                        else "Finish a workout or log cardio and it shows up here.",
                        style = OpenType.monoXS
                    )
                }
            }
        } else {
            state.months.forEach { month ->
                item("m:${month.key}", contentType = "month") {
                    Column {
                        Spacer(Modifier.height(40.dp))
                        Row(
                            Modifier.fillMaxWidth().semantics { heading() },
                            horizontalArrangement = Arrangement.SpaceBetween,
                            verticalAlignment = Alignment.CenterVertically
                        ) {
                            OpenLabel(month.label)
                            Spacer(Modifier.width(12.dp))
                            OpenLabel(month.meta)
                        }
                        Spacer(Modifier.height(8.dp))
                    }
                }
                items(month.items, key = { it.key }, contentType = { "row" }) { item ->
                    when (item) {
                        is HistoryItem.Workout -> {
                            LaunchedEffect(item.session.id) { actions.requestDetail(item.session.id) }
                            WorkoutHistoryRow(item.session, detail(item.session.id)) { actions.openSession(item.session.id) }
                        }
                        is HistoryItem.Cardio -> CardioRow(item.entry) { actions.openCardio(item.entry.id) }
                    }
                }
            }
        }
    }
}

/** The search the top bar opens: a plain line with a glyph, the field, and a close. */
@Composable
private fun HistorySearchField(query: String, onQuery: (String) -> Unit, onClose: () -> Unit, focusOnOpen: Boolean) {
    val ink = MaterialTheme.colorScheme.onBackground
    val muted = MaterialTheme.colorScheme.onSurfaceVariant
    val focus = remember { FocusRequester() }
    val focusManager = LocalFocusManager.current
    if (focusOnOpen) LaunchedEffect(Unit) { runCatching { focus.requestFocus() } }
    Column {
        Spacer(Modifier.height(8.dp))
        Row(
            Modifier.fillMaxWidth().heightIn(min = 44.dp),
            verticalAlignment = Alignment.CenterVertically,
            horizontalArrangement = Arrangement.spacedBy(12.dp)
        ) {
            Icon(OpenIcons.Search, contentDescription = null, tint = muted, modifier = Modifier.size(18.dp))
            Box(Modifier.weight(1f)) {
                if (query.isEmpty()) Text("Day, lift or note", style = OpenType.body, color = muted)
                BasicTextField(
                    value = query,
                    onValueChange = onQuery,
                    textStyle = OpenType.body.copy(color = ink),
                    cursorBrush = SolidColor(MaterialTheme.colorScheme.primary),
                    keyboardOptions = KeyboardOptions(imeAction = ImeAction.Search),
                    keyboardActions = KeyboardActions(onSearch = { focusManager.clearFocus() }),
                    modifier = Modifier.fillMaxWidth().focusRequester(focus)
                        .semantics { contentDescription = "Search history" }
                )
            }
            OpenTopBarIcon(OpenIcons.X, "Close search", onClose)
        }
        Box(Modifier.fillMaxWidth().height(1.dp).background(OpenColors.Rule))
    }
}

/**
 * The type filter as the page tabs every redesigned screen uses: All · Gym · Cardio · PRs. The older
 * narrowing pills (Heavy, Under 45, Over 60, one per tag) made a row of eight that ran off the screen
 * (Antho, 2026-10-21); tags are found by search instead.
 */
@Composable
private fun FilterRow(state: SessionHistoryUiState, actions: HistoryActions) {
    OpenTabs(HistoryKind.entries, state.kind, { it.label }, { k ->
        actions.clearPills()
        actions.kind(k)
    })
}

private val ChipShape = RoundedCornerShape(4.dp)

/** One filter: 36dp, 4dp corners; the chosen one washed red inside a red line. */
@Composable
internal fun HistoryChip(label: String, selected: Boolean, onClick: () -> Unit) {
    val accent = MaterialTheme.colorScheme.primary
    Box(
        Modifier
            .touchSlop(vertical = 6.dp)
            .selectable(selected = selected, role = Role.RadioButton, onClick = onClick)
            .padding(vertical = 6.dp)
            .heightIn(min = 36.dp)
            .then(
                if (selected) Modifier.background(accent.copy(alpha = 0.15f), ChipShape).border(1.dp, accent, ChipShape)
                else Modifier.border(1.dp, OpenColors.Rule, ChipShape)
            )
            .padding(horizontal = 14.dp),
        contentAlignment = Alignment.Center
    ) {
        Text(label, style = HistoryType.choice, color = MaterialTheme.colorScheme.onBackground)
    }
}

/** The date column every row leads with: weekday over the day of the month. */
@Composable
private fun DateColumn(epochMs: Long) {
    val date = Instant.ofEpochMilli(epochMs).atZone(ZoneId.systemDefault()).toLocalDate()
    Column(Modifier.width(34.dp)) {
        OpenLabel(date.dayOfWeek.getDisplayName(TextStyle.SHORT, currentLocale()), style = OpenType.monoXS)
        Text("${date.dayOfMonth}", style = OpenType.figureS, color = MaterialTheme.colorScheme.onBackground)
    }
}

/**
 * One gym session: date, the day's name with its PR tag, "52 min · Squat 275×5", and each lift
 * against its last time. The tags the user wrote keep a line of their own under that.
 */
@Composable
private fun WorkoutHistoryRow(session: Session, detail: HistoryRowDetail?, onClick: () -> Unit) {
    val ink = MaterialTheme.colorScheme.onBackground
    val muted = MaterialTheme.colorScheme.onSurfaceVariant
    val name = Program.dayDisplayName(session.dayKey)
    val marker = if (session.deloadMarkedHere) SessionType.DELOAD.pillLabel
    else SessionType.fromKey(session.sessionType)?.pillLabel
    val sub = listOfNotNull(
        session.durationMinutes()?.let { "$it min" } ?: "0 min",
        detail?.lead,
        marker?.lowercase()?.replaceFirstChar { it.uppercase() }
    ).joinToString(" · ")
    val tags = session.tags.split(",").map { it.trim() }.filter { it.isNotEmpty() }
    HistoryRowFrame(session.startedAt, "Open $name", onClick) {
        Column(Modifier.weight(1f), verticalArrangement = Arrangement.spacedBy(2.dp)) {
            Row(horizontalArrangement = Arrangement.spacedBy(8.dp), verticalAlignment = Alignment.CenterVertically) {
                Text(name, style = OpenType.rowTitle, color = ink, modifier = Modifier.weight(1f, fill = false))
                if (session.prCount > 0) OpenPrTag()
            }
            Text(sub, style = OpenType.bodyS, color = muted)
            if (tags.isNotEmpty()) OpenLabel(tags.joinToString(" · ") { "#$it" }, style = OpenType.monoXS)
        }
        val bars = detail?.deltas?.let { deltaBars(it) }.orEmpty()
        OpenDeltaBars(
            bars,
            Modifier.width(64.dp).height(28.dp),
            description = if (bars.none { it != null }) "No lift to compare with last time"
            else "${bars.count { (it ?: 0f) > 0f }} of ${bars.count { it != null }} lifts up on last time"
        )
    }
}

/** One cardio session: date, the activity, "40 min · 4.2 mi · 9:31 /mi", and its glyph. */
@Composable
private fun CardioRow(entry: CardioEntry, onClick: () -> Unit) {
    val ink = MaterialTheme.colorScheme.onBackground
    val muted = MaterialTheme.colorScheme.onSurfaceVariant
    val activity = CardioActivity.resolve(entry.type, com.forge.app.ui.cardio.LocalCardioTypes.current)
    val miles = LocalForgeSettings.current.useMiles
    val sub = listOfNotNull(
        "${entry.durationMin} min",
        entry.distanceKm?.takeIf { it > 0 }?.let { formatDistance(it, miles) },
        pacePerUnit(entry.durationMin, entry.distanceKm, miles)?.let { "$it /${distanceUnitLabel(miles)}" }
    ).joinToString(" · ")
    HistoryRowFrame(entry.date, "Open ${activity.displayName}", onClick) {
        Column(Modifier.weight(1f), verticalArrangement = Arrangement.spacedBy(2.dp)) {
            Text(activity.displayName, style = OpenType.rowTitle, color = ink)
            Text(sub, style = OpenType.bodyS, color = muted)
            entry.note?.trim()?.takeIf { it.isNotEmpty() }?.let { OpenLabel(it, style = OpenType.monoXS) }
        }
        Box(Modifier.width(64.dp).height(28.dp), contentAlignment = Alignment.CenterEnd) {
            Icon(OpenIcons.Activity, contentDescription = null, tint = muted, modifier = Modifier.size(18.dp))
        }
    }
}

/** The shared row: the whole row is the one tap target, 22dp of air below it. */
@Composable
private fun HistoryRowFrame(
    epochMs: Long,
    label: String,
    onClick: () -> Unit,
    content: @Composable androidx.compose.foundation.layout.RowScope.() -> Unit
) {
    Row(
        Modifier
            .fillMaxWidth()
            .bounceCombinedClick(onClickLabel = label, role = Role.Button, onClick = onClick)
            .heightIn(min = 48.dp)
            .padding(vertical = 11.dp),
        horizontalArrangement = Arrangement.spacedBy(14.dp),
        verticalAlignment = Alignment.CenterVertically
    ) {
        DateColumn(epochMs)
        content()
    }
}
