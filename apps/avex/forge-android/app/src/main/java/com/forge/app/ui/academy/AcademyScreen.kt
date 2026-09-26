@file:OptIn(
    androidx.compose.material3.ExperimentalMaterial3Api::class,
    androidx.compose.foundation.layout.ExperimentalLayoutApi::class
)

package com.forge.app.ui.academy

import androidx.compose.animation.AnimatedContent
import androidx.compose.animation.SizeTransform
import androidx.compose.animation.fadeIn
import androidx.compose.animation.fadeOut
import androidx.compose.animation.togetherWith
import androidx.compose.foundation.background
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.FlowRow
import androidx.compose.foundation.layout.PaddingValues
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.heightIn
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.layout.width
import androidx.compose.foundation.lazy.LazyColumn
import androidx.compose.foundation.shape.CircleShape
import androidx.compose.foundation.shape.RoundedCornerShape
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
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.saveable.rememberSaveable
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.input.nestedscroll.nestedScroll
import androidx.compose.ui.draw.clip
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.semantics.heading
import androidx.compose.ui.semantics.semantics
import androidx.compose.ui.unit.dp
import androidx.hilt.navigation.compose.hiltViewModel
import androidx.lifecycle.compose.collectAsStateWithLifecycle
import com.forge.app.domain.academy.AcademyRegistry
import com.forge.app.domain.academy.LessonTrack
import com.forge.app.domain.academy.readMinutes
import com.forge.app.ui.common.LocalTouchExplorationEnabled
import com.forge.app.ui.common.bounceClick
import com.forge.app.ui.theme.ForgeMotion
import kotlinx.coroutines.delay

/**
 * # The Academy — a contents page taught by drawings (2026-09-26)
 *
 * Antho, on the page this replaced: *"I hate it ... the knowledge given, the UX the UI the design I
 * hate everything"*, and *"it's too many"*. The gallery before it held 35 pieces in nine sections
 * (31 lessons, four articles), each behind a greyscaled stock-style photograph that set a mood and
 * explained nothing, in a five-beat plate rhythm that made you scroll six screens to see what was
 * there.
 *
 * ## What it is now
 *
 * **Twelve lessons in three chapters**, mostly about lifting itself (getting stronger, effort,
 * volume, form, recovery, soreness, protein), then the coach, then cardio. The old ids live on as
 * aliases in `AcademyRegistry`, so coach links and the read history still land.
 *
 * **Every lesson is a drawing that teaches its idea** (`AcademySketches`): the staircase of double
 * progression, the two reps left in reserve, the volume curve's sweet spot. The picture is the
 * lesson's answer, so a reader can get it before reading a word.
 *
 * **The page opens on one large drawing assembling itself**, and it turns through a few lessons
 * ([FEATURE_CAP]): what the coach has flagged for you first, then what you have not read yet in
 * reading order. Antho asked for this directly: *"the big sketch should rotate like the big one does
 * today, don't have just one"*. Under it, the whole Academy as one contents list that fits in about
 * a screen and a half, each line with its drawing in miniature. Read lessons step back to the muted
 * tone; nothing is scored.
 */
@Composable
fun AcademyScreen(
    onBack: (() -> Unit)? = null,
    onOpenLesson: (String) -> Unit = {},
    viewModel: AcademyViewModel = hiltViewModel()
) {
    val state by viewModel.state.collectAsStateWithLifecycle()
    AcademyContent(state = state, onBack = onBack, onOpenLesson = onOpenLesson)
}

/**
 * The page with its state passed in, so it renders without Hilt in a preview or screenshot test.
 */
@Composable
fun AcademyContent(
    state: AcademyViewModel.UiState,
    onBack: (() -> Unit)? = null,
    onOpenLesson: (String) -> Unit = {}
) {
    val onBg = MaterialTheme.colorScheme.onBackground
    val muted = MaterialTheme.colorScheme.onSurfaceVariant
    val accent = MaterialTheme.colorScheme.primary

    val featured = remember(state.all) { featured(state) }
    val turns = featured.size > 1 &&
        !ForgeMotion.animationsOff &&
        !LocalTouchExplorationEnabled.current

    // Transparent while the page sits at the top, so the drawing reads as the head of the page; solid
    // the moment anything scrolls under it. Without that, lesson text slid under the clock and the
    // battery in the status bar (Antho, 2026-09-26).
    val bar = TopAppBarDefaults.pinnedScrollBehavior()
    Scaffold(
        modifier = Modifier.nestedScroll(bar.nestedScrollConnection),
        topBar = {
            if (onBack != null) {
                TopAppBar(
                    title = {},
                    navigationIcon = {
                        IconButton(onClick = onBack) {
                            Icon(Icons.AutoMirrored.Filled.ArrowBack, contentDescription = "Back", tint = muted)
                        }
                    },
                    colors = TopAppBarDefaults.topAppBarColors(
                        containerColor = Color.Transparent,
                        scrolledContainerColor = MaterialTheme.colorScheme.background
                    ),
                    scrollBehavior = bar
                )
            }
        },
        containerColor = Color.Transparent
    ) { inner ->
        LazyColumn(
            modifier = Modifier.fillMaxSize().padding(inner),
            contentPadding = PaddingValues(horizontal = GUTTER, vertical = 12.dp)
        ) {
            item("masthead") {
                Masthead(state, onBg, muted)
            }

            if (featured.isNotEmpty()) {
                item("featured") {
                    Spacer(Modifier.height(20.dp))
                    FeaturedRotator(
                        featured = featured,
                        turns = turns,
                        onBg = onBg,
                        muted = muted,
                        accent = accent,
                        onOpen = onOpenLesson
                    )
                }
            }

            LessonTrack.entries.forEach { track ->
                val lessons = state.lessonsIn(track)
                if (lessons.isEmpty()) return@forEach
                item("chapter-${track.code}") {
                    Spacer(Modifier.height(44.dp))
                    ChapterHeading(track, onBg, muted)
                    Spacer(Modifier.height(8.dp))
                }
                lessons.forEach { lesson ->
                    item(lesson.lesson.id) {
                        ContentsRow(lesson, onBg, muted, accent) { onOpenLesson(lesson.lesson.id) }
                    }
                }
            }

            item("tail") { Spacer(Modifier.height(72.dp)) }
        }
    }
}

