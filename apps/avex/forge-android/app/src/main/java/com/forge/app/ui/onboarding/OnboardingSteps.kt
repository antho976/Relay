package com.forge.app.ui.onboarding

import androidx.compose.foundation.background
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.PaddingValues
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.shape.CircleShape
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.runtime.mutableStateMapOf
import androidx.compose.runtime.remember
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.platform.LocalDensity
import androidx.compose.ui.text.AnnotatedString
import androidx.compose.ui.text.TextStyle
import androidx.compose.ui.text.buildAnnotatedString
import androidx.compose.ui.text.style.TextAlign
import androidx.compose.ui.text.withStyle
import androidx.compose.ui.unit.dp
import com.forge.app.domain.schedule.WeeklySchedule
import com.forge.app.program.DayArchetype
import com.forge.app.program.GeneratedDay
import com.forge.app.program.SessionEstimate
import com.forge.app.ui.common.OpenLabel
import com.forge.app.ui.theme.ForgeWarning
import com.forge.app.ui.theme.OnboardingType
import com.forge.app.ui.theme.OpenType

/**
 * The plan-shaping questions, in the order the generator needs them: the plan-mode fork first (it
 * decides how much of the flow even runs), then goal, experience, day-count and weekdays. See
 * [OnboardingScreen] for the page order and `OnboardingGymSteps.kt` for the gym half.
 *
 * Every page is `serif question → one caption → its answers`: outlined tiles for a pick, a big serif
 * figure over a red slider for a value (redrawn 2026-10-21 to the "Open panel" designs).
 */

/** Goal options: key, label, what it changes, and the mono rep-range meta. Settings reads these too. */
internal val GOAL_DETAILS = listOf(
    listOf("build_muscle", "Build muscle", "Balanced for size. Most people start here.", "8-12 reps"),
    listOf("get_stronger", "Get stronger", "Heavier work on the big lifts.", "4-6 reps"),
    listOf("lose_weight", "Lose weight", "Higher reps with more conditioning.", "12-20 reps"),
    listOf("general_fitness", "General fitness", "Balanced, all-round training.", "8-15 reps")
)

/** Experience bands — non-overlapping, mapped to the generator's level keys. */
internal val EXPERIENCE_DETAILS = listOf(
    listOf("beginner", "New to lifting", "A bit less volume, no advanced lifts yet.", "Under 6 mo"),
    listOf("intermediate", "Got the basics down", "Standard volume.", "6 mo to 2 yr"),
    listOf("advanced", "Experienced", "A touch more volume.", "2+ yr")
)

/** Plan source — generated, self-built, or no plan at all. Each tile carries its live vignette. */
internal val PLAN_MODE_DETAILS = listOf(
    Triple(PLAN_GENERATED, "Build me a plan", "Avex picks your exercises from your gear and goal"),
    Triple(PLAN_CUSTOM, "I'll make my own", "Set your goal, then build your plan from scratch"),
    Triple(PLAN_FREESTYLE, "Go with the flow", "No fixed plan. Log what you did, whenever")
)

/** A tile's supporting line as the redesign sets it: no closing full stop on a fragment. */
internal fun tileCopy(text: String): String = text.removeSuffix(".")

/** A range as the redesign sets it, with an en dash ("8–12 reps"). */
internal fun rangeCopy(text: String): String = text.replace('-', '–')

/**
 * The fork, and the first thing a new install shows. It leads because it decides the length of
 * everything after it: "build me a plan" walks the gym steps to a finished week, the other two
 * answer goal and experience (which steer the coach) and go straight to the closing step.
 *
 * Each tile carries its looping vignette video, exactly as it shipped (`PlanModeMedia.kt`); only the
 * tile around it changed in the redesign.
 */
