package com.forge.app.ui.gym.stats

import android.content.Context
import android.content.Intent
import android.graphics.Bitmap
import android.graphics.Canvas
import android.graphics.LinearGradient
import android.graphics.Paint
import android.graphics.Shader
import android.graphics.Typeface
import android.net.Uri
import androidx.core.content.FileProvider
import java.io.File
import java.io.FileOutputStream

/**
 * Renders a single stat metric (e.g. e1RM) into a 1080×1080 PNG and shares it.
 * Pure Android Canvas — no off-screen Compose, deterministic, offline.
 * Mirrors SessionCardRenderer / RankCardRenderer.
 */
object StatMetricCardRenderer {
    private const val S = 1080

    /**
     * @param metricLabel  e.g. "ESTIMATED 1RM"
     * @param liftName     e.g. "Barbell Squat"
     * @param heroValue    e.g. "142" (pre-formatted, unit-correct)
     * @param heroUnit     e.g. "KG"
     * @param rateLine     optional second line, e.g. "+1.4% / mo"  — null = omit
     * @param accentArgb   app accent color as ARGB Long (0xFF________)
     */
    fun render(
        context: Context,
        metricLabel: String,
        liftName: String,
        heroValue: String,
        heroUnit: String,
        rateLine: String?,
        accentArgb: Long
    ): Uri? = runCatching {
        val bmp = Bitmap.createBitmap(S, S, Bitmap.Config.ARGB_8888)
        try {
            val canvas = Canvas(bmp)
            val cx = S / 2f
            val accent = accentArgb.toInt()
            val white = 0xFFF2F2F4.toInt()
            val mutedC = 0xFFAEAEB6.toInt()
            // dark gradient background matching session/rank cards
            canvas.drawRect(0f, 0f, S.toFloat(), S.toFloat(), Paint().apply {
                shader = LinearGradient(
                    0f, 0f, 0f, S.toFloat(),
                    0xFF131318.toInt(), 0xFF090909.toInt(),
                    Shader.TileMode.CLAMP
                )
            })
            // metric label (small caps)
            canvas.drawText(metricLabel, cx, 220f, Paint().apply {
                isAntiAlias = true; color = mutedC; textAlign = Paint.Align.CENTER
                textSize = 44f; letterSpacing = 0.18f
            })
            // lift name (serif)
            canvas.drawText(liftName, cx, 310f, Paint().apply {
                isAntiAlias = true; color = white; textAlign = Paint.Align.CENTER
                textSize = 72f; typeface = Typeface.create(Typeface.SERIF, Typeface.NORMAL)
            })
            // hero value (big accent serif)
            canvas.drawText(heroValue, cx, 580f, Paint().apply {
                isAntiAlias = true; color = accent; textAlign = Paint.Align.CENTER
                textSize = 200f; typeface = Typeface.create(Typeface.SERIF, Typeface.BOLD)
            })
            // unit below hero
            canvas.drawText(heroUnit, cx, 660f, Paint().apply {
                isAntiAlias = true; color = mutedC; textAlign = Paint.Align.CENTER
                textSize = 48f; letterSpacing = 0.2f
            })
            // optional rate line
            if (!rateLine.isNullOrBlank()) {
                canvas.drawText(rateLine, cx, 770f, Paint().apply {
                    isAntiAlias = true; color = white; textAlign = Paint.Align.CENTER
                    textSize = 52f; typeface = Typeface.create(Typeface.SERIF, Typeface.ITALIC)
                })
            }
            // FORGE wordmark
            canvas.drawText("FORGE", cx, 1012f, Paint().apply {
                isAntiAlias = true; color = accent; textAlign = Paint.Align.CENTER
                textSize = 52f; letterSpacing = 0.28f; typeface = Typeface.create(Typeface.SERIF, Typeface.ITALIC)
            })
            context.filesDir.listFiles { f -> f.name.startsWith("forge_stat_card") }?.forEach { it.delete() }
            val file = File(context.filesDir, "forge_stat_card_${System.currentTimeMillis()}.png")
            FileOutputStream(file).use { bmp.compress(Bitmap.CompressFormat.PNG, 100, it) }
            FileProvider.getUriForFile(context, "${context.packageName}.fileprovider", file)
        } finally {
            bmp.recycle()
        }
    }.getOrNull()

    fun share(context: Context, uri: Uri) {
        val send = Intent(Intent.ACTION_SEND).apply {
            type = "image/png"
            putExtra(Intent.EXTRA_STREAM, uri)
            addFlags(Intent.FLAG_GRANT_READ_URI_PERMISSION)
        }
        context.startActivity(Intent.createChooser(send, "Share your stat"))
    }
}