/** The page's name, and one line of what is behind it. */
@Composable
private fun Masthead(state: AcademyViewModel.UiState, onBg: Color, muted: Color) {
    val minutes = state.all.sumOf { it.lesson.blocks.readMinutes() }
    val fresh = state.forYou.size
    Column(Modifier.fillMaxWidth().padding(top = 8.dp)) {
        Text(
            "Academy",
            style = MaterialTheme.typography.headlineLarge,
            color = onBg,
            modifier = Modifier.semantics { heading() }
        )
        Spacer(Modifier.height(8.dp))
        Text(
            buildString {
                append("${state.all.size} lessons · $minutes min of reading")
                if (fresh > 0) append(" · $fresh new for you")
            },
            style = MaterialTheme.typography.bodyMedium,
            color = muted
        )
    }
}

/** How many lessons the opening drawing turns through. */
private const val FEATURE_CAP = 4

/**
 * How long one lesson holds the opening slot. The drawing takes about three seconds to assemble,
 * and the title and answer under it are about twenty words, so ten seconds leaves time to watch the
 * one and read the other.
 */
private const val FEATURE_HOLD_MS = 10_000L

/**
 * The lessons the page opens on, in order.
 *
 * 1. What the coach flagged for you and you have not opened, newest first.
 * 2. Then what you have not read, in page order (Training is written to be read in order).
 * 3. With everything read, the first lesson of each chapter, to read again.
 */
internal fun featured(state: AcademyViewModel.UiState): List<AcademyRegistry.LessonState> {
    val picked = LinkedHashMap<String, AcademyRegistry.LessonState>()
    state.forYou.forEach { picked[it.lesson.id] = it }
    state.all.filter { !it.opened }.forEach { if (picked.size < FEATURE_CAP) picked.putIfAbsent(it.lesson.id, it) }
    if (picked.isEmpty()) {
        LessonTrack.entries.mapNotNull { state.lessonsIn(it).firstOrNull() }.forEach { picked[it.lesson.id] = it }
    }
    return picked.values.take(FEATURE_CAP)
}

/**
 * The opening block: one large drawing assembling itself, its title and its answer, turning to the
 * next lesson every [FEATURE_HOLD_MS].
 *
 * No swipe, because the Academy is a page of the hub pager and a horizontal drag belongs to the
 * tabs. The whole block is one tap target. When animations are off or TalkBack is exploring, it
 * holds still on the first lesson; everything it would have shown is in the contents below anyway.
 */
