@file:OptIn(androidx.compose.material3.ExperimentalMaterial3Api::class)
package com.forge.app.ui.gym.freestyle

import androidx.activity.compose.BackHandler
import androidx.compose.foundation.background
import androidx.compose.foundation.border
import androidx.compose.foundation.horizontalScroll
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.PaddingValues
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.layout.width
import androidx.compose.foundation.lazy.grid.GridCells
import androidx.compose.foundation.lazy.grid.GridItemSpan
import androidx.compose.foundation.lazy.grid.LazyVerticalGrid
import androidx.compose.foundation.lazy.grid.items
import androidx.compose.foundation.rememberScrollState
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.foundation.text.BasicTextField
import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.automirrored.filled.ArrowBack
import androidx.compose.material.icons.filled.Close
import androidx.compose.material.icons.filled.Search
import androidx.compose.material3.Icon
import androidx.compose.material3.IconButton
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Scaffold
import androidx.compose.material3.Text
import androidx.compose.material3.TopAppBar
import androidx.compose.material3.TopAppBarDefaults
import androidx.compose.runtime.Composable
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.clip
import androidx.compose.ui.graphics.Brush
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.graphics.SolidColor
import androidx.compose.ui.graphics.lerp
import androidx.compose.ui.semantics.Role
import androidx.compose.ui.semantics.contentDescription
import androidx.compose.ui.semantics.semantics
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.text.style.TextOverflow
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp
import androidx.hilt.navigation.compose.hiltViewModel
import androidx.lifecycle.compose.collectAsStateWithLifecycle
import com.forge.app.program.ExerciseDef
import com.forge.app.program.ExerciseLibrary
import com.forge.app.program.MuscleGroup
import com.forge.app.ui.common.EditorialHeader
import com.forge.app.ui.common.ForgeOutlineCapsule
import com.forge.app.ui.common.ForgePrimaryCapsule
import com.forge.app.ui.common.InlineEmptyHint
import com.forge.app.ui.common.bounceClick
import com.forge.app.ui.common.clickableLabeled
import com.forge.app.ui.gym.stats.components.FullBodyFigure
import com.forge.app.ui.gym.stats.components.MuscleFigure

/**
 * Pure library filter for the browser — muscle/search/favorites, minus the already-added [exclude]
 * and owner-only plate stations ([ExerciseDef.curatedOnly]). The async bits (favorites set, recency)
 * come from [ExerciseBrowserViewModel]; everything derivable from the query stays here so it's testable.
 */
fun browseLibrary(
    query: String,
    muscle: MuscleGroup?,
    favoritesOnly: Boolean,
    favorites: Set<String>,
    exclude: Set<String>
): List<ExerciseDef> {
    val q = query.trim().lowercase()
    return ExerciseLibrary.all.filter { def ->
        !def.curatedOnly && def.id !in exclude &&
            (muscle == null || def.muscle == muscle) &&
            (!favoritesOnly || def.id in favorites) &&
            (q.isEmpty() || def.name.lowercase().contains(q) || def.muscle.displayName.lowercase().contains(q))
    }
}

/** The Recently-performed rail minus the moves already on the log, so Recent can never offer a
 *  duplicate of something the grid is already hiding. */
fun recentForBrowser(recent: List<ExerciseDef>, exclude: Set<String>): List<ExerciseDef> =
    recent.filter { it.id !in exclude }

/** Prefix marking a user-created move. Distinct from the program editor's `custom_…` ids (a
 *  different table) and from the importer's `ext-…`, so the three never alias each other. */
private const val CUSTOM_PREFIX = "custom-"

/** True when this logged-exercise id is a move the user created rather than a library one. */
fun isCustomExerciseId(id: String): Boolean = id.startsWith(CUSTOM_PREFIX)

/** Longest slug an id carries; anything past it is identified by the digest instead. */
private const val CUSTOM_SLUG_MAX = 40

