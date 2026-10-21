package com.forge.app.ui.profile

import androidx.compose.foundation.background
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.WindowInsets
import androidx.compose.foundation.layout.asPaddingValues
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.heightIn
import androidx.compose.foundation.layout.navigationBars
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.layout.statusBars
import androidx.compose.foundation.rememberScrollState
import androidx.compose.foundation.text.BasicTextField
import androidx.compose.foundation.text.KeyboardActions
import androidx.compose.foundation.text.KeyboardOptions
import androidx.compose.foundation.verticalScroll
import androidx.compose.material3.Icon
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.Stable
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.saveable.rememberSaveable
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.drawWithContent
import androidx.compose.ui.focus.FocusRequester
import androidx.compose.ui.focus.focusRequester
import androidx.compose.ui.focus.onFocusChanged
import androidx.compose.ui.geometry.Offset
import androidx.compose.ui.graphics.BlendMode
import androidx.compose.ui.graphics.CompositingStrategy
import androidx.compose.ui.graphics.Shadow
import androidx.compose.ui.graphics.SolidColor
import androidx.compose.ui.graphics.graphicsLayer
import androidx.compose.ui.semantics.heading
import androidx.compose.ui.semantics.semantics
import androidx.compose.ui.text.input.ImeAction
import androidx.compose.ui.unit.dp
import com.forge.app.data.db.entities.BodyFatEntry
import com.forge.app.data.db.entities.BodyweightEntry
import com.forge.app.data.db.entities.LeanMassEntry
import com.forge.app.data.repo.ProgressPhoto
import com.forge.app.domain.units.WeightUnit
import com.forge.app.ui.common.OpenLabel
import com.forge.app.ui.common.OpenTabs
import com.forge.app.ui.common.OpenTopBarIcon
import com.forge.app.ui.common.clickableLabeled
import com.forge.app.ui.profile.state.ProfileUiState
import com.forge.app.ui.theme.OpenColors
import com.forge.app.ui.theme.OpenIcons
import com.forge.app.ui.theme.OpenType
import com.forge.app.ui.theme.ProfileColors
import java.time.LocalDate

/*
 * # Profile, the open page (2026-10-21 redesign, Relay pf1 / pf2 / ph / lk4)
 *
 * Your cover across the top, your name in serif over its foot, and three tabs: Overview (all time,
 * the lifetime curve, consistency month by month), Body (weight, then composition and size three
 * across) and Photos (then and now, the camera, the gallery by month).
 *
 * Ranks and trophies are gone from the page (Antho, 2026-10-21). The days grid that was Stats'
 * "Days" lens lives here now, as Overview's Consistency month, and opens the same day sheet.
 *
 * The gallery lock gates the Photos tab IN PLACE: the cover, the name and the other two tabs stay
 * open, the count shows, and nothing of a photo is composed until the session is unlocked.
 */

private val GUTTER = 24.dp

/** The cover's height below the status bar (the design's 250 less its 36dp status bar). */
private val COVER_HEIGHT = 214.dp

internal enum class ProfileTab(val label: String) { OVERVIEW("Overview"), BODY("Body"), PHOTOS("Photos") }

/** Everything the Body tab reads. */
internal data class ProfileBody(
    val weight: List<BodyweightEntry> = emptyList(),
    val goalLb: Double? = null,
    val bodyFat: List<BodyFatEntry> = emptyList(),
    val leanMass: List<LeanMassEntry> = emptyList(),
    val leanMassConnected: Boolean = false,
    val measurements: BodyMeasurementsUiState = BodyMeasurementsUiState(),
)

/** Everything the page can do, so it renders from plain data in a golden. */
@Stable
internal class ProfileActions(
    val onBack: (() -> Unit)? = null,
    val onPickCover: () -> Unit = {},
    val onSetName: (String) -> Unit = {},
    val onDayTap: (LocalDate) -> Unit = {},
    val onLogWeight: () -> Unit = {},
    val onLogBodyFat: () -> Unit = {},
    val onSyncMuscle: () -> Unit = {},
    val onLogSize: () -> Unit = {},
    val onOpenSizes: () -> Unit = {},
    val onOpenGallery: () -> Unit = {},
    val onOpenCamera: () -> Unit = {},
    val onUnlock: () -> Unit = {},
)

