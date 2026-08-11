@file:OptIn(androidx.compose.material3.ExperimentalMaterial3Api::class)

package com.forge.app.ui.recipes

import androidx.compose.foundation.Canvas
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.rememberScrollState
import androidx.compose.foundation.verticalScroll
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
import androidx.compose.ui.geometry.CornerRadius
import androidx.compose.ui.geometry.Offset
import androidx.compose.ui.geometry.Size
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.semantics.contentDescription
import androidx.compose.ui.semantics.semantics
import androidx.compose.ui.tooling.preview.Preview
import androidx.compose.ui.unit.dp
import com.forge.app.ui.common.EditorialFigure
import com.forge.app.ui.common.EditorialHeader
import com.forge.app.ui.common.ForgePrimaryCapsule
import com.forge.app.ui.common.InlineEmptyHint
import com.forge.app.ui.common.SegmentPill
import com.forge.app.ui.common.rememberDrawProgress
import com.forge.app.ui.common.statsEntrance
import com.forge.app.ui.theme.ForgeTheme

/**
 * RECIPE — Overview / dashboard archetype (DESIGN §3).
 *
 * Copy this file's shape when building Home, Stats, Cardio, Coach or Profile. It is compiled in the
 * debug variant only, so it never ships, but it DOES compile — if a primitive's signature changes,
 * this breaks and the recipe gets updated instead of silently rotting.
 *
 * The order below is the archetype, not a suggestion:
 *
 *   top bar (← + ≤1 action, never the screen name, §4.6)
 *   ├─ hero        mono eyebrow → serif name/verdict            §3
 *   ├─ figures     2–4 EditorialFigure, honest zeros            §2①, §12
 *   ├─ lens pills  SegmentPill row, ONE-word labels             §4.4
 *   ├─ sections    each LEADS WITH A MARK, drawn at zero        §2②, §12
 *   └─ primary action, above the fold where it fits             §3
 *
 * Every element carries a `statsEntrance(index)` so the page cascades once (§9). Reduced motion is
 * automatic — `ForgeMotion` collapses every spec when the system preference is off, so no call site
 * checks it.
 */
@Composable
fun OverviewRecipe(
    // Drive the preview through both states. A real screen takes these off its ViewModel.
    weekSessions: List<Int> = listOf(1, 0, 2, 1, 0, 1, 0),
    prCount: Int = 3,
) {
    val onBg = MaterialTheme.colorScheme.onBackground
    val muted = MaterialTheme.colorScheme.onSurfaceVariant
    val accent = MaterialTheme.colorScheme.primary
    val outline = MaterialTheme.colorScheme.outline

    var lens by remember { mutableStateOf("Week") }

    Scaffold(
        topBar = {
            TopAppBar(
                // §4.6 — the bar carries NO title. NEVER the screen's own name: the serif hero
                // below names the screen, or nothing does.
                title = {},
                colors = TopAppBarDefaults.topAppBarColors(containerColor = Color.Transparent)
            )
        },
        containerColor = Color.Transparent
    ) { inner ->
        Column(
            Modifier
                .padding(inner)
                .verticalScroll(rememberScrollState())
                // §7 — page gutter is 24dp, always.
                .padding(horizontal = 24.dp)
        ) {
            // ── Hero ────────────────────────────────────────────────────────────────────────────
            // §3: mono eyebrow (identity + human date) over ONE serif line. The serif line appears
            // ONLY when it carries a decision or result. For plain status, drop it and let the
            // figures be the hero. §11: no terminal period on a serif title.
            Spacer(Modifier.height(8.dp))
            Column(Modifier.statsEntrance(0)) {
                Text(
                    "MONDAY 24 JUL",
                    style = MaterialTheme.typography.labelMedium,
                    color = muted
                )
                Spacer(Modifier.height(2.dp))
                Text(
                    "Pull B",
                    style = MaterialTheme.typography.headlineLarge,
                    color = onBg
                )
            }

            // ── Figure row ──────────────────────────────────────────────────────────────────────
            // §2①: 2–4 headline readings. §12: honest zeros ("0 PRS"), never a dash, never hidden.
            Spacer(Modifier.height(28.dp))
            Row(
                Modifier.fillMaxWidth().statsEntrance(1),
                horizontalArrangement = Arrangement.spacedBy(20.dp)
            ) {
                EditorialFigure("12", "sets", onBg, muted, accent)
                EditorialFigure("4.5k", "volume", onBg, muted, accent, delta = 2)
                EditorialFigure("$prCount", "prs", onBg, muted, accent)
            }

            // ── Lens pills ──────────────────────────────────────────────────────────────────────
            // §4.4: sub-paging is pills, labels ONE short word. Not buttons, not tabs.
            Spacer(Modifier.height(28.dp))
            Row(
                Modifier.statsEntrance(2),
                horizontalArrangement = Arrangement.spacedBy(8.dp)
            ) {
                listOf("Week", "Month", "Year").forEach { name ->
                    SegmentPill(
                        text = name,
                        selected = lens == name,
                        onClick = { lens = name },
                        accent = accent, onBg = onBg, muted = muted, outline = outline
                    )
                }
            }

            // ── A section that leads with a MARK ─────────────────────────────────────────────────
            // §7 rhythm: 28 → mono header → 10 → content. No hairline, no card — air separates.
            Spacer(Modifier.height(28.dp))
            EditorialHeader("THIS WEEK", muted, accent, Modifier.statsEntrance(3))
            Spacer(Modifier.height(10.dp))
            // §2②: "value per day-of-week" → 7 accent bars, and at zero they render as an empty
            // track rather than disappearing. The mark IS the content; words are only its caption.
            WeekBars(
                values = weekSessions,
                accent = accent,
                outline = outline,
                modifier = Modifier.statsEntrance(4)
            )
            // §4.3: at most ONE muted caption per section, ~12 words. Not a second one. Not a hint
            // as well (§12 — a hint REPLACES a caption).
            Spacer(Modifier.height(8.dp))
            Text(
                "Sessions per day, Monday first",
                style = MaterialTheme.typography.bodySmall,
                color = muted.copy(alpha = 0.65f) // §5: 0.65 is the hard floor, measured 4.54:1
            )

            // ── A section with nothing to show yet ───────────────────────────────────────────────
            // §12: an all-ghost group drops its mark (a lone flat line reads as broken) and
            // collapses to ONE line naming the concrete unlock. InlineEmptyHint is the LAST resort,
            // ≤1 per lens, and it replaces the caption rather than joining it.
            Spacer(Modifier.height(28.dp))
            EditorialHeader("RECORDS", muted, accent, Modifier.statsEntrance(5))
            Spacer(Modifier.height(10.dp))
            InlineEmptyHint(
                "First record lands after two sessions on a lift",
                muted.copy(alpha = 0.65f),
                Modifier.statsEntrance(6)
            )

            // ── Primary action ──────────────────────────────────────────────────────────────────
            // §8: ONE filled capsule, level ①. Its siblings would be outlined (level ②) and its
            // navigation would be a mono accent `action →` (level ③) — never three filled capsules.
            Spacer(Modifier.height(28.dp))
            ForgePrimaryCapsule(
                label = "Start session",
                onClick = { },
                modifier = Modifier.fillMaxWidth().statsEntrance(7)
            )
            Spacer(Modifier.height(32.dp))
        }
    }
}

