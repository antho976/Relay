package com.forge.app.ui.gym.train.components

import androidx.compose.foundation.background
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.ExperimentalLayoutApi
import androidx.compose.foundation.layout.FlowRow
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.navigationBarsPadding
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.lazy.LazyColumn
import androidx.compose.foundation.lazy.itemsIndexed
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.material3.ExperimentalMaterial3Api
import androidx.compose.material3.Icon
import androidx.compose.material3.MaterialTheme
import com.forge.app.ui.common.window.ModalBottomSheet
import androidx.compose.material3.Text
import androidx.compose.material3.rememberModalBottomSheetState
import androidx.compose.runtime.Composable
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.clip
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.text.font.FontStyle
import androidx.compose.ui.unit.dp
import com.forge.app.program.ExerciseDef
import com.forge.app.program.ExercisePlan
import com.forge.app.ui.common.EditorialHeader
import com.forge.app.ui.common.ExerciseIcons
import androidx.compose.foundation.layout.Box
import androidx.compose.ui.semantics.Role
import androidx.compose.ui.semantics.role
import androidx.compose.ui.semantics.selected
import androidx.compose.ui.semantics.semantics
import com.forge.app.ui.common.Corners
import com.forge.app.ui.common.ForgeGlyphBadge
import com.forge.app.ui.common.ForgePrimaryCapsule
import com.forge.app.ui.common.ForgeSecondaryCapsule
import com.forge.app.ui.common.ForgeSlidingSegments
import com.forge.app.ui.common.GROUP_SEAM
import com.forge.app.ui.common.InlineEmptyHint
import com.forge.app.ui.common.ROW_H
import com.forge.app.ui.common.bounceClick
import com.forge.app.ui.common.memberFill
import com.forge.app.ui.common.memberShape

/**
 * Swap picker — the Modal archetype (DESIGN §3) hosting a List: one mono anchor, trim rows, and the
 * transient detail of the ONE exercise you tapped.
 *
 * **One scope control, one pick, one commit** (grouped-surface pass, 2026-09-27). The `Today` /
 * `Every week` choice used to repeat on every row as a pair of pills; it is a single
 * [ForgeSlidingSegments] at the top now, and the candidates are one group of filled members where
 * the whole row is the (radio) tap target, so there are no nested taps. Nothing commits until the
 * confirm. Selection starts on the LEAD candidate under `Today` (the library is ordered best-first),
 * so the likeliest swap is one tap from done and the commit is on screen from the moment the sheet
 * opens.
 *
 * [candidates] arrive best-first out of [com.forge.app.program.ExerciseLibrary], so the lead entry
 * carries the sheet's ONE caption: its situational guidance, drawn inside the row it belongs to.
 * [hasPersistentSwap] surfaces the restore action, the only route back out of a persistent swap.
 *
 * [currentSwapName] names the move being replaced in the anchor. It is deliberately NOT used to mark
 * a row: `DayScreen` filters the day's own effective names out of [candidates], so the active swap is
 * never in this list and a "current" mark here could never fire. The accent ring means PICKED.
 *
 * No search field: the pool is already scoped to one muscle and the user's own equipment (4–18 moves
 * before dislikes and same-day exclusions), and this opens one-handed mid-set. A keyboard over six
 * rows costs more than it finds.
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
    val sheetState = rememberModalBottomSheetState(skipPartiallyExpanded = true)

    ModalBottomSheet(
        onDismissRequest = onDismiss,
        sheetState = sheetState,
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

/** Which scope an armed row is waiting to commit under. */
internal enum class SwapScope { TODAY, EVERY_WEEK }

/**
 * The sheet's body, separated from its chrome so it renders on its own — the sheet frame adds
 * nothing you need to design, and `SwapPickerScreenshotTest` pins this at 100% and 200% font scale
 * where a regex can't reach (§14).
 */
