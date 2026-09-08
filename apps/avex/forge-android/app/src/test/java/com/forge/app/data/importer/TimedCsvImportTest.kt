package com.forge.app.data.importer
import org.junit.Assert.*
import org.junit.Test
class TimedCsvImportTest {
 @Test fun `Strong timed hold preserves duration`() {
  val header="Date,Workout Name,Duration,Exercise Name,Set Order,Weight,Weight Unit,Reps,Seconds"
  val prefix="2026-09-07 12:00:00,Test,10m,Plank,1,0,kg,"
  assertEquals(1,StrongImporter().parse(header+"\n"+prefix+"8,0",false).size)
  assertEquals(90,StrongImporter().parse(header+"\n"+prefix+",90",false).single().exercises.single().sets.single().durationSeconds)
 }
 @Test fun `Hevy timed hold preserves duration`() {
  val header="title,start_time,end_time,exercise_title,set_index,set_type,weight_kg,reps,duration_seconds"
  val prefix="Test,2026-09-07 12:00:00,2026-09-07 12:10:00,Plank,1,normal,0,"
  assertEquals(1,HevyImporter().parse(header+"\n"+prefix+"8,0",false).size)
  assertEquals(90,HevyImporter().parse(header+"\n"+prefix+",90",false).single().exercises.single().sets.single().durationSeconds)
 }
}