@file:OptIn(androidx.compose.foundation.layout.ExperimentalLayoutApi::class)
package com.forge.app.ui.settings

import android.Manifest
import android.content.Intent
import android.content.pm.PackageManager
import android.os.Build
import android.provider.Settings
import androidx.compose.animation.AnimatedVisibility
import androidx.compose.foundation.background
import androidx.compose.foundation.border
import androidx.compose.foundation.clickable
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.rememberScrollState
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.foundation.text.BasicTextField
import androidx.compose.foundation.verticalScroll
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.FlowRow
import androidx.compose.foundation.layout.PaddingValues
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.width
import androidx.compose.foundation.layout.widthIn
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.lazy.LazyColumn
import androidx.compose.foundation.lazy.items
import androidx.compose.material3.Icon
import com.forge.app.ui.common.window.DropdownMenu
import androidx.compose.material3.DropdownMenuItem
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.minimumInteractiveComponentSize
import androidx.compose.material3.OutlinedTextField
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.runtime.DisposableEffect
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.clip
import androidx.compose.ui.graphics.SolidColor
import androidx.compose.ui.platform.LocalContext
import androidx.compose.ui.text.font.FontStyle
import androidx.compose.ui.text.style.TextAlign
import androidx.compose.ui.text.style.TextOverflow
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp
import androidx.lifecycle.Lifecycle
import androidx.lifecycle.LifecycleEventObserver
import androidx.lifecycle.compose.LocalLifecycleOwner
import com.forge.app.ui.common.clickableLabeled

@Composable
internal fun AppearancePage(state: SettingsUiState, vm: SettingsViewModel, modifier: Modifier = Modifier) {
    var showAppIconSheet by remember { mutableStateOf(false) }
    Column(
        modifier
            .fillMaxSize()
            .verticalScroll(rememberScrollState())
            .padding(bottom = 24.dp)
    ) {
        // Grouped by quiet mono anchors + air — no per-row hairlines (DESIGN §1/§7).
        SettingsSectionHeader("Display", top = 12.dp)
        ToggleRow("AMOLED pure black", "Pure-black backgrounds. Saves battery on OLED screens; on an LCD phone it just looks darker.", state.amoledMode, vm::setAmoledMode)
        ToggleRow("Privacy mode", "Hide the app preview in recent apps & block screenshots", state.privacyMode, vm::setPrivacyMode)

        SettingsSectionHeader("Accent")
        ToggleRow(
            "Use accent color",
            "Off makes the app monochrome; highlights use a neutral tone.",
            state.accentEnabled,
            vm::setAccentEnabled
        )
        // The picker is only meaningful while the accent is on, so it collapses away when off (the
        // chosen colour is kept and returns on re-enable).
        AnimatedVisibility(visible = state.accentEnabled) {
            AccentColorRow(state.accentColorHex, vm::setAccentColorHex)
        }

        SettingsSectionHeader("App icon")
        AppIconRow(state.appIconKey) { showAppIconSheet = true }
        ToggleRow(
            "Custom startup animation",
            "Off shows the plain black-and-white Avex instead of the icon-themed launch.",
            state.themedLaunchIntro,
            vm::setThemedLaunchIntro
        )

        Spacer(Modifier.height(8.dp))
        SectionResetRow(com.forge.app.data.prefs.SettingsSection.APPEARANCE, vm)
    }

    if (showAppIconSheet) {
        AppIconPickerSheet(
            selectedKey = state.appIconKey,
            onSelect = { vm.setAppIcon(it); showAppIconSheet = false },
            onDismiss = { showAppIconSheet = false },
        )
    }
}

