package com.forge.app.core.crypto

import java.io.ByteArrayOutputStream
import java.io.DataInputStream
import java.io.EOFException
import java.io.File
import java.io.InputStream
import java.io.OutputStream
import java.nio.ByteBuffer
import java.security.GeneralSecurityException
import java.security.MessageDigest
import java.security.SecureRandom
import java.text.Normalizer
import javax.crypto.Cipher
import javax.crypto.Mac
import javax.crypto.SecretKeyFactory
import javax.crypto.spec.GCMParameterSpec
import javax.crypto.spec.PBEKeySpec
import javax.crypto.spec.SecretKeySpec

/**
 * The password-protected backup container (format v1).
 *
 * A backup ZIP can hold a whole training history and every progress photo, and it goes wherever the
 * user sends it: Downloads, a USB stick, a cloud drive's synced folder. With a backup password set,
 * the ZIP is streamed through this container instead of being written as-is.
 *
 * Layout. Every integer is big-endian; the whole header is authenticated as the associated data of
 * every segment, so no field of it can be edited without the file failing to open.
 *
 * ```
 *   magic        8   "AVEXENC\u0000"
 *   version      1   1
 *   kdf          1   1 = PBKDF2-HMAC-SHA256
 *   iterations   4   PBKDF2 work factor
 *   kdfSalt     16   random per password (the same for every backup made under one password)
 *   fileSalt    32   random per file
 *   noncePrefix  7   random per file
 *   segmentSize  4   plaintext bytes per segment
 *   keyCheck    16   HMAC tag that tells a wrong password from a damaged file
 *   segments …       AES-256-GCM, each [segmentSize] plaintext bytes + a 16-byte tag; the last is
 *                    shorter (possibly empty)
 * ```
 *
 * Keys. The password goes through PBKDF2 once into a 256-bit master key. Each file then gets its own
 * encryption key from HKDF-SHA256 over the master key and that file's random salt, so no two backups
 * ever share a GCM key and nonce reuse across files is impossible even though every weekly backup is
 * made from the same cached master key (see BackupEncryption).
 *
 * Segments. GCM on the JVM releases no plaintext until the whole message is authenticated, which
 * would mean holding a 500 MB backup in memory to restore it. The payload is therefore cut into
 * segments that are each sealed separately (the STREAM construction, as in Tink's streaming AEAD):
 * the nonce is `noncePrefix ‖ segmentIndex ‖ lastFlag`, so a segment moved, dropped, duplicated or
 * cut off at a boundary no longer authenticates.
 */
object BackupCrypto {

    private val MAGIC = byteArrayOf(0x41, 0x56, 0x45, 0x58, 0x45, 0x4E, 0x43, 0x00) // AVEXENC\0
    private const val VERSION: Byte = 1
    private const val KDF_PBKDF2_SHA256: Byte = 1

    /**
     * PBKDF2-HMAC-SHA256 work factor for new passwords: OWASP's current recommendation for this
     * function. It is stored in each file, so it can be raised later without breaking old backups.
     */
    const val DEFAULT_ITERATIONS = 600_000

    /**
     * Bounds on the iteration count a FILE may ask for. The floor stops a doctored header from
     * turning the check into a cheap one; the ceiling stops one from pinning a CPU for hours.
     */
    internal const val MIN_ITERATIONS = 100_000
    internal const val MAX_ITERATIONS = 10_000_000

    const val SEGMENT_SIZE = 64 * 1024
    private const val MIN_SEGMENT_SIZE = 4 * 1024
    private const val MAX_SEGMENT_SIZE = 1024 * 1024

    private const val KDF_SALT_BYTES = 16
    private const val FILE_SALT_BYTES = 32
    private const val NONCE_PREFIX_BYTES = 7
    private const val KEY_CHECK_BYTES = 16
    private const val KEY_BYTES = 32
    private const val TAG_BYTES = 16
    private const val TAG_BITS = TAG_BYTES * 8

