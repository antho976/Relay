package com.forge.app.ui.profile

import androidx.compose.ui.test.junit4.createComposeRule
import androidx.compose.ui.test.onNodeWithText
import androidx.compose.ui.test.performClick
import com.forge.app.data.db.entities.BodyweightEntry
import com.forge.app.domain.units.WeightUnit
import com.forge.app.ui.theme.ForgeTheme
import java.time.LocalDate
import org.junit.Assert.assertEquals
import org.junit.Assert.assertNull
import org.junit.Rule
import org.junit.Test
import org.junit.runner.RunWith
import org.robolectric.RobolectricTestRunner
import org.robolectric.annotation.Config
import org.robolectric.annotation.GraphicsMode

/** Audit 2026-09-26 (03): the bodyweight log sheet and the BODY row. */
@RunWith(RobolectricTestRunner::class)
@GraphicsMode(GraphicsMode.Mode.NATIVE)
@Config(sdk = [34], qualifiers = "w360dp-h800dp-mdpi")
class BodyweightAuditTest {
    @get:Rule val compose = createComposeRule()

    @Test
    fun aDayOutsideTheRecentWindowSeedsFromStorageAndKeepsItsNote() {
        // The sheet's entries are only the recent window; the picked day's weigh-in is not in them.
        val day = LocalDate.now()
        val stored = BodyweightEntry(id = 7, dateKey = day.toString(), weightLb = 181.0, recordedAt = 0, note = "fasted")
        var saved: Triple<Double, LocalDate, String?>? = null
        compose.setContent {
            ForgeTheme {
                BodyweightLogSheet(
                    entries = listOf(BodyweightEntry(id = 1, dateKey = day.minusDays(1).toString(), weightLb = 190.0, recordedAt = 0)),
                    canImport = false,
                    message = null,
                    lookupDay = { if (it == day) stored else null },
                    onSave = { lb, d, note -> saved = Triple(lb, d, note) },
                    onImport = {},
                    onDismiss = {}
                )
            }
        }
        compose.onNodeWithText("fasted").assertExists()
        compose.onNodeWithText("Save").performClick()
        compose.runOnIdle {
            val (lb, d, note) = saved!!
            // The day's own weight, not the latest one, and an untouched note is left to the day.
            assertEquals(181.0, lb, 0.01)
            assertEquals(day, d)
            assertNull(note)
        }
    }

    @Test
    fun stonesReadAsStonesAndPoundsWithAUnitOnTheDelta() {
        val day = 86_400_000L
        val entries = listOf(
            BodyweightEntry(id = 1, dateKey = "2026-09-01", weightLb = 182.0, recordedAt = 10 * day),
            BodyweightEntry(id = 2, dateKey = "2026-09-08", weightLb = 180.0, recordedAt = 17 * day)
        )
        val st = weightReading(entries, goalLb = null, unit = WeightUnit.ST)
        // 180 lb is 12 st 12 lb; the whole-stone figure used to read "13".
        assertEquals("12 st 12 lb", st.figure)
        assertEquals("", st.unit)
        assertEquals("↓ 2 lb since Sep 1", st.change)

        val lb = weightReading(entries, goalLb = null, unit = WeightUnit.LB)
        assertEquals("180.0", lb.figure)
        assertEquals(" lb", lb.unit)
        assertEquals("↓ 2.0 lb since Sep 1", lb.change)
    }
}
