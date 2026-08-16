package com.forge.app.ui.academy

import androidx.compose.foundation.background
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.width
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.clip
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.text.font.FontStyle
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp
import com.forge.app.domain.academy.Article
import com.forge.app.domain.academy.Lesson
import com.forge.app.domain.academy.LessonBlock
import com.forge.app.domain.academy.Source
import com.forge.app.ui.common.EditorialHeader

/**
 * The lesson renderer (Coach v3 B3, plan Mechanics M5).
 *
 * Structured blocks rather than markdown: the app ships no markdown renderer and 30-odd short
 * lessons don't justify adding one. Blocks also let the design system own each voice — mono
 * headings, sans prose, an accent-washed callout — and let an [LessonBlock.Example] interpolate the
 * reader's own numbers, which is the entire point of the later "your numbers" lessons.
 */
@Composable
internal fun LessonBody(
    lesson: Lesson,
    /** Live values for [LessonBlock.Example] slots, by key. Missing keys fall back gracefully. */
    examples: Map<String, String> = emptyMap()
) = BlockBody(lesson.blocks, examples)

/**
 * A Library article's body: the same blocks, closing with its sources.
 *
 * Sources are a field on [Article] rather than a block on purpose. They always belong at the end,
 * and modelling them as a block would let an author drop a reference list into the middle of an
 * argument. Making the position un-authorable is cheaper than a rule nobody reads.
 */
@Composable
internal fun ArticleBody(article: Article) {
    val muted = MaterialTheme.colorScheme.onSurfaceVariant
    val accent = MaterialTheme.colorScheme.primary

    BlockBody(article.blocks)

    if (article.sources.isEmpty()) return
    Spacer(Modifier.height(28.dp))
    EditorialHeader(label = "Sources", muted = muted, accent = accent)
    Spacer(Modifier.height(10.dp))
    article.sources.forEach { source ->
        SourceLine(source, muted)
        Spacer(Modifier.height(10.dp))
    }
}

/**
 * One reference: authors and year lead, the title carries the weight, the journal closes.
 *
 * Plain text, never a link. The app holds no INTERNET permission, so a tappable citation would be
 * an affordance that cannot run (§2③), and a bare URL printed as text is worse than the citation
 * itself. A reader who wants the paper has enough here to find it.
 */
@Composable
private fun SourceLine(source: Source, muted: Color) {
    Column(Modifier.fillMaxWidth()) {
        Text(
            "${source.authors}, ${source.year}".uppercase(),
            style = MaterialTheme.typography.labelSmall,
            color = muted.copy(alpha = 0.65f),
            letterSpacing = 1.2.sp
        )
        Spacer(Modifier.height(3.dp))
        Text(
            source.title,
            style = MaterialTheme.typography.bodySmall,
            color = muted
        )
        source.journal?.let { journal ->
            Spacer(Modifier.height(2.dp))
            Text(
                journal,
                style = MaterialTheme.typography.bodySmall,
                color = muted.copy(alpha = 0.65f),
                fontStyle = FontStyle.Italic
            )
        }
    }
}

/** The shared block renderer both halves of the Academy draw through. */
@Composable
private fun BlockBody(
    blocks: List<LessonBlock>,
    examples: Map<String, String> = emptyMap()
) {
    val onBg = MaterialTheme.colorScheme.onBackground
    val muted = MaterialTheme.colorScheme.onSurfaceVariant
    val accent = MaterialTheme.colorScheme.primary
    val wash = MaterialTheme.colorScheme.primaryContainer

    Column(Modifier.fillMaxWidth()) {
        blocks.forEach { block ->
            when (block) {
                is LessonBlock.Heading -> {
                    Spacer(Modifier.height(18.dp))
                    Text(
                        block.text.uppercase(),
                        style = MaterialTheme.typography.labelLarge,
                        color = muted,
                        letterSpacing = 1.5.sp
                    )
                    Spacer(Modifier.height(8.dp))
                }

                is LessonBlock.Paragraph -> {
                    Text(
                        block.text,
                        style = MaterialTheme.typography.bodyMedium,
                        color = onBg
                    )
                    Spacer(Modifier.height(12.dp))
                }

                is LessonBlock.Bullets -> {
                    block.items.forEach { item ->
                        Row(Modifier.fillMaxWidth().padding(vertical = 3.dp)) {
                            // The one glyph the doctrine allows in content, carrying structure.
                            Text("·", style = MaterialTheme.typography.bodyMedium, color = accent)
                            Spacer(Modifier.width(10.dp))
                            Text(
                                item,
                                style = MaterialTheme.typography.bodyMedium,
                                color = onBg
                            )
                        }
                    }
                    Spacer(Modifier.height(12.dp))
                }

                is LessonBlock.Callout -> {
                    // The one thing to take away. A surface here is earned: it is the lesson's
                    // conclusion, not passive prose.
                    Spacer(Modifier.height(4.dp))
                    Text(
                        block.text,
                        style = MaterialTheme.typography.bodyMedium,
                        color = onBg,
                        modifier = Modifier
                            .fillMaxWidth()
                            .clip(RoundedCornerShape(12.dp))
                            .background(wash)
                            .padding(horizontal = 14.dp, vertical = 12.dp)
                    )
                    Spacer(Modifier.height(12.dp))
                }

                is LessonBlock.Example -> {
                    val value = examples[block.key]
                    Row(
                        Modifier.fillMaxWidth().padding(vertical = 6.dp),
                        verticalAlignment = Alignment.CenterVertically
                    ) {
                        Text(
                            block.label.uppercase(),
                            style = MaterialTheme.typography.labelSmall,
                            color = muted,
                            fontSize = 9.sp,
                            letterSpacing = 1.5.sp
                        )
                        Spacer(Modifier.width(10.dp))
                        Text(
                            value ?: block.fallback,
                            style = if (value != null) MaterialTheme.typography.titleMedium
                            else MaterialTheme.typography.bodySmall,
                            color = if (value != null) onBg else muted,
                            fontStyle = if (value != null) FontStyle.Normal else FontStyle.Italic
                        )
                    }
                    Spacer(Modifier.height(8.dp))
                }
            }
        }
    }
}
