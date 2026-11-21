package com.forge.app.ui.settings

import com.forge.app.program.Equipment
import org.junit.Assert.assertTrue
import org.junit.Test

class ExercisePreferencePoolTest {

    @Test
    fun fullCatalogIsDefaultAndGearIsAnOptionalFilter() {
        val available = setOf(Equipment.DUMBBELLS, Equipment.BENCH)
        val all = exercisePreferencePool(gearOnly = false, available = available, frozenIds = null)
        val gear = exercisePreferencePool(gearOnly = true, available = available, frozenIds = null)

        assertTrue("full catalog includes exercises beyond owned gear", all.any { Equipment.MACHINE in it.equipment })
        assertTrue("gear filter excludes unavailable equipment", gear.none { Equipment.MACHINE in it.equipment })
        assertTrue("owner-only plate stations never enter the public catalog", all.none { it.curatedOnly })
    }
}