@Composable
internal fun SwapPickerContent(
    forExercise: ExercisePlan,
    candidates: List<ExerciseDef>,
    hasPersistentSwap: Boolean,
    currentSwapName: String?,
    onPickForSession: (ExerciseDef) -> Unit,
    onPickPersistent: (ExerciseDef) -> Unit,
    onClearPersistent: () -> Unit,
    /** Preview/test seam: overrides which row opens picked, and under which scope. Null = the real
     *  default, the lead candidate under `Today`. */
    initialArmed: Pair<String, SwapScope>? = null
) {
    val cs = MaterialTheme.colorScheme
    val muted = cs.onSurfaceVariant
    val accent = cs.primary

    // The scope is one choice for the whole sheet, not one per row.
    var scope by remember { mutableStateOf(initialArmed?.second ?: SwapScope.TODAY) }
    // The library id the confirm capsule will commit. Re-keyed on `candidates` so a refreshed pool
    // re-seeds rather than stranding a row that is no longer there.
    var pickedId by remember(candidates) {
        mutableStateOf(initialArmed?.first ?: candidates.firstOrNull()?.id)
    }
    val pickedDef = candidates.firstOrNull { it.id == pickedId }

    Column(
        Modifier
            .fillMaxWidth()
            .navigationBarsPadding()
            // §7: the sheet gutter, plus enough top air that the anchor clears the sheet's drag
            // handle instead of sitting on it.
            .padding(horizontal = 16.dp, vertical = 8.dp)
    ) {
        // §4.6 / ModalRecipe: one mono anchor naming what the sheet is about. It names the move
        // being replaced rather than adding a serif title over the card it already covers.
        Box(Modifier.padding(horizontal = 8.dp)) {
            EditorialHeader("SWAP · ${(currentSwapName ?: forExercise.name).uppercase()}", muted, accent)
        }
        Spacer(Modifier.height(10.dp))
        // §4.3: the sheet's one caption, and the only place the two scopes are explained.
        Text(
            "Today swaps this session. Every week replaces it in your plan.",
            style = MaterialTheme.typography.bodySmall,
            color = muted.copy(alpha = 0.65f),
            modifier = Modifier.padding(horizontal = 8.dp)
        )
        Spacer(Modifier.height(14.dp))
        ForgeSlidingSegments(
            options = listOf("Today", "Every week"),
            selectedIndex = if (scope == SwapScope.TODAY) 0 else 1,
            onSelect = { scope = if (it == 0) SwapScope.TODAY else SwapScope.EVERY_WEEK },
            modifier = Modifier.fillMaxWidth()
        )

        Spacer(Modifier.height(16.dp))

        if (candidates.isEmpty()) {
            // §12: a pool with nothing in it has no zero-SHAPE to draw, which is the last-resort
            // case InlineEmptyHint exists for. It names the concrete unlock, not the absence.
            InlineEmptyHint(
                "No other ${forExercise.muscle.displayName.lowercase()} moves your equipment can do. " +
                    "Add gear in Settings to widen the pool.",
                muted.copy(alpha = 0.65f),
                modifier = Modifier.padding(horizontal = 8.dp)
            )
        } else {
            LazyColumn(
                Modifier.fillMaxWidth().weight(1f, fill = false),
                verticalArrangement = Arrangement.spacedBy(GROUP_SEAM)
            ) {
                itemsIndexed(candidates, key = { _, def -> def.id }) { index, def ->
                    SwapRow(
                        def = def,
                        picked = def.id == pickedId,
                        corners = Corners.ofRow(index, candidates.size),
                        // Best-first ordering earns the lead row the sheet's ONE caption (§4.3).
                        caption = if (index == 0) def.whenToUse ?: def.why else null,
                        // Radio-style: one row is always picked, so re-tapping it is a no-op rather
                        // than leaving the sheet with no confirm to press.
                        onPick = { pickedId = def.id }
                    )
                }
            }
        }

        // §8: actions at the END — ① the confirm, ② its sidekick. The confirm is absent rather than
        // dimmed until something is picked, because nothing renders as an affordance while it
        // cannot run. No enter animation: §9 keeps presentational motion out of working surfaces.
        if (pickedDef != null) {
            Spacer(Modifier.height(16.dp))
            ForgePrimaryCapsule(
                // §11: names the move AND the scope, so the confirm restates the decision instead
                // of asking "are you sure" about an unnamed one.
                when (scope) {
                    SwapScope.TODAY -> "Swap to ${pickedDef.name} for today"
                    SwapScope.EVERY_WEEK -> "Swap to ${pickedDef.name} every week"
                },
                onClick = {
                    val commitScope = scope
                    pickedId = null
                    when (commitScope) {
                        SwapScope.TODAY -> onPickForSession(pickedDef)
                        SwapScope.EVERY_WEEK -> onPickPersistent(pickedDef)
                    }
                },
                modifier = Modifier.fillMaxWidth(),
                // Accent-filled: a modal's one commit has to out-rank the accent-ringed pick above
                // it (§8, 2026-08-24).
                accent = true
            )
        }

        // Reverting is the sidekick to picking: a filled secondary capsule. Names its referent
        // (§11), and is the only route back out of a persistent swap.
        if (hasPersistentSwap) {
            Spacer(Modifier.height(if (pickedDef != null) 8.dp else 16.dp))
            ForgeSecondaryCapsule(
                "Back to ${forExercise.name}",
                onClick = onClearPersistent,
                modifier = Modifier.fillMaxWidth()
            )
        }
        Spacer(Modifier.height(24.dp))
    }
}

