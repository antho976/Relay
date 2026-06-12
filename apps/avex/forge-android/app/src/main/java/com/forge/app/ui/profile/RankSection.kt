package com.forge.app.ui.profile

import androidx.compose.foundation.Canvas
import androidx.compose.foundation.background
import androidx.compose.foundation.border
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.shape.CircleShape
import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.filled.AutoAwesome
import androidx.compose.material.icons.filled.Bolt
import androidx.compose.material.icons.filled.FitnessCenter
import androidx.compose.material.icons.filled.Layers
import androidx.compose.material.icons.filled.LocalFireDepartment
import androidx.compose.material.icons.filled.WorkspacePremium
import androidx.compose.material3.ExperimentalMaterial3Api
import androidx.compose.material3.HorizontalDivider
import androidx.compose.material3.Icon
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.ModalBottomSheet
import androidx.compose.material3.Text
import androidx.compose.material3.rememberModalBottomSheetState
import androidx.compose.runtime.Composable
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.clip
import androidx.compose.ui.geometry.Offset
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.graphics.StrokeCap
import androidx.compose.ui.graphics.vector.ImageVector
import androidx.compose.ui.text.font.FontStyle
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp
import com.forge.app.domain.rank.RankInfo
import com.forge.app.domain.rank.RankLadder
import com.forge.app.domain.rank.RankTier
import com.forge.app.domain.rank.SUB_RANKS_PER_TIER
import com.forge.app.domain.rank.XpBreakdown
import com.forge.app.ui.common.bounceClick
import com.forge.app.ui.theme.emphasized

private fun tierIcon(t: RankTier): ImageVector = when (t) {
    RankTier.EMBER -> Icons.Filled.LocalFireDepartment
    RankTier.IRON -> Icons.Filled.FitnessCenter
    RankTier.STEEL -> Icons.Filled.Layers
    RankTier.TEMPERED -> Icons.Filled.Bolt
    RankTier.FORGED -> Icons.Filled.AutoAwesome
    RankTier.DAMASCUS -> Icons.Filled.WorkspacePremium
}

/** The rank track: six tier nodes (Ember → Damascus) with a progress line and the XP-to-next line. */
@Composable
internal fun RankSection(
    rank: RankInfo,
    onBg: Color,
    muted: Color,
    accent: Color,
    outline: Color,
    onInfo: () -> Unit
) {
    val tiers = RankTier.entries
    // Smooth fill: how far along the 5 gaps between the 6 nodes we are.
    val withinTier = ((rank.subRank - 1) + rank.progressInRank) / SUB_RANKS_PER_TIER
    val pos = ((rank.tier.ordinal + withinTier) / (tiers.size - 1)).coerceIn(0f, 1f)

    Column(Modifier.fillMaxWidth().bounceClick { onInfo() }) {
        Box(Modifier.fillMaxWidth()) {
            // Connecting line + fill, behind the nodes, aligned to node centres.
            Canvas(Modifier.matchParentSize()) {
                val cell = size.width / tiers.size
                val y = 15.dp.toPx()
                val startX = cell / 2
                val endX = size.width - cell / 2
                drawLine(outline.copy(alpha = 0.25f), Offset(startX, y), Offset(endX, y), strokeWidth = 2.dp.toPx(), cap = StrokeCap.Round)
                if (pos > 0f) {
                    drawLine(accent, Offset(startX, y), Offset(startX + (endX - startX) * pos, y), strokeWidth = 2.dp.toPx(), cap = StrokeCap.Round)
                }
            }
            Row(Modifier.fillMaxWidth()) {
                tiers.forEach { tier ->
                    Column(
                        Modifier.weight(1f),
                        horizontalAlignment = Alignment.CenterHorizontally
                    ) {
                        RankNode(tier, rank.tier, onBg, accent, outline)
                        Spacer(Modifier.height(6.dp))
                        Text(
                            tier.display.uppercase(),
                            style = MaterialTheme.typography.labelSmall,
                            color = if (tier == rank.tier) emphasized(onBg) else muted.copy(alpha = 0.55f),
                            fontSize = 7.sp,
                            fontWeight = if (tier == rank.tier) FontWeight.Bold else FontWeight.Normal
                        )
                    }
                }
            }
        }
        Spacer(Modifier.height(12.dp))
        Row(Modifier.fillMaxWidth(), horizontalArrangement = Arrangement.SpaceBetween, verticalAlignment = Alignment.CenterVertically) {
            Text("HOW XP WORKS →", style = MaterialTheme.typography.labelSmall, color = accent, fontSize = 9.sp)
            val tail = if (rank.isMax) "MAX RANK" else "${rank.xpToNextTier} TO ${rank.nextTierName?.uppercase()}"
            Text("${rank.xpTotal} XP · $tail", style = MaterialTheme.typography.labelSmall, color = muted, fontSize = 9.sp)
        }
    }
}

