package com.forge.app.ui.launch

import androidx.compose.ui.geometry.Offset
import androidx.compose.ui.geometry.Size
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.graphics.Path
import androidx.compose.ui.graphics.StrokeCap
import androidx.compose.ui.graphics.StrokeJoin
import androidx.compose.ui.graphics.drawscope.DrawScope
import androidx.compose.ui.graphics.drawscope.Stroke
import androidx.compose.ui.graphics.drawscope.rotate
import androidx.compose.ui.graphics.drawscope.scale
import androidx.compose.ui.graphics.drawscope.translate
import androidx.compose.ui.graphics.drawscope.withTransform
import androidx.compose.ui.graphics.vector.PathParser
import com.forge.app.ui.theme.MascotColors as C
import kotlin.math.PI
import kotlin.math.abs
import kotlin.math.max
import kotlin.math.sin

/**
 * The launch mascots: four characters who occasionally act on the Avex name in the cold-launch
 * intro (see [MascotChoreography]). Ported from the approved sketches: each is drawn in a 200-unit
 * box standing on y = [GROUND], with one soft body, one thing on top, stub arms and legs, and the
 * same six moods. They keep their own colours whatever icon is chosen.
 */
enum class LaunchMascot(internal val rig: MascotRig) {
    Kettle(
        MascotRig(
            ink = C.KettleInk, cheek = C.KettleCheek, heart = C.KettleHeart, tongue = C.KettleTongue,
            top = 50f, eyeY = 116f, eyeDx = 19f, mouthY = 134f, cheekY = 128f, cheekDx = 34f,
            shoulder = Offset(46f, 126f), armLen = 24f, armWidth = 12f, armColor = C.KettleArm,
            hip = Offset(18f, 160f), legLen = 14f, legWidth = 11f, legColor = C.KettleShade,
            back = listOf(
                Part.Stroke("M74 92 C 68 38, 132 38, 126 92", C.KettleBody, 14f),
            ),
            body = listOf(
                Part.Fill("M76 156 L124 156 A6 6 0 0 1 124 168 L76 168 A6 6 0 0 1 76 156 Z", C.KettleShade),
                Part.Oval(100f, 120f, 52f, 46f, C.KettleBody),
                Part.Oval(74f, 100f, 7f, 12f, C.Highlight, rotation = 35f, opacity = 0.35f),
            ),
        )
    ),
    Mochi(
        MascotRig(
            ink = C.SoftInk, cheek = C.SoftCheek, heart = C.HeartRed, tongue = C.SoftTongue,
            top = 48f, eyeY = 120f, eyeDx = 18f, mouthY = 136f, cheekY = 130f, cheekDx = 32f,
            shoulder = Offset(44f, 132f), armLen = 20f, armWidth = 13f, armColor = C.MochiArm,
            hip = Offset(20f, 162f), legLen = 13f, legWidth = 13f, legColor = C.MochiLeg,
            back = listOf(
                Part.Stroke("M100 74 C 100 64, 100 60, 102 54", C.MochiStem, 3.5f),
                Part.Oval(92f, 54f, 9f, 5f, C.MochiLeaf, rotation = -25f),
                Part.Oval(111f, 50f, 10f, 5.5f, C.MochiLeaf, rotation = 20f),
            ),
            body = listOf(
                Part.Fill("M50 162 C 44 108, 64 70, 100 70 C 136 70, 156 108, 150 162 C 130 172, 70 172, 50 162 Z", C.MochiBody),
                Part.Oval(76f, 92f, 8f, 12f, C.Highlight, rotation = 35f, opacity = 0.6f),
            ),
        )
    ),
    Nori(
        MascotRig(
            ink = C.SoftInk, cheek = C.SoftCheek, heart = C.HeartRed, tongue = C.SoftTongue,
            top = 64f, eyeY = 118f, eyeDx = 16f, mouthY = 132f, cheekY = 128f, cheekDx = 29f,
            shoulder = Offset(40f, 144f), armLen = 20f, armWidth = 12f, armColor = C.NoriArm,
            hip = Offset(18f, 164f), legLen = 12f, legWidth = 12f, legColor = C.NoriLeg,
            back = emptyList(),
            body = listOf(
                Part.Fill(
                    "M100 64 C 114 64, 156 130, 154 150 C 152 168, 136 170, 100 170 C 64 170, 48 168, 46 150 C 44 130, 86 64, 100 64 Z",
                    C.NoriBody, outline = 4f,
                ),
                Part.Oval(84f, 96f, 2.5f, 1.5f, C.NoriGrain, rotation = -30f),
                Part.Oval(118f, 104f, 2.5f, 1.5f, C.NoriGrain, rotation = 25f),
                Part.Oval(66f, 140f, 2.5f, 1.5f, C.NoriGrain),
                Part.Oval(136f, 136f, 2.5f, 1.5f, C.NoriGrain, rotation = -20f),
                Part.Fill("M80 148 L120 148 L120 171 L80 171 Z", C.NoriSeaweed, outline = 4f),
                Part.Oval(86f, 86f, 6f, 10f, C.Highlight, rotation = 35f, opacity = 0.7f),
            ),
        )
    ),
    Momo(
        MascotRig(
            ink = C.MomoInk, cheek = C.MomoCheek, heart = C.HeartRed, tongue = C.MomoTongue,
            top = 56f, eyeY = 122f, eyeDx = 18f, mouthY = 138f, cheekY = 132f, cheekDx = 32f,
            shoulder = Offset(44f, 134f), armLen = 20f, armWidth = 12f, armColor = C.MomoArm,
            hip = Offset(18f, 162f), legLen = 13f, legWidth = 12f, legColor = C.MomoLeg,
            back = listOf(
                Part.Stroke("M100 82 C 100 74, 101 68, 104 62", C.MomoStem, 4f),
                Part.Oval(116f, 62f, 13f, 6f, C.MomoLeaf, rotation = -18f),
            ),
            body = listOf(
                Part.Fill("M100 82 C 86 66, 50 72, 50 120 C 50 152, 72 170, 100 170 C 128 170, 150 152, 150 120 C 150 72, 114 66, 100 82 Z", C.MomoBody),
                Part.Stroke("M100 82 C 97 88, 97 94, 99 100", C.MomoCrease, 3f),
                Part.Oval(74f, 98f, 8f, 13f, C.Highlight, rotation = 35f, opacity = 0.35f),
            ),
        )
    );