@Composable
internal fun FormatPage(state: SettingsUiState, vm: SettingsViewModel, modifier: Modifier = Modifier) {
    Column(
        modifier = modifier
            .fillMaxSize()
            .verticalScroll(rememberScrollState())
            .padding(bottom = 32.dp)
    ) {
        // The title's line IS the live preview: it re-renders the moment any unit below flips, so
        // the page answers "what will my numbers look like" before a single tap (one home, §4.3).
        SettingsPageTitle("Units & format", formatPreview(state))

        SettingsSectionHeader("Units")
        val weightUnits = listOf("lb", "kg", "st")
        SettingsSegmentRow("Weight", weightUnits, weightUnits.indexOf(state.weightUnit.label).coerceAtLeast(0)) {
            vm.setWeightUnit(com.forge.app.domain.units.WeightUnit.fromKey(weightUnits[it]))
        }
        SettingsSegmentRow("Distance", listOf("km", "mi"), if (state.useMiles) 1 else 0) { vm.setUseMiles(it == 1) }
        SettingsSegmentRow("Length", listOf("cm", "in"), if (state.useCm) 0 else 1) { vm.setUseCm(it == 0) }

        // Only the controls the app honours (2026-09-26 audit, "Settings that do nothing"). Date
        // format and Timezone were saved and previewed here but read by nothing: dates are drawn
        // by ~45 fixed patterns and every clock follows the phone's zone. Both rows are gone until
        // a redesign can honour them (`design/SETTLED.md`); their pref keys stay so a restore and
        // a section reset still round-trip.
        SettingsSectionHeader("Time")
        SettingsSegmentRow("Clock", listOf("12h", "24h"), if (state.timeFormat24h) 1 else 0) { vm.setTimeFormat24h(it == 1) }
        SettingsSegmentRow(
            "Week starts",
            listOf("Mon", "Sun"),
            if (state.firstDayMonday) 0 else 1,
            explainer = "Orders Home's week and what counts as this week."
        ) { vm.setFirstDayMonday(it == 0) }

        SettingsSectionHeader("Strength standards")
        val sexes = listOf("male", "female")
        SettingsSegmentRow(
            "Sex",
            listOf("Male", "Female"),
            sexes.indexOf(state.userSex).coerceAtLeast(0),
            explainer = "Scales only the bodyweight standards on Stats."
        ) { vm.setUserSex(sexes[it]) }

        SectionResetRow(com.forge.app.data.prefs.SettingsSection.FORMAT, vm)
    }
}

// ─── Format-page building blocks ─────────────────────────────────────────────

/** "135 lb · 5.0 km · 90 cm · 6:30 PM": one sample of every format on the page. */
private fun formatPreview(state: SettingsUiState): String {
    val time = java.time.LocalTime.of(18, 30).format(
        java.time.format.DateTimeFormatter.ofPattern(com.forge.app.domain.units.clockPattern(state.timeFormat24h))
    )
    return listOf(
        com.forge.app.domain.units.formatWeight(135.0, state.weightUnit),
        com.forge.app.domain.units.formatDistance(5.0, state.useMiles),
        com.forge.app.domain.units.formatLength(90.0, state.useCm),
        time
    ).joinToString(" · ")
}

private val COMPOUND_REST = listOf(120, 150, 180, 210, 240, 300)
private val ISOLATION_REST = listOf(45, 60, 90, 120, 150)

