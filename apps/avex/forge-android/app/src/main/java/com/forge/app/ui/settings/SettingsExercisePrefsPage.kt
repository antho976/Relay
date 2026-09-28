package com.forge.app.ui.settings

import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.lazy.itemsIndexed
import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.rounded.ArrowDropDown
import androidx.compose.material.icons.rounded.Check
import androidx.compose.material.icons.rounded.Favorite
import androidx.compose.material.icons.rounded.FavoriteBorder
import androidx.compose.material.icons.rounded.RadioButtonUnchecked
import androidx.compose.material.icons.rounded.SearchOff
import androidx.compose.material.icons.rounded.VisibilityOff
import androidx.compose.material3.DropdownMenuItem
import androidx.compose.material3.FilterChip
import androidx.compose.material3.FilterChipDefaults
import androidx.compose.material3.Icon
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.setValue
import androidx.compose.ui.Modifier
import androidx.compose.ui.graphics.vector.ImageVector
import androidx.compose.ui.unit.dp
import com.forge.app.ui.common.clickableLabeled
import com.forge.app.ui.common.window.DropdownMenu

@Composable
internal fun ExercisePrefsPage(state: SettingsUiState, vm: SettingsViewModel, onBack: () -> Unit) {
    val available = remember(state.availableEquipment) {
        state.availableEquipment.mapNotNull {
            runCatching { com.forge.app.program.Equipment.valueOf(it) }.getOrNull()
        }.toSet()
    }
    // The full public library by default. Gear is an optional lens, never an eligibility gate: the
    // old source was availablePool(), so unselected gear silently erased exercises.
    val allByMuscle = remember { exercisePreferencePool(false, emptySet(), null).groupBy { it.muscle } }
    val gearByMuscle = remember(available, state.frozenExerciseIds) {
        exercisePreferencePool(true, available, state.frozenExerciseIds).groupBy { it.muscle }
    }

    // Search plus two independent filters, WHERE (a muscle, your gear, your custom moves) and
    // STATUS (preferred / hidden), over one list with a group per muscle.
    var query by remember { mutableStateOf("") }
    var scope by remember { mutableStateOf<PrefScope>(PrefScope.All) }
    var status by remember { mutableStateOf<Pref?>(null) }
    val q = query.trim()

    val visibleByMuscle = remember(allByMuscle, gearByMuscle, q, scope, status, state.liked, state.disliked) {
        if (scope == PrefScope.Custom) emptyMap()
        else (if (scope == PrefScope.Gear) gearByMuscle else allByMuscle)
            .filterKeys { m -> (scope as? PrefScope.Muscle)?.let { it.m == m } ?: true }
            .mapValues { (_, defs) -> defs.filter { libVisible(it, q, status, state.liked, state.disliked) } }
            .filterValues { it.isNotEmpty() }
    }
    val visibleCustom = remember(state.customExercises, q, scope, status, state.liked, state.disliked) {
        state.customExercises.filter { customVisible(it, q, scope, status, state.liked, state.disliked) }
    }
    val nothingMatches = visibleByMuscle.isEmpty() && visibleCustom.isEmpty()
    val muscles = remember(allByMuscle) { allByMuscle.keys.toList() }

    // Read once here so the item lambdas below capture these, not the whole state: a change to any
    // other setting then leaves the rows alone.
    val liked = state.liked
    val disliked = state.disliked
    val swapPrompt = state.swapDislikePromptEnabled
    val hasCustom = state.customExercises.isNotEmpty()

    SettingsLazyScaffold("Exercise likes", onBack) {
        item("search") { SettingsSearchBar(query, "Search exercises", { query = it }) }
        item("filters") {
            Row(Modifier.fillMaxWidth().padding(top = 12.dp), horizontalArrangement = Arrangement.spacedBy(8.dp)) {
                PrefFilter(
                    value = when (val s = scope) {
                        PrefScope.All -> "All exercises"
                        PrefScope.Gear -> "Your gear"
                        is PrefScope.Muscle -> s.m.displayName.sentenceCase()
                        PrefScope.Custom -> "Custom"
                    },
                    isDefault = scope == PrefScope.All,
                    options = buildList {
                        add("All exercises")
                        add("Your gear")
                        addAll(muscles.map { it.displayName.sentenceCase() })
                        if (hasCustom) add("Custom")
                    },
                    selectedIndex = when (val s = scope) {
                        PrefScope.All -> 0
                        PrefScope.Gear -> 1
                        is PrefScope.Muscle -> muscles.indexOf(s.m) + 2
                        PrefScope.Custom -> muscles.size + 2
                    }
                ) { i ->
                    scope = when {
                        i == 0 -> PrefScope.All
                        i == 1 -> PrefScope.Gear
                        i <= muscles.size + 1 -> PrefScope.Muscle(muscles[i - 2])
                        else -> PrefScope.Custom
                    }
                }
                PrefFilter(
                    value = when (status) {
                        Pref.PREFERRED -> "Preferred"
                        Pref.HIDDEN -> "Hidden"
                        else -> "Any status"
                    },
                    isDefault = status == null,
                    options = listOf("Any status", "Preferred", "Hidden"),
                    selectedIndex = when (status) {
                        Pref.PREFERRED -> 1
                        Pref.HIDDEN -> 2
                        else -> 0
                    }
                ) { i ->
                    status = when (i) {
                        1 -> Pref.PREFERRED
                        2 -> Pref.HIDDEN
                        else -> null
                    }
                }
            }
        }
        item("swap-prompt") {
            // A page-level preference, so it sits with the page's other controls rather than at the
            // end of a list that can run to several hundred rows.
            SettingsGroup(
                modifier = Modifier.padding(top = 20.dp),
                footer = if (liked.isEmpty() && disliked.isEmpty()) "Tap any exercise to prefer or hide it. Preferred moves come up more; hidden ones never do."
                         else "${liked.size} preferred · ${disliked.size} hidden. Tap any exercise to change it."
            ) {
                SettingsSwitchRow(
                    "Ask to hide after swapping",
                    "After a Make default swap, offer to hide the old exercise",
                    swapPrompt,
                    onCheckedChange = { vm.setSwapDislikePromptEnabled(it) }
                )
            }
        }
        visibleByMuscle.forEach { (m, defs) ->
            item("hdr-${m.code}", contentType = "header") {
                val likedN = defs.count { it.id in liked }
                val dislikedN = defs.count { it.id in disliked }
                SettingsGroupHeader(m.displayName.sentenceCase(), Modifier.padding(top = KitGroupSpacing), trailing = prefSummary(likedN, dislikedN, defs.size))
            }
            itemsIndexed(defs, key = { _, def -> "lib-${def.id}" }, contentType = { _, _ -> "pref-row" }) { i, def ->
                val pref = prefOf(def.id in liked, def.id in disliked)
                KitLazyRow(i, defs.size) {
                    ExercisePrefRow(
                        name = def.name,
                        subtitle = libSubtitle(def),
                        icon = com.forge.app.ui.common.ExerciseIcons.forEquipment(def.equipment),
                        pref = pref,
                        onSet = { applyPref(it, pref, setOf(def.id), vm) }
                    )
                }
            }
        }
        if (visibleCustom.isNotEmpty()) {
            item("hdr-custom", contentType = "header") {
                SettingsGroupHeader(
                    "Custom",
                    Modifier.padding(top = KitGroupSpacing),
                    trailing = "${visibleCustom.size} ${if (visibleCustom.size == 1) "exercise" else "exercises"}"
                )
            }
            // Group id-sets are disjoint, so the smallest id is a unique, stable per-group key
            // (name+muscle could collide on malformed data and crash the list).
            itemsIndexed(visibleCustom, key = { _, ref -> "cus-${ref.ids.minOrNull()}" }, contentType = { _, _ -> "pref-row" }) { i, ref ->
                val pref = prefOf(
                    liked = ref.ids.any { it in liked },
                    disliked = ref.ids.any { it in disliked }
                )
                KitLazyRow(i, visibleCustom.size) {
                    ExercisePrefRow(
                        name = ref.name,
                        // Null muscle means the stored code was unreadable: say "Custom" rather than
                        // guess a muscle.
                        subtitle = ref.muscle?.let { "Custom · ${it.displayName}" } ?: "Custom",
                        icon = com.forge.app.ui.common.ExerciseIcons.Custom,
                        pref = pref,
                        onSet = { applyPref(it, pref, ref.ids, vm) }
                    )
                }
            }
        }
        if (nothingMatches) {
            item("empty") {
                SettingsGroup(modifier = Modifier.padding(top = KitGroupSpacing)) {
                    SettingsEmptyBlock(
                        Icons.Rounded.SearchOff,
                        when {
                            q.isNotEmpty() -> "No exercises match “$q”"
                            else -> "Nothing matches these filters"
                        },
                        "Search also reaches muscles and equipment, like chest or dumbbell."
                    )
                }
            }
        }
    }
}