    companion object {
        /** The y (in rig units) the feet stand on. */
        const val GROUND = 182f

        /** The rig's box: every mascot is drawn in a [BOX]×[BOX] square. */
        const val BOX = 200f
    }
}

/** One of the six sketched moods: which eyes, which mouth, how much blush, which flourish. */
enum class Mood(internal val eyes: Eyes, internal val mouth: Mouth, internal val blush: Float, internal val extra: Extra?) {
    Happy(Eyes.Open, Mouth.Smile, 0.45f, null),
    Cheer(Eyes.Happy, Mouth.Grin, 0.8f, Extra.Sparkles),
    Love(Eyes.Heart, Mouth.Smile, 0.9f, Extra.Hearts),
    Sleepy(Eyes.Closed, Mouth.SmallO, 0.35f, Extra.Zzz),
    Sad(Eyes.Sad, Mouth.Frown, 0.2f, Extra.Tear),
    Shock(Eyes.Wide, Mouth.O, 0.2f, Extra.Shock),
    /** Not a sketched mood: the screwed-shut face of a heavy press. */
    Strain(Eyes.Closed, Mouth.Grin, 0.8f, null),
    /** One eye narrowed, a wobbly mouth and a question mark: what just happened. */
    Confused(Eyes.Puzzled, Mouth.Wavy, 0.3f, Extra.Question),
    /** Eyes closed, smiling: lost in it. */
    Bliss(Eyes.Closed, Mouth.Smile, 0.7f, null),
}