/**
 * A stable id for a user-created move, keyed on its name — so creating "Sled Push" again next week
 * lands on the same id and its sets group with the earlier ones in history/PRs/stats. Mirrors the
 * importer's synthetic-id slugging for the same reason.
 *
 * An ordinary name (one whose slug is non-empty and fits) keeps exactly the id it always had, so
 * existing history still groups. The slug alone was lossy in two cases, and both merged DISTINCT
 * movements under one id, one inheriting the other's last sets and PR frontier: a punctuation-only
 * name ("!!!" and "@@@" both slugged to nothing), and two names sharing their first 40 slug
 * characters. Those ids now carry a short digest of the full canonical name after the slug, so
 * two different names can no longer share an id while one name still always maps to one id.
 */
fun customExerciseId(name: String): String {
    val canonical = canonicalCustomExerciseName(name)
    val fullSlug = canonical
        .map { if (it.isLetterOrDigit()) it else '-' }
        .joinToString("")
        .replace(Regex("-+"), "-")
        .trim('-')
    if (fullSlug.isNotEmpty() && fullSlug.length <= CUSTOM_SLUG_MAX) return CUSTOM_PREFIX + fullSlug
    val slug = fullSlug.take(CUSTOM_SLUG_MAX).trimEnd('-').ifBlank { "exercise" }
    return CUSTOM_PREFIX + slug + "-" + customNameDigest(canonical)
}

/** The name as identity: trimmed, lower-cased, inner whitespace collapsed to one space. */
fun canonicalCustomExerciseName(name: String): String =
    name.trim().lowercase().split(Regex("\\s+")).filter { it.isNotEmpty() }.joinToString(" ")

/**
 * Eight hex characters of FNV-1a over the canonical name. Stable across processes and builds
 * (unlike an identity hash) and short enough to sit in an id; a full hash would be overkill for
 * telling apart the handful of moves one person names alike.
 */
private fun customNameDigest(canonical: String): String {
    var h = 0x811C9DC5.toInt()
    for (c in canonical) {
        h = h xor c.code
        h *= 0x01000193
    }
    return String.format(java.util.Locale.US, "%08x", h)
}

/**
 * The freestyle exercise browser (GYMAP-27): a full-screen List/browser that replaces the old search
 * dialog. Anatomy muscle-filter row → search → most-performed → a grid of anatomy-thumbnail tiles you
 * multi-select and add. Overlaid over the logger, so it paints its own background + owns system back.
 */
