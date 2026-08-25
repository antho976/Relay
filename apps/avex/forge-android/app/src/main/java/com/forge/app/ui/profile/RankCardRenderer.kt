package com.forge.app.ui.profile

import android.content.Context
import android.content.Intent
import android.graphics.Bitmap
import android.graphics.Canvas
import android.graphics.LinearGradient
import android.graphics.Paint
import android.graphics.RadialGradient
import android.graphics.Shader
import android.graphics.Typeface
import android.net.Uri
import androidx.core.content.FileProvider
import java.io.File
import java.io.FileOutputStream
import java.util.Locale
import com.forge.app.core.io.exportFile

/**
 * Renders the profile's rank into a 1080×1080 PNG "rank card" and shares it. Pure Android Canvas (no
 * off-screen Compose capture) so it's deterministic and works without the view being on screen. The
 * first genuinely shareable artifact in the app — all local, no network.
 */
object RankCardRenderer {

    private const val S = 1080

    /** Renders the card to filesDir (covered by file_paths.xml) and returns a FileProvider uri, or null on failure. */
    fun render(
        context: Context,
        name: String,
        rankName: String,
        roman: String,
        xpTotal: Long,
        tierColorArgb: Long,
        standingLine: String?
    ): Uri? = runCatching {
        val bmp = Bitmap.createBitmap(S, S, Bitmap.Config.ARGB_8888)
        try {
        val canvas = Canvas(bmp)
        val cx = S / 2f
        val tier = tierColorArgb.toInt()
        val tierRgb = tier and 0x00FFFFFF

        // ── Background: the Pearl dark gradient (#131318 → #090909) ──
        canvas.drawRect(0f, 0f, S.toFloat(), S.toFloat(), Paint().apply {
            shader = LinearGradient(0f, 0f, 0f, S.toFloat(), 0xFF131318.toInt(), 0xFF090909.toInt(), Shader.TileMode.CLAMP)
        })

        // ── Name (top, serif) ──
        if (name.isNotBlank()) {
            canvas.drawText(name.trim(), cx, 170f, Paint().apply {
                isAntiAlias = true; color = 0xFFF2F2F4.toInt(); textAlign = Paint.Align.CENTER
                textSize = 56f; typeface = Typeface.create(Typeface.SERIF, Typeface.NORMAL)
            })
        }

        // ── Emblem: tier-colored glow + ring + roman numeral ──
        val cy = 440f
        canvas.drawCircle(cx, cy, 300f, Paint().apply {
            isAntiAlias = true
            shader = RadialGradient(cx, cy, 300f, tierRgb or 0x66000000, tierRgb, Shader.TileMode.CLAMP)
        })
        canvas.drawCircle(cx, cy, 156f, Paint().apply {
            isAntiAlias = true; style = Paint.Style.STROKE; strokeWidth = 7f; color = tier
        })
        canvas.drawText(roman, cx, cy + 50f, Paint().apply {
            isAntiAlias = true; color = tier; textAlign = Paint.Align.CENTER
            textSize = 140f; typeface = Typeface.create(Typeface.SERIF, Typeface.BOLD)
        })

        // ── Rank name (serif, large) ──
        canvas.drawText(rankName, cx, 760f, Paint().apply {
            isAntiAlias = true; color = 0xFFF2F2F4.toInt(); textAlign = Paint.Align.CENTER
            textSize = 88f; typeface = Typeface.create(Typeface.SERIF, Typeface.NORMAL)
        })

        // ── XP + standing (muted) ──
        val sub = Paint().apply {
            isAntiAlias = true; color = 0xFFAEAEB6.toInt(); textAlign = Paint.Align.CENTER; textSize = 42f
        }
        canvas.drawText("${"%,d".format(Locale.US, xpTotal)} XP", cx, 832f, sub)
        if (!standingLine.isNullOrBlank()) canvas.drawText(standingLine, cx, 892f, sub)

        // ── "• Avex" wordmark (tier-tinted, italic serif) — matches BeforeAfterCardRenderer so the
        //    two shareable cards read as one family ──
        canvas.drawText("• Avex", cx, 1010f, Paint().apply {
            isAntiAlias = true; color = tier; textAlign = Paint.Align.CENTER
            textSize = 52f; letterSpacing = 0.12f; typeface = Typeface.create(Typeface.SERIF, Typeface.ITALIC)
        })

        val file = exportFile(context, "avex_rank_card.png")
        FileOutputStream(file).use { bmp.compress(Bitmap.CompressFormat.PNG, 100, it) }
        FileProvider.getUriForFile(context, "${context.packageName}.fileprovider", file)
        } finally {
            // Recycle on every path — a throw during compress()/file I/O would otherwise leak ~4.7 MB.
            bmp.recycle()
        }
    }.getOrNull()

    fun share(context: Context, uri: Uri) {
        val send = Intent(Intent.ACTION_SEND).apply {
            type = "image/png"
            putExtra(Intent.EXTRA_STREAM, uri)
            addFlags(Intent.FLAG_GRANT_READ_URI_PERMISSION)
        }
        context.startActivity(Intent.createChooser(send, "Share your rank"))
    }
}