    /** Header bytes before the key check; the key check is an HMAC over exactly these. */
    private const val PREFIX_BYTES = 8 + 1 + 1 + 4 + KDF_SALT_BYTES + FILE_SALT_BYTES + NONCE_PREFIX_BYTES + 4
    const val HEADER_BYTES = PREFIX_BYTES + KEY_CHECK_BYTES

    private val HKDF_INFO = "avex backup v1".toByteArray(Charsets.US_ASCII)

    private val random = SecureRandom()

    /**
     * The password-derived key plus the parameters that produced it. [master] is secret; the salt
     * and iteration count go into every file's header in the clear.
     */
    class MasterKey(val master: ByteArray, val kdfSalt: ByteArray, val iterations: Int) {
        init {
            require(master.size == KEY_BYTES) { "master key must be $KEY_BYTES bytes" }
            require(kdfSalt.size == KDF_SALT_BYTES) { "salt must be $KDF_SALT_BYTES bytes" }
        }

        /** True when a file made under these KDF parameters can be opened with this key. */
        fun matches(header: Header): Boolean =
            iterations == header.iterations && MessageDigest.isEqual(kdfSalt, header.kdfSalt)

        /** Overwrite the secret. The object must not be used afterwards. */
        fun wipe() = master.fill(0)
    }

    /** The parsed, bounds-checked plaintext header of an encrypted backup. */
    class Header internal constructor(
        val iterations: Int,
        val kdfSalt: ByteArray,
        internal val fileSalt: ByteArray,
        internal val noncePrefix: ByteArray,
        internal val segmentSize: Int,
        internal val keyCheck: ByteArray,
        internal val bytes: ByteArray
    )

    sealed interface DecryptResult {
        data object Ok : DecryptResult
        /** The key check failed: the password (or cached key) is not the one this file was made with. */
        data object WrongPassword : DecryptResult
        /** The key was right but the content does not authenticate: damaged, truncated or edited. */
        data object Corrupt : DecryptResult
        /** The plaintext ran past the caller's limit. Nothing beyond the limit was written. */
        data object TooLarge : DecryptResult
    }

    // ── Keys ────────────────────────────────────────────────────────────────────────────────────

    /** A fresh master key for a new password. Slow on purpose (PBKDF2); call off the main thread. */
    fun newMasterKey(password: CharArray, iterations: Int = DEFAULT_ITERATIONS): MasterKey {
        val salt = ByteArray(KDF_SALT_BYTES).also(random::nextBytes)
        return MasterKey(pbkdf2(password, salt, iterations), salt, iterations)
    }

    /** Re-derive the master key a file was made with, from the password the user typed. Slow. */
    fun masterKeyFor(header: Header, password: CharArray): MasterKey =
        MasterKey(pbkdf2(password, header.kdfSalt, header.iterations), header.kdfSalt.copyOf(), header.iterations)

    /**
     * PBKDF2-HMAC-SHA256 over the password's UTF-8 bytes after Unicode NFC normalisation, so a
     * password typed with a composed "é" on one keyboard opens a backup made with a decomposed one
     * on another.
     */
    internal fun pbkdf2(password: CharArray, salt: ByteArray, iterations: Int): ByteArray {
        val normalized = Normalizer.normalize(java.nio.CharBuffer.wrap(password), Normalizer.Form.NFC).toCharArray()
        val spec = PBEKeySpec(normalized, salt, iterations, KEY_BYTES * 8)
        try {
            return SecretKeyFactory.getInstance("PBKDF2WithHmacSHA256").generateSecret(spec).encoded
        } finally {
            spec.clearPassword()
            normalized.fill('\u0000')
        }
    }

