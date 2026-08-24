package com.forge.app.ui.gym.train.components

import androidx.compose.foundation.background
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.ColumnScope
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.rememberScrollState
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.foundation.verticalScroll
import androidx.compose.material3.ExperimentalMaterial3Api
import androidx.compose.material3.Icon
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.ModalBottomSheet
import androidx.compose.material3.Text
import androidx.compose.material3.rememberModalBottomSheetState
import androidx.compose.runtime.Composable
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.saveable.rememberSaveable
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.clip
import androidx.compose.ui.semantics.Role
import androidx.compose.ui.semantics.role
import androidx.compose.ui.semantics.selected
import androidx.compose.ui.semantics.semantics
import androidx.compose.ui.unit.dp
import com.forge.app.program.ExerciseDef
import com.forge.app.program.ExercisePlan
import com.forge.app.ui.common.EditorialHeader
import com.forge.app.ui.common.EntranceItem
import com.forge.app.ui.common.ExerciseIcons
import com.forge.app.ui.common.ForgeHeroAction
import com.forge.app.ui.common.ForgeOutlineCapsule
import com.forge.app.ui.common.InlineEmptyHint
import com.forge.app.ui.common.SegmentPill
import com.forge.app.ui.common.bounceCombinedClick

/** §7 — one vertical padding for every row in this sheet, so the list reads as one rhythm. */
private val SWAP_ROW_PAD = 10.dp

/** How far a pick reaches. Chosen per candidate, then spent by the one confirm. */
private enum class SwapScope(val label: String) { TODAY("Today"), ALWAYS("Always") }

/**
 * Modal sheet listing the swap [candidates] for [forExercise]'s muscle group, drawn from the single
 * [com.forge.app.program.ExerciseLibrary] pool (already filtered by equipment + dislikes).
 *
 * DESIGN §3 Modal over §3 List: surface fill, 24dp gutter, trim rows, no hero. **Select, then
 * confirm** (Antho, 2026-08-24): each candidate carries its own `Today` / `Always` pair, and one
 * accent [ForgeHeroAction] pinned under the list commits the armed pick. The first candidate opens
 * with `Today` already lit, so the mechanism is visible the moment the sheet arrives rather than
 * waiting to be discovered.
 *
 * This is a deliberate departure from §8's "per-row action = one drawn outlined pill, never a button
 * per row" — see `design/SETTLED.md`. That rule exists because filled capsules per row stack into a
 * button wall; what keeps this the right side of it is that the row pair is a SELECTION (the §3 pill
 * formula, borders only, nothing filled) and the sheet still has exactly ONE filled action, at the end.
 *
 * Rows carry the library's `muscleTarget` and nothing else. `why` / `whenToUse` ran two to four
 * sentences per candidate, which turned five alternatives into a prose wall no one reads between
 * sets; §4.2 cuts prose rather than folding it behind a tap.
 *
 * [currentSwapName] opens armed on the swap already in force. [hasPersistentSwap] reveals the way
 * back to the programmed exercise, which the old sheet accepted as a parameter and then never drew.
 */
@OptIn(ExperimentalMaterial3Api::class)
@Composable
fun SwapPickerSheet(
    forExercise: ExercisePlan,
    candidates: List<ExerciseDef>,
    hasPersistentSwap: Boolean,
    currentSwapName: String? = null,
    onPickForSession: (ExerciseDef) -> Unit,
    onPickPersistent: (ExerciseDef) -> Unit,
    onClearPersistent: () -> Unit,
    onDismiss: () -> Unit
) {
    ModalBottomSheet(
        onDismissRequest = onDismiss,
        sheetState = rememberModalBottomSheetState(skipPartiallyExpanded = true),
        containerColor = MaterialTheme.colorScheme.surface
    ) {
        SwapPickerContent(
            forExercise = forExercise,
            candidates = candidates,
            hasPersistentSwap = hasPersistentSwap,
            currentSwapName = currentSwapName,
            onPickForSession = onPickForSession,
            onPickPersistent = onPickPersistent,
            onClearPersistent = onClearPersistent
        )
    }
}

