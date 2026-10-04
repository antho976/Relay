package com.tally.app.ui.entry

import com.tally.core.AmountInput
import com.tally.core.Frequency
import com.tally.core.TxType
import kotlinx.serialization.Serializable
import java.time.LocalDate

/**
 * An [EntryDraft] as the editor keeps it in its SavedStateHandle, so a half-typed entry outlives
 * the process. The keypad's text is kept as typed (with its currency's digits) and the date as an
 * epoch day, so nothing here needs a serializer of its own.
 */
@Serializable
internal data class SavedEntry(
    val id: Long = 0,
    val type: TxType = TxType.EXPENSE,
    val amountText: String = "",
    val fractionDigits: Int = 2,
    val epochDay: Long,
    val accountId: Long? = null,
    val toAccountId: Long? = null,
    val categoryId: Long? = null,
    val categoryChosen: Boolean = false,
    val note: String = "",
    val repeat: Boolean = false,
    val frequency: Frequency = Frequency.MONTHLY,
    val recurringId: Long? = null,
) {
    fun toDraft(): EntryDraft = EntryDraft(
        id = id,
        type = type,
        amount = AmountInput(amountText, fractionDigits),
        date = LocalDate.ofEpochDay(epochDay),
        accountId = accountId,
        toAccountId = toAccountId,
        categoryId = categoryId,
        categoryChosen = categoryChosen,
        note = note,
        repeat = repeat,
        frequency = frequency,
        recurringId = recurringId,
    )

    companion object {
        fun of(d: EntryDraft): SavedEntry = SavedEntry(
            id = d.id,
            type = d.type,
            amountText = d.amount.text,
            fractionDigits = d.amount.fractionDigits,
            epochDay = d.date.toEpochDay(),
            accountId = d.accountId,
            toAccountId = d.toAccountId,
            categoryId = d.categoryId,
            categoryChosen = d.categoryChosen,
            note = d.note,
            repeat = d.repeat,
            frequency = d.frequency,
            recurringId = d.recurringId,
        )
    }
}
