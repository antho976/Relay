package com.forge.app.core.io

import org.junit.Assert.assertEquals
import org.junit.Assert.assertFalse
import org.junit.Assert.assertTrue
import org.junit.Test

/**
 * M-17: every EXIF orientation, 1 through 8, brings the pixels upright.
 *
 * The three renderers each carried their own copy that handled 3, 6 and 8 only, so the four
 * mirrored tags (2, 4, 5, 7) came out mirrored or sideways. The table and the matrix it builds are
 * pure, so the mapping is checked here rather than on a device: each case applies the transform to
 * the corners of a unit square and states where they land.
 */
class OrientedBitmapsTest {

    /** Apply the row-major 3x3 [ImageOrientation.matrixValues] to a point, as Matrix would. */
    private fun map(o: ImageOrientation, x: Float, y: Float): Pair<Float, Float> {
        val m = o.matrixValues()
        return Pair(m[0] * x + m[1] * y + m[2], m[3] * x + m[4] * y + m[5])
    }

    private fun assertMaps(tag: Int, from: Pair<Float, Float>, to: Pair<Float, Float>) {
        val o = ImageOrientation.fromExifTag(tag)
        val (gotX, gotY) = map(o, from.first, from.second)
        assertEquals("tag $tag x", to.first, gotX, 1e-5f)
        assertEquals("tag $tag y", to.second, gotY, 1e-5f)
    }

    @Test
    fun normalAndUnknownTagsAreUpright() {
        assertTrue(ImageOrientation.fromExifTag(1).isUpright)
        assertTrue(ImageOrientation.fromExifTag(0).isUpright)
        assertTrue(ImageOrientation.fromExifTag(99).isUpright)
        assertEquals(ImageOrientation.UPRIGHT, ImageOrientation.fromExifTag(1))
    }

    @Test
    fun rotationOnlyTagsMatchTheirAngles() {
        assertEquals(ImageOrientation(90, false), ImageOrientation.fromExifTag(6))
        assertEquals(ImageOrientation(180, false), ImageOrientation.fromExifTag(3))
        assertEquals(ImageOrientation(270, false), ImageOrientation.fromExifTag(8))
        // Screen y grows downward, so +90 is clockwise: the right edge goes to the bottom.
        assertMaps(6, 1f to 0f, 0f to 1f)
        assertMaps(3, 1f to 0f, -1f to 0f)
        assertMaps(8, 1f to 0f, 0f to -1f)
    }

    @Test
    fun theFourMirroredTagsAreFlaggedAsFlipped() {
        listOf(2, 4, 5, 7).forEach {
            assertTrue("tag $it must be flipped", ImageOrientation.fromExifTag(it).flipped)
            assertFalse("tag $it is not upright", ImageOrientation.fromExifTag(it).isUpright)
        }
        listOf(1, 3, 6, 8).forEach {
            assertFalse("tag $it must not be flipped", ImageOrientation.fromExifTag(it).flipped)
        }
    }

    @Test
    fun horizontalAndVerticalFlipsMirrorAboutTheRightAxis() {
        // 2: mirror about the vertical axis — x negates, y is untouched.
        assertMaps(2, 1f to 2f, -1f to 2f)
        // 4: mirror about the horizontal axis — y negates, x is untouched.
        assertMaps(4, 1f to 2f, 1f to -2f)
    }

    @Test
    fun theTwoTransposedTagsAreExactTransposes() {
        // 5 (transpose): the 0th row is the visual left side, so (x, y) lands at (y, x).
        assertMaps(5, 1f to 2f, 2f to 1f)
        // 7 (transverse): the anti-transpose, (x, y) -> (-y, -x).
        assertMaps(7, 1f to 2f, -2f to -1f)
        assertEquals(ImageOrientation(270, true), ImageOrientation.fromExifTag(5))
        assertEquals(ImageOrientation(90, true), ImageOrientation.fromExifTag(7))
    }

    @Test
    fun everyTransformPreservesAreaSoNoOrientationScalesTheImage() {
        (1..8).forEach { tag ->
            val m = ImageOrientation.fromExifTag(tag).matrixValues()
            val determinant = m[0] * m[4] - m[1] * m[3]
            assertEquals("tag $tag must be area-preserving", 1f, kotlin.math.abs(determinant), 1e-5f)
        }
    }
}
