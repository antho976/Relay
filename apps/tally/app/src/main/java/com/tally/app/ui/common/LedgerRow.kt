package com.tally.app.ui.common

import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.heightIn
import androidx.compose.foundation.layout.padding
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.semantics.clearAndSetSemantics
import androidx.compose.ui.semantics.contentDescription
import androidx.compose.ui.semantics.semantics
import androidx.compose.ui.text.style.TextAlign
import androidx.compose.ui.unit.dp
import com.tally.app.data.db.TransactionRow
import com.tally.core.TxType

/**
 * One ledger entry, bare on the page as in Avex's Recent: badge, what it was, where and when,
 * and the amount. The whole row is the tap. Income carries a "+", an expense is just the amount.
 */
@Composable
fun LedgerRow(
    row: TransactionRow,
    meta: String,
    onClick: () -> Unit,
    modifier: Modifier = Modifier,
) {
    val money = LocalMoney.current
    val title = when {
        row.note.isNotBlank() -> row.note
        row.type == TxType.TRANSFER -> "Transfer"
        else -> row.categoryName ?: "Uncategorized"
    }
    val where = when (row.type) {
        TxType.TRANSFER -> "${row.accountName} to ${row.toAccountName.orEmpty()}"
        else -> listOfNotNull(row.categoryName.takeIf { row.note.isNotBlank() }, row.accountName).joinToString(" · ")
    }
    val amount = when (row.type) {
        TxType.INCOME -> money.formatSigned(row.amount)
        else -> money.format(row.amount)
    }
    Row(
        modifier
            .fillMaxWidth()
            .bounceClick(label = "Edit $title", onClick = onClick)
            .semantics(mergeDescendants = true) { contentDescription = "$title, $amount, $where, $meta" }
            .heightIn(min = 64.dp)
            .padding(vertical = 8.dp),
        verticalAlignment = Alignment.CenterVertically,
        horizontalArrangement = Arrangement.spacedBy(14.dp),
    ) {
        if (row.type == TxType.TRANSFER) TransferBadge() else CategoryBadge(row.categoryIcon, row.categoryColor)
        Column(Modifier.weight(1f).clearAndSetSemantics { }, verticalArrangement = Arrangement.spacedBy(2.dp)) {
            Text(title, style = MaterialTheme.typography.bodyLarge, color = MaterialTheme.colorScheme.onBackground)
            Text(
                listOf(meta, where).filter { it.isNotBlank() }.joinToString(" · "),
                style = MaterialTheme.typography.bodyMedium,
                color = MaterialTheme.colorScheme.onSurfaceVariant,
            )
        }
        Text(
            amount,
            modifier = Modifier.clearAndSetSemantics { },
            style = MaterialTheme.typography.titleMedium,
            color = if (row.type == TxType.TRANSFER) MaterialTheme.colorScheme.onSurfaceVariant else MaterialTheme.colorScheme.onBackground,
            textAlign = TextAlign.End,
        )
    }
}
