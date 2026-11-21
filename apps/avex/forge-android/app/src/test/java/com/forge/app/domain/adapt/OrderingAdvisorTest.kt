package com.forge.app.domain.adapt

import com.forge.app.domain.adapt.OrderingAdvisor.OrderingItem
import com.forge.app.program.MuscleGroup
import org.junit.Assert.assertEquals
import org.junit.Assert.assertNotNull
import org.junit.Assert.assertNull
import org.junit.Assert.assertTrue
import org.junit.Test

/** System 3 of the adaptation engine: fatigue-aware session ordering. */
class OrderingAdvisorTest {

    private var nextId = 0

    private fun item(
        muscle: MuscleGroup,
        compound: Boolean = false,
        group: String? = null,
        id: String = "ex${nextId++}"
    ) = OrderingItem(exerciseId = id, muscle = muscle, isCompound = compound, supersetGroup = group)

    private fun suggest(items: List<OrderingItem>, priority: Set<MuscleGroup> = emptySet()) =
        OrderingAdvisor.suggestOrder("upper-a", items, priority)

    // ── Silence (the "cold start" of a structural rule: no problem → no advice) ──

    @Test
    fun alternatingMuscles_noAdjacency_staysSilent() {
        assertNull(
            suggest(listOf(
                item(MuscleGroup.CHEST, compound = true),
                item(MuscleGroup.BACK, compound = true),
                item(MuscleGroup.SHOULDERS),
                item(MuscleGroup.BICEPS)
            ))
        )
    }

    @Test
    fun tooFewExercises_staysSilent() {
        assertNull(suggest(listOf(item(MuscleGroup.CHEST), item(MuscleGroup.CHEST))))
    }

    @Test
    fun unfixable_allSameMuscle_staysSilent() {
        // Any order of three chest movements still has two chest-chest pairings — a reorder
        // that fixes nothing is noise, not advice.
        assertNull(
            suggest(listOf(
                item(MuscleGroup.CHEST, compound = true),
                item(MuscleGroup.CHEST, compound = true),
                item(MuscleGroup.CHEST, compound = true)
            ))
        )
    }

    // ── Happy path ─────────────────────────────────────────────────────────────

    @Test
    fun backToBackChest_getsSpacedByTheRowBetween() {
        val bench = item(MuscleGroup.CHEST, compound = true, id = "bench")
        val incline = item(MuscleGroup.CHEST, compound = true, id = "incline")
        val row = item(MuscleGroup.BACK, compound = true, id = "row")
        val curls = item(MuscleGroup.BICEPS, id = "curls")

        val s = suggest(listOf(bench, incline, row, curls))
        assertNotNull(s)
        assertEquals(listOf("bench", "row", "incline", "curls"), s!!.orderedExerciseIds)
        assertEquals(1, s.adjacenciesBefore)
        assertEquals(0, s.adjacenciesAfter)
        assertEquals(Confidence.MEDIUM, s.confidence)
        assertTrue(s.reason.contains("pre-fatigued"))
    }

    @Test
    fun fixingTwoPairings_isHighConfidence() {
        val s = suggest(listOf(
            item(MuscleGroup.CHEST, compound = true, id = "c1"),
            item(MuscleGroup.CHEST, compound = true, id = "c2"),
            item(MuscleGroup.BACK, compound = true, id = "b1"),
            item(MuscleGroup.BACK, compound = true, id = "b2")
        ))
        assertNotNull(s)
        assertEquals(listOf("c1", "b1", "c2", "b2"), s!!.orderedExerciseIds)
        assertEquals(2, s.adjacenciesBefore)
        assertEquals(0, s.adjacenciesAfter)
        assertEquals(Confidence.HIGH, s.confidence)
    }

    // ── Hard constraints ───────────────────────────────────────────────────────

    @Test
    fun compoundsStayEarly_evenWhenAnIsolationOpenedTheDay() {
        val s = suggest(listOf(
            item(MuscleGroup.SHOULDERS, id = "raises"),
            item(MuscleGroup.CHEST, compound = true, id = "bench"),
            item(MuscleGroup.CHEST, compound = true, id = "incline"),
            item(MuscleGroup.BACK, compound = true, id = "row")
        ))
        assertNotNull(s)
        assertEquals(listOf("bench", "row", "incline", "raises"), s!!.orderedExerciseIds)
    }

    @Test
    fun supersetGroupsAreNeverSplit() {
        // The chest pairing is fixed inside the compound block; the biceps/triceps superset
        // must come through the reorder as one intact, internally-ordered block.
        val s = suggest(listOf(
            item(MuscleGroup.CHEST, compound = true, id = "c1"),
            item(MuscleGroup.CHEST, compound = true, id = "c2"),
            item(MuscleGroup.BACK, compound = true, id = "row"),
            item(MuscleGroup.BICEPS, group = "ss1", id = "curl"),
            item(MuscleGroup.TRICEPS, group = "ss1", id = "ext"),
            item(MuscleGroup.SHOULDERS, id = "raises")
        ))
        assertNotNull(s)
        val order = s!!.orderedExerciseIds
        assertEquals(listOf("c1", "row", "c2", "curl", "ext", "raises"), order)
        assertEquals("superset must stay adjacent and in internal order",
            order.indexOf("curl") + 1, order.indexOf("ext"))
    }

    @Test
    fun priorityMuscleGetsTheFreshestSlotInItsBlock() {
        // Isolation block: triceps, triceps, shoulders, biceps with shoulders prioritized —
        // shoulders open the block AND the triceps pairing gets spaced.
        val s = suggest(
            listOf(
                item(MuscleGroup.TRICEPS, id = "t1"),
                item(MuscleGroup.TRICEPS, id = "t2"),
                item(MuscleGroup.SHOULDERS, id = "sh"),
                item(MuscleGroup.BICEPS, id = "bi")
            ),
            priority = setOf(MuscleGroup.SHOULDERS)
        )
        assertNotNull(s)
        assertEquals(listOf("sh", "t1", "bi", "t2"), s!!.orderedExerciseIds)
    }

    // ── Conflicting constraints: adjacency fix loses to the superset block ──────

    @Test
    fun conflict_supersetBlockingTheOnlyFix_staysSilent() {
        // chest + (chest,back superset): the only way to space the chest pair is to split
        // the superset, which is forbidden — so any reorder still has 1 pairing → silent.
        val s = suggest(listOf(
            item(MuscleGroup.CHEST, compound = true, id = "c1"),
            item(MuscleGroup.CHEST, compound = true, group = "ss1", id = "c2"),
            item(MuscleGroup.BACK, compound = true, group = "ss1", id = "row")
        ))
        assertNull(s)
    }

    // ── Determinism ────────────────────────────────────────────────────────────

    @Test
    fun sameInputsProduceTheSameOrder() {
        val items = listOf(
            item(MuscleGroup.CHEST, compound = true, id = "c1"),
            item(MuscleGroup.CHEST, compound = true, id = "c2"),
            item(MuscleGroup.BACK, compound = true, id = "b1"),
            item(MuscleGroup.SHOULDERS, id = "s1")
        )
        assertEquals(suggest(items), suggest(items))
    }
}
