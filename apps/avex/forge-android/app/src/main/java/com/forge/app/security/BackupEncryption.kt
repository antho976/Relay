package com.forge.app.security

import android.content.Context
import android.security.keystore.KeyGenParameterSpec
import android.security.keystore.KeyProperties
import android.util.AtomicFile
import com.forge.app.core.crypto.BackupCrypto
import dagger.Binds
import dagger.Module
import dagger.hilt.InstallIn
import dagger.hilt.android.qualifiers.ApplicationContext
import dagger.hilt.components.SingletonComponent
import java.io.ByteArrayInputStream
import java.io.ByteArrayOutputStream
import java.io.DataInputStream
import java.io.DataOutputStream
import java.io.File
import java.security.KeyStore
import javax.crypto.Cipher
import javax.crypto.KeyGenerator
import javax.crypto.SecretKey
import javax.crypto.spec.GCMParameterSpec
import javax.inject.Inject
import javax.inject.Singleton
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.withContext

/**
 * Wraps a small secret under a key this device holds and cannot export. The wrapped blob is useless
 * anywhere else, including on this phone after a reinstall.
 */
interface DeviceKeyWrapper {
    fun wrap(secret: ByteArray): ByteArray
    fun unwrap(blob: ByteArray): ByteArray
    /** Forget the device key; everything it wrapped becomes unreadable. */
    fun destroy()
}

/**
 * [DeviceKeyWrapper] on the Android Keystore: AES-256-GCM, generated inside the keystore (in
 * StrongBox where the phone has one), never leaving it.
 *
 * No user-authentication requirement: the weekly backup runs in the background with the screen off,
 * and the key being wrapped is itself only as strong as the backup password.
 */
class AndroidKeystoreKeyWrapper @Inject constructor() : DeviceKeyWrapper {

    private fun keyStore() = KeyStore.getInstance(ANDROID_KEYSTORE).apply { load(null) }

    private fun key(): SecretKey =
        (keyStore().getKey(ALIAS, null) as? SecretKey) ?: generate()

    private fun generate(): SecretKey {
        fun spec(strongBox: Boolean) = KeyGenParameterSpec.Builder(
            ALIAS, KeyProperties.PURPOSE_ENCRYPT or KeyProperties.PURPOSE_DECRYPT
        )
            .setBlockModes(KeyProperties.BLOCK_MODE_GCM)
            .setEncryptionPaddings(KeyProperties.ENCRYPTION_PADDING_NONE)
            .setKeySize(256)
            .apply { if (strongBox && android.os.Build.VERSION.SDK_INT >= 28) setIsStrongBoxBacked(true) }
            .build()
        val gen = KeyGenerator.getInstance(KeyProperties.KEY_ALGORITHM_AES, ANDROID_KEYSTORE)
        return try {
            gen.init(spec(strongBox = true))
            gen.generateKey()
        } catch (e: Exception) {
            // No StrongBox on this phone (StrongBoxUnavailableException, API 28+): the TEE keystore.
            gen.init(spec(strongBox = false))
            gen.generateKey()
        }
    }

    override fun wrap(secret: ByteArray): ByteArray {
        val cipher = Cipher.getInstance(TRANSFORMATION).apply { init(Cipher.ENCRYPT_MODE, key()) }
        val sealed = cipher.doFinal(secret)
        val iv = cipher.iv
        return byteArrayOf(iv.size.toByte()) + iv + sealed
    }

    override fun unwrap(blob: ByteArray): ByteArray {
        val ivLen = blob[0].toInt()
        require(ivLen in 12..16 && blob.size > 1 + ivLen) { "malformed wrapped key" }
        val key = keyStore().getKey(ALIAS, null) as? SecretKey ?: error("device key missing")
        val cipher = Cipher.getInstance(TRANSFORMATION)
        cipher.init(Cipher.DECRYPT_MODE, key, GCMParameterSpec(128, blob, 1, ivLen))
        return cipher.doFinal(blob, 1 + ivLen, blob.size - 1 - ivLen)
    }

    override fun destroy() {
        runCatching { keyStore().deleteEntry(ALIAS) }
    }

