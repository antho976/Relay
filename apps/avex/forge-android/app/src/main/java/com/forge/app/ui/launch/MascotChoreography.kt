package com.forge.app.ui.launch

import androidx.compose.ui.geometry.Offset
import androidx.compose.ui.geometry.Rect
import androidx.compose.ui.geometry.Size
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.graphics.StrokeCap
import androidx.compose.ui.graphics.drawscope.DrawScope
import androidx.compose.ui.graphics.drawscope.rotate
import androidx.compose.ui.graphics.drawscope.scale
import androidx.compose.ui.graphics.drawscope.translate
import androidx.compose.ui.graphics.lerp as lerpColor
import androidx.compose.ui.util.lerp
import com.forge.app.appicon.IconFamily
import com.forge.app.ui.theme.MascotColors
import kotlin.math.PI
import kotlin.math.atan2
import kotlin.math.cos
import kotlin.math.exp
import kotlin.math.hypot
import kotlin.math.max
import kotlin.math.pow
import kotlin.math.sin

/**
 * Where the Avex name sits on the intro plate, in the mascot canvas's pixels. [layer] is the
 * wordmark's whole effect layer (padded for the warped families), which is where Nebula's black
 * hole is anchored; [aLeft]..[aRight] is the "A"'s own box, whose peak Avex's mascot lands on.
 */
data class WordFrame(
    val text: Rect,
    val baseline: Float,
    val capTop: Float,
    val layer: Rect,
    val aLeft: Float = text.left,
    val aRight: Float = text.left + text.width * 0.27f,
) {
    val capHeight: Float get() = baseline - capTop
    /** The top of the lower-case letters (v, e, x), where a mascot can stand. */
    val xTop: Float get() = baseline - capHeight * 0.55f
}

/** Everything a routine needs that does not change frame to frame. */
data class MascotScene(
    val family: IconFamily,
    val mascot: LaunchMascot,
    val word: WordFrame,
    /** The on-screen size of the rig's 200-unit box, px. */
    val box: Float,
    val canvas: Size,
    /** The icon's launch palette, deep → bright (never empty: callers pass a neutral fallback). */
    val palette: List<Color>,
)

/**
 * The mascot's part in each icon family's intro. One routine per family; the icon's palette
 * reskins the scene (the wordmark, the light trails, the plucked gem) and any of the four mascots
 * can play it.
 *
 * Time `s` is seconds since the intro started. The mascot arrives during the family's [leadFor]
 * seconds, then the wordmark's own clock starts (`tw = s - lead`), so every beat here lines up with
 * the family effect it acts on: Solid's wipe, Metal's sheen, Stealth's flicker, Gem's crystals.
 * `exit` is the intro's choreographed death (0 until the hold ends) and `sinceExit` the seconds
 * since it began (negative before), for reactions that should flow at their own pace rather than at
 * the death's easing.
 */
object MascotChoreography {
    /** Seconds a mascot takes to hop on stage. */
    const val ENTER = 0.55f

    /** Stealth's mascot gets a lever thrown at it and switches the name on itself. */
    private const val STEALTH_LEAD = 1.6f

    /** Seconds before the name appears: the mascot's entrance, plus Stealth's whole bit. */
    fun leadFor(family: IconFamily): Float = if (family == IconFamily.Stealth) STEALTH_LEAD else ENTER

