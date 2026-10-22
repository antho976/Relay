@file:OptIn(androidx.compose.material3.ExperimentalMaterial3Api::class)

package com.forge.app.ui.notifications

import android.content.Intent
import android.provider.Settings as AndroidSettings
import androidx.compose.foundation.background
import androidx.compose.foundation.border
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
import androidx.compose.foundation.lazy.LazyColumn
import androidx.compose.foundation.lazy.items
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.material3.Icon
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Scaffold
import androidx.compose.material3.SwipeToDismissBox
import androidx.compose.material3.SwipeToDismissBoxValue
import androidx.compose.material3.Text
import androidx.compose.material3.rememberSwipeToDismissBoxState
import androidx.compose.runtime.Composable
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.graphics.vector.ImageVector
import androidx.compose.ui.platform.LocalContext
import androidx.compose.ui.semantics.CustomAccessibilityAction
import androidx.compose.ui.semantics.Role
import androidx.compose.ui.semantics.customActions
import androidx.compose.ui.semantics.heading
import androidx.compose.ui.semantics.semantics
import androidx.compose.ui.unit.dp
import androidx.hilt.navigation.compose.hiltViewModel
import androidx.lifecycle.compose.collectAsStateWithLifecycle
import com.forge.app.data.repo.AppNotice
import com.forge.app.data.repo.NoticeAction
import com.forge.app.data.repo.NoticeGlyph
import com.forge.app.data.repo.NoticeKind
import com.forge.app.ui.checkin.CheckinSheet
import com.forge.app.ui.checkin.CheckinViewModel
import com.forge.app.ui.common.OpenLabel
import com.forge.app.ui.common.OpenTopBarIcon
import com.forge.app.ui.common.bounceClick
import com.forge.app.ui.common.bounceCombinedClick
import com.forge.app.ui.common.touchSlop
import com.forge.app.ui.goals.OpenBackBar
import com.forge.app.ui.theme.NoticeType
import com.forge.app.ui.theme.OpenColors
import com.forge.app.ui.theme.OpenIcons
import com.forge.app.ui.theme.OpenType

/**
 * The notifications page (2026-10-21 redesign, "pg-notif nf"): the bell. Rows are grouped by what
 * they ask of you, decisions first (Needs you), then news (New), then setup (Set up). A decision made
 * right here carries its verb as a small outlined button; a row that goes somewhere carries a
 * chevron. Swiping a clearable row clears it, with an Undo; the gear holds Clear all and the
 * settings for which kinds appear at all.
 *
 * [viewModel] is the navigation host's root-scoped instance, the same one feeding every bell. It is
 * a required parameter rather than a `hiltViewModel()` default so the page can never quietly own a
 * destination-scoped copy whose writes die when the page is popped (M-27).
 */
@Composable
fun NotificationsScreen(
    onBack: () -> Unit,
    onResumeSession: (dayKey: String) -> Unit,
    onOpenCoachBrief: () -> Unit,
    onConnectWearable: () -> Unit,
    onOpenNotificationSettings: () -> Unit,
    onOpenLesson: (lessonId: String) -> Unit,
    viewModel: NotificationsViewModel,
    onOpenAcademy: () -> Unit = {},
    checkinViewModel: CheckinViewModel = hiltViewModel(),
) {
    val notices by viewModel.notices.collectAsStateWithLifecycle()
    val context = LocalContext.current
    var sheetOpen by remember { mutableStateOf(false) }

    fun act(notice: AppNotice) {
        when (val action = notice.action) {
            is NoticeAction.ResumeSession -> onResumeSession(action.dayKey)
            NoticeAction.OpenCheckin -> checkinViewModel.open()
            NoticeAction.OpenCoachBrief -> { viewModel.onCoachBriefOpened(); onOpenCoachBrief() }
            NoticeAction.ConnectWearable -> onConnectWearable()
            // Opening the lesson is what clears its row: the feed shows unlocked-and-unread
            // lessons, and the Academy records the open. No separate dismissal needed.
            is NoticeAction.OpenLesson -> onOpenLesson(action.lessonId)
            NoticeAction.OpenAcademy -> onOpenAcademy()
            // The OS app-notification screen rather than a permission request: it works however many
            // times the permission was already denied, which a re-request does not.
            NoticeAction.EnableNotifications -> context.startActivity(
                Intent(AndroidSettings.ACTION_APP_NOTIFICATION_SETTINGS)
                    .putExtra(AndroidSettings.EXTRA_APP_PACKAGE, context.packageName)
            )
            NoticeAction.None -> Unit
        }
    }

    if (sheetOpen) {
        NotificationsOptionsSheet(
            canClear = notices.any { it.dismissible },
            onClearAll = { sheetOpen = false; viewModel.clearAll() },
            onOpenNotificationSettings = { sheetOpen = false; onOpenNotificationSettings() },
            onDismiss = { sheetOpen = false }
        )
    }

    CheckinSheet(checkinViewModel)

    Scaffold(
        topBar = {
            OpenBackBar(onBack) { OpenTopBarIcon(OpenIcons.Settings, "Notification options", { sheetOpen = true }) }
        },
        containerColor = Color.Transparent
    ) { inner ->
        NotificationsPage(
            notices = notices,
            onAct = ::act,
            onDismiss = { viewModel.dismiss(it.id) },
            modifier = Modifier.padding(inner)
        )
    }
}

