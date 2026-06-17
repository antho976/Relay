package com.forge.app.ui.gym.train.components

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
 * Renders a finished workout into a 1080×1080 PNG "session card" and shares it — the post-workout
 * counterpart to [com.forge.app.ui.profile.RankCardRenderer]. Pure Android Canvas (no off-screen
 * Compose capture), so it's deterministic and works without any view on screen. All local, no network.
 * The caller pre-formats every string (unit-correct), keeping this renderer free of settings/Compose.
 */
object SessionCardRenderer {

    private const val S = 1080

    /**
     * @param title day name, e.g. "Upper A"  ·  @param dateLine e.g. "JUN 16, 2026"
     * @param heroValue the headline stat value (e.g. "12,400 lb")  ·  @param heroLabel e.g. "VOLUME"
     * @param stats up to 3 secondary (value,label) pairs — PRs / sets / minutes
     * @param footnote optional celebratory line (best session / PRs)  ·  @param accentArgb app accent
     */
    fun render(
        context: Context,
        title: String,
        dateLine: String,
        heroValue: String,
        heroLabel: String,
        stats: List<Pair<String, String>>,
        footnote: String?,
        accentArgb: Long
    ): Uri? = runCatching {
        val bmp = Bitmap.createBitmap(S, S, Bitmap.Config.ARGB_8888)
        try {
            val canvas = Canvas(bmp)
            val cx = S / 2f
            val accent = accentArgb.toInt()
            val white = 0xFFF2F2F4.toInt()
            val mutedC = 0xFFAEAEB6.toInt()

            // ── Background: the Pearl dark gradient (matches the rank card) ──
            canvas.drawRect(0f, 0f, S.toFloat(), S.toFloat(), Paint().apply {
                shader = LinearGradient(0f, 0f, 0f, S.toFloat(), 0xFF131318.toInt(), 0xFF090909.toInt(), Shader.TileMode.CLAMP)
            })

            // ── Title + date ──
            canvas.drawText(title, cx, 190f, Paint().apply {
                isAntiAlias = true; color = white; textAlign = Paint.Align.CENTER
                textSize = 84f; typeface = Typeface.create(Typeface.SERIF, Typeface.NORMAL)
            })
            canvas.drawText(dateLine, cx, 256f, Paint().apply {
                isAntiAlias = true; color = mutedC; textAlign = Paint.Align.CENTER
                textSize = 40f; letterSpacing = 0.12f
            })

            // ── Hero stat (volume) ──
            canvas.drawText(heroValue, cx, 510f, Paint().apply {
                isAntiAlias = true; color = accent; textAlign = Paint.Align.CENTER
                textSize = 124f; typeface = Typeface.create(Typeface.SERIF, Typeface.BOLD)
            })
            canvas.drawText(heroLabel, cx, 575f, Paint().apply {
                isAntiAlias = true; color = mutedC; textAlign = Paint.Align.CENTER
                textSize = 40f; letterSpacing = 0.2f
            })

            // ── Secondary stats row (evenly spaced) ──
            if (stats.isNotEmpty()) {
                val n = stats.size
                stats.forEachIndexed { i, (value, label) ->
                    val x = S * (i + 0.5f) / n
                    canvas.drawText(value, x, 760f, Paint().apply {
                        isAntiAlias = true; color = white; textAlign = Paint.Align.CENTER
                        textSize = 78f; typeface = Typeface.create(Typeface.SERIF, Typeface.NORMAL)
                    })
                    canvas.drawText(label, x, 812f, Paint().apply {
                        isAntiAlias = true; color = mutedC; textAlign = Paint.Align.CENTER
                        textSize = 32f; letterSpacing = 0.15f
                    })
                }
            }

            // ── Footnote (celebratory) ──
            if (!footnote.isNullOrBlank()) {
                canvas.drawText(footnote, cx, 905f, Paint().apply {
                    isAntiAlias = true; color = white; textAlign = Paint.Align.CENTER
                    textSize = 48f; typeface = Typeface.create(Typeface.SERIF, Typeface.ITALIC)
                })
            }

            // ── FORGE wordmark (accent-tinted, italic serif) ──
            canvas.drawText("FORGE", cx, 1012f, Paint().apply {
                isAntiAlias = true; color = accent; textAlign = Paint.Align.CENTER
                textSize = 52f; letterSpacing = 0.28f; typeface = Typeface.create(Typeface.SERIF, Typeface.ITALIC)
            })

            // Unique name per render so a rapid re-share can't overwrite a file a previous share's URI
            // still points at; sweep older cards first so filesDir doesn't accumulate them.
            context.filesDir.listFiles { f -> f.name.startsWith("forge_session_card") }?.forEach { it.delete() }
            val file = File(context.filesDir, "forge_session_card_${System.currentTimeMillis()}.png")
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
        context.startActivity(Intent.createChooser(send, "Share your session"))
    }
}
