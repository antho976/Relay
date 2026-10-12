package com.tally.app.ui.kit

import androidx.compose.runtime.Composable
import androidx.compose.ui.Modifier
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.text.TextStyle
import com.tally.app.ui.common.LocalMoney
import com.tally.app.ui.theme.Tally
import com.tally.core.TxType

/**
 * An entry's amount as a row shows it: money in with a plus in Relay's live green, a transfer in
 * ink-2, money out plain. Never red for an ordinary expense: red is for over budget.
 */
@Composable
fun EntryAmount(amount: Long, type: TxType, modifier: Modifier = Modifier, style: TextStyle = Tally.type.amount) {
    val fmt = LocalMoney.current
    val c = Tally.colors
    val (text, color) = when (type) {
        TxType.INCOME -> "+" + fmt.format(amount) to c.liveText
        TxType.TRANSFER -> fmt.format(amount) to c.ink2
        TxType.EXPENSE -> fmt.format(amount) to c.ink
    }
    T(text, modifier, style, color, maxLines = 1)
}

/** A signed balance or net: below zero reads in held red's text colour. */
@Composable
fun Balance(amount: Long, modifier: Modifier = Modifier, style: TextStyle = Tally.type.amount, color: Color? = null) {
    val fmt = LocalMoney.current
    T(fmt.format(amount), modifier, style, color ?: if (amount < 0) Tally.colors.heldText else Tally.colors.ink, maxLines = 1)
}

/** Formats with the owner's currency, for text built in composition. */
@Composable
fun money(amount: Long): String = LocalMoney.current.format(amount)

@Composable
fun moneyWhole(amount: Long): String = LocalMoney.current.formatWhole(amount)