@Composable
fun ExerciseBrowserScreen(
    exclude: Set<String>,
    onClose: () -> Unit,
    onConfirm: (Set<String>) -> Unit,
    /** Create a move the library doesn't have (name, target muscle) and add it to the log. */
    onCreateCustom: (String, MuscleGroup) -> Unit,
    viewModel: ExerciseBrowserViewModel = hiltViewModel()
) {
    val favorites by viewModel.favorites.collectAsStateWithLifecycle()
    val recent by viewModel.recent.collectAsStateWithLifecycle()

    var query by remember { mutableStateOf("") }
    var muscle by remember { mutableStateOf<MuscleGroup?>(null) }
    var favoritesOnly by remember { mutableStateOf(false) }
    var picked by remember { mutableStateOf(emptySet<String>()) }

    val results = remember(query, muscle, favoritesOnly, favorites, exclude) {
        browseLibrary(query, muscle, favoritesOnly, favorites, exclude)
    }
    val unfiltered = query.isBlank() && muscle == null && !favoritesOnly
    // The grid already hides what's on the log; Recent has to as well, or picking a move from
    // both rails appends a duplicate id and the keyed lazy list throws on measure.
    val recentShown = remember(recent, exclude) { recentForBrowser(recent, exclude) }
    val showRecent = unfiltered && recentShown.isNotEmpty()
    val sectionLabel = when {
        query.isNotBlank() -> "Results · ${results.size}"
        favoritesOnly -> "Favorites · ${results.size}"
        muscle != null -> "${muscle!!.displayName} · ${results.size}"
        else -> "All · ${results.size}"
    }

    BackHandler(onBack = onClose)

    val amoled = com.forge.app.ui.theme.LocalForgeSettings.current.amoledMode
    val (gradTop, gradBottom) = com.forge.app.ui.theme.forgeBackgroundGradient(amoled)

    Box(Modifier.fillMaxSize().background(Brush.verticalGradient(listOf(gradTop, gradBottom)))) {
        Scaffold(
            containerColor = Color.Transparent,
            topBar = {
                TopAppBar(
                    // §4.6: bell + back, never the screen's name.
                    title = {},
                    navigationIcon = {
                        IconButton(onClick = onClose) { Icon(Icons.AutoMirrored.Filled.ArrowBack, "Close") }
                    },
                    colors = TopAppBarDefaults.topAppBarColors(containerColor = Color.Transparent)
                )
            },
            bottomBar = {
                ForgePrimaryCapsule(
                    if (picked.isEmpty()) "Add" else "Add ${picked.size}",
                    onClick = { onConfirm(picked) },
                    enabled = picked.isNotEmpty(),
                    modifier = Modifier.fillMaxWidth().padding(horizontal = 24.dp, vertical = 16.dp)
                )
            }
        ) { inner ->
            LazyVerticalGrid(
                columns = GridCells.Fixed(2),
                modifier = Modifier.fillMaxSize().padding(inner),
                contentPadding = PaddingValues(horizontal = 24.dp, vertical = 8.dp),
                horizontalArrangement = Arrangement.spacedBy(12.dp),
                verticalArrangement = Arrangement.spacedBy(12.dp)
            ) {
                val full: androidx.compose.foundation.lazy.grid.LazyGridItemSpanScope.() -> GridItemSpan =
                    { GridItemSpan(maxLineSpan) }

                item(span = full) {
                    Column {
                        Text(
                            "ADD TO YOUR LOG",
                            style = MaterialTheme.typography.labelMedium,
                            color = MaterialTheme.colorScheme.onSurfaceVariant,
                            letterSpacing = 1.sp
                        )
                        Text(
                            "Exercises",
                            style = MaterialTheme.typography.headlineSmall,
                            color = MaterialTheme.colorScheme.onSurface
                        )
                    }
                }
                item(span = full) {
                    MuscleFilterRow(
                        selected = muscle,
                        favoritesOnly = favoritesOnly,
                        onSelectMuscle = { m -> muscle = m; favoritesOnly = false },
                        onToggleFavorites = { favoritesOnly = !favoritesOnly; if (favoritesOnly) muscle = null }
                    )
                }
                item(span = full) {
                    BrowserSearchField(query = query, onQueryChange = { query = it })
                }
                if (showRecent) {
                    item(span = full) {
                        Column {
                            EditorialHeader(
                                "Recently performed",
                                muted = MaterialTheme.colorScheme.onSurfaceVariant,
                                accent = MaterialTheme.colorScheme.primary
                            )
                            Spacer(Modifier.height(10.dp))
                            Row(
                                Modifier.horizontalScroll(rememberScrollState()),
                                horizontalArrangement = Arrangement.spacedBy(12.dp)
                            ) {
                                recentShown.forEach { def ->
                                    ExerciseTile(
                                        def = def,
                                        selected = def.id in picked,
                                        favorite = def.id in favorites,
                                        onSelect = { picked = if (def.id in picked) picked - def.id else picked + def.id },
                                        onToggleFavorite = { viewModel.toggleFavorite(def.id, def.id !in favorites) },
                                        modifier = Modifier.width(136.dp)
                                    )
                                }
                            }
                        }
                    }
                }
                item(span = full) {
                    Spacer(Modifier.height(2.dp))
                    EditorialHeader(
                        sectionLabel,
                        muted = MaterialTheme.colorScheme.onSurfaceVariant,
                        accent = MaterialTheme.colorScheme.primary
                    )
                }
                items(results, key = { it.id }) { def ->
                    ExerciseTile(
                        def = def,
                        selected = def.id in picked,
                        favorite = def.id in favorites,
                        onSelect = { picked = if (def.id in picked) picked - def.id else picked + def.id },
                        onToggleFavorite = { viewModel.toggleFavorite(def.id, def.id !in favorites) }
                    )
                }
                if (results.isEmpty()) {
                    item(span = full) {
                        // A search that finds nothing is the moment to offer a custom move — the
                        // library will never cover every gym, and the log shouldn't dead-end here.
                        if (query.isNotBlank() && customExerciseId(query.trim()) in exclude) {
                            // They already created this one and it's sitting in the log — say so
                            // rather than offering a "Create" that would silently do nothing.
                            InlineEmptyHint(
                                "\u201C${query.trim()}\u201D is already in your log.",
                                color = MaterialTheme.colorScheme.onSurfaceVariant
                            )
                        } else if (query.isNotBlank()) {
                            CreateCustomCard(
                                name = query.trim(),
                                // Seed from the muscle filter if one is on — usually the right guess.
                                initialMuscle = muscle ?: MuscleGroup.entries.first(),
                                onCreate = { m -> onCreateCustom(query.trim(), m) }
                            )
                        } else {
                            InlineEmptyHint(
                                if (favoritesOnly) "No favorites yet. Tap a star to bookmark a move."
                                else "No exercises match.",
                                color = MaterialTheme.colorScheme.onSurfaceVariant
                            )
                        }
                    }
                }
            }
        }
    }
}

