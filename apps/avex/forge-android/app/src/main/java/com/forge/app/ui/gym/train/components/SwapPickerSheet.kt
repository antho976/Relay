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
import androidx.compose.material3.ModalBottomSheet
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
import com.forge.app.ui.common.ForgeOutlineCapsule
import com.forge.app.ui.common.ForgePrimaryCapsule
import com.forge.app.ui.common.InlineEmptyHint
import com.forge.app.ui.common.SegmentPill

/**
 * Swap picker — the Modal archetype (DESIGN §3) hosting a List: one mono anchor, trim rows, and the
 * transient detail of the ONE exercise you tapped.
 *
 * **Every row carries its own `Today` / `Every week` choice, and nothing commits until you confirm.**
 * Selection is radio-style and starts on the LEAD candidate's `Today` (the library is ordered
 * best-first), so the likeliest swap is one tap from done and the accent-filled commit is on screen
 * from the moment the sheet opens. Arming a row is a selection, not an action, so both per-row
 * controls are [SegmentPill]s rather than buttons — which is what keeps a two-control row off the *button wall* in `FAILURES.md` (that names
 * a FILLED capsule per row, and there is exactly one filled capsule here, at the END per §8). The row
 * itself is not clickable, so the two pills are siblings rather than taps nested inside a third
 * (§2③). The previous build put a filled + outlined capsule pair in every row and fired on the first
 * touch, which both stacked the wall and made a mis-tap a committed write.
 *
 * [candidates] arrive best-first out of [com.forge.app.program.ExerciseLibrary], so the lead entry
 * carries the sheet's ONE caption: its situational guidance, drawn inside the row it belongs to.
 * [hasPersistentSwap] surfaces the restore action, which an earlier sheet accepted as a parameter and
 * then never drew, leaving `onClearPersistent` unreachable.
 *
 * [currentSwapName] names the move being replaced in the anchor. It is deliberately NOT used to mark
 * a row: `DayScreen` filters the day's own effective names out of [candidates], so the active swap is
 * never in this list and a "current" mark here could never fire. The accent wash means ARMED.
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
    /** Preview/test seam: overrides which row opens armed. Null = the real default, the lead
     *  candidate's `Today`. */
    initialArmed: Pair<String, SwapScope>? = null
) {
    val cs = MaterialTheme.colorScheme
    val onBg = cs.onBackground
    val muted = cs.onSurfaceVariant
    val accent = cs.primary
    val outline = cs.outline

    // The (library id, scope) the confirm capsule will commit. The sheet OPENS with the lead
    // candidate's "Today" already selected — the library is ordered best-first, so the most likely
    // swap is one tap away and the confirm is never a control you have to go find. Re-keyed on
    // `candidates` so a refreshed pool re-seeds rather than stranding a row that is no longer there.
    var armed by remember(candidates) {
        mutableStateOf(initialArmed ?: candidates.firstOrNull()?.let { it.id to SwapScope.TODAY })
    }
    val armedDef = candidates.firstOrNull { it.id == armed?.first }

    Column(
        Modifier
            .fillMaxWidth()
            .navigationBarsPadding()
            // §7: the 24dp page gutter, plus enough top air that the anchor clears the sheet's
            // drag handle instead of sitting on it.
            .padding(horizontal = 24.dp, vertical = 8.dp)
    ) {
        // §4.6 / ModalRecipe: one mono anchor naming what the sheet is about. It names the move
        // being replaced rather than adding a serif title over the card it already covers.
        EditorialHeader("SWAP · ${(currentSwapName ?: forExercise.name).uppercase()}", muted, accent)
        Spacer(Modifier.height(10.dp))
        // §4.3: the sheet's one caption, and the only place the two scopes are explained. The pills
        // themselves are the labels; this says what each one costs you.
        Text(
            "Today swaps this session. Every week replaces it in your plan.",
            style = MaterialTheme.typography.bodySmall,
            color = muted.copy(alpha = 0.65f)
        )

        Spacer(Modifier.height(20.dp))

        if (candidates.isEmpty()) {
            // §12: a pool with nothing in it has no zero-SHAPE to draw, which is the last-resort
            // case InlineEmptyHint exists for. It names the concrete unlock, not the absence.
            InlineEmptyHint(
                "No other ${forExercise.muscle.displayName.lowercase()} moves your equipment can do. " +
                    "Add gear in Settings to widen the pool.",
                muted.copy(alpha = 0.65f)
            )
        } else {
            LazyColumn(Modifier.fillMaxWidth().weight(1f, fill = false)) {
                itemsIndexed(candidates, key = { _, def -> def.id }) { index, def ->
                    SwapRow(
                        def = def,
                        armedScope = armed?.takeIf { it.first == def.id }?.second,
                        // Best-first ordering earns the lead row the sheet's ONE caption (§4.3).
                        caption = if (index == 0) def.whenToUse ?: def.why else null,
                        // Radio-style: one row is always armed, so re-tapping the lit pill is a
                        // no-op rather than leaving the sheet with no confirm to press.
                        onArm = { scope -> armed = def.id to scope }
                    )
                }
            }
        }

        // §8: actions at the END — ① the filled confirm, ② its outlined sidekick. The confirm is
        // absent rather than dimmed until something is armed, because §2③ says nothing renders as an
        // affordance while it cannot run. No enter animation: §9 keeps presentational motion out of
        // the working surfaces, and the row's wash has already acknowledged the tap.
        val armedScope = armed?.second
        if (armedDef != null && armedScope != null) {
            Spacer(Modifier.height(16.dp))
            ForgePrimaryCapsule(
                // §11: names the move AND the scope, so the confirm restates the decision instead
                // of asking "are you sure" about an unnamed one.
                when (armedScope) {
                    SwapScope.TODAY -> "Swap to ${armedDef.name} for today"
                    SwapScope.EVERY_WEEK -> "Swap to ${armedDef.name} every week"
                },
                onClick = {
                    armed = null
                    when (armedScope) {
                        SwapScope.TODAY -> onPickForSession(armedDef)
                        SwapScope.EVERY_WEEK -> onPickPersistent(armedDef)
                    }
                },
                modifier = Modifier.fillMaxWidth(),
                // Accent-filled: it has to out-rank a whole list of accent-bordered selection pills,
                // and the light fill read as one more chip among them (§8, 2026-08-24).
                accent = true
            )
        }

        // §8 level ②: reverting is the sidekick to picking, never a filled capsule. Names its
        // referent (§11), and is the only route back out of a persistent swap.
        if (hasPersistentSwap) {
            Spacer(Modifier.height(if (armedDef != null) 10.dp else 16.dp))
            ForgeOutlineCapsule(
                "Back to ${forExercise.name}",
                onClick = onClearPersistent,
                modifier = Modifier.fillMaxWidth()
            )
        }
        Spacer(Modifier.height(24.dp))
    }
}

