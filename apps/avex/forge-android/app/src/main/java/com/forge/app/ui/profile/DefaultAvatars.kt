package com.forge.app.ui.profile

import androidx.annotation.DrawableRes
import com.forge.app.R

/**
 * The bundled cover photos a user can pick instead of their own (GYMAP-22). The source art ships as
 * `.avif` in the repo; it's converted to `drawable-nodpi` WebP at build time (WebP decodes natively
 * from minSdk 26, AVIF would not below API 31). Picking one bakes it into the ordinary `avatar.jpg`
 * via [com.forge.app.data.repo.AvatarRepository.setFromResource], so nothing downstream changes.
 *
 * Each item's [key] is a stable string persisted in prefs (R ids shift across builds) so the picker
 * can ring the active default; category labels are human, never the raw file prefix (DESIGN §11).
 */
object DefaultAvatars {

    data class Item(val key: String, @DrawableRes val resId: Int)

    data class Category(val label: String, val items: List<Item>)

    val categories: List<Category> = listOf(
        Category(
            "Dark scenery",
            listOf(
                Item("scenery_1", R.drawable.avatar_default_scenery_1),
                Item("scenery_2", R.drawable.avatar_default_scenery_2),
                Item("scenery_3", R.drawable.avatar_default_scenery_3),
                Item("scenery_4", R.drawable.avatar_default_scenery_4),
                Item("scenery_5", R.drawable.avatar_default_scenery_5),
            )
        ),
        Category(
            "Mountains",
            listOf(
                Item("mountain_1", R.drawable.avatar_default_mountain_1),
                Item("mountain_2", R.drawable.avatar_default_mountain_2),
                Item("mountain_3", R.drawable.avatar_default_mountain_3),
                Item("mountain_4", R.drawable.avatar_default_mountain_4),
                Item("mountain_5", R.drawable.avatar_default_mountain_5),
                Item("mountain_6", R.drawable.avatar_default_mountain_6),
            )
        ),
        Category(
            "Gym",
            listOf(
                Item("gym_1", R.drawable.avatar_default_gym_1),
                Item("gym_2", R.drawable.avatar_default_gym_2),
                Item("gym_3", R.drawable.avatar_default_gym_3),
                Item("gym_4", R.drawable.avatar_default_gym_4),
            )
        ),
        Category(
            "Industrial",
            listOf(
                Item("industrial_1", R.drawable.avatar_default_industrial_1),
                Item("industrial_2", R.drawable.avatar_default_industrial_2),
                Item("industrial_3", R.drawable.avatar_default_industrial_3),
                Item("industrial_4", R.drawable.avatar_default_industrial_4),
            )
        ),
        Category(
            "Trees",
            listOf(
                Item("tree_1", R.drawable.avatar_default_tree_1),
                Item("tree_2", R.drawable.avatar_default_tree_2),
            )
        ),
    )

    /** Flat view of every default, in category order. */
    val all: List<Item> = categories.flatMap { it.items }

    fun byKey(key: String?): Item? = key?.let { k -> all.firstOrNull { it.key == k } }
}
