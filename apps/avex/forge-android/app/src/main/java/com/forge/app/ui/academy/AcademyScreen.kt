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
import com.forge.app.ui.common.LocalTouchExplorationEnabled
import com.forge.app.ui.common.statsEntrance
import com.forge.app.ui.theme.ForgeMotion

/**
 * The Academy — one page holding everything the coach knows, browsable end to end.
 *
 * ## What changed, 2026-08-20
 *
 * The 2026-08-16 rebuild fixed the gate (everything is readable from install) and the nesting
 * (tracks are chapters, not sub-screens). What it did not fix, in Antho's words: it still *reads as
 * blocks*, there is *no sense of where to start*, and *the reading itself is plain*.
 *
 * Three answers, one per complaint:
 *
 * 1. **The blocks were the cards.** Every piece was a filled, hairlined tile from the Home
 *    experiment's kit, which §1 bans around passive content for exactly this reason. Pieces are
 *    plates on the page now — see `AcademyGallery`.
 * 2. **The page opens by pointing.** A masthead, then ONE piece named as the thing to read next
 *    ([startHere]): the coach's poke if one fired, otherwise the next unread Fundamentals lesson,
 *    which is the only track authored in a reading order. Each chapter then prints the blurb it was
 *    authored with, and Fundamentals numbers its pieces, so "where do I start" is answered three
 *    times on the way down the page without a single progress bar.
 * 3. **The reading moved out of a sheet.** A lesson is a screen now (`LessonScreen`), the same one
 *    an article gets, so the two halves of the Academy finally read alike. `LessonSheet` is gone.
 *
 * ## Chapter order
 *
 * The five lesson tracks in reading order, then the Library's articles grouped by topic. Lesson
 * tracks lead because they are the coach's own curriculum; the Library is the wider reading beside
 * it, and neither is hidden from the other. There is no separate FOR YOU shelf: the pokes are the
 * page's opening block, which turns through them one at a time ([StartHereRotator]), and each one
 * drops back into its own chapter the moment it is read.
 *
 * ## What changed, 2026-08-23
 *
 * The opening block got its picture back and started turning. Both were the same request — from
 * Antho: *"can you add pictures here and make it rotate ... so we know which could we look and
 * cycle between them, and when there's one he read we remove it from the cycle"*. A fired coach
 * moment is the app's own notification about a lesson, several can be outstanding at once, and the
 * page was naming the newest and hiding the others. See [startHere] for what enters the cycle and
 * [StartHereRotator] for how it turns.
 */
@Composable
fun AcademyScreen(
    onBack: (() -> Unit)? = null,
    onOpenLesson: (String) -> Unit = {},
    onOpenArticle: (String) -> Unit = {},
    viewModel: AcademyViewModel = hiltViewModel(),
    libraryViewModel: LibraryViewModel = hiltViewModel()
) {
    val state by viewModel.state.collectAsStateWithLifecycle()
    val libraryState by libraryViewModel.state.collectAsStateWithLifecycle()
    AcademyContent(
        state = state,
        library = libraryState,
        onBack = onBack,
        onOpenLesson = onOpenLesson,
        onOpenArticle = onOpenArticle
    )
}

/**
 * The page itself, with its state passed in.
 *
 * Split out from [AcademyScreen] so the gallery can be rendered without Hilt — by a screenshot
 * test, by a preview, and by anything that wants to see the chapters at a state the database is not
 * currently in (an empty shelf, everything read, a poke that fired).
 */
