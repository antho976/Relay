package com.forge.app.ui.profile

import androidx.activity.compose.LocalActivity
import androidx.biometric.BiometricPrompt
import androidx.compose.foundation.background
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.BoxScope
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.aspectRatio
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.material3.Icon
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.rememberUpdatedState
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.clip
import androidx.compose.ui.graphics.Brush
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.semantics.clearAndSetSemantics
import androidx.compose.ui.semantics.contentDescription
import androidx.compose.ui.semantics.heading
import androidx.compose.ui.semantics.semantics
import androidx.compose.ui.unit.dp
import androidx.fragment.app.FragmentActivity
import com.forge.app.data.repo.ProgressPhoto
import com.forge.app.domain.photo.PhotoPose
import com.forge.app.domain.units.WeightUnit
import com.forge.app.domain.units.formatWeight
import com.forge.app.domain.units.formatWeightDelta
import com.forge.app.security.BiometricAuthenticator
import com.forge.app.ui.common.OpenLabel
import com.forge.app.ui.common.OpenPrimaryButton
import com.forge.app.ui.common.OpenSectionHead
import com.forge.app.ui.common.OpenSmallButton
import com.forge.app.ui.common.bounceClick
import com.forge.app.ui.common.currentLocale
import com.forge.app.ui.theme.OpenIcons
import com.forge.app.ui.theme.OpenType
import com.forge.app.ui.theme.ProfileColors
import java.time.Instant
import java.time.LocalDate
import java.time.ZoneId
import java.time.format.DateTimeFormatter
import java.time.format.TextStyle
import kotlin.math.abs

/*
 * Photos (ph): the comparison leads, the camera is the one action, and the gallery runs by month,
 * Front, Side and Back. Every photo opens the full gallery, where comparing, albums, notes and the
 * shareable before-and-after card live. Locked (lk4), the tab says how many photos there are and
 * draws empty frames where they go: no photo is composed until the session is unlocked.
 */

private val PHOTO_SHAPE = RoundedCornerShape(4.dp)

/** Months the tab shows before handing the rest to the full gallery. */
private const val MONTHS_SHOWN = 6