@Composable
private fun FeaturedRotator(
    featured: List<AcademyRegistry.LessonState>,
    turns: Boolean,
    onBg: Color,
    muted: Color,
    accent: Color,
    onOpen: (String) -> Unit
) {
    var shownId by rememberSaveable { mutableStateOf(featured.first().lesson.id) }
    val at = featured.indexOfFirst { it.lesson.id == shownId }.coerceAtLeast(0)
    val current = featured[at]

    LaunchedEffect(featured, at, turns) {
        shownId = featured[at].lesson.id
        if (!turns) return@LaunchedEffect
        delay(FEATURE_HOLD_MS)
        shownId = featured[(at + 1) % featured.size].lesson.id
    }

    val turn = ForgeMotion.standardTween<Float>(ForgeMotion.DurationEmphasized)
    AnimatedContent(
        targetState = current,
        contentKey = { it.lesson.id },
        transitionSpec = {
            (fadeIn(turn) togetherWith fadeOut(turn))
                .using(SizeTransform(clip = false) { _, _ -> ForgeMotion.standardTween(ForgeMotion.DurationEmphasized) })
        },
        label = "academy_featured"
    ) { lesson ->
        val position = featured.indexOfFirst { it.lesson.id == lesson.lesson.id }
        Column(Modifier.fillMaxWidth().bounceClick { onOpen(lesson.lesson.id) }) {
            LessonSketch(lessonId = lesson.lesson.id, replayKey = lesson.lesson.id)
            Spacer(Modifier.height(18.dp))
            Text(lesson.lesson.title, style = MaterialTheme.typography.headlineMedium, color = onBg)
            Spacer(Modifier.height(8.dp))
            Text(lesson.lesson.summary, style = MaterialTheme.typography.bodyLarge, color = muted)
            Spacer(Modifier.height(14.dp))
            FlowRow(
                horizontalArrangement = Arrangement.spacedBy(12.dp),
                verticalArrangement = Arrangement.spacedBy(6.dp),
                itemVerticalAlignment = Alignment.CenterVertically
            ) {
                if (lesson.isNew) ForYouMark(accent, muted)
                Text(
                    listOfNotNull(
                        if (featured.size > 1 && position >= 0) "${position + 1} of ${featured.size}" else null,
                        "${lesson.lesson.blocks.readMinutes()} min"
                    ).joinToString(" · "),
                    style = MaterialTheme.typography.labelMedium,
                    color = muted.copy(alpha = 0.65f)
                )
                Text("read →", style = MaterialTheme.typography.labelMedium, color = accent)
            }
        }
    }
}

/** A chapter's name as a real heading, and the one line it was written with. */
@Composable
private fun ChapterHeading(track: LessonTrack, onBg: Color, muted: Color) {
    Column(Modifier.fillMaxWidth()) {
        Text(
            track.displayName,
            style = MaterialTheme.typography.headlineSmall,
            color = onBg,
            modifier = Modifier.semantics { heading() }
        )
        Spacer(Modifier.height(4.dp))
        Text(chapterLine(track), style = MaterialTheme.typography.bodySmall, color = muted)
    }
}

private fun chapterLine(track: LessonTrack): String = when (track) {
    LessonTrack.TRAINING -> "How lifting works. Best read in order."
    LessonTrack.COACH -> "What the coach decides, and how to overrule it."
    LessonTrack.CARDIO -> "Conditioning that helps your lifting."
}

/**
 * One line of the contents: the lesson's drawing in miniature, its title and its answer.
 *
 * The whole row is the tap target. A read lesson steps its title back to muted, the visited-link
 * convention, so what you have not read stands out without a tick or a count.
 */
@Composable
private fun ContentsRow(
    state: AcademyRegistry.LessonState,
    onBg: Color,
    muted: Color,
    accent: Color,
    onClick: () -> Unit
) {
    val lesson = state.lesson
    Row(
        Modifier
            .fillMaxWidth()
            .heightIn(min = 48.dp)
            .bounceClick { onClick() }
            .padding(vertical = 12.dp),
        verticalAlignment = Alignment.Top
    ) {
        ThumbTile(lesson.id, Modifier.width(THUMB_WIDTH).padding(top = 2.dp))
        Spacer(Modifier.width(16.dp))
        Column(Modifier.weight(1f)) {
            Text(
                lesson.title,
                style = MaterialTheme.typography.titleMedium,
                color = if (state.opened) muted else onBg
            )
            Spacer(Modifier.height(4.dp))
            Text(
                lesson.summary,
                style = MaterialTheme.typography.bodySmall,
                color = if (state.opened) muted.copy(alpha = 0.65f) else muted
            )
            Spacer(Modifier.height(8.dp))
            Row(verticalAlignment = Alignment.CenterVertically) {
                if (state.isNew) {
                    ForYouMark(accent, muted)
                    Spacer(Modifier.width(10.dp))
                }
                Text(
                    "${lesson.blocks.readMinutes()} min",
                    style = MaterialTheme.typography.labelSmall,
                    color = muted.copy(alpha = 0.65f)
                )
            }
        }
    }
}

/**
 * A lesson's miniature on its tile. The tile is the row's cover: it gives every miniature the same
 * frame and ground, so twelve different drawings read as one set rather than as loose marks.
 */
@Composable
internal fun ThumbTile(lessonId: String, modifier: Modifier = Modifier) {
    Box(
        modifier
            .clip(RoundedCornerShape(12.dp))
            .background(MaterialTheme.colorScheme.surfaceVariant)
            .padding(horizontal = 6.dp, vertical = 8.dp)
    ) {
        LessonSketch(lessonId = lessonId, labels = false, animate = false)
    }
}

/** The one place colour marks a lesson: the coach flagged it and you have not opened it. */
@Composable
private fun ForYouMark(accent: Color, muted: Color) {
    Row(verticalAlignment = Alignment.CenterVertically) {
        Box(Modifier.size(6.dp).clip(CircleShape).background(accent))
        Spacer(Modifier.width(6.dp))
        Text("For you", style = MaterialTheme.typography.labelMedium, color = muted)
    }
}

private val GUTTER = 24.dp

/** Wide enough that the miniature still shows its shape; the page gutter holds the rest. */
private val THUMB_WIDTH = 84.dp
