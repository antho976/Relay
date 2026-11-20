package com.tally.app.ui.banks

import androidx.compose.foundation.background
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.layout.offset
import androidx.compose.foundation.shape.CircleShape
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.runtime.Composable
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.clip
import androidx.compose.ui.draw.rotate
import androidx.compose.ui.draw.shadow
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.unit.Dp
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp
import com.tally.app.ui.kit.Glyph
import com.tally.app.ui.kit.Key
import com.tally.app.ui.kit.KeyKind
import com.tally.app.ui.kit.Postcard
import com.tally.app.ui.kit.Scene
import com.tally.app.ui.kit.T
import com.tally.app.ui.theme.Fonts
import com.tally.app.ui.theme.Tally

/** A bank's mark: its initial on its colour, never its logo. */
@Composable
fun BankMark(name: String, modifier: Modifier = Modifier, size: Dp = 42.dp) {
    val (fill, letter) = bankLook(name)
    Box(modifier.size(size).clip(RoundedCornerShape(size * .33f)).background(fill), contentAlignment = Alignment.Center) {
        T(letter, style = Tally.type.brand.copy(fontFamily = Fonts.Sora, fontSize = (size.value * .42f).sp), color = Color.White, weight = FontWeight.SemiBold)
    }
}

fun bankLook(name: String): Pair<Color, String> {
    val n = name.lowercase()
    return when {
        "desjardins" in n -> Color(0xFF187A48) to "D"
        "wealthsimple" in n -> Color(0xFF111111) to "W"
        "rbc" in n || "royal" in n -> Color(0xFF1F4FA8) to "R"
        "td" == n.trim() || "toronto" in n -> Color(0xFF2C7A2C) to "TD"
        "national" in n -> Color(0xFFB3261E) to "N"
        "bmo" in n -> Color(0xFF1E6BB8) to "B"
        "scotia" in n -> Color(0xFFC2452D) to "S"
        "tangerine" in n -> Color(0xFFD9661F) to "T"
        else -> Color(0xFF4F5BB8) to (name.trim().firstOrNull()?.uppercase() ?: "?")
    }
}

/**
 * The Accounts page's way in to bank connections: a postcard of the two banks linked, a line on
 * what it saves, and one key. Once a bank is connected it reads as the connections' state instead.
 */
@Composable
fun ConnectBanksCard(onConnect: () -> Unit, modifier: Modifier = Modifier, connected: List<String> = emptyList(), lastSync: String? = null) {
    Postcard(Scene.BANKS, modifier, sceneHeight = 112.dp, onClick = if (connected.isNotEmpty()) onConnect else null) {
        T(if (connected.isEmpty()) "Stop importing CSVs" else "Connected: " + connected.joinToString(", "), style = Tally.type.title.copy(fontSize = 19.sp), color = Color(0xFF1A1A2E))
        T(
            if (connected.isEmpty()) "Connect Desjardins and Wealthsimple once. New purchases land in Tally on their own, already sorted."
            else lastSync ?: "Your server checks for new purchases every few hours.",
            style = Tally.type.body.copy(fontSize = 14.sp), color = Color(0xFF4A4A63),
        )
        if (connected.isEmpty()) Key("Connect a bank", onConnect, Modifier.fillMaxWidth(), kind = KeyKind.Primary, glyph = "link")
    }
}