    fun pose(scene: MascotScene, s: Float, exit: Float, sinceExit: Float): MascotPose {
        val word = scene.word
        val box = scene.box
        val tw = s - leadFor(scene.family)
        val w = word.text.width
        val rightHome = Offset(word.text.right + box * 0.42f, word.baseline)
        val offRight = Offset(scene.canvas.width + box * 0.6f, word.baseline)
        val offLeft = Offset(-box * 0.6f, word.baseline)

        return when (scene.family) {
            IconFamily.Avex -> avexSlide(scene, s, tw)
            IconFamily.Metal -> metalSkate(scene, s, tw)
            IconFamily.Stealth -> stealthLever(scene, s, tw, rightHome, offRight)
            IconFamily.Gem -> gemPluck(scene, s, tw, rightHome, offRight)
            IconFamily.Aurora -> auroraConduct(s, tw, rightHome, offRight, box)
            IconFamily.Molten -> moltenHammer(s, tw, sinceExit, rightHome, offRight, box)
            IconFamily.Nebula -> nebulaPull(scene, s, exit)

            // Runs across dragging the plate colour behind it, then runs back to wipe it out.
            IconFamily.Solid -> {
                val leftHome = Offset(word.text.left - box * 0.35f, word.baseline)
                when {
                    tw < 0.10f -> arrive(offLeft, leftHome, s, box).copy(mood = Mood.Happy, time = s)
                    exit > 0f -> MascotPose(
                        ground = Offset(word.text.left + (1.10f - 1.30f * exit) * w, word.baseline),
                        mood = Mood.Happy, stride = s * 30f, tilt = -8f, armLeft = 40f, armRight = 40f, time = s,
                    )
                    else -> {
                        val u = ((tw - 0.10f) / 0.55f).coerceIn(0f, 1f)
                        val edge = word.text.left + (-0.10f + 1.2f * u) * w
                        val settle = ((tw - 0.65f) / 0.2f).coerceIn(0f, 1f)
                        val x = lerp(edge, rightHome.x, ease(settle))
                        val running = settle < 1f
                        MascotPose(
                            ground = Offset(x, word.baseline),
                            mood = Mood.Cheer,
                            stride = if (running) s * 30f else null,
                            tilt = if (running) 8f else 0f,
                            armLeft = if (running) 40f else 150f + 20f * sin(s * 16f),
                            armRight = if (running) 40f else 150f + 20f * sin(s * 16f + 1.6f),
                            time = s,
                        )
                    }
                }
            }



            // Presses the name overhead like a bar: crouch, drive, lock out.
            IconFamily.Gym -> {
                val mascot = scene.mascot
                val k = box / LaunchMascot.BOX
                val handTop = mascot.rig.top - 8f
                val reachUp = (mascot.rig.shoulder.y - handTop) / mascot.rig.armLen
                val ground = Offset(word.text.center.x, word.baseline + box * 0.1f + (LaunchMascot.GROUND - handTop) * k)
                val armsUp = ((s - (ENTER - 0.2f)) / 0.2f).coerceIn(0f, 1f)
                val from = Offset(-box * 0.6f, ground.y)
                val base = arrive(from, ground, s, box)
                base.copy(
                    squash = if (tw < 0f) base.squash else gymSquash(tw),
                    mood = if (tw in 0f..0.45f) Mood.Strain else if (tw > 0.45f) Mood.Cheer else Mood.Happy,
                    armLeft = lerp(12f, 172f, ease(armsUp)),
                    armRight = lerp(12f, 172f, ease(armsUp)),
                    armReach = lerp(1f, reachUp, ease(armsUp)),
                    time = s,
                )
            }
        }
    }

    // ── Avex: lands on the peak of the A, wobbles, slides down it, hops off and waves ──────────

    private fun avexSlide(scene: MascotScene, s: Float, tw: Float): MascotPose {
        val word = scene.word
        val box = scene.box
        val apex = Offset((word.aLeft + word.aRight) / 2f, word.capTop)
        val foot = Offset(word.aLeft + box * 0.04f, word.baseline)
        val home = Offset(word.text.left - box * 0.42f, word.baseline)
        return when {
            // Drops out of the sky onto the point of the A.
            tw < 0f -> {
                val u = (s / ENTER).coerceIn(0f, 1f)
                MascotPose(
                    ground = Offset(apex.x, lerp(-box, apex.y, u * u)),
                    squash = -0.12f, armLeft = 160f, armRight = 160f, mood = Mood.Shock, time = s,
                )
            }
            // Balancing on the peak.
            tw < 0.40f -> MascotPose(
                ground = apex,
                squash = 0.22f * exp(-tw * 12f) * cos(tw * 30f),
                tilt = 16f * sin(tw * 20f) * exp(-tw * 3f),
                armLeft = 95f + 18f * sin(tw * 26f), armRight = 95f + 18f * sin(tw * 26f + PI.toFloat()),
                mood = Mood.Shock, time = s,
            )
            // Down the left stroke like a slide, gathering speed.
            tw < 0.76f -> {
                val u = (tw - 0.40f) / 0.36f
                MascotPose(
                    ground = apex + (foot - apex) * (u * u),
                    tilt = -28f, armLeft = 150f, armRight = 150f, mood = Mood.Cheer, time = s,
                )
            }
            // Off the end with a hop.
            tw < 1.0f -> {
                val q = (tw - 0.76f) / 0.24f
                MascotPose(
                    ground = foot + (home - foot) * ease(q),
                    lift = box * 0.2f * 4f * q * (1f - q),
                    tilt = lerp(-28f, 0f, ease(q * 2f)),
                    armLeft = 150f, armRight = 150f, mood = Mood.Cheer, time = s,
                )
            }
            else -> {
                val t = tw - 1.0f
                MascotPose(
                    ground = home,
                    squash = 0.2f * exp(-t * 10f) * cos(t * 28f),
                    armRight = 130f + 25f * sin(t * 14f),
                    mood = Mood.Happy, time = s,
                )
            }
        }
    }

