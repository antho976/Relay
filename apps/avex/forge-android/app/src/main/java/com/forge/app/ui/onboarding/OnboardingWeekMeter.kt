package com.forge.app.ui.onboarding

import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.runtime.Composable
import androidx.compose.ui.Modifier
import androidx.compose.ui.semantics.contentDescription
import androidx.compose.ui.semantics.semantics
import androidx.compose.ui.unit.Dp
import androidx.compose.ui.unit.dp
import com.forge.app.program.DayArchetype
import com.forge.app.program.GeneratedDay
import com.forge.app.ui.common.WeekBarRail

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
 *
 * **On the week page it also navigates** (2026-08-23). Pass [selectedIndex] / [onSelect] and each
 * bar becomes the tap target for its own day: accent for the day being read, muted for the rest,
 * which is §4.5's "aggregate visuals answer a tap with detail" and the reason [StepWeek] no longer
 * needs three viewports of rows. Without those two arguments nothing is tappable and the mark draws
 * exactly as it did under the questions — one implementation, so the two can't drift into reading
 * as two different weeks.
 *
 * @param label the mono anchor over the bars; the week page overrides it because its own title
 *   already says "your week", and §4.3 gives a fact one home.
 */
@Composable
internal fun PlanLedger(
    archetypes: List<DayArchetype>,
    plannedSets: List<Int>,
    days: List<GeneratedDay>?,
    modifier: Modifier = Modifier,
    trackHeight: Dp = 72.dp,
    label: String = "Your week",
    selectedIndex: Int? = null,
    onSelect: ((Int) -> Unit)? = null
) {
    if (archetypes.isEmpty()) return
    // The generated week wins once it exists; before that, the split's own planned volume.
    val sets = archetypes.indices.map { i ->
        days?.getOrNull(i)?.exercises?.sumOf { it.sets } ?: plannedSets.getOrElse(i) { 0 }
    }

    Column(modifier.fillMaxWidth(), verticalArrangement = Arrangement.spacedBy(10.dp)) {
        StepSectionLabel(label, meta = "${sets.sum()} sets")
        // The bars themselves live in ui/common so Your program draws the same week (2026-09-25).
        WeekBarRail(
            names = archetypes.map { it.name },
            sets = sets,
            // When the bars are tappable each one announces itself, so a readout on the parent would
            // talk over its own children; the whole-week reading only stands in for the passive mark.
            modifier = if (onSelect == null) {
                Modifier.semantics { contentDescription = weekReadout(archetypes, sets) }
            } else Modifier,
            trackHeight = trackHeight,
            selectedIndex = selectedIndex,
            onSelect = onSelect
        )
    }
}

/** TalkBack reads the values, not the shape (§14): "Upper A, 24 sets. Lower A, 20 sets…". */
private fun weekReadout(archetypes: List<DayArchetype>, sets: List<Int>): String =
    "Your week, ${archetypes.size} days. " + archetypes.mapIndexed { i, a ->
        "${a.name}, ${sets.getOrElse(i) { 0 }} sets"
    }.joinToString(". ")