/**
 * The sheet's body, split out so it renders on its own in a preview and a golden. The sheet chrome
 * adds nothing you need to design (`ui/recipes/ModalRecipe.kt`).
 */
@Composable
internal fun ColumnScope.SwapPickerContent(
    forExercise: ExercisePlan,
    candidates: List<ExerciseDef>,
    hasPersistentSwap: Boolean,
    currentSwapName: String? = null,
    onPickForSession: (ExerciseDef) -> Unit = {},
    onPickPersistent: (ExerciseDef) -> Unit = {},
    onClearPersistent: () -> Unit = {}
) {
    val onBg = MaterialTheme.colorScheme.onBackground
    val muted = MaterialTheme.colorScheme.onSurfaceVariant
    val accent = MaterialTheme.colorScheme.primary
    val outline = MaterialTheme.colorScheme.outline

    // Opening armed is the point: a sheet of inert rows gives no clue the pills are yours to press.
    // It lands on the swap already in force when there is one, otherwise the first candidate, and on
    // ALWAYS only when that live swap is the persistent kind — so the lit pill always tells the
    // truth about what is set right now.
    val opening = candidates.firstOrNull { it.name == currentSwapName } ?: candidates.firstOrNull()
    var pickedId by rememberSaveable(opening?.id) { mutableStateOf(opening?.id) }
    var scope by rememberSaveable(opening?.id) {
        mutableStateOf(
            if (currentSwapName != null && hasPersistentSwap) SwapScope.ALWAYS else SwapScope.TODAY
        )
    }
    val picked = candidates.firstOrNull { it.id == pickedId }

    Column(
        modifier = Modifier
            .fillMaxWidth()
            .padding(horizontal = 24.dp)
    ) {
        EditorialHeader("SWAP · ${forExercise.muscle.displayName}", muted, accent)

        if (candidates.isEmpty()) {
            // §12 zero: name what would unlock the list. The count caption is suppressed rather
            // than printed as "0 alternatives" — the hint already carries the fact, and saying it
            // twice is the §4.3 one-home rule broken over two lines.
            Spacer(Modifier.height(8.dp))
            InlineEmptyHint(
                "Your equipment covers one ${forExercise.muscle.displayName.lowercase()} " +
                    "movement. Add equipment in Settings to open up the rest.",
                muted
            )
            Spacer(Modifier.height(32.dp))
        } else {
            Spacer(Modifier.height(2.dp))
            Text(
                // Singular gets its own string — a paren-plural is machine prose (§11).
                if (candidates.size == 1) "One alternative to ${forExercise.name}."
                else "${candidates.size} alternatives to ${forExercise.name}.",
                style = MaterialTheme.typography.bodySmall,
                color = muted
            )
            Spacer(Modifier.height(2.dp))
            Text(
                // §13 — the one explainer this sheet earns, beside the controls it explains.
                "Today swaps this session. Always replaces it in your program.",
                style = MaterialTheme.typography.bodySmall,
                color = muted
            )
        }
    }

    if (candidates.isEmpty()) return

    // The list scrolls; the confirm does not. Eleven alternatives would otherwise bury the action
    // that spends them under a full screen of scrolling.
    Column(
        modifier = Modifier
            .weight(1f, fill = false)
            .verticalScroll(rememberScrollState())
            .padding(horizontal = 24.dp)
            .padding(top = 14.dp, bottom = 4.dp)
    ) {
        candidates.forEachIndexed { index, swap ->
            EntranceItem(index) {
                SwapRow(
                    swap = swap,
                    isPicked = swap.id == pickedId,
                    scope = scope,
                    onArm = { armed ->
                        pickedId = swap.id
                        scope = armed
                    }
                )
            }
        }
    }

    // §8 — one filled action, at the END. Accent-filled rather than the light `ForgePrimaryCapsule`
    // (Antho, 2026-08-24): the sheet's whole job is this one commit, so it reads as the thing to
    // press. `ForgeHeroAction` takes `onPrimary` for its label, so a light or monochrome accent
    // never renders same-on-same (§5).
    Column(modifier = Modifier.fillMaxWidth().padding(24.dp)) {
        ForgeHeroAction(
            text = picked?.let {
                when (scope) {
                    SwapScope.TODAY -> "Swap to ${it.name}"
                    SwapScope.ALWAYS -> "Always use ${it.name}"
                }
            } ?: "Pick an alternative",
            onClick = {
                val swap = picked ?: return@ForgeHeroAction
                when (scope) {
                    SwapScope.TODAY -> onPickForSession(swap)
                    SwapScope.ALWAYS -> onPickPersistent(swap)
                }
            },
            modifier = Modifier.fillMaxWidth()
        )
        if (hasPersistentSwap) {
            Spacer(Modifier.height(10.dp))
            ForgeOutlineCapsule(
                "Restore ${forExercise.name}",
                onClick = onClearPersistent,
                modifier = Modifier.fillMaxWidth()
            )
        }
    }
}

