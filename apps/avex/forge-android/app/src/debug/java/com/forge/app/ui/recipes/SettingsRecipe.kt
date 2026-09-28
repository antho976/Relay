package com.forge.app.ui.recipes

import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.rounded.FavoriteBorder
import androidx.compose.runtime.Composable
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableIntStateOf
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.setValue
import androidx.compose.ui.tooling.preview.Preview
import com.forge.app.ui.settings.ButtonKind
import com.forge.app.ui.settings.SettingsButton
import com.forge.app.ui.settings.SettingsButtonBar
import com.forge.app.ui.settings.SettingsFilterChip
import com.forge.app.ui.settings.SettingsChipBlock
import com.forge.app.ui.settings.SettingsGroup
import com.forge.app.ui.settings.SettingsIcons
import com.forge.app.ui.settings.SettingsNavigationRow
import com.forge.app.ui.settings.SettingsRadioRow
import com.forge.app.ui.settings.SettingsScaffold
import com.forge.app.ui.settings.SettingsSearchBar
import com.forge.app.ui.settings.SettingsSegmentedRow
import com.forge.app.ui.settings.SettingsStepperRow
import com.forge.app.ui.settings.SettingsSwitchRow
import com.forge.app.ui.settings.SettingsTheme
import com.forge.app.ui.theme.ForgeTheme

/**
 * RECIPE — Settings / form / editor archetype.
 *
 * Settings runs on its own kit (`ui/settings/SettingsKit.kt`), not the editorial one:
 *
 *   SettingsTheme                  labels re-mapped to sans for every control on the screen
 *   └─ SettingsScaffold            large serif title that collapses into the bar on scroll
 *      ├─ SettingsSearchBar        root page only
 *      └─ SettingsGroup            sans header · rows as 4dp slabs 2dp apart · 16dp outer clip
 *         ├─ navigation row        icon tile · title · live value · chevron
 *         ├─ switch / radio row    the WHOLE row is the tap target, the control is drawn
 *         ├─ segmented row         2–4 short values, sliding thumb
 *         └─ footer                one note under the group, never above it
 *
 * Groups own no page gutter: [SettingsScaffold] applies it once. A do-it-now button closes the
 * page in a [SettingsButtonBar]; the Primary kind takes the accent, the rest stay neutral.
 */
@Composable
fun SettingsRecipe(onBack: () -> Unit = {}) {
    var query by remember { mutableStateOf("") }
    var privacy by remember { mutableStateOf(false) }
    var keepAwake by remember { mutableStateOf(true) }
    var unit by remember { mutableIntStateOf(1) }
    var haptics by remember { mutableIntStateOf(2) }
    var rest by remember { mutableIntStateOf(2) }
    var goal by remember { mutableIntStateOf(0) }
    val picked = remember { mutableStateOf(setOf("Chest", "Back")) }

    SettingsTheme {
        SettingsScaffold(title = "Settings", onBack = onBack) {
            SettingsSearchBar(query, "Search settings", { query = it })

            SettingsGroup("General") {
                SettingsNavigationRow("Appearance", "Warm dark · Red accent", SettingsIcons.Appearance) {}
                SettingsNavigationRow("Units & format", "kg · km · cm · 24h", SettingsIcons.Units) {}
                SettingsNavigationRow("Notifications", "Reminders · recap", SettingsIcons.Notifications) {}
            }

            SettingsGroup("Screen", footer = "A footer carries the group's one note, under it and never above.") {
                SettingsSwitchRow("Privacy mode", "Hides Avex in recent apps and blocks screenshots", privacy, onCheckedChange = { privacy = it })
                SettingsSwitchRow("Keep screen on", "The display stays awake between sets", keepAwake, onCheckedChange = { keepAwake = it })
            }

            SettingsGroup("Units") {
                SettingsSegmentedRow("Weight", listOf("lb", "kg", "st"), unit, onSelect = { unit = it })
                SettingsSegmentedRow(
                    "Haptic feedback", listOf("Off", "Light", "Medium", "Strong"), haptics,
                    supporting = "Set logged, PR hit, rest over.", stacked = true, onSelect = { haptics = it }
                )
                SettingsStepperRow(
                    "Compound lifts", "${1 + rest / 2}:${if (rest % 2 == 0) "00" else "30"}",
                    canDecrease = rest > 0, canIncrease = rest < 5,
                    onDecrease = { rest-- }, onIncrease = { rest++ },
                    supporting = "Squat, bench, deadlift, rows"
                )
            }

            SettingsGroup("Goal") {
                SettingsRadioRow("Build muscle", goal == 0, "Moderate loads, 8 to 12 reps", Icons.Rounded.FavoriteBorder) { goal = 0 }
                SettingsRadioRow("Get stronger", goal == 1, "Heavy loads, 3 to 6 reps", Icons.Rounded.FavoriteBorder) { goal = 1 }
            }

            SettingsGroup("Priority muscles", footer = "These get extra volume.") {
                SettingsChipBlock {
                    listOf("Chest", "Back", "Shoulders", "Arms", "Legs", "Glutes").forEach { m ->
                        SettingsFilterChip(m, m in picked.value) {
                            picked.value = if (m in picked.value) picked.value - m else picked.value + m
                        }
                    }
                }
            }

            SettingsButtonBar {
                SettingsButton("Generate 4-day plan") {}
                SettingsButton("Re-roll exercises", kind = ButtonKind.Tonal) {}
            }
        }
    }
}

@Preview(name = "Settings", showBackground = true, backgroundColor = 0xFF110F0C)
@Composable
private fun SettingsRecipePreview() {
    ForgeTheme { SettingsRecipe() }
}

@Preview(name = "Settings · 200% font", showBackground = true, backgroundColor = 0xFF110F0C, fontScale = 2.0f)
@Composable
private fun SettingsRecipeLargeFontPreview() {
    ForgeTheme { SettingsRecipe() }
}
