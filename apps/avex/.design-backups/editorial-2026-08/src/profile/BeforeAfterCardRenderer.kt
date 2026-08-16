package com.forge.app.ui.profile

import android.content.Context
import android.content.Intent
import android.graphics.Bitmap
import android.graphics.BitmapFactory
import android.graphics.Canvas
import android.graphics.LinearGradient
import android.graphics.Matrix
import android.graphics.Paint
import android.graphics.Path
import android.graphics.Rect
import android.graphics.RectF
import android.graphics.Shader
import android.graphics.Typeface
import android.media.ExifInterface
import android.net.Uri
import androidx.core.content.FileProvider
import java.io.File
import java.io.FileOutputStream
import java.text.SimpleDateFormat
import java.util.Date
import java.util.Locale
import kotlin.math.roundToInt

/**
 * Renders a before/after progress pair into a 1080×1350 (4:5) PNG "share card" and shares it. Pure
 * Android Canvas — no off-screen Compose capture — so it's deterministic and works without the view
 * being on screen. Sibling of [RankCardRenderer]; the two share the Pearl-gradient / serif-hero /
 * tinted-wordmark language so the app's shareable artifacts read as one family. All local, no network.
 *
 * Weight is delta-only by design (GYMAP-55): a public card shows the change ("−12 lb"), never the
 * user's absolute bodyweight.
 */
object BeforeAfterCardRenderer {

    private const val W = 1080
    private const val H = 1350

    private const val COL_ON_BG = 0xFFF2F2F4.toInt()
    private const val COL_MUTED = 0xFFAEAEB6.toInt()
    private const val COL_TAG = 0xE6FFFFFF.toInt()      // white @0.9 — the BEFORE tag
    private const val COL_TAG_SUB = 0xB3FFFFFF.toInt()  // white @0.7 — the date under a tag

    /**
     * Renders the card to filesDir (covered by file_paths.xml) and returns a FileProvider uri, or
     * null on failure. [spanLabel] is the gallery span ("3 months", "" for same day); [weightDelta]
     * is the pre-signed delta-only line ("−12 lb") or null; [poseLine] is the uppercase pose read
     * ("FRONT", "FRONT → BACK") or null.
     */
    fun render(
        context: Context,
        beforeFile: File,
        afterFile: File,
        beforeMs: Long,
        afterMs: Long,
        spanLabel: String,
        weightDelta: String?,
        poseLine: String?,
        accentArgb: Int
    ): Uri? = runCatching {
        val card = Bitmap.createBitmap(W, H, Bitmap.Config.ARGB_8888)
        // Photos are decoded up front so they can be recycled in the finally even if drawing throws.
        val before = decodeOriented(beforeFile, reqPx = 1200)
        val after = decodeOriented(afterFile, reqPx = 1200)
        try {
            val canvas = Canvas(card)
            val cx = W / 2f

            // ── Background: the Pearl dark gradient (#131318 → #090909), matching RankCardRenderer ──
            canvas.drawRect(0f, 0f, W.toFloat(), H.toFloat(), Paint().apply {
                shader = LinearGradient(0f, 0f, 0f, H.toFloat(), 0xFF131318.toInt(), 0xFF090909.toInt(), Shader.TileMode.CLAMP)
            })

            // ── Photos: two center-cropped cells side by side, rounded 28 ──
            val margin = 48f
            val gap = 20f
            val cellW = (W - margin * 2 - gap) / 2f
            val top = 56f
            val cellH = 900f
            drawPhotoCell(canvas, before, margin, top, cellW, cellH, "BEFORE", dateOf(beforeMs), COL_TAG)
            drawPhotoCell(canvas, after, margin + cellW + gap, top, cellW, cellH, "AFTER", dateOf(afterMs), accentArgb)

            // ── Readout: span hero → delta → pose, stacked; wordmark pinned to the bottom ──
            val serif = Typeface.create(Typeface.SERIF, Typeface.NORMAL)
            val mono = Typeface.MONOSPACE
            var y = top + cellH + 84f
            canvas.drawText(spanLabel.ifEmpty { "Same day" }, cx, y, Paint().apply {
                isAntiAlias = true; color = COL_ON_BG; textAlign = Paint.Align.CENTER
                textSize = 72f; typeface = serif
            })
            if (weightDelta != null) {
                y += 58f
                canvas.drawText(weightDelta, cx, y, Paint().apply {
                    isAntiAlias = true; color = COL_MUTED; textAlign = Paint.Align.CENTER; textSize = 42f
                })
            }
            if (poseLine != null) {
                y += 52f
                canvas.drawText(poseLine, cx, y, Paint().apply {
                    isAntiAlias = true; color = accentArgb; textAlign = Paint.Align.CENTER
                    textSize = 28f; typeface = mono; letterSpacing = 0.18f
                })
            }

            // ── "• Avex" wordmark (accent-tinted italic serif) ──
            canvas.drawText("• Avex", cx, H - 58f, Paint().apply {
                isAntiAlias = true; color = accentArgb; textAlign = Paint.Align.CENTER
                textSize = 50f; letterSpacing = 0.12f; typeface = Typeface.create(Typeface.SERIF, Typeface.ITALIC)
            })

            val file = File(context.filesDir, "avex_before_after_card.png")
            FileOutputStream(file).use { card.compress(Bitmap.CompressFormat.PNG, 100, it) }
            FileProvider.getUriForFile(context, "${context.packageName}.fileprovider", file)
        } finally {
            // Recycle on every path — a throw during compress()/file I/O would otherwise leak the card
            // (~5.8 MB) plus both source bitmaps.
            card.recycle()
            before?.recycle()
            after?.recycle()
        }
    }.getOrNull()