@Composable
internal fun ProfilePhotosTab(
    photos: List<ProgressPhoto>,
    unit: WeightUnit,
    today: LocalDate,
    actions: ProfileActions,
    photo: @Composable (ProgressPhoto, Modifier) -> Unit,
) {
    val ink = MaterialTheme.colorScheme.onBackground
    val locale = currentLocale()
    val zone = ZoneId.systemDefault()
    val dateFmt = remember(locale) { DateTimeFormatter.ofPattern("MMM d, yyyy", locale) }
    val dayFmt = remember(locale) { DateTimeFormatter.ofPattern("MMM d", locale) }
    fun dateOf(p: ProgressPhoto) = Instant.ofEpochMilli(p.takenAtMs).atZone(zone).toLocalDate()

    // ── Then and now ──
    val pair = remember(photos) { thenAndNow(photos, zone) }
    OpenSectionHead(
        "Then and now",
        action = if (photos.size >= 2) "Compare" else "Gallery",
        actionLabel = if (photos.size >= 2) "Compare photos in the gallery" else "Open the gallery",
        onAction = actions.onOpenGallery
    )
    Spacer(Modifier.height(14.dp))
    Row(Modifier.fillMaxWidth(), horizontalArrangement = Arrangement.spacedBy(10.dp)) {
        val then = pair?.then ?: photos.firstOrNull()
        val now = pair?.now
        listOf(then, now).forEach { p ->
            PhotoFrame(
                p, photo, actions.onOpenGallery,
                caption = p?.let { dateOf(it).format(dateFmt) },
                sub = p?.weightLb?.let { formatWeight(it, unit) },
                modifier = Modifier.weight(1f).aspectRatio(177f / 236f)
            )
        }
    }
    Spacer(Modifier.height(12.dp))
    Row(Modifier.fillMaxWidth(), horizontalArrangement = Arrangement.SpaceBetween, verticalAlignment = Alignment.CenterVertically) {
        if (pair != null) {
            OpenLabel(
                listOfNotNull(pair.pose?.label, pair.apart).joinToString(" · "),
                style = OpenType.monoXS,
                modifier = Modifier.weight(1f, fill = false)
            )
            pair.weightDeltaLb?.takeIf { abs(it) >= 0.05 }?.let { d ->
                OpenLabel(
                    "${if (d > 0) "↑" else "↓"} ${formatWeightDelta(abs(d), unit)}",
                    style = OpenType.monoXS, color = ink, modifier = Modifier.padding(start = 12.dp)
                )
            }
        } else {
            OpenLabel(
                if (photos.isEmpty()) "Your first photo starts the comparison" else "A second photo makes the pair",
                style = OpenType.monoXS
            )
        }
    }
    Spacer(Modifier.height(24.dp))
    OpenPrimaryButton("Take today's photo", onClick = actions.onOpenCamera, icon = OpenIcons.Camera)

    // ── Gallery ──
    Spacer(Modifier.height(44.dp))
    Row(Modifier.fillMaxWidth(), horizontalArrangement = Arrangement.SpaceBetween, verticalAlignment = Alignment.CenterVertically) {
        OpenLabel(
            "Gallery · ${photos.size} ${if (photos.size == 1) "photo" else "photos"}",
            modifier = Modifier.weight(1f, fill = false).semantics { heading() }
        )
        OpenLabel("Front · Side · Back", modifier = Modifier.padding(start = 12.dp))
    }
    Spacer(Modifier.height(20.dp))
    val months = remember(photos) { galleryByMonth(photos, zone) }
    if (months.isEmpty()) {
        OpenLabel("Each month's photos land here", style = OpenType.monoXS)
        return
    }
    months.take(MONTHS_SHOWN).forEach { m ->
        val name = m.month.month.getDisplayName(TextStyle.FULL, locale)
        OpenLabel(
            if (m.month.year == today.year) name else "$name ${m.month.year}",
            style = OpenType.monoXS, color = ink,
            modifier = Modifier.semantics { heading() }
        )
        Spacer(Modifier.height(10.dp))
        Row(Modifier.fillMaxWidth(), horizontalArrangement = Arrangement.spacedBy(8.dp)) {
            repeat(3) { i ->
                val p = m.tiles.getOrNull(i)
                if (p == null) Spacer(Modifier.weight(1f))
                else PhotoFrame(
                    p, photo, actions.onOpenGallery,
                    caption = PhotoPose.fromKey(p.pose)?.label ?: dateOf(p).format(dayFmt),
                    sub = p.weightLb?.let { formatWeight(it, unit) },
                    modifier = Modifier.weight(1f).aspectRatio(116f / 154f)
                )
            }
        }
        Spacer(Modifier.height(22.dp))
    }
    if (months.size > MONTHS_SHOWN) {
        OpenSectionHead(
            "Earlier months",
            action = "All ${photos.size}",
            actionLabel = "Open every photo in the gallery",
            onAction = actions.onOpenGallery
        )
    }
}

/**
 * A photo with its caption over a shade at its foot, or, with no photo, the empty frame that says a
 * photo goes here. Tapping either opens the gallery.
 */
@Composable
private fun PhotoFrame(
    p: ProgressPhoto?,
    photo: @Composable (ProgressPhoto, Modifier) -> Unit,
    onOpen: () -> Unit,
    caption: String?,
    sub: String?,
    modifier: Modifier,
) {
    val ink = MaterialTheme.colorScheme.onBackground
    val muted = MaterialTheme.colorScheme.onSurfaceVariant
    Box(
        modifier
            .clip(PHOTO_SHAPE)
            .background(ProfileColors.photoGround)
            .bounceClick(role = androidx.compose.ui.semantics.Role.Button, onClick = onOpen)
            .semantics(mergeDescendants = true) {
                contentDescription = listOfNotNull(if (p == null) "Empty photo frame" else "Progress photo", caption, sub).joinToString(", ")
            }
    ) {
        if (p == null) {
            EmptyGlyph()
        } else {
            photo(p, Modifier.fillMaxSize())
            Box(
                Modifier.align(Alignment.BottomCenter).fillMaxWidth().height(64.dp)
                    .background(Brush.verticalGradient(listOf(Color.Transparent, ProfileColors.CaptionShade)))
            )
        }
        if (caption != null) {
            Column(Modifier.align(Alignment.BottomStart).padding(10.dp).clearAndSetSemantics { }) {
                OpenLabel(caption, style = OpenType.monoXS, color = ink)
                if (sub != null) OpenLabel(sub, style = OpenType.monoXS, color = muted)
            }
        }
    }
}

