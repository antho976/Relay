package com.forge.app.core.crypto

import com.forge.app.core.crypto.BackupCrypto.DecryptResult
import org.junit.Assert.assertArrayEquals
import org.junit.Assert.assertEquals
import org.junit.Assert.assertFalse
import org.junit.Assert.assertNotNull
import org.junit.Assert.assertNull
import org.junit.Assert.assertTrue
import org.junit.Test
import java.io.ByteArrayInputStream
import java.io.ByteArrayOutputStream
import java.nio.ByteBuffer
import kotlin.random.Random

/**
 * The encrypted backup container, end to end on the JVM: what goes in comes out, and every way a
 * file can be wrong (password, bits, order, length, header) is refused rather than half-read.
 */
class BackupCryptoTest {

    // The floor, so the suite stays fast; the format stores the count, so nothing else changes.
    private val iterations = BackupCrypto.MIN_ITERATIONS
    private val key = BackupCrypto.newMasterKey("correct horse battery".toCharArray(), iterations)
    private val seg = 4096

    private fun seal(plain: ByteArray, k: BackupCrypto.MasterKey = key, segmentSize: Int = seg): ByteArray {
        val out = ByteArrayOutputStream()
        BackupCrypto.encryptingStream(out, k, segmentSize).use { it.write(plain) }
        return out.toByteArray()
    }

    private fun open(
        sealed: ByteArray,
        k: BackupCrypto.MasterKey = key,
        max: Long = Long.MAX_VALUE
    ): Pair<DecryptResult, ByteArray> {
        val input = ByteArrayInputStream(sealed)
        val header = BackupCrypto.readHeader(input) ?: error("header refused")
        val out = ByteArrayOutputStream()
        return BackupCrypto.decrypt(header, k, input, out, max) to out.toByteArray()
    }

    @Test fun everySizeAroundASegmentBoundaryRoundTrips() {
        for (size in listOf(0, 1, seg - 1, seg, seg + 1, 2 * seg, 2 * seg + 7, 5 * seg + 123)) {
            val plain = Random(size).nextBytes(size)
            val (result, back) = open(seal(plain))
            assertEquals("size $size", DecryptResult.Ok, result)
            assertArrayEquals("size $size", plain, back)
        }
    }

    @Test fun byteAtATimeWritesProduceTheSameStream() {
        val plain = Random(1).nextBytes(3 * seg + 5)
        val out = ByteArrayOutputStream()
        BackupCrypto.encryptingStream(out, key, seg).use { s -> plain.forEach { s.write(it.toInt()) } }
        val (result, back) = open(out.toByteArray())
        assertEquals(DecryptResult.Ok, result)
        assertArrayEquals(plain, back)
    }

    @Test fun theSamePlaintextNeverEncryptsTheSameWayTwice() {
        val plain = ByteArray(1000)
        assertFalse(seal(plain).contentEquals(seal(plain)))
    }

    @Test fun thePlaintextDoesNotAppearInTheFile() {
        val marker = "Bench press 100 kg".toByteArray()
        val plain = ByteArray(2 * seg) + marker + ByteArray(seg)
        val sealed = String(seal(plain), Charsets.ISO_8859_1)
        assertFalse(sealed.contains(String(marker, Charsets.ISO_8859_1)))
    }

    @Test fun aWrongPasswordIsReportedAsSuchNotAsDamage() {
        val sealed = seal(Random(2).nextBytes(10_000))
        val header = BackupCrypto.readHeader(ByteArrayInputStream(sealed))!!
        val wrong = BackupCrypto.masterKeyFor(header, "correct horse battery!".toCharArray())
        val (result, back) = open(sealed, wrong)
        assertEquals(DecryptResult.WrongPassword, result)
        assertEquals(0, back.size)
    }

    @Test fun theRightPasswordReDerivesTheKeyFromTheHeader() {
        val plain = Random(3).nextBytes(9_000)
        val sealed = seal(plain)
        val header = BackupCrypto.readHeader(ByteArrayInputStream(sealed))!!
        assertTrue(key.matches(header))
        val again = BackupCrypto.masterKeyFor(header, "correct horse battery".toCharArray())
        val (result, back) = open(sealed, again)
        assertEquals(DecryptResult.Ok, result)
        assertArrayEquals(plain, back)
    }

    @Test fun composedAndDecomposedAccentsAreTheSamePassword() {
        val composed = "café-lifts".toCharArray()
        val decomposed = "café-lifts".toCharArray()
        val salt = ByteArray(16) { it.toByte() }
        assertArrayEquals(
            BackupCrypto.pbkdf2(composed, salt, iterations),
            BackupCrypto.pbkdf2(decomposed, salt, iterations)
        )
    }

    @Test fun aFlippedBitAnywhereInTheBodyIsCorrupt() {
        val sealed = seal(Random(4).nextBytes(3 * seg))
        for (at in listOf(BackupCrypto.HEADER_BYTES, BackupCrypto.HEADER_BYTES + seg + 20, sealed.size - 1)) {
            val bad = sealed.copyOf().also { it[at] = (it[at].toInt() xor 0x01).toByte() }
            assertEquals("offset $at", DecryptResult.Corrupt, open(bad).first)
        }
    }

