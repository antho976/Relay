package com.forge.app.ui.goals

import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.PaddingValues
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.lazy.LazyColumn
import androidx.compose.foundation.lazy.items
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.automirrored.filled.ArrowBack
import androidx.compose.material.icons.filled.Close
import androidx.compose.material.icons.filled.Search
import androidx.compose.material3.ExperimentalMaterial3Api
import androidx.compose.material3.Icon
import androidx.compose.material3.IconButton
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.OutlinedTextField
import androidx.compose.material3.OutlinedTextFieldDefaults
import androidx.compose.material3.Scaffold
import androidx.compose.material3.Text
import androidx.compose.material3.TopAppBar
import androidx.compose.material3.TopAppBarDefaults
import androidx.compose.runtime.Composable
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.saveable.rememberSaveable
import androidx.compose.runtime.setValue
import androidx.compose.ui.Modifier
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.unit.dp
import androidx.hilt.navigation.compose.hiltViewModel
import androidx.lifecycle.compose.collectAsStateWithLifecycle
import com.forge.app.data.repo.ExtendedGoalRepository
import com.forge.app.data.repo.GoalRepository
import com.forge.app.ui.common.ForgePrimaryCapsule
import com.forge.app.ui.common.InlineEmptyHint
import com.forge.app.ui.common.SegmentPill
import com.forge.app.ui.common.forgeItemMotion

/**
 * The aggregated Goals list. Adding and editing happen on the routed [GoalEditorScreen] (a full
 * screen, not a dialog) — this list refreshes itself reactively when you come back from it.
 *
 * ## One ladder, split by state rather than by table (2026-08-23)
 *
 * The screen used to open with a thirty-word explainer and then split its rows under two headers,
 * "Lift targets" and "Other goals". That split is the database's shape, not the user's: the two
 * tables behind it are an implementation detail, and no question a person brings to this screen is
 * answered by which one a goal lives in. What they arrive wanting is the ladder — what is close,
 * what is stalled, what is done — and that ordering was thrown away twice over, once by grouping and
 * again by the screen never re-sorting the rows it filtered.
 *
 * So the sections are gone and the rows are one ranked list, closest-first. What splits them now is
 * the [GoalLens] pills, and the reason is worth stating: a finished goal used to announce itself by
 * putting the word REACHED where its numbers belonged, on every one of its rows. One lens says it
 * once, for all of them, which is §12's collapse-repetition rule applied to a state word.
 *
 * With both kinds of goal interleaved, the rows also carry their glyphs here for the first time —
 * see [GoalProgressLine]. A column of identical marks says nothing, which is why they were off; a
 * column of *different* marks is the fastest read on the screen.
 */