    private companion object {
        const val ANDROID_KEYSTORE = "AndroidKeyStore"
        const val ALIAS = "avex_backup_key_wrap"
        const val TRANSFORMATION = "AES/GCM/NoPadding"
    }
}

@Module
@InstallIn(SingletonComponent::class)
abstract class BackupEncryptionModule {
    @Binds abstract fun deviceKeyWrapper(impl: AndroidKeystoreKeyWrapper): DeviceKeyWrapper
}

/**
 * The optional backup password.
 *
 * The password itself is never stored. It is stretched once (PBKDF2, see [BackupCrypto]) into a
 * master key, and that key is kept wrapped by the device keystore in `noBackupFilesDir`, so the weekly
 * backup can seal a new file without asking for the password. The file lives outside every backup
 * this app writes and outside Android's own backup rules, so restoring a backup never brings a key
 * with it; the password setting belongs to this phone.
 *
 * On a new phone, or after a reinstall, the key is gone and a backup is opened by typing the password,
 * from which [BackupCrypto.masterKeyFor] re-derives the same key using the salt in the file's header.
 */
@Singleton
class BackupEncryption @Inject constructor(
    @ApplicationContext private val context: Context,
    private val wrapper: DeviceKeyWrapper
) {
    sealed interface State {
        /** No password: backups are plain ZIPs. */
        data object Off : State
        data class On(val key: BackupCrypto.MasterKey) : State
        /**
         * A password was set but its key can no longer be read (the keystore entry was lost or the
         * file damaged). Backups must not quietly go out unencrypted, so they fail until the user
         * sets the password again.
         */
        data object Unavailable : State
    }

    private val file get() = File(context.noBackupFilesDir, KEY_FILE)

    /** Whether a password is set, without touching the keystore. Cheap; fine for UI state. */
    fun isConfigured(): Boolean = file.exists() || File("${file.path}.bak").exists()

    fun state(): State {
        if (!isConfigured()) return State.Off
        return runCatching {
            DataInputStream(ByteArrayInputStream(AtomicFile(file).readFully())).use { input ->
                require(input.readByte() == FILE_VERSION) { "unknown key file version" }
                val iterations = input.readInt()
                val salt = ByteArray(input.readUnsignedByte()).also(input::readFully)
                val wrapped = ByteArray(input.readUnsignedShort()).also(input::readFully)
                State.On(BackupCrypto.MasterKey(wrapper.unwrap(wrapped), salt, iterations))
            }
        }.getOrElse { State.Unavailable }
    }

    /**
     * Set (or replace) the password. Slow by design: the key derivation is the point. Backups made
     * under an earlier password still need that password to open.
     */
    suspend fun setPassword(password: CharArray) = withContext(Dispatchers.Default) {
        val key = BackupCrypto.newMasterKey(password)
        try {
            val wrapped = wrapper.wrap(key.master)
            val bytes = ByteArrayOutputStream().also { bos ->
                DataOutputStream(bos).use { out ->
                    out.writeByte(FILE_VERSION.toInt())
                    out.writeInt(key.iterations)
                    out.writeByte(key.kdfSalt.size)
                    out.write(key.kdfSalt)
                    out.writeShort(wrapped.size)
                    out.write(wrapped)
                }
            }.toByteArray()
            withContext(Dispatchers.IO) {
                val atomic = AtomicFile(file)
                val stream = atomic.startWrite()
                try {
                    stream.write(bytes)
                    atomic.finishWrite(stream)
                } catch (e: Exception) {
                    atomic.failWrite(stream)
                    throw e
                }
            }
        } finally {
            key.wipe()
        }
    }

    /** Remove the password: later backups are plain again. Earlier encrypted ones stay encrypted. */
    suspend fun clear() = withContext(Dispatchers.IO) {
        AtomicFile(file).delete()
        wrapper.destroy()
    }

    companion object {
        private const val KEY_FILE = "backup_encryption.key"
        private const val FILE_VERSION: Byte = 1

        /** NIST SP 800-63B: at least 8 characters, no composition rules, long passphrases welcome. */
        const val MIN_PASSWORD_LENGTH = 8
        const val MAX_PASSWORD_LENGTH = 128
    }
}