    // ── Metal: skates the polished letters with the sheen, slips off the x, laughs ─────────────

    private fun metalSkate(scene: MascotScene, s: Float, tw: Float): MascotPose {
        val word = scene.word
        val box = scene.box
        val w = word.text.width
        val start = Offset(word.text.left + 0.30f * w, word.xTop)
        val floor = Offset(start.x - box * 0.55f, word.baseline)
        val edge = word.text.right + box * 0.04f
        // When the sheen reaches the edge, the mascot runs out of metal.
        val slipAt = 0.25f + 0.60f * (((edge - word.text.left) / w + 0.25f) / 1.5f)
        val landX = edge + box * 0.38f
        return when {
            tw < 0f -> arrive(Offset(-box * 0.6f, word.baseline), floor, s, box).copy(mood = Mood.Happy, time = s)
            // Up onto the lower-case letters.
            tw < 0.22f -> {
                val q = tw / 0.22f
                MascotPose(
                    ground = floor + (start - floor) * ease(q),
                    lift = box * 0.25f * 4f * q * (1f - q),
                    armLeft = 70f, armRight = 70f, mood = Mood.Happy, time = s,
                )
            }
            // Skating with the sheen: no stride, arms out for balance.
            tw < slipAt -> {
                val u = ((tw - 0.25f) / 0.60f).coerceIn(0f, 1f)
                val x = max(start.x, word.text.left + (-0.25f + 1.5f * u) * w)
                MascotPose(
                    ground = Offset(x, word.xTop),
                    squash = if (tw < 0.3f) 0.15f * exp(-(tw - 0.22f) * 20f) else 0f,
                    tilt = 7f + 5f * sin(s * 7f),
                    armLeft = 95f + 12f * sin(s * 9f), armRight = 95f + 12f * sin(s * 9f + 2f),
                    mood = Mood.Cheer, time = s,
                )
            }
            // Off the end of the x.
            tw < slipAt + 0.30f -> {
                val q = (tw - slipAt) / 0.30f
                MascotPose(
                    ground = Offset(lerp(edge, landX, q), lerp(word.xTop, word.baseline, q * q)),
                    tilt = lerp(7f, -35f, q),
                    armLeft = 165f + 10f * sin(s * 30f), armRight = 165f + 10f * sin(s * 30f + 1f),
                    mood = Mood.Shock, time = s,
                )
            }
            else -> {
                val t = tw - (slipAt + 0.30f)
                MascotPose(
                    ground = Offset(landX, word.baseline),
                    squash = 0.3f * exp(-t * 9f) * cos(t * 26f),
                    tilt = -35f * exp(-t * 7f),
                    armLeft = if (t < 0.25f) 60f else 150f + 18f * sin(t * 16f),
                    armRight = if (t < 0.25f) 60f else 150f + 18f * sin(t * 16f + 1.6f),
                    mood = if (t < 0.25f) Mood.Shock else Mood.Cheer, time = s,
                )
            }
        }
    }

    // ── Stealth: a lever is thrown at it, it is baffled, then it switches the name on ─────────

    private const val LEVER_THROWN = 0.60f
    private const val LEVER_BONK = 0.92f
    private const val LEVER_LANDS = 1.16f
    private const val REACH_AT = 1.40f
    private const val PULL_AT = 1.50f

    private fun stealthLever(scene: MascotScene, s: Float, tw: Float, home: Offset, offRight: Offset): MascotPose {
        val base = arrive(offRight, home, s, scene.box)
        val bonk = s - LEVER_BONK
        val pull = ((s - PULL_AT) / (STEALTH_LEAD - PULL_AT)).coerceIn(0f, 1f)
        val reach = ((s - REACH_AT) / (PULL_AT - REACH_AT)).coerceIn(0f, 1f)
        return base.copy(
            squash = base.squash + if (bonk >= 0f) 0.28f * exp(-bonk * 9f) * cos(bonk * 26f) else 0f,
            tilt = when {
                s in LEVER_LANDS..REACH_AT -> 12f * ease((s - LEVER_LANDS) / 0.15f)
                s in REACH_AT..PULL_AT -> 12f * (1f - reach)
                else -> 0f
            },
            mood = when {
                s < LEVER_BONK -> Mood.Happy
                s < LEVER_LANDS -> Mood.Shock
                s < REACH_AT -> Mood.Confused
                tw < 0f -> Mood.Happy
                tw < 0.3f -> Mood.Shock
                else -> Mood.Cheer
            },
            armRight = when {
                s < ENTER -> base.armRight
                s < LEVER_BONK -> 12f
                s < LEVER_LANDS -> 110f
                // Scratching its head at the thing.
                s < REACH_AT -> 158f + 8f * sin(s * 28f)
                s < PULL_AT -> lerp(158f, 62f, ease(reach))
                tw < 0f -> lerp(62f, 38f, pull)
                tw < 0.3f -> 38f
                else -> 150f + 18f * sin(s * 16f)
            },
            armLeft = when {
                s in LEVER_BONK..LEVER_LANDS -> 110f
                tw > 0.3f -> 150f + 18f * sin(s * 16f + 1.6f)
                else -> base.armLeft
            },
            time = s,
        )
    }