@Composable
private fun BoxScope.EmptyGlyph() {
    Icon(
        OpenIcons.User,
        contentDescription = null,
        tint = ProfileColors.PhotoGlyph,
        modifier = Modifier.align(Alignment.Center).size(36.dp)
    )
}

/**
 * The Photos tab while the gallery lock is on (lk4): the count, one serif line, why, and a small
 * Unlock that raises the system prompt. The empty tiles stand where the photos go; nothing of a
 * photo is read or drawn here.
 */
@Composable
internal fun ProfilePhotosLocked(count: Int, note: String?, onUnlock: () -> Unit) {
    val ink = MaterialTheme.colorScheme.onBackground
    val muted = MaterialTheme.colorScheme.onSurfaceVariant
    Row(verticalAlignment = Alignment.CenterVertically, horizontalArrangement = Arrangement.spacedBy(8.dp)) {
        Icon(OpenIcons.Lock, contentDescription = null, tint = muted, modifier = Modifier.size(14.dp))
        OpenLabel("Gallery · $count ${if (count == 1) "photo" else "photos"}")
    }
    Spacer(Modifier.height(12.dp))
    Text("Your photos are locked", style = OpenType.figureMS, color = ink, modifier = Modifier.semantics { heading() })
    Spacer(Modifier.height(8.dp))
    Text("Unlock to see them. Overview and Body stay open.", style = OpenType.body, color = muted)
    Spacer(Modifier.height(22.dp))
    OpenSmallButton("Unlock", filled = true, onClick = onUnlock, icon = OpenIcons.Fingerprint)
    if (note != null) {
        Spacer(Modifier.height(10.dp))
        Text(note, style = OpenType.bodyS, color = muted)
    }
    Spacer(Modifier.height(36.dp))
    Row(Modifier.fillMaxWidth().clearAndSetSemantics { }, horizontalArrangement = Arrangement.spacedBy(8.dp)) {
        repeat(3) {
            Box(Modifier.weight(1f).height(154.dp).clip(RoundedCornerShape(2.dp)).background(ProfileColors.Hidden))
        }
    }
}

/**
 * The gallery's unlock prompt, raised from the Photos tab in place: the same system sheet and the
 * same rules as the full-screen gate ([com.forge.app.ui.security.AppLockScreen]) — a device with no
 * credential fails open, a cancel leaves the tab locked, a lockout says so. Returns the action and
 * the line to show under it.
 */
internal class GalleryUnlock(val launch: () -> Unit, val note: String?)

@Composable
internal fun rememberGalleryUnlock(onUnlocked: () -> Unit): GalleryUnlock {
    val activity = LocalActivity.current as? FragmentActivity
    val unlocked by rememberUpdatedState(onUnlocked)
    var note by remember { mutableStateOf<String?>(null) }
    val launch: () -> Unit = remember(activity) { fun() {
        val act = activity ?: run { note = "Unlock isn't available right now. Try again."; return }
        when (BiometricAuthenticator.availability(act)) {
            BiometricAuthenticator.Availability.NO_CREDENTIAL -> { unlocked(); return }
            BiometricAuthenticator.Availability.UNAVAILABLE -> {
                note = "Unlock isn't available right now. Try again."
                return
            }
            BiometricAuthenticator.Availability.AVAILABLE -> Unit
        }
        note = null
        BiometricAuthenticator.authenticate(
            activity = act,
            subtitle = "Unlock your progress photos",
            onSuccess = { unlocked() },
            onError = { code, message ->
                when {
                    BiometricAuthenticator.availability(act) == BiometricAuthenticator.Availability.NO_CREDENTIAL -> unlocked()
                    code == BiometricPrompt.ERROR_USER_CANCELED ||
                        code == BiometricPrompt.ERROR_NEGATIVE_BUTTON ||
                        code == BiometricPrompt.ERROR_CANCELED -> Unit
                    code == BiometricPrompt.ERROR_LOCKOUT ||
                        code == BiometricPrompt.ERROR_LOCKOUT_PERMANENT -> note = "Too many attempts. Try again in a moment."
                    else -> note = message.toString().ifBlank { "Couldn't verify. Try again." }
                }
            }
        )
    } }
    return GalleryUnlock(launch, note)
}