/** WHERE the Exercise likes list looks: the whole pool, owned gear, one muscle, or custom moves. */
private sealed interface PrefScope {
    data object All : PrefScope
    data object Gear : PrefScope
    data class Muscle(val m: com.forge.app.program.MuscleGroup) : PrefScope
    data object Custom : PrefScope
}

/** The full public catalog by default; equipment is an explicit filter, never an eligibility gate. */
internal fun exercisePreferencePool(
    gearOnly: Boolean,
    available: Set<com.forge.app.program.Equipment>,
    frozenIds: Set<String>?
): List<com.forge.app.program.ExerciseDef> =
    if (gearOnly) com.forge.app.program.ExerciseLibrary.availablePool(available, frozenIds)
    else com.forge.app.program.ExerciseLibrary.all.filterNot { it.curatedOnly }

/** The mutually-exclusive preference an exercise can carry (mirrors the liked/disliked data model). */
private enum class Pref(val label: String, val icon: ImageVector) {
    PREFERRED("Preferred", Icons.Rounded.Favorite),
    NEUTRAL("No preference", Icons.Rounded.RadioButtonUnchecked),
    HIDDEN("Hidden", Icons.Rounded.VisibilityOff)
}

/** Collapse the two independent liked/disliked flags into the single 3-state preference the UI shows. */
private fun prefOf(liked: Boolean, disliked: Boolean): Pref = when {
    liked -> Pref.PREFERRED
    disliked -> Pref.HIDDEN
    else -> Pref.NEUTRAL
}