    /** Draws one photo center-cropped into a rounded cell, with a top scrim + tag + date over it. */
    private fun drawPhotoCell(
        canvas: Canvas,
        photo: Bitmap?,
        left: Float,
        top: Float,
        cellW: Float,
        cellH: Float,
        tag: String,
        date: String,
        tagColor: Int
    ) {
        val rect = RectF(left, top, left + cellW, top + cellH)
        val clip = Path().apply { addRoundRect(rect, 28f, 28f, Path.Direction.CW) }
        canvas.save()
        canvas.clipPath(clip)
        if (photo != null) {
            canvas.drawBitmap(photo, centerCropSrc(photo.width, photo.height, cellW / cellH), rect, Paint().apply {
                isAntiAlias = true; isFilterBitmap = true
            })
        } else {
            canvas.drawRect(rect, Paint().apply { color = 0xFF1C1D24.toInt() }) // surfaceVariant fallback
        }
        // Soft top scrim so the tag + date stay legible over any photo.
        canvas.drawRect(RectF(left, top, left + cellW, top + 190f), Paint().apply {
            shader = LinearGradient(left, top, left, top + 190f, 0x99000000.toInt(), 0x00000000, Shader.TileMode.CLAMP)
        })
        canvas.restore()

        canvas.drawText(tag, left + 26f, top + 52f, Paint().apply {
            isAntiAlias = true; color = tagColor; textSize = 30f
            typeface = Typeface.MONOSPACE; letterSpacing = 0.16f
        })
        canvas.drawText(date, left + 26f, top + 90f, Paint().apply {
            isAntiAlias = true; color = COL_TAG_SUB; textSize = 25f; typeface = Typeface.MONOSPACE
        })
    }

    /** The centered sub-rect of a [srcW]×[srcH] bitmap that matches [dstAspect] (w/h) — a center crop. */
    private fun centerCropSrc(srcW: Int, srcH: Int, dstAspect: Float): Rect {
        val srcAspect = srcW.toFloat() / srcH
        return if (srcAspect > dstAspect) {
            val cropW = (srcH * dstAspect).roundToInt().coerceIn(1, srcW)
            val x = (srcW - cropW) / 2
            Rect(x, 0, x + cropW, srcH)
        } else {
            val cropH = (srcW / dstAspect).roundToInt().coerceIn(1, srcH)
            val y = (srcH - cropH) / 2
            Rect(0, y, srcW, y + cropH)
        }
    }

    private fun dateOf(ms: Long): String =
        SimpleDateFormat("MMM d, yyyy", Locale.getDefault()).format(Date(ms)).uppercase(Locale.getDefault())

    /**
     * Decodes a progress photo downsampled to ~[reqPx] on its longest edge and rotated per its EXIF
     * orientation (portrait phone shots otherwise composite sideways). Mirrors the gallery's
     * `decodeOriented` (ProgressPhotoImage.kt) — kept self-contained so the renderer has no UI-layer
     * coupling, exactly as RankCardRenderer stays self-contained.
     */
    private fun decodeOriented(file: File, reqPx: Int): Bitmap? {
        if (!file.exists()) return null
        val bounds = BitmapFactory.Options().apply { inJustDecodeBounds = true }
        BitmapFactory.decodeFile(file.path, bounds)
        var sample = 1
        val maxDim = maxOf(bounds.outWidth, bounds.outHeight)
        while (maxDim / (sample * 2) >= reqPx) sample *= 2
        val decoded = BitmapFactory.decodeFile(file.path, BitmapFactory.Options().apply {
            inSampleSize = sample
            inPreferredConfig = Bitmap.Config.ARGB_8888
        }) ?: return null

        val orientation = runCatching {
            ExifInterface(file.path).getAttributeInt(ExifInterface.TAG_ORIENTATION, ExifInterface.ORIENTATION_NORMAL)
        }.getOrDefault(ExifInterface.ORIENTATION_NORMAL)
        val degrees = when (orientation) {
            ExifInterface.ORIENTATION_ROTATE_90 -> 90f
            ExifInterface.ORIENTATION_ROTATE_180 -> 180f
            ExifInterface.ORIENTATION_ROTATE_270 -> 270f
            else -> return decoded
        }
        return runCatching {
            Bitmap.createBitmap(decoded, 0, 0, decoded.width, decoded.height, Matrix().apply { postRotate(degrees) }, true)
        }.getOrDefault(decoded).also { if (it !== decoded) decoded.recycle() }
    }

    fun share(context: Context, uri: Uri) {
        val send = Intent(Intent.ACTION_SEND).apply {
            type = "image/png"
            putExtra(Intent.EXTRA_STREAM, uri)
            addFlags(Intent.FLAG_GRANT_READ_URI_PERMISSION)
        }
        context.startActivity(Intent.createChooser(send, "Share progress"))
    }
}
