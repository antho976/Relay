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
 * The default accent, 2026-08-23. It was Ember from 2026-08-16, and Navy before that; the reasoning
 * that moved it off Navy still governs, so it is kept here.
 *
 * **Heat.** Navy on a near-black page is the deadest pairing available: it never reads as energy, so
 * every place the accent appeared was a colour the eye skipped. A warm accent, spent in FEW places at
 * LARGE size rather than many at postage-stamp size, was the fix.
 *
 * **Contrast is the constraint, not a nicety.** Accent-as-text — `action →` links, accent glyphs —
 * is a real pattern in this app, so the DEFAULT has to clear AA on the page or every new install
 * ships failing text. Navy measured 2.34:1 and the old deep red `#8B3535` 2.42:1; both failed. This
 * red measures **4.53:1** on Pearl, and at luminance 0.198 it sits above [pearlColorScheme]'s 0.18
 * onPrimary threshold, so dark-on-accent fills clear AA too (4.53:1). That threshold and this value
 * move together — lower this colour and filled-primary controls go same-on-same.
 *
 * The alternates below do NOT clear AA as text and are not expected to; §14 and `SETTLED.md` scope
 * that guarantee to the default only.
 */
val AccentRed   = Color(0xFFE23D3D)   // default dark
/**
 * The former default (2026-08-16 → 2026-08-23). Deliberately yellower and more saturated than a
 * terracotta or clay orange (Antho, 2026-08-16: a softer #C9662E "feels a bit too Claude like") —
 * molten metal, not pottery. Do not drift it back toward the desaturated red-orange range.
 */
val AccentEmber = Color(0xFFD4761F)
val AccentNavy  = Color(0xFF3D4F73)
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
