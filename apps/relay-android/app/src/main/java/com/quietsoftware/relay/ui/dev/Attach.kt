package com.quietsoftware.relay.ui.dev

import android.content.Context
import android.graphics.Bitmap
import android.graphics.ImageDecoder
import android.net.Uri
import androidx.activity.compose.rememberLauncherForActivityResult
import androidx.activity.result.PickVisualMediaRequest
import androidx.activity.result.contract.ActivityResultContracts
import androidx.compose.foundation.Image
import androidx.compose.foundation.background
import androidx.compose.foundation.border
import androidx.compose.foundation.clickable
import androidx.compose.foundation.horizontalScroll
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.rememberScrollState
import androidx.compose.foundation.shape.CircleShape
import androidx.compose.runtime.Composable
import androidx.compose.runtime.getValue
import androidx.compose.runtime.rememberUpdatedState
import androidx.compose.runtime.snapshots.SnapshotStateList
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.clip
import androidx.compose.ui.graphics.ImageBitmap
import androidx.compose.ui.graphics.asImageBitmap
import androidx.compose.ui.layout.ContentScale
import androidx.compose.ui.platform.LocalContext
import androidx.compose.ui.semantics.Role
import androidx.compose.ui.unit.dp
import com.quietsoftware.relay.ui.LocalNav
import com.quietsoftware.relay.ui.kit.Glyph
import com.quietsoftware.relay.ui.kit.Radii
import com.quietsoftware.relay.ui.kit.T
import com.quietsoftware.relay.ui.theme.Relay
import com.quietsoftware.relay.ui.threads.Spinner
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.launch
import kotlinx.coroutines.withContext
import java.io.ByteArrayOutputStream
import kotlin.math.roundToInt

/**
 * A photo or screenshot in a message box, on its way to the PC: picked, shrunk and sent at once,
 * so the message only has to name it. The agent chat and a thread's message box share it. [sent]
 * is what the PC made of it: the path in the agent's checkout (`session.upload`), or a thread's
 * image (`thread.upload`); null while it goes, or for good when [failed].
 */
internal data class Attachment<T>(val id: Long, val thumb: ImageBitmap?, val sent: T? = null, val failed: Boolean = false) {
    val sending: Boolean get() = sent == null && !failed
}

/** Pictures one message may carry, to an agent or in a thread. */
internal const val MAX_PICTURES = 4

/** The longer side a photo is sent at: enough to read a screenshot's text, small enough for a phone's link. */
private const val MAX_SIDE = 2048
private const val THUMB_SIDE = 160

/**
 * Decodes a picture upright and at most [side] on its longer side, in memory a [Bitmap] can be
 * compressed or scaled from, which a hardware bitmap cannot be.
 */
internal fun atMost(side: Int) = ImageDecoder.OnHeaderDecodedListener { decoder, info, _ ->
    val w = info.size.width
    val h = info.size.height
    val scale = minOf(1f, side.toFloat() / maxOf(w, h, 1))
    if (scale < 1f) decoder.setTargetSize((w * scale).roundToInt().coerceAtLeast(1), (h * scale).roundToInt().coerceAtLeast(1))
    decoder.allocator = ImageDecoder.ALLOCATOR_SOFTWARE
}

/** A picked picture, upright and at most [MAX_SIDE] on its longer side, as JPEG, with a thumbnail; null when it cannot be read. */
internal fun shrink(context: Context, uri: Uri): Pair<ByteArray, ImageBitmap>? = runCatching {
    val bitmap = ImageDecoder.decodeBitmap(ImageDecoder.createSource(context.contentResolver, uri), atMost(MAX_SIDE))
    val out = ByteArrayOutputStream()
    bitmap.compress(Bitmap.CompressFormat.JPEG, 88, out)
    val t = THUMB_SIDE.toFloat() / maxOf(bitmap.width, bitmap.height)
    val thumb = Bitmap.createScaledBitmap(bitmap, (bitmap.width * t).roundToInt().coerceAtLeast(1), (bitmap.height * t).roundToInt().coerceAtLeast(1), true)
    out.toByteArray() to thumb.asImageBitmap()
}.getOrNull()

/**
 * The photo picker of a message box. What is picked joins [into], at most [MAX_PICTURES] in all;
 * each picture is shrunk off the main thread and handed to [upload] at once, which answers what
 * the PC made of it, or null when it would not take it. It runs past the screen ([Feeds.scope]),
 * so a picture still going when the person leaves arrives all the same.
 */