    @Test fun anEditedHeaderIsRefused() {
        val sealed = seal(Random(5).nextBytes(2 * seg))
        // Every byte after the magic, version and kdf id: the rest is either range-checked on parse
        // or bound into the key check and every segment's associated data.
        for (at in 10 until BackupCrypto.HEADER_BYTES) {
            val bad = sealed.copyOf().also { it[at] = (it[at].toInt() xor 0x01).toByte() }
            val input = ByteArrayInputStream(bad)
            val header = BackupCrypto.readHeader(input) ?: continue
            val result = BackupCrypto.decrypt(header, key, input, ByteArrayOutputStream(), Long.MAX_VALUE)
            assertTrue("offset $at gave $result", result != DecryptResult.Ok)
        }
    }

    @Test fun cuttingTheFileOffAtASegmentBoundaryIsCorrupt() {
        val sealed = seal(Random(6).nextBytes(3 * seg))
        val oneSegment = BackupCrypto.HEADER_BYTES + seg + 16
        assertEquals(DecryptResult.Corrupt, open(sealed.copyOf(oneSegment)).first)
        assertEquals(DecryptResult.Corrupt, open(sealed.copyOf(oneSegment * 2 - BackupCrypto.HEADER_BYTES)).first)
    }

    @Test fun cuttingTheFileOffMidSegmentIsCorrupt() {
        val sealed = seal(Random(7).nextBytes(3 * seg))
        assertEquals(DecryptResult.Corrupt, open(sealed.copyOf(sealed.size - 5)).first)
        assertEquals(DecryptResult.Corrupt, open(sealed.copyOf(BackupCrypto.HEADER_BYTES + 3)).first)
    }

    @Test fun anEmptyBodyIsCorruptNotAnEmptyBackup() {
        val sealed = seal(ByteArray(100))
        assertEquals(DecryptResult.Corrupt, open(sealed.copyOf(BackupCrypto.HEADER_BYTES)).first)
    }

    @Test fun trailingBytesAreCorrupt() {
        val sealed = seal(Random(8).nextBytes(seg))
        assertEquals(DecryptResult.Corrupt, open(sealed + byteArrayOf(0)).first)
        assertEquals(DecryptResult.Corrupt, open(sealed + ByteArray(seg + 16)).first)
    }

    @Test fun swappedSegmentsAreCorrupt() {
        val sealed = seal(Random(9).nextBytes(3 * seg))
        val h = BackupCrypto.HEADER_BYTES
        val c = seg + 16
        val swapped = sealed.copyOfRange(0, h) +
            sealed.copyOfRange(h + c, h + 2 * c) + sealed.copyOfRange(h, h + c) + sealed.copyOfRange(h + 2 * c, sealed.size)
        assertEquals(DecryptResult.Corrupt, open(swapped).first)
    }

    @Test fun theOutputCapStopsDecryption() {
        val plain = Random(10).nextBytes(4 * seg)
        val (result, back) = open(seal(plain), max = (2 * seg + 1).toLong())
        assertEquals(DecryptResult.TooLarge, result)
        assertTrue(back.size <= 2 * seg + 1)
    }

    @Test fun headersThatAskForAbsurdWorkAreRefused() {
        val sealed = seal(ByteArray(10))
        fun withIterations(n: Int) = sealed.copyOf().also { ByteBuffer.wrap(it).putInt(10, n) }
        assertNull(BackupCrypto.readHeader(ByteArrayInputStream(withIterations(BackupCrypto.MAX_ITERATIONS + 1))))
        assertNull(BackupCrypto.readHeader(ByteArrayInputStream(withIterations(1))))
        assertNotNull(BackupCrypto.readHeader(ByteArrayInputStream(sealed)))
    }

    @Test fun somethingElseIsNotAHeader() {
        assertNull(BackupCrypto.readHeader(ByteArrayInputStream("PK\u0003\u0004 not ours".toByteArray())))
        assertNull(BackupCrypto.readHeader(ByteArrayInputStream(ByteArray(3))))
    }

    /** RFC 7914 §11: PBKDF2-HMAC-SHA256, P = "passwd", S = "salt", c = 1, dkLen 64 (first 32 bytes). */
    @Test fun pbkdf2MatchesThePublishedVector() {
        val spec = javax.crypto.spec.PBEKeySpec("passwd".toCharArray(), "salt".toByteArray(), 1, 256)
        val dk = javax.crypto.SecretKeyFactory.getInstance("PBKDF2WithHmacSHA256").generateSecret(spec).encoded
        assertEquals("55ac046e56e3089fec1691c22544b605f94185216dde0465e68b9d57c20dacbc", dk.hex())
        // And the wrapper agrees with the raw call for an ASCII password.
        assertArrayEquals(dk, BackupCrypto.pbkdf2("passwd".toCharArray(), "salt".toByteArray(), 1))
    }

    /** RFC 5869 appendix A.1. */
    @Test fun hkdfMatchesThePublishedVector() {
        val okm = BackupCrypto.hkdf(
            ikm = ByteArray(22) { 0x0b },
            salt = ByteArray(13) { it.toByte() },
            info = ByteArray(10) { (0xf0 + it).toByte() },
            length = 42
        )
        assertEquals(
            "3cb25f25faacd57a90434f64d0362f2a2d2d0a90cf1a5a4c5db02d56ecc4c5bf34007208d5b887185865",
            okm.hex()
        )
    }

    private fun ByteArray.hex() = joinToString("") { "%02x".format(it) }
}