    /** The thrown lever: where its base is, how it is turned, and how far it is pulled. */
    private data class LeverState(val at: Offset, val spin: Float, val pulled: Float)

    private fun leverState(scene: MascotScene, s: Float): LeverState? {
        if (s < LEVER_THROWN) return null
        val word = scene.word
        val box = scene.box
        val k = box / LaunchMascot.BOX
        val home = Offset(word.text.right + box * 0.42f, word.baseline)
        val head = Offset(home.x, word.baseline - (scene.mascot.headroom + 6f) * k)
        val spot = Offset(home.x + box * 0.46f, word.baseline)
        return when {
            s < LEVER_BONK -> {
                val q = (s - LEVER_THROWN) / (LEVER_BONK - LEVER_THROWN)
                val from = Offset(word.text.left - box, -box)
                LeverState(from + (head - from) * q - Offset(0f, box * 0.5f * 4f * q * (1f - q)), 720f * q, 0f)
            }
            s < LEVER_LANDS -> {
                val q = (s - LEVER_BONK) / (LEVER_LANDS - LEVER_BONK)
                LeverState(head + (spot - head) * q - Offset(0f, box * 0.35f * 4f * q * (1f - q)), 720f + 360f * q, 0f)
            }
            else -> {
                val t = s - LEVER_LANDS
                val pulled = ((s - PULL_AT) / (STEALTH_LEAD - PULL_AT)).coerceIn(0f, 1f)
                LeverState(spot, 14f * sin(t * 24f) * exp(-t * 6f), ease(pulled))
            }
        }
    }

    // ── Gem: startled by the crystals, plucks one, holds it up ──────────────────────────────────

    private const val PLUCK = 0.80f

    private fun gemPluck(scene: MascotScene, s: Float, tw: Float, home: Offset, offRight: Offset): MascotPose {
        val box = scene.box
        val base = arrive(offRight, home, s, box)
        val gem = Prop.Gem(light = scene.palette.last(), dark = scene.palette[scene.palette.size / 2])
        return when {
            tw < 0.32f -> base.copy(mood = Mood.Happy, time = s)
            // The crystals burst out: it hops back.
            tw < 0.55f -> {
                val q = (tw - 0.32f) / 0.23f
                base.copy(
                    ground = home + Offset(box * 0.14f * ease(q * 2f), 0f),
                    lift = box * 0.14f * sin(PI.toFloat() * q),
                    armLeft = 120f, armRight = 120f, mood = Mood.Shock, time = s,
                )
            }
            // Leans in and reaches for one.
            tw < PLUCK -> {
                val q = (tw - 0.55f) / (PLUCK - 0.55f)
                base.copy(
                    ground = home + Offset(box * 0.14f * (1f - ease(q)), 0f),
                    tilt = -14f * ease(q), armLeft = lerp(12f, 100f, ease(q)), mood = Mood.Happy, time = s,
                )
            }
            // Plucked: pulls it back, then lifts it high.
            tw < PLUCK + 0.10f -> base.copy(
                tilt = -14f * (1f - (tw - PLUCK) / 0.10f),
                armLeft = lerp(100f, 35f, (tw - PLUCK) / 0.10f), prop = gem, mood = Mood.Shock, time = s,
            )
            else -> {
                val q = ((tw - PLUCK - 0.10f) / 0.22f).coerceIn(0f, 1f)
                base.copy(
                    armLeft = lerp(35f, 168f, ease(q)), armReach = lerp(1f, 1.9f, ease(q)), prop = gem,
                    squash = if (q >= 1f) 0.04f * sin(s * 10f) else 0f,
                    mood = Mood.Love, time = s,
                )
            }
        }
    }

    // ── Aurora: conducts the lights, trailing ribbons of them from its hands ────────────────────

    private const val CONDUCT_FROM = 0.25f
    private const val CONDUCT_TO = 1.15f