/**
 * The dead-end rescue: when a search matches nothing, offer to create the typed name as a custom
 * move so the workout can still be logged. Target muscle is a one-tap row (seeded from the active
 * muscle filter) — it only drives the tile figure and the meta line, so a wrong guess costs nothing.
 */
@Composable
private fun CreateCustomCard(name: String, initialMuscle: MuscleGroup, onCreate: (MuscleGroup) -> Unit) {
    val cs = MaterialTheme.colorScheme
    var picked by remember(initialMuscle) { mutableStateOf(initialMuscle) }
    Column(
        Modifier
            .fillMaxWidth()
            .clip(RoundedCornerShape(12.dp))
            .background(cs.surfaceVariant)
            .padding(16.dp)
    ) {
        Text(
            "No exercises match \u201C$name\u201D",
            style = MaterialTheme.typography.bodyMedium,
            fontWeight = FontWeight.SemiBold,
            color = cs.onSurface
        )
        Spacer(Modifier.height(4.dp))
        Text(
            "Do you want to create it as a custom exercise? You can log it just like any other move.",
            style = MaterialTheme.typography.bodySmall,
            color = cs.onSurfaceVariant
        )
        Spacer(Modifier.height(14.dp))
        Text(
            "TARGET MUSCLE",
            style = MaterialTheme.typography.labelSmall,
            color = cs.onSurfaceVariant,
            letterSpacing = 1.sp
        )
        Spacer(Modifier.height(8.dp))
        Row(
            Modifier.fillMaxWidth().horizontalScroll(rememberScrollState()),
            horizontalArrangement = Arrangement.spacedBy(8.dp)
        ) {
            MuscleGroup.entries.forEach { m ->
                val sel = m == picked
                Text(
                    m.displayName,
                    style = MaterialTheme.typography.labelMedium,
                    color = if (sel) cs.primary else cs.onSurfaceVariant,
                    modifier = Modifier
                        .clip(RoundedCornerShape(50))
                        .background(if (sel) cs.primaryContainer else Color.Transparent)
                        .then(if (sel) Modifier.border(1.dp, cs.primary, RoundedCornerShape(50)) else Modifier)
                        .clickableLabeled("${m.displayName} target", role = Role.RadioButton) { picked = m }
                        .padding(horizontal = 12.dp, vertical = 6.dp)
                )
            }
        }
        Spacer(Modifier.height(14.dp))
        ForgeOutlineCapsule(
            "Create \u201C$name\u201D",
            onClick = { onCreate(picked) },
            modifier = Modifier.fillMaxWidth()
        )
    }
}