    /** HKDF-SHA256 (RFC 5869): [length] bytes of key material bound to [salt] and [info]. */
    internal fun hkdf(ikm: ByteArray, salt: ByteArray, info: ByteArray, length: Int): ByteArray {
        val prk = hmac(salt, ikm)
        val out = ByteArrayOutputStream(length)
        var t = ByteArray(0)
        var counter = 1
        while (out.size() < length) {
            t = hmac(prk, t + info + byteArrayOf(counter.toByte()))
            out.write(t)
            counter++
        }
        prk.fill(0)
        return out.toByteArray().copyOf(length)
    }

    private fun hmac(key: ByteArray, data: ByteArray): ByteArray =
        Mac.getInstance("HmacSHA256").run { init(SecretKeySpec(key, "HmacSHA256")); doFinal(data) }

    /** The file's GCM key and its key-check key, both derived from the master key and file salt. */
    private class FileKeys(val enc: ByteArray, val check: ByteArray) {
        fun wipe() { enc.fill(0); check.fill(0) }
    }

    private fun fileKeys(master: MasterKey, fileSalt: ByteArray): FileKeys {
        val okm = hkdf(master.master, fileSalt, HKDF_INFO, KEY_BYTES * 2)
        return FileKeys(okm.copyOfRange(0, KEY_BYTES), okm.copyOfRange(KEY_BYTES, KEY_BYTES * 2))
            .also { okm.fill(0) }
    }

    private fun keyCheck(checkKey: ByteArray, prefix: ByteArray): ByteArray =
        hmac(checkKey, prefix).copyOf(KEY_CHECK_BYTES)

    private fun nonce(prefix: ByteArray, index: Int, last: Boolean): ByteArray =
        ByteBuffer.allocate(NONCE_PREFIX_BYTES + 5)
            .put(prefix).putInt(index).put(if (last) 1 else 0)
            .array()

    // ── Format sniffing ─────────────────────────────────────────────────────────────────────────

    /** True when [file] starts with this container's magic, whatever state the rest is in. */
    fun isEncrypted(file: File): Boolean = runCatching {
        file.inputStream().use { ins ->
            val buf = ByteArray(MAGIC.size)
            DataInputStream(ins).readFully(buf)
            buf.contentEquals(MAGIC)
        }
    }.getOrDefault(false)

    /**
     * Read and validate the header from [input], leaving it positioned at the first segment.
     * Null for anything that is not a v1 container with sane parameters.
     */
    fun readHeader(input: InputStream): Header? {
        val bytes = ByteArray(HEADER_BYTES)
        try {
            DataInputStream(input).readFully(bytes)
        } catch (e: EOFException) {
            return null
        }
        val buf = ByteBuffer.wrap(bytes)
        val magic = ByteArray(MAGIC.size).also { buf.get(it) }
        if (!magic.contentEquals(MAGIC)) return null
        if (buf.get() != VERSION) return null
        if (buf.get() != KDF_PBKDF2_SHA256) return null
        val iterations = buf.int
        if (iterations !in MIN_ITERATIONS..MAX_ITERATIONS) return null
        val kdfSalt = ByteArray(KDF_SALT_BYTES).also { buf.get(it) }
        val fileSalt = ByteArray(FILE_SALT_BYTES).also { buf.get(it) }
        val noncePrefix = ByteArray(NONCE_PREFIX_BYTES).also { buf.get(it) }
        val segmentSize = buf.int
        if (segmentSize !in MIN_SEGMENT_SIZE..MAX_SEGMENT_SIZE) return null
        val keyCheck = ByteArray(KEY_CHECK_BYTES).also { buf.get(it) }
        return Header(iterations, kdfSalt, fileSalt, noncePrefix, segmentSize, keyCheck, bytes)
    }

    // ── Encrypt ─────────────────────────────────────────────────────────────────────────────────