@Composable
internal fun ProfilePage(
    state: ProfileUiState,
    body: ProfileBody,
    weightUnit: WeightUnit,
    galleryLocked: Boolean,
    today: LocalDate,
    nowMs: Long,
    planOn: (LocalDate) -> Boolean?,
    onVacation: (LocalDate) -> Boolean,
    actions: ProfileActions,
    /** Draws the cover photo; null draws the placeholder that invites one. */
    cover: (@Composable (Modifier) -> Unit)?,
    /** Draws one progress photo. Never called while [galleryLocked]. */
    photo: @Composable (ProgressPhoto, Modifier) -> Unit,
    modifier: Modifier = Modifier,
    unlockNote: String? = null,
    initialTab: ProfileTab = ProfileTab.OVERVIEW,
    showCoverHint: Boolean = false,
) {
    var tab by rememberSaveable { mutableStateOf(initialTab) }
    val statusInset = WindowInsets.statusBars.asPaddingValues().calculateTopPadding()
    val navInset = WindowInsets.navigationBars.asPaddingValues().calculateBottomPadding()

    Column(modifier.fillMaxSize().verticalScroll(rememberScrollState())) {
        Box(Modifier.fillMaxWidth()) {
            ProfileCover(cover, actions.onPickCover, Modifier.fillMaxWidth().height(statusInset + COVER_HEIGHT))
            Column(Modifier.fillMaxWidth().padding(horizontal = GUTTER)) {
                Spacer(Modifier.height(statusInset + 4.dp))
                ProfileHeader(state, actions, showCoverHint)
                Spacer(Modifier.height(28.dp))
                OpenTabs(ProfileTab.entries, tab, { it.label }, { tab = it })
            }
        }
        Column(Modifier.fillMaxWidth().padding(horizontal = GUTTER)) {
            when (tab) {
                ProfileTab.OVERVIEW -> {
                    Spacer(Modifier.height(36.dp))
                    ProfileOverviewTab(state, weightUnit, today, nowMs, planOn, onVacation, actions.onDayTap)
                }
                ProfileTab.BODY -> {
                    Spacer(Modifier.height(36.dp))
                    ProfileBodyTab(body, weightUnit, nowMs, actions)
                }
                ProfileTab.PHOTOS -> if (galleryLocked) {
                    Spacer(Modifier.height(40.dp))
                    ProfilePhotosLocked(state.photos.size, unlockNote, actions.onUnlock)
                } else {
                    Spacer(Modifier.height(36.dp))
                    ProfilePhotosTab(state.photos, weightUnit, today, actions, photo)
                }
            }
            Spacer(Modifier.height(56.dp + navInset))
        }
    }
}

/**
 * The cover: your photo across the top, dissolving into the page at its foot, or, before you have
 * chosen one, a quiet ground that says what goes there. Tapping it opens the cover chooser.
 */
@Composable
private fun ProfileCover(cover: (@Composable (Modifier) -> Unit)?, onPick: () -> Unit, modifier: Modifier) {
    Box(modifier.clickableLabeled("Change cover photo", onClick = onPick)) {
        if (cover != null) {
            Box(
                Modifier
                    .fillMaxSize()
                    .graphicsLayer { compositingStrategy = CompositingStrategy.Offscreen }
                    .drawWithContent {
                        drawContent()
                        drawRect(brush = ProfileColors.coverFade, blendMode = BlendMode.DstIn)
                    }
            ) { cover(Modifier.fillMaxSize()) }
        } else {
            Box(Modifier.fillMaxSize().background(ProfileColors.coverPlaceholder))
            Column(
                Modifier.align(Alignment.Center).padding(bottom = 10.dp),
                horizontalAlignment = Alignment.CenterHorizontally
            ) {
                Icon(OpenIcons.Image, contentDescription = null, tint = OpenColors.Bar, modifier = Modifier.size(24.dp))
                Spacer(Modifier.height(8.dp))
                OpenLabel("Your cover photo", color = OpenColors.Glyph)
            }
        }
    }
}

/** The legibility halo behind the name and meta, for a bright cover photo. */
private val NameShadow = Shadow(color = ProfileColors.NameHalo, offset = Offset(0f, 2f), blurRadius = 14f)