/**
 * One candidate, a member of the candidate group: equipment glyph on its badge, name, what it
 * actually hits. The whole row picks it. [caption] is the lead entry's situational guidance, drawn
 * inside the row it describes.
 */
@Composable
private fun SwapRow(
    def: ExerciseDef,
    picked: Boolean,
    corners: Corners,
    caption: String?,
    onPick: () -> Unit
) {
    val cs = MaterialTheme.colorScheme
    val muted = cs.onSurfaceVariant
    val shape = memberShape(corners, picked)

    Row(
        Modifier
            .fillMaxWidth()
            .memberFill(shape, picked)
            .bounceClick(onClick = onPick)
            .semantics { selected = picked; role = Role.RadioButton }
            .padding(horizontal = ROW_H, vertical = 14.dp),
        verticalAlignment = Alignment.Top,
        horizontalArrangement = Arrangement.spacedBy(14.dp)
    ) {
        // §8: pickers lead with the ExerciseIcons equipment-class glyph. Decorative to TalkBack —
        // the row's text carries the movement (§14).
        ForgeGlyphBadge(ExerciseIcons.forEquipment(def.equipment), selected = picked, size = 34.dp)
        Column(Modifier.weight(1f).padding(top = 2.dp)) {
            Row(verticalAlignment = Alignment.Top, horizontalArrangement = Arrangement.spacedBy(8.dp)) {
                // §14: no maxLines on a movement name — a long one wraps rather than truncating.
                Text(
                    def.name,
                    style = MaterialTheme.typography.bodyLarge,
                    color = cs.onSurface,
                    modifier = Modifier.weight(1f)
                )
                // §8: flag only the exception. A timed hold swaps the set logger's REPS column for a
                // m:ss field, which is the one swap consequence the name and glyph don't carry.
                if (def.timed) {
                    Text("HOLD", style = MaterialTheme.typography.labelSmall, color = muted)
                }
            }
            // Every candidate shares this row's muscle group, so the differentiator is what the
            // movement specifically hits — never the group name repeated down the list (§4.3).
            def.muscleTarget?.let { target ->
                Spacer(Modifier.height(2.dp))
                Text(target, style = MaterialTheme.typography.bodySmall, color = muted)
            }
            caption?.let { guidance ->
                Spacer(Modifier.height(8.dp))
                Text(
                    guidance,
                    style = MaterialTheme.typography.bodySmall,
                    color = muted.copy(alpha = 0.7f),
                    fontStyle = FontStyle.Italic
                )
            }
        }
    }
}
