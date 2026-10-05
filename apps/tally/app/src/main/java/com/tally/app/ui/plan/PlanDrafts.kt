package com.tally.app.ui.plan

import com.tally.core.Frequency
import com.tally.core.GoalKind
import com.tally.core.TxType
import kotlinx.serialization.Serializable
import java.time.LocalDate

/*
 * The Plan editors' drafts as they are kept in a SavedStateHandle, so typed input outlives the
 * process. Dates are kept as epoch days: plain numbers, no date serializer to maintain.
 */

/** A [BillDraft] as kept. */
@Serializable
internal data class SavedBill(
    val id: Long = 0,
    val name: String = "",
    val type: TxType = TxType.EXPENSE,
    val amountText: String = "",
    val amount: Long? = null,
    val accountId: Long? = null,
    val toAccountId: Long? = null,
    val categoryId: Long? = null,
    val frequency: Frequency = Frequency.MONTHLY,
    val interval: Int = 1,
    val anchorEpochDay: Long,
    val autoPost: Boolean = true,
    val active: Boolean = true,
) {
    fun toDraft(): BillDraft = BillDraft(
        id = id,
        name = name,
        type = type,
        amountText = amountText,
        amount = amount,
        accountId = accountId,
        toAccountId = toAccountId,
        categoryId = categoryId,
        frequency = frequency,
        interval = interval,
        anchorDate = LocalDate.ofEpochDay(anchorEpochDay),
        autoPost = autoPost,
        active = active,
    )

    companion object {
        fun of(d: BillDraft): SavedBill = SavedBill(
            id = d.id,
            name = d.name,
            type = d.type,
            amountText = d.amountText,
            amount = d.amount,
            accountId = d.accountId,
            toAccountId = d.toAccountId,
            categoryId = d.categoryId,
            frequency = d.frequency,
            interval = d.interval,
            anchorEpochDay = d.anchorDate.toEpochDay(),
            autoPost = d.autoPost,
            active = d.active,
        )
    }
}

/** A [GoalDraft] as kept. */
@Serializable
internal data class SavedGoal(
    val id: Long = 0,
    val name: String = "",
    val targetText: String = "",
    val target: Long? = null,
    val targetEpochDay: Long? = null,
    val color: Int = 0,
    val archived: Boolean = false,
    val kind: GoalKind = GoalKind.SAVINGS,
    val accountId: Long? = null,
    val percent: Int = 10,
    val byShare: Boolean = true,
) {
    fun toDraft(): GoalDraft = GoalDraft(
        id = id,
        name = name,
        targetText = targetText,
        target = target,
        targetDate = targetEpochDay?.let { LocalDate.ofEpochDay(it) },
        color = color,
        archived = archived,
        kind = kind,
        accountId = accountId,
        percent = percent,
        byShare = byShare,
    )

    companion object {
        fun of(d: GoalDraft): SavedGoal = SavedGoal(
            id = d.id,
            name = d.name,
            targetText = d.targetText,
            target = d.target,
            targetEpochDay = d.targetDate?.toEpochDay(),
            color = d.color,
            archived = d.archived,
            kind = d.kind,
            accountId = d.accountId,
            percent = d.percent,
            byShare = d.byShare,
        )
    }
}
