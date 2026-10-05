package com.tally.app.ui.activity

import androidx.compose.foundation.background
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.ExperimentalLayoutApi
import androidx.compose.foundation.layout.FlowRow
import androidx.compose.foundation.layout.IntrinsicSize
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.fillMaxHeight
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.heightIn
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.layout.width
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.foundation.text.BasicTextField
import androidx.compose.foundation.text.KeyboardActions
import androidx.compose.foundation.text.KeyboardOptions
import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.rounded.Close
import androidx.compose.material.icons.rounded.Search
import androidx.compose.material3.Icon
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Text
import androidx.compose.material3.minimumInteractiveComponentSize
import androidx.compose.runtime.Composable
import androidx.compose.runtime.Immutable
import androidx.compose.runtime.key
import androidx.compose.runtime.remember
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.clip
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.graphics.SolidColor
import androidx.compose.ui.graphics.vector.ImageVector
import androidx.compose.ui.platform.LocalDensity
import androidx.compose.ui.platform.LocalFocusManager
import androidx.compose.ui.semantics.Role
import androidx.compose.ui.semantics.contentDescription
import androidx.compose.ui.semantics.heading
import androidx.compose.ui.semantics.semantics
import androidx.compose.ui.text.input.ImeAction
import androidx.compose.ui.unit.dp
import com.tally.app.ui.common.Aside
import com.tally.app.ui.common.Dates
import com.tally.app.ui.common.EndsRow
import com.tally.app.ui.common.FIGURE_GAP
import com.tally.app.ui.common.HeroPanel
import com.tally.app.ui.common.LedgerRow
import com.tally.app.ui.common.LocalMoney
import com.tally.app.ui.common.Panel
import com.tally.app.ui.common.PanelHeader
import com.tally.app.ui.common.StackedBar
import com.tally.app.ui.common.StatChip
import com.tally.app.ui.common.TextAction
import com.tally.app.ui.common.bounceClick
import java.time.LocalDate

/*
 * The pieces Activity and the one-category / one-account ledger share: the filled search field,
 * the day panel, the in-and-out hero, the tile pair and the quiet zero panel.
 */

/** A chip for a hero: a glyph and a few words. */
@Immutable
internal data class HeroChip(val icon: ImageVector, val text: String)

/**
 * The filled rounded search field, the Avex History look: glyph at the start, a clear capsule at
 * the end once something is typed. The search key closes the keyboard; results already follow
 * the text.
 */
@Composable
internal fun SearchField(
    query: String,
    onQuery: (String) -> Unit,
    modifier: Modifier = Modifier,
    placeholder: String = "Search notes, categories, accounts",
) {
    val focus = LocalFocusManager.current
    val shape = RoundedCornerShape(16.dp)
    BasicTextField(
        value = query,
        onValueChange = onQuery,
        modifier = modifier
            .fillMaxWidth()
            .semantics { contentDescription = placeholder },
        singleLine = true,
        textStyle = MaterialTheme.typography.bodyLarge.copy(color = MaterialTheme.colorScheme.onBackground),
        cursorBrush = SolidColor(MaterialTheme.colorScheme.primary),
        keyboardOptions = KeyboardOptions(imeAction = ImeAction.Search),
        keyboardActions = KeyboardActions(onSearch = { focus.clearFocus() }),
        decorationBox = { inner ->
            Row(
                Modifier
                    .fillMaxWidth()
                    .clip(shape)
                    .background(MaterialTheme.colorScheme.surfaceContainerHigh)
                    .heightIn(min = 56.dp)
                    .padding(start = 18.dp, end = 4.dp),
                verticalAlignment = Alignment.CenterVertically,
                horizontalArrangement = Arrangement.spacedBy(12.dp),
            ) {
                Icon(Icons.Rounded.Search, contentDescription = null, tint = MaterialTheme.colorScheme.onSurfaceVariant)
                Box(Modifier.weight(1f).padding(vertical = 16.dp)) {
                    if (query.isEmpty()) {
                        Text(placeholder, style = MaterialTheme.typography.bodyLarge, color = MaterialTheme.colorScheme.onSurfaceVariant)
                    }
                    inner()
                }
                if (query.isNotEmpty()) {
                    Box(
                        Modifier
                            .minimumInteractiveComponentSize()
                            .clip(RoundedCornerShape(50))
                            .bounceClick(label = "Clear search", role = Role.Button, focusShape = RoundedCornerShape(50)) { onQuery("") },
                        contentAlignment = Alignment.Center,
                    ) {
                        Icon(Icons.Rounded.Close, contentDescription = "Clear search", tint = MaterialTheme.colorScheme.onSurfaceVariant)
                    }
                } else {
                    Spacer(Modifier.width(14.dp))
                }
            }
        },
    )
}

/**
 * One day of the ledger on its own panel: the mono day name with that day's total at the end
 * (under it, once the two cannot share a line without breaking the name), then the day's rows.
 * The header carries the day, so rows carry no date.
 */
@Composable
internal fun DayPanel(group: DayGroup, today: LocalDate, onOpen: (Long) -> Unit, modifier: Modifier = Modifier) {
    Panel(modifier) {
        DayHeader(group, today)
        Spacer(Modifier.height(2.dp))
        group.rows.forEach { row ->
            key(row.id) { LedgerRow(row, meta = "", onClick = { onOpen(row.id) }) }
        }
    }
}

