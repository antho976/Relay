package com.tally.app.ui.common

import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.PaddingValues
import androidx.compose.foundation.layout.Row
import androidx.compose.runtime.Composable
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.unit.dp
import com.tally.app.data.db.TransactionRow
import com.tally.app.ui.kit.CategoryBadge
import com.tally.app.ui.kit.EntryAmount
import com.tally.app.ui.kit.Glyph
import com.tally.app.ui.kit.ListRow
import com.tally.app.ui.kit.T
import com.tally.app.ui.kit.TransferBadge
import com.tally.app.ui.theme.Tally
import com.tally.core.TxType
import java.time.LocalDate

/** What an entry is called in a row: its note, else its category, else what kind it is. */
fun entryName(row: TransactionRow): String = when {
    row.note.isNotBlank() -> row.note
    row.type == TxType.TRANSFER -> "Transfer"
    else -> row.categoryName ?: "Uncategorized"
}

/** The line under it: the category (when the note is the title) and the account, or the transfer's way. */
fun entryLine(row: TransactionRow, day: String? = null): String {
    val where = when (row.type) {
        TxType.TRANSFER -> row.accountName + " → " + (row.toAccountName ?: "?")
        else -> listOfNotNull(row.categoryName.takeIf { row.note.isNotBlank() }, row.accountName).joinToString(" · ")
    }
    return listOfNotNull(day, where.ifBlank { null }).joinToString(" · ")
}

/**
 * One entry in a list: its badge, its name over its category and account, its amount. A bill that
 * posted itself carries Relay's repeat glyph. Tap edits; a long press deletes, with Undo.
 */
@Composable
fun EntryRow(
    row: TransactionRow,
    onClick: () -> Unit,
    modifier: Modifier = Modifier,
    onLongClick: (() -> Unit)? = null,
    today: LocalDate? = null,
) {
    val c = Tally.colors
    ListRow(modifier, onClick = onClick, onLongClick = onLongClick, padding = PaddingValues(horizontal = 4.dp, vertical = 8.dp)) {
        if (row.type == TxType.TRANSFER) TransferBadge() else CategoryBadge(row.categoryIcon, row.categoryColor)
        Column(Modifier.weight(1f), verticalArrangement = Arrangement.spacedBy(2.dp)) {
            Row(verticalAlignment = Alignment.CenterVertically, horizontalArrangement = Arrangement.spacedBy(5.dp)) {
                T(entryName(row), Modifier.weight(1f, fill = false), Tally.type.ui, c.ink, maxLines = 1)
                if (row.recurringId != null) Glyph("repeat", 12.dp, c.ink3)
            }
            T(entryLine(row, today?.let { Dates.day(row.date, it) }), style = Tally.type.caption, color = c.ink3, maxLines = 1)
        }
        EntryAmount(row.amount, row.type)
    }
}
