@file:OptIn(androidx.compose.material3.ExperimentalMaterial3Api::class)

package com.forge.app.ui.academy

import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.PaddingValues
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.lazy.LazyColumn
import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.automirrored.filled.ArrowBack
import androidx.compose.material3.Icon
import androidx.compose.material3.IconButton
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Scaffold
import androidx.compose.material3.Text
import androidx.compose.material3.TopAppBar
import androidx.compose.material3.TopAppBarDefaults
import androidx.compose.runtime.Composable
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.getValue
import androidx.compose.runtime.remember
import androidx.compose.ui.Modifier
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.text.font.FontStyle
import androidx.compose.ui.unit.dp
import androidx.hilt.navigation.compose.hiltViewModel
import androidx.lifecycle.compose.collectAsStateWithLifecycle
import com.forge.app.domain.academy.ArticleTopic
import com.forge.app.domain.academy.LessonTrack
import com.forge.app.ui.common.EditorialHeader
import com.forge.app.ui.common.statsEntrance
import com.forge.app.ui.experiment.surfacePalette

/**
 * The Academy — one page holding everything the coach knows, browsable end to end.
 *
 * ## What changed, 2026-08-16
 *
 * It was a hub of five gated tracks, each a sub-screen, each reporting its own progress twice. Antho:
 * *"too crowded and behind 50 sub menus, and the worst thing is it feels like achievement, not a hub
 * to knowledge. You should be able to see everything."*
 *
 * So: **no gate** (every lesson readable from install — a fired coach moment now only marks a lesson
 * FOR YOU rather than granting access), **no track screen** (tracks are section headers), **no lens
 * pills** (lessons and articles share one gallery, told apart by a word on each tile, which is what
 * Antho asked for), **no search** and **no progress rails**.
 *
 * The poke survives untouched, because it was never the problem: the notifications feed, the bell
 * count, the tab badge and `ArrivalBannerHost` all still fire on the same ledger events. The only
 * change is that the thing they point at was already readable.
 *
 * ## Section order
 *
 * FOR YOU first when the coach has flagged anything, then the five lesson tracks in reading order,
 * then the Library's articles grouped by topic. Lesson tracks lead because they are the coach's own
 * curriculum; the Library is the wider reading beside it, and neither is hidden from the other.
 */