@Composable
fun AcademyContent(
    state: AcademyViewModel.UiState,
    library: LibraryViewModel.UiState,
    onBack: (() -> Unit)? = null,
    onOpenLesson: (String) -> Unit = {},
    onOpenArticle: (String) -> Unit = {}
) {
    val libraryState = library
    val onBg = MaterialTheme.colorScheme.onBackground
    val muted = MaterialTheme.colorScheme.onSurfaceVariant
    val accent = MaterialTheme.colorScheme.primary

    val pointers = remember(state.all, libraryState.articles) { startHere(state, libraryState) }

    // Decided here rather than inside the block, because it governs the chapters too. When the
    // opening slot turns, every poke it will show has to come OUT of the chapters below or the
    // same photograph prints twice. When it cannot turn — animations off, or TalkBack exploring —
    // only the piece actually on screen comes out, and the rest stay in their chapters with their
    // accent dots, which is the only thing keeping them reachable once nothing is rotating.
    val turns = pointers.size > 1 &&
        !ForgeMotion.animationsOff &&
        !LocalTouchExplorationEnabled.current
    val sections = remember(state.all, libraryState.articles, pointers, turns) {
        val promoted = if (turns) pointers.map { it.item.id } else pointers.take(1).map { it.item.id }
        buildSections(state, libraryState, promoted = promoted.toSet())
    }
    val open: (GalleryItem) -> Unit = { item ->
        when (item) {
            is GalleryItem.LessonTile -> onOpenLesson(item.id)
            is GalleryItem.ArticleTile -> onOpenArticle(item.id)
        }
    }

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
            item("masthead") {
                // Counted from the shelf, not from the rendered chapters: the pieces the opening
                // block names are lifted out of their chapters, and the Academy does not shrink by
                // one piece every time one of them is being pointed at.
                val pieces = state.all.map { GalleryItem.LessonTile(it) } +
                    libraryState.articles.map { GalleryItem.ArticleTile(it) }
                Masthead(
                    pieces = pieces.size,
                    minutes = pieces.sumOf { it.minutes },
                    onBg = onBg,
                    muted = muted
                )
            }

            if (pointers.isNotEmpty()) {
                item("start") {
                    Spacer(Modifier.height(28.dp))
                    StartHereRotator(
                        pointers = pointers,
                        turns = turns,
                        onBg = onBg,
                        muted = muted,
                        accent = accent,
                        modifier = Modifier.statsEntrance(1),
                        onOpen = open
                    )
                }
            }

            sections.forEachIndexed { sectionIndex, section ->
                item("h-${section.label}") {
                    Spacer(Modifier.height(CHAPTER_GAP))
                    ChapterHeader(
                        label = section.label,
                        blurb = section.blurb,
                        muted = muted,
                        accent = accent,
                        modifier = Modifier.statsEntrance(sectionIndex + 2)
                    )
                    Spacer(Modifier.height(18.dp))
                }

                // Walk the chapter in the plate rhythm: a lead, then two-up posters until the next
                // lead. Grouping here rather than inside the entry keeps the shape decision in one
                // place — `isLeadSlot` is the only thing that knows the beat.
                var i = 0
                while (i < section.items.size) {
                    // A two-piece chapter renders as one poster row rather than a lead plus a
                    // widow: a lead with a single half-width piece under it reads as a mistake.
                    val leadSlot = isLeadSlot(i) && section.items.size != 2
                    if (leadSlot) {
                        val piece = section.items[i]
                        val index = i
                        item("l-${section.label}-$i") {
                            if (index > 0) Spacer(Modifier.height(ENTRY_GAP))
                            PieceEntry(
                                item = piece,
                                shape = PlateShape.LEAD,
                                onBg = onBg,
                                muted = muted,
                                accent = accent,
                                modifier = Modifier.fillMaxWidth(),
                                numeral = section.numerals.getOrNull(index)
                            ) { open(piece) }
                        }
                        i += 1
                    } else {
                        val at = i
                        val pair = section.items.subList(at, minOf(at + 2, section.items.size))
                        item("p-${section.label}-$i") {
                            Spacer(Modifier.height(if (at > 0) ENTRY_GAP else 0.dp))
                            PosterRow(
                                pair = pair,
                                onBg = onBg,
                                muted = muted,
                                accent = accent,
                                numerals = pair.indices.map { section.numerals.getOrNull(at + it) },
                                onOpen = open
                            )
                        }
                        i += pair.size
                    }
                }
            }

            item("tail") { Spacer(Modifier.height(72.dp)) }
        }
    }
}

