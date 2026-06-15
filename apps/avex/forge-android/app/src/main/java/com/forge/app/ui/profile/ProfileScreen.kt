package com.forge.app.ui.profile

import androidx.activity.compose.rememberLauncherForActivityResult
import androidx.activity.result.PickVisualMediaRequest
import androidx.activity.result.contract.ActivityResultContracts
import androidx.compose.foundation.background
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.aspectRatio
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.rememberScrollState
import androidx.compose.foundation.shape.CircleShape
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.foundation.verticalScroll
import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.automirrored.filled.ArrowBack
import androidx.compose.material.icons.filled.Add
import androidx.compose.material3.ExperimentalMaterial3Api
import androidx.compose.material3.HorizontalDivider
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
import androidx.compose.ui.text.font.FontStyle
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp
import androidx.compose.ui.window.Dialog
import androidx.hilt.navigation.compose.hiltViewModel
import androidx.lifecycle.compose.collectAsStateWithLifecycle
import com.forge.app.data.repo.ProgressPhoto
import com.forge.app.ui.common.bounceClick
import com.forge.app.ui.theme.emphasized
import java.io.File
import java.text.SimpleDateFormat
import java.util.Date
import java.util.Locale

/**
 * The "You" hub: rank ladder + earned XP, an offline standing estimate, signature lifts, the
 * private mirror-test photos, the trophy case and the month/year recaps. All local — no account,
 * no server. See [ProfileViewModel] / `data/repo/ProfileRepository` / `domain/rank`.
 */
@OptIn(ExperimentalMaterial3Api::class)
@Composable
fun ProfileScreen(
    onBack: () -> Unit,
    onOpenTrophies: () -> Unit,
    onOpenRecaps: () -> Unit,
    viewModel: ProfileViewModel = hiltViewModel()
) {
    val state by viewModel.state.collectAsStateWithLifecycle()
    var viewing by remember { mutableStateOf<ProgressPhoto?>(null) }
    var showXpInfo by remember { mutableStateOf(false) }

    val photoPicker = rememberLauncherForActivityResult(ActivityResultContracts.PickVisualMedia()) { uri ->
        uri?.let { viewModel.addPhoto(it) }
    }
    val avatarPicker = rememberLauncherForActivityResult(ActivityResultContracts.PickVisualMedia()) { uri ->
        uri?.let { viewModel.setAvatar(it) }
    }
    fun pickPhoto() = photoPicker.launch(PickVisualMediaRequest(ActivityResultContracts.PickVisualMedia.ImageOnly))
    fun pickAvatar() = avatarPicker.launch(PickVisualMediaRequest(ActivityResultContracts.PickVisualMedia.ImageOnly))

    val onBg = MaterialTheme.colorScheme.onBackground
    val muted = MaterialTheme.colorScheme.onSurfaceVariant
    val accent = MaterialTheme.colorScheme.primary
    val outline = MaterialTheme.colorScheme.outline

    Scaffold(
        topBar = {
            TopAppBar(
                title = { Text("You.", style = MaterialTheme.typography.headlineMedium) },
                navigationIcon = {
                    IconButton(onClick = onBack) { Icon(Icons.AutoMirrored.Filled.ArrowBack, "Back") }
                },
                colors = TopAppBarDefaults.topAppBarColors(containerColor = Color.Transparent)
            )
        },
        containerColor = Color.Transparent
    ) { inner ->
        Column(
            Modifier.fillMaxSize().padding(inner).verticalScroll(rememberScrollState()).padding(horizontal = 24.dp)
        ) {
            // ── Header ──────────────────────────────────────────────────────────
            Text(
                "ATHLETE PROFILE" + if (state.sinceLabel.isNotBlank()) " · SINCE ${state.sinceLabel}" else "",
                style = MaterialTheme.typography.labelSmall, color = emphasized(muted), fontSize = 9.sp
            )
            Spacer(Modifier.height(6.dp))
            Row(Modifier.fillMaxWidth(), horizontalArrangement = Arrangement.SpaceBetween, verticalAlignment = Alignment.CenterVertically) {
                Column(Modifier.weight(1f)) {
                    Text(state.name.ifBlank { "Athlete" } + ".", style = MaterialTheme.typography.displaySmall, color = emphasized(onBg))
                    state.rank?.let { r ->
                        Spacer(Modifier.height(2.dp))
                        val streak = if (state.streakDays >= 2) " · ${state.streakDays}-day streak, still alive." else ""
                        Text(
                            "Rank ${r.roman} — ${r.tier.display}$streak",
                            style = MaterialTheme.typography.bodySmall, color = muted, fontStyle = FontStyle.Italic
                        )
                    }
                }
                AvatarCircle(state.avatarFile, accent, outline) { pickAvatar() }
            }

            // ── Rank track ──────────────────────────────────────────────────────
            state.rank?.let { r ->
                Spacer(Modifier.height(20.dp))
                RankSection(r, muted, accent, outline, onInfo = { showXpInfo = true })
            }

            // ── Sections ────────────────────────────────────────────────────────
            LedgerSection(state.totalSessions, state.totalVolumeLb, state.totalPrs, state.rank?.xpTotal ?: 0L, muted, accent, outline)
            StandingSection(state.standings, onBg, muted, accent, outline)
            SignatureSection(state.topLift, state.mostLoggedDay, state.usualHour, onBg, muted, accent, outline)
            MirrorTestSection(state.photos, viewModel::fileFor, onAdd = { pickPhoto() }, onView = { viewing = it }, onBg, muted, accent, outline)

            state.memory?.let { m ->
                ProfileBlock("ON THIS DAY", muted, accent, outline) {
                    val ago = if (m.monthsAgo % 12 == 0) "${m.monthsAgo / 12} year${if (m.monthsAgo == 12) "" else "s"} ago"
                    else "${m.monthsAgo} months ago"
                    Text(
                        "$ago you trained ${m.dayName} — ${formatVolume(m.totalVolumeLb)} lb" +
                            if (m.prCount > 0) " · ${m.prCount} PR${if (m.prCount == 1) "" else "s"}" else "",
                        style = MaterialTheme.typography.bodyMedium, color = onBg
                    )
                }
            }

            TrophyCaseSection(state.trophyGrid, state.trophyUnlocked, state.trophyTotal, state.closestTrophy, onOpenTrophies, onBg, muted, accent, outline)
            OnTheRecordSection(state.recaps, onOpenRecaps, onBg, muted, accent, outline)

            Spacer(Modifier.height(40.dp))
        }
    }

    viewing?.let { photo ->
        PhotoViewerDialog(
            file = viewModel.fileFor(photo),
            takenAtMs = photo.takenAtMs,
            onDelete = { viewModel.deletePhoto(photo); viewing = null },
            onDismiss = { viewing = null }
        )
    }

    if (showXpInfo) {
        val r = state.rank
        val xp = state.xp
        if (r != null && xp != null) RankInfoSheet(r, xp, onDismiss = { showXpInfo = false })
    }
}