/**
 * One alternative and the two reaches it can be taken with. The pills are a SELECTION, not two
 * actions — they arm the sheet's single confirm — so they take the §3 pill formula and nothing on
 * the row is filled. Tapping anywhere else on the row arms it at the scope already chosen, which
 * keeps a full-width target for the common case.
 */
@Composable
private fun SwapRow(
    swap: ExerciseDef,
    isPicked: Boolean,
    scope: SwapScope,
    onArm: (SwapScope) -> Unit
) {
    val onBg = MaterialTheme.colorScheme.onBackground
    val muted = MaterialTheme.colorScheme.onSurfaceVariant
    val accent = MaterialTheme.colorScheme.primary
    val outline = MaterialTheme.colorScheme.outline

    Row(
        modifier = Modifier
            .fillMaxWidth()
            .clip(RoundedCornerShape(12.dp))
            // The armed row takes the active-row wash (§5's 0.15 rung) and no border — the lit pill
            // inside it already carries one, and two would read as a box inside a box.
            .then(if (isPicked) Modifier.background(accent.copy(alpha = 0.15f)) else Modifier)
            .bounceCombinedClick(
                onClickLabel = "Choose ${swap.name}",
                onClick = { onArm(scope) }
            )
            .semantics { role = Role.RadioButton; selected = isPicked }
            .padding(horizontal = 10.dp, vertical = SWAP_ROW_PAD),
        // Top, not centre: at 200% font scale the meta line wraps and a centred glyph drifts down
        // to the second line, away from the name it labels (§14).
        verticalAlignment = Alignment.Top,
        horizontalArrangement = Arrangement.spacedBy(10.dp)
    ) {
        // §8 — exercise rows in a picker lead with their equipment-class glyph.
        Icon(
            ExerciseIcons.forEquipment(swap.equipment),
            contentDescription = null,
            tint = muted,
            // Optical alignment with the name's cap height, which sits below the line box top.
            modifier = Modifier.padding(top = 2.dp).size(20.dp)
        )
        Column(modifier = Modifier.weight(1f)) {
            Text(swap.name, style = MaterialTheme.typography.bodyLarge, color = onBg)
            Spacer(Modifier.height(2.dp))
            Text(
                swap.muscleTarget ?: swap.muscle.displayName,
                style = MaterialTheme.typography.bodySmall,
                color = muted
            )
            Spacer(Modifier.height(8.dp))
            Row(horizontalArrangement = Arrangement.spacedBy(6.dp)) {
                SwapScope.entries.forEach { option ->
                    SegmentPill(
                        text = option.label,
                        selected = isPicked && scope == option,
                        onClick = { onArm(option) },
                        accent = accent, onBg = onBg, muted = muted, outline = outline
                    )
                }
            }
        }
    }
}
