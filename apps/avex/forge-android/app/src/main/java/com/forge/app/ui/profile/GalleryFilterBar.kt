@file:OptIn(androidx.compose.foundation.layout.ExperimentalLayoutApi::class, androidx.compose.material3.ExperimentalMaterial3Api::class)

package com.forge.app.ui.profile

import androidx.compose.animation.AnimatedVisibility
import androidx.compose.animation.expandVertically
import androidx.compose.animation.fadeIn
import androidx.compose.animation.fadeOut
import androidx.compose.animation.shrinkVertically
import androidx.compose.foundation.horizontalScroll
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.FlowRow
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.rememberScrollState
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.filled.Close
import androidx.compose.material.icons.filled.Search
import androidx.compose.material3.Icon
import androidx.compose.material3.IconButton
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.OutlinedTextField
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.ui.Modifier
import androidx.compose.ui.focus.FocusRequester
import androidx.compose.ui.focus.focusRequester
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.unit.dp
import com.forge.app.domain.photo.PhotoPose
import com.forge.app.domain.photo.PhotoTag
import com.forge.app.program.MuscleGroup
import com.forge.app.ui.common.SegmentPill
import com.forge.app.ui.common.bounceClick
import com.forge.app.ui.theme.ForgeMotion

/**
 * The library's browse bar: an always-open search field, the pose lens, the tool chips, and — behind
 * the Filters chip — the three tag rails (WHEN, MUSCLE, TAGS) plus sort and density.
 *
 * **Everything here draws at every count, including zero.** It used to be gated: search appeared at
 * four photos, the pose lens at two distinct poses, the muscle and tag rails only once something
 * carried those tags. The reasoning was that a control which can only ever return the same grid is
 * an affordance that does nothing (§4.5) — but applied to a photo library it produced a first-run
 * screen with no search, no lens and no filters, which does not read as a restrained gallery. It
 * reads as an unfinished one. A gallery states its own shape before it has anything in it, the same
 * way the progress band draws its ghost frames rather than waiting for a photo to justify itself.
 *
 * Pose and muscle are FIXED vocabularies, so their rails show the whole set rather than only the
 * values in use: the rail then doubles as the answer to "what can I even tag a shot with", which is
 * the question a new library actually raises. Tags are user-invented, so that rail names its own
 * emptiness instead of drawing a row of nothing.
 *
 * **A narrowed grid always says what is narrowing it.** With the panel shut, the active facet values
 * ride above the grid as removable chips, so a filter you set three scrolls ago can never quietly
 * eat your library. With the panel open that row would repeat the rails' own selection, so it goes.
 */
@Composable
internal fun GalleryFilterBar(
    knownTags: List<String>,
    tools: GalleryTools,
    onStartCompare: () -> Unit,
    onBg: Color, muted: Color, accent: Color, outline: Color
) {
    val filter = tools.filter

    Column(Modifier.fillMaxWidth()) {
        GallerySearchBar(filter.query, tools::onQueryChange, focusRequester = tools.searchFocus)
        Spacer(Modifier.height(10.dp))

        Row(
            Modifier.fillMaxWidth().horizontalScroll(rememberScrollState()),
            horizontalArrangement = Arrangement.spacedBy(8.dp)
        ) {
            SegmentPill("All", selected = filter.pose == null, onClick = { tools.onPoseChange(null) }, accent, onBg, muted, outline)
            PhotoPose.entries.forEach { p ->
                SegmentPill(p.label, selected = filter.pose == p, onClick = { tools.onPoseChange(p) }, accent, onBg, muted, outline)
            }
        }
        Spacer(Modifier.height(10.dp))

        Row(
            Modifier.fillMaxWidth().horizontalScroll(rememberScrollState()),
            horizontalArrangement = Arrangement.spacedBy(8.dp)
        ) {
            GalleryChip(
                "Filters",
                selected = tools.filtersOpen || filter.activeFacets > 0,
                trailing = filter.activeFacets.takeIf { it > 0 }?.toString()
            ) { tools.onToggleFilters() }
            GalleryChip("Compare", selected = false) { onStartCompare() }
        }

        AnimatedVisibility(
            visible = tools.filtersOpen,
            enter = fadeIn(ForgeMotion.enterTween()) + expandVertically(ForgeMotion.enterTween()),
            exit = fadeOut(ForgeMotion.exitTween()) + shrinkVertically(ForgeMotion.exitTween())
        ) {
            Column(Modifier.fillMaxWidth()) {
                Spacer(Modifier.height(14.dp))
                FacetRail("When", muted) {
                    GalleryRange.entries.forEach { r ->
                        GalleryChip(r.label, selected = r == filter.range) { tools.onRangeChange(r) }
                    }
                }
                Spacer(Modifier.height(14.dp))
                FacetRail("Muscle", muted) {
                    MuscleGroup.entries.forEach { m ->
                        GalleryChip(m.displayName, selected = m.code in filter.muscles) { tools.onToggleMuscle(m.code) }
                    }
                }
                Spacer(Modifier.height(14.dp))
                FacetRail("Tags", muted) {
                    if (knownTags.isEmpty()) {
                        // The one rail whose vocabulary you write yourself, so it says where that
                        // happens rather than drawing an empty row (§12: no undrawn state).
                        Text(
                            "Tag a photo in the viewer and it lands here.",
                            style = MaterialTheme.typography.bodySmall, color = muted.copy(alpha = 0.7f)
                        )
                    } else {
                        knownTags.forEach { t ->
                            GalleryChip(PhotoTag.display(t), selected = t in filter.tags) { tools.onToggleTag(t) }
                        }
                    }
                }
                // How the grid is laid out, not what is in it. These lived on the top row until the
                // four chips there ran past the screen edge and made two presentation switches look
                // as load-bearing as the filters beside them.
                Spacer(Modifier.height(14.dp))
                FacetRail("View", muted) {
                    GalleryChip(filter.sort.label, selected = false) { tools.onToggleSort() }
                    GalleryChip("${filter.columns} across", selected = false) { tools.onCycleColumns() }
                }
            }
        }

        // The standing summary of what is hiding photos right now (panel shut only).
        if (!tools.filtersOpen && filter.activeFacets > 0) {
            Spacer(Modifier.height(12.dp))
            ActiveFacetChips(tools)
        }
    }
}