@Composable
internal fun StepPlanMode(selected: String, onSelect: (String) -> Unit) {
    // One coordinator for the tile videos so they start — and therefore loop and freeze — together.
    val videoSync = remember { PlanModeSync(PLAN_MODE_DETAILS.count { planModeHasVideo(it.first) }) }
    // Tapping a tile plays its vignette again. The illustration IS the answer to the question, so
    // choosing an option should show you the answer rather than leave you on a frozen last frame.
    // Per-tile, because picking one must not restart the two you didn't pick.
    val replays = remember { mutableStateMapOf<String, Int>() }
    Column {
        StepHeader("How do you want to train?", "Nothing here is permanent. Change it later in Settings.")
        Spacer(Modifier.height(24.dp))
        Column(verticalArrangement = Arrangement.spacedBy(10.dp)) {
            PLAN_MODE_DETAILS.forEach { (key, label, desc) ->
                val picked = selected == key
                OnbTile(
                    selected = picked,
                    onClick = {
                        replays[key] = (replays[key] ?: 0) + 1
                        onSelect(key)
                    },
                    contentPadding = PaddingValues(start = 16.dp, top = 14.dp, end = 16.dp, bottom = 16.dp)
                ) {
                    PlanModeMedia(key, videoSync, replays[key] ?: 0)
                    Spacer(Modifier.height(14.dp))
                    // Maturity tags: the generated path is the pick; custom has shipped (no tag);
                    // freestyle is still earlier-stage but out of alpha now.
                    TileRow(
                        title = label,
                        subtitle = desc,
                        meta = when (key) {
                            PLAN_GENERATED -> "Recommended"
                            PLAN_CUSTOM -> null
                            else -> "Beta"
                        },
                        selected = picked,
                        metaInk = key == PLAN_GENERATED
                    )
                }
            }
        }
    }
}

/** A pick-one list of tiles: name, what it changes, and its mono reading. */
@Composable
private fun TileList(details: List<List<String>>, selected: String, onSelect: (String) -> Unit) {
    Column(verticalArrangement = Arrangement.spacedBy(10.dp)) {
        details.forEach { (key, label, desc, meta) ->
            OnbTile(selected = selected == key, onClick = { onSelect(key) }) {
                TileRow(label, tileCopy(desc), rangeCopy(meta), selected == key)
            }
        }
    }
}

@Composable
internal fun StepGoal(selected: String, onSelect: (String) -> Unit) {
    Column {
        StepHeader("What's your main goal?", "Same exercises, different loading. Switch it anytime.")
        Spacer(Modifier.height(28.dp))
        TileList(GOAL_DETAILS, selected, onSelect)
    }
}

@Composable
internal fun StepExperience(selected: String, onSelect: (String) -> Unit) {
    Column {
        StepHeader("How long have you been training?", "Sets your volume and which movements you're given.")
        Spacer(Modifier.height(28.dp))
        TileList(EXPERIENCE_DETAILS, selected, onSelect)
    }
}

/** A serif figure with its unit at about half size, muted ("5 days", "60 min"). */
@Composable
internal fun FigureText(
    value: String,
    unit: String?,
    style: TextStyle,
    unitStyle: TextStyle,
    modifier: Modifier = Modifier,
    dim: Boolean = false
) {
    val ink = MaterialTheme.colorScheme.onBackground
    val muted = MaterialTheme.colorScheme.onSurfaceVariant
    val text: AnnotatedString = buildAnnotatedString {
        withStyle(style.toSpanStyle().copy(color = if (dim) muted else ink)) { append(value) }
        if (unit != null) withStyle(unitStyle.toSpanStyle().copy(color = muted)) { append(" $unit") }
    }
    Text(text, modifier = modifier, style = style)
}

/**
 * Days a week and the session length, each a big serif value over a red slider with named ends, and
 * under them the week those two answers draw: one bar per training day carrying its sets, which
 * redraws as either answer moves. Both sliders and the week sit on one page so the cost of an
 * answer is seen while it is given.
 */
