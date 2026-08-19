package com.forge.app.ui.theme

import androidx.compose.ui.graphics.Color

// ── Pearl (dark default) ──────────────────────────────────────────────────────
//
// WARM near-black, 2026-08-16. Every value below was blue-leaning (B channel highest); the page read
// as neutral-cool, which is the temperature of a banking app and the single biggest reason Home felt
// lifeless with nothing wrong in the layout. The channel order is now inverted (R highest) at the
// same luminance, so nothing about contrast moved but the whole app sits under a warm light.
val PearlBackground  = Color(0xFF110F0C)   // was #0E0E11
val PearlSurface     = Color(0xFF1A1613)   // was #15161B (sheets)
val PearlSurfaceVar  = Color(0xFF221C16)   // was #1C1D24 (interactive tile fill)
val PearlOutline     = Color(0xFF38302A)   // was #2E2E38
val PearlOnBg        = Color(0xFFF2EFEA)   // was #EEEEF2 — 16.77:1 on the warm bg
val PearlMuted       = Color(0xFFBFB6AA)   // was #B4B4C2 — 9.61:1 full, 5.20:1 @0.7, 4.65:1 @0.65 (the floor still holds; 0.60 still fails at 4.10:1)

val PearlGradTop     = Color(0xFF17120E)   // was #131318
val PearlGradBottom  = Color(0xFF0A0806)   // was #090909

// ── Indigo (light alternate) ──────────────────────────────────────────────────
val IndigoBackground = Color(0xFFF4F4F8)
val IndigoSurface    = Color(0xFFFFFFFF)
val IndigoSurfaceVar = Color(0xFFEAEAF0)
val IndigoOutline    = Color(0xFFDCDCE8)
val IndigoOnBg       = Color(0xFF111118)
val IndigoMuted      = Color(0xFF6B6B7A)

val IndigoGradTop    = Color(0xFFF8F8FD)
val IndigoGradBottom = Color(0xFFEBEBF2)

// ── Accent presets (map to secondary token) ───────────────────────────────────
/**
 * The default accent, 2026-08-16. Two reasons it replaced Navy.
 *
 * **Heat.** Navy on a near-black page is the deadest pairing available: it never reads as energy, so
 * every place the accent appeared was a colour the eye skipped. Ember is warm at the same restraint —
 * burnt, not neon — and it is spent in FEW places at LARGE size rather than many places at postage-
 * stamp size, which was the old failure.
 *
 * **Hue.** It is deliberately yellower and more saturated than a terracotta or clay orange (Antho,
 * 2026-08-16: a softer #C9662E "feels a bit too Claude like"). Molten metal, not pottery — which suits
 * an app called Forge better anyway. Do not drift it back toward the desaturated red-orange range.
 *
 * **Contrast.** Accent-as-text was a documented AA failure — Navy measures 2.34:1 on the page. Ember
 * measures **5.84:1** either way round, so `action →` links, accent glyphs AND dark-on-accent fills
 * all clear AA. That last one is why [pearlColorScheme]'s onPrimary threshold moved.
 */
val AccentEmber = Color(0xFFD4761F)   // default dark
val AccentNavy  = Color(0xFF3D4F73)
val AccentRed   = Color(0xFF8B3535)
val AccentOlive = Color(0xFF4D6040)
val AccentGold  = Color(0xFF7A6435)

val AccentIndigoDefault = Color(0xFFCFCFCF)  // default light

// ── Semantic ──────────────────────────────────────────────────────────────────
val ForgeSuccess = Color(0xFF4CAF7D)
val ForgeWarning = Color(0xFFCFAB47)
val ForgeError   = Color(0xFFBF4040)

/** Bright gold reserved for PR moments (PR star, PR weight). */
val ForgePrGold  = Color(0xFFE3B341)
/** Visible green for the "△ LAST" beat-your-last indicator. */
val ForgeLastGreen = Color(0xFF5BC873)