/**
 * One candidate: equipment glyph, name, what it actually hits, and its own `Today` / `Every week`
 * choice. Both controls are [SegmentPill]s because arming a row SELECTS rather than acts — the
 * commit is the single filled capsule at the sheet's end. [caption] is the lead entry's situational
 * guidance, drawn inside the row it describes.
 */
@OptIn(ExperimentalLayoutApi::class)
@Composable
private fun SwapRow(
    def: ExerciseDef,
    armedScope: SwapScope?,
    caption: String?,
    onArm: (SwapScope) -> Unit
) {
    val cs = MaterialTheme.colorScheme
    val onBg = cs.onBackground
    val muted = cs.onSurfaceVariant
    val accent = cs.primary
    val outline = cs.outline

    Row(
        Modifier
            .fillMaxWidth()
            .clip(RoundedCornerShape(12.dp))
            // The §3 tile wash marks the row waiting on the confirm, so the capsule at the bottom
            // and the row it will act on read as one decision.
            .background(if (armedScope != null) accent.copy(alpha = 0.15f) else Color.Transparent)
            .padding(horizontal = 8.dp, vertical = 12.dp),
        verticalAlignment = Alignment.Top,
        horizontalArrangement = Arrangement.spacedBy(12.dp)
    ) {
        // §8: pickers lead with the ExerciseIcons equipment-class glyph, muted and never
        // accent-tinted. Decorative to TalkBack — the row's text carries the movement (§14).
        Icon(
            ExerciseIcons.forEquipment(def.equipment),
            contentDescription = null,
            tint = muted,
            modifier = Modifier.padding(top = 2.dp).size(20.dp)
        )
        Column(Modifier.weight(1f)) {
            Row(verticalAlignment = Alignment.Top, horizontalArrangement = Arrangement.spacedBy(8.dp)) {
                // §14: no maxLines on a movement name — a long one wraps rather than truncating.
                Text(
                    def.name,
                    style = MaterialTheme.typography.bodyMedium,
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
                Text(
                    target,
                    style = MaterialTheme.typography.bodySmall,
                    color = muted.copy(alpha = 0.65f)
                )
            }
            caption?.let { guidance ->
                Spacer(Modifier.height(8.dp))
                Text(
                    guidance,
                    style = MaterialTheme.typography.bodySmall,
                    color = muted.copy(alpha = 0.65f),
                    fontStyle = FontStyle.Italic
                )
            }
            Spacer(Modifier.height(10.dp))
            // §14: FlowRow so the pair stacks instead of clipping once the font scale grows them
            // past the row's width.
            FlowRow(
                horizontalArrangement = Arrangement.spacedBy(8.dp),
                verticalArrangement = Arrangement.spacedBy(8.dp)
            ) {
                SegmentPill(
                    "Today", armedScope == SwapScope.TODAY, { onArm(SwapScope.TODAY) },
                    accent, onBg, muted, outline
                )
                SegmentPill(
                    "Every week", armedScope == SwapScope.EVERY_WEEK, { onArm(SwapScope.EVERY_WEEK) },
                    accent, onBg, muted, outline
                )
            }
        }
    }
}
