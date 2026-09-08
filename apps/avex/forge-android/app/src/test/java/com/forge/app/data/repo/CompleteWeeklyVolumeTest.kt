package com.forge.app.data.repo
import com.forge.app.data.db.dao.SessionDao.SessionVolumeDeloadRow
import java.time.*
import org.junit.Assert.*
import org.junit.Test
class CompleteWeeklyVolumeTest {
 @Test fun `weekly chart groups complete weeks before limiting history`() {
  val rows=(0 until 8).flatMap { week -> (0 until 4).map { day ->
   SessionVolumeDeloadRow((week*4+day).toLong(),"day",LocalDate.of(2026,7,6).plusWeeks(week.toLong()).plusDays(day.toLong()).atStartOfDay(ZoneOffset.UTC).toInstant().toEpochMilli(),100.0,false)
  } }
  val all=buildWeeklyTonnage(buildVolumeDeloadTrend(rows, Int.MAX_VALUE),ZoneOffset.UTC)
  assertEquals(8,all.size);assertEquals(400.0,all.first().volumeLb,0.001);assertEquals(400.0,all.last().volumeLb,0.001)
 }
}