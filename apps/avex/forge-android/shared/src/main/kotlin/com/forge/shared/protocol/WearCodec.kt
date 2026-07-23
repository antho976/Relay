package com.forge.shared.protocol

import kotlinx.serialization.Serializable
import kotlinx.serialization.encodeToString
import kotlinx.serialization.json.Json

/**
 * Serializes protocol DTOs to the bytes that ride DataItems/Messages, and decodes them with the
 * version rule: additive fields are ignored by old readers (ignoreUnknownKeys), and a payload
 * stamped with a NEWER protocol version than this build knows decodes to [DecodeResult.NewerVersion]
 * so the UI can show "update the other app" — never a crash, never silently-wrong data.
 */
object WearCodec {

    val json: Json = Json {
        ignoreUnknownKeys = true
        encodeDefaults = true
    }

    /** Just the version stamp, readable from ANY payload regardless of its shape. */
    @Serializable
    private data class VersionProbe(val v: Int = 0)

    sealed interface DecodeResult<out T> {
        data class Ok<T>(val value: T) : DecodeResult<T>
        /** The other side speaks a newer protocol — surface "update the other app". */
        data object NewerVersion : DecodeResult<Nothing>
        /** Corrupt / unparseable payload — drop silently. */
        data object Invalid : DecodeResult<Nothing>
    }

    inline fun <reified T> encode(dto: T): ByteArray =
        json.encodeToString(dto).encodeToByteArray()

    inline fun <reified T> decode(bytes: ByteArray): DecodeResult<T> {
        val text = try { bytes.decodeToString() } catch (t: Throwable) { return DecodeResult.Invalid }
        val version = probeVersion(text) ?: return DecodeResult.Invalid
        if (version > WearProtocol.VERSION) return DecodeResult.NewerVersion
        return try {
            DecodeResult.Ok(json.decodeFromString<T>(text))
        } catch (t: Throwable) {
            DecodeResult.Invalid
        }
    }

    /** The `v` stamp of a serialized payload, or null when it isn't even parseable JSON. */
    fun probeVersion(text: String): Int? = try {
        json.decodeFromString<VersionProbe>(text).v.takeIf { it > 0 }
    } catch (t: Throwable) {
        null
    }
}
