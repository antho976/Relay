package com.forge.app.ui.gym.stats.components

import com.forge.app.program.MuscleGroup
import org.junit.Assert.assertEquals
import org.junit.Test

class MusclesReadoutTest {

    @Test fun `lists trained muscles most sets first`() {
        val text = musclesReadout(mapOf(MuscleGroup.CHEST to 3, MuscleGroup.QUADS to 9, MuscleGroup.BICEPS to 0))
        assertEquals("${MuscleGroup.QUADS.displayName} 9 sets, ${MuscleGroup.CHEST.displayName} 3 sets", text)
    }

    @Test fun `one set is singular and nothing trained says so`() {
        assertEquals("${MuscleGroup.CHEST.displayName} 1 set", musclesReadout(mapOf(MuscleGroup.CHEST to 1)))
        assertEquals("No muscles trained", musclesReadout(emptyMap()))
    }
}