@Composable
internal fun SessionPage(state: SettingsUiState, vm: SettingsViewModel, modifier: Modifier = Modifier) {
    Column(
        modifier
            .fillMaxSize()
            .verticalScroll(rememberScrollState())
            .padding(bottom = 24.dp)
    ) {
        SettingsPageTitle("Session", "Applies while you log a workout.")

        SettingsSectionHeader("Feel")
        val haptics = listOf("off", "light", "medium", "strong")
        SettingsSegmentRow(
            "Haptic feedback",
            listOf("Off", "Light", "Medium", "Strong"),
            haptics.indexOf(state.hapticStrength).coerceAtLeast(0),
            explainer = "Set logged, PR hit, rest over.",
            fill = true
        ) { vm.setHapticStrength(haptics[it]) }
        ToggleRow(
            "Keep screen on",
            "The display stays awake between sets.",
            state.keepScreenOn,
            vm::setKeepScreenOn
        )

        SettingsSectionHeader("Rest timer")
        SettingsCaption("Starting points. Hard sets add time, and the timer learns your pace.")
        RestStepper(
            label = "Compound lifts",
            explainer = "Squat, bench, deadlift, rows",
            seconds = state.restCompoundSeconds,
            steps = COMPOUND_REST,
            onChange = vm::setRestCompoundSeconds
        )
        RestStepper(
            label = "Isolation lifts",
            explainer = "Curls, raises, extensions",
            seconds = state.restIsolationSeconds,
            steps = ISOLATION_REST,
            onChange = vm::setRestIsolationSeconds
        )

        SettingsSectionHeader("Note templates")
        SettingsCaption("One-tap starters under the note field when you log a set.")
        NoteTemplatesEditor(state.noteTemplates, vm::addNoteTemplate, vm::removeNoteTemplate)

        SectionResetRow(com.forge.app.data.prefs.SettingsSection.SESSION, vm)
    }
}

/** A rest time walked along its fixed [steps]; an off-list stored value snaps to the nearest step. */
@Composable
private fun RestStepper(label: String, explainer: String, seconds: Int, steps: List<Int>, onChange: (Int) -> Unit) {
    val i = steps.indexOf(seconds).takeIf { it >= 0 }
        ?: steps.indices.minBy { kotlin.math.abs(steps[it] - seconds) }
    SettingsStepperRow(
        label = label,
        explainer = explainer,
        value = "${steps[i] / 60}:${(steps[i] % 60).toString().padStart(2, '0')}",
        canDecrease = i > 0,
        canIncrease = i < steps.lastIndex,
        onDecrease = { onChange(steps[i - 1]) },
        onIncrease = { onChange(steps[i + 1]) }
    )
}

/**
 * Edit the tap-to-insert note starters shown under the set note field (#540). Each template drops
 * onto its own line in the note with the cursor parked after it, so prompt-style entries
 * ("energy:", "tempo:") are the useful shape. Removing every template just leaves the field bare.
 * A template is drawn as a removable capsule in the selectable family's weight (bodyMedium, 14×9),
 * the whole capsule its ≥48dp remove target.
 */
@Composable
private fun NoteTemplatesEditor(
    templates: Set<String>,
    onAdd: (String) -> Unit,
    onRemove: (String) -> Unit
) {
    val muted = MaterialTheme.colorScheme.onSurfaceVariant
    val onBg = MaterialTheme.colorScheme.onBackground
    val outline = MaterialTheme.colorScheme.outline
    var input by remember { mutableStateOf("") }
    val sorted = remember(templates) {
        templates.filter { it.isNotBlank() }.sortedBy { it.trim().lowercase() }
    }
    Column(Modifier.fillMaxWidth().padding(horizontal = SETTINGS_GUTTER)) {
        if (sorted.isEmpty()) {
            SettingsExplainer("None yet, so the note field starts blank.")
        } else {
            FlowRow(horizontalArrangement = Arrangement.spacedBy(8.dp), verticalArrangement = Arrangement.spacedBy(0.dp)) {
                sorted.forEach { t ->
                    Box(
                        Modifier
                            .minimumInteractiveComponentSize()
                            .clip(RoundedCornerShape(50))
                            .clickableLabeled("Remove ${t.trim()}") { onRemove(t) },
                        contentAlignment = Alignment.Center
                    ) {
                        Row(
                            Modifier
                                .border(1.dp, outline.copy(alpha = 0.35f), RoundedCornerShape(50))
                                .padding(start = 14.dp, end = 12.dp, top = 9.dp, bottom = 9.dp),
                            verticalAlignment = Alignment.CenterVertically,
                            horizontalArrangement = Arrangement.spacedBy(8.dp)
                        ) {
                            Text(t.trim(), style = MaterialTheme.typography.bodyMedium, color = onBg)
                            Text("✕", style = MaterialTheme.typography.bodySmall, color = muted)
                        }
                    }
                }
            }
        }
        Spacer(Modifier.height(12.dp))
        Row(verticalAlignment = Alignment.CenterVertically, horizontalArrangement = Arrangement.spacedBy(10.dp)) {
            OutlinedTextField(
                value = input,
                onValueChange = { input = it },
                singleLine = true,
                shape = RoundedCornerShape(12.dp),
                textStyle = MaterialTheme.typography.bodyMedium,
                placeholder = {
                    Text(
                        "Add a starter, e.g. tempo:",
                        style = MaterialTheme.typography.bodyMedium,
                        color = muted.copy(alpha = 0.5f)
                    )
                },
                modifier = Modifier.weight(1f)
            )
            SettingsOutlineAction("Add", enabled = input.isNotBlank()) {
                onAdd(input)
                input = ""
            }
        }
    }
}

