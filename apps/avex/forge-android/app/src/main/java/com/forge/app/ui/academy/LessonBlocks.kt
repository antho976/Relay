package com.forge.app.ui.academy

import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.width
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.ui.Modifier
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.text.font.FontStyle
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp
import com.forge.app.domain.academy.LessonBlock
import com.forge.app.domain.academy.Source
import com.forge.app.ui.common.EditorialHeader

/**
 * The block renderer both halves of the Academy read through (Coach v3 B3, plan Mechanics M5).
 *
 * Structured blocks rather than markdown: the app ships no markdown renderer and 35 short pieces
 * don't justify adding one. Blocks also let the design system own each voice, and let an
 * [LessonBlock.Example] interpolate the reader's own numbers, which is the entire point of the later
 * "your numbers" lessons.
 *
 * ## Retuned for reading, 2026-08-20
 *
 * Antho: *"the reading itself is plain"*. It was, and the causes were all in this file.
 *
 *  - **Prose is `bodyLarge`** (16sp), not `bodyMedium`. 14sp is the size a row label takes; a page
 *    of it reads as an interface, not as something written. Paragraph air went 12 → 14 with it.
 *  - **A heading is a real section anchor** — `EditorialHeader`, which puts it on §6's mono anchor
 *    rung AND marks it as a TalkBack heading. It used to be a hand-rolled `labelLarge` with an
 *    inline `letterSpacing`, which was both off the type scale and invisible to a screen reader.
 *  - **The callout lost its box.** It was `primaryContainer`-washed, which is a surface around
 *    passive content and therefore §1's central ban, sitting in the middle of the one screen whose
 *    whole job is prose. It is a serif pull-quote now: the lesson's one takeaway, set in the voice
 *    the app reserves for the single most important thing on a page.
 *  - **An example's value is a serif figure.** It IS the reader's own number, which §2① says is a
 *    serif figure with a mono label, not a line of body text with a label in front of it.
 */
@Composable
internal fun BlockBody(
    blocks: List<LessonBlock>,
    /** Live values for [LessonBlock.Example] slots, by key. Missing keys fall back gracefully. */
    examples: Map<String, String> = emptyMap()
) {
    val onBg = MaterialTheme.colorScheme.onBackground
    val muted = MaterialTheme.colorScheme.onSurfaceVariant
    val accent = MaterialTheme.colorScheme.primary

    Column(Modifier.fillMaxWidth()) {
        blocks.forEach { block ->
            when (block) {
                is LessonBlock.Heading -> {
                    Spacer(Modifier.height(24.dp))
                    EditorialHeader(label = block.text, muted = muted, accent = accent)
                    Spacer(Modifier.height(10.dp))
                }

                is LessonBlock.Paragraph -> {
                    Text(
                        block.text,
                        style = MaterialTheme.typography.bodyLarge,
                        color = onBg
                    )
                    Spacer(Modifier.height(14.dp))
                }

                is LessonBlock.Bullets -> {
                    block.items.forEach { item ->
                        Row(Modifier.fillMaxWidth().padding(vertical = 4.dp)) {
                            // The one glyph the doctrine allows in content, carrying structure.
                            Text("·", style = MaterialTheme.typography.bodyLarge, color = accent)
                            Spacer(Modifier.width(12.dp))
                            Text(
                                item,
                                style = MaterialTheme.typography.bodyLarge,
                                color = onBg
                            )
                        }
                    }
                    Spacer(Modifier.height(14.dp))
                }

                is LessonBlock.Callout -> {
                    // The one thing to take away, set as a pull-quote. Air and a change of voice do
                    // the work a box used to do badly.
                    Spacer(Modifier.height(14.dp))
                    Text(
                        block.text,
                        style = MaterialTheme.typography.headlineSmall,
                        color = onBg,
                        modifier = Modifier.fillMaxWidth().padding(end = 12.dp)
                    )
                    Spacer(Modifier.height(24.dp))
                }

                is LessonBlock.Example -> {
                    val value = examples[block.key]
                    Spacer(Modifier.height(6.dp))
                    Column(Modifier.fillMaxWidth()) {
                        Text(
                            block.label.uppercase(),
                            style = MaterialTheme.typography.labelSmall,
                            color = muted,
                            letterSpacing = 1.5.sp
                        )
                        Spacer(Modifier.height(4.dp))
                        // The reader's own number is a serif figure (§2①). Without one, the
                        // fallback stays in the aside voice so a placeholder never impersonates
                        // real data (§12: ghost visuals yes, ghost numbers no).
                        Text(
                            value ?: block.fallback,
                            style = if (value != null) MaterialTheme.typography.headlineSmall
                            else MaterialTheme.typography.bodyMedium,
                            color = if (value != null) onBg else muted,
                            fontStyle = if (value != null) FontStyle.Normal else FontStyle.Italic
                        )
                    }
                    Spacer(Modifier.height(18.dp))
                }
            }
        }
    }
}

/**
 * An article's references, closing the page.
 *
 * Sources are a field on the article rather than a block on purpose. They always belong at the end,
 * and modelling them as a block would let an author drop a reference list into the middle of an
 * argument. Making the position un-authorable is cheaper than a rule nobody reads.
 */
@Composable
internal fun SourceList(sources: List<Source>) {
    val muted = MaterialTheme.colorScheme.onSurfaceVariant
    val accent = MaterialTheme.colorScheme.primary

    Spacer(Modifier.height(36.dp))
    EditorialHeader(label = "Sources", muted = muted, accent = accent)
    Spacer(Modifier.height(12.dp))
    sources.forEach { source ->
        SourceLine(source, muted)
        Spacer(Modifier.height(12.dp))
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
        Spacer(Modifier.height(4.dp))
        Text(
            source.title,
            style = MaterialTheme.typography.bodyMedium,
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
