package com.forge.shared.protocol
import org.junit.Assert.*
import org.junit.Test
class LoadAdjustmentTest {
 @Test fun `signed load adjustment and legacy payloads share honest text`() {
  for (value in listOf(-5,0,5)) {
   assertEquals(value, GlanceTodayDto(computedAtMs=0, loadAdjustmentPercent=value).loadAdjustment)
   assertEquals(value, GlanceTodayDto(computedAtMs=0, readinessPercent=value).loadAdjustment)
  }
  assertEquals("-5%", loadAdjustmentText(-5))
  assertEquals("0%", loadAdjustmentText(0))
  assertEquals("+5%", loadAdjustmentText(5))
  assertNull(GlanceTodayDto(computedAtMs=0, readinessPercent=82).loadAdjustment)
 }
}