/** One labeled rail of chips inside the filter panel: a mono label over a wrapping chip row. */
@Composable
private fun FacetRail(label: String, muted: Color, chips: @Composable () -> Unit) {
    Text(label.uppercase(), style = MaterialTheme.typography.labelMedium, color = muted)
    Spacer(Modifier.height(8.dp))
    FlowRow(
        Modifier.fillMaxWidth(),
        horizontalArrangement = Arrangement.spacedBy(8.dp),
        verticalArrangement = Arrangement.spacedBy(8.dp)
    ) { chips() }
}

/**
 * Every active facet value as its own removable chip, plus one `Clear all` when several are on.
 * Tapping a chip drops that value alone, which is the difference between a filter you can steer and
 * one you can only switch off wholesale.
 */
@Composable
private fun ActiveFacetChips(tools: GalleryTools) {
    val filter = tools.filter
    val accent = MaterialTheme.colorScheme.primary
    FlowRow(
        Modifier.fillMaxWidth(),
        horizontalArrangement = Arrangement.spacedBy(8.dp),
        verticalArrangement = Arrangement.spacedBy(8.dp)
    ) {
        if (filter.range != GalleryRange.ALL) {
            GalleryChip(filter.range.label, selected = true, trailing = "✕") { tools.onRangeChange(GalleryRange.ALL) }
        }
        filter.pose?.let { p -> GalleryChip(p.label, selected = true, trailing = "✕") { tools.onPoseChange(null) } }
        filter.muscles.forEach { code ->
            val name = facetMuscleName(code)
            GalleryChip(name, selected = true, trailing = "✕") { tools.onToggleMuscle(code) }
        }
        filter.tags.forEach { t ->
            GalleryChip(PhotoTag.display(t), selected = true, trailing = "✕") { tools.onToggleTag(t) }
        }
        if (filter.activeFacets > 1) {
            Text(
                "Clear all",
                style = MaterialTheme.typography.labelMedium, color = accent,
                modifier = Modifier.bounceClick { tools.onClear() }.padding(horizontal = 6.dp, vertical = 6.dp)
            )
        }
    }
}

private fun facetMuscleName(code: String): String =
    MuscleGroup.entries.firstOrNull { it.code == code }?.displayName ?: code

/**
 * The search field. Filled rather than outlined — the standard phone-search look (§13), which is
 * what makes the page read as a library rather than as a form.
 *
 * It stands ALWAYS OPEN once the library is worth searching. It used to hide behind a "Search" chip,
 * which is a tool-drawer idiom: a photo library's search is the thing you reach for first, and a
 * gallery that makes you find its search does not read as a gallery. The placeholder names what it
 * matches, so the fields it covers are discoverable without a caption above the bar.
 */
@Composable
internal fun GallerySearchBar(
    query: String,
    onQueryChange: (String) -> Unit,
    modifier: Modifier = Modifier,
    focusRequester: FocusRequester? = null
) {
    OutlinedTextField(
        value = query,
        onValueChange = onQueryChange,
        modifier = modifier.fillMaxWidth().let { if (focusRequester != null) it.focusRequester(focusRequester) else it },
        placeholder = { Text("Title, muscle, tag, pose or date…", maxLines = 1) },
        leadingIcon = { Icon(Icons.Filled.Search, contentDescription = null) },
        colors = bodyLogFieldColors(),
        trailingIcon = {
            if (query.isNotEmpty()) IconButton(onClick = { onQueryChange("") }) {
                Icon(Icons.Filled.Close, contentDescription = "Clear search")
            }
        },
        singleLine = true,
        shape = RoundedCornerShape(12.dp)
    )
}
