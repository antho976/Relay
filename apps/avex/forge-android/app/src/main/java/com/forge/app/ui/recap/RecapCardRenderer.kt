package com.forge.app.ui.recap

import android.content.Context
import android.content.Intent
import android.graphics.Bitmap
import android.graphics.Canvas
import android.graphics.Paint
import android.graphics.Typeface
import android.net.Uri
import androidx.core.content.FileProvider
import androidx.core.content.res.ResourcesCompat
import com.forge.app.R
import com.forge.app.core.io.exportFile
import java.io.FileOutputStream

/**
 * "Share October as an image": the recap's period, its four figures and what stood out, drawn into
 * a 1080×1350 PNG in the page's own fonts and colours and handed to the share sheet. Plain Android
 * Canvas, like the rank card, so it renders without the page on screen.
 */
internal object RecapCardRenderer {

    private const val W = 1080
    private const val H = 1350

    /** The page's colours as ARGB, passed in from the theme so a custom accent carries over. */
    data class Palette(val ground: Int, val ink: Int, val muted: Int, val accent: Int)

    fun render(
        context: Context,
        kicker: String,
        period: String,
        figures: List<RecapFigure>,
        highlights: List<RecapHighlight>,
        palette: Palette,
    ): Uri? = runCatching {
        val bmp = Bitmap.createBitmap(W, H, Bitmap.Config.ARGB_8888)
        try {
            val c = Canvas(bmp)
            c.drawColor(palette.ground)
            val serif = ResourcesCompat.getFont(context, R.font.noto_serif_display_regular) ?: Typeface.SERIF
            val sans = ResourcesCompat.getFont(context, R.font.noto_sans_bold) ?: Typeface.DEFAULT_BOLD
            val mono = ResourcesCompat.getFont(context, R.font.noto_sans_mono_regular) ?: Typeface.MONOSPACE
            fun paint(tf: Typeface, size: Float, color: Int, tracking: Float = 0f) = Paint().apply {
                isAntiAlias = true; typeface = tf; textSize = size; this.color = color; letterSpacing = tracking
            }
            val left = 96f
            c.drawText(kicker.uppercase(), left, 170f, paint(mono, 30f, palette.muted, 0.12f))
            c.drawText(period, left, 330f, paint(serif, 150f, palette.ink))
            c.drawRect(left, 380f, left + 120f, 386f, Paint().apply { color = palette.accent })

            val colW = (W - 2 * left) / 2f
            figures.take(4).forEachIndexed { i, f ->
                val x = left + (i % 2) * colW
                val y = 500f + (i / 2) * 230f
                c.drawText(f.label.uppercase(), x, y, paint(mono, 28f, palette.muted, 0.1f))
                val big = paint(serif, 110f, palette.ink)
                c.drawText(f.value, x, y + 120f, big)
                if (f.suffix.isNotBlank()) {
                    c.drawText(f.suffix.trim(), x + big.measureText(f.value) + 10f, y + 120f, paint(serif, 54f, palette.muted))
                }
            }
            highlights.take(4).forEachIndexed { i, h ->
                val x = left + (i % 2) * colW
                val y = 1000f + (i / 2) * 150f
                c.drawText(h.label.uppercase(), x, y, paint(mono, 24f, palette.muted, 0.1f))
                c.drawText(fit(h.value, paint(sans, 40f, palette.ink), colW - 30f), x, y + 56f, paint(sans, 40f, palette.ink))
            }
            c.drawText("Avex", W - left - paint(serif, 44f, palette.accent).measureText("Avex"), H - 80f, paint(serif, 44f, palette.accent))

            val file = exportFile(context, "avex_recap.png")
            FileOutputStream(file).use { bmp.compress(Bitmap.CompressFormat.PNG, 100, it) }
            FileProvider.getUriForFile(context, "${context.packageName}.fileprovider", file)
        } finally {
            bmp.recycle()
        }
    }.getOrNull()

    /** Shorten [text] with an ellipsis until it fits [max] pixels. */
    private fun fit(text: String, p: Paint, max: Float): String {
        if (p.measureText(text) <= max) return text
        var t = text
        while (t.length > 1 && p.measureText("$t…") > max) t = t.dropLast(1)
        return "$t…"
    }

    fun share(context: Context, uri: Uri, title: String) {
        val send = Intent(Intent.ACTION_SEND).apply {
            type = "image/png"
            putExtra(Intent.EXTRA_STREAM, uri)
            addFlags(Intent.FLAG_GRANT_READ_URI_PERMISSION)
        }
        context.startActivity(Intent.createChooser(send, title))
    }
}