internal enum class Eyes { Open, Happy, Closed, Sad, Wide, Heart, Puzzled }
internal enum class Mouth { Smile, Grin, O, SmallO, Frown, Wavy }
internal enum class Extra { Sparkles, Hearts, Zzz, Tear, Shock, Question }

/** A hand-held or standing prop, drawn in the rig's own units. */
sealed interface Prop {
    /** Held in the left hand (the arm nearest the word), head beyond the fist. */
    data object Hammer : Prop

    /** A cut gem held up in the left hand, in the icon's own colours. */
    data class Gem(val light: Color, val dark: Color) : Prop
}

/**
 * Where a mascot is and what it is doing on one frame. Arm angles are degrees raised OUTWARD from
 * hanging straight down (90 = straight out to the side, 180 = straight up).
 */
data class MascotPose(
    /** The point between the feet, in the canvas's pixels. */
    val ground: Offset,
    /** How far the body is off the ground (px) — the shadow stays down. */
    val lift: Float = 0f,
    val mood: Mood = Mood.Happy,
    val armLeft: Float = 12f,
    val armRight: Float = 12f,
    /** Arm length as a multiple of the rig's own (the press reaches over its head). */
    val armReach: Float = 1f,
    /** + squashes (wider, shorter) from the feet, − stretches. */
    val squash: Float = 0f,
    /** Degrees, about the body's middle. */
    val tilt: Float = 0f,
    val scale: Float = 1f,
    /** A running stride; null stands still. */
    val stride: Float? = null,
    val prop: Prop? = null,
    val alpha: Float = 1f,
    /** Seconds, for blinks and the mood flourishes. */
    val time: Float = 0f,
)

internal sealed interface Part {
    data class Fill(val d: String, val color: Color, val outline: Float = 0f) : Part {
        val path: Path by lazy { PathParser().parsePathString(d).toPath() }
    }
    data class Stroke(val d: String, val color: Color, val width: Float) : Part {
        val path: Path by lazy { PathParser().parsePathString(d).toPath() }
    }
    data class Oval(
        val cx: Float, val cy: Float, val rx: Float, val ry: Float, val color: Color,
        val rotation: Float = 0f, val opacity: Float = 1f,
    ) : Part
}

internal data class MascotRig(
    val ink: Color, val cheek: Color, val heart: Color, val tongue: Color,
    val top: Float, val eyeY: Float, val eyeDx: Float, val mouthY: Float, val cheekY: Float, val cheekDx: Float,
    val shoulder: Offset, val armLen: Float, val armWidth: Float, val armColor: Color,
    val hip: Offset, val legLen: Float, val legWidth: Float, val legColor: Color,
    val back: List<Part>, val body: List<Part>,
)

/** The rig's height above the ground to the top of its head, in rig units. */
internal val LaunchMascot.headroom: Float get() = LaunchMascot.GROUND - rig.top

/**
 * Draws [mascot] in [pose]; [boxPx] is the size of the rig's 200-unit box on screen. Everything is
 * drawn in rig units inside one transform, so the sketches' numbers carry over unchanged.
 */
