package com.forge.app.ui.common

import androidx.compose.foundation.lazy.LazyItemScope
import androidx.compose.ui.Modifier
import com.forge.app.ui.theme.ForgeMotion

/**
 * Shared lazy-list item animation. Rows fade + ease into place on insert, fade on removal, and
 * glide rather than snap when reordered. Routed through [ForgeMotion] so list motion speaks the
 * app's one motion language and honors the system "remove animations" setting (the Phase-0
 * reduced-motion gate) — a bare Modifier.animateItem() uses default springs that ignore it.
 *
 * Use inside a LazyColumn/LazyRow item as the row's modifier: `modifier = forgeItemMotion()`.
 */
fun LazyItemScope.forgeItemMotion(): Modifier =
    Modifier.animateItem(
        fadeInSpec = ForgeMotion.enterTween(),
        placementSpec = ForgeMotion.snappy(),
        fadeOutSpec = ForgeMotion.exitTween()
    )