    private fun auroraConduct(s: Float, tw: Float, home: Offset, offRight: Offset, box: Float): MascotPose {
        val base = arrive(offRight, home, s, box)
        if (tw < CONDUCT_FROM) return base.copy(mood = Mood.Happy, time = s)
        val c = (tw - CONDUCT_FROM).coerceAtMost(CONDUCT_TO - CONDUCT_FROM)
        val beat = 2f * PI.toFloat() * 1.5f * c
        val conducting = tw < CONDUCT_TO
        // Ease into the sweep so the first beat is not a jump.
        val swell = ease(c / 0.2f)
        return base.copy(
            armRight = if (conducting) lerp(12f, 125f + 40f * sin(beat), swell) else 155f,
            armLeft = if (conducting) lerp(12f, 125f + 40f * sin(beat + PI.toFloat()), swell) else 155f,
            tilt = if (conducting) 6f * sin(beat) * swell else 0f,
            squash = if (conducting) 0.04f * sin(beat * 2f) * swell else 0f,
            mood = if (conducting) Mood.Bliss else Mood.Love,
            time = s,
        )
    }

    // ── Molten: hammers the name, drops the hammer when it melts, hops back and cries ─────────

    private fun moltenHammer(s: Float, tw: Float, sinceExit: Float, home: Offset, offRight: Offset, box: Float): MascotPose {
        val base = arrive(offRight, home, s, box)
        if (sinceExit < 0f) {
            return base.copy(
                mood = if (tw >= 0f) Mood.Strain else Mood.Happy,
                armLeft = hammerArm(tw),
                squash = base.squash + impact(tw) * 0.1f,
                prop = Prop.Hammer,
                time = s,
            )
        }
        val se = sinceExit
        val back = home + Offset(box * 0.36f, 0f)
        return when {
            // Flinches and hops back as the name gives way; the hammer falls.
            se < 0.30f -> {
                val q = se / 0.30f
                MascotPose(
                    ground = home + (back - home) * ease(q),
                    lift = box * 0.16f * sin(PI.toFloat() * q),
                    armLeft = lerp(hammerArm(tw), 125f, ease(q * 3f)), armRight = 125f,
                    mood = Mood.Shock, time = s,
                )
            }
            // Lands, stares for a beat.
            se < 0.45f -> {
                val t = se - 0.30f
                MascotPose(
                    ground = back, squash = 0.2f * exp(-t * 12f),
                    armLeft = lerp(125f, 30f, t / 0.15f), armRight = lerp(125f, 30f, t / 0.15f),
                    mood = Mood.Shock, time = s,
                )
            }
            // Slumps into tears.
            else -> {
                val q = ease((se - 0.45f) / 0.25f)
                MascotPose(
                    ground = back,
                    squash = 0.07f * q + 0.02f * sin(se * 18f) * q,
                    tilt = 7f * q,
                    armLeft = lerp(30f, 4f, q), armRight = lerp(30f, 4f, q),
                    mood = Mood.Sad, time = s,
                )
            }
        }
    }

    // ── Nebula: floats in asleep, then is dragged round and into the black hole ─────────────────

    private fun nebulaPull(scene: MascotScene, s: Float, exit: Float): MascotPose {
        val word = scene.word
        val box = scene.box
        val home = Offset(word.text.right + box * 0.35f, word.baseline - box * 0.3f)
        val from = Offset(scene.canvas.width + box, word.text.top - box * 0.6f)
        val u = ease((s / ENTER).coerceIn(0f, 1f))
        val floating = Offset(lerp(from.x, home.x, u), lerp(from.y, home.y, u)) +
            Offset(box * 0.04f * sin(s * 0.9f), box * 0.05f * sin(s * 1.4f))
        val sway = 12f * sin(s * 1.3f)
        if (exit <= 0f) {
            return MascotPose(
                ground = floating, mood = Mood.Sleepy,
                armLeft = 45f + 10f * sin(s * 2f), armRight = 45f + 10f * sin(s * 2f + 1f),
                tilt = sway, time = s,
            )
        }
        // The body's middle is [BODY_MID] rig units above its feet; that is what orbits the hole.
        val k = box / LaunchMascot.BOX
        val hole = Offset(word.layer.left + word.layer.width * 0.68f, word.layer.top + word.layer.height * 0.42f)
        val mid0 = floating - Offset(0f, BODY_MID * k)
        val r0 = hypot(mid0.x - hole.x, mid0.y - hole.y)
        val a0 = atan2(mid0.y - hole.y, mid0.x - hole.x)
        // A tightening spiral: barely moves while it resists, then whips round and in.
        val e = exit
        val r = r0 * (1f - ease(e).pow(1.4f))
        val a = a0 + 2.4f * PI.toFloat() * e * e
        val scale = 1f - 0.92f * e.pow(3f)
        val mid = hole + Offset(cos(a) * r, sin(a) * r)
        // Feet first: turn so its "down" points at the singularity.
        val toward = Math.toDegrees(atan2(-(hole.x - mid.x), hole.y - mid.y).toDouble()).toFloat()
        return MascotPose(
            ground = mid + Offset(0f, BODY_MID * k * scale),
            squash = -0.55f * e.pow(1.5f),
            tilt = lerpAngle(sway, toward, ease(e * 3f)),
            scale = scale,
            armLeft = 95f + 55f * sin(s * 32f), armRight = 95f + 55f * sin(s * 32f + 2f),
            stride = s * 40f,
            mood = Mood.Shock,
            alpha = 1f - ease((e - 0.85f) / 0.15f),
            time = s,
        )
    }