    /**
     * An [OutputStream] that seals everything written to it into [out] under [key]. The header goes
     * out immediately; the final segment is written by [OutputStream.close], which also closes
     * [out]. A stream that is never closed leaves a file that will not open, never a readable one
     * missing its tail.
     */
    fun encryptingStream(out: OutputStream, key: MasterKey, segmentSize: Int = SEGMENT_SIZE): OutputStream {
        require(segmentSize in MIN_SEGMENT_SIZE..MAX_SEGMENT_SIZE)
        val fileSalt = ByteArray(FILE_SALT_BYTES).also(random::nextBytes)
        val noncePrefix = ByteArray(NONCE_PREFIX_BYTES).also(random::nextBytes)
        val keys = fileKeys(key, fileSalt)
        val prefix = ByteBuffer.allocate(PREFIX_BYTES)
            .put(MAGIC).put(VERSION).put(KDF_PBKDF2_SHA256).putInt(key.iterations)
            .put(key.kdfSalt).put(fileSalt).put(noncePrefix).putInt(segmentSize)
            .array()
        val header = prefix + keyCheck(keys.check, prefix)
        keys.check.fill(0)
        out.write(header)
        return EncryptingOutputStream(out, keys, header, noncePrefix, segmentSize)
    }

    private class EncryptingOutputStream(
        private val out: OutputStream,
        private val keys: FileKeys,
        private val header: ByteArray,
        private val noncePrefix: ByteArray,
        segmentSize: Int
    ) : OutputStream() {
        private val buf = ByteArray(segmentSize)
        private var pos = 0
        private var index = 0
        private var closed = false
        private val cipher = Cipher.getInstance("AES/GCM/NoPadding")
        private val keySpec = SecretKeySpec(keys.enc, "AES")

        override fun write(b: Int) = write(byteArrayOf(b.toByte()), 0, 1)

        override fun write(b: ByteArray, off: Int, len: Int) {
            check(!closed) { "stream closed" }
            var o = off
            var remaining = len
            while (remaining > 0) {
                // A full buffer is only sealed once more data arrives: until then it may be the
                // last segment, and the last one carries a different nonce.
                if (pos == buf.size) { seal(last = false); pos = 0 }
                val n = minOf(remaining, buf.size - pos)
                System.arraycopy(b, o, buf, pos, n)
                pos += n; o += n; remaining -= n
            }
        }

        override fun flush() = out.flush()

        override fun close() {
            if (closed) return
            closed = true
            try {
                seal(last = true)
                out.flush()
            } finally {
                buf.fill(0)
                keys.wipe()
                out.close()
            }
        }

        private fun seal(last: Boolean) {
            check(index != Int.MAX_VALUE) { "backup too large" }
            cipher.init(Cipher.ENCRYPT_MODE, keySpec, GCMParameterSpec(TAG_BITS, nonce(noncePrefix, index, last)))
            cipher.updateAAD(header)
            out.write(cipher.doFinal(buf, 0, pos))
            index++
        }
    }

    // ── Decrypt ─────────────────────────────────────────────────────────────────────────────────

    /** A segment failed to authenticate: the file is damaged, truncated or edited. */
    class IntegrityException : java.io.IOException("backup failed its integrity check")

    /** The key check failed: the password (or cached key) is not the one this file was made with. */
    class WrongKeyException : java.io.IOException("wrong backup password")

    /**
     * An [InputStream] of the plaintext that follows [header] in [input] (positioned just past the
     * header, as [readHeader] leaves it). Only authenticated bytes are ever returned: a segment that
     * fails raises [IntegrityException], and a file cut off early raises it at the point the missing
     * final segment should have been, never a clean end of stream. Throws [WrongKeyException] at
     * once when [key] is not the file's key. Closing it closes [input].
     */
    fun decryptingStream(header: Header, key: MasterKey, input: InputStream): InputStream {
        val keys = fileKeys(key, header.fileSalt)
        if (!MessageDigest.isEqual(keyCheck(keys.check, header.bytes.copyOf(PREFIX_BYTES)), header.keyCheck)) {
            keys.wipe()
            throw WrongKeyException()
        }
        return DecryptingInputStream(input, keys, header)
    }

