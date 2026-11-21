package com.forge.app.domain.measurement

/**
 * The body measurements tracked on the Profile (GYMAP-52) — circumference readings the user logs
 * periodically, each with its own history. Order is display order (waist → chest → arms → thighs →
 * hips). [key] is the stable stored discriminator ([BodyMeasurementEntry.type]); [label] is the
 * user-facing name. A generous sane range in cm catches fat-finger entries without rejecting real
 * bodies (an arm and a waist share one range — the bound only guards against a slipped decimal).
 */
enum class BodyMeasurementType(val key: String, val label: String) {
    WAIST("waist", "Waist"),
    CHEST("chest", "Chest"),
    ARMS("arms", "Arms"),
    THIGHS("thighs", "Thighs"),
    HIPS("hips", "Hips");

    companion object {
        /** Resolve a stored [key] back to its type, or null for an unknown/legacy value. */
        fun fromKey(key: String): BodyMeasurementType? = entries.firstOrNull { it.key == key }

        /** Sane storage bounds (cm) — a circumference outside this is a mistyped entry, not a body. */
        const val MIN_CM = 5.0
        const val MAX_CM = 300.0
    }
}
