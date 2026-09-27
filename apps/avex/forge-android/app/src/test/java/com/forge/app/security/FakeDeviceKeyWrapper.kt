package com.forge.app.security

/**
 * [DeviceKeyWrapper] for JVM tests, where there is no Android Keystore. It XORs with a fixed pad so
 * a test can still tell a wrapped blob from the secret, and [destroy] makes later unwraps fail the
 * way a lost keystore entry does.
 */
class FakeDeviceKeyWrapper : DeviceKeyWrapper {
    var destroyed = false
        private set

    private fun xor(bytes: ByteArray) = ByteArray(bytes.size) { (bytes[it].toInt() xor 0x5A).toByte() }

    override fun wrap(secret: ByteArray): ByteArray { destroyed = false; return xor(secret) }

    override fun unwrap(blob: ByteArray): ByteArray {
        check(!destroyed) { "device key missing" }
        return xor(blob)
    }

    override fun destroy() { destroyed = true }
}