@Composable
fun AcademyScreen(
    onBack: (() -> Unit)? = null,
    onOpenArticle: (String) -> Unit = {},
    /** Opens straight onto one lesson's sheet — how a notifications-feed row lands here. */
    initialLessonId: String? = null,
    viewModel: AcademyViewModel = hiltViewModel(),
    libraryViewModel: LibraryViewModel = hiltViewModel()
) {
    val state by viewModel.state.collectAsStateWithLifecycle()
    val libraryState by libraryViewModel.state.collectAsStateWithLifecycle()
    val onBg = MaterialTheme.colorScheme.onBackground
    val muted = MaterialTheme.colorScheme.onSurfaceVariant
    val accent = MaterialTheme.colorScheme.primary
    val palette = surfacePalette()

    // Keyed on the id so a second arrival for a different lesson still opens, while a rotation on
    // the same one does not re-open a sheet the reader just dismissed.
    LaunchedEffect(initialLessonId) {
        initialLessonId?.let { viewModel.open(it) }
    }

    LessonSheet(state, viewModel, onBg)

    val sections = remember(state.all, libraryState.articles) {
        buildSections(state, libraryState)
    }
    val total = state.all.size + libraryState.articles.size

    Scaffold(
        topBar = {
            TopAppBar(
                title = {},
                navigationIcon = {
                    if (onBack != null) IconButton(onClick = onBack) {
                        Icon(Icons.AutoMirrored.Filled.ArrowBack, contentDescription = "Back", tint = muted)
                    }
                },
                colors = TopAppBarDefaults.topAppBarColors(containerColor = Color.Transparent)
            )
        },
        containerColor = Color.Transparent
    ) { inner ->
        LazyColumn(
            modifier = Modifier.fillMaxSize().padding(inner),
            contentPadding = PaddingValues(horizontal = 24.dp, vertical = 4.dp)
        ) {
            item("hero") {
                Column(Modifier.fillMaxWidth().statsEntrance(0).padding(vertical = 8.dp)) {
                    // A plain count of what is here. It used to read "4 OF 31 UNLOCKED", which is a
                    // score, and a score is the exact thing this page should not open with.
                    Text(
                        "$total PIECES",
                        style = MaterialTheme.typography.labelMedium,
                        color = muted
                    )
                    Spacer(Modifier.height(2.dp))
                    Text("Academy", style = MaterialTheme.typography.headlineLarge, color = onBg)
                    Spacer(Modifier.height(6.dp))
                    Text(
                        "Everything the coach knows, open from the start.",
                        style = MaterialTheme.typography.bodyMedium.copy(fontStyle = FontStyle.Italic),
                        color = muted
                    )
                }
            }

            sections.forEachIndexed { sectionIndex, section ->
                item("h-${section.label}") {
                    Spacer(Modifier.height(28.dp))
                    EditorialHeader(label = section.label, muted = muted, accent = accent)
                    Spacer(Modifier.height(12.dp))
                }
                val open: (GalleryItem) -> Unit = { item ->
                    when (item) {
                        is GalleryItem.LessonTile -> viewModel.open(item.id)
                        is GalleryItem.ArticleTile -> onOpenArticle(item.id)
                    }
                }
                // Walk the section in the mosaic rhythm: a wide tile, then a row of two posters,
                // repeating. Grouping here rather than in the tile keeps the shape decision in one
                // place — `isWideSlot` is the only thing that knows the pattern.
                var i = 0
                var block = 0
                while (i < section.items.size) {
                    val entrance = sectionIndex + block + 1
                    if (section.featured || isWideSlot(i)) {
                        val item = section.items[i]
                        item("w-${section.label}-$i") {
                            if (i > 0) Spacer(Modifier.height(10.dp))
                            GalleryTile(
                                item = item,
                                palette = palette,
                                onBg = onBg,
                                muted = muted,
                                accent = accent,
                                modifier = Modifier.fillMaxWidth().statsEntrance(entrance),
                                wide = true,
                                // The header above it already says FOR YOU; repeating the mark
                                // inside the tile is the same fact twice in four inches.
                                showForYouMark = !section.featured
                            ) { open(item) }
                        }
                        i += 1
                    } else {
                        val pair = section.items.subList(i, minOf(i + 2, section.items.size))
                        item("p-${section.label}-$i") {
                            Spacer(Modifier.height(10.dp))
                            GalleryRow(
                                pair = pair,
                                palette = palette,
                                onBg = onBg,
                                muted = muted,
                                accent = accent,
                                modifier = Modifier.statsEntrance(entrance),
                                onOpen = open
                            )
                        }
                        i += pair.size
                    }
                    block += 1
                }
            }

            item("tail") { Spacer(Modifier.height(56.dp)) }
        }
    }
}

/**
 * Assemble the gallery.
 *
 * FOR YOU deliberately REPEATS tiles that also appear in their track below. That breaks "a fact has
 * one home on a screen", and it is the right call here: the shelf is the poke, the track section is
 * the shelf's permanent address, and a reader who wants to browse should not have to know which one
 * a flagged lesson fell into. It is capped at [FOR_YOU_CAP] so it stays a nudge rather than becoming
 * a backlog — a nine-item "for you" queue would be the achievement feeling returning by the side
 * door.
 */
private fun buildSections(
    state: AcademyViewModel.UiState,
    library: LibraryViewModel.UiState
): List<GallerySection> = buildList {
    val flagged = state.forYou.take(FOR_YOU_CAP)

    // Skip the poke's own section when that lesson is already the LEAD tile of a track below. Both
    // render full width and large, so the two land within a screen of each other as what looks like
    // the same card printed twice — and at cold start this is the common case, since the first
    // moments to fire are Fundamentals ones. The lead tile keeps its FOR YOU mark, so nothing is
    // lost: the poke is still on the page, just not duplicated.
    val leadsATrack = flagged.any { s ->
        state.lessonsIn(s.lesson.track).firstOrNull()?.lesson?.id == s.lesson.id
    }
    if (flagged.isNotEmpty() && !leadsATrack) {
        add(GallerySection("For you", flagged.map { GalleryItem.LessonTile(it) }, featured = true))
    }

    LessonTrack.entries.forEach { track ->
        val items = state.lessonsIn(track).map { GalleryItem.LessonTile(it) }
        if (items.isNotEmpty()) add(GallerySection(track.displayName, items))
    }

    // Articles group by their own topic rather than being forced into a lesson track: the two
    // taxonomies are genuinely different, and mapping one onto the other would file articles under
    // headings they were not written for.
    ArticleTopic.entries.forEach { topic ->
        val items = library.articles
            .filter { it.article.topic == topic }
            .map { GalleryItem.ArticleTile(it) }
        if (items.isNotEmpty()) add(GallerySection(topic.displayName, items))
    }
}
