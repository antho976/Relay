package com.tally.app.ui.kit

import androidx.compose.foundation.background
import androidx.compose.foundation.clickable
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.shape.CircleShape
import androidx.compose.ui.draw.clip
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.semantics.Role
import androidx.compose.ui.semantics.contentDescription
import androidx.compose.ui.semantics.semantics
import androidx.compose.ui.unit.sp
import com.tally.app.ui.theme.ground
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.BoxWithConstraints
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.ColumnScope
import androidx.compose.foundation.layout.PaddingValues
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.RowScope
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.heightIn
import androidx.compose.foundation.layout.imePadding
import androidx.compose.foundation.layout.navigationBarsPadding
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.statusBarsPadding
import androidx.compose.foundation.lazy.LazyColumn
import androidx.compose.foundation.lazy.LazyListScope
import androidx.compose.foundation.lazy.LazyListState
import androidx.compose.foundation.lazy.rememberLazyListState
import androidx.compose.foundation.rememberScrollState
import androidx.compose.foundation.verticalScroll
import androidx.compose.runtime.Composable
import androidx.compose.runtime.DisposableEffect
import androidx.compose.runtime.mutableStateMapOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.staticCompositionLocalOf
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.layout.onSizeChanged
import androidx.compose.ui.platform.LocalDensity
import androidx.compose.ui.unit.Dp
import androidx.compose.ui.unit.dp
import com.tally.app.ui.theme.Tally

/** Relay's gutter: 16dp on a phone, 24dp from a tablet's width. */
fun gutter(width: Dp): Dp = if (width < 600.dp) 16.dp else 24.dp

/** Room under a page's last row; the bottom bar sits below the page, not over it. */
val ADD_CLEARANCE = 28.dp

/**
 * Where the one toast must clear: each page's pinned footer (an editor's keypad and Save) reports
 * its height here while it is on screen, so a notice never lands on a key. Keyed per footer, so a
 * page leaving during a transition does not wipe the one arriving.
 */
class ToastClearance {
    private val footers = mutableStateMapOf<Any, Dp>()

    /** The tallest footer showing, or zero on a page without one. */
    val bottom: Dp get() = footers.values.maxOrNull() ?: 0.dp

    internal fun set(key: Any, height: Dp) { footers[key] = height }

    internal fun clear(key: Any) { footers.remove(key) }
}

val LocalToastClearance = staticCompositionLocalOf { ToastClearance() }

/**
 * A space page's body, as Relay's source pages: a readable column that scrolls, the page's title
 * and its one-line context over the body, the page's keys at the title's right.
 */
@Composable
fun SpacePage(
    title: String,
    modifier: Modifier = Modifier,
    context: String? = null,
    contextColor: androidx.compose.ui.graphics.Color = Tally.colors.ink3,
    actions: @Composable RowScope.() -> Unit = {},
    bottom: Dp = ADD_CLEARANCE,
    body: @Composable ColumnScope.() -> Unit,
) {
    BoxWithConstraints(modifier.fillMaxSize()) {
        val side = gutter(maxWidth)
        Column(Modifier.fillMaxSize().verticalScroll(rememberScrollState()), horizontalAlignment = Alignment.CenterHorizontally) {
            Column(
                Modifier.column().fillMaxWidth().padding(horizontal = side).padding(top = 14.dp, bottom = bottom),
                verticalArrangement = Arrangement.spacedBy(12.dp),
            ) {
                PageTitle(title, context, contextColor, actions)
                body()
            }
        }
    }
}

/** The same page as a lazy list, for pages of many rows (entries, a category's history). */
@Composable
fun SpaceList(
    title: String,
    modifier: Modifier = Modifier,
    context: String? = null,
    state: LazyListState = rememberLazyListState(),
    actions: @Composable RowScope.() -> Unit = {},
    header: (@Composable ColumnScope.() -> Unit)? = null,
    bottom: Dp = ADD_CLEARANCE,
    showTitle: Boolean = true,
    content: LazyListScope.() -> Unit,
) {
    BoxWithConstraints(modifier.fillMaxSize()) {
        val side = gutter(maxWidth)
        LazyColumn(
            Modifier.fillMaxSize(),
            state = state,
            contentPadding = PaddingValues(start = side, end = side, top = 14.dp, bottom = bottom),
            horizontalAlignment = Alignment.CenterHorizontally,
        ) {
            item(key = "page-title") {
                Column(Modifier.column().fillMaxWidth(), verticalArrangement = Arrangement.spacedBy(12.dp)) {
                    if (showTitle) PageTitle(title, context, Tally.colors.ink3, actions)
                    header?.invoke(this)
                    Gap(4.dp)
                }
            }
            content()
        }
    }
}

