package com.forge.app.ui.onboarding

import androidx.compose.animation.animateColorAsState
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
import androidx.compose.material3.minimumInteractiveComponentSize
import androidx.compose.runtime.Composable
import androidx.compose.runtime.getValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.clip
import androidx.compose.ui.semantics.contentDescription
import androidx.compose.ui.semantics.selected
import androidx.compose.ui.semantics.semantics
import androidx.compose.ui.text.style.TextAlign
import androidx.compose.ui.text.style.TextOverflow
import androidx.compose.ui.unit.Dp
import androidx.compose.ui.unit.dp
import com.forge.app.program.DayArchetype
import com.forge.app.program.GeneratedDay
import com.forge.app.ui.common.bounceClick
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
    val peak = (sets.maxOrNull() ?: 0).coerceAtLeast(1)

    Column(modifier.fillMaxWidth(), verticalArrangement = Arrangement.spacedBy(10.dp)) {
        StepSectionLabel(label, meta = "${sets.sum()} sets")
        Row(
            // When the bars are tappable each one announces itself, so a readout on the parent would
            // talk over its own children; the whole-week reading only stands in for the passive mark.
            Modifier
                .fillMaxWidth()
                .then(
                    if (onSelect == null) {
                        Modifier.semantics { contentDescription = weekReadout(archetypes, sets) }
                    } else Modifier
                ),
            horizontalArrangement = Arrangement.spacedBy(BAR_GAP),
            // Top, not bottom: every track is the same height, so aligning their TOPS keeps the
            // whole row of bars on one baseline even when one day's name wraps to two lines. Bottom
            // alignment measured the label into the column and floated that day's bar above its
            // neighbours (the 7-day split's "Arms & Delts" did exactly this).
            verticalAlignment = Alignment.Top
        ) {
            archetypes.forEachIndexed { i, archetype ->
                DayBar(
                    name = archetype.name,
                    // A day that trains at all keeps a visible stub, so a light day reads as light
                    // rather than as missing.
                    fraction = if (sets[i] <= 0) 0f else (sets[i].toFloat() / peak).coerceAtLeast(0.15f),
                    lit = sets[i] > 0,
                    // No selection = every bar reads as data. With one, accent marks the day being
                    // read and the rest step down to muted, so the accent means "you are here"
                    // rather than being spent on all seven at once (§5).
                    selected = selectedIndex == null || selectedIndex == i,
                    onClick = onSelect?.let { select -> { select(i) } },
                    readout = "${archetype.name}, ${sets[i]} sets",
                    trackHeight = trackHeight,
                    modifier = Modifier.weight(1f)
                )
            }
        }
    }
}

private val BAR_GAP = 8.dp

/**
 * One day's bar in its track, with the day's mono name beneath. The WHOLE column is the tap target
 * when it has one — bar plus label, never a nested tap (§2③) — which is also what gets it near
 * §14's 48dp at seven days across a phone gutter.
 */
@Composable
private fun DayBar(
    name: String,
    fraction: Float,
    lit: Boolean,
    selected: Boolean,
    onClick: (() -> Unit)?,
    readout: String,
    trackHeight: Dp,
    modifier: Modifier = Modifier
) {
    val animated by animateFloatAsState(fraction, ForgeMotion.standardTween(), label = "day_sets")
    // Muted at the 0.7 rung, not at full: six near-white slabs beside one accent bar out-shout the
    // accent (muted 1.0 measures 9.56:1 on Pearl, Ember 5.84:1), which inverts what the colour is
    // for. 0.7 is 5.18:1 — comfortably past §14's 3:1 floor for a mark that carries meaning, and
    // visibly a step below the day being read.
    val fill by animateColorAsState(
        if (selected) MaterialTheme.colorScheme.primary
        else MaterialTheme.colorScheme.onSurfaceVariant.copy(alpha = 0.7f),
        ForgeMotion.standardTween(ForgeMotion.DurationFast),
        label = "day_fill"
    )
    Column(
        modifier
            .then(
                if (onClick == null) Modifier
                else Modifier
                    .minimumInteractiveComponentSize()
                    .bounceClick(onClick = onClick)
                    .semantics {
                        contentDescription = readout
                        this.selected = selected
                    }
            ),
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
                    .background(fill)
            )
        }
        Text(
            // The same name the week page's day sections use, so the mark and the list can't read
            // as two different weeks.
            name.uppercase(),
            style = MaterialTheme.typography.labelSmall,
            color = if (lit && selected) MaterialTheme.colorScheme.onBackground
            else MaterialTheme.colorScheme.onSurfaceVariant,
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
