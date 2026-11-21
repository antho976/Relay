package com.forge.app.domain.photo

/**
 * The angle a progress photo is shot from. A photo carries at most one pose (stored as [name], or ""
 * for untagged), so the gallery can filter to a single view and compare the same angle over time.
 * Order here is the order the lens pills + tag chips render in.
 */
enum class PhotoPose(val label: String) {
    FRONT("Front"),
    BACK("Back"),
    SIDE("Side"),
    LEGS("Legs"),
    ARMS("Arms"),
    CHEST("Chest"),
    SHOULDERS("Shoulders"),
    ABS("Abs"),
    GLUTES("Glutes");

    companion object {
        /** Resolve a stored pose string to its enum, or null for untagged / unknown values. */
        fun fromKey(key: String): PhotoPose? = entries.firstOrNull { it.name == key }
    }
}
