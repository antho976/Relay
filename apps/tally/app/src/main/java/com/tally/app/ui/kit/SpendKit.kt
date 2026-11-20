package com.tally.app.ui.kit

import androidx.compose.foundation.background
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.ColumnScope
import androidx.compose.foundation.layout.PaddingValues
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.padding
import androidx.compose.runtime.Composable
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.clip
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.text.style.TextAlign
import androidx.compose.ui.unit.Dp
import androidx.compose.ui.unit.dp
import com.tally.app.ui.common.LineTone
import com.tally.app.ui.theme.Tally
import com.tally.core.TxType
import java.time.LocalDate

/*
 * The Spend pages' shared pieces in direction C: a line's tone readable on the sky, a bill row that
 * leads with its date, a day's group of entries on glass, a centred status pill.
 */

/** A generated line's tone as words that read on the sky: amber words are the deep amber. */
@Composable
fun skyTone(tone: LineTone): Color = when (tone) {
    LineTone.QUIET -> Tally.colors.ink3
    LineTone.WARN -> Tally.colors.waitingText
    LineTone.HELD -> Tally.colors.heldText
}

/** A small pill of state words, on a soft tint of its meaning. */
@Composable
fun StatePill(text: String, tone: LineTone, modifier: Modifier = Modifier, glyph: String? = null) {
    val c = Tally.colors
    val (fill, ink) = when (tone) {
        LineTone.QUIET -> softOf(c.live) to c.liveText
        LineTone.WARN -> softOf(c.waiting) to c.waitingText
        LineTone.HELD -> softOf(c.held) to c.heldText
    }
    Row(
        modifier.clip(Radii.pill).background(fill).padding(horizontal = 12.dp, vertical = 5.dp),
        verticalAlignment = Alignment.CenterVertically,
        horizontalArrangement = Arrangement.spacedBy(6.dp),
    ) {
        glyph?.let { Glyph(it, 13.dp, ink) }
        T(text, style = Tally.type.caption, color = ink, weight = FontWeight.SemiBold, maxLines = 2, align = TextAlign.Center)
    }
}

/** A bill coming up: its date as a badge, its name over its line, its amount. */
@Composable
fun DatedBillRow(
    name: String,
    date: LocalDate,
    line: String,
    type: TxType,
    amount: Long,
    hue: Color,
    onClick: () -> Unit,
    due: Long? = null,
    autoPost: Boolean = true,
) {
    val c = Tally.colors
    ListRow(onClick = onClick, padding = PaddingValues(horizontal = 4.dp, vertical = 7.dp)) {
        DateBadge(date, hue = hue)
        Column(Modifier.weight(1f), verticalArrangement = Arrangement.spacedBy(2.dp)) {
            Row(verticalAlignment = Alignment.CenterVertically, horizontalArrangement = Arrangement.spacedBy(5.dp)) {
                T(name, Modifier.weight(1f, fill = false), Tally.type.body, c.ink, maxLines = 1, weight = FontWeight.SemiBold)
                if (!autoPost) Glyph("bell", 12.dp, c.ink3)
            }
            T(line, style = Tally.type.caption, color = if (due != null && due < 0) c.heldText else if (due == 0L) c.waitingText else c.ink3, maxLines = 1)
        }
        EntryAmount(amount, type, style = Tally.type.amount.copy(fontWeight = FontWeight.SemiBold))
    }
}

/** A day's header over its group: the date on the left, what it took (and brought) on the right. */
@Composable
fun DayHeader(day: String, modifier: Modifier = Modifier, trailing: @Composable () -> Unit = {}) {
    Row(modifier.fillMaxWidth().padding(start = 4.dp, end = 4.dp, top = 10.dp, bottom = 6.dp), verticalAlignment = Alignment.CenterVertically) {
        T(day, Modifier.weight(1f), Tally.type.uiMedium, Tally.colors.ink, weight = FontWeight.Bold, maxLines = 1)
        trailing()
    }
}

/** A day's entries on one glass card. */
@Composable
fun GlassGroup(modifier: Modifier = Modifier, padding: Dp = 10.dp, content: @Composable ColumnScope.() -> Unit) {
    Slab(modifier.fillMaxWidth(), padding = PaddingValues(horizontal = padding, vertical = 6.dp), content = content)
}
