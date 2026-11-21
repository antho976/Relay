package com.forge.app.ui.gym.train.components

import androidx.compose.runtime.Composable
import androidx.compose.runtime.ReadOnlyComposable
import androidx.compose.ui.platform.LocalDensity
import androidx.compose.ui.unit.dp

/**
 * The set table's column system, in ONE place.
 *
 * The four fixed widths used to be declared three times — the header in `ExerciseCard`, the logged
 * rows in `SetRow`, and the input row in `SetInputRow` — which is how the header and its rows can
 * drift a column apart without anything failing.
 *
 * ## Why the table stops being a table at large font scales
 *
 * Five columns of `sp` text inside fixed `dp` boxes is a density bet, and it loses at 200% (§14
 * requires every screen to survive it). Measured on a 412dp phone, at 2x: the set number plus its PR
 * star needs ~40dp of a 36dp gutter, "10" at `headlineSmall` needs ~50dp of 48dp, the RPE chip needs
 * ~58dp of 44dp, and the weight column — the actual content — needs ~200dp of the 164dp left over.
 * No redistribution fixes that; the row is asking for roughly 1.6x the width the phone has.
 *
 * So above [STACK_AT] the row reflows: identity and weight stay on the first line, and reps, RPE and
 * the delta drop to a second line that carries its own inline labels. Nothing is hidden, every touch
 * target survives, and the reading order is unchanged. Below it, the table is byte-for-byte what it
 * always was — 1.3x still fits with room to spare, so the common large-text user never sees the
 * reflow.
 */
internal object SetTable {

    val SET_COL_W = 36.dp
    val REPS_COL_W = 48.dp
    val RPE_COL_W = 44.dp
    val DELTA_COL_W = 72.dp

    /**
     * The font scale at which the columns stop fitting. Android offers 0.85 / 1.0 / 1.15 / 1.3 /
     * 1.5 / 1.8 / 2.0, so this keeps every scale through 1.3 tabular and stacks from 1.5 up.
     */
    private const val STACK_AT = 1.4f

    /** True when the row should render as two lines rather than five columns. */
    @Composable
    @ReadOnlyComposable
    fun stacked(): Boolean = LocalDensity.current.fontScale >= STACK_AT
}
