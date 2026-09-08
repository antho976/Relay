package com.forge.app.ui.profile

import com.forge.app.data.repo.ProgressPhoto
import java.time.ZoneOffset
import kotlin.math.abs
import kotlin.random.Random
import org.junit.Assert.assertEquals
import org.junit.Test

class GalleryPairSelectionTest {
    @Test fun `bounded selection preserves full-sort ranking and exclusion on varied archives`() {
        val random = Random(7)
        repeat(25) {
            val photos = (0 until 70).map { i -> ProgressPhoto("$i.jpg",
                takenAtMs = random.nextLong(1000) * 86_400_000L,
                pose = if (i % 2 == 0) "FRONT" else "BACK", weightLb = 175.0 + random.nextInt(10)) }
            val excluded = setOf("0.jpg", "2.jpg")
            val all = mutableListOf<SameWeightPair>()
            for (i in photos.indices) for (j in i + 1 until photos.size) {
                val a = photos[i]; val b = photos[j]
                val days = abs(a.takenAtMs / 86_400_000L - b.takenAtMs / 86_400_000L)
                if (a.pose != b.pose || abs(a.weightLb!! - b.weightLb!!) > 2.0 || days < 30) continue
                val (before, after) = if (a.takenAtMs <= b.takenAtMs) a to b else b to a
                all += SameWeightPair(before, after, (a.weightLb!! + b.weightLb!!) / 2, days)
            }
            val used = mutableSetOf<String>()
            val expected = all.sortedWith(compareByDescending<SameWeightPair> { it.daysApart }
                .thenBy { abs(it.after.weightLb!! - it.before.weightLb!!) }).filter { pair ->
                if (setOf(pair.before.fileName, pair.after.fileName) == excluded ||
                    pair.before.fileName in used || pair.after.fileName in used) false
                else { used += pair.before.fileName; used += pair.after.fileName; true }
            }.take(3)
            assertEquals(expected, sameWeightPairs(photos, ZoneOffset.UTC, excluded))
        }
    }
}
