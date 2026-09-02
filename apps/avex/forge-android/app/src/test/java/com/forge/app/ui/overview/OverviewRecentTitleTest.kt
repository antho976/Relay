package com.forge.app.ui.overview

import com.forge.app.domain.cardio.CustomCardioType
import com.forge.app.ui.overview.state.OverviewRecentItem
import org.junit.Assert.assertEquals
import org.junit.Test
import org.junit.runner.RunWith
import org.robolectric.RobolectricTestRunner

/**
 * Home's recent-row title for a cardio session. The mapper used to title-case the stored type
 * code, so a custom activity read `Cardio · Custom_ab12cd34` and HIIT read `Cardio · Hiit`. The
 * code is carried on the item now and resolved through the user's definitions at composition.
 */
@RunWith(RobolectricTestRunner::class)
class OverviewRecentTitleTest {

    private val padel = CustomCardioType("custom_ab12cd34", "Padel")

    private fun cardio(code: String) = OverviewRecentItem(
        dayLabel = "TODAY",
        title = cardioRecentTitle("Other"),
        subtitle = "30 min",
        tag = "MOVE",
        isGym = false,
        cardioTypeCode = code
    )

    @Test
    fun aCustomActivityReadsItsOwnName() {
        assertEquals("Cardio · Padel", cardio(padel.code).displayTitle(listOf(padel)))
    }

    @Test
    fun aDeletedDefinitionFallsBackToOtherNeverTheStorageCode() {
        assertEquals("Cardio · Other", cardio(padel.code).displayTitle(emptyList()))
    }

    @Test
    fun builtInCodesResolveToTheirRealNames() {
        assertEquals("Cardio · HIIT", cardio("hiit").displayTitle(emptyList()))
        assertEquals("Cardio · Run", cardio("run").displayTitle(listOf(padel)))
    }

    @Test
    fun gymRowsAndCodelessRowsKeepTheirTitle() {
        val gym = OverviewRecentItem(dayLabel = "TODAY", title = "Push A", subtitle = "", tag = "PUSH")
        assertEquals("Push A", gym.displayTitle(listOf(padel)))
        val codeless = cardio("run").copy(cardioTypeCode = null, title = "Cardio · Row")
        assertEquals("Cardio · Row", codeless.displayTitle(listOf(padel)))
    }
}