/** The anatomy muscle-filter row: a Favorites star, an All chip, then one lit-muscle figure per group. */
@Composable
private fun MuscleFilterRow(
    selected: MuscleGroup?,
    favoritesOnly: Boolean,
    onSelectMuscle: (MuscleGroup?) -> Unit,
    onToggleFavorites: () -> Unit
) {
    val cs = MaterialTheme.colorScheme
    Row(
        Modifier.fillMaxWidth().horizontalScroll(rememberScrollState()),
        horizontalArrangement = Arrangement.spacedBy(8.dp)
    ) {
        GlyphFilterChip(glyph = if (favoritesOnly) "★" else "☆", label = "Saved", selected = favoritesOnly, onClick = onToggleFavorites)
        AllFilterChip(selected = selected == null && !favoritesOnly, onClick = { onSelectMuscle(null) })
        MuscleGroup.entries.forEach { m ->
            val isSel = selected == m
            Column(
                horizontalAlignment = Alignment.CenterHorizontally,
                modifier = Modifier
                    .clip(RoundedCornerShape(12.dp))
                    .background(if (isSel) cs.primaryContainer else Color.Transparent)
                    .bounceClick { onSelectMuscle(m) }
                    .semantics { contentDescription = "${m.displayName} filter" }
                    .padding(horizontal = 8.dp, vertical = 6.dp)
            ) {
                MuscleFigure(
                    muscle = m,
                    lit = if (isSel) lerp(cs.primary, cs.onSurface, 0.32f) else cs.onSurfaceVariant,
                    body = cs.onSurfaceVariant.copy(alpha = 0.12f),
                    detail = cs.onSurfaceVariant.copy(alpha = 0.28f),
                    // Width generous enough that BOTH the torso (wide) and leg (narrow) figures fill
                    // this height — so legs read the same length as torso, and all figure boxes match.
                    modifier = Modifier.width(58.dp).height(40.dp)
                )
                Spacer(Modifier.height(4.dp))
                Text(
                    m.displayName,
                    style = MaterialTheme.typography.labelSmall,
                    color = if (isSel) cs.primary else cs.onSurfaceVariant
                )
            }
        }
    }
}

/** The "All" chip — a full-body figure (every muscle) that lights up whole when selected. */
@Composable
private fun AllFilterChip(selected: Boolean, onClick: () -> Unit) {
    val cs = MaterialTheme.colorScheme
    Column(
        horizontalAlignment = Alignment.CenterHorizontally,
        modifier = Modifier
            .clip(RoundedCornerShape(12.dp))
            .background(if (selected) cs.primaryContainer else Color.Transparent)
            .bounceClick { onClick() }
            .semantics { contentDescription = "All exercises filter" }
            .padding(horizontal = 8.dp, vertical = 6.dp)
    ) {
        Box(Modifier.width(58.dp).height(40.dp), contentAlignment = Alignment.Center) {
            FullBodyFigure(
                body = cs.onSurfaceVariant.copy(alpha = 0.12f),
                detail = if (selected) lerp(cs.primary, cs.onSurface, 0.32f) else cs.onSurfaceVariant,
                modifier = Modifier.fillMaxSize()
            )
        }
        Spacer(Modifier.height(4.dp))
        Text("All", style = MaterialTheme.typography.labelSmall, color = if (selected) cs.primary else cs.onSurfaceVariant)
    }
}

/** A text-glyph filter chip (Saved) matching the muscle chips' selected wash. */
@Composable
private fun GlyphFilterChip(glyph: String, label: String, selected: Boolean, onClick: () -> Unit) {
    val cs = MaterialTheme.colorScheme
    Column(
        horizontalAlignment = Alignment.CenterHorizontally,
        modifier = Modifier
            .clip(RoundedCornerShape(12.dp))
            .background(if (selected) cs.primaryContainer else Color.Transparent)
            .bounceClick { onClick() }
            .semantics { contentDescription = "$label filter" }
            .padding(horizontal = 8.dp, vertical = 6.dp)
    ) {
        // Same box as the muscle figures so every chip's label lands on the same line.
        Box(Modifier.width(58.dp).height(40.dp), contentAlignment = Alignment.Center) {
            Text(glyph, style = MaterialTheme.typography.titleLarge, color = if (selected) cs.primary else cs.onSurfaceVariant)
        }
        Spacer(Modifier.height(4.dp))
        Text(label, style = MaterialTheme.typography.labelSmall, color = if (selected) cs.primary else cs.onSurfaceVariant)
    }
}