@Composable
private fun RankNode(tier: RankTier, current: RankTier, onBg: Color, accent: Color, outline: Color) {
    val isCurrent = tier == current
    val reached = tier.ordinal <= current.ordinal
    val node = if (isCurrent) 30.dp else 26.dp
    val bg = when {
        isCurrent -> accent
        reached -> accent.copy(alpha = 0.85f)
        else -> Color.Transparent
    }
    Box(
        Modifier
            .size(node)
            .clip(CircleShape)
            .background(bg)
            .then(if (!reached) Modifier.border(1.dp, outline.copy(alpha = 0.45f), CircleShape) else Modifier),
        contentAlignment = Alignment.Center
    ) {
        Icon(
            tierIcon(tier),
            contentDescription = tier.display,
            tint = if (reached) MaterialTheme.colorScheme.background else outline.copy(alpha = 0.6f),
            modifier = Modifier.size(node * 0.52f)
        )
    }
}

/** "How XP works" sheet — the earned-XP breakdown + the rank ladder, the answer to "how do I level up". */
@OptIn(ExperimentalMaterial3Api::class)
@Composable
internal fun RankInfoSheet(
    rank: RankInfo,
    xp: XpBreakdown,
    onDismiss: () -> Unit
) {
    val onBg = MaterialTheme.colorScheme.onBackground
    val muted = MaterialTheme.colorScheme.onSurfaceVariant
    val accent = MaterialTheme.colorScheme.primary
    val outline = MaterialTheme.colorScheme.outline

    ModalBottomSheet(onDismissRequest = onDismiss, sheetState = rememberModalBottomSheetState(skipPartiallyExpanded = true)) {
        Column(Modifier.fillMaxWidth().padding(horizontal = 24.dp).padding(bottom = 32.dp)) {
            Text("How XP works", style = MaterialTheme.typography.headlineSmall, color = emphasized(onBg), fontStyle = FontStyle.Italic)
            Spacer(Modifier.height(4.dp))
            Text(
                "${rank.displayName} · ${rank.xpTotal} XP" +
                    if (!rank.isMax) " · ${rank.xpToNextTier} to ${rank.nextTierName}" else " · max rank",
                style = MaterialTheme.typography.bodySmall, color = muted
            )

            Spacer(Modifier.height(18.dp))
            Text("WHERE YOUR XP CAME FROM", style = MaterialTheme.typography.labelSmall, color = emphasized(muted), fontSize = 10.sp)
            Spacer(Modifier.height(8.dp))
            if (xp.sources.isEmpty()) {
                Text("Log your first workout to start earning XP.", style = MaterialTheme.typography.bodySmall, color = muted, fontStyle = FontStyle.Italic)
            }
            xp.sources.forEach { s ->
                Row(Modifier.fillMaxWidth().padding(vertical = 6.dp), horizontalArrangement = Arrangement.SpaceBetween, verticalAlignment = Alignment.CenterVertically) {
                    Column(Modifier.weight(1f)) {
                        Text(s.label, style = MaterialTheme.typography.bodyMedium, color = onBg)
                        Text(s.detail, style = MaterialTheme.typography.labelSmall, color = muted, fontSize = 10.sp)
                    }
                    Text("+${s.xp}", style = MaterialTheme.typography.bodyMedium, color = accent, fontWeight = FontWeight.SemiBold)
                }
            }

            Spacer(Modifier.height(18.dp))
            HorizontalDivider(color = outline.copy(alpha = 0.3f))
            Spacer(Modifier.height(14.dp))
            Text("HOW TO EARN MORE", style = MaterialTheme.typography.labelSmall, color = emphasized(muted), fontSize = 10.sp)
            Spacer(Modifier.height(8.dp))
            listOf(
                "Finish a workout" to "+100",
                "Each set logged" to "+4",
                "Set a PR" to "+50",
                "Every 1,000 lb moved" to "+5",
                "Each week you train" to "+40",
                "Unlock a trophy" to "+10–100"
            ).forEach { (what, amt) ->
                Row(Modifier.fillMaxWidth().padding(vertical = 4.dp), horizontalArrangement = Arrangement.SpaceBetween) {
                    Text(what, style = MaterialTheme.typography.bodySmall, color = muted)
                    Text(amt, style = MaterialTheme.typography.bodySmall, color = onBg)
                }
            }

            Spacer(Modifier.height(18.dp))
            HorizontalDivider(color = outline.copy(alpha = 0.3f))
            Spacer(Modifier.height(14.dp))
            Text("THE LADDER", style = MaterialTheme.typography.labelSmall, color = emphasized(muted), fontSize = 10.sp)
            Spacer(Modifier.height(8.dp))
            RankTier.entries.forEach { t ->
                val here = t == rank.tier
                Row(Modifier.fillMaxWidth().padding(vertical = 4.dp), horizontalArrangement = Arrangement.SpaceBetween) {
                    Text(
                        "${t.display} I–V" + if (here) "  · you are here" else "",
                        style = MaterialTheme.typography.bodySmall,
                        color = if (here) accent else muted,
                        fontWeight = if (here) FontWeight.SemiBold else FontWeight.Normal
                    )
                    Text("${RankLadder.tierFloor(t)}+ XP", style = MaterialTheme.typography.labelSmall, color = muted.copy(alpha = 0.7f), fontSize = 10.sp)
                }
            }
        }
    }
}
