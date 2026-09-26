package com.forge.app.ui.launch

import androidx.compose.foundation.Canvas
import androidx.compose.runtime.Composable
import androidx.compose.runtime.remember
import androidx.compose.ui.Modifier
import androidx.compose.ui.geometry.Offset
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.layout.onGloballyPositioned
import androidx.compose.ui.layout.positionInRoot
import androidx.compose.ui.platform.LocalDensity
import androidx.compose.ui.unit.dp
import com.forge.app.appicon.IconFamily
import com.forge.app.ui.launch.MascotChoreography.drawSceneEffects
import kotlin.random.Random

/** The on-screen size of a mascot's 200-unit rig box. */
private val MASCOT_BOX = 96.dp

/** Every cold launch brings a mascot; which of the four is a fresh roll each time. */
fun rollLaunchMascot(random: Random = Random.Default): LaunchMascot = LaunchMascot.entries.random(random)

/** How far the name sits below its place (px) this frame, for the families where the mascot moves it. */
@Composable
fun rememberMascotWordDrop(mascot: LaunchMascot?, family: IconFamily, clock: () -> Float): () -> Float {
    val box = with(LocalDensity.current) { MASCOT_BOX.toPx() }
    return remember(mascot, family, box) {
        if (mascot == null) { { 0f } } else { { MascotChoreography.wordDrop(family, mascot, clock(), box) } }
    }
}

/**
 * The mascot layer of the launch intro: draws [mascot] playing [family]'s routine over the plate,
 * with the icon's [palette] for the scene's light. Every input is a deferred read, so it redraws
 * each frame without recomposing. [word] is the name's frame in ROOT coordinates (from
 * `AvexWordmark`); nothing is drawn until it is known. [exitStart] is the clock time the intro's
 * death began, or null before it.
 */
@Composable
fun MascotStage(
    mascot: LaunchMascot,
    family: IconFamily,
    palette: List<Color>,
    clock: () -> Float,
    exit: () -> Float,
    exitStart: () -> Float?,
    word: () -> WordFrame?,
    modifier: Modifier = Modifier,
) {
    val box = with(LocalDensity.current) { MASCOT_BOX.toPx() }
    val origin = remember { FloatArray(2) }
    Canvas(
        modifier.onGloballyPositioned {
            val p = it.positionInRoot()
            origin[0] = p.x
            origin[1] = p.y
        }
    ) {
        val root = word() ?: return@Canvas
        val shift = Offset(-origin[0], -origin[1])
        val local = WordFrame(
            text = root.text.translate(shift),
            baseline = root.baseline + shift.y,
            capTop = root.capTop + shift.y,
            layer = root.layer.translate(shift),
            aLeft = root.aLeft + shift.x,
            aRight = root.aRight + shift.x,
        )
        val scene = MascotScene(family, mascot, local, box, size, palette)
        val s = clock()
        val e = exit()
        val sinceExit = exitStart()?.let { s - it } ?: -1f
        drawSceneEffects(scene, s, e, sinceExit)
        MascotChoreography.ghosts(scene, s, e).forEach { drawMascot(mascot, it, box) }
        drawMascot(mascot, MascotChoreography.pose(scene, s, e, sinceExit), box)
    }
}
