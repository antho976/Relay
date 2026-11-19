package com.forge.app.ui.theme

import androidx.compose.ui.graphics.Color
import androidx.compose.ui.graphics.SolidColor
import androidx.compose.ui.graphics.vector.ImageVector
import androidx.compose.ui.graphics.vector.addPathNodes
import androidx.compose.ui.unit.dp

/**
 * One glyph per kind of training day: a small body with the part the day trains lit.
 *
 * Drawn 2026-11-18 (Relay design 8, page "Icons + days step") to replace the shirt and the shorts,
 * which read as clothing. The figure is one smooth silhouette in six states. The lit part is solid
 * and the rest of the body is the same shape at [REST_ALPHA], so a row's glyph answers "what did
 * this session train" before its name is read. Push and Pull light the same muscles a small glyph
 * can show, so the arms tell them apart: straight out for Push, bent up for Pull.
 *
 * Unlike [OpenIcons] these are filled, not stroked: a region has to read as lit at 30dp, and an
 * outline of a torso does not. Paint is black; `Icon(tint = ...)` recolours both rungs and keeps
 * the alpha between them.
 */
object DayIcons {

    /** How much of the ink the untrained part of the body keeps. */
    private const val REST_ALPHA = 0.24f

    private fun body(name: String, lit: String, rest: String): ImageVector {
        val b = ImageVector.Builder(
            name = "Day.$name",
            defaultWidth = 24.dp,
            defaultHeight = 24.dp,
            viewportWidth = 24f,
            viewportHeight = 24f,
        )
        if (rest.isNotEmpty()) {
            b.addPath(pathData = addPathNodes(rest), fill = SolidColor(Color.Black), fillAlpha = REST_ALPHA)
        }
        if (lit.isNotEmpty()) b.addPath(pathData = addPathNodes(lit), fill = SolidColor(Color.Black))
        return b.build()
    }

    /** Upper body: head, torso and arms lit. */
    val Upper: ImageVector by lazy {
        body(
            "Upper",
            lit = "M10.05 3.05 a1.95 1.95 0 1 0 3.9 0 a1.95 1.95 0 1 0 -3.9 0 Z M10.6 5.7 C9.8 6.2 8.8 6.4 7.8 6.6 C7 6.8 6.8 7.5 7.1 8.2 C7.9 9.7 8.6 10.9 8.9 12.2 C9 12.7 9 13.2 9 13.6 C10.9 14.2 13.1 14.2 15 13.6 C15 13.2 15 12.7 15.1 12.2 C15.4 10.9 16.1 9.7 16.9 8.2 C17.2 7.5 17 6.8 16.2 6.6 C15.2 6.4 14.2 6.2 13.4 5.7 C12.5 6.15 11.5 6.15 10.6 5.7 Z M5.8 7.1 C6.5 7.2 6.9 7.9 6.7 8.6 L5.5 13.9 C5.4 14.5 4.8 14.8 4.25 14.65 C3.75 14.5 3.5 14 3.6 13.5 L4.6 8 C4.7 7.4 5.2 7 5.8 7.1 Z M18.2 7.1 C17.5 7.2 17.1 7.9 17.3 8.6 L18.5 13.9 C18.6 14.5 19.2 14.8 19.75 14.65 C20.25 14.5 20.5 14 20.4 13.5 L19.4 8 C19.3 7.4 18.8 7 18.2 7.1 Z",
            rest = "M8.95 14.6 C9.85 14.95 10.8 15.1 11.75 15.1 L11.15 21.9 C11.1 22.45 10.65 22.8 10.15 22.8 C9.65 22.8 9.2 22.45 9.15 21.9 Z M15.05 14.6 C14.15 14.95 13.2 15.1 12.25 15.1 L12.85 21.9 C12.9 22.45 13.35 22.8 13.85 22.8 C14.35 22.8 14.8 22.45 14.85 21.9 Z"
        )
    }

