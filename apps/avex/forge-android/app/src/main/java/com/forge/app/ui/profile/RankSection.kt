package com.forge.app.ui.profile

import androidx.compose.animation.core.animateFloatAsState
import androidx.compose.animation.core.animateIntAsState
import androidx.compose.animation.core.tween
import androidx.compose.foundation.Canvas
import androidx.compose.foundation.background
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.fillMaxHeight
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.material3.ExperimentalMaterial3Api
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.ModalBottomSheet
import androidx.compose.material3.Text
import androidx.compose.material3.rememberModalBottomSheetState
import androidx.compose.runtime.Composable
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.setValue
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.clip
import androidx.compose.ui.geometry.Offset
import androidx.compose.ui.graphics.Brush
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.graphics.StrokeCap
import androidx.compose.ui.graphics.drawscope.Stroke
import androidx.compose.ui.graphics.graphicsLayer
import androidx.compose.ui.semantics.contentDescription
import androidx.compose.ui.semantics.semantics
import androidx.compose.ui.graphics.lerp
import androidx.compose.ui.layout.boundsInWindow
import androidx.compose.ui.layout.onGloballyPositioned
import androidx.compose.ui.text.font.FontStyle
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp
import com.forge.app.domain.rank.RankInfo
import com.forge.app.domain.rank.RankLadder
import com.forge.app.domain.rank.RankTier
import com.forge.app.domain.rank.SUB_RANKS_PER_TIER
import com.forge.app.domain.rank.XpBreakdown
import com.forge.app.domain.rank.XpEngine
import com.forge.app.domain.units.toDisplayWeight
import com.forge.app.domain.units.unitLabel
import com.forge.app.ui.common.bounceClick
import com.forge.app.ui.theme.ForgeMotion
import com.forge.app.ui.theme.LocalForgeSettings
import kotlin.math.roundToInt

/**
 * The rank section: a big, alive hero emblem for the current tier (a real drawn fire at Ember /
 * Flare — see [HeroEmblem]), a slim six-dot rail beneath it tracking the journey Ember → Supernova,
 * and the XP-to-next bar. The connector lives only in the gaps between dots, so it never cuts
 * through an emblem, and every dot wears its own tier colour. Tap anywhere for the "How XP works" sheet.
 */