/** A scoped "reset to defaults" affordance for a settings sub-page (#544) — clears only this page's
 *  preferences (no data loss), so a user can clean-slate one area without a global/factory reset. */
@Composable
internal fun SectionResetRow(section: com.forge.app.data.prefs.SettingsSection, vm: SettingsViewModel) {
    // §8 ②: a one-shot that changes state is a capsule at the END of the page, not a text link at
    // `muted@0.8`, which made the only destructive control on the page the faintest thing on it.
    Spacer(Modifier.height(20.dp))
    SettingsActionRow {
        SettingsOutlineAction("Reset this page to defaults") { vm.resetSection(section) }
    }
}

/**
 * Shown at the top of Notifications when the OS notification permission is denied (Android 13+) —
 * every toggle below is inert until it's granted. The in-app rationale (N1) is one-time, so this is
 * the re-enable path for a user who declined it: tapping opens the OS app-notification settings,
 * which always works regardless of how many times the permission was denied. Re-checks on resume so
 * it disappears the moment the user flips notifications on and returns.
 */
@Composable
private fun NotificationsBlockedBanner() {
    if (Build.VERSION.SDK_INT < Build.VERSION_CODES.TIRAMISU) return
    val context = LocalContext.current
    val lifecycleOwner = LocalLifecycleOwner.current
    fun granted() = context.checkSelfPermission(
        Manifest.permission.POST_NOTIFICATIONS
    ) == PackageManager.PERMISSION_GRANTED
    var blocked by remember { mutableStateOf(!granted()) }
    DisposableEffect(lifecycleOwner) {
        val observer = LifecycleEventObserver { _, event ->
            if (event == Lifecycle.Event.ON_RESUME) blocked = !granted()
        }
        lifecycleOwner.lifecycle.addObserver(observer)
        onDispose { lifecycleOwner.lifecycle.removeObserver(observer) }
    }
    if (!blocked) return
    Column(
        Modifier
            .fillMaxWidth()
            .clickable(onClickLabel = "Open notification settings") {
                context.startActivity(
                    Intent(Settings.ACTION_APP_NOTIFICATION_SETTINGS)
                        .putExtra(Settings.EXTRA_APP_PACKAGE, context.packageName)
                )
            }
            .padding(horizontal = SETTINGS_GUTTER, vertical = SETTINGS_ROW_PAD)
    ) {
        Text("Notifications are turned off", style = MaterialTheme.typography.bodyMedium, color = MaterialTheme.colorScheme.onBackground)
        Spacer(Modifier.height(2.dp))
        Text(
            "Avex can't send any of the below until you turn them on for the app. Tap to open system settings →",
            style = MaterialTheme.typography.bodySmall, color = MaterialTheme.colorScheme.onSurfaceVariant
        )
    }
    Spacer(Modifier.height(8.dp))
}