    private const val BODY_MID = 62f

    /** Nebula leaves a short smear of itself behind as it is pulled: the poses a moment earlier. */
    fun ghosts(scene: MascotScene, s: Float, exit: Float): List<MascotPose> {
        if (scene.family != IconFamily.Nebula || exit <= 0.05f) return emptyList()
        return (2 downTo 1).mapNotNull { i ->
            val e = exit - i * 0.09f
            if (e <= 0f) null
            else nebulaPull(scene, s - i * 0.03f, e).let { it.copy(alpha = it.alpha * GHOST_OPACITY / i) }
        }
    }

    private const val GHOST_OPACITY = 0.22f

    /**
     * How far below its resting place the name sits (px), for Gym: it rides on the mascot's hands
     * through the press. Zero for every other family.
     */
    fun wordDrop(family: IconFamily, mascot: LaunchMascot, s: Float, box: Float): Float {
        if (family != IconFamily.Gym) return 0f
        val tw = s - leadFor(family)
        val k = box / LaunchMascot.BOX
        val handTop = mascot.rig.top - 8f
        val sq = if (tw < 0f) GYM_CROUCH else gymSquash(tw)
        return sq * (LaunchMascot.GROUND - handTop) * k
    }

    /**
     * Everything in the scene that is not the mascot: the Molten sparks and dropped hammer, the
     * thrown Stealth lever, the Gym chalk, the Gem pluck and glint, the Aurora light trails.
     * Drawn under the mascot, in canvas pixels.
     */
    fun DrawScope.drawSceneEffects(scene: MascotScene, s: Float, exit: Float, sinceExit: Float) {
        val word = scene.word
        val box = scene.box
        val k = box / LaunchMascot.BOX
        val tw = s - leadFor(scene.family)
        when (scene.family) {
            IconFamily.Molten -> if (sinceExit >= 0f) {
                // The dropped hammer: falls from the fist and lies where it landed.
                val home = Offset(word.text.right + box * 0.42f, word.baseline)
                val q = ease(sinceExit / 0.22f)
                val from = home + Offset(-box * 0.3f, -box * 0.3f)
                val to = home + Offset(-box * 0.16f, -box * 0.04f)
                translate(from.x + (to.x - from.x) * q, from.y + (to.y - from.y) * q * q) {
                    scale(k, pivot = Offset.Zero) {
                        rotate(lerp(20f, 96f, q), pivot = Offset.Zero) { drawHammer(Offset(0f, -18f), 1f) }
                    }
                }
            } else STRIKES.forEach { t0 ->
                val u = (tw - t0) / 0.3f
                if (u in 0f..1f) {
                    val at = Offset(word.text.right - word.text.width * 0.04f, word.baseline - word.capHeight * 0.35f)
                    for (i in 0 until 9) {
                        val ang = (-150f + i * 26f + hash(i + t0 * 10f) * 18f) * (PI.toFloat() / 180f)
                        val r = box * (0.12f + 0.28f * u) * (0.6f + 0.6f * hash(i * 3.1f + t0))
                        val p = at + Offset(cos(ang) * r, sin(ang) * r + u * u * box * 0.15f)
                        drawLine(
                            MascotColors.Spark.copy(alpha = 1f - u),
                            start = p, end = p - Offset(cos(ang), sin(ang)) * box * 0.04f,
                            strokeWidth = box * 0.018f, cap = StrokeCap.Round,
                        )
                    }
                }
            }
            IconFamily.Gym -> {
                val u = (tw - 0.05f) / 0.6f
                if (u in 0f..1f) {
                    for (i in 0 until 14) {
                        val ang = (hash(i * 1.7f) * 360f) * (PI.toFloat() / 180f)
                        val r = box * (0.05f + 0.35f * u) * (0.5f + hash(i * 5.3f))
                        val p = Offset(word.text.center.x + (hash(i * 2.9f) - 0.5f) * word.text.width * 0.5f, word.baseline) +
                            Offset(cos(ang) * r, sin(ang) * r * 0.5f - u * box * 0.1f)
                        drawCircle(MascotColors.Chalk.copy(alpha = (1f - u) * CHALK_OPACITY), radius = box * (0.02f + 0.03f * u), center = p)
                    }
                }
            }
            // Star dust from the nebula spiralling into the hole alongside the mascot.
            IconFamily.Nebula -> if (exit > 0f) {
                val hole = Offset(word.layer.left + word.layer.width * 0.68f, word.layer.top + word.layer.height * 0.42f)
                for (i in 0 until 26) {
                    val start = hash(i * 3.3f) * 0.35f
                    val q = ((exit - start) / (1f - start)).coerceIn(0f, 1f)
                    if (q <= 0f || q >= 1f) continue
                    val r0 = box * (0.6f + 1.4f * hash(i * 1.9f))
                    val a = hash(i * 7.7f) * 2f * PI.toFloat() + 3f * PI.toFloat() * q * q
                    val r = r0 * (1f - ease(q))
                    val p = hole + Offset(cos(a) * r, sin(a) * r)
                    val col = if (i % 3 == 0) scene.palette.last() else scene.palette[scene.palette.size / 2]
                    drawCircle(col.copy(alpha = sin(PI.toFloat() * q)), radius = box * (0.012f + 0.02f * hash(i * 5.1f)), center = p)
                }
            }
            IconFamily.Stealth -> leverState(scene, s)?.let { lever ->
                translate(lever.at.x, lever.at.y) {
                    scale(k * 1.15f, pivot = Offset.Zero) {
                        rotate(lever.spin, pivot = Offset(0f, -20f)) { drawLever(lever.pulled, 1f) }
                    }
                }
            }
            IconFamily.Gem -> {
                // A pop where the crystal came away.
                val u = (tw - PLUCK) / 0.35f
                if (u in 0f..1f) {
                    val at = Offset(word.text.right - word.text.width * 0.03f, word.baseline - word.capHeight * 0.45f)
                    for (i in 0 until 5) {
                        val ang = (i * 72f + 20f) * (PI.toFloat() / 180f)
                        val p = at + Offset(cos(ang), sin(ang)) * box * 0.18f * ease(u)
                        star(p, k * 1.2f * (1f - u), MascotColors.Sparkle.copy(alpha = 1f - u))
                    }
                }
                // The held gem glints.
                if (tw > PLUCK + 0.3f) {
                    val hand = handWorld(scene, pose(scene, s, exit, sinceExit), side = -1f)
                    for (i in 0 until 3) {
                        val t = s * 2.2f + i * 0.33f
                        val f = t - kotlin.math.floor(t)
                        val p = hand + Offset((hash(i * 7.1f) - 0.5f) * box * 0.35f, -box * 0.12f - f * box * 0.18f)
                        star(p, k * 1.1f * sin(PI.toFloat() * f), MascotColors.Sparkle)
                    }
                }
            }
            IconFamily.Aurora -> if (tw > CONDUCT_FROM) {
                // Ribbons of the icon's light trailing from both hands through the last third of a second.
                val steps = 22
                for (side in listOf(-1f, 1f)) {
                    var prev: Offset? = null
                    for (j in 0..steps) {
                        val back = s - j * 0.022f
                        if (back - leadFor(scene.family) < CONDUCT_FROM) break
                        // Each point of the ribbon keeps rising after it leaves the hand, so the light
                        // streams up off the conductor like a curtain rather than tracing its arm.
                        val age = s - back
                        val p = handWorld(scene, pose(scene, back, exit, sinceExit), side) +
                            Offset(side * age * box * 0.35f, -age * box * 1.6f)
                        prev?.let { q ->
                            val f = j / steps.toFloat()
                            val col = lerpColor(scene.palette.last(), scene.palette[scene.palette.size / 2], f)
                            drawLine(col.copy(alpha = (1f - f) * TRAIL_GLOW), q, p, strokeWidth = box * 0.16f * (1f - f * 0.6f), cap = StrokeCap.Round)
                            drawLine(col.copy(alpha = 1f - f), q, p, strokeWidth = box * 0.05f * (1f - f * 0.7f), cap = StrokeCap.Round)
                        }
                        prev = p
                    }
                }
            }
            else -> Unit
        }
    }

