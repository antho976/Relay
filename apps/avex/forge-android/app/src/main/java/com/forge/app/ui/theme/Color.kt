package com.forge.app.ui.theme

import androidx.compose.ui.graphics.Color

// ── Pearl (dark default) ──────────────────────────────────────────────────────
val PearlBackground  = Color(0xFF0E0E11)
// Surfaces sit as a quiet, faintly-cool lift off the near-black bg rather than flat pale-grey slabs:
// darker + a hair of blue (B channel highest) so the "boxes" read as intentional dark cards, not grey.
val PearlSurface     = Color(0xFF15161B)   // was #18181C
val PearlSurfaceVar  = Color(0xFF1C1D24)   // was #222228 (the card/"grey box" the content sits on)
val PearlOutline     = Color(0xFF2E2E38)
val PearlOnBg        = Color(0xFFEEEEF2)
val PearlMuted       = Color(0xFFB4B4C2)   // A6: brightened so muted text — incl. the .65–.7 alpha captions — clears WCAG-AA 4.5:1 on PearlBackground (old #A6A6B6 fell to ~4.4:1 once alpha-dimmed; full tone now ~9.4:1, alpha-0.7 ~5.1:1)

val PearlGradTop     = Color(0xFF131318)
val PearlGradBottom  = Color(0xFF090909)

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
val AccentNavy  = Color(0xFF3D4F73)   // default dark
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
