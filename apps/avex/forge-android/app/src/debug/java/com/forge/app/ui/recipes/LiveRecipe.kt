@file:OptIn(androidx.compose.material3.ExperimentalMaterial3Api::class)

package com.forge.app.ui.recipes

import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.width
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.rememberScrollState
import androidx.compose.foundation.verticalScroll
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Scaffold
import androidx.compose.material3.Text
import androidx.compose.material3.TopAppBar
import androidx.compose.material3.TopAppBarDefaults
import androidx.compose.runtime.Composable
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableIntStateOf
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.platform.LocalView
import androidx.compose.ui.tooling.preview.Preview
import androidx.compose.ui.unit.dp
import com.forge.app.ui.common.EditorialHeader
import com.forge.app.ui.common.ForgeHapticType
import com.forge.app.ui.common.ForgePrimaryCapsule
import com.forge.app.ui.common.ForgeWordmark
import com.forge.app.ui.common.GlyphButton
import com.forge.app.ui.common.forgeHaptic
import com.forge.app.ui.theme.ForgeTheme
import com.forge.app.ui.theme.LocalForgeSettings

/**
 * RECIPE — Live / flow archetype (DESIGN §3).
 *
 * The live session and the freestyle log. Function-first: this screen is used one-handed, mid-set,
 * sometimes sweating. Reach and target size beat elegance, and presentational motion is banned —
 * NO entrance cascade, NO draw-in, NO decorative figures.
 *
 *   top bar (wordmark + ≤1 action)
 *   ├─ the target        the one number that matters, big                    §6
 *   ├─ steppers          hot-path numbers are ± targets, never keyboard-first §13
 *   └─ log action        haptic on commit, Undo over confirm                  §9, §12
 *
 * Haptics are RARE (§9): set logged, PR/finish, timer ticks. Nothing else vibrates, ever.
 */
@Composable
fun LiveRecipe() {
    val onBg = MaterialTheme.colorScheme.onBackground
    val muted = MaterialTheme.colorScheme.onSurfaceVariant
    val accent = MaterialTheme.colorScheme.primary

    val view = LocalView.current
    val hapticStrength = LocalForgeSettings.current.hapticStrength

    var weight by remember { mutableIntStateOf(70) }
    var reps by remember { mutableIntStateOf(8) }
    var logged by remember { mutableStateOf(0) }

    Scaffold(
        topBar = {
            TopAppBar(
                title = { ForgeWordmark() },
                colors = TopAppBarDefaults.topAppBarColors(containerColor = Color.Transparent)
            )
        },
        containerColor = Color.Transparent
    ) { inner ->
        Column(
            Modifier
                .padding(inner)
                .verticalScroll(rememberScrollState())
                .padding(horizontal = 24.dp)
        ) {
            Spacer(Modifier.height(8.dp))
            Text("Barbell Row", style = MaterialTheme.typography.headlineSmall, color = onBg)
            Spacer(Modifier.height(2.dp))
            Text(
                "SET ${logged + 1} OF 4",
                style = MaterialTheme.typography.labelMedium,
                color = muted
            )

            // ── The target ──────────────────────────────────────────────────────────────────────
            // §6: ONE serif figure — the number the user is about to lift. Everything else steps
            // down. No figure row here; a live screen is not a dashboard.
            Spacer(Modifier.height(24.dp))
            Stepper(
                label = "WEIGHT",
                value = "$weight",
                unit = "KG",
                onMinus = { weight -= 5 },
                onPlus = { weight += 5 },
            )
            Spacer(Modifier.height(16.dp))
            Stepper(
                label = "REPS",
                value = "$reps",
                unit = "",
                onMinus = { if (reps > 1) reps-- },
                onPlus = { reps++ },
            )

            // ── Commit ──────────────────────────────────────────────────────────────────────────
            // §9: the set-log haptic is one of only three the app fires. §12: this would raise an
            // Undo snackbar via SnackbarController rather than a confirm dialog — undo over confirm.
            Spacer(Modifier.height(28.dp))
            ForgePrimaryCapsule(
                label = "Log set",
                onClick = {
                    logged++
                    view.forgeHaptic(ForgeHapticType.SET_LOGGED, hapticStrength)
                },
                modifier = Modifier.fillMaxWidth()
            )

            Spacer(Modifier.height(28.dp))
            EditorialHeader("DONE", muted, accent)
            Spacer(Modifier.height(10.dp))
            if (logged == 0) {
                // §12: honest zero, in the section's own vocabulary — not the words "no sets yet".
                Text("0 SETS", style = MaterialTheme.typography.labelMedium, color = muted)
            } else {
                Text(
                    "$logged × $weight kg",
                    style = MaterialTheme.typography.bodyMedium,
                    color = onBg
                )
            }
            Spacer(Modifier.height(32.dp))
        }
    }
}

/**
 * §13: hot-path numbers are steppers, never keyboard-first — nobody types on a phone between sets.
 * §8/§14: `GlyphButton` guarantees the ≥48dp target and the TalkBack label; the visual glyph stays
 * trim while the touch area comes from padding.
 */
@Composable
private fun Stepper(
    label: String,
    value: String,
    unit: String,
    onMinus: () -> Unit,
    onPlus: () -> Unit,
) {
    val onBg = MaterialTheme.colorScheme.onBackground
    val muted = MaterialTheme.colorScheme.onSurfaceVariant
    Column {
        Text(label, style = MaterialTheme.typography.labelMedium, color = muted)
        Spacer(Modifier.height(2.dp))
        Row(
            Modifier.fillMaxWidth(),
            verticalAlignment = Alignment.CenterVertically,
            horizontalArrangement = Arrangement.spacedBy(16.dp)
        ) {
            GlyphButton("−", "Decrease $label", muted, onMinus)
            Row(Modifier.weight(1f), verticalAlignment = Alignment.Bottom) {
                // The serif figure IS the target. tnum keeps it from reflowing as it changes (§6).
                Text(value, style = MaterialTheme.typography.headlineLarge, color = onBg)
                if (unit.isNotEmpty()) {
                    // §7: spacing comes from the layout, not from a leading space inside the
                    // string. (This previously used a zero-height Spacer inside a Row, which does
                    // nothing at all, plus a padded space glyph doing the real work.)
                    Spacer(Modifier.width(4.dp))
                    Text(
                        unit,
                        style = MaterialTheme.typography.labelMedium,
                        color = muted,
                        modifier = Modifier.padding(bottom = 6.dp)
                    )
                }
            }
            GlyphButton("+", "Increase $label", muted, onPlus)
        }
    }
}

@Preview(name = "Live", showBackground = true, backgroundColor = 0xFF0E0E11)
@Composable
private fun LiveRecipePreview() {
    ForgeTheme { LiveRecipe() }
}

@Preview(name = "Live · 200% font", showBackground = true, backgroundColor = 0xFF0E0E11, fontScale = 2.0f)
@Composable
private fun LiveRecipeLargeFontPreview() {
    ForgeTheme { LiveRecipe() }
}
