package com.tally.app.ui.plan

import com.tally.core.AmountInput

/** One key of the budget keypad. */
sealed interface PadKey {
    data class Digit(val value: Int) : PadKey
    data object Decimal : PadKey
    data object Backspace : PadKey
}

/** 1 2 3 / 4 5 6 / 7 8 9 / separator 0 backspace; no separator cell for a currency without minor units. */
internal fun padRows(showDecimal: Boolean): List<List<PadKey?>> = listOf(
    listOf(PadKey.Digit(1), PadKey.Digit(2), PadKey.Digit(3)),
    listOf(PadKey.Digit(4), PadKey.Digit(5), PadKey.Digit(6)),
    listOf(PadKey.Digit(7), PadKey.Digit(8), PadKey.Digit(9)),
    listOf(if (showDecimal) PadKey.Decimal else null, PadKey.Digit(0), PadKey.Backspace),
)

/** What one key press does to the typed amount. */
internal fun AmountInput.pressed(key: PadKey): AmountInput = when (key) {
    is PadKey.Digit -> digit(key.value)
    PadKey.Decimal -> decimal()
    PadKey.Backspace -> backspace()
}
