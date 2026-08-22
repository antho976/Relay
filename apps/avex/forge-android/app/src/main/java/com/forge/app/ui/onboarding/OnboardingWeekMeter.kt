package com.forge.app.ui.onboarding

import androidx.compose.animation.core.animateFloatAsState
import androidx.compose.foundation.background
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.fillMaxHeight
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.widthIn
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.runtime.getValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.clip
import androidx.compose.ui.semantics.contentDescription
import androidx.compose.ui.semantics.semantics
import androidx.compose.ui.text.style.TextAlign
import androidx.compose.ui.text.style.TextOverflow
import androidx.compose.ui.unit.Dp
import androidx.compose.ui.unit.dp
import com.forge.app.program.DayArchetype
import com.forge.app.program.GeneratedDay
import com.forge.app.ui.theme.ForgeMotion

/**
 * The plan under construction — onboarding's one persistent mark, and the reason the rebuilt flow
 * reads as *building* rather than as an interview (2026-08-22).
 *
 * It draws the §2② mark for a value per training day: **one bar per day, carrying that day's sets**,
 * scaled against the heaviest day of the week. Both stages of it are true:
 *
 * - as soon as the day-count is picked, the split and its planned volume are known without any gear
 *   at all ([com.forge.app.program.ProgramGenerator.plannedSetsPerDay]), so the week's shape lands
 *   immediately rather than as a row of dead placeholders;
 * - once the gym is picked, the bars carry the generated week's real sets. A well-stocked gym holds
 *   the shape; a sparse one can't fill every slot, so the bars visibly drop. That is the true
 *   consequence of the answer, drawn instead of narrated.
 *
 * Why not "exercises the gear supports, out of the slots the split wants": measured, it barely
 * moves. The generator keeps a last-resort bodyweight fill, so even a bodyweight-only setup lands
 * 24 of 25 — a meter whose needle never moves is decoration.
 *
 * Seven days always share the row's width rather than scrolling: a bar row that runs off the gutter
 * reads as broken, and a two-line day name under a narrow bar does not. The name clamps at two lines
 * as §14 allows a mono label to, and the value-reading `contentDescription` carries it regardless.
 *
 * The mark lives OUTSIDE the page slider in [OnboardingScreen], so questions come and go while the
 * week stays put and animates its own values. It carries a value-reading `contentDescription`
 * (§14), and day identity rides on the mono day name under each bar, so the §5 one-accent rule
 * holds without spending a colour per day.
 */
@Composable
internal fun PlanLedger(
    archetypes: List<DayArchetype>,
    plannedSets: List<Int>,
    days: List<GeneratedDay>?,
    modifier: Modifier = Modifier,
    trackHeight: Dp = 72.dp
) {
    if (archetypes.isEmpty()) return
    // The generated week wins once it exists; before that, the split's own planned volume.
    val sets = archetypes.indices.map { i ->
        days?.getOrNull(i)?.exercises?.sumOf { it.sets } ?: plannedSets.getOrElse(i) { 0 }
    }
    val peak = (sets.maxOrNull() ?: 0).coerceAtLeast(1)

    Column(modifier.fillMaxWidth(), verticalArrangement = Arrangement.spacedBy(10.dp)) {
        StepSectionLabel("Your week", meta = "${sets.sum()} sets")
        Row(
            Modifier
                .fillMaxWidth()
                .semantics { contentDescription = weekReadout(archetypes, sets) },
            horizontalArrangement = Arrangement.spacedBy(BAR_GAP),
            verticalAlignment = Alignment.Bottom
        ) {
            archetypes.forEachIndexed { i, archetype ->
                DayBar(
                    name = archetype.name,
                    // A day that trains at all keeps a visible stub, so a light day reads as light
                    // rather than as missing.
                    fraction = if (sets[i] <= 0) 0f else (sets[i].toFloat() / peak).coerceAtLeast(0.15f),
                    lit = sets[i] > 0,
                    trackHeight = trackHeight,
                    modifier = Modifier.weight(1f)
                )
            }
        }
    }
}

private val BAR_GAP = 8.dp

/** One day's bar in its track, with the day's mono name beneath. */
@Composable
private fun DayBar(
    name: String,
    fraction: Float,
    lit: Boolean,
    trackHeight: Dp,
    modifier: Modifier = Modifier
) {
    val animated by animateFloatAsState(fraction, ForgeMotion.standardTween(), label = "day_sets")
    Column(
        modifier,
        horizontalAlignment = Alignment.CenterHorizontally,
        verticalArrangement = Arrangement.spacedBy(8.dp)
    ) {
        Box(
            Modifier
                // Capped so four days read as bars rather than as slabs of accent, and seven still
                // fit the gutter. No text inside, so a fixed height is safe at any font scale (§14).
                .widthIn(max = 28.dp)
                .fillMaxWidth()
                .height(trackHeight)
                .clip(RoundedCornerShape(4.dp))
                .background(MaterialTheme.colorScheme.outline.copy(alpha = 0.25f)),
            contentAlignment = Alignment.BottomCenter
        ) {
            Box(
                Modifier
                    .fillMaxWidth()
                    .fillMaxHeight(animated.coerceIn(0f, 1f))
                    .clip(RoundedCornerShape(4.dp))
                    .background(MaterialTheme.colorScheme.primary)
            )
        }
        Text(
            // The same name the week page's day sections use, so the mark and the list can't read
            // as two different weeks.
            name.uppercase(),
            style = MaterialTheme.typography.labelSmall,
            color = if (lit) MaterialTheme.colorScheme.onBackground else MaterialTheme.colorScheme.onSurfaceVariant,
            textAlign = TextAlign.Center,
            maxLines = 2,
            overflow = TextOverflow.Ellipsis
        )
    }
}

/** TalkBack reads the values, not the shape (§14): "Upper A, 24 sets. Lower A, 20 sets…". */
private fun weekReadout(archetypes: List<DayArchetype>, sets: List<Int>): String =
    "Your week, ${archetypes.size} days. " + archetypes.mapIndexed { i, a ->
        "${a.name}, ${sets.getOrElse(i) { 0 }} sets"
    }.joinToString(". ")