@Composable
private fun DayHeader(group: DayGroup, today: LocalDate) {
    val money = LocalMoney.current
    val label = Dates.day(group.date, today)
    // What the day did: its spending, or what came in on a day of only income. A day of only
    // transfers moved money between your own accounts and reads as nothing out.
    val total = when {
        group.spent > 0 -> money.format(group.spent)
        group.income > 0 -> money.formatSigned(group.income)
        else -> money.format(0L)
    }
    val totalColor = if (group.spent == 0L && group.income > 0) MaterialTheme.colorScheme.tertiary else MaterialTheme.colorScheme.onBackground
    Row(
        Modifier
            .fillMaxWidth()
            .heightIn(min = 40.dp)
            .semantics(mergeDescendants = true) { heading() },
        verticalAlignment = Alignment.CenterVertically,
        horizontalArrangement = Arrangement.spacedBy(12.dp),
    ) {
        EndsRow(
            start = {
                Text(label.uppercase(), style = MaterialTheme.typography.labelLarge, color = MaterialTheme.colorScheme.onBackground)
            },
            end = { Text(total, style = MaterialTheme.typography.labelLarge, color = totalColor) },
            modifier = Modifier.weight(1f),
        )
    }
}

/**
 * The hero's label row: a glyph tile, the mono name of what the hero reads, and a mono reading
 * at the end.
 */
@Composable
internal fun HeroLabel(
    label: String,
    meta: String? = null,
    iconTint: Color = MaterialTheme.colorScheme.primary,
) {
    PanelHeader(label, meta = meta, tint = iconTint)
}

/** A serif figure over its mono label, keyed to its colour in the bar by a dot. */
@Composable
private fun KeyedFigure(value: String, label: String, dot: Color, valueColor: Color) {
    Column(verticalArrangement = Arrangement.spacedBy(2.dp)) {
        Text(value, style = MaterialTheme.typography.headlineLarge, color = valueColor)
        Row(verticalAlignment = Alignment.CenterVertically, horizontalArrangement = Arrangement.spacedBy(6.dp)) {
            Box(Modifier.size(8.dp).clip(RoundedCornerShape(50)).background(dot))
            Text(label.uppercase(), style = MaterialTheme.typography.labelLarge, color = MaterialTheme.colorScheme.onSurfaceVariant)
        }
    }
}

/**
 * The lit panel for a set of entries: money out and money in as two serif figures, the margin
 * between them in words, the split as one bar, and chips for the readings around it.
 */
@OptIn(ExperimentalLayoutApi::class)
@Composable
internal fun FlowHero(
    label: String,
    meta: String?,
    spent: Long,
    income: Long,
    chips: List<HeroChip>,
    modifier: Modifier = Modifier,
) {
    val money = LocalMoney.current
    val outColor = MaterialTheme.colorScheme.primary
    val inColor = MaterialTheme.colorScheme.tertiary
    val segments = remember(spent, income, outColor, inColor) {
        listOf(spent.toFloat() to outColor, income.toFloat() to inColor).filter { it.first > 0f }
    }
    val out = money.formatWhole(spent)
    val inn = money.formatWhole(income)
    HeroPanel(modifier) {
        HeroLabel(label, meta)
        Spacer(Modifier.height(14.dp))
        FlowRow(horizontalArrangement = Arrangement.spacedBy(28.dp), verticalArrangement = Arrangement.spacedBy(10.dp)) {
            KeyedFigure(out, "Out", dot = outColor, valueColor = MaterialTheme.colorScheme.onBackground)
            KeyedFigure(inn, "In", dot = inColor, valueColor = inColor)
        }
        Spacer(Modifier.height(10.dp))
        Text(
            netLine(spent, income) { money.formatWhole(it) },
            style = MaterialTheme.typography.bodyMedium,
            color = MaterialTheme.colorScheme.onSurfaceVariant,
        )
        Spacer(Modifier.height(14.dp))
        StackedBar(segments, "Out $out, in $inn", height = 10.dp)
        if (chips.isNotEmpty()) {
            Spacer(Modifier.height(16.dp))
            FlowRow(horizontalArrangement = Arrangement.spacedBy(8.dp), verticalArrangement = Arrangement.spacedBy(8.dp)) {
                chips.forEach { StatChip(it.icon, it.text) }
            }
        }
    }
}

/**
 * Two different readings as tiles side by side, stacked once the text is too large for two
 * columns to hold a figure on one line.
 */
@Composable
internal fun TilePair(
    modifier: Modifier = Modifier,
    first: @Composable (Modifier) -> Unit,
    second: @Composable (Modifier) -> Unit,
) {
    if (LocalDensity.current.fontScale > 1.5f) {
        Column(modifier.fillMaxWidth(), verticalArrangement = Arrangement.spacedBy(FIGURE_GAP)) {
            first(Modifier.fillMaxWidth())
            second(Modifier.fillMaxWidth())
        }
    } else {
        Row(modifier.fillMaxWidth().height(IntrinsicSize.Min), horizontalArrangement = Arrangement.spacedBy(FIGURE_GAP)) {
            first(Modifier.weight(1f).fillMaxHeight())
            second(Modifier.weight(1f).fillMaxHeight())
        }
    }
}

/** A list's zero: one quiet line, with the one act that changes it when there is one. */
@Composable
internal fun EmptyPanel(
    text: String,
    modifier: Modifier = Modifier,
    action: String? = null,
    onAction: (() -> Unit)? = null,
) {
    Panel(modifier) {
        Aside(text)
        if (action != null && onAction != null) {
            Row(Modifier.fillMaxWidth(), horizontalArrangement = Arrangement.End) {
                TextAction(action, onAction, color = MaterialTheme.colorScheme.primary)
            }
        }
    }
}