    private class DecryptingInputStream(
        private val input: InputStream,
        private val keys: FileKeys,
        private val header: Header
    ) : InputStream() {
        private val chunk = header.segmentSize + TAG_BYTES
        private var cur = ByteArray(chunk)
        private var next = ByteArray(chunk)
        private var curLen = -1 // -1: nothing read yet
        private var index = 0
        private var plain = ByteArray(0)
        private var plainPos = 0
        private var done = false
        private val cipher = Cipher.getInstance("AES/GCM/NoPadding")
        private val keySpec = SecretKeySpec(keys.enc, "AES")

        override fun read(): Int {
            val one = ByteArray(1)
            return if (read(one, 0, 1) < 0) -1 else one[0].toInt() and 0xFF
        }

        override fun read(b: ByteArray, off: Int, len: Int): Int {
            if (len == 0) return 0
            while (plainPos == plain.size) {
                if (done) return -1
                openNextSegment()
            }
            val n = minOf(len, plain.size - plainPos)
            System.arraycopy(plain, plainPos, b, off, n)
            plainPos += n
            return n
        }

        private fun openNextSegment() {
            if (curLen < 0) curLen = readUpTo(input, cur)
            if (curLen < TAG_BYTES) throw IntegrityException()
            // Only a FULL chunk can have another after it; a short one must be the end.
            val nextLen = if (curLen == chunk) readUpTo(input, next) else 0
            val last = nextLen == 0
            plain.fill(0)
            plain = try {
                cipher.init(Cipher.DECRYPT_MODE, keySpec, GCMParameterSpec(TAG_BITS, nonce(header.noncePrefix, index, last)))
                cipher.updateAAD(header.bytes)
                cipher.doFinal(cur, 0, curLen)
            } catch (e: GeneralSecurityException) {
                throw IntegrityException()
            }
            plainPos = 0
            if (last) { done = true; return }
            if (index == Int.MAX_VALUE) throw IntegrityException()
            index++
            val t = cur; cur = next; next = t
            curLen = nextLen
        }

        override fun close() {
            plain.fill(0)
            keys.wipe()
            input.close()
        }
    }

    /**
     * Decrypt the segments that follow [header] in [input] into [output], writing at most
     * [maxBytes] of plaintext. [input] must be positioned just past the header (as [readHeader]
     * leaves it). Plaintext is only ever written for segments that authenticated, but a Corrupt
     * result can follow earlier good segments, so on anything but [DecryptResult.Ok] the caller
     * must discard what was written. Does not close [input].
     */
    fun decrypt(header: Header, key: MasterKey, input: InputStream, output: OutputStream, maxBytes: Long): DecryptResult {
        val plain = try {
            decryptingStream(header, key, NonClosing(input))
        } catch (e: WrongKeyException) {
            return DecryptResult.WrongPassword
        }
        return plain.use { stream -> copyAtMost(stream, output, maxBytes) }
    }

    private fun copyAtMost(stream: InputStream, output: OutputStream, maxBytes: Long): DecryptResult {
        val buf = ByteArray(64 * 1024)
        var total = 0L
        while (true) {
            val n = try {
                stream.read(buf)
            } catch (e: IntegrityException) {
                return DecryptResult.Corrupt
            }
            if (n < 0) return DecryptResult.Ok
            total += n
            if (total > maxBytes) return DecryptResult.TooLarge
            output.write(buf, 0, n)
        }
    }

    private class NonClosing(input: InputStream) : java.io.FilterInputStream(input) {
        override fun close() = Unit
    }

    /** Fill [buf] from [input] until it is full or the stream ends; the number of bytes read. */
    private fun readUpTo(input: InputStream, buf: ByteArray): Int {
        var n = 0
        while (n < buf.size) {
            val r = input.read(buf, n, buf.size - n)
            if (r < 0) break
            n += r
        }
        return n
    }
}
