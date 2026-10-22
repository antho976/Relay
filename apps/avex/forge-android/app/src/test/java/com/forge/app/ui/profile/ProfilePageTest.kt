package com.forge.app.ui.profile

import androidx.compose.ui.test.assertIsDisplayed
import androidx.compose.ui.test.assertIsEnabled
import androidx.compose.ui.test.assertIsNotEnabled
import androidx.compose.ui.test.junit4.createComposeRule
import androidx.compose.ui.test.onNodeWithContentDescription
import androidx.compose.ui.test.onNodeWithText
import androidx.compose.ui.test.performClick
import androidx.compose.ui.test.performScrollTo
import com.forge.app.data.repo.ProgressPhoto
import com.forge.app.ui.profile.state.ProfileUiState
import com.forge.app.ui.theme.ForgeTheme
import java.time.LocalDate
import org.junit.Assert.assertEquals
import org.junit.Assert.assertNull
import org.junit.Assert.assertTrue
import org.junit.Rule
import org.junit.Test
import org.junit.runner.RunWith
import org.robolectric.RobolectricTestRunner
import org.robolectric.annotation.Config

/** The Profile page's interactions: the Consistency month, the tabs, and the gallery lock in place. */
@RunWith(RobolectricTestRunner::class)
@Config(sdk = [34], qualifiers = "w412dp-h2000dp-mdpi")
class ProfilePageTest {

    @get:Rule
    val compose = createComposeRule()

    private fun setContent(
        tab: ProfileTab = ProfileTab.OVERVIEW,
        state: ProfileUiState = ProfileFixtures.state(),
        locked: Boolean = false,
        actions: ProfileActions = ProfileActions(),
        photo: @androidx.compose.runtime.Composable (ProgressPhoto, androidx.compose.ui.Modifier) -> Unit = ProfileFixtures.photo,
    ) {
        compose.setContent {
            ForgeTheme {
                ProfilePage(
                    state = state,
                    body = ProfileFixtures.body(),
                    weightUnit = ProfileFixtures.unit,
                    galleryLocked = locked,
                    today = ProfileFixtures.TODAY,
                    nowMs = ProfileFixtures.NOW,
                    planOn = ProfileFixtures.planOn,
                    onVacation = ProfileFixtures.onVacation,
                    actions = actions,
                    cover = null,
                    photo = photo,
                    initialTab = tab,
                )
            }
        }
    }

    @Test
    fun tappingATrainedDayOpensThatDay() {
        var tapped: LocalDate? = null
        setContent(actions = ProfileActions(onDayTap = { tapped = it }))
        compose.onNodeWithContentDescription("October 20, 1 session").performScrollTo().performClick()
        assertEquals(LocalDate.of(2026, 10, 20), tapped)
    }

    /** A rest day has nothing to show, so it stays inert rather than opening an empty sheet. */
    @Test
    fun tappingARestDayDoesNothing() {
        var tapped: LocalDate? = null
        setContent(actions = ProfileActions(onDayTap = { tapped = it }))
        compose.onNodeWithContentDescription("October 15, planned rest").performScrollTo().performClick()
        assertNull(tapped)
    }

    @Test
    fun theEarlierChevronPagesBackAMonth() {
        setContent()
        compose.onNodeWithContentDescription("October 2026").assertIsDisplayed()
        compose.onNodeWithContentDescription("Earlier month").performScrollTo().performClick()
        compose.onNodeWithContentDescription("September 2026").assertIsDisplayed()
        compose.onNodeWithContentDescription("September 30, 1 session").assertExists()
    }

    /** Forward stops at this month; back stops at the oldest month there is anything in. */
    @Test
    fun theChevronsStopAtBothEnds() {
        val one = ProfileUiState(loading = false, activityByDay = mapOf(LocalDate.of(2026, 9, 1).toEpochDay() to 1))
        setContent(state = one)
        compose.onNodeWithContentDescription("Later month").assertIsNotEnabled()
        compose.onNodeWithContentDescription("Earlier month").assertIsEnabled().performClick()
        compose.onNodeWithContentDescription("Later month").assertIsEnabled()
        compose.onNodeWithContentDescription("Earlier month").assertIsNotEnabled()
    }

    @Test
    fun aLockedGalleryShowsTheCountAndNeverDrawsAPhoto() {
        var unlocked = false
        setContent(
            tab = ProfileTab.PHOTOS,
            locked = true,
            actions = ProfileActions(onUnlock = { unlocked = true }),
            photo = { _, _ -> error("A locked gallery must not draw a photo") }
        )
        compose.onNodeWithText("GALLERY · 38 PHOTOS").assertIsDisplayed()
        compose.onNodeWithText("Your photos are locked").assertIsDisplayed()
        compose.onNodeWithText("Unlock").performClick()
        assertTrue(unlocked)
    }

    /** The lock gates Photos only: the other tabs stay open. */
    @Test
    fun theLockLeavesTheOtherTabsOpen() {
        setContent(tab = ProfileTab.PHOTOS, locked = true, photo = { _, _ -> error("locked") })
        compose.onNodeWithText("BODY").performClick()
        compose.onNodeWithText("WEIGHT · 7-DAY AVERAGE").assertIsDisplayed()
    }

    @Test
    fun theCameraIsThePhotosTabsOneAction() {
        var opened = false
        setContent(tab = ProfileTab.PHOTOS, actions = ProfileActions(onOpenCamera = { opened = true }))
        compose.onNodeWithText("Take today's photo").performClick()
        assertTrue(opened)
    }

    @Test
    fun eachCompositionReadingOpensItsOwnLog() {
        var fat = false
        var size = false
        var sizes = false
        setContent(
            tab = ProfileTab.BODY,
            actions = ProfileActions(onLogBodyFat = { fat = true }, onLogSize = { size = true }, onOpenSizes = { sizes = true })
        )
        compose.onNodeWithText("BODY FAT").performScrollTo().performClick()
        compose.onNodeWithText("WAIST").performScrollTo().performClick()
        compose.onNodeWithText("1 OF 5 SIZES").performScrollTo().performClick()
        assertTrue(fat && size && sizes)
    }
}
