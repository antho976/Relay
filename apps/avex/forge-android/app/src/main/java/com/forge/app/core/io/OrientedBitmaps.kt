package com.forge.app.core.io

import android.graphics.Bitmap
import android.graphics.BitmapFactory
import android.graphics.Matrix
import androidx.exifinterface.media.ExifInterface
import java.io.File

/**
 * The transform that brings decoded pixels upright, as an EXIF orientation tag describes it: an
 * optional mirror about the vertical axis applied FIRST, then a clockwise rotation.
 *
 * That order is the one AndroidX ExifInterface's [ExifInterface.getRotationDegrees] and
 * [ExifInterface.isFlipped] pair describe, and it is what makes the two transposed orientations
 * (5 and 7) come out right: orientation 5 is "mirror, then rotate 270", which is a plain transpose
 * of the pixel grid. Rotation-only handling had turned every mirrored shot (2, 4, 5, 7) into either
 * a mirror image or a sideways one.
 *
 * Pure so the tag-to-matrix mapping can be unit-tested without a bitmap: [matrixValues] is the
 * row-major 3x3 [Matrix] holds, and [fromExifTag] is the same table AndroidX's getters implement.
 */
internal data class ImageOrientation(val rotationDegrees: Int, val flipped: Boolean) {

    val isUpright: Boolean get() = rotationDegrees.mod(360) == 0 && !flipped

    /**
     * Row-major 3x3 affine values in [Matrix.setValues] layout: R(rotation) * S(mirror), so the
     * mirror is applied to the source first. Screen y grows downward, so a positive rotation is
     * clockwise, exactly as [Matrix.setRotate] treats it.
     */
    fun matrixValues(): FloatArray {
        val degrees = rotationDegrees.mod(360)
        val c = when (degrees) { 0 -> 1f; 180 -> -1f; else -> 0f }
        val s = when (degrees) { 90 -> 1f; 270 -> -1f; else -> 0f }
        val f = if (flipped) -1f else 1f
        return floatArrayOf(
            c * f, -s, 0f,
            s * f, c, 0f,
            0f, 0f, 1f
        )
    }

    fun toMatrix(): Matrix = Matrix().apply { setValues(matrixValues()) }

    companion object {
        val UPRIGHT = ImageOrientation(0, false)

        /**
         * EXIF orientation tag values 1 to 8, mapped the way AndroidX's `rotationDegrees` /
         * `isFlipped` report them. Anything else (undefined, garbage) reads as upright.
         */
        fun fromExifTag(tag: Int): ImageOrientation = when (tag) {
            ExifInterface.ORIENTATION_FLIP_HORIZONTAL -> ImageOrientation(0, true)
            ExifInterface.ORIENTATION_ROTATE_180 -> ImageOrientation(180, false)
            ExifInterface.ORIENTATION_FLIP_VERTICAL -> ImageOrientation(180, true)
            ExifInterface.ORIENTATION_TRANSPOSE -> ImageOrientation(270, true)
            ExifInterface.ORIENTATION_ROTATE_90 -> ImageOrientation(90, false)
            ExifInterface.ORIENTATION_TRANSVERSE -> ImageOrientation(90, true)
            ExifInterface.ORIENTATION_ROTATE_270 -> ImageOrientation(270, false)
            else -> UPRIGHT
        }

        /**
         * The orientation [file] declares. AndroidX ExifInterface rather than the platform class:
         * the framework parser only learned PNG and WebP containers in API 30, so an
         * orientation-tagged PNG imported on API 26 to 29 read as upright there and displayed
         * sideways. An unreadable or absent tag is upright.
         */
        fun of(file: File): ImageOrientation = runCatching {
            val exif = ExifInterface(file)
            ImageOrientation(exif.rotationDegrees, exif.isFlipped)
        }.getOrDefault(UPRIGHT)
    }
}

/**
 * The one decoder every progress-photo renderer goes through: the gallery grid, the full-screen
 * viewer and camera ghost, and the exported share card. Three private copies of the same
 * downsample-then-rotate routine had each handled orientations 3, 6 and 8 only, through the
 * platform ExifInterface; a fix in one did not reach the others.
 */
internal object OrientedBitmaps {

    /**
     * Decode [file] downsampled to about [reqPx] on its longest edge, then bring it upright per its
     * EXIF orientation. `inSampleSize` only halves, so the result can be up to ~2x [reqPx]; with
     * [exactFit] it is scaled precisely to [reqPx] first (bilinear), which caps the memory a grid
     * of thumbnails holds. Null for a missing or undecodable file.
     */
    fun decode(file: File, reqPx: Int, exactFit: Boolean = false): Bitmap? {
        if (!file.exists()) return null
        val bounds = BitmapFactory.Options().apply { inJustDecodeBounds = true }
        BitmapFactory.decodeFile(file.path, bounds)
        var sample = 1
        val maxDim = maxOf(bounds.outWidth, bounds.outHeight)
        while (maxDim / (sample * 2) >= reqPx) sample *= 2
        val opts = BitmapFactory.Options().apply {
            inSampleSize = sample
            inPreferredConfig = Bitmap.Config.ARGB_8888
        }
        val decoded = BitmapFactory.decodeFile(file.path, opts) ?: return null
        val sized = if (exactFit) scaleToFit(decoded, reqPx) else decoded
        return orient(sized, ImageOrientation.of(file))
    }

    /** [bitmap] brought upright per [orientation]; the input is recycled when a new bitmap replaces it. */
    fun orient(bitmap: Bitmap, orientation: ImageOrientation): Bitmap {
        if (orientation.isUpright) return bitmap
        // Guarded: the rotated copy is a second full allocation, and an OOM here must fall back to
        // the valid (if sideways) bitmap rather than crash the caller's decode coroutine.
        return runCatching {
            Bitmap.createBitmap(bitmap, 0, 0, bitmap.width, bitmap.height, orientation.toMatrix(), true)
        }.getOrDefault(bitmap).also { if (it !== bitmap) bitmap.recycle() }
    }

    private fun scaleToFit(decoded: Bitmap, reqPx: Int): Bitmap {
        val decodedMax = maxOf(decoded.width, decoded.height)
        if (decodedMax <= reqPx || decodedMax <= 0) return decoded
        val ratio = reqPx.toFloat() / decodedMax
        // Guarded like the rotation step: a very large source can OOM here, and an uncaught throw
        // would crash produceState's coroutine. Fall back to the valid `decoded`.
        return runCatching {
            Bitmap.createScaledBitmap(
                decoded,
                (decoded.width * ratio).toInt().coerceAtLeast(1),
                (decoded.height * ratio).toInt().coerceAtLeast(1),
                true
            )
        }.getOrDefault(decoded).also { if (it !== decoded) decoded.recycle() }
    }
}
