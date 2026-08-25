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
        /**
         * An unknown ENUM constant falls back to the property's declared default instead of failing
         * the whole payload.
         *
         * `ignoreUnknownKeys` covers added FIELDS, which is what the "additive changes don't bump
         * VERSION" rule promised — it does not cover added enum constants. So a future
         * `ProtocolWeightUnit.G` shipped as "additive" would have passed the version gate on an
         * older watch, thrown inside the decoder, and been mapped to a silent drop: the wrist
         * falling back to its idle glance MID-WORKOUT, with every subsequent republish dropped the
         * same way. Every enum field on the wire carries a sane default (`unit` defaults to LB), so
         * degrading to it is strictly better than showing nothing.
         *
         * This does NOT make every enum change safe: a new value the other side must ACT on (a new
         * `TimerCommand.Action`) still has to bump [WearProtocol.VERSION], because being silently
         * coerced to the default action is worse than being told to update.
         */
        coerceInputValues = true
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

    /** Just the command id, readable from a payload this build can't otherwise decode. */
    @Serializable
    private data class CommandIdProbe(val commandId: String = "")

    /**
     * The `commandId` of a command payload, even when its body is from a newer protocol.
     *
     * Without this the phone could only drop such a command in silence, so the wrist had no way to
     * tell "your phone app is too old" from "bluetooth dropped" — it printed
     * "Not logged · reconnecting" and invited a re-tap that would fail identically forever.
     */
    fun probeCommandId(bytes: ByteArray): String? = try {
        json.decodeFromString<CommandIdProbe>(bytes.decodeToString()).commandId.takeIf { it.isNotBlank() }
    } catch (t: Throwable) {
        null
    }
}
