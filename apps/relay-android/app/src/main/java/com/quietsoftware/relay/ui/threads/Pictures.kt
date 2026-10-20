package com.quietsoftware.relay.ui.threads

import android.content.Context
import android.graphics.ImageDecoder
import android.util.Base64
import android.util.LruCache
import androidx.compose.foundation.Image
import androidx.compose.foundation.background
import androidx.compose.foundation.border
import androidx.compose.foundation.ExperimentalFoundationApi
import androidx.compose.foundation.combinedClickable
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.ExperimentalLayoutApi
import androidx.compose.foundation.layout.FlowRow
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.runtime.Composable
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.produceState
import androidx.compose.runtime.remember
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.clip
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.graphics.ImageBitmap
import androidx.compose.ui.graphics.asImageBitmap
import androidx.compose.ui.layout.ContentScale
import androidx.compose.ui.platform.LocalContext
import androidx.compose.ui.semantics.Role
import androidx.compose.ui.semantics.contentDescription
import androidx.compose.ui.semantics.semantics
import androidx.compose.ui.text.style.TextAlign
import androidx.compose.ui.unit.Dp
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp
import androidx.compose.ui.window.Dialog
import androidx.compose.ui.window.DialogProperties
import com.quietsoftware.relay.core.wire.BusError
import com.quietsoftware.relay.core.wire.BusException
import com.quietsoftware.relay.core.wire.arr
import com.quietsoftware.relay.core.wire.l
import com.quietsoftware.relay.core.wire.o
import com.quietsoftware.relay.core.wire.s
import com.quietsoftware.relay.data.Relay as RelayData
import com.quietsoftware.relay.ui.LocalNav
import com.quietsoftware.relay.ui.Nav
import com.quietsoftware.relay.ui.dev.atMost
import com.quietsoftware.relay.ui.kit.Glyph
import com.quietsoftware.relay.ui.kit.IconKey
import com.quietsoftware.relay.ui.kit.Radii
import com.quietsoftware.relay.ui.kit.T
import com.quietsoftware.relay.ui.theme.Relay
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.withContext
import kotlinx.serialization.json.JsonElement
import kotlinx.serialization.json.JsonNull
import kotlinx.serialization.json.JsonObject
import kotlinx.serialization.json.JsonPrimitive
import kotlinx.serialization.json.buildJsonObject
import kotlinx.serialization.json.put
import java.io.File
import java.nio.ByteBuffer

/**
 * Pictures in a thread. One is sent to the PC as soon as it is picked (`thread.upload`), so a
 * message only names it (`images`, its ids), and a message of the person's carries them in its
 * body: `{"text", "images": [{"id", "media_type", "width", "height"}]}`. They are read back with
 * `thread.image` and kept on this phone, so a picture seen once, or sent from here, shows with the
 * PC away.
 */
internal data class Picture(val id: JsonElement, val width: Int? = null, val height: Int? = null) {
    /** Width over height when the PC said them. */
    val ratio: Float? get() = if (width != null && height != null && width > 0 && height > 0) width.toFloat() / height else null
}

/** A message's pictures: its body's `images`, or the ids an op's payload names. */
internal fun picturesOf(body: JsonObject?): List<Picture> = body?.arr("images")?.mapNotNull { e ->
    when (e) {
        is JsonObject -> e["id"]?.takeIf { it !is JsonNull }?.let { Picture(it, e.l("width")?.toInt(), e.l("height")?.toInt()) }
        is JsonPrimitive -> e.takeIf { it !is JsonNull }?.let { Picture(it) }
        else -> null
    }
}.orEmpty()

/** Claude Code's own names for Claude's models, which a thread may store instead of an id. */
private val CLAUDE_ALIASES = setOf("default", "opus", "sonnet", "haiku", "fable", "opusplan")

/**
 * Whether a thread on [model] can be shown pictures. Claude's models read them; a model on the PC
 * itself (local_models.rs, "On this PC" in its picker) does not, and the PC refuses them
 * (`thread.images_unsupported`). The phone does not list the PC's own models (`thread.models` is
 * not a phone op), so a model that is not Claude's is taken for one of them.
 */
internal fun seesPictures(model: String?): Boolean {
    val m = model.orEmpty().trim().lowercase()
    return m.isEmpty() || m.startsWith("claude") || m.substringBefore('[') in CLAUDE_ALIASES
}

/** What the message box says where it does not offer pictures for that reason. */
internal const val PICTURES_NEED_CLAUDE = "Pictures need a Claude model"

/** Why the PC would not take a picture, in a toast's words. */
internal fun pictureRefused(e: BusError): String = when (e.code) {
    "link.down", "link.timeout" -> "A picture did not reach the PC: it is out of reach"
    "bus.unknown_op", "bus.not_implemented", "remote.op" -> "The PC's Relay is older than this app: update it to send pictures in threads"
    "thread.images_unsupported" -> "$PICTURES_NEED_CLAUDE: this thread runs on a model on the PC"
    else -> "A picture did not reach the PC: ${e.message.ifBlank { e.code }}"
}