/** The page body, free of the ViewModel so the goldens render exactly what the app does. */
@Composable
internal fun NotificationsPage(
    notices: List<AppNotice>,
    onAct: (AppNotice) -> Unit,
    onDismiss: (AppNotice) -> Unit,
    modifier: Modifier = Modifier,
) {
    val groups = remember(notices) { groupNotices(notices) }
    LazyColumn(
        modifier.fillMaxSize(),
        contentPadding = PaddingValues(start = 24.dp, end = 24.dp, top = 4.dp, bottom = 64.dp)
    ) {
        item("head") {
            Column {
                Spacer(Modifier.height(20.dp))
                Text(
                    "Notifications",
                    style = NoticeType.pageTitle,
                    color = MaterialTheme.colorScheme.onBackground,
                    modifier = Modifier.semantics { heading() }
                )
                Spacer(Modifier.height(8.dp))
                OpenLabel(noticesMetaLine(notices), style = OpenType.monoXS)
                Spacer(Modifier.height(36.dp))
                // An inbox has no shape to draw at zero; one plain line says what fills it.
                if (notices.isEmpty()) OpenLabel("Nothing waiting on you · new calls and lessons land here", style = OpenType.monoXS)
            }
        }
        groups.forEachIndexed { gi, (group, rows) ->
            item("g-${group.name}") {
                Column {
                    if (gi > 0) Spacer(Modifier.height(18.dp))
                    Row(Modifier.fillMaxWidth(), horizontalArrangement = Arrangement.SpaceBetween) {
                        OpenLabel(group.label, Modifier.semantics { heading() })
                        OpenLabel("${rows.size}")
                    }
                    Spacer(Modifier.height(16.dp))
                }
            }
            items(rows, key = { it.id }) { notice ->
                Box(Modifier.padding(bottom = 22.dp)) {
                    if (notice.dismissible) {
                        SwipeRow(onDismiss = { onDismiss(notice) }) {
                            NoticeRow(notice, group, onAct = { onAct(notice) }, onClear = { onDismiss(notice) })
                        }
                    } else {
                        NoticeRow(notice, group, onAct = { onAct(notice) }, onClear = null)
                    }
                }
            }
        }
    }
}

/**
 * Swipe either way to clear. The dismiss is not confirmed here: the row leaves when the feed drops
 * it, so a failed clear snaps back rather than vanishing while the notice still stands.
 */
