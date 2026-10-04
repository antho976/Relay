package com.tally.core

/**
 * The keypad's state: what has been typed, held as text so "12." and "12.0" stay distinct while
 * typing, and converted to minor units only when read. Immutable; every key returns a new value.
 */
data class AmountInput(val text: String = "", val fractionDigits: Int = 2) {

    val minor: Long get() = MoneyFormatter.parseAmount(text.ifEmpty { "0" }, fractionDigits) ?: 0L

    val isEmpty: Boolean get() = minor == 0L

    private val hasDecimal get() = text.contains('.')
    private val decimals get() = if (hasDecimal) text.substringAfter('.').length else 0
    private val integerDigits get() = text.substringBefore('.').length

    fun digit(d: Int): AmountInput {
        require(d in 0..9)
        if (hasDecimal && decimals >= fractionDigits) return this
        if (!hasDecimal && integerDigits >= MAX_INTEGER_DIGITS) return this
        // No leading zeros: "0" then "5" is "5", not "05".
        if (text == "0") return copy(text = d.toString())
        return copy(text = text + d)
    }

    fun decimal(): AmountInput {
        if (fractionDigits == 0 || hasDecimal) return this
        return copy(text = if (text.isEmpty()) "0." else "$text.")
    }

    fun backspace(): AmountInput = copy(text = text.dropLast(1))

    fun clear(): AmountInput = copy(text = "")

    companion object {
        /** Ten billion in any currency is not a purchase; the cap keeps a Long far from overflow. */
        const val MAX_INTEGER_DIGITS = 10

        fun of(minor: Long, fractionDigits: Int): AmountInput {
            if (minor <= 0) return AmountInput("", fractionDigits)
            val scale = MoneyFormatter.pow10(fractionDigits)
            val whole = minor / scale
            val frac = minor % scale
            val text = when {
                fractionDigits == 0 -> whole.toString()
                frac == 0L -> whole.toString()
                else -> "$whole." + frac.toString().padStart(fractionDigits, '0').trimEnd('0')
            }
            return AmountInput(text, fractionDigits)
        }
    }
}
