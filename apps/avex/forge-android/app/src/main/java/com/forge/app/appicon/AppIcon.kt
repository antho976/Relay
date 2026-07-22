package com.forge.app.appicon

import android.content.ComponentName
import android.content.Context
import android.content.pm.PackageManager
import android.os.Build
import androidx.annotation.DrawableRes
import com.forge.app.R
import dagger.hilt.android.qualifiers.ApplicationContext
import javax.inject.Inject
import javax.inject.Singleton

/** The effect/style family a launcher icon belongs to. Typed (not a raw string) so every `when` over
 *  it is exhaustive — adding a family is a compile error at each dispatch site instead of a silent
 *  fall-through to the plain wordmark / no scene. The name doubles as the picker's family header. */
enum class IconFamily { Forge, Solid, Metal, Stealth, Molten, Nebula, Aurora, Gem }

/**
 * A selectable home-screen launcher icon.
 *
 * Each entry maps 1:1 to an `<activity-alias android:name=".icon.<enum name>">` in the manifest —
 * the enum's own [name] IS the alias suffix, so the two can never drift. Exactly one alias is
 * enabled at a time (see [AppIconManager]); [Default] is the alias shipped enabled and reuses the
 * stock `@mipmap/ic_launcher` emblem, so a user who never picks keeps today's icon.
 *
 * Persisted by enum [name] (a stable string) rather than an R id — resource ids aren't stable
 * across builds, the same reason [PreferenceKeys.AVATAR_DEFAULT_ID] stores a name.
 */
