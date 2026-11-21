package com.forge.app.ui.onboarding

import androidx.compose.animation.core.animateFloatAsState
import androidx.compose.foundation.background
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.widthIn
import androidx.compose.foundation.selection.selectable
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.material3.MaterialTheme
import androidx.compose.runtime.Composable
import androidx.compose.runtime.getValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.clip
import androidx.compose.ui.layout.layout
import androidx.compose.ui.semantics.Role
import androidx.compose.ui.semantics.clearAndSetSemantics
import androidx.compose.ui.semantics.contentDescription
import androidx.compose.ui.semantics.selected
import androidx.compose.ui.semantics.semantics
import androidx.compose.ui.text.style.TextAlign
import androidx.compose.ui.unit.Dp
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp
import com.forge.app.program.DayArchetype
import com.forge.app.program.GeneratedDay
import com.forge.app.ui.common.OpenLabel
import com.forge.app.ui.theme.ForgeMotion
import com.forge.app.ui.theme.OnboardingColors
import com.forge.app.ui.theme.OpenColors
import com.forge.app.ui.theme.OpenType

/**
 * The plan under construction, drawn as the Program page draws a week: **one bar per training day
 * carrying that day's sets**, scaled against the heaviest day. Both stages of it are true:
 *
 * - as soon as the day-count is picked, the split and its planned volume are known without any gear
 *   at all ([com.forge.app.program.ProgramGenerator.plannedSetsPerDay]), so the week's shape lands
 *   immediately rather than as a row of dead placeholders;
 * - once the gym is picked, the bars carry the generated week's real sets. A well-stocked gym holds
 *   the shape; a sparse one can't fill every slot, so the bars visibly drop.
 *
 * Seven days always share the row's width rather than scrolling: a bar row that runs off the gutter
 * reads as broken, and a two-line day name under a narrow bar does not.
 */

/** The sets each day carries: the generated week once it exists, the split's planned volume before. */
internal fun weekSets(archetypes: List<DayArchetype>, plannedSets: List<Int>, days: List<GeneratedDay>?): List<Int> =
    archetypes.indices.map { i ->
        days?.getOrNull(i)?.exercises?.sumOf { it.sets } ?: plannedSets.getOrElse(i) { 0 }
    }

/**
 * The bars. Without [onSelect] every bar is lit (the Days page, where the whole week is the answer)
 * and the row reads its values aloud as one. With it each bar is the tap target for its own day:
 * red for the day being read, the quiet grey for the rest (the week page).
 */
@Composable
internal fun WeekBars(
    names: List<String>,
    sets: List<Int>,
    trackWidth: Dp,
    trackHeight: Dp,
    labelGap: Dp,
    modifier: Modifier = Modifier,
    selectedIndex: Int? = null,
    onSelect: ((Int) -> Unit)? = null
) {
    val max = (sets.maxOrNull() ?: 0).coerceAtLeast(1)
    val accent = MaterialTheme.colorScheme.primary
    Row(
        modifier
            .fillMaxWidth()
            .then(
                if (onSelect == null) Modifier.clearAndSetSemantics { contentDescription = weekReadout(names, sets) }
                else Modifier
            ),
        horizontalArrangement = Arrangement.spacedBy(10.dp),
        verticalAlignment = Alignment.Top
    ) {
        names.forEachIndexed { i, name ->
            val picked = selectedIndex == i
            val lit = onSelect == null || picked
            val fraction by animateFloatAsState(
                sets.getOrElse(i) { 0 }.toFloat() / max, ForgeMotion.standardTween(), label = "week_bar"
            )
            Column(
                Modifier
                    .weight(1f)
                    .then(
                        if (onSelect != null) Modifier
                            .selectable(selected = picked, role = Role.Tab, onClick = { onSelect(i) })
                            .semantics {
                                selected = picked
                                contentDescription = "$name, ${sets.getOrElse(i) { 0 }} sets"
                            }
                        else Modifier
                    ),
                horizontalAlignment = Alignment.CenterHorizontally
            ) {
                Box(
                    Modifier
                        .widthIn(max = trackWidth)
                        .fillMaxWidth()
                        .height(trackHeight)
                        .clip(RoundedCornerShape(4.dp))
                        .background(OnboardingColors.BarTrack),
                    contentAlignment = Alignment.BottomCenter
                ) {
                    if (fraction > 0f) {
                        Box(
                            Modifier
                                .fillMaxWidth()
                                .height(trackHeight * fraction)
                                .clip(RoundedCornerShape(4.dp))
                                .background(if (lit) accent else OpenColors.Bar)
                        )
                    }
                }
                Spacer(Modifier.height(labelGap))
                // Six or seven bars leave ~39dp a column at a large font: there each word of a name
                // takes its own line, and a long one ("CALVES") may use the gap beside its bar
                // rather than break mid-word. The tracking goes too.
                val crowded = names.size >= 6
                OpenLabel(
                    if (crowded) name.replace(' ', '\n') else name,
                    modifier = Modifier.layout { measurable, constraints ->
                        val room = constraints.maxWidth + if (crowded) 10.dp.roundToPx() else 0
                        val p = measurable.measure(constraints.copy(minWidth = 0, maxWidth = room))
                        layout(constraints.maxWidth, p.height) { p.place((constraints.maxWidth - p.width) / 2, 0) }
                    },
                    color = if (onSelect == null || !picked) MaterialTheme.colorScheme.onSurfaceVariant
                    else MaterialTheme.colorScheme.onBackground,
                    style = OpenType.monoXS.copy(
                        textAlign = TextAlign.Center,
                        letterSpacing = if (crowded) 0.sp else OpenType.monoXS.letterSpacing
                    )
                )
            }
        }
    }
}

/** TalkBack reads the values, not the shape: "Your week, 4 days. Upper A, 24 sets. Lower A, 20 sets…". */
private fun weekReadout(names: List<String>, sets: List<Int>): String =
    "Your week, ${names.size} days. " + names.mapIndexed { i, n -> "$n, ${sets.getOrElse(i) { 0 }} sets" }.joinToString(". ")
