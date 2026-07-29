@file:OptIn(androidx.compose.material3.ExperimentalMaterial3Api::class)

package com.forge.app.ui.notifications

import android.content.Intent
import android.provider.Settings as AndroidSettings
import androidx.compose.foundation.background
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.PaddingValues
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.layout.sizeIn
import androidx.compose.foundation.lazy.LazyColumn
import androidx.compose.foundation.lazy.items
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.automirrored.filled.ArrowBack
import androidx.compose.material.icons.filled.Settings
import androidx.compose.material3.Icon
import androidx.compose.material3.IconButton
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Scaffold
import androidx.compose.material3.Text
import androidx.compose.material3.TopAppBar
import androidx.compose.material3.TopAppBarDefaults
import androidx.compose.runtime.Composable
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.clip
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.platform.LocalContext
import androidx.compose.ui.unit.dp
import androidx.hilt.navigation.compose.hiltViewModel
import androidx.lifecycle.compose.collectAsStateWithLifecycle
import com.forge.app.data.repo.AppNotice
import com.forge.app.data.repo.NoticeAction
import com.forge.app.ui.common.InlineEmptyHint
import com.forge.app.ui.common.bounceCombinedClick
import com.forge.app.ui.common.clickableLabeled
import com.forge.app.ui.common.forgeItemMotion

/**
 * The notifications page (DESIGN §3, list archetype) — every strip that used to sit at the top of
 * Home or Cardio, in one place, ranked live-first.
 *
 * It is the list recipe minus the search field: the feed tops out at a handful of rows, all of them
 * visible at once, so a search box would be a control that never earns a tap. Rows are NOT uniform
 * dot-text (§4.10) — each carries its own glyph, kind, reading and destination.
 *
 * Dismissal is deliberately not a per-row ×: that would be a tap nested inside the row's own tap
 * target (§2③). Acting on a row clears it, and the top bar's ONE action (§4.6) holds the two things
 * that apply to the whole page: Clear all, and the settings that decide which kinds appear at all.
 */