enum class AppIcon(
    /** Short variant name shown under the tile ("Gold", "Rose gold"). */
    val label: String,
    /** Family the tile groups under; also the effect family the launch wordmark dispatches on. */
    val family: IconFamily,
    /** Full-bleed 512² preview art. [Default] instead composites the emblem foreground over its
     *  background colour in the picker (there's no flat bitmap of the adaptive default). */
    @param:DrawableRes val previewRes: Int,
    val isDefault: Boolean = false,
    /** The icon's full colour story, as `0xAARRGGBB` sky→horizon, for the cold-launch effect (see
     *  AvexIntro). null = no themed launch — the plain wordmark plays. A family shares an effect
     *  STYLE; each icon supplies its own PALETTE (first = deep sky wash, last = horizon glow, the
     *  ribbon sweeps through all of them). Kept as Longs so this model stays Compose-free. */
    val launchPalette: List<Long>? = null,
) {
    // Declaration order IS the picker order: the family headers come from
    // `entries.map { it.family }.distinct()` and each family's tiles keep their declaration order.
    // Kept in the design-reference order (Solid, Metal, Stealth, Molten, Nebula, Aurora, Gem), with
    // Default first so the revert-to-stock option always leads. Persistence is by [name], so this
    // order is free to change without migrating anyone's pick.
    Default("Default", IconFamily.Forge, R.drawable.ic_launcher_foreground, isDefault = true),
    SolidNavy("Navy", IconFamily.Solid, R.drawable.app_icon_solid_navy,
        launchPalette = listOf(0xFF283349, 0xFF3D4F73, 0xFF6B84AD)),
    SolidAmber("Amber", IconFamily.Solid, R.drawable.app_icon_solid_amber,
        launchPalette = listOf(0xFF9C6F20, 0xFFE6A532, 0xFFF5C562)),
    SolidPaper("Paper", IconFamily.Solid, R.drawable.app_icon_solid_paper,
        launchPalette = listOf(0xFFB8B4A8, 0xFFFAF9F6, 0xFFFFFFFF)),
    SolidEmber("Ember", IconFamily.Solid, R.drawable.app_icon_solid_ember,
        launchPalette = listOf(0xFF141414, 0xFFE6A532, 0xFFF5C562)),
    SolidPrism("Prism", IconFamily.Solid, R.drawable.app_icon_solid_prism,
        launchPalette = listOf(0xFF384867, 0xFFFAF9F6, 0xFFFFFFFF)),
    MetalGold("Gold", IconFamily.Metal, R.drawable.app_icon_metal_gold,
        launchPalette = listOf(0xFF342D1F, 0xFFD4AF57, 0xFFFFF7DC)),
    MetalChrome("Chrome", IconFamily.Metal, R.drawable.app_icon_metal_chrome,
        launchPalette = listOf(0xFF212631, 0xFF8E9AAC, 0xFFFFFFFF)),
    MetalRosegold("Rose gold", IconFamily.Metal, R.drawable.app_icon_metal_rosegold,
        launchPalette = listOf(0xFF2B2529, 0xFFD8A090, 0xFFFFFBF3)),
    MetalCopper("Copper", IconFamily.Metal, R.drawable.app_icon_metal_copper,
        launchPalette = listOf(0xFF2B231F, 0xFFC8845A, 0xFFFFE4B1)),
    MetalGunmetal("Gunmetal", IconFamily.Metal, R.drawable.app_icon_metal_gunmetal,
        launchPalette = listOf(0xFF21252B, 0xFF6E7A88, 0xFFEEF7FF)),
    StealthAmber("Amber", IconFamily.Stealth, R.drawable.app_icon_stealth_amber,
        launchPalette = listOf(0xFF171208, 0xFFD9A032, 0xFFFFDC6B)),
    StealthCrimson("Crimson", IconFamily.Stealth, R.drawable.app_icon_stealth_crimson,
        launchPalette = listOf(0xFF170A0E, 0xFFE0405E, 0xFFFF8BA3)),
    StealthCyan("Cyan", IconFamily.Stealth, R.drawable.app_icon_stealth_cyan,
        launchPalette = listOf(0xFF081517, 0xFF38C4DC, 0xFF90FFFF)),
    StealthViolet("Violet", IconFamily.Stealth, R.drawable.app_icon_stealth_violet,
        launchPalette = listOf(0xFF130A1A, 0xFFA85CE0, 0xFFF3C1FF)),
    MoltenEmber("Ember", IconFamily.Molten, R.drawable.app_icon_molten_ember,
        launchPalette = listOf(0xFF47220C, 0xFFE07820, 0xFFFFD98C)),
    MoltenPlasma("Plasma", IconFamily.Molten, R.drawable.app_icon_molten_plasma,
        launchPalette = listOf(0xFF331A4D, 0xFFA855E8, 0xFFEFCBFF)),
    MoltenCrimson("Crimson", IconFamily.Molten, R.drawable.app_icon_molten_crimson,
        launchPalette = listOf(0xFF451318, 0xFFC93038, 0xFFFFC9CB)),
    MoltenOcean("Ocean", IconFamily.Molten, R.drawable.app_icon_molten_ocean,
        launchPalette = listOf(0xFF122C48, 0xFF3E8FD6, 0xFFC8ECFB)),
    NebulaViolet("Violet", IconFamily.Nebula, R.drawable.app_icon_nebula_violet,
        launchPalette = listOf(0xFF301650, 0xFF7938B8, 0xFFF2E6FF)),
    NebulaTeal("Teal", IconFamily.Nebula, R.drawable.app_icon_nebula_teal,
        launchPalette = listOf(0xFF0C3C40, 0xFF209C99, 0xFFE2FFFB)),
    NebulaCrimson("Crimson", IconFamily.Nebula, R.drawable.app_icon_nebula_crimson,
        launchPalette = listOf(0xFF4A101E, 0xFFB62846, 0xFFFFE3E8)),
    NebulaAmber("Amber", IconFamily.Nebula, R.drawable.app_icon_nebula_amber,
        launchPalette = listOf(0xFF4D2F0B, 0xFFC07C22, 0xFFFFF3D8)),
    AuroraClassic("Classic", IconFamily.Aurora, R.drawable.app_icon_aurora_classic,
        launchPalette = listOf(0xFF6E63A8, 0xFF9A8FD0, 0xFFE0A34A)),
    AuroraNorthern("Northern", IconFamily.Aurora, R.drawable.app_icon_aurora_northern,
        launchPalette = listOf(0xFF145247, 0xFF2FA57E, 0xFF8FE0AC)),
    AuroraDusk("Dusk", IconFamily.Aurora, R.drawable.app_icon_aurora_dusk,
        launchPalette = listOf(0xFF6B3F7E, 0xFFC0619B, 0xFFE2743C)),
    AuroraDawn("Dawn", IconFamily.Aurora, R.drawable.app_icon_aurora_dawn,
        launchPalette = listOf(0xFF574B7E, 0xFFE87F9E, 0xFFE9C75F)),
    GemEmerald("Emerald", IconFamily.Gem, R.drawable.app_icon_gem_emerald,
        launchPalette = listOf(0xFF143325, 0xFF35B57A, 0xFFB9F0D4)),
    GemFrost("Frost", IconFamily.Gem, R.drawable.app_icon_gem_frost,
        launchPalette = listOf(0xFF1A3440, 0xFF6FC4E0, 0xFFF4FCFF)),
    GemHolo("Holo", IconFamily.Gem, R.drawable.app_icon_gem_holo,
        launchPalette = listOf(0xFF241A3D, 0xFFA060F0, 0xFFFFA8D8));

    /** Human name for the current-selection row: "Default", else "Nebula Violet". */
    val displayName: String get() = if (isDefault) label else "$family $label"

    companion object {
        /**
         * Namespace the aliases live under. The manifest's `.icon.*` names resolve against the module
         * NAMESPACE (`com.forge.app`), which differs from the applicationId (`com.quietsoftware.avex`,
         * `+.debug`), so the class name is namespace-based while [ComponentName]'s package comes from
         * the running context.
         */
        const val NAMESPACE: String = "com.forge.app"

        /** Persisted key → enum. Empty/unknown ⇒ [Default]. */
        fun fromKey(key: String): AppIcon = entries.firstOrNull { it.name == key } ?: Default

        /** Family headers in declaration order (Forge, Solid, Metal, Stealth, …), de-duped. */
        val families: List<IconFamily> = entries.map { it.family }.distinct()
    }
}