@OptIn(ExperimentalMaterial3Api::class)
@Composable
fun GoalsScreen(
    onBack: () -> Unit,
    onAddGoal: () -> Unit,
    onEditLift: (exerciseId: String) -> Unit,
    onEditCustom: (goalId: Long) -> Unit,
    viewModel: GoalsViewModel = hiltViewModel()
) {
    val state by viewModel.state.collectAsStateWithLifecycle()
    var query by rememberSaveable { mutableStateOf("") }
    // Null until the user picks one, so the screen can open on whichever lens actually has rows
    // without ever overriding a choice they made (see [lens] below).
    var chosenLens by rememberSaveable { mutableStateOf<GoalLens?>(null) }

    val onBg = MaterialTheme.colorScheme.onBackground
    val muted = MaterialTheme.colorScheme.onSurfaceVariant
    val accent = MaterialTheme.colorScheme.primary
    val outline = MaterialTheme.colorScheme.outline

    val all = remember(state.liftGoals, state.customGoals) {
        state.liftGoals.map(GoalRow::Lift) + state.customGoals.map(GoalRow::Custom)
    }
    val liveCount = remember(all) { all.count { !it.achieved } }
    val reachedCount = all.size - liveCount

    // Search only earns its space once the list is long enough to need finding rather than reading.
    val searchVisible = all.size >= SEARCH_THRESHOLD
    val q = if (searchVisible) query.trim() else ""

    // Both lenses have to be populated for the pills to mean anything; a toggle where one side is
    // always empty is a control that can't run (§2③).
    val lensVisible = liveCount > 0 && reachedCount > 0
    val lens = chosenLens
        ?: if (liveCount == 0 && reachedCount > 0) GoalLens.REACHED else GoalLens.LIVE

    val rows = remember(all, lens, q) {
        all.asSequence()
            .filter { it.achieved == (lens == GoalLens.REACHED) }
            .filter { q.isBlank() || it.matches(q) }
            // Live: closest to done first, which is the order a person reads this screen in.
            // Reached: newest first, because a finished goal's only remaining question is when.
            .sortedWith(
                if (lens == GoalLens.REACHED) compareByDescending { it.createdAt }
                else compareByDescending { it.fraction }
            )
            .toList()
    }

    Scaffold(
        topBar = {
            TopAppBar(
                // §4.6: the top bar never names the screen — back alone; the content title does.
                title = {},
                navigationIcon = {
                    IconButton(onClick = onBack) { Icon(Icons.AutoMirrored.Filled.ArrowBack, "Back") }
                },
                colors = TopAppBarDefaults.topAppBarColors(containerColor = Color.Transparent)
            )
        },
        containerColor = Color.Transparent
    ) { inner ->
        // No spinners (§13): the local DB is instant, the list simply appears when state lands.
        if (state.loading) {
            Box(Modifier.fillMaxSize().padding(inner))
            return@Scaffold
        }
        LazyColumn(
            Modifier.fillMaxSize().padding(inner),
            contentPadding = PaddingValues(horizontal = 24.dp)
        ) {
            item(key = "hero") {
                // §3 List archetype: a small hero — a title and its count, no serif figure row and
                // no explainer. It runs one step above the recipe's `headlineSmall` (Antho,
                // 2026-08-23: *"not just the add a goal button, everything"*); a list of six goals
                // left two thirds of the page empty while every mark on it sat at the smallest rung
                // it had. Still not a display hero, and still no figure row. The thirty words that used to sit here narrated
                // how the feature works ("custom goals track themselves from what you log"), which
                // §4.3 cuts rather than trims: the rows filling in by themselves ARE that sentence.
                Spacer(Modifier.height(8.dp))
                Text("Goals", style = MaterialTheme.typography.headlineMedium, color = onBg)
                Spacer(Modifier.height(4.dp))
                Text(
                    goalCountLine(liveCount, reachedCount),
                    style = MaterialTheme.typography.labelLarge,
                    color = muted
                )
                Spacer(Modifier.height(24.dp))
            }

            if (lensVisible) {
                item(key = "lens") {
                    Row(horizontalArrangement = Arrangement.spacedBy(8.dp)) {
                        GoalLens.entries.forEach { l ->
                            SegmentPill(
                                text = l.label,
                                selected = l == lens,
                                onClick = { chosenLens = l },
                                accent = accent, onBg = onBg, muted = muted, outline = outline
                            )
                        }
                    }
                    Spacer(Modifier.height(16.dp))
                }
            }

            if (searchVisible) {
                item(key = "search") {
                    GoalSearchField(query, { query = it }, muted, accent, outline, onBg)
                    Spacer(Modifier.height(16.dp))
                }
            }

            if (rows.isEmpty()) {
                item(key = "empty") {
                    // §12: last resort, ≤1 per lens, replacing the caption rather than joining it.
                    // None of these three has a zero SHAPE to draw — there is no mark for a goal
                    // that does not exist, and the count line above is already the honest figure.
                    InlineEmptyHint(
                        when {
                            all.isEmpty() -> "Nothing tracked yet"
                            q.isNotBlank() -> "No goals match that"
                            lens == GoalLens.REACHED -> "Nothing reached yet"
                            else -> "Every goal is done"
                        },
                        muted.copy(alpha = 0.65f)
                    )
                    Spacer(Modifier.height(8.dp))
                }
            } else {
                items(rows, key = { it.key }) { row ->
                    // §9: lists get a LIGHT stagger only — never the overview's entrance cascade.
                    when (row) {
                        is GoalRow.Lift -> LiftGoalRow(
                            row.g, onBg, muted, accent, outline, forgeItemMotion()
                        ) { onEditLift(row.g.exerciseId) }
                        is GoalRow.Custom -> CustomGoalRow(
                            row.g, onBg, muted, accent, outline, forgeItemMotion()
                        ) { onEditCustom(row.g.id) }
                    }
                    Spacer(Modifier.height(24.dp))
                }
            }

            item(key = "add") {
                Spacer(Modifier.height(8.dp))
                // §8 level ① at its STANDARD trim size — no `fillMaxWidth()`, so the capsule hugs
                // its label at ~44dp and sits on the same left rail as the rows above it.
                //
                // It was the mono accent `+ add goal` line, which was too quiet to be the only thing
                // a person can DO on this screen. Two overshoots got it here and both are recorded
                // in `design/SETTLED.md`: a corner cube (smaller and pinned to an edge — the
                // complaint restated), then a full-width bar, which outweighed the ladder once the
                // rows themselves grew. The trim capsule is the level that was wanted all along; it
                // only reads as enough now because everything above it got bigger too.
                ForgePrimaryCapsule("Add a goal", onClick = onAddGoal)
                Spacer(Modifier.height(40.dp))
            }
        }
    }
}