@Composable
internal fun RankSection(
    rank: RankInfo,
    muted: Color,
    accent: Color,
    outline: Color,
    onInfo: () -> Unit
) {
    val tiers = RankTier.entries
    // Smooth fill: how far along the 5 gaps between the 6 dots we are.
    val withinTier = ((rank.subRank - 1) + rank.progressInRank) / SUB_RANKS_PER_TIER
    val pos = ((rank.tier.ordinal + withinTier) / (tiers.size - 1)).coerceIn(0f, 1f)
    val tierColor = rank.tier.color()
    val motion = ForgeMotion.durationScale > 0f
    // The emblem animation loops forever; pause it while the section is scrolled out of view
    // (the parent screen is a verticalScroll, which keeps children composed off-screen).
    var visible by remember { mutableStateOf(true) }

    var play by remember { mutableStateOf(false) }
    LaunchedEffect(Unit) { play = true }
    val enter by animateFloatAsState(
        targetValue = if (play) 1f else 0f,
        animationSpec = ForgeMotion.standardTween(ForgeMotion.DurationEmphasized),
        label = "rank-enter"
    )

    Column(
        Modifier.fillMaxWidth()
            .onGloballyPositioned { visible = it.boundsInWindow().height > 0f }
            .bounceClick { onInfo() }
    ) {
        // ── Hero emblem ───────────────────────────────────────────────────────
        Box(Modifier.fillMaxWidth(), contentAlignment = Alignment.Center) {
            Box(Modifier.size(104.dp), contentAlignment = Alignment.Center) {
                // Soft tier-coloured glow behind the emblem. Fades to the same hue at 0 alpha — NOT
                // to Color.Transparent (transparent black), which interpolates through grey and
                // leaves a muddy dark fringe.
                Canvas(Modifier.matchParentSize()) {
                    val r = size.minDimension / 2f
                    drawCircle(
                        brush = Brush.radialGradient(
                            colors = listOf(tierColor.copy(alpha = 0.30f), tierColor.copy(alpha = 0f)),
                            center = center,
                            radius = r
                        ),
                        radius = r,
                        center = center
                    )
                }
                // Faint tier disc + thin ring, drawn in one pass.
                Canvas(Modifier.size(86.dp)) {
                    val r = size.minDimension / 2f
                    drawCircle(tierColor.copy(alpha = 0.06f), radius = r)
                    drawCircle(tierColor.copy(alpha = 0.40f), radius = r, style = Stroke(width = 1.5.dp.toPx()))
                }
                Box(
                    Modifier
                        // The animated rank emblem is the Profile hero but a silent Canvas — name it.
                        .semantics { contentDescription = "${rank.displayName} rank emblem" }
                        .graphicsLayer {
                            val s = 0.82f + 0.18f * enter
                            scaleX = s; scaleY = s; alpha = enter
                        },
                    contentAlignment = Alignment.Center
                ) {
                    HeroEmblem(rank.tier, animated = motion && visible, size = 60.dp)
                }
            }
        }

        Spacer(Modifier.height(16.dp))
        // ── Slim rail ─────────────────────────────────────────────────────────
        Canvas(Modifier.fillMaxWidth().height(18.dp)) {
            val n = tiers.size
            val cy = size.height / 2f
            val dotR = 4.5.dp.toPx()
            val curR = 6.5.dp.toPx()
            val startX = curR + 1.dp.toPx()
            val endX = size.width - curR - 1.dp.toPx()
            fun cxAt(i: Int) = startX + (endX - startX) * i / (n - 1)
            val gap = 2.5.dp.toPx()
            val stroke = 2.dp.toPx()

            // Connector segments — gaps only, so the line never crosses a dot.
            for (i in 0 until n - 1) {
                val a = cxAt(i) + dotR + gap
                val b = cxAt(i + 1) - dotR - gap
                if (b <= a) continue
                drawLine(outline.copy(alpha = 0.25f), Offset(a, cy), Offset(b, cy), strokeWidth = stroke, cap = StrokeCap.Round)
                val segFrac = ((pos * enter - i.toFloat() / (n - 1)) * (n - 1)).coerceIn(0f, 1f)
                if (segFrac > 0f) {
                    val fill = lerp(tiers[i].color(), tiers[i + 1].color(), 0.5f)
                    drawLine(fill, Offset(a, cy), Offset(a + (b - a) * segFrac, cy), strokeWidth = stroke, cap = StrokeCap.Round)
                }
            }
            // Dots — reached solid in tier colour, current with a halo ring, locked faint.
            for (i in 0 until n) {
                val c = Offset(cxAt(i), cy)
                val tc = tiers[i].color()
                val reached = i <= rank.tier.ordinal
                val isCur = i == rank.tier.ordinal
                if (reached) {
                    drawCircle(tc, radius = if (isCur) curR else dotR, center = c)
                    if (isCur) drawCircle(tc.copy(alpha = 0.35f), radius = curR + 3.dp.toPx(), center = c, style = Stroke(width = 1.5.dp.toPx()))
                } else {
                    drawCircle(outline.copy(alpha = 0.35f), radius = dotR, center = c, style = Stroke(width = 1.5.dp.toPx()))
                }
            }
        }

        Spacer(Modifier.height(16.dp))
        // XP-to-next bar: progress through the current tier toward the next (full once maxed). Fills in on
        // enter. Inset to 5/6 width so it reads as one piece with the rail above.
        val barTarget = if (rank.isMax) 1f else withinTier
        val barFill = barTarget * enter
        Box(Modifier.fillMaxWidth(), contentAlignment = Alignment.Center) {
            Box(Modifier.fillMaxWidth(5f / 6f).height(4.dp).clip(RoundedCornerShape(50)).background(outline.copy(alpha = 0.25f))) {
                Box(Modifier.fillMaxWidth(barFill).fillMaxHeight().clip(RoundedCornerShape(50)).background(tierColor))
            }
        }

        // Apex treatment: at Supernova V there's no "next", so the to-next line gives way to a badge.
        if (rank.isMax) {
            Spacer(Modifier.height(12.dp))
            Box(Modifier.fillMaxWidth(), contentAlignment = Alignment.Center) {
                Text(
                    "★ ${rank.displayName.uppercase()} · MAX RANK ★",
                    style = MaterialTheme.typography.labelSmall, color = tierColor,
                    fontSize = 9.sp, letterSpacing = 2.sp, fontWeight = FontWeight.Bold,
                    modifier = Modifier.clip(RoundedCornerShape(50)).background(tierColor.copy(alpha = 0.15f)).padding(horizontal = 14.dp, vertical = 4.dp)
                )
            }
        }

        Spacer(Modifier.height(10.dp))
        Row(Modifier.fillMaxWidth(), horizontalArrangement = Arrangement.SpaceBetween, verticalAlignment = Alignment.CenterVertically) {
            Text("HOW XP WORKS →", style = MaterialTheme.typography.labelSmall, color = accent, fontSize = 9.sp)
            // Next milestone = the immediate next sub-rank (closer + more motivating than the far-off
            // tier). Sourced from RankInfo so the UI never re-derives the ladder structure.
            val tail = if (rank.isMax) "MAX RANK" else "${rank.xpToNextRank} TO ${rank.nextRankName?.uppercase()}"
            Text("${rank.xpTotal} XP · $tail", style = MaterialTheme.typography.labelSmall, color = muted, fontSize = 9.sp)
        }
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

    ModalBottomSheet(onDismissRequest = onDismiss, sheetState = rememberModalBottomSheetState(skipPartiallyExpanded = true)) {
        Column(Modifier.fillMaxWidth().padding(horizontal = 24.dp).padding(bottom = 32.dp)) {
            Text("How XP works", style = MaterialTheme.typography.headlineSmall, color = onBg, fontStyle = FontStyle.Italic)
            Spacer(Modifier.height(4.dp))
            Text(
                "${rank.displayName} · ${rank.xpTotal} XP" +
                    if (!rank.isMax) " · ${rank.xpToNextTier} to ${rank.nextTierName}" else " · max rank",
                style = MaterialTheme.typography.bodySmall, color = muted
            )

            Spacer(Modifier.height(18.dp))
            Text("WHERE YOUR XP CAME FROM", style = MaterialTheme.typography.labelSmall, color = muted, fontSize = 10.sp)
            Spacer(Modifier.height(8.dp))
            if (xp.sources.isEmpty()) {
                Text("Log your first workout to start earning XP.", style = MaterialTheme.typography.bodySmall, color = muted, fontStyle = FontStyle.Italic)
            }
            // Each source row counts up from 0 the first time the sheet opens.
            var playXp by remember { mutableStateOf(false) }
            LaunchedEffect(Unit) { playXp = true }
            xp.sources.forEach { s ->
                Row(Modifier.fillMaxWidth().padding(vertical = 6.dp), horizontalArrangement = Arrangement.SpaceBetween, verticalAlignment = Alignment.CenterVertically) {
                    Column(Modifier.weight(1f)) {
                        Text(s.label, style = MaterialTheme.typography.bodyMedium, color = onBg)
                        Text(s.detail, style = MaterialTheme.typography.labelSmall, color = muted, fontSize = 10.sp)
                    }
                    val animatedXp by animateIntAsState(
                        targetValue = if (playXp) s.xp.toInt() else 0,
                        animationSpec = tween(ForgeMotion.scaledDuration(ForgeMotion.DurationEmphasized)),
                        label = "xp-source"
                    )
                    Text("+$animatedXp", style = MaterialTheme.typography.bodyMedium, color = accent, fontWeight = FontWeight.SemiBold)
                }
            }

            // Air + the mono header separate the sheet's sections (§1) — no hairline strips.
            Spacer(Modifier.height(28.dp))
            Text("HOW TO EARN MORE", style = MaterialTheme.typography.labelSmall, color = muted, fontSize = 10.sp)
            Spacer(Modifier.height(8.dp))
            val weightUnit = LocalForgeSettings.current.weightUnit
            // Amounts + the volume threshold are derived from XpEngine so this list can never drift
            // from the actual scoring. The kg figure is the true conversion of 1000 lb (≈454 kg),
            // not a hand-rounded "450".
            val per1000 = toDisplayWeight(1000.0, weightUnit).roundToInt()
            listOf(
                "Finish a workout" to "+${XpEngine.WORKOUT_XP}",
                "Each set logged" to "+${XpEngine.SET_XP}",
                "Set a PR" to "+${XpEngine.PR_XP}",
                "Every ${String.format(java.util.Locale.US, "%,d", per1000)} ${unitLabel(weightUnit)} moved" to "+${XpEngine.VOLUME_XP_PER_1000LB}",
                "Each week you train" to "+${XpEngine.ACTIVE_WEEK_XP}",
                "Unlock a trophy" to "+10–100"
            ).forEach { (what, amt) ->
                Row(Modifier.fillMaxWidth().padding(vertical = 4.dp), horizontalArrangement = Arrangement.SpaceBetween) {
                    Text(what, style = MaterialTheme.typography.bodySmall, color = muted)
                    Text(amt, style = MaterialTheme.typography.bodySmall, color = onBg)
                }
            }

            Spacer(Modifier.height(28.dp))
            Text("THE LADDER", style = MaterialTheme.typography.labelSmall, color = muted, fontSize = 10.sp)
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