/**
 * Swaps the home-screen launcher icon by toggling which `.icon.*` activity-alias is enabled.
 *
 * There is no runtime "set app icon" API on Android — the supported approach is N launcher aliases,
 * exactly one enabled, flipped via [PackageManager.setComponentEnabledSetting]. We enable the target
 * FIRST, then disable the rest, so there's never an instant with zero enabled launcher components
 * (which drops the icon off the home screen). [PackageManager.DONT_KILL_APP] keeps the process alive;
 * the launcher redraws the icon shortly after — often only once the app is backgrounded, which is an
 * Android/OEM-launcher limitation, not something we can force from here.
 *
 * IMPORTANT: [reconcileTo]/[applyIcon] MUST run only when the USER has backgrounded the app (Home/
 * Recents), never while it's foreground OR merely covered by a sub-activity we launched (the system
 * photo picker, share sheet, export/file picker). Disabling the alias that launched the current task
 * tears the task down and closes the app on some OEMs (notably Samsung), even with
 * [PackageManager.DONT_KILL_APP]. The pick is persisted immediately (SettingsRepository); the swap is
 * deferred to [MainActivity]'s onStop, gated on onUserLeaveHint so overlays don't trigger it, and runs
 * [reconcileTo] SYNCHRONOUSLY — which is also when launchers redraw, so there's no UX cost.
 */
@Singleton
class AppIconManager @Inject constructor(
    @param:ApplicationContext private val context: Context,
) {
    /** The alias we've applied in this process, cached so the frequent background reconcile early-outs
     *  with a compare instead of rescanning every alias' component state on each app-background. Null
     *  until the first [reconcileTo]/[applyIcon]; a rescan then establishes the on-device truth. */
    @Volatile private var appliedIcon: AppIcon? = null

    /** The launcher alias currently enabled on the device. Nothing explicitly enabled ⇒ the manifest
     *  default ([AppIcon.Default]), so a user who never picked reads back as [AppIcon.Default]. */
    fun currentIcon(): AppIcon {
        val pm = context.packageManager
        return AppIcon.entries.firstOrNull {
            pm.getComponentEnabledSetting(componentFor(it)) ==
                PackageManager.COMPONENT_ENABLED_STATE_ENABLED
        } ?: AppIcon.Default
    }

    /**
     * Make [desired] the enabled launcher alias if it isn't already. MUST be called SYNCHRONOUSLY from a
     * background transition ([MainActivity.onStop]) — completing the [PackageManager] toggle inline (not
     * on a fire-and-forget coroutine) is what guarantees the swap lands before the OS can reap the
     * backgrounded process. A missed swap is invisible until noticed: the persisted pick already drives
     * the launch intro, so the icon silently lags the animation.
     */
    fun reconcileTo(desired: AppIcon) {
        if (appliedIcon == desired) return          // already applied this process — cheap no-op
        if (currentIcon() != desired) applyIcon(desired)
        appliedIcon = desired
    }

    fun applyIcon(icon: AppIcon) {
        val pm = context.packageManager
        // API 33+ toggles all aliases in ONE binder call (enable target, disable the rest atomically —
        // no window with zero enabled launcher components, no 30-IPC storm on the background transition).
        if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.TIRAMISU) {
            pm.setComponentEnabledSettings(
                AppIcon.entries.map { entry ->
                    PackageManager.ComponentEnabledSetting(
                        componentFor(entry),
                        if (entry == icon) PackageManager.COMPONENT_ENABLED_STATE_ENABLED
                        else PackageManager.COMPONENT_ENABLED_STATE_DISABLED,
                        PackageManager.DONT_KILL_APP,
                    )
                }
            )
        } else {
            // Legacy: enable the target FIRST so there's never an instant with zero enabled.
            pm.setComponentEnabledSetting(
                componentFor(icon),
                PackageManager.COMPONENT_ENABLED_STATE_ENABLED,
                PackageManager.DONT_KILL_APP,
            )
            AppIcon.entries.forEach { other ->
                if (other != icon) pm.setComponentEnabledSetting(
                    componentFor(other),
                    PackageManager.COMPONENT_ENABLED_STATE_DISABLED,
                    PackageManager.DONT_KILL_APP,
                )
            }
        }
        appliedIcon = icon
    }

    private fun componentFor(icon: AppIcon): ComponentName =
        // package = running applicationId (via context), class = namespace-qualified alias name.
        ComponentName(context, "${AppIcon.NAMESPACE}.icon.${icon.name}")
}