    private const val TRAIL_GLOW = 0.25f

    /** Where a hand ([side] 1 = right, −1 = left) is on screen for [pose], following the rig's transforms. */
    private fun handWorld(scene: MascotScene, pose: MascotPose, side: Float): Offset {
        val rig = scene.mascot.rig
        val angle = if (side > 0f) pose.armRight else pose.armLeft
        val rad = Math.toRadians(angle.toDouble()).toFloat()
        val len = rig.armLen * pose.armReach
        var x = 100f + side * (rig.shoulder.x + len * sin(rad))
        var y = rig.shoulder.y + len * cos(rad)
        // Squash about the feet, then tilt about the body's middle, as drawMascot does.
        x = 100f + (x - 100f) * (1f + pose.squash * 0.6f)
        y = LaunchMascot.GROUND + (y - LaunchMascot.GROUND) * (1f - pose.squash)
        val t = Math.toRadians(pose.tilt.toDouble()).toFloat()
        val dx = x - 100f
        val dy = y - 120f
        x = 100f + dx * cos(t) - dy * sin(t)
        y = 120f + dx * sin(t) + dy * cos(t)
        val k = scene.box / LaunchMascot.BOX * pose.scale
        return Offset(pose.ground.x + (x - 100f) * k, pose.ground.y - pose.lift + (y - LaunchMascot.GROUND) * k)
    }