/** One exercise tile: anatomy thumbnail (target muscle lit) + name + muscle, with a bookmark star. */
@Composable
private fun ExerciseTile(
    def: ExerciseDef,
    selected: Boolean,
    favorite: Boolean,
    onSelect: () -> Unit,
    onToggleFavorite: () -> Unit,
    modifier: Modifier = Modifier
) {
    val cs = MaterialTheme.colorScheme
    Column(
        modifier = modifier
            .height(190.dp)
            .clip(RoundedCornerShape(12.dp))
            .background(if (selected) cs.primaryContainer else cs.surfaceVariant)
            .then(if (selected) Modifier.border(1.dp, cs.primary, RoundedCornerShape(12.dp)) else Modifier)
            .bounceClick { onSelect() }
            .semantics { contentDescription = "${def.name}, ${def.muscle.displayName}${if (selected) ", selected" else ""}" }
            .padding(14.dp)
    ) {
        // The figure claims the flexible top space so every tile is the same height regardless of name.
        Box(Modifier.fillMaxWidth().weight(1f)) {
            MuscleFigure(
                muscle = def.muscle,
                // Lift the accent toward onBg so the muscle reads on the grey body (muted navy vanished).
                lit = lerp(cs.primary, cs.onSurface, 0.32f),
                body = cs.onSurfaceVariant.copy(alpha = 0.12f),
                detail = cs.onSurfaceVariant.copy(alpha = 0.3f),
                modifier = Modifier.fillMaxSize()
            )
            Text(
                if (favorite) "★" else "☆",
                style = MaterialTheme.typography.titleMedium,
                color = if (favorite) cs.primary else cs.onSurfaceVariant,
                modifier = Modifier
                    .align(Alignment.TopEnd)
                    .clip(RoundedCornerShape(50))
                    .clickableLabeled(if (favorite) "Remove bookmark" else "Bookmark", role = Role.Checkbox, onClick = onToggleFavorite)
                    .padding(4.dp)
            )
        }
        Spacer(Modifier.height(6.dp))
        // Reserve two lines for every name so 1- and 2-line titles occupy identical space (§ uniform grid).
        Text(
            def.name,
            style = MaterialTheme.typography.bodyMedium,
            fontWeight = FontWeight.SemiBold,
            color = cs.onSurface,
            minLines = 2,
            maxLines = 2,
            overflow = TextOverflow.Ellipsis
        )
        Text(
            def.muscle.displayName,
            style = MaterialTheme.typography.labelMedium,
            color = cs.onSurfaceVariant
        )
    }
}

/** Filled, rounded search field (§13) — leading magnifier, trailing clear, muted ghost placeholder. */
@Composable
private fun BrowserSearchField(query: String, onQueryChange: (String) -> Unit) {
    val onBg = MaterialTheme.colorScheme.onSurface
    val muted = MaterialTheme.colorScheme.onSurfaceVariant
    Row(
        modifier = Modifier
            .fillMaxWidth()
            .clip(RoundedCornerShape(12.dp))
            .background(MaterialTheme.colorScheme.surfaceVariant)
            .padding(horizontal = 14.dp, vertical = 12.dp),
        verticalAlignment = Alignment.CenterVertically,
        horizontalArrangement = Arrangement.spacedBy(10.dp)
    ) {
        Icon(Icons.Filled.Search, contentDescription = null, tint = muted, modifier = Modifier.size(20.dp))
        BasicTextField(
            value = query,
            onValueChange = onQueryChange,
            textStyle = MaterialTheme.typography.bodyMedium.copy(color = onBg),
            cursorBrush = SolidColor(onBg),
            singleLine = true,
            modifier = Modifier.weight(1f),
            decorationBox = { inner ->
                Box {
                    if (query.isEmpty()) {
                        Text("Search exercises or muscles", style = MaterialTheme.typography.bodyMedium, color = muted.copy(alpha = 0.6f))
                    }
                    inner()
                }
            }
        )
        if (query.isNotEmpty()) {
            Icon(
                Icons.Filled.Close,
                contentDescription = "Clear search",
                tint = muted,
                modifier = Modifier.size(20.dp).clip(RoundedCornerShape(50)).clickableLabeled("Clear search") { onQueryChange("") }
            )
        }
    }
}
