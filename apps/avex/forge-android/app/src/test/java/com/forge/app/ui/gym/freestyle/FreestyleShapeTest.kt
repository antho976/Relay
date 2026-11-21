package com.forge.app.ui.gym.freestyle
import com.forge.app.domain.units.WeightUnit
import com.forge.app.program.CustomExerciseDef
import com.forge.app.program.CustomExerciseRegistry
import com.forge.app.program.MuscleGroup
import org.junit.After
import org.junit.Assert.*
import org.junit.Test
import org.junit.runner.RunWith
import org.robolectric.RobolectricTestRunner
@RunWith(RobolectricTestRunner::class)
class FreestyleShapeTest {
 @After fun tearDown() = CustomExerciseRegistry.clear()

 @Test fun `custom hold and unknown imported bodyweight retain identity and shape after resume`() {
  val items = listOf(
   FsExercise("custom-hold", "My hold", MuscleGroup.CORE, bodyweight=true, timed=true, custom=true, sets=listOf(FsSet(hold="1:30"))),
   // An imported id no program knows has no muscle; it must not come back as a guessed one.
   FsExercise("ext-imported", "Imported move", null, bodyweight=true, custom=true, sets=listOf(FsSet(reps="8")))
  )
  val json = draftFrom(items, 1000, WeightUnit.LB, "retry").toJson()
  val restored = draftToItems(FreestyleDraft.fromJson(json)!!, WeightUnit.KG)
  assertEquals(items, restored)
 }

 // Audit 2026-09-26 (02): repeating a workout labelled every non-library move Chest, and an
 // earlier build's save then registered that Chest for the imported id.
 @Test fun `repeating a workout keeps imported and seed moves out of Chest`() {
  CustomExerciseRegistry.put(CustomExerciseDef("ext-lat-pulldown-cable", "Lat Pulldown (Cable)", MuscleGroup.CHEST.code))
  CustomExerciseRegistry.put(CustomExerciseDef("custom-sled", "Sled", MuscleGroup.QUADS.code))
  val template = listOf(
   FreestyleTemplateExercise("ext-lat-pulldown-cable", listOf(FreestyleTemplateSet(50.0, 10)), customName = "Lat Pulldown (Cable)"),
   FreestyleTemplateExercise("la6", listOf(FreestyleTemplateSet(null, 12)), customName = "Hanging Knee Raise"),
   FreestyleTemplateExercise("ub1", listOf(FreestyleTemplateSet(40.0, 8)), customName = "DB Row (1-arm)"),
   FreestyleTemplateExercise("custom-sled", listOf(FreestyleTemplateSet(90.0, 6)), customName = "Sled", muscleCode = MuscleGroup.QUADS.code)
  )
  val items = template.toItems(WeightUnit.LB).associateBy { it.libId }
  assertNull(items.getValue("ext-lat-pulldown-cable").muscle)
  assertEquals(MuscleGroup.CORE, items.getValue("la6").muscle)
  assertTrue(items.getValue("la6").bodyweight)
  assertEquals(MuscleGroup.BACK, items.getValue("ub1").muscle)
  assertFalse(items.getValue("ub1").bodyweight)
  assertEquals(MuscleGroup.QUADS, items.getValue("custom-sled").muscle)
  // A draft written by an earlier build carries the wrong Chest for the imported id; it is re-derived.
  val stale = FreestyleDraft(
   openedAtMs = 0,
   exercises = listOf(FreestyleDraftExercise("ext-lat-pulldown-cable", listOf(FreestyleDraftSet("50", "10")), name = "Lat Pulldown (Cable)", muscleCode = MuscleGroup.CHEST.code)),
   unitLabel = WeightUnit.LB.label
  )
  assertNull(draftToItems(stale, WeightUnit.LB).single().muscle)
  // And the Chest an earlier build already registered no longer reaches the aggregations.
  assertNull(com.forge.app.program.Program.exercise("ext-lat-pulldown-cable"))
  assertEquals(MuscleGroup.QUADS, com.forge.app.program.Program.exercise("custom-sled")?.muscle)
 }

 // A legacy plank kept its seconds in `reps` with a null duration; repeating it must not drop the set.
 @Test fun `repeating a workout keeps legacy timed sets stored in reps`() {
  val template = listOf(FreestyleTemplateExercise("plank", listOf(FreestyleTemplateSet(null, 60))))
  val plank = template.toItems(WeightUnit.LB).single()
  assertTrue(plank.timed)
  assertEquals(listOf("1:00"), plank.sets.map { it.hold })
 }

 // The exercise browser must find the user's own moves so a re-typed name reuses the custom id.
 @Test fun `browser search finds a registered custom move`() {
  CustomExerciseRegistry.put(CustomExerciseDef("custom-sled-push", "Sled Push", MuscleGroup.QUADS.code))
  val custom = customBrowserDefs()
  assertEquals(listOf("custom-sled-push"), custom.map { it.id })
  val hits = browseLibrary("sled", null, false, emptySet(), emptySet(), custom)
  assertEquals(listOf("custom-sled-push"), hits.map { it.id })
  assertTrue(browseLibrary("sled", null, false, emptySet(), setOf("custom-sled-push"), custom).isEmpty())
  assertEquals("Sled Push", fsExerciseFor("custom-sled-push")?.name)
 }
}