fun DrawScope.drawMascot(mascot: LaunchMascot, pose: MascotPose, boxPx: Float) {
    if (pose.alpha <= 0f) return
    val rig = mascot.rig
    val k = boxPx / LaunchMascot.BOX * pose.scale
    val a = pose.alpha

    // Shadow: stays on the ground and shrinks as the body rises.
    val shadowScale = 1f / (1f + pose.lift / (boxPx * 0.5f))
    drawOval(
        C.Shadow.copy(alpha = SHADOW_OPACITY * a * shadowScale),
        topLeft = Offset(pose.ground.x - 42f * k * shadowScale, pose.ground.y - 5f * k),
        size = Size(84f * k * shadowScale, 10f * k),
    )

    withTransform({
        translate(pose.ground.x - 100f * k, pose.ground.y - pose.lift - LaunchMascot.GROUND * k)
        scale(k, k, pivot = Offset.Zero)
        // Squash/stretch from the feet; volume roughly kept.
        scale(1f + pose.squash * 0.6f, 1f - pose.squash, pivot = Offset(100f, LaunchMascot.GROUND))
        rotate(pose.tilt, pivot = Offset(100f, 120f))
    }) {
        rig.back.forEach { drawPart(it, a) }
        drawLegs(rig, pose, a)
        rig.body.forEach { drawPart(it, a) }
        drawFace(rig, pose, a)
        drawArm(rig, side = 1f, angle = pose.armRight, reach = pose.armReach, alpha = a, prop = null)
        drawArm(rig, side = -1f, angle = pose.armLeft, reach = pose.armReach, alpha = a, prop = pose.prop)
        pose.mood.extra?.let { drawExtra(rig, it, pose.time, a) }
    }
}

private fun DrawScope.drawPart(part: Part, a: Float) {
    when (part) {
        is Part.Fill -> {
            drawPath(part.path, part.color.copy(alpha = a))
            if (part.outline > 0f) {
                drawPath(part.path, part.color.copy(alpha = a), style = Stroke(part.outline, join = StrokeJoin.Round))
            }
        }
        is Part.Stroke -> drawPath(part.path, part.color.copy(alpha = a), style = Stroke(part.width, cap = StrokeCap.Round))
        is Part.Oval -> rotate(part.rotation, pivot = Offset(part.cx, part.cy)) {
            drawOval(
                part.color.copy(alpha = part.opacity * a),
                topLeft = Offset(part.cx - part.rx, part.cy - part.ry),
                size = Size(part.rx * 2f, part.ry * 2f),
            )
        }
    }
}

private fun DrawScope.drawLegs(rig: MascotRig, pose: MascotPose, a: Float) {
    for (side in listOf(-1f, 1f)) {
        // A stride lifts each foot in turn and swings it a little.
        val phase = pose.stride?.let { sin(it + if (side < 0f) 0f else PI.toFloat()) } ?: 0f
        val up = max(0f, phase) * 7f
        val swing = phase * 4f
        val x = 100f + side * rig.hip.x
        drawLine(
            rig.legColor.copy(alpha = a),
            start = Offset(x, rig.hip.y),
            end = Offset(x + swing, rig.hip.y + rig.legLen - up),
            strokeWidth = rig.legWidth,
            cap = StrokeCap.Round,
        )
    }
}

private fun DrawScope.drawArm(rig: MascotRig, side: Float, angle: Float, reach: Float, alpha: Float, prop: Prop?) {
    val len = rig.armLen * reach
    withTransform({
        translate(100f + side * rig.shoulder.x, rig.shoulder.y)
        if (side < 0f) scale(-1f, 1f, pivot = Offset.Zero)
        rotate(-angle, pivot = Offset.Zero)
    }) {
        if (prop == Prop.Hammer) drawHammer(Offset(0f, len - 4f), alpha)
        drawLine(rig.armColor.copy(alpha = alpha), Offset.Zero, Offset(0f, len), strokeWidth = rig.armWidth, cap = StrokeCap.Round)
        // The gem sits in the fist, drawn over it so it reads as held up.
        if (prop is Prop.Gem) translate(0f, len + 14f) { rotate(180f, pivot = Offset.Zero) { drawGem(prop.light, prop.dark, 1.7f, alpha) } }
    }
}

/** A hammer whose handle starts at [grip] and runs down its local +y, head across the end. */
internal fun DrawScope.drawHammer(grip: Offset, alpha: Float) {
    drawLine(C.HammerHandle.copy(alpha = alpha), grip, grip + Offset(0f, 28f), strokeWidth = 5f, cap = StrokeCap.Round)
    drawRoundRect(
        C.HammerHead.copy(alpha = alpha), topLeft = grip + Offset(-14f, 24f), size = Size(28f, 15f),
        cornerRadius = androidx.compose.ui.geometry.CornerRadius(3f),
    )
    drawLine(C.HammerShine.copy(alpha = alpha), grip + Offset(-10f, 27f), grip + Offset(10f, 27f), strokeWidth = 2f, cap = StrokeCap.Round)
}