    private fun DrawScope.star(at: Offset, size: Float, color: Color) {
        if (size <= 0f) return
        translate(at.x, at.y) { scale(size, pivot = Offset.Zero) { drawPath(star(), color) } }
    }

    // ── shared beats ────────────────────────────────────────────────────────────────────────────

    /** Two hops from [from] to [to] across [ENTER], a squash on landing, and stillness after. */
    private fun arrive(from: Offset, to: Offset, s: Float, box: Float): MascotPose {
        val u = (s / ENTER).coerceIn(0f, 1f)
        val hops = 2f
        val q = (u * hops) % 1f
        val airborne = u < 1f
        val lift = if (airborne) box * 0.26f * 4f * q * (1f - q) else 0f
        val landed = s - ENTER
        val squash = when {
            airborne -> if (q > 0.85f || q < 0.1f) 0.12f else -0.08f
            landed < 0.35f -> 0.22f * exp(-landed * 10f) * cos(landed * 28f)
            else -> 0f
        }
        return MascotPose(
            ground = Offset(lerp(from.x, to.x, u), lerp(from.y, to.y, u)),
            lift = lift,
            squash = squash,
            armLeft = if (airborne) 60f else 12f,
            armRight = if (airborne) 60f else 12f,
        )
    }

    private val STRIKES = floatArrayOf(0f, 0.32f, 0.64f)

    /** The hammer arm: wind up, drop onto the name, rest; three times. */
    private fun hammerArm(tw: Float): Float {
        for (t0 in STRIKES) {
            if (tw in (t0 - 0.18f)..(t0 - 0.04f)) return lerp(30f, 165f, ease((tw - (t0 - 0.18f)) / 0.14f))
            if (tw in (t0 - 0.04f)..t0) return lerp(165f, 70f, (tw - (t0 - 0.04f)) / 0.04f)
            if (tw in t0..(t0 + 0.10f)) return 70f
        }
        return 30f
    }

    /** 1 at the instant of each hammer strike, decaying quickly. */
    private fun impact(tw: Float): Float =
        STRIKES.maxOf { t0 -> if (tw >= t0) exp(-(tw - t0) * 25f) else 0f }

    private const val GYM_CROUCH = 0.18f

    /** The press, as squash: crouched, driven up past neutral, settled at lockout. */
    private fun gymSquash(tw: Float): Float = when {
        tw < 0.05f -> GYM_CROUCH
        tw < 0.45f -> lerp(GYM_CROUCH, -0.05f, ease((tw - 0.05f) / 0.40f))
        tw < 0.65f -> lerp(-0.05f, 0f, ease((tw - 0.45f) / 0.20f))
        else -> 0f
    }

    private const val CHALK_OPACITY = 0.8f

    private fun ease(u: Float): Float {
        val x = u.coerceIn(0f, 1f)
        return x * x * (3f - 2f * x)
    }

    /** Interpolates degrees the short way round. */
    private fun lerpAngle(a: Float, b: Float, t: Float): Float {
        var d = (b - a) % 360f
        if (d > 180f) d -= 360f
        if (d < -180f) d += 360f
        return a + d * t
    }

    private fun hash(x: Float): Float {
        val v = sin(x * 127.1f + 311.7f) * 43758.547f
        return v - kotlin.math.floor(v)
    }
}
