package com.forge.app.ui.theme

import androidx.compose.ui.graphics.Color

/**
 * The launch mascots' own colours (`ui/launch/LaunchMascot.kt`). They are characters, not UI, so
 * they keep their colours whatever the accent or the chosen app icon; only the scene around them is
 * themed. Values match the approved sketches (Kettle, Mochi, Nori, Momo), 2026-09-26.
 */
object MascotColors {
    // Kettle: the red kettlebell.
    val KettleBody = Color(0xFFE23D3D)
    val KettleShade = Color(0xFFA62B2B)
    val KettleArm = Color(0xFFC22F2F)
    val KettleInk = Color(0xFF2A0E0E)
    val KettleCheek = Color(0xFFFFB3B3)
    val KettleHeart = Color(0xFFFFE3E3)
    val KettleTongue = Color(0xFFFF8F9A)

    // Mochi: the soft rice-cake blob with a sprout.
    val MochiBody = Color(0xFFF4ECDF)
    val MochiArm = Color(0xFFE3D5BF)
    val MochiLeg = Color(0xFFD6C6AE)
    val MochiStem = Color(0xFF7DB074)
    val MochiLeaf = Color(0xFF8CC084)

    // Nori: the rice ball in a seaweed strip.
    val NoriBody = Color(0xFFF6F3EC)
    val NoriGrain = Color(0xFFE4DDD0)
    val NoriSeaweed = Color(0xFF26332C)
    val NoriArm = Color(0xFFE6DFD1)
    val NoriLeg = Color(0xFFD4CCBB)

    // Momo: the peach with one leaf.
    val MomoBody = Color(0xFFF7A58F)
    val MomoCrease = Color(0xFFE4846E)
    val MomoArm = Color(0xFFEE937D)
    val MomoLeg = Color(0xFFDE806B)
    val MomoStem = Color(0xFF6E8F4E)
    val MomoLeaf = Color(0xFF86B05E)
    val MomoInk = Color(0xFF3A1A14)
    val MomoCheek = Color(0xFFEE6F62)
    val MomoTongue = Color(0xFFE8616A)

    // Shared by the rice-cake pair (Mochi, Nori).
    val SoftInk = Color(0xFF2B2320)
    val SoftCheek = Color(0xFFF4A3A8)
    val SoftTongue = Color(0xFFF27C88)
    val HeartRed = Color(0xFFE23D3D)

    // Mood extras and props.
    val Sparkle = Color(0xFFFFE08A)
    val Tear = Color(0xFF8FC7F2)
    val Snore = Color(0xFFBFB6AA)
    val Shadow = Color(0xFF000000)
    val Highlight = Color(0xFFFFFFFF)
    val HammerHandle = Color(0xFF8A5A36)
    val HammerHead = Color(0xFF5C6470)
    val HammerShine = Color(0xFFA9B2BE)
    val LeverBase = Color(0xFF3A3F46)
    val LeverKnob = Color(0xFFE23D3D)
    val Spark = Color(0xFFFFD27A)
    val Chalk = Color(0xFFF4F1EA)
}