/**
 * A floor lever, base centred on the origin's floor: [pulled] 0 = up, 1 = thrown. Drawn in rig units;
 * the caller places and scales it.
 */
internal fun DrawScope.drawLever(pulled: Float, a: Float) {
    drawRoundRect(
        C.LeverBase.copy(alpha = a), topLeft = Offset(-12f, -12f), size = Size(24f, 12f),
        cornerRadius = androidx.compose.ui.geometry.CornerRadius(3f),
    )
    rotate(-35f + 70f * pulled, pivot = Offset(0f, -10f)) {
        drawLine(C.LeverBase.copy(alpha = a), Offset(0f, -10f), Offset(0f, -40f), strokeWidth = 4f, cap = StrokeCap.Round)
        drawCircle(C.LeverKnob.copy(alpha = a), radius = 6f, center = Offset(0f, -42f))
    }
}

/** A brilliant-cut gem about 22 units across, point down, centred on the origin. */
internal fun DrawScope.drawGem(light: Color, dark: Color, size: Float, a: Float) {
    val s = size
    val crown = Path().apply { moveTo(-11f * s, -3f * s); lineTo(-6f * s, -9f * s); lineTo(6f * s, -9f * s); lineTo(11f * s, -3f * s); close() }
    val left = Path().apply { moveTo(-11f * s, -3f * s); lineTo(0f, 12f * s); lineTo(0f, -3f * s); close() }
    val right = Path().apply { moveTo(11f * s, -3f * s); lineTo(0f, 12f * s); lineTo(0f, -3f * s); close() }
    drawPath(left, light.copy(alpha = a))
    drawPath(right, dark.copy(alpha = a))
    drawPath(crown, light.copy(alpha = a))
    drawLine(C.Highlight.copy(alpha = a), Offset(-5f * s, -7f * s), Offset(-1f * s, -7f * s), strokeWidth = 1.6f * s, cap = StrokeCap.Round)
}

private fun DrawScope.drawFace(rig: MascotRig, pose: MascotPose, a: Float) {
    val mood = pose.mood
    for (side in listOf(-1f, 1f)) {
        drawOval(
            rig.cheek.copy(alpha = mood.blush * a),
            topLeft = Offset(100f + side * rig.cheekDx - 7f, rig.cheekY - 4f),
            size = Size(14f, 8f),
        )
    }
    // Blink for ~0.12s every ~2.6s, offset so it never lands on the first beat.
    val blinking = ((pose.time + 1.1f) % 2.6f) < 0.12f
    for (side in listOf(-1f, 1f)) {
        translate(100f + side * rig.eyeDx, rig.eyeY) { drawEye(rig, mood.eyes, side, blinking, pose.time, a) }
    }
    translate(100f, rig.mouthY) { drawMouth(rig, mood.mouth, a) }
}