/** Below this many goals the list is read, not searched, and the field is just a row of chrome. */
private const val SEARCH_THRESHOLD = 4

/**
 * The two states a goal can be in, as the screen's lens pills (§4.4 — labels are ONE short word).
 * This is the only place the word "reached" is said on a populated screen; the rows themselves keep
 * their numbers.
 */
private enum class GoalLens(val label: String) { LIVE("Live"), REACHED("Reached") }

/** The tiny hero's count line. Honest at zero (§12) rather than hidden. */
private fun goalCountLine(live: Int, reached: Int): String = when {
    live == 0 && reached == 0 -> "NO GOALS YET"
    reached == 0 -> "$live LIVE"
    live == 0 -> "$reached REACHED"
    else -> "$live LIVE · $reached REACHED"
}

/**
 * One row of the ladder, whichever table it came from. The screen sorts and filters across both
 * kinds, so they need one shape to be compared in — the alternative is the kind-grouped list this
 * replaced, which sorted neither.
 */
private sealed interface GoalRow {
    val key: String
    val achieved: Boolean
    val fraction: Float
    val createdAt: Long
    fun matches(q: String): Boolean

    data class Lift(val g: GoalRepository.GoalProgress) : GoalRow {
        override val key get() = liftPinKey(g.exerciseId)
        override val achieved get() = g.achieved
        override val fraction get() = g.fraction
        override val createdAt get() = g.createdAt
        override fun matches(q: String) = g.name.contains(q, ignoreCase = true)
    }

    data class Custom(val g: ExtendedGoalRepository.Progress) : GoalRow {
        override val key get() = customPinKey(g.id)
        override val achieved get() = g.achieved
        override val fraction get() = g.fraction
        override val createdAt get() = g.createdAt
        // The generated title already contains the metric name, but a user-named goal replaces it —
        // so search both, or renaming a goal would hide it from a search for what it tracks.
        override fun matches(q: String) =
            g.label.contains(q, ignoreCase = true) || metricDisplayName(g.metric).contains(q, ignoreCase = true)
    }
}

/**
 * §13's search treatment: bordered because it is interactive, leading magnifier, trailing clear, and
 * a placeholder allowed below the muted floor because it is a ghost affordance rather than content.
 * Shaped to match History's field so the app's two searches read as one control.
 */
@OptIn(ExperimentalMaterial3Api::class)
@Composable
private fun GoalSearchField(
    query: String,
    onQueryChange: (String) -> Unit,
    muted: Color,
    accent: Color,
    outline: Color,
    onBg: Color
) {
    OutlinedTextField(
        value = query,
        onValueChange = onQueryChange,
        modifier = Modifier.fillMaxWidth(),
        placeholder = { Text("Search goals", color = muted.copy(alpha = 0.5f)) },
        leadingIcon = { Icon(Icons.Filled.Search, contentDescription = null, tint = muted) },
        trailingIcon = {
            if (query.isNotEmpty()) {
                IconButton(onClick = { onQueryChange("") }) {
                    Icon(Icons.Filled.Close, contentDescription = "Clear search", tint = muted)
                }
            }
        },
        singleLine = true,
        shape = RoundedCornerShape(12.dp),
        colors = OutlinedTextFieldDefaults.colors(
            focusedBorderColor = accent,
            unfocusedBorderColor = outline.copy(alpha = 0.35f),
            focusedTextColor = onBg,
            unfocusedTextColor = onBg,
        )
    )
}