/**
 * The section's mark. Note three things the doctrine requires and that are easy to miss:
 *
 *  1. It renders at zero — all-zero input draws an empty track, it does not vanish (§12).
 *  2. It carries a `contentDescription` reading its VALUE, not its shape (§14). A Canvas is
 *     invisible to TalkBack otherwise, and every mark in this app is a Canvas.
 *  3. Fixed `.height()` is correct HERE because the Canvas holds no text. The §14 rule bans fixed
 *     heights on containers that hold text, which is what breaks at 200% font scale.
 */
@Composable
private fun WeekBars(
    values: List<Int>,
    accent: Color,
    outline: Color,
    modifier: Modifier = Modifier,
) {
    val progress = rememberDrawProgress()          // §9: draws in ONCE, never re-triggers on scroll
    val max = (values.maxOrNull() ?: 0).coerceAtLeast(1)
    val total = values.sum()

    val reading = if (total == 0) {
        "No sessions logged this week"
    } else {
        "$total sessions this week, ${values.count { it > 0 }} of ${values.size} days"
    }

    Canvas(
        modifier
            .fillMaxWidth()
            .height(56.dp)
            .semantics { contentDescription = reading }
    ) {
        val gap = 8.dp.toPx()
        val barW = (size.width - gap * (values.size - 1)) / values.size
        val radius = CornerRadius(2.dp.toPx(), 2.dp.toPx())
        values.forEachIndexed { i, v ->
            val x = i * (barW + gap)
            // The track is always drawn — this is what makes zero honest instead of absent.
            drawRoundRect(
                color = outline.copy(alpha = 0.25f),   // §5 ladder: data lines / tracks
                topLeft = Offset(x, 0f),
                size = Size(barW, size.height),
                cornerRadius = radius
            )
            if (v > 0) {
                val h = size.height * (v.toFloat() / max) * progress
                drawRoundRect(
                    color = accent,                    // §5 ladder: accent 1.0 for the data itself
                    topLeft = Offset(x, size.height - h),
                    size = Size(barW, h),
                    cornerRadius = radius
                )
            }
        }
    }
}

@Preview(name = "Overview · with data", showBackground = true, backgroundColor = 0xFF0E0E11)
@Composable
private fun OverviewRecipePreview() {
    ForgeTheme { OverviewRecipe() }
}

/** Design each section at its EMPTIEST realistic state, not its fullest (§12). */
@Preview(name = "Overview · at zero", showBackground = true, backgroundColor = 0xFF0E0E11)
@Composable
private fun OverviewRecipeZeroPreview() {
    ForgeTheme {
        OverviewRecipe(weekSessions = List(7) { 0 }, prCount = 0)
    }
}

/** §14: the app must survive 200%. Check every screen you touch at this scale. */
@Preview(name = "Overview · 200% font", showBackground = true, backgroundColor = 0xFF0E0E11, fontScale = 2.0f)
@Composable
private fun OverviewRecipeLargeFontPreview() {
    ForgeTheme { OverviewRecipe() }
}