private fun DrawScope.drawEye(rig: MascotRig, eyes: Eyes, side: Float, blinking: Boolean, time: Float, a: Float) {
    val ink = rig.ink.copy(alpha = a)
    val white = C.Highlight.copy(alpha = a)
    when (eyes) {
        Eyes.Open -> scale(1f, if (blinking) 0.12f else 1f, pivot = Offset.Zero) {
            drawOval(ink, topLeft = Offset(-6.5f, -8.5f), size = Size(13f, 17f))
            drawCircle(white, radius = 2.3f, center = Offset(2f, -3.2f))
        }
        Eyes.Happy -> drawPath(quad(-7f, 3f, 0f, -7f, 7f, 3f), ink, style = line(3.8f))
        Eyes.Closed -> drawPath(quad(-7f, -1f, 0f, 5f, 7f, -1f), ink, style = line(3.4f))
        Eyes.Sad -> {
            drawOval(ink, topLeft = Offset(-5.5f, -5f), size = Size(11f, 14f))
            drawCircle(white, radius = 2f, center = Offset(1.8f, -1f))
            val brow = if (side < 0f) listOf(-8f, -9f, 7f, -14f) else listOf(8f, -9f, -7f, -14f)
            drawLine(ink, Offset(brow[0], brow[1]), Offset(brow[2], brow[3]), strokeWidth = 3f, cap = StrokeCap.Round)
        }
        Eyes.Wide -> {
            drawCircle(ink, radius = 9f, center = Offset.Zero)
            drawCircle(white, radius = 3.4f, center = Offset(3f, -3f))
            drawCircle(white, radius = 1.4f, center = Offset(-3f, 3.5f))
            drawPath(quad(-7f, -16f, 0f, -20f, 7f, -16f), ink, style = line(3f))
        }
        Eyes.Puzzled -> if (side < 0f) {
            drawOval(ink, topLeft = Offset(-6.5f, -8.5f), size = Size(13f, 17f))
            drawCircle(white, radius = 2.3f, center = Offset(2f, -3.2f))
            drawPath(quad(-7f, -15f, 0f, -20f, 7f, -15f), ink, style = line(3f))
        } else {
            drawLine(ink, Offset(-6f, 0f), Offset(6f, 0f), strokeWidth = 3.4f, cap = StrokeCap.Round)
            drawLine(ink, Offset(-7f, -9f), Offset(7f, -7f), strokeWidth = 3f, cap = StrokeCap.Round)
        }
        Eyes.Heart -> scale(1.1f + 0.08f * sin(time * 9f), pivot = Offset.Zero) {
            drawPath(heart(), rig.heart.copy(alpha = a))
        }
    }
}

private fun DrawScope.drawMouth(rig: MascotRig, mouth: Mouth, a: Float) {
    val ink = rig.ink.copy(alpha = a)
    when (mouth) {
        Mouth.Smile -> drawPath(quad(-7f, -2f, 0f, 6f, 7f, -2f), ink, style = line(3.5f))
        Mouth.Frown -> drawPath(quad(-7f, 3f, 0f, -4f, 7f, 3f), ink, style = line(3.5f))
        Mouth.Wavy -> drawPath(
            Path().apply { moveTo(-8f, 0f); quadraticTo(-4f, -3f, 0f, 0f); quadraticTo(4f, 3f, 8f, 0f) },
            ink, style = line(3f),
        )
        Mouth.O -> drawOval(ink, topLeft = Offset(-5f, -6.5f), size = Size(10f, 13f))
        Mouth.SmallO -> drawOval(ink, topLeft = Offset(-3f, -2.5f), size = Size(6f, 7f))
        Mouth.Grin -> {
            val p = Path().apply {
                moveTo(-10f, -3f); lineTo(10f, -3f)
                quadraticTo(10f, 10f, 0f, 10f); quadraticTo(-10f, 10f, -10f, -3f); close()
            }
            drawPath(p, ink)
            drawOval(rig.tongue.copy(alpha = a), topLeft = Offset(-5f, 3f), size = Size(10f, 6f))
        }
    }
}

