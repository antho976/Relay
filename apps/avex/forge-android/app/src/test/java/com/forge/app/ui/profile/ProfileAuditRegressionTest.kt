package com.forge.app.ui.profile

import androidx.compose.material3.MaterialTheme
import androidx.compose.ui.test.*
import androidx.compose.ui.test.junit4.createComposeRule
import com.forge.app.data.repo.ProgressPhoto
import com.forge.app.domain.units.WeightUnit
import com.forge.app.ui.theme.ForgeTheme
import java.io.File
import java.time.*
import java.util.*
import org.junit.Assert.*
import org.junit.Rule
import org.junit.Test
import org.junit.runner.RunWith
import org.robolectric.RobolectricTestRunner
import org.robolectric.annotation.Config
import org.robolectric.annotation.GraphicsMode

@RunWith(RobolectricTestRunner::class)
@GraphicsMode(GraphicsMode.Mode.NATIVE)
@Config(sdk=[34], qualifiers="w360dp-h800dp-mdpi")
class ProfileAuditRegressionTest {
 @get:Rule val compose=createComposeRule()
 @Test fun `photo date picker preserves unchanged late evening date`() {
  val oldZone=TimeZone.getDefault(); val oldLocale=Locale.getDefault()
  TimeZone.setDefault(TimeZone.getTimeZone("America/Toronto")); Locale.setDefault(Locale.US)
  try {
   val zone=ZoneId.of("America/Toronto")
   val date=LocalDate.of(2026,9,7)
   val stamp=date.atTime(23,0).atZone(zone).toInstant().toEpochMilli()
   var saved:Long?=null
   compose.setContent { ForgeTheme { GalleryViewerPager(
    photos=listOf(ProgressPhoto("missing.jpg",stamp)),startIndex=0,albumNames=emptyList(),knownTags=emptyList(),weightUnit=WeightUnit.LB,
    fileFor={File("/tmp/audit-missing-image.jpg")},onSaveNote={_,_->},onSaveTitle={_,_->},onMove={_,_->},onSetPose={_,_->},onSetMuscles={_,_->},onSetTags={_,_->},onSetWeight={_,_->},onSetDate={_,ms->saved=ms},onDelete={},onDismiss={}) } }
   compose.onNodeWithText("Monday, Sep 7, 2026").performClick()
   compose.onNodeWithText("Set").performClick()
   compose.runOnIdle {
    val actual=Instant.ofEpochMilli(saved!!).atZone(zone).toLocalDate()
    assertEquals(date,actual)
   }
  } finally { TimeZone.setDefault(oldZone);Locale.setDefault(oldLocale) }
 }
 @Test fun `profile name editor remains open after tap`() {
  var savedName: String? = null
  compose.setContent { ForgeTheme { val c=MaterialTheme.colorScheme; ProfileHeaderCard("Audit Athlete","",false,File("/tmp/audit-missing-avatar.jpg"),0,{savedName=it}, {},c.onBackground,c.onSurfaceVariant,c.primary) } }
  compose.onNodeWithText("Audit Athlete").performClick()
  val editors=compose.onAllNodes(hasSetTextAction()).fetchSemanticsNodes().size
  assertEquals(1, editors)
  compose.onNode(hasSetTextAction()).performTextReplacement("New Athlete")
  compose.onNode(hasSetTextAction()).performImeAction()
  compose.onAllNodes(hasSetTextAction()).assertCountEquals(0)
  compose.runOnIdle { assertEquals("New Athlete", savedName) }
 }
 @Test fun `large photo archives return bounded distinct pairs`() {
  val zone=ZoneOffset.UTC
  for(n in listOf(100,250,500,1000)) {
   val photos=(0 until n).map { ProgressPhoto("$it.jpg",LocalDate.of(2020,1,1).plusDays(it.toLong()).atStartOfDay(zone).toInstant().toEpochMilli(),pose="FRONT",weightLb=180.0) }
   sameWeightPairs(photos.take(50),zone)
   val pairs=sameWeightPairs(photos,zone)
   assertEquals(3,pairs.size)
  }
 }
}