@Composable
internal fun StepDays(
    days: Int,
    experience: String,
    onChange: (Int) -> Unit,
    sessionMinutes: Int? = null,
    onSessionMinutes: (Int) -> Unit = {},
    archetypes: List<DayArchetype> = emptyList(),
    plannedSets: List<Int> = emptyList(),
    previewDays: List<GeneratedDay>? = null
) {
    val answered = days in 1..7
    Column {
        StepHeader("How many days a week?", "Your split follows, and it becomes your weekly target on Home.")
        Spacer(Modifier.height(28.dp))
        Row(
            Modifier.fillMaxWidth(),
            horizontalArrangement = Arrangement.spacedBy(16.dp),
        ) {
            FigureText(
                value = if (answered) "$days" else "0",
                unit = if (days == 1) "day" else "days",
                style = OnboardingType.sliderFigure,
                unitStyle = OnboardingType.sliderFigureUnit,
                dim = !answered,
                modifier = Modifier.alignByBaseline()
            )
            OpenLabel(
                // Each day's name holds together on a line ("PUSH B", never "PUSH / B").
                if (answered) archetypes.joinToString("\u00A0· ") { it.name.replace(' ', '\u00A0') } else "Slide to pick",
                modifier = Modifier.weight(1f).alignByBaseline(),
                color = if (answered) MaterialTheme.colorScheme.onBackground else MaterialTheme.colorScheme.onSurfaceVariant,
                style = OpenType.monoXS.copy(textAlign = TextAlign.End)
            )
        }
        Spacer(Modifier.height(14.dp))
        OnbSlider(
            labels = (1..7).map { "$it" },
            selectedIndex = if (answered) days - 1 else -1,
            onSelect = { onChange(it + 1) },
            valueDescription = { if (it == 0) "1 day a week" else "${it + 1} days a week" }
        )
        val advice = daysAdvice(days, experience)
        if (advice != null) {
            Spacer(Modifier.height(14.dp))
            DaysAdvice(advice)
        }

        // Optional: the CTA doesn't wait on it. The week below redraws as it changes, because the
        // answer trims each day to fit (accessories first, never the main lift).
        Spacer(Modifier.height(40.dp))
        StepSectionLabel("Session length", meta = "Optional")
        Spacer(Modifier.height(10.dp))
        val lengths = SessionEstimate.SESSION_LENGTH_CHOICES
        // null = unanswered and 0 = an explicit "Any": both mean no ceiling, so both read as Any.
        val lengthIndex = when (sessionMinutes) {
            null, 0 -> lengths.lastIndex
            else -> lengths.indexOf(sessionMinutes).takeIf { it >= 0 } ?: lengths.lastIndex
        }
        val minutes = lengths[lengthIndex]
        FigureText(
            value = minutes?.toString() ?: "Any",
            unit = if (minutes != null) "min" else "length",
            style = OnboardingType.sliderFigureS,
            unitStyle = OnboardingType.sliderFigureSUnit
        )
        Spacer(Modifier.height(12.dp))
        OnbSlider(
            labels = lengths.map { SessionEstimate.sessionLengthLabel(it) },
            selectedIndex = lengthIndex,
            onSelect = { onSessionMinutes(lengths[it] ?: 0) },
            valueDescription = { i -> lengths[i]?.let { "$it minutes" } ?: "Any length" }
        )
        Spacer(Modifier.height(12.dp))
        Text(
            "Avex trims each workout to fit, extras first.",
            style = OpenType.bodyS,
            color = MaterialTheme.colorScheme.onSurfaceVariant
        )

        if (archetypes.isNotEmpty()) {
            val sets = weekSets(archetypes, plannedSets, previewDays)
            Spacer(Modifier.height(36.dp))
            StepSectionLabel("Your week", meta = "${sets.sum()} sets")
            Spacer(Modifier.height(12.dp))
            WeekBars(
                names = archetypes.map { it.name },
                sets = sets,
                trackWidth = 36.dp,
                trackHeight = 70.dp,
                labelGap = 6.dp
            )
        }
    }
}

/** "Same days each week" vs "whenever I can" — keys for [StepWeekdays]. */
internal const val DAYS_FIXED = "fixed"
internal const val DAYS_FLEXIBLE = "flexible"