@Composable
internal fun NotificationsPage(state: SettingsUiState, vm: SettingsViewModel, modifier: Modifier = Modifier) {
    // Two groups named by where the thing arrives. "On your phone" / "In the app" is a parallel pair
    // that needs no gloss — the earlier version captioned each ("Push alerts, delivered even when
    // Avex is closed") and was explaining push notifications to someone who already knows. §4.3: cut
    // mechanics narration, don't trim it. Quiet hours lives INSIDE the phone group rather than
    // trailing the in-app switches, where it read as though it silenced those too (Antho).
    Column(
        modifier
            .fillMaxSize()
            // Was missing, so everything past the fold was unreachable — the quiet-hours toggle fell
            // off the bottom the moment the in-app group was added. Same idiom as every sibling page.
            .verticalScroll(rememberScrollState())
            .padding(bottom = 24.dp)
    ) {
        NotificationsBlockedBanner()

        SettingsSectionHeader("On your phone", top = 12.dp)
        ToggleRow(
            "Training reminders",
            "A daily nudge to train on scheduled days and keep your streak alive",
            state.trainingReminderEnabled, vm::setTrainingReminderEnabled
        )
        if (state.trainingReminderEnabled) {
            HourPickerRow("Remind me at", state.trainingReminderHour, vm::setTrainingReminderHour)
        }
        ToggleRow(
            "Weekly recap",
            "A weekly summary of workouts, volume, and streak",
            state.weeklyRecapEnabled, vm::setWeeklyRecapEnabled
        )
        ToggleRow(
            "Rest timer alerts",
            "Buzz + notify when your rest ends while the app is in the background",
            state.restTimerAlertEnabled, vm::setRestTimerAlertEnabled
        )
        // A suppressor rather than an alert, but it suppresses exactly the three above, so it belongs
        // with them. Worded to name what it silences instead of echoing a header (§4.3).
        ToggleRow(
            "Silence during quiet hours",
            "Mute the alerts above during each day's window",
            state.quietHoursEnabled, vm::setQuietHoursEnabled
        )
        if (state.quietHoursEnabled) {
            QuietHoursDays(state, vm)
        }

        SettingsSectionHeader("In the app")
        com.forge.app.data.repo.NoticeKind.entries.forEach { kind ->
            ToggleRow(
                kind.label,
                kind.explainer,
                kind.key !in state.disabledNoticeKinds,
                { enabled -> vm.setNoticeKindEnabled(kind.key, enabled) }
            )
        }

        SectionResetRow(com.forge.app.data.prefs.SettingsSection.NOTIFICATIONS, vm)
    }
}

