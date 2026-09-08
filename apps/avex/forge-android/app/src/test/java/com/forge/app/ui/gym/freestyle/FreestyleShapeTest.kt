package com.forge.app.ui.gym.freestyle
import com.forge.app.domain.units.WeightUnit
import com.forge.app.program.MuscleGroup
import org.junit.Assert.*
import org.junit.Test
import org.junit.runner.RunWith
import org.robolectric.RobolectricTestRunner
@RunWith(RobolectricTestRunner::class)
class FreestyleShapeTest {
 @Test fun `custom hold and unknown imported bodyweight retain identity and shape after resume`() {
  val items = listOf(
   FsExercise("custom:hold", "My hold", MuscleGroup.CORE, bodyweight=true, timed=true, custom=true, sets=listOf(FsSet(hold="1:30"))),
   FsExercise("ext-imported", "Imported move", MuscleGroup.CORE, bodyweight=true, custom=true, sets=listOf(FsSet(reps="8")))
  )
  val json = draftFrom(items, 1000, WeightUnit.LB, "retry").toJson()
  val restored = draftToItems(FreestyleDraft.fromJson(json)!!, WeightUnit.KG)
  assertEquals(items, restored)
 }
}