/**
 * Which weekdays the user trains on. Fixed days pin each workout to a weekday (Home shows the right
 * one on the right day); flexible keeps the old "next in line" sequence. Picking fixed pre-rings a
 * sensible spread for the day count, so most people only confirm it.
 */
@Composable
internal fun StepWeekdays(
    daysPerWeek: Int,
    fixedDays: Boolean?,
    weekdays: Set<Int>,
    onFixedDays: (Boolean) -> Unit,
    onToggleWeekday: (Int) -> Unit
) {
    Column {
        StepHeader(
            "Same days every week?",
            "Pick your days and Home shows the right workout on each one. Change it anytime in your plan."
        )
        Spacer(Modifier.height(28.dp))
        val picked = when (fixedDays) { true -> DAYS_FIXED; false -> DAYS_FLEXIBLE; null -> "" }
        Column(verticalArrangement = Arrangement.spacedBy(10.dp)) {
            listOf(
                Triple(DAYS_FIXED, "Same days each week", "Each workout gets its own weekday"),
                Triple(DAYS_FLEXIBLE, "Whenever I can", "The next workout in line, whatever the day")
            ).forEach { (key, label, desc) ->
                OnbTile(selected = picked == key, onClick = { onFixedDays(key == DAYS_FIXED) }) {
                    TileRow(label, desc, null, picked == key)
                }
            }
        }
        if (fixedDays == true) {
            Spacer(Modifier.height(32.dp))
            StepSectionLabel("Your $daysPerWeek days", meta = "${weekdays.size} of $daysPerWeek")
            Spacer(Modifier.height(12.dp))
            // A week laid out as a week: seven cells on one row, four and three past 1.3x.
            val cols = if (LocalDensity.current.fontScale > 1.3f) 4 else 7
            Column(verticalArrangement = Arrangement.spacedBy(6.dp)) {
                WeeklySchedule.WEEKDAY_SHORT.withIndex().chunked(cols).forEach { row ->
                    Row(horizontalArrangement = Arrangement.spacedBy(6.dp)) {
                        row.forEach { (i, label) ->
                            OnbTile(
                                selected = i in weekdays,
                                onClick = { onToggleWeekday(i) },
                                multi = true,
                                modifier = Modifier.weight(1f),
                                contentPadding = PaddingValues(vertical = 14.dp)
                            ) {
                                OpenLabel(
                                    label,
                                    modifier = Modifier.align(Alignment.CenterHorizontally),
                                    color = if (i in weekdays) MaterialTheme.colorScheme.onBackground
                                    else MaterialTheme.colorScheme.onSurfaceVariant,
                                    style = OpenType.monoXS
                                )
                            }
                        }
                        repeat(cols - row.size) { Spacer(Modifier.weight(1f)) }
                    }
                }
            }
        }
    }
}

/**
 * What a day-count outside 3 to 5 costs you, or null inside it. Advice, never a gate: the CTA stays
 * live, because someone with two free evenings a week still gets a real plan. Each line names what
 * that count costs (the pace, the rest days) rather than repeating one cue.
 */
internal fun daysAdvice(days: Int, experience: String): String? = when (days) {
    1 -> "One day a week maintains more than it builds. Most people progress on 3 to 5."
    2 -> "Two days works, just slower. Most people progress fastest on 3 to 5."
    6 -> if (experience == "advanced") "Six days leaves one rest day. Fine if you recover well, but 3 to 5 suits most."
        else "Six days leaves one rest day, which is hard to recover from. 3 to 5 suits most people."
    7 -> "Seven days leaves no rest day to recover on. 3 to 5 suits most people."
    else -> null
}

/** The off-range note under the day slider: a warning dot beside plain prose. */
@Composable
private fun DaysAdvice(text: String) {
    Row(Modifier.fillMaxWidth(), horizontalArrangement = Arrangement.spacedBy(10.dp)) {
        Box(
            Modifier
                .padding(top = 6.dp)
                .size(7.dp)
                .background(ForgeWarning, CircleShape)
        )
        Text(text, style = OpenType.bodyS, color = MaterialTheme.colorScheme.onBackground)
    }
}
