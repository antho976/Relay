package com.forge.app.data.repo

import android.app.Application
import com.forge.app.data.db.inMemoryForgeDb
import com.forge.app.data.db.entities.ProgramCustomization
import com.forge.app.program.Program
import kotlinx.coroutines.runBlocking
import org.junit.After
import org.junit.Assert.*
import org.junit.Test
import org.junit.runner.RunWith
import org.robolectric.RobolectricTestRunner
import org.robolectric.annotation.Config

@RunWith(RobolectricTestRunner::class)
@Config(application = Application::class, sdk = [34])
class EffectiveSessionPlanTest {
    private val db = inMemoryForgeDb()
    private val repo = ProgramCustomizationRepository(db.programCustomizationDao())
    private val original = Program.days

    @After fun close() { Program.setActive(original); db.close() }

    @Test fun `phone and watch plan removes reorders adds and updates prescriptions`() = runBlocking {
        val day = Program.seedDays.first()
        Program.setActive(listOf(day))
        val first = day.exercises[0]
        val second = day.exercises[1]
        db.programCustomizationDao().upsert(ProgramCustomization(day.key, first.id, removed = true))
        db.programCustomizationDao().upsert(ProgramCustomization(day.key, second.id,
            setsOverride = 1, repRangeOverride = "12-15", orderOverride = -10))
        db.programCustomizationDao().upsert(ProgramCustomization(day.key, "custom_test",
            customName = "My movement", setsOverride = 2, orderOverride = -20))
        val plan = repo.effectivePlanForSession(day.key, emptyList())
        assertEquals("custom_test", plan.first().id)
        assertEquals(second.id, plan[1].id)
        assertEquals(1, plan[1].sets)
        assertEquals("12-15", plan[1].reps)
        assertFalse(plan.any { it.id == first.id })
        repo.setSetsOverride(day.key, second.id, 5)
        assertEquals(5, repo.effectivePlanForSession(day.key, emptyList())[1].sets)
        assertTrue(repo.effectivePlanForSession("unknown-day", listOf(first.id)).isEmpty())
    }
}