@Composable
private fun AvatarCircle(file: File?, accent: Color, outline: Color, onPick: () -> Unit) {
    // The outer Box is NOT clipped to a circle: the "+" badge sits at the bottom-right corner,
    // which is outside the avatar's inscribed circle, so a circle clip would shear it off the
    // bubble. The photo and "PIC" placeholder each clip themselves to a circle instead.
    Box(
        Modifier.size(64.dp).bounceClick { onPick() },
        contentAlignment = Alignment.BottomEnd
    ) {
        if (file != null) {
            ProgressPhotoImage(file, Modifier.fillMaxSize().clip(CircleShape), reqPx = 200)
        } else {
            Box(
                Modifier.fillMaxSize().clip(CircleShape)
                    .background(MaterialTheme.colorScheme.surfaceVariant.copy(alpha = 0.35f)),
                contentAlignment = Alignment.Center
            ) {
                Text("PIC", style = MaterialTheme.typography.labelSmall, color = outline, fontSize = 9.sp)
            }
        }
        Box(Modifier.size(20.dp).clip(CircleShape).background(accent), contentAlignment = Alignment.Center) {
            Icon(Icons.Filled.Add, contentDescription = "Set photo", tint = MaterialTheme.colorScheme.background, modifier = Modifier.size(13.dp))
        }
    }
}

@Composable
private fun PhotoViewerDialog(file: File, takenAtMs: Long, onDelete: () -> Unit, onDismiss: () -> Unit) {
    Dialog(onDismissRequest = onDismiss) {
        Column(Modifier.fillMaxWidth().clip(RoundedCornerShape(12.dp))) {
            ProgressPhotoImage(file, Modifier.fillMaxWidth().aspectRatio(0.8f).clip(RoundedCornerShape(12.dp)), reqPx = 1200)
            Spacer(Modifier.height(10.dp))
            Row(Modifier.fillMaxWidth(), horizontalArrangement = Arrangement.SpaceBetween, verticalAlignment = Alignment.CenterVertically) {
                Text(
                    SimpleDateFormat("MMM d, yyyy", Locale.getDefault()).format(Date(takenAtMs)),
                    style = MaterialTheme.typography.labelSmall, color = MaterialTheme.colorScheme.onSurfaceVariant
                )
                Text(
                    "delete", style = MaterialTheme.typography.labelSmall, color = MaterialTheme.colorScheme.error,
                    modifier = Modifier.bounceClick { onDelete() }.padding(8.dp)
                )
            }
            Spacer(Modifier.height(2.dp))
            HorizontalDivider(color = Color.Transparent)
        }
    }
}