private fun DrawScope.drawExtra(rig: MascotRig, extra: Extra, t: Float, a: Float) {
    when (extra) {
        Extra.Sparkles -> listOf(Triple(38f, 70f, 0f), Triple(162f, 58f, 0.3f), Triple(150f, 110f, 0.6f), Triple(52f, 120f, 0.15f))
            .forEach { (x, y, d) ->
                val s = 0.35f + 0.65f * abs(sin((t + d) * 5f))
                translate(x, y) { scale(s, pivot = Offset.Zero) { drawPath(star(), C.Sparkle.copy(alpha = a)) } }
            }
        Extra.Hearts -> listOf(Triple(150f, 78f, 0f), Triple(50f, 92f, 0.4f), Triple(140f, 50f, 0.8f)).forEach { (x, y, d) ->
            val u = ((t * 0.7f + d) % 1f)
            translate(x, y - u * 26f) { scale(0.8f, pivot = Offset.Zero) { drawPath(heart(), C.HeartRed.copy(alpha = a * (1f - u))) } }
        }
        Extra.Zzz -> listOf(Triple(138f, 70f, 12f), Triple(150f, 56f, 15f), Triple(164f, 40f, 18f)).forEachIndexed { i, (x, y, s) ->
            val u = ((t * 0.6f + i * 0.33f) % 1f)
            val z = Path().apply { moveTo(0f, 0f); lineTo(s * 0.6f, 0f); lineTo(0f, s * 0.7f); lineTo(s * 0.6f, s * 0.7f) }
            translate(x, y - u * 10f) { drawPath(z, C.Snore.copy(alpha = a * (1f - u)), style = line(2.4f)) }
        }
        Extra.Tear -> {
            val u = (t * 0.9f) % 1f
            translate(100f - rig.eyeDx + 3f, rig.eyeY + 9f + u * 22f) { drawPath(drop(), C.Tear.copy(alpha = a * (1f - u))) }
        }
        Extra.Question -> translate(146f, rig.top - 6f + 3f * sin(t * 4f)) {
            val mark = C.Chalk.copy(alpha = a)
            val hook = Path().apply { moveTo(-7f, -12f); cubicTo(-7f, -22f, 9f, -22f, 9f, -12f); cubicTo(9f, -5f, 0f, -5f, 0f, 3f) }
            drawPath(hook, mark, style = line(3.6f))
            drawCircle(mark, radius = 2.4f, center = Offset(0f, 10f))
        }
        Extra.Shock -> {
            translate(100f + rig.eyeDx + 30f, rig.eyeY - 30f) { drawPath(drop(), C.Tear.copy(alpha = a)) }
            val top = rig.top
            val lines = C.Chalk.copy(alpha = SHOCK_LINE_OPACITY * a)
            drawLine(lines, Offset(100f, top - 10f), Offset(100f, top - 20f), strokeWidth = 3f, cap = StrokeCap.Round)
            drawLine(lines, Offset(84f, top - 6f), Offset(78f, top - 15f), strokeWidth = 3f, cap = StrokeCap.Round)
            drawLine(lines, Offset(116f, top - 6f), Offset(122f, top - 15f), strokeWidth = 3f, cap = StrokeCap.Round)
        }
    }
}

// Illustration opacities from the sketches. These paint a character, not UI, so they sit outside
// the §5 intensity ladder the way a gradient or scrim does.
private const val SHADOW_OPACITY = 0.45f
private const val SHOCK_LINE_OPACITY = 0.8f

private fun line(w: Float) = Stroke(w, cap = StrokeCap.Round, join = StrokeJoin.Round)

private fun quad(x0: Float, y0: Float, cx: Float, cy: Float, x1: Float, y1: Float) =
    Path().apply { moveTo(x0, y0); quadraticTo(cx, cy, x1, y1) }

private fun heart() = Path().apply {
    moveTo(0f, 7f)
    cubicTo(-12f, -1f, -9f, -13f, 0f, -6f)
    cubicTo(9f, -13f, 12f, -1f, 0f, 7f)
    close()
}

internal fun star() = Path().apply {
    moveTo(0f, -7f); lineTo(1.8f, -1.8f); lineTo(7f, 0f); lineTo(1.8f, 1.8f)
    lineTo(0f, 7f); lineTo(-1.8f, 1.8f); lineTo(-7f, 0f); lineTo(-1.8f, -1.8f); close()
}

private fun drop() = Path().apply {
    moveTo(0f, 0f)
    cubicTo(-5f, 7f, -5f, 12f, 0f, 12f)
    cubicTo(5f, 12f, 5f, 7f, 0f, 0f)
    close()
}