/** The longer side a picture is drawn at in a bubble, and full screen. */
private const val BUBBLE_SIDE = 720
private const val FULL_SIDE = 2048

/** Kept on this phone: about 200 pictures as the phone sends them. */
private const val KEEP_BYTES = 96L * 1024 * 1024

/**
 * A thread's pictures on this phone: decoded ones for the bubbles in memory, and every picture's
 * bytes in the app's cache, read from the PC once. The op cache keeps at most 600,000 characters
 * of an answer (`RoomReplica.putQuery`), and a picture's bytes are often more, so they are kept here.
 */
internal object ThreadPictures {
    private val shown = object : LruCache<String, ImageBitmap>(32 * 1024 * 1024) {
        override fun sizeOf(key: String, value: ImageBitmap) = value.width * value.height * 4
    }

    @Volatile private var lastToast: Pair<String, Long>? = null

    private fun keyOf(id: JsonElement): String = (id as? JsonPrimitive)?.content ?: id.toString()

    private fun dir(context: Context) = File(context.cacheDir, "thread-pictures")

    /** A file name from any id: its UTF-8 bytes in hex. */
    private fun file(context: Context, id: JsonElement): File =
        File(dir(context), keyOf(id).toByteArray().joinToString("") { "%02x".format(it.toInt() and 0xff) }.take(200))

    /** Keeps [bytes] as picture [id]'s, the oldest kept going past [KEEP_BYTES]. On a background thread. */
    fun keep(context: Context, id: JsonElement, bytes: ByteArray) {
        runCatching {
            val f = file(context, id)
            f.parentFile?.mkdirs()
            val part = File(f.parentFile, f.name + ".part")
            part.writeBytes(bytes)
            if (!part.renameTo(f)) part.delete()
            val all = dir(context).listFiles().orEmpty().sortedByDescending { it.lastModified() }
            var total = 0L
            for (x in all) {
                total += x.length()
                if (total > KEEP_BYTES) x.delete()
            }
        }
    }

    /**
     * Picture [id]'s bytes: kept here (and so kept longest), else read from the PC and kept.
     * Throws [BusException] when the PC cannot answer; null when it sent none.
     */
    private suspend fun bytes(context: Context, relay: RelayData, id: JsonElement): ByteArray? {
        val f = file(context, id)
        withContext(Dispatchers.IO) {
            runCatching { f.takeIf { it.exists() }?.readBytes()?.also { f.setLastModified(System.currentTimeMillis()) } }.getOrNull()
        }?.let { return it }
        val answer = relay.call("thread.image", buildJsonObject { put("id", id) }) as? JsonObject
        val b64 = answer?.s("bytes_b64") ?: return null
        return withContext(Dispatchers.IO) {
            runCatching { Base64.decode(b64, Base64.DEFAULT) }.getOrNull()?.also { keep(context, id, it) }
        }
    }

    /**
     * Picture [id] upright, at most [side] on its longer side; null when the phone cannot draw
     * what the PC sent. Throws [BusException] when it is not on this phone and the PC cannot say.
     */
    suspend fun image(context: Context, relay: RelayData, id: JsonElement, side: Int = BUBBLE_SIDE): ImageBitmap? {
        val key = "${keyOf(id)}@$side"
        shown.get(key)?.let { return it }
        val bytes = bytes(context, relay, id) ?: return null
        val image = withContext(Dispatchers.Default) {
            runCatching { ImageDecoder.decodeBitmap(ImageDecoder.createSource(ByteBuffer.wrap(bytes)), atMost(side)).asImageBitmap() }.getOrNull()
        } ?: return null
        if (side <= BUBBLE_SIDE) shown.put(key, image)
        return image
    }

    /** The bubble's picture [id] if it was drawn already. */
    fun drawn(id: JsonElement): ImageBitmap? = shown.get("${keyOf(id)}@$BUBBLE_SIDE")

    /**
     * A picture for a thread (`thread.upload`, a JPEG the phone shrank): the image the PC keeps
     * for it, its bytes kept here too so its bubble shows at once; null, said in a toast, when the
     * PC would not take it.
     */
    suspend fun upload(context: Context, nav: Nav, jpeg: ByteArray): JsonObject? = try {
        val payload = withContext(Dispatchers.Default) {
            buildJsonObject {
                put("media_type", "image/jpeg")
                put("bytes_b64", Base64.encodeToString(jpeg, Base64.NO_WRAP))
            }
        }
        val image = (nav.relay.call("thread.upload", payload) as? JsonObject)?.o("image")?.takeIf { it["id"] != null && it["id"] !is JsonNull }
        if (image == null) say(nav, "A picture did not reach the PC: it sent nothing back")
        image?.get("id")?.let { id -> withContext(Dispatchers.IO) { keep(context, id, jpeg) } }
        image
    } catch (e: BusException) {
        say(nav, pictureRefused(e.error))
        null
    }