@Composable
internal fun ExercisePrefsPage(state: SettingsUiState, vm: SettingsViewModel, modifier: Modifier = Modifier) {
    val muted = MaterialTheme.colorScheme.onSurfaceVariant
    val available = remember(state.availableEquipment) {
        state.availableEquipment.mapNotNull {
            runCatching { com.forge.app.program.Equipment.valueOf(it) }.getOrNull()
        }.toSet()
    }
    // Default to the full public library. The previous source was always availablePool(), so gear
    // the user had not selected silently erased exercises instead of acting as an optional lens.
    val allByMuscle = remember { exercisePreferencePool(false, emptySet(), null).groupBy { it.muscle } }
    val gearByMuscle = remember(available, state.frozenExerciseIds) {
        exercisePreferencePool(true, available, state.frozenExerciseIds).groupBy { it.muscle }
    }

    // Search-first flat list: a search box + two independent filter dimensions — WHERE (a muscle,
    // or your custom moves) and STATUS (preferred/hidden) — over one scrolling list with muscle
    // sub-headers. Both combine, so "Chest + Preferred" or "Custom + Hidden" are one tap each.
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

    // ONE scrolling list, header included. The title, search, filters and the page toggle used to sit
    // pinned above the list and took roughly a third of the screen from the rows they filter; they now
    // scroll away like every other settings page's top does.
    LazyColumn(modifier.fillMaxSize(), contentPadding = PaddingValues(bottom = 32.dp)) {
        item("title") {
            SettingsPageTitle(
                "Exercise likes",
                if (state.liked.isEmpty() && state.disliked.isEmpty()) "Preferred moves come up more. Hidden ones never do."
                else "${state.liked.size} preferred · ${state.disliked.size} hidden"
            )
        }
        item("search") { PrefSearchField(query, onQuery = { query = it }) }
        item("filters") {
            // Two compact selectors — WHERE (muscle / custom) and STATUS (preferred / hidden). A chip
            // per muscle turned into a wall that scrolled off screen; dropdowns keep both dimensions
            // one tap deep and always fully visible.
            Row(
                modifier = Modifier.fillMaxWidth().padding(horizontal = SETTINGS_GUTTER),
                horizontalArrangement = Arrangement.spacedBy(8.dp)
            ) {
                PrefSelector(
                    value = when (val s = scope) {
                        PrefScope.All -> "All exercises"
                        PrefScope.Gear -> "Your gear"
                        is PrefScope.Muscle -> s.m.displayName
                        PrefScope.Custom -> "Custom"
                    },
                    isDefault = scope == PrefScope.All,
                    options = buildList {
                        add("All exercises")
                        add("Your gear")
                        addAll(muscles.map { it.displayName })
                        if (state.customExercises.isNotEmpty()) add("Custom")
                    },
                    selectedIndex = when (val s = scope) {
                        PrefScope.All -> 0
                        PrefScope.Gear -> 1
                        is PrefScope.Muscle -> muscles.indexOf(s.m) + 2
                        PrefScope.Custom -> muscles.size + 2
                    },
                    modifier = Modifier.weight(1f)
                ) { i ->
                    scope = when {
                        i == 0 -> PrefScope.All
                        i == 1 -> PrefScope.Gear
                        i <= muscles.size + 1 -> PrefScope.Muscle(muscles[i - 2])
                        else -> PrefScope.Custom
                    }
                }
                PrefSelector(
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
                    },
                    modifier = Modifier.weight(1f)
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
            // A page-level preference, so it sits WITH the page's other controls. It used to be the
            // last item of a list that can hold several hundred exercise rows.
            Spacer(Modifier.height(4.dp))
            ToggleRow(
                label = "Ask to hide after swapping",
                subtitle = "After a \"Make default\" swap, offer to hide the old exercise.",
                checked = state.swapDislikePromptEnabled,
                onCheckedChange = { vm.setSwapDislikePromptEnabled(it) }
            )
        }
            visibleByMuscle.forEach { (m, defs) ->
                item("hdr-${m.code}") {
                    val likedN = defs.count { it.id in state.liked }
                    val dislikedN = defs.count { it.id in state.disliked }
                    PrefSectionHeader(m.displayName.uppercase(), prefSummary(likedN, dislikedN, defs.size))
                }
                items(defs, key = { "lib-${it.id}" }) { def ->
                    val pref = prefOf(def.id in state.liked, def.id in state.disliked)
                    ExercisePrefRow(
                        name = def.name,
                        subtitle = libSubtitle(def),
                        icon = com.forge.app.ui.common.ExerciseIcons.forEquipment(def.equipment),
                        pref = pref,
                        onSet = { applyPref(it, pref, setOf(def.id), vm) }
                    )
                }
            }
            if (visibleCustom.isNotEmpty()) {
                item("hdr-custom") {
                    PrefSectionHeader(
                        "CUSTOM",
                        "${visibleCustom.size} ${if (visibleCustom.size == 1) "exercise" else "exercises"}"
                    )
                }
                // Group id-sets are disjoint, so the smallest id is a unique, stable per-group key
                // (name+muscle could collide on malformed data and crash the list).
                items(visibleCustom, key = { "cus-${it.ids.minOrNull()}" }) { ref ->
                    val pref = prefOf(
                        liked = ref.ids.any { it in state.liked },
                        disliked = ref.ids.any { it in state.disliked }
                    )
                    ExercisePrefRow(
                        name = ref.name,
                        // Null muscle ⇒ the stored code was missing/unparseable; show a plain "Custom"
                        // label rather than fabricating a (wrong) muscle.
                        subtitle = ref.muscle?.let { "Custom · ${it.displayName}" } ?: "Custom",
                        icon = com.forge.app.ui.common.ExerciseIcons.Custom,
                        pref = pref,
                        onSet = { applyPref(it, pref, ref.ids, vm) }
                    )
                }
            }
            if (nothingMatches) {
                item("empty") {
                    Text(
                        when {
                            q.isNotEmpty() -> "No exercises match “$q”."
                            status != null || scope != PrefScope.All -> "Nothing matches these filters."
                            else -> "No exercises here yet."
                        },
                        style = MaterialTheme.typography.bodyMedium,
                        color = muted,
                        textAlign = TextAlign.Center,
                        modifier = Modifier.fillMaxWidth().padding(horizontal = SETTINGS_GUTTER, vertical = 48.dp)
                    )
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
private enum class Pref(val label: String) {
    PREFERRED("Preferred"), NEUTRAL("Neutral"), HIDDEN("Hidden")
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

/** Per-muscle header summary: preferred/hidden tallies, or a plain move count when neither is set. */
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

@Composable
private fun PrefSearchField(query: String, onQuery: (String) -> Unit) =
    SettingsSearchField(
        query = query,
        placeholder = "Search exercises",
        onQueryChange = onQuery,
        modifier = Modifier.padding(horizontal = SETTINGS_GUTTER, vertical = 12.dp)
    )

/**
 * One filter dimension as a compact dropdown capsule showing the current pick. It shares the
 * selectable formula (§3): [isDefault] = not filtering, so it draws the quiet outline@0.35 border
 * and a muted label; an ACTIVE filter takes the accent border + accent@0.15 wash, the same
 * "this is on" every chip and tile in the app says.
 */
@Composable
private fun PrefSelector(
    value: String,
    options: List<String>,
    selectedIndex: Int,
    isDefault: Boolean,
    modifier: Modifier = Modifier,
    onSelect: (Int) -> Unit
) {
    val onBg = MaterialTheme.colorScheme.onBackground
    val muted = MaterialTheme.colorScheme.onSurfaceVariant
    val (border, fill) = com.forge.app.ui.common.selectableColors(selected = !isDefault)
    var open by remember { mutableStateOf(false) }
    Box(modifier) {
        Row(
            Modifier
                .fillMaxWidth()
                .minimumInteractiveComponentSize()
                .clip(RoundedCornerShape(50))
                .border(1.dp, border, RoundedCornerShape(50))
                .background(fill)
                .clickableLabeled("Filter: $value. Change") { open = true }
                .padding(horizontal = 14.dp, vertical = 9.dp),
            verticalAlignment = Alignment.CenterVertically,
            horizontalArrangement = Arrangement.spacedBy(6.dp)
        ) {
            Text(
                value,
                style = MaterialTheme.typography.bodyMedium,
                color = if (isDefault) muted else onBg,
                modifier = Modifier.weight(1f)
            )
            Text("▾", style = MaterialTheme.typography.labelMedium, color = muted)
        }
        DropdownMenu(expanded = open, onDismissRequest = { open = false }) {
            options.forEachIndexed { i, label ->
                DropdownMenuItem(
                    text = {
                        Row(
                            verticalAlignment = Alignment.CenterVertically,
                            horizontalArrangement = Arrangement.spacedBy(10.dp)
                        ) {
                            // A leading dot marks the active pick; a blank keeps the labels aligned.
                            Box(Modifier.width(8.dp)) {
                                if (i == selectedIndex) StatusDot(active = true, size = 7.dp)
                            }
                            Text(
                                label,
                                color = if (i == selectedIndex) onBg else muted,
                                style = MaterialTheme.typography.bodyMedium
                            )
                        }
                    },
                    onClick = { onSelect(i); open = false }
                )
            }
        }
    }
}

// Air + the mono header ARE the separator (DESIGN §1/§7) — no hairline under the anchor.
/** The muscle group anchor + its tally. Uses [EditorialHeader] like every other settings section,
 *  so the group ranks a size ABOVE its rows (§6) instead of matching their 10sp exactly. */
@Composable
private fun PrefSectionHeader(title: String, summary: String) {
    Row(
        modifier = Modifier.fillMaxWidth(),
        horizontalArrangement = Arrangement.SpaceBetween,
        verticalAlignment = Alignment.Bottom
    ) {
        Box(Modifier.weight(1f)) { SettingsSectionHeader(title, top = 22.dp) }
        Text(
            summary,
            style = MaterialTheme.typography.labelSmall,
            color = MaterialTheme.colorScheme.onSurfaceVariant,
            modifier = Modifier.padding(end = SETTINGS_GUTTER, bottom = 8.dp)
        )
    }
}

@Composable
private fun ExercisePrefRow(
    name: String,
    subtitle: String,
    icon: androidx.compose.ui.graphics.vector.ImageVector,
    pref: Pref,
    onSet: (Pref) -> Unit
) {
    val onBg = MaterialTheme.colorScheme.onBackground
    val muted = MaterialTheme.colorScheme.onSurfaceVariant
    var menuOpen by remember { mutableStateOf(false) }

    Box {
        Row(
            modifier = Modifier
                .fillMaxWidth()
                .clickableLabeled("$name, ${pref.label.lowercase()}. Change preference") { menuOpen = true }
                .padding(horizontal = SETTINGS_GUTTER, vertical = SETTINGS_ROW_PAD),
            verticalAlignment = Alignment.CenterVertically,
            horizontalArrangement = Arrangement.spacedBy(16.dp)
        ) {
            // Equipment glyph — quiet wayfinding (DESIGN §8), same muted tint as the subtitle.
            Icon(icon, contentDescription = null, tint = muted, modifier = Modifier.size(20.dp))
            Column(Modifier.weight(1f), verticalArrangement = Arrangement.spacedBy(2.dp)) {
                Text(name, style = MaterialTheme.typography.bodyMedium, color = onBg)
                SettingsExplainer(subtitle)
            }
            // §8: "a row's right meta is a count or reading only, never a state word" — and §8's
            // dot rule: paint the mark only for the EXCEPTION, never for the neutral majority (a
            // column of identical grey words is the same noise as a grey dot column). Preferred and
            // Hidden get the dot; the ~600 neutral rows reserve the gutter and stay quiet.
            Row(
                verticalAlignment = Alignment.CenterVertically,
                horizontalArrangement = Arrangement.spacedBy(6.dp)
            ) {
                when (pref) {
                    Pref.PREFERRED -> StatusDot(active = true, size = 7.dp)
                    Pref.HIDDEN -> StatusDot(active = false, size = 7.dp)
                    Pref.NEUTRAL -> Spacer(Modifier.size(7.dp))
                }
                Text("▾", style = MaterialTheme.typography.labelMedium, color = muted)
            }
        }
        DropdownMenu(expanded = menuOpen, onDismissRequest = { menuOpen = false }) {
            Pref.entries.forEach { option ->
                DropdownMenuItem(
                    text = {
                        Row(
                            verticalAlignment = Alignment.CenterVertically,
                            horizontalArrangement = Arrangement.spacedBy(10.dp)
                        ) {
                            // A leading dot marks the active state; a blank keeps the labels aligned.
                            Box(Modifier.width(8.dp)) {
                                if (option == pref) StatusDot(active = true, size = 7.dp)
                            }
                            Text(
                                option.label,
                                color = if (option == pref) onBg else muted,
                                style = MaterialTheme.typography.bodyMedium
                            )
                        }
                    },
                    onClick = { onSet(option); menuOpen = false }
                )
            }
        }
    }
}