@Composable
fun NotificationsScreen(
    onBack: () -> Unit,
    onResumeSession: (dayKey: String) -> Unit,
    onOpenCoachBrief: () -> Unit,
    onConnectWearable: () -> Unit,
    onOpenNotificationSettings: () -> Unit,
    viewModel: NotificationsViewModel = hiltViewModel(),
) {
    val notices by viewModel.notices.collectAsStateWithLifecycle()
    val context = LocalContext.current
    var sheetOpen by remember { mutableStateOf(false) }

    val onBg = MaterialTheme.colorScheme.onBackground
    val muted = MaterialTheme.colorScheme.onSurfaceVariant
    val accent = MaterialTheme.colorScheme.primary
    val surfaceVariant = MaterialTheme.colorScheme.surfaceVariant

    fun act(notice: AppNotice) {
        when (val action = notice.action) {
            is NoticeAction.ResumeSession -> onResumeSession(action.dayKey)
            NoticeAction.OpenCoachBrief -> { viewModel.onCoachBriefOpened(); onOpenCoachBrief() }
            NoticeAction.ConnectWearable -> onConnectWearable()
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

    Scaffold(
        topBar = {
            TopAppBar(
                // §4.6: the bar carries chrome only. The page names itself in its own hero below,
                // and the bell isn't repeated here — you are already inside it.
                title = {},
                navigationIcon = {
                    IconButton(onClick = onBack) {
                        Icon(
                            Icons.AutoMirrored.Filled.ArrowBack,
                            contentDescription = "Back",
                            tint = muted
                        )
                    }
                },
                actions = {
                    Box(
                        // §14: the ≥44dp target comes from padding, the glyph stays small.
                        modifier = Modifier
                            .sizeIn(minWidth = 44.dp, minHeight = 44.dp)
                            .clickableLabeled("Notification options") { sheetOpen = true },
                        contentAlignment = Alignment.Center
                    ) {
                        Icon(
                            Icons.Default.Settings,
                            contentDescription = "Notification options",
                            tint = muted.copy(alpha = 0.7f),
                            modifier = Modifier.size(20.dp)
                        )
                    }
                },
                colors = TopAppBarDefaults.topAppBarColors(containerColor = Color.Transparent)
            )
        },
        containerColor = Color.Transparent
    ) { inner ->
        LazyColumn(
            modifier = Modifier.fillMaxSize().padding(inner),
            contentPadding = PaddingValues(horizontal = 24.dp)
        ) {
            item("hero") {
                // §3: a list gets a TINY hero — one line and a count, never a display-size figure.
                Spacer(Modifier.height(8.dp))
                Text("Notifications", style = MaterialTheme.typography.headlineSmall, color = onBg)
                Spacer(Modifier.height(2.dp))
                // §12: an honest zero, never a dash and never hidden.
                Text(
                    "${notices.size} WAITING",
                    style = MaterialTheme.typography.labelMedium,
                    color = muted
                )
                Spacer(Modifier.height(20.dp))
            }

            if (notices.isEmpty()) {
                item("empty") {
                    // §12: an inbox has no zero-SHAPE to draw — there is no mark for "nothing
                    // happened". This is the last-resort case InlineEmptyHint exists for.
                    InlineEmptyHint("Nothing waiting on you.", muted.copy(alpha = 0.65f))
                }
            } else {
                items(notices, key = { it.id }) { notice ->
                    // §9: lists take a light stagger, never the overview's entrance cascade.
                    NoticeRow(
                        notice = notice,
                        onBg = onBg, muted = muted, accent = accent, surfaceVariant = surfaceVariant,
                        onClick = { act(notice) },
                        modifier = forgeItemMotion()
                    )
                    Spacer(Modifier.height(10.dp))
                }
            }

            item("tail") { Spacer(Modifier.height(32.dp)) }
        }
    }
}

/**
 * One notice: a glyph chip, the line, and its one supporting reading.
 *
 * The row carries NO outline. §1 earns a border with interactivity, but a bordered row per notice
 * stacked into a wall of boxes on a page whose whole job is to be scanned; the chip IS the row's
 * mark (§12), and boxing a mark is not the same as boxing passive content. Actionable rows keep the
 * accent ` →` (§11) — with the border gone that arrow is the only thing separating a row that goes
 * somewhere from one that just happened, so it is load-bearing, not decoration.
 */
@Composable
private fun NoticeRow(
    notice: AppNotice,
    onBg: Color,
    muted: Color,
    accent: Color,
    surfaceVariant: Color,
    onClick: () -> Unit,
    modifier: Modifier = Modifier,
) {
    val actionable = notice.action != NoticeAction.None
    Row(
        modifier = modifier
            .fillMaxWidth()
            .then(
                if (actionable) Modifier.bounceCombinedClick(onClickLabel = notice.title, onClick = onClick)
                else Modifier
            )
            .padding(vertical = 10.dp),
        // Top-aligned so the chip stays beside the FIRST line when a long title wraps.
        verticalAlignment = Alignment.Top,
        horizontalArrangement = Arrangement.spacedBy(14.dp)
    ) {
        Box(
            // Holds a glyph, not text, so a fixed size is safe at any font scale (§14).
            Modifier
                .size(40.dp)
                .clip(RoundedCornerShape(12.dp))
                .background(surfaceVariant),
            contentAlignment = Alignment.Center
        ) {
            Icon(
                NoticeIcons.forGlyph(notice.glyph),
                // The glyph replaced the mono eyebrow, so it carries the kind for TalkBack (§14).
                contentDescription = notice.kind.label,
                tint = muted,
                modifier = Modifier.size(20.dp)
            )
        }
        Column(Modifier.weight(1f)) {
            // §14: no maxLines — a long coach summary wraps rather than clipping.
            Text(notice.title, style = MaterialTheme.typography.bodyMedium, color = onBg)
            notice.detail?.let { detail ->
                Spacer(Modifier.height(4.dp))
                Text(detail, style = MaterialTheme.typography.bodySmall, color = muted)
            }
        }
        if (actionable) Text("→", style = MaterialTheme.typography.bodyLarge, color = accent)
    }
}