@Composable
private fun SwipeRow(onDismiss: () -> Unit, content: @Composable () -> Unit) {
    val state = rememberSwipeToDismissBoxState(
        confirmValueChange = { v ->
            if (v != SwipeToDismissBoxValue.Settled) onDismiss()
            false
        }
    )
    SwipeToDismissBox(
        state = state,
        backgroundContent = {
            val start = state.dismissDirection == SwipeToDismissBoxValue.StartToEnd
            Box(
                Modifier.fillMaxSize(),
                contentAlignment = if (start) Alignment.CenterStart else Alignment.CenterEnd
            ) {
                if (state.dismissDirection != SwipeToDismissBoxValue.Settled) OpenLabel("Clear", style = OpenType.monoXS)
            }
        }
    ) {
        Box(Modifier.fillMaxWidth().background(MaterialTheme.colorScheme.background)) { content() }
    }
}

/** The row's glyph, from the redesign's icon family, by what the row is. */
private fun glyphFor(notice: AppNotice): ImageVector = when (notice.kind) {
    NoticeKind.CHECK_IN -> OpenIcons.Activity
    NoticeKind.COACH_BRIEF -> OpenIcons.Compass
    else -> when (notice.glyph) {
        NoticeGlyph.SESSION -> OpenIcons.Dumbbell
        NoticeGlyph.COACH -> OpenIcons.Compass
        NoticeGlyph.MILESTONE -> OpenIcons.Sparkles
        NoticeGlyph.IMPORT -> OpenIcons.Copy
        NoticeGlyph.BACKUP -> OpenIcons.History
        NoticeGlyph.HOUSEKEEPING -> OpenIcons.Check
        NoticeGlyph.BELL -> OpenIcons.Bell
        NoticeGlyph.WATCH -> OpenIcons.Smartphone
        NoticeGlyph.ACADEMY -> OpenIcons.Book
    }
}

/**
 * One notice: its glyph (red when it needs you), the line and its one supporting reading, then its
 * verb as a small outlined button or a chevron when it goes somewhere.
 */
@Composable
private fun NoticeRow(notice: AppNotice, group: NoticeGroup, onAct: () -> Unit, onClear: (() -> Unit)?) {
    val ink = MaterialTheme.colorScheme.onBackground
    val muted = MaterialTheme.colorScheme.onSurfaceVariant
    val verb = actionVerb(notice.action)
    val goes = verb == null && notice.action != NoticeAction.None
    Row(
        Modifier
            .fillMaxWidth()
            .then(if (goes) Modifier.bounceCombinedClick(onClickLabel = notice.title, role = Role.Button, onClick = onAct) else Modifier)
            .semantics(mergeDescendants = verb == null) {
                if (onClear != null) customActions = listOf(CustomAccessibilityAction("Clear") { onClear(); true })
            },
        horizontalArrangement = Arrangement.spacedBy(14.dp),
        verticalAlignment = Alignment.Top
    ) {
        Icon(
            glyphFor(notice),
            contentDescription = notice.kind.label,
            tint = if (group == NoticeGroup.NEEDS_YOU) MaterialTheme.colorScheme.primary else muted,
            modifier = Modifier.padding(top = 1.dp).size(20.dp)
        )
        Column(Modifier.weight(1f), verticalArrangement = Arrangement.spacedBy(3.dp)) {
            Text(notice.title, style = OpenType.rowTitleL, color = ink)
            notice.detail?.let { Text(it, style = OpenType.bodyS, color = muted) }
        }
        when {
            verb != null -> RowButton(verb, onAct)
            goes -> Icon(OpenIcons.ChevronRight, contentDescription = null, tint = OpenColors.Glyph, modifier = Modifier.padding(top = 3.dp).size(16.dp))
        }
    }
}

/** A row's own verb: a 32dp outline, 48dp to the finger. */
@Composable
private fun RowButton(text: String, onClick: () -> Unit) {
    val shape = RoundedCornerShape(4.dp)
    Box(
        Modifier
            .touchSlop(vertical = 8.dp)
            .bounceClick(role = Role.Button, onClick = onClick)
            .padding(vertical = 8.dp)
            .heightIn(min = 32.dp)
            .border(1.dp, OpenColors.Rule, shape)
            .padding(horizontal = 12.dp, vertical = 6.dp),
        contentAlignment = Alignment.Center
    ) {
        Text(text, style = NoticeType.action, color = MaterialTheme.colorScheme.onBackground)
    }
}
