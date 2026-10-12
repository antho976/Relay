package com.tally.app.ui.plan

import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.PaddingValues
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.runtime.Composable
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.unit.dp
import com.tally.app.ui.common.Dates
import com.tally.app.ui.common.LocalMoney
import com.tally.app.ui.kit.CategoryBadge
import com.tally.app.ui.kit.EntryAmount
import com.tally.app.ui.kit.Glyph
import com.tally.app.ui.kit.GlyphBadge
import com.tally.app.ui.kit.ListRow
import com.tally.app.ui.kit.Meter
import com.tally.app.ui.kit.PaceMeter
import com.tally.app.ui.kit.T
import com.tally.app.ui.kit.TransferBadge
import com.tally.app.ui.theme.Tally
import com.tally.app.ui.theme.categoryColor
import com.tally.core.Copy
import com.tally.core.GoalKind
import com.tally.core.PaceReading
import com.tally.core.PaceStatus
import com.tally.core.TxType
import java.time.LocalDate

/** The colour a reading's words take: amber over pace, red over budget, quiet otherwise. */
@Composable
fun paceTone(r: PaceReading): Color = when (r.status) {
    PaceStatus.OVER_BUDGET -> Tally.colors.heldText
    PaceStatus.OVER_PACE -> Tally.colors.waiting
    else -> Tally.colors.ink3
}

/** One budget: its category, the pace meter, what was spent of what, and the verdict. */
@Composable
fun BudgetRow(name: String, icon: String?, color: Int?, reading: PaceReading, onClick: () -> Unit, modifier: Modifier = Modifier) {
    val c = Tally.colors
    val fmt = LocalMoney.current
    ListRow(modifier, onClick = onClick, padding = PaddingValues(horizontal = 4.dp, vertical = 10.dp)) {
        CategoryBadge(icon, color)
        Column(Modifier.weight(1f), verticalArrangement = Arrangement.spacedBy(6.dp)) {
            Row(verticalAlignment = Alignment.CenterVertically) {
                T(name, Modifier.weight(1f), Tally.type.ui, c.ink, maxLines = 1)
                T(Copy.ofBudget(reading.spent, reading.budget, fmt), style = Tally.type.amount, color = c.ink2, maxLines = 1)
            }
            PaceMeter(reading.spentFraction, reading.paceFraction)
            val words = when (reading.status) {
                PaceStatus.OVER_BUDGET -> fmt.formatWhole(-reading.remaining) + " over"
                else -> fmt.formatWhole(reading.remaining) + " left · " + Copy.paceLine(reading, fmt).lowercase()
            }
            T(words, style = Tally.type.caption, color = paceTone(reading), maxLines = 1)
        }
    }
}

/** A bill or a subscription: its badge, its name and when it is due, its amount. */
@Composable
fun BillRow(
    name: String,
    type: TxType,
    amount: Long,
    icon: String?,
    color: Int?,
    line: String,
    onClick: () -> Unit,
    modifier: Modifier = Modifier,
    due: Long? = null,
    autoPost: Boolean = true,
) {
    val c = Tally.colors
    ListRow(modifier, onClick = onClick, padding = PaddingValues(horizontal = 4.dp, vertical = 8.dp)) {
        if (type == TxType.TRANSFER) TransferBadge() else CategoryBadge(icon, color)
        Column(Modifier.weight(1f), verticalArrangement = Arrangement.spacedBy(2.dp)) {
            Row(verticalAlignment = Alignment.CenterVertically, horizontalArrangement = Arrangement.spacedBy(5.dp)) {
                T(name, Modifier.weight(1f, fill = false), Tally.type.ui, c.ink, maxLines = 1)
                if (!autoPost) Glyph("bell", 12.dp, c.ink3)
            }
            T(line, style = Tally.type.caption, color = if (due != null && due < 0) c.heldText else if (due == 0L) c.waiting else c.ink3, maxLines = 1)
        }
        EntryAmount(amount, type)
    }
}

/** A bill's line: when it is due, and how often. */
fun billLineText(next: LocalDate, daysUntil: Long, every: String, today: LocalDate): String =
    (if (daysUntil in -30L..14L) Copy.dueLine(daysUntil) else Dates.short(next, today)) + " · " + every

/** One goal: its colour, its name, its meter and the line that says what it needs. */
@Composable
fun GoalRow(g: GoalLine, today: LocalDate, onClick: () -> Unit, modifier: Modifier = Modifier) {
    val c = Tally.colors
    val fmt = LocalMoney.current
    val hue = categoryColor(g.color)
    ListRow(modifier, onClick = onClick, padding = PaddingValues(horizontal = 4.dp, vertical = 10.dp)) {
        GlyphBadge(goalGlyph(g.kind), hue, hue.copy(alpha = .16f))
        Column(Modifier.weight(1f), verticalArrangement = Arrangement.spacedBy(6.dp)) {
            Row(verticalAlignment = Alignment.CenterVertically) {
                T(g.name, Modifier.weight(1f), Tally.type.ui, c.ink, maxLines = 1)
                T(goalReading(g, fmt), style = Tally.type.amount, color = c.ink2, maxLines = 1)
            }
            if (g.paceFraction != null) PaceMeter(g.fraction, g.paceFraction) else Meter(g.fraction, color = if (g.reached) c.live else c.ink)
            T(goalRowLine(g, fmt, today), style = Tally.type.caption, color = if (g.status == GoalStatus.BEHIND) c.waiting else if (g.reached) c.liveText else c.ink3, maxLines = 2)
        }
    }
}

fun goalGlyph(kind: GoalKind): String = when (kind) {
    GoalKind.SAVINGS -> "target"
    GoalKind.BALANCE -> "flag"
    GoalKind.INVEST -> "trend"
    GoalKind.SAVE -> "wallet"
}

/** A whole row's width: used where a row sits outside a ListRow. */
@Composable
fun RowLine(modifier: Modifier = Modifier, content: @Composable () -> Unit) {
    Row(modifier.fillMaxWidth(), verticalAlignment = Alignment.CenterVertically) { content() }
}