/**
 * The page's own name and what is behind it.
 *
 * The count used to read "4 OF 31 UNLOCKED", which is a score, and a score is the exact thing this
 * page must not open with. It states the size of the shelf and how long it would take to read, both
 * of which are facts about the content rather than about the reader.
 */
@Composable
private fun Masthead(pieces: Int, minutes: Int, onBg: Color, muted: Color) {
    Column(Modifier.fillMaxWidth().statsEntrance(0).padding(vertical = 8.dp)) {
        Text(
            "$pieces pieces · ${readingSpan(minutes)}".uppercase(),
            style = MaterialTheme.typography.labelMedium,
            color = muted
        )
        Spacer(Modifier.height(4.dp))
        Text("Academy", style = MaterialTheme.typography.headlineLarge, color = onBg)
        Spacer(Modifier.height(8.dp))
        Text(
            "Everything the coach knows, open from the start.",
            style = MaterialTheme.typography.bodyMedium.copy(fontStyle = FontStyle.Italic),
            color = muted
        )
    }
}

/** "40 min" under the hour, "3 hr" over it. Never "0 hr", and never a decimal hour. */
private fun readingSpan(minutes: Int): String = when {
    minutes < 60 -> "$minutes min"
    else -> "${(minutes + 30) / 60} hr"
}

/**
 * The air between one piece and the next, and between one chapter and the next.
 *
 * These two numbers and `CAPTION_GAP` (8dp) are the only thing binding a caption to its picture:
 * there is no box and no rule to do it, so the gaps have to say it. 8dp up, 44dp down — a caption
 * is more than five times closer to its own plate than to the next piece, which is past the point
 * where the eye can read it either way.
 */
private val ENTRY_GAP = 44.dp
private val CHAPTER_GAP = 52.dp

/** Where the page points first, and the word above it. */
data class Pointer(
    val kicker: String,
    val item: GalleryItem,
    val chapter: String,
    val numeral: String?
)

/**
 * The most pokes the opening block will hold at once.
 *
 * A cap rather than the whole of `forYou`, because a cycle you cannot get to the end of is the
 * backlog `forYou`'s own doc warns about, wearing a different shape — nine dots under the block
 * would be a queue drawn as a queue. Four is what a row of dots reads as a *count* at rather than
 * as a bar, and the ones past it are not lost: every piece keeps its accent dot in its own chapter,
 * and one of them moves up here as soon as something ahead of it is read.
 */
private const val POKE_CAP = 4

/**
 * Pick the pieces the page opens by naming — newest poke first, and everything still unread behind
 * it.
 *
 * The order of preference is the order of how much the app actually knows about the reader:
 *
 * 1. **Coach moments fired** whose lessons are unread. Those are the pokes, and they are the only
 *    pointers grounded in something that just happened to this reader. All of them, up to
 *    [POKE_CAP] — the block turns through them ([StartHereRotator]) rather than naming the newest
 *    and dropping the rest on the floor.
 * 2. **The next unread Fundamentals lesson.** Fundamentals is the one track authored in a reading
 *    order, so it is the only place a "next" exists without inventing a ranking.
 * 3. **Anything else unread**, lessons before articles, in registry order.
 * 4. **Nothing left** — the shelf is read end to end, so the pointer says so and offers the top of
 *    the curriculum again. §12: the finished state is drawn, not hidden.
 *
 * Only case 1 ever returns more than one: the other three are the page reasoning about a shelf it
 * has no personal signal on, and a cycle through *those* would be an invented ranking turning
 * itself over every few seconds. The fallbacks stay one still piece, as they were.
 *
 * Everything returned here is lifted out of the chapters below by [buildSections], so no
 * photograph is printed twice.
 */
