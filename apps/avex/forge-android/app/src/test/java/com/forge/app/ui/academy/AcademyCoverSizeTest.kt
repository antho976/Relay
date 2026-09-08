package com.forge.app.ui.academy
import org.junit.Assert.*
import org.junit.Test
@org.junit.runner.RunWith(org.robolectric.RobolectricTestRunner::class)
class AcademyCoverSizeTest {
 @Test fun `hero and thumbnail decodes stay within requested bounds`() {
  for (size in listOf(256, 512, 1024, 4096)) {
   val sample = AcademyCoverCache.sampleSize(1200,1600,size)
   assertTrue(1600/sample <= size.coerceAtMost(1024))
   assertTrue(sample >= 2)
  }
 }
}
