package com.forge.app.core
import kotlinx.coroutines.CompletableDeferred
import kotlinx.coroutines.test.*
import org.junit.Assert.*
import org.junit.Test
@OptIn(kotlinx.coroutines.ExperimentalCoroutinesApi::class)
class ConflatedRefreshTest {
 @Test fun `burst coalesces but request during read gets fresh followup`() = runTest {
  var reads = 0
  val gate = CompletableDeferred<Unit>()
  val refresh = ConflatedRefresh(backgroundScope) { reads++; if (reads == 1) gate.await() }
  repeat(10) { refresh.request() }
  runCurrent(); advanceTimeBy(101); runCurrent()
  assertEquals(1, reads)
  repeat(10) { refresh.request() }
  runCurrent(); advanceTimeBy(101); runCurrent()
  assertEquals(1, reads)
  gate.complete(Unit); runCurrent(); advanceTimeBy(101); runCurrent()
  assertEquals(2, reads)
 }
}
