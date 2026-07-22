package com.forge.app.domain.health

/**
 * Which watch the user wears, chosen on the onboarding wearable step and changeable in
 * Settings → Recovery. Single source of truth for the canonical stored keys, the display
 * labels, and each brand's Health Connect source app, so onboarding and Recovery can't
 * drift on bare string literals.
 *
 * PURELY advisory — every Health Connect read is vendor-neutral and works the same
 * regardless of this choice. The brand only tailors setup pointers: which companion app
 * feeds Health Connect (Samsung Health for Galaxy, Fitbit for Pixel) and which signals
 * are known to vary by app version on that path.
 */
enum class WearableBrand(val key: String, val label: String, val sourceApp: String?) {
    GALAXY("galaxy", "Galaxy Watch", "Samsung Health"),
    PIXEL("pixel", "Pixel Watch", "Fitbit"),
    /** No watch, or any other brand that feeds Health Connect (Garmin, Whoop, a ring…). */
    NONE("none", "No watch · other", null);

    companion object {
        /** The brand for a stored key, or null if unset/unrecognized. */
        fun fromKey(key: String): WearableBrand? = entries.firstOrNull { it.key == key }
    }
}