    /** Lower body and leg days: the legs lit. */
    val Lower: ImageVector by lazy {
        body(
            "Lower",
            lit = "M8.95 14.6 C9.85 14.95 10.8 15.1 11.75 15.1 L11.15 21.9 C11.1 22.45 10.65 22.8 10.15 22.8 C9.65 22.8 9.2 22.45 9.15 21.9 Z M15.05 14.6 C14.15 14.95 13.2 15.1 12.25 15.1 L12.85 21.9 C12.9 22.45 13.35 22.8 13.85 22.8 C14.35 22.8 14.8 22.45 14.85 21.9 Z",
            rest = "M10.05 3.05 a1.95 1.95 0 1 0 3.9 0 a1.95 1.95 0 1 0 -3.9 0 Z M10.6 5.7 C9.8 6.2 8.8 6.4 7.8 6.6 C7 6.8 6.8 7.5 7.1 8.2 C7.9 9.7 8.6 10.9 8.9 12.2 C9 12.7 9 13.2 9 13.6 C10.9 14.2 13.1 14.2 15 13.6 C15 13.2 15 12.7 15.1 12.2 C15.4 10.9 16.1 9.7 16.9 8.2 C17.2 7.5 17 6.8 16.2 6.6 C15.2 6.4 14.2 6.2 13.4 5.7 C12.5 6.15 11.5 6.15 10.6 5.7 Z M5.8 7.1 C6.5 7.2 6.9 7.9 6.7 8.6 L5.5 13.9 C5.4 14.5 4.8 14.8 4.25 14.65 C3.75 14.5 3.5 14 3.6 13.5 L4.6 8 C4.7 7.4 5.2 7 5.8 7.1 Z M18.2 7.1 C17.5 7.2 17.1 7.9 17.3 8.6 L18.5 13.9 C18.6 14.5 19.2 14.8 19.75 14.65 C20.25 14.5 20.5 14 20.4 13.5 L19.4 8 C19.3 7.4 18.8 7 18.2 7.1 Z"
        )
    }

    /** Push: the upper body lit, arms straight out. */
    val Push: ImageVector by lazy {
        body(
            "Push",
            lit = "M10.05 3.05 a1.95 1.95 0 1 0 3.9 0 a1.95 1.95 0 1 0 -3.9 0 Z M10.6 5.7 C9.8 6.2 8.8 6.4 7.8 6.6 C7 6.8 6.8 7.5 7.1 8.2 C7.9 9.7 8.6 10.9 8.9 12.2 C9 12.7 9 13.2 9 13.6 C10.9 14.2 13.1 14.2 15 13.6 C15 13.2 15 12.7 15.1 12.2 C15.4 10.9 16.1 9.7 16.9 8.2 C17.2 7.5 17 6.8 16.2 6.6 C15.2 6.4 14.2 6.2 13.4 5.7 C12.5 6.15 11.5 6.15 10.6 5.7 Z M6.4 6.9 C6.85 7.6 6.85 8.5 6.4 9.2 L2.7 8.9 C2.15 8.85 1.75 8.5 1.75 8.05 C1.75 7.6 2.15 7.25 2.7 7.2 Z M17.6 6.9 C17.15 7.6 17.15 8.5 17.6 9.2 L21.3 8.9 C21.85 8.85 22.25 8.5 22.25 8.05 C22.25 7.6 21.85 7.25 21.3 7.2 Z",
            rest = "M8.95 14.6 C9.85 14.95 10.8 15.1 11.75 15.1 L11.15 21.9 C11.1 22.45 10.65 22.8 10.15 22.8 C9.65 22.8 9.2 22.45 9.15 21.9 Z M15.05 14.6 C14.15 14.95 13.2 15.1 12.25 15.1 L12.85 21.9 C12.9 22.45 13.35 22.8 13.85 22.8 C14.35 22.8 14.8 22.45 14.85 21.9 Z"
        )
    }

    /** Pull: the upper body lit, arms bent up. */
    val Pull: ImageVector by lazy {
        body(
            "Pull",
            lit = "M10.05 3.05 a1.95 1.95 0 1 0 3.9 0 a1.95 1.95 0 1 0 -3.9 0 Z M10.6 5.7 C9.8 6.2 8.8 6.4 7.8 6.6 C7 6.8 6.8 7.5 7.1 8.2 C7.9 9.7 8.6 10.9 8.9 12.2 C9 12.7 9 13.2 9 13.6 C10.9 14.2 13.1 14.2 15 13.6 C15 13.2 15 12.7 15.1 12.2 C15.4 10.9 16.1 9.7 16.9 8.2 C17.2 7.5 17 6.8 16.2 6.6 C15.2 6.4 14.2 6.2 13.4 5.7 C12.5 6.15 11.5 6.15 10.6 5.7 Z M6.4 7 C6.85 7.6 6.9 8.5 6.5 9.2 L5 10.9 C4.3 11.6 3.1 11.2 3.1 10.2 L3.3 5.6 C3.32 5.1 3.75 4.7 4.25 4.7 C4.75 4.7 5.15 5.1 5.15 5.6 L5.1 8.2 Z M17.6 7 C17.15 7.6 17.1 8.5 17.5 9.2 L19 10.9 C19.7 11.6 20.9 11.2 20.9 10.2 L20.7 5.6 C20.68 5.1 20.25 4.7 19.75 4.7 C19.25 4.7 18.85 5.1 18.85 5.6 L18.9 8.2 Z",
            rest = "M8.95 14.6 C9.85 14.95 10.8 15.1 11.75 15.1 L11.15 21.9 C11.1 22.45 10.65 22.8 10.15 22.8 C9.65 22.8 9.2 22.45 9.15 21.9 Z M15.05 14.6 C14.15 14.95 13.2 15.1 12.25 15.1 L12.85 21.9 C12.9 22.45 13.35 22.8 13.85 22.8 C14.35 22.8 14.8 22.45 14.85 21.9 Z"
        )
    }