/** Back and edit, then your name in serif over the cover's foot, and when you started. */
@Composable
private fun ProfileHeader(state: ProfileUiState, actions: ProfileActions, showCoverHint: Boolean) {
    val ink = MaterialTheme.colorScheme.onBackground
    var editing by remember { mutableStateOf(false) }
    var input by remember { mutableStateOf("") }
    fun startEditing() { input = state.name; editing = true }

    Row(Modifier.fillMaxWidth().height(48.dp), verticalAlignment = Alignment.CenterVertically) {
        if (actions.onBack != null) OpenTopBarIcon(OpenIcons.ArrowLeft, "Back", actions.onBack)
        Spacer(Modifier.weight(1f))
        OpenTopBarIcon(OpenIcons.Edit, "Edit your name", { startEditing() })
    }
    Spacer(Modifier.height(130.dp))
    if (editing) {
        val focus = remember { FocusRequester() }
        var hadFocus by remember { mutableStateOf(false) }
        LaunchedEffect(Unit) { focus.requestFocus() }
        // Commit on Done or on focus loss so a typed name is never silently lost; a blank one is
        // ignored rather than wiping you back to "Athlete".
        fun commit() {
            val trimmed = input.trim()
            if (trimmed.isNotEmpty() && trimmed != state.name) actions.onSetName(trimmed)
            editing = false
        }
        BasicTextField(
            value = input,
            onValueChange = { input = it.take(30) },
            singleLine = true,
            textStyle = OpenType.title.copy(color = ink, shadow = NameShadow),
            cursorBrush = SolidColor(MaterialTheme.colorScheme.primary),
            keyboardOptions = KeyboardOptions(imeAction = ImeAction.Done),
            keyboardActions = KeyboardActions(onDone = { commit() }),
            modifier = Modifier.fillMaxWidth().focusRequester(focus).onFocusChanged {
                if (it.isFocused) hadFocus = true else if (hadFocus && editing) commit()
            }
        )
    } else {
        Text(
            state.name.ifBlank { "Athlete" },
            style = OpenType.title.copy(shadow = NameShadow),
            color = ink,
            modifier = Modifier.semantics { heading() }.clickableLabeled("Edit your name") { startEditing() }
        )
    }
    Spacer(Modifier.height(8.dp))
    val meta = when {
        showCoverHint -> "Tap the cover to change it"
        state.sinceLabel.isNotBlank() -> "Training since ${state.sinceLabel}"
        else -> "Your first workout starts the record"
    }
    OpenLabel(meta, style = OpenType.monoXS.copy(shadow = NameShadow))
}

/**
 * A serif figure with its unit that steps down a size rather than break: a narrow column at a
 * large font scale split "148.2lb" into "148.2l" over "b". Each of [sizes] is a (figure, unit)
 * style pair, largest first; the first that sets on one line wins, and the last one wraps.
 */
@Composable
internal fun FittedFigure(
    figure: String,
    unit: String,
    sizes: List<Pair<androidx.compose.ui.text.TextStyle, androidx.compose.ui.text.TextStyle>>,
    modifier: Modifier = Modifier,
) {
    val ink = MaterialTheme.colorScheme.onBackground
    val muted = MaterialTheme.colorScheme.onSurfaceVariant
    val measurer = androidx.compose.ui.text.rememberTextMeasurer()
    fun text(p: Pair<androidx.compose.ui.text.TextStyle, androidx.compose.ui.text.TextStyle>) =
        androidx.compose.ui.text.buildAnnotatedString {
            append(figure)
            pushStyle(p.second.toSpanStyle().copy(color = muted)); append(unit); pop()
        }
    // Held at the largest size's line height, so a stepped-down figure keeps its neighbours' rows.
    val minHeight = with(androidx.compose.ui.platform.LocalDensity.current) { sizes.first().first.lineHeight.toDp() }
    androidx.compose.foundation.layout.BoxWithConstraints(
        modifier.heightIn(min = minHeight),
        contentAlignment = Alignment.BottomStart
    ) {
        val max = constraints.maxWidth
        val pick = sizes.firstOrNull { p ->
            measurer.measure(text(p), p.first, softWrap = false).size.width <= max
        } ?: sizes.last()
        Text(text(pick), style = pick.first, color = ink)
    }
}