@Composable
private fun PageTitle(title: String, context: String?, contextColor: androidx.compose.ui.graphics.Color, actions: @Composable RowScope.() -> Unit) {
    Row(verticalAlignment = Alignment.CenterVertically, horizontalArrangement = Arrangement.spacedBy(6.dp)) {
        Column(Modifier.weight(1f), verticalArrangement = Arrangement.spacedBy(3.dp)) {
            T(title, style = Tally.type.heading, color = Tally.colors.ink, maxLines = 2)
            context?.let { T(it, style = Tally.type.caption, color = contextColor, maxLines = 3) }
        }
        actions()
    }
}

/** A frosted round key, as the C mockups' back and header keys: 44dp, glass, an icon in ink. */
@Composable
fun RoundKey(glyph: String, onClick: () -> Unit, modifier: Modifier = Modifier, label: String? = null, filled: Boolean = false, tint: Color? = null) {
    val c = Tally.colors
    Box(
        modifier.size(44.dp).clip(CircleShape).background(if (filled) c.ink else c.slab)
            .clickable(role = Role.Button, onClick = onClick)
            .then(if (label != null) Modifier.semantics { contentDescription = label } else Modifier),
        contentAlignment = Alignment.Center,
    ) {
        Glyph(glyph, 19.dp, tint ?: if (filled) c.onInk else c.ink)
    }
}

/** A page's own bar: a round back key, the title, then its keys. */
@Composable
fun PageBar(title: String, onBack: () -> Unit, subtitle: String? = null, actions: @Composable RowScope.() -> Unit = {}) {
    val c = Tally.colors
    Row(
        Modifier.fillMaxWidth().statusBarsPadding().heightIn(min = 64.dp).padding(horizontal = 12.dp),
        verticalAlignment = Alignment.CenterVertically,
        horizontalArrangement = Arrangement.spacedBy(10.dp),
    ) {
        RoundKey("chevron-left", onBack, label = "Back")
        Column(Modifier.weight(1f)) {
            T(title, style = Tally.type.title.copy(fontSize = 20.sp, lineHeight = 25.sp), color = c.ink, maxLines = 1)
            subtitle?.let { T(it, style = Tally.type.caption, color = c.ink3, maxLines = 1) }
        }
        actions()
    }
}

/**
 * A detail page: its bar, then a readable column that scrolls, and an optional bar of keys pinned at
 * the bottom over the keyboard (an editor's Save).
 */
@Composable
fun DetailPage(
    title: String,
    onBack: () -> Unit,
    subtitle: String? = null,
    actions: @Composable RowScope.() -> Unit = {},
    footer: (@Composable ColumnScope.() -> Unit)? = null,
    spacing: Dp = 12.dp,
    body: @Composable ColumnScope.() -> Unit,
) {
    val c = Tally.colors
    Column(Modifier.fillMaxSize().ground(c).imePadding()) {
        PageBar(title, onBack, subtitle, actions)
        BoxWithConstraints(Modifier.fillMaxWidth().weight(1f)) {
            val side = gutter(maxWidth)
            Column(Modifier.fillMaxSize().verticalScroll(rememberScrollState()), horizontalAlignment = Alignment.CenterHorizontally) {
                Column(
                    Modifier.column().fillMaxWidth().padding(horizontal = side).padding(top = 6.dp, bottom = 24.dp),
                    verticalArrangement = Arrangement.spacedBy(spacing),
                    content = body,
                )
            }
        }
        footer?.let { f ->
            val clearance = LocalToastClearance.current
            val density = LocalDensity.current
            val key = remember { Any() }
            DisposableEffect(clearance, key) { onDispose { clearance.clear(key) } }
            Column(
                Modifier.fillMaxWidth().clip(Radii.sheet).background(c.console).navigationBarsPadding()
                    .onSizeChanged { clearance.set(key, with(density) { it.height.toDp() }) },
            ) {
                Box(Modifier.fillMaxWidth(), contentAlignment = Alignment.Center) {
                    Column(Modifier.column().fillMaxWidth().padding(horizontal = 16.dp, vertical = 14.dp), verticalArrangement = Arrangement.spacedBy(10.dp), content = f)
                }
            }
        } ?: Box(Modifier.navigationBarsPadding())
    }
}
