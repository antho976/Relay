package com.quietsoftware.relay.ui.code

import android.graphics.Bitmap
import androidx.compose.ui.geometry.Size
import androidx.compose.ui.graphics.Canvas
import androidx.compose.ui.graphics.ImageBitmap
import androidx.compose.ui.graphics.asAndroidBitmap
import androidx.compose.ui.graphics.drawscope.CanvasDrawScope
import androidx.compose.ui.graphics.drawscope.translate
import androidx.compose.ui.text.font.createFontFamilyResolver
import androidx.compose.ui.unit.Density
import androidx.compose.ui.unit.LayoutDirection
import androidx.test.core.app.ApplicationProvider
import com.quietsoftware.relay.core.design.DesignGot
import com.quietsoftware.relay.core.design.DesignNode
import com.quietsoftware.relay.core.design.DesignRow
import com.quietsoftware.relay.core.design.DesignStyle
import kotlin.math.abs

/** Pages planned and drawn as the phone does, for tests that look at what lands where. */
internal object DesignPlans {
    private val resolver by lazy { createFontFamilyResolver(ApplicationProvider.getApplicationContext()) }

    fun measure(): ComposeMeasure = ComposeMeasure(resolver)

    /** A one-page design holding [nodes] (each with parent `p1` unless it names another), planned. */
    fun page(vararg nodes: DesignNode, fill: String? = null, colors: Map<String, String> = emptyMap(), styles: Map<String, DesignStyle> = emptyMap()): PagePlan {
        val all = listOf(DesignNode(id = "p1", kind = "page", name = "Home", fill = fill)) + nodes.map { if (it.parent == null) it.copy(parent = "p1") else it }
        return planPages(DesignGot(DesignRow(1), all, colors = colors, styles = styles), measure()).single()
    }

    /** [plan] drawn [w] by [h] px at one px a page unit, moved by ([dx], [dy]), on its page's ground. */
    fun draw(plan: PagePlan, w: Int = 240, h: Int = 240, dx: Float = 0f, dy: Float = 0f): Bitmap {
        val image = ImageBitmap(w, h)
        CanvasDrawScope().draw(Density(1f), LayoutDirection.Ltr, Canvas(image), Size(w.toFloat(), h.toFloat())) {
            drawRect(plan.ground)
            translate(dx, dy) { drawPage(plan, hair = 1f) }
        }
        return image.asAndroidBitmap()
    }
}

/** A pixel's colour as `0xRRGGBB`, ignoring alpha (the drawings are on an opaque ground). */
internal fun Bitmap.rgb(x: Int, y: Int): Int = getPixel(x, y) and 0xFFFFFF

/** Whether pixel ([x], [y]) is [want] (`0xRRGGBB`) within [slack] on each channel. */
internal fun Bitmap.near(x: Int, y: Int, want: Int, slack: Int = 24): Boolean {
    val got = rgb(x, y)
    return (0..2).all { abs(((got shr (it * 8)) and 0xFF) - ((want shr (it * 8)) and 0xFF)) <= slack }
}

/** Every pixel's colour in a box, as `0xRRGGBB`. */
internal fun Bitmap.rgbs(x0: Int, y0: Int, x1: Int, y1: Int): List<Int> =
    (y0 until y1).flatMap { y -> (x0 until x1).map { x -> rgb(x, y) } }

internal fun hex(rgb: Int) = "#%06X".format(rgb)

internal const val RED = 0xFF0000
internal const val BLUE = 0x0000FF
internal const val GREEN = 0x00FF00
internal const val WHITE = 0xFFFFFF
internal const val BLACK = 0x000000

/** The PC's canvas, the ground of a page that names no fill. */
internal const val GROUND = 0x1E1D1B