    /** One toast for pictures refused together, not one each. */
    private fun say(nav: Nav, text: String) {
        val now = System.currentTimeMillis()
        val last = lastToast
        if (last != null && last.first == text && now - last.second < 5_000) return
        lastToast = text to now
        nav.shell.toast(text)
    }
}

private sealed interface Shown {
    data object Reading : Shown

    data class Ready(val image: ImageBitmap) : Shown

    data class Absent(val why: String) : Shown
}

/**
 * The pictures of a message of the person's, above its words: one up to 240dp wide, several side
 * by side, each in its own shape and rounded as a card. A tap hands one to [onView], which opens
 * it full screen from outside the list: a reply that scrolls the message away must not close it.
 * A long press is [onKeys], the message's keys, which a message of pictures alone has no words for.
 */
@OptIn(ExperimentalLayoutApi::class)
@Composable
internal fun SentPictures(pictures: List<Picture>, onView: (Picture) -> Unit, modifier: Modifier = Modifier, onKeys: (() -> Unit)? = null) {
    val one = pictures.size == 1
    FlowRow(modifier, horizontalArrangement = Arrangement.spacedBy(6.dp, Alignment.End), verticalArrangement = Arrangement.spacedBy(6.dp)) {
        for (p in pictures) SentPicture(p, if (one) 240.dp else 116.dp, if (one) 320.dp else 160.dp, onKeys) { onView(p) }
    }
}

@OptIn(ExperimentalFoundationApi::class)
@Composable
private fun SentPicture(p: Picture, maxWidth: Dp, maxHeight: Dp, onKeys: (() -> Unit)?, onOpen: () -> Unit) {
    val c = Relay.colors
    val nav = LocalNav.current
    val app = LocalContext.current.applicationContext
    val state by produceState<Shown>(ThreadPictures.drawn(p.id)?.let { Shown.Ready(it) } ?: Shown.Reading, p.id) {
        if (value is Shown.Ready) return@produceState
        value = try {
            ThreadPictures.image(app, nav.relay, p.id)?.let { Shown.Ready(it) } ?: Shown.Absent("The phone cannot draw it")
        } catch (e: BusException) {
            Shown.Absent(if (e.error.transient) "Shows once the PC is in reach" else "Could not be read")
        }
    }
    val ready = state as? Shown.Ready
    // Its own shape once drawn, the PC's size until then, square when neither is known; a very
    // long or wide one is cut to fit.
    val ratio = (ready?.image?.let { it.width.toFloat() / maxOf(it.height, 1) } ?: p.ratio ?: 1f).coerceIn(.25f, 4f)
    val w = minOf(maxWidth, maxHeight * ratio)
    Box(
        Modifier.size(w, w / ratio).clip(Radii.card).background(c.slab).border(1.dp, c.edge, Radii.card)
            .then(
                if (ready != null || onKeys != null) {
                    Modifier.combinedClickable(
                        onClickLabel = if (ready != null) "Open the picture" else null,
                        role = Role.Button,
                        onLongClickLabel = if (onKeys != null) "Show this message's keys" else null,
                        onLongClick = onKeys,
                        onClick = { if (ready != null) onOpen() },
                    )
                } else Modifier,
            )
            .semantics { if (ready == null) contentDescription = "Picture you sent" },
        contentAlignment = Alignment.Center,
    ) {
        when (val s = state) {
            is Shown.Ready -> Image(s.image, "Picture you sent", Modifier.fillMaxSize(), contentScale = ContentScale.Crop)
            Shown.Reading -> Spinner(13.dp)
            is Shown.Absent -> Column(Modifier.padding(8.dp), horizontalAlignment = Alignment.CenterHorizontally, verticalArrangement = Arrangement.spacedBy(4.dp)) {
                Glyph("file-image", 16.dp, c.ink3)
                T(s.why, style = Relay.type.caption.copy(fontSize = 11.sp, lineHeight = 14.sp, textAlign = TextAlign.Center), color = c.ink3, maxLines = 3)
            }
        }
    }
}

/** A picture over the whole screen, fitted, at up to 2048 px; Back or the close key puts it away. */
@Composable
internal fun PictureViewer(p: Picture, onClose: () -> Unit) {
    val c = Relay.colors
    val nav = LocalNav.current
    val app = LocalContext.current.applicationContext
    val full by produceState(ThreadPictures.drawn(p.id), p.id) {
        try {
            ThreadPictures.image(app, nav.relay, p.id, FULL_SIDE)?.let { value = it }
        } catch (_: BusException) {
            // The bubble's copy stays: the PC is away, or would not send it larger.
        }
    }
    Dialog(onDismissRequest = onClose, properties = DialogProperties(usePlatformDefaultWidth = false)) {
        Box(Modifier.fillMaxSize().background(Color.Black), contentAlignment = Alignment.Center) {
            val image = full
            if (image != null) Image(image, "Picture you sent", Modifier.fillMaxSize(), contentScale = ContentScale.Fit) else Spinner(16.dp, c.ink)
            IconKey("close", onClose, Modifier.align(Alignment.TopEnd).padding(8.dp), tint = c.ink, label = "Close the picture")
        }
    }
}