    /** Full body: everything lit. */
    val FullBody: ImageVector by lazy {
        body(
            "FullBody",
            lit = "M10.05 3.05 a1.95 1.95 0 1 0 3.9 0 a1.95 1.95 0 1 0 -3.9 0 Z M10.6 5.7 C9.8 6.2 8.8 6.4 7.8 6.6 C7 6.8 6.8 7.5 7.1 8.2 C7.9 9.7 8.6 10.9 8.9 12.2 C9 12.7 9 13.2 9 13.6 C10.9 14.2 13.1 14.2 15 13.6 C15 13.2 15 12.7 15.1 12.2 C15.4 10.9 16.1 9.7 16.9 8.2 C17.2 7.5 17 6.8 16.2 6.6 C15.2 6.4 14.2 6.2 13.4 5.7 C12.5 6.15 11.5 6.15 10.6 5.7 Z M5.8 7.1 C6.5 7.2 6.9 7.9 6.7 8.6 L5.5 13.9 C5.4 14.5 4.8 14.8 4.25 14.65 C3.75 14.5 3.5 14 3.6 13.5 L4.6 8 C4.7 7.4 5.2 7 5.8 7.1 Z M18.2 7.1 C17.5 7.2 17.1 7.9 17.3 8.6 L18.5 13.9 C18.6 14.5 19.2 14.8 19.75 14.65 C20.25 14.5 20.5 14 20.4 13.5 L19.4 8 C19.3 7.4 18.8 7 18.2 7.1 Z M8.95 14.6 C9.85 14.95 10.8 15.1 11.75 15.1 L11.15 21.9 C11.1 22.45 10.65 22.8 10.15 22.8 C9.65 22.8 9.2 22.45 9.15 21.9 Z M15.05 14.6 C14.15 14.95 13.2 15.1 12.25 15.1 L12.85 21.9 C12.9 22.45 13.35 22.8 13.85 22.8 C14.35 22.8 14.8 22.45 14.85 21.9 Z",
            rest = ""
        )
    }

    /** Core: the torso alone lit. */
    val Core: ImageVector by lazy {
        body(
            "Core",
            lit = "M10.6 5.7 C9.8 6.2 8.8 6.4 7.8 6.6 C7 6.8 6.8 7.5 7.1 8.2 C7.9 9.7 8.6 10.9 8.9 12.2 C9 12.7 9 13.2 9 13.6 C10.9 14.2 13.1 14.2 15 13.6 C15 13.2 15 12.7 15.1 12.2 C15.4 10.9 16.1 9.7 16.9 8.2 C17.2 7.5 17 6.8 16.2 6.6 C15.2 6.4 14.2 6.2 13.4 5.7 C12.5 6.15 11.5 6.15 10.6 5.7 Z",
            rest = "M10.05 3.05 a1.95 1.95 0 1 0 3.9 0 a1.95 1.95 0 1 0 -3.9 0 Z M5.8 7.1 C6.5 7.2 6.9 7.9 6.7 8.6 L5.5 13.9 C5.4 14.5 4.8 14.8 4.25 14.65 C3.75 14.5 3.5 14 3.6 13.5 L4.6 8 C4.7 7.4 5.2 7 5.8 7.1 Z M18.2 7.1 C17.5 7.2 17.1 7.9 17.3 8.6 L18.5 13.9 C18.6 14.5 19.2 14.8 19.75 14.65 C20.25 14.5 20.5 14 20.4 13.5 L19.4 8 C19.3 7.4 18.8 7 18.2 7.1 Z M8.95 14.6 C9.85 14.95 10.8 15.1 11.75 15.1 L11.15 21.9 C11.1 22.45 10.65 22.8 10.15 22.8 C9.65 22.8 9.2 22.45 9.15 21.9 Z M15.05 14.6 C14.15 14.95 13.2 15.1 12.25 15.1 L12.85 21.9 C12.9 22.45 13.35 22.8 13.85 22.8 C14.35 22.8 14.8 22.45 14.85 21.9 Z"
        )
    }

    /**
     * The glyph for a training day, read from its name: Push, Pull, Upper, Lower and Legs, Full body,
     * Core. Null when the name says none of them, so the caller keeps its own fallback.
     */
    fun forDayName(title: String): ImageVector? {
        val t = title.lowercase()
        return when {
            "push" in t -> Push
            "pull" in t -> Pull
            "lower" in t || "leg" in t || "glute" in t -> Lower
            "upper" in t || "chest" in t || "back" in t || "shoulder" in t || "arm" in t -> Upper
            "full" in t || "total" in t -> FullBody
            "core" in t || "abs" in t -> Core
            else -> null
        }
    }
}