/**
 * Move a row from its [current] preference to [target] using the existing mutually-exclusive toggles
 * (liking clears dislike and vice-versa), so the data layer stays the source of truth. The toggles key
 * "on" off whether ANY id is set — matching how [prefOf] derives the row state — so this round-trips
 * cleanly for grouped custom exercises too.
 */
private fun applyPref(target: Pref, current: Pref, ids: Set<String>, vm: SettingsViewModel) {
    if (target == current) return
    when (target) {
        Pref.PREFERRED -> vm.toggleExercisesLiked(ids)      // adds liked, clears any dislike
        Pref.HIDDEN -> vm.toggleExercisesDisliked(ids)      // adds disliked, clears any like
        Pref.NEUTRAL -> when (current) {
            Pref.PREFERRED -> vm.toggleExercisesLiked(ids)  // toggles the like back off
            Pref.HIDDEN -> vm.toggleExercisesDisliked(ids)  // toggles the dislike back off
            Pref.NEUTRAL -> Unit
        }
    }
}

private fun matchesQuery(name: String, q: String): Boolean =
    q.isEmpty() || name.contains(q, ignoreCase = true)

/** Whether [status] (null = any) admits a row whose flags collapse to [pref]. */
private fun matchesStatus(status: Pref?, pref: Pref): Boolean = status == null || status == pref

private fun libVisible(
    def: com.forge.app.program.ExerciseDef,
    q: String,
    status: Pref?,
    liked: Set<String>,
    disliked: Set<String>
): Boolean {
    // Search reaches past the name into what the user actually thinks in: the muscle
    // ("chest") and the implement ("dumbbell") — GYMAP-13's findability complaint.
    val hit = matchesQuery(def.name, q) ||
        (q.isNotEmpty() && def.muscle.displayName.contains(q, ignoreCase = true)) ||
        (q.isNotEmpty() && def.equipment.any { it.display.contains(q, ignoreCase = true) })
    return hit && matchesStatus(status, prefOf(def.id in liked, def.id in disliked))
}

private fun customVisible(
    ref: com.forge.app.data.repo.CustomExerciseRef,
    q: String,
    scope: PrefScope,
    status: Pref?,
    liked: Set<String>,
    disliked: Set<String>
): Boolean {
    // A muscle scope includes custom moves OF that muscle — scoping to Chest should surface
    // your custom chest move next to the library's, not hide it behind the Custom chip.
    val inScope = when (scope) {
        PrefScope.All, PrefScope.Custom -> true
        PrefScope.Gear -> false // Custom exercises do not store equipment metadata.
        is PrefScope.Muscle -> ref.muscle == scope.m
    }
    if (!inScope) return false
    val hit = matchesQuery(ref.name, q) ||
        (q.isNotEmpty() && ref.muscle?.displayName?.contains(q, ignoreCase = true) == true)
    return hit && matchesStatus(status, prefOf(ref.ids.any { it in liked }, ref.ids.any { it in disliked }))
}

/** Per-muscle header tally: preferred/hidden counts, or a plain move count when neither is set. */
private fun prefSummary(likedN: Int, dislikedN: Int, total: Int): String =
    buildList {
        if (likedN > 0) add("$likedN preferred")
        if (dislikedN > 0) add("$dislikedN hidden")
    }.joinToString(" · ").ifEmpty { "$total ${if (total == 1) "move" else "moves"}" }

