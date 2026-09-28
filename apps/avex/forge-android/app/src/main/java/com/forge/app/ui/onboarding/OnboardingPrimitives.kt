package com.forge.app.ui.onboarding

import androidx.compose.animation.animateColorAsState
import androidx.compose.foundation.background
import androidx.compose.foundation.border
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.material3.Icon
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.minimumInteractiveComponentSize
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.runtime.getValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.clip
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.semantics.contentDescription
import androidx.compose.ui.semantics.selected
import androidx.compose.ui.semantics.semantics
import androidx.compose.ui.text.style.TextAlign
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp
import com.forge.app.ui.common.ForgeChoiceChip
import com.forge.app.ui.common.ForgePrimaryCapsule
import com.forge.app.ui.theme.ForgeMotion

/**
 * Onboarding building blocks: the question, its caption, section anchors, the step rail and the
 * CTA capsules. The answers themselves (grouped rows, tile grids, segmented controls) live in
 * `OnboardingGroups.kt`, so every page shares one surface language.
 */

/** The step's question — the page title voice (serif, no terminal period). */
@Composable
internal fun StepTitle(text: String) {
    Text(
        text,
        style = MaterialTheme.typography.headlineSmall,
        color = MaterialTheme.colorScheme.onBackground
    )
}

/** One quiet caption line under the title (~12 words, §4.3). */
@Composable
internal fun StepCaption(text: String) {
    Text(
        text,
        style = MaterialTheme.typography.bodySmall,
        color = MaterialTheme.colorScheme.onSurfaceVariant.copy(alpha = 0.7f)
    )
}

/** Small mono section anchor inside a step (UNITS / PLATE WEIGHT / RACKS & BENCHES). */
@Composable
internal fun StepSectionLabel(text: String, meta: String? = null) {
    Row(
        Modifier.fillMaxWidth(),
        horizontalArrangement = Arrangement.SpaceBetween,
        verticalAlignment = Alignment.CenterVertically
    ) {
        Text(
            text.uppercase(),
            style = MaterialTheme.typography.labelMedium,
            color = MaterialTheme.colorScheme.onSurfaceVariant,
            letterSpacing = 1.sp
        )
        if (meta != null) {
            Text(
                meta.uppercase(),
                style = MaterialTheme.typography.labelSmall,
                color = MaterialTheme.colorScheme.onSurfaceVariant
            )
        }
    }
}

/** A capsule choice chip (plates, refresh cadence, sore spots, sex, watch). */
@Composable
internal fun ChoiceChip(label: String, selected: Boolean, onClick: () -> Unit, modifier: Modifier = Modifier) =
    ForgeChoiceChip(label, selected, onClick, modifier)

/**
 * The step rail: one cell per step of the path you are actually on, accent behind you, hollow ahead
 * (§2② — a filled / hollow rail for a set of items, some present). It replaced a single continuous
 * bar (2026-08-22) whose denominator had to *guess* the path length before the plan-mode fork; cells
 * say how many steps are left instead of implying a fraction, and committing to the short custom /
 * freestyle path visibly drops the cells that will never run.
 */
@Composable
internal fun StepRail(step: Int, total: Int, modifier: Modifier = Modifier) {
    Row(
        modifier.semantics { contentDescription = "Step ${step + 1} of $total" },
        horizontalArrangement = Arrangement.spacedBy(4.dp)
    ) {
        repeat(total) { i ->
            val color by animateColorAsState(
                if (i <= step) MaterialTheme.colorScheme.primary
                else MaterialTheme.colorScheme.outline.copy(alpha = 0.25f),
                ForgeMotion.standardTween(),
                label = "rail_cell"
            )
            Box(
                Modifier
                    .weight(1f)
                    .height(4.dp)
                    .clip(RoundedCornerShape(50))
                    .background(color)
            )
        }
    }
}

/** §8 level ① — the filled light do-it-now capsule (Continue / Start training). Onboarding's
 *  full-width CTA is the shared [ForgePrimaryCapsule] under an onboarding-local name, so the two
 *  can't drift. */
@Composable
internal fun PrimaryCapsule(
    label: String,
    onClick: () -> Unit,
    modifier: Modifier = Modifier,
    enabled: Boolean = true
) = ForgePrimaryCapsule(label = label, onClick = onClick, modifier = modifier, enabled = enabled)