@Composable
internal fun <T : Any> rememberPicturePicker(into: SnapshotStateList<Attachment<T>>, upload: suspend (ByteArray) -> T?): () -> Unit {
    val nav = LocalNav.current
    val app = LocalContext.current.applicationContext
    val send by rememberUpdatedState(upload)
    val picker = rememberLauncherForActivityResult(ActivityResultContracts.PickMultipleVisualMedia(MAX_PICTURES)) { uris ->
        val list = into
        val room = (MAX_PICTURES - list.size).coerceAtLeast(0)
        if (uris.size > room) nav.shell.toast("Up to $MAX_PICTURES pictures in one message")
        for (uri in uris.take(room)) {
            val id = System.nanoTime()
            list.add(Attachment(id, null))
            Feeds.scope.launch {
                val shrunk = withContext(Dispatchers.IO) { shrink(app, uri) }
                if (shrunk == null) return@launch list.change(id) { it.copy(failed = true) }
                list.change(id) { it.copy(thumb = shrunk.second) }
                val answer = send(shrunk.first)
                list.change(id) { if (answer != null) it.copy(sent = answer) else it.copy(failed = true) }
            }
        }
    }
    return { picker.launch(PickVisualMediaRequest(ActivityResultContracts.PickVisualMedia.ImageOnly)) }
}

/** The picture [id] as [edit] makes it; nothing when it was taken out meanwhile. */
private fun <T> SnapshotStateList<Attachment<T>>.change(id: Long, edit: (Attachment<T>) -> Attachment<T>) {
    val i = indexOfFirst { it.id == id }
    if (i >= 0) this[i] = edit(this[i])
}

/** The message as sent: what was typed, then each picture's path on a line of its own, which both CLIs read as the picture. */
internal fun withAttachments(text: String, paths: List<String>): String =
    (listOf(text.trim()).filter { it.isNotEmpty() } + paths).joinToString("\n\n")

private val ATTACHED = Regex("""^\s*\S*\.relay/attachments/(phone-[0-9a-f]+\.(?:png|jpe?g|gif|webp))\s*$""")

/** A message's words apart from the pictures it named: the text, and each picture's file name. */
internal fun splitAttached(text: String): Pair<String, List<String>> {
    val names = ArrayList<String>()
    val rest = text.lines().filter { line ->
        val m = ATTACHED.matchEntire(line) ?: return@filter true
        names += m.groupValues[1]
        false
    }
    return rest.joinToString("\n").trim() to names
}

/** The pictures waiting in the message box: thumbnails, a spinner while each goes, a key to take one out. */
@Composable
internal fun AttachmentStrip(items: List<Attachment<*>>, onRemove: (Long) -> Unit) {
    val c = Relay.colors
    Row(Modifier.horizontalScroll(rememberScrollState()).padding(end = 8.dp, bottom = 8.dp), horizontalArrangement = Arrangement.spacedBy(8.dp)) {
        for (a in items) {
            Box(Modifier.size(64.dp)) {
                Box(Modifier.size(58.dp).align(Alignment.BottomStart).clip(Radii.keycap).background(c.slab).border(1.dp, if (a.failed) c.held else c.strong, Radii.keycap), contentAlignment = Alignment.Center) {
                    a.thumb?.let { Image(it, null, Modifier.size(58.dp), contentScale = ContentScale.Crop) }
                    when {
                        a.sending -> Box(Modifier.size(24.dp).clip(CircleShape).background(c.wall.copy(alpha = .7f)), contentAlignment = Alignment.Center) { Spinner(13.dp, c.ink) }
                        a.failed -> Box(Modifier.size(24.dp).clip(CircleShape).background(c.wall.copy(alpha = .8f)), contentAlignment = Alignment.Center) { Glyph("close", 12.dp, c.heldText) }
                    }
                }
                Box(
                    Modifier.align(Alignment.TopEnd).size(22.dp).clip(CircleShape).background(c.ink)
                        .clickable(onClickLabel = "Take the picture out", role = Role.Button) { onRemove(a.id) },
                    contentAlignment = Alignment.Center,
                ) {
                    Glyph("close", 11.dp, c.wall)
                }
            }
        }
    }
}

/** A picture a sent message named, drawn in its bubble as a quiet pill rather than a path. */
@Composable
internal fun AttachedPill(name: String) {
    val c = Relay.colors
    Row(
        Modifier.clip(Radii.pill).background(c.slab).border(1.dp, c.strong, Radii.pill).padding(horizontal = 9.dp, vertical = 5.dp),
        verticalAlignment = Alignment.CenterVertically,
        horizontalArrangement = Arrangement.spacedBy(6.dp),
    ) {
        Glyph("file-image", 13.dp, c.ink3)
        T("Picture", style = Relay.type.caption, color = c.ink2, maxLines = 1)
        T(name.substringAfter("phone-").take(6), style = Relay.type.mono.copy(fontSize = Relay.type.caption.fontSize), color = c.ink3, maxLines = 1)
    }
}