/** "Barbell · Compound"-style secondary line for a library movement. */
private fun libSubtitle(def: com.forge.app.program.ExerciseDef): String {
    val equip = def.equipment.firstOrNull()?.display ?: "Bodyweight"
    val kind = when {
        com.forge.app.program.ExerciseTag.COMPOUND in def.tags -> "Compound"
        com.forge.app.program.ExerciseTag.ISOLATION in def.tags -> "Isolation"
        else -> def.difficulty.displayName
    }
    return "$equip · $kind"
}

/**
 * One filter as a Material filter chip showing its current value. Filtering (not the default) is
 * the chip's selected state, so an active filter reads as on at a glance.
 */
@Composable
private fun PrefFilter(
    value: String,
    options: List<String>,
    selectedIndex: Int,
    isDefault: Boolean,
    onSelect: (Int) -> Unit
) {
    var open by remember { mutableStateOf(false) }
    Box {
        FilterChip(
            selected = !isDefault,
            onClick = { open = true },
            label = { Text(value) },
            trailingIcon = { Icon(Icons.Rounded.ArrowDropDown, contentDescription = null, modifier = Modifier.size(FilterChipDefaults.IconSize)) },
            colors = FilterChipDefaults.filterChipColors(
                containerColor = MaterialTheme.colorScheme.surfaceContainerHigh,
                labelColor = MaterialTheme.colorScheme.onSurface,
                iconColor = MaterialTheme.colorScheme.onSurfaceVariant,
                selectedContainerColor = MaterialTheme.colorScheme.primaryContainer,
                selectedLabelColor = MaterialTheme.colorScheme.onSurface,
                selectedTrailingIconColor = MaterialTheme.colorScheme.onSurface
            ),
            border = FilterChipDefaults.filterChipBorder(
                enabled = true,
                selected = !isDefault,
                borderColor = MaterialTheme.colorScheme.surfaceContainerHigh,
                selectedBorderColor = MaterialTheme.colorScheme.primary
            )
        )
        SettingsMenu(open, options, selectedIndex, onDismiss = { open = false }) { onSelect(it); open = false }
    }
}

/**
 * One movement: its equipment glyph, name and kind, and its preference on the right only when it
 * has one (a heart when preferred, a struck eye when hidden), so the neutral majority stays quiet.
 * The whole row opens the three-way choice.
 */
@Composable
private fun ExercisePrefRow(
    name: String,
    subtitle: String,
    icon: ImageVector,
    pref: Pref,
    onSet: (Pref) -> Unit
) {
    var menuOpen by remember { mutableStateOf(false) }
    Box {
        SettingsRowContainer(
            interaction = Modifier.clickableLabeled("$name, ${pref.label.lowercase()}. Change preference") { menuOpen = true }
        ) {
            SettingsIconTile(icon)
            SettingsRowText(name, subtitle)
            when (pref) {
                Pref.PREFERRED -> Icon(Icons.Rounded.Favorite, contentDescription = null, tint = MaterialTheme.colorScheme.primary, modifier = Modifier.size(22.dp))
                Pref.HIDDEN -> Icon(Icons.Rounded.VisibilityOff, contentDescription = null, tint = MaterialTheme.colorScheme.onSurfaceVariant, modifier = Modifier.size(22.dp))
                Pref.NEUTRAL -> Unit
            }
        }
        DropdownMenu(expanded = menuOpen, onDismissRequest = { menuOpen = false }) {
            Pref.entries.forEach { option ->
                val picked = option == pref
                DropdownMenuItem(
                    leadingIcon = {
                        Icon(
                            if (option == Pref.PREFERRED && !picked) Icons.Rounded.FavoriteBorder else option.icon,
                            contentDescription = null,
                            tint = if (option == Pref.PREFERRED) MaterialTheme.colorScheme.primary else MaterialTheme.colorScheme.onSurfaceVariant
                        )
                    },
                    text = {
                        Text(
                            option.label,
                            style = MaterialTheme.typography.bodyLarge,
                            color = if (picked) MaterialTheme.colorScheme.onSurface else MaterialTheme.colorScheme.onSurfaceVariant
                        )
                    },
                    trailingIcon = if (picked) {
                        { Icon(Icons.Rounded.Check, contentDescription = "Current", tint = MaterialTheme.colorScheme.primary) }
                    } else null,
                    onClick = { onSet(option); menuOpen = false }
                )
            }
        }
    }
}
