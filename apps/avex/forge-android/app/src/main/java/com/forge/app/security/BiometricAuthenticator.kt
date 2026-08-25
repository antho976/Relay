package com.forge.app.security

import android.content.Context
import android.os.Build
import androidx.biometric.BiometricManager
import androidx.biometric.BiometricManager.Authenticators.BIOMETRIC_STRONG
import androidx.biometric.BiometricManager.Authenticators.BIOMETRIC_WEAK
import androidx.biometric.BiometricManager.Authenticators.DEVICE_CREDENTIAL
import androidx.biometric.BiometricPrompt
import androidx.core.content.ContextCompat
import androidx.fragment.app.FragmentActivity

/**
 * Thin wrapper over [BiometricPrompt] for the app / gallery lock (GYMAP-69). We authenticate against
 * the device's own biometric OR its screen lock (PIN/pattern/password) — no app-specific PIN is
 * stored, so there is no keystore, no [BiometricPrompt.CryptoObject], and no forgot-PIN flow. The OS
 * owns the credential.
 */
object BiometricAuthenticator {

    enum class Availability {
        AVAILABLE,
        NO_CREDENTIAL,
        UNAVAILABLE,
    }

    /**
     * Which authenticators the prompt accepts. `BIOMETRIC_STRONG or DEVICE_CREDENTIAL` is the clean
     * combination on API 30+; the pairing is rejected on API 28–29, where the library wants
     * `BIOMETRIC_WEAK or DEVICE_CREDENTIAL` instead. minSdk here is 26, so both branches ship.
     */
    private fun authenticators(): Int =
        if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.R) BIOMETRIC_STRONG or DEVICE_CREDENTIAL
        else BIOMETRIC_WEAK or DEVICE_CREDENTIAL

    /** True when the device has a credential the system can prompt against. */
    fun canAuthenticate(context: Context): Boolean =
        availability(context) == Availability.AVAILABLE

    fun availability(context: Context): Availability =
        availabilityForStatus(BiometricManager.from(context).canAuthenticate(authenticators()))

    internal fun availabilityForStatus(status: Int): Availability = when (status) {
        BiometricManager.BIOMETRIC_SUCCESS -> Availability.AVAILABLE
        BiometricManager.BIOMETRIC_ERROR_NONE_ENROLLED -> Availability.NO_CREDENTIAL
        else -> Availability.UNAVAILABLE
    }

    /**
     * Show the system unlock sheet. [onSuccess] fires on the main thread after a valid credential;
     * [onError] carries a terminal result — the user cancelled, a lockout, or no enrolled credential
     * — so the caller decides whether to stay locked, go back, or fail open. A single wrong fingerprint
     * ([BiometricPrompt.AuthenticationCallback.onAuthenticationFailed]) is transient (the sheet stays
     * up) and is intentionally ignored.
     */
    fun authenticate(
        activity: FragmentActivity,
        subtitle: String,
        onSuccess: () -> Unit,
        onError: (code: Int, message: CharSequence) -> Unit,
    ) {
        val executor = ContextCompat.getMainExecutor(activity)
        val prompt = BiometricPrompt(
            activity,
            executor,
            object : BiometricPrompt.AuthenticationCallback() {
                override fun onAuthenticationSucceeded(result: BiometricPrompt.AuthenticationResult) = onSuccess()
                override fun onAuthenticationError(errorCode: Int, errString: CharSequence) = onError(errorCode, errString)
            }
        )
        val info = BiometricPrompt.PromptInfo.Builder()
            .setTitle("Unlock Avex")
            .setSubtitle(subtitle)
            // Do NOT set a negative-button text: it is illegal when DEVICE_CREDENTIAL is allowed
            // (the device-PIN affordance replaces it).
            .setAllowedAuthenticators(authenticators())
            .build()
        prompt.authenticate(info)
    }
}