fun startHere(
    state: AcademyViewModel.UiState,
    library: LibraryViewModel.UiState
): List<Pointer> {
    val fundamentals = state.lessonsIn(LessonTrack.FUNDAMENTALS)

    fun lessonPointer(kicker: String, lesson: com.forge.app.domain.academy.AcademyRegistry.LessonState): Pointer {
        val at = fundamentals.indexOfFirst { it.lesson.id == lesson.lesson.id }
        return Pointer(
            kicker = kicker,
            item = GalleryItem.LessonTile(lesson),
            chapter = lesson.lesson.track.displayName,
            numeral = if (at >= 0) numeralOf(at) else null
        )
    }

    val pokes = state.forYou.take(POKE_CAP).map { lessonPointer("For you", it) }
    if (pokes.isNotEmpty()) return pokes

    fundamentals.firstOrNull { !it.opened }?.let {
        val started = fundamentals.any { f -> f.opened }
        return listOf(lessonPointer(if (started) "Continue" else "Start here", it))
    }

    state.all.firstOrNull { !it.opened }?.let { return listOf(lessonPointer("Next", it)) }

    library.articles.firstOrNull { !it.finished }?.let {
        return listOf(Pointer("Next", GalleryItem.ArticleTile(it), it.article.topic.displayName, null))
    }

    return listOfNotNull(fundamentals.firstOrNull()?.let { lessonPointer("Read again", it) })
}

/** "01", "02" — the position of a piece inside an ordered chapter, never a total. */
private fun numeralOf(index: Int): String = (index + 1).toString().padStart(2, '0')

/**
 * Assemble the chapters.
 *
 * [promoted] is the set of pieces the page already names at the top, and they are lifted OUT of
 * their chapters here. Leaving one in printed the same serif title and the same deck twice within a
 * screen and a half, which is the "one card rendered twice" bug the previous pass hit from the
 * other direction. It is a set rather than a single id because the opening block turns through
 * every unread poke: a piece that is only shown seven seconds from now is still shown here, so all
 * of them come out, and each returns to its chapter — in the read tone — as soon as it is opened.
 * Nothing is lost meanwhile: the pointer states which chapter the piece belongs to and, in
 * Fundamentals, its position — and the numerals are computed BEFORE the lift, so the chapter opens
 * at 02 rather than silently renumbering itself and claiming a piece is the first when it is not.
 *
 * Articles group by their own topic rather than being forced into a lesson track: the two
 * taxonomies are genuinely different, and mapping one onto the other would file articles under
 * headings they were not written for. Only topics holding an article appear (§12) — eight empty
 * shelves against four articles would open the Library as a promise nothing keeps.
 */
private fun buildSections(
    state: AcademyViewModel.UiState,
    library: LibraryViewModel.UiState,
    promoted: Set<String>
): List<GallerySection> = buildList {
    LessonTrack.entries.forEach { track ->
        val all = state.lessonsIn(track).map { GalleryItem.LessonTile(it) }
        // Fundamentals is the only track authored in a reading order, so it is the only one whose
        // pieces carry a position.
        val numerals = all.indices.map {
            if (track == LessonTrack.FUNDAMENTALS) numeralOf(it) else null
        }
        val kept = all.indices.filter { all[it].id !in promoted }
        if (kept.isNotEmpty()) {
            add(
                GallerySection(
                    label = track.displayName,
                    items = kept.map { all[it] },
                    numerals = kept.map { numerals[it] },
                    blurb = track.blurb
                )
            )
        }
    }

    ArticleTopic.entries.forEach { topic ->
        val items = library.articles
            .filter { it.article.topic == topic }
            .map { GalleryItem.ArticleTile(it) }
            .filter { it.id !in promoted }
        if (items.isNotEmpty()) add(GallerySection(label = topic.displayName, items = items))
    }
}
