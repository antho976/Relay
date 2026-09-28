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
enum class IconFamily { Avex, Solid, Metal, Stealth, Molten, Nebula, Aurora, Gem, Gym }

/**
 * A selectable home-screen launcher icon.
 *
 * Every icon is the same Avex mark in its own colour, material and backdrop. The art is GENERATED
 * (`forge-android/tools/app-icons`): each icon is a real adaptive icon with a backdrop layer, a
 * floating mark layer and the shared monochrome mark, plus a flattened [previewRes] for the picker.
 *
 * Each entry maps 1:1 to an `<activity-alias android:name=".icon.<enum name>">` in the manifest —
 * the enum's own [name] IS the alias suffix, so the two can never drift. Exactly one alias is
 * enabled at a time (see [AppIconManager]); [Default] is the alias shipped enabled and reuses the
 * app's own `@mipmap/ic_launcher`, so a user who never picks keeps the house icon.
 *
 * Persisted by enum [name] (a stable string) rather than an R id — resource ids aren't stable
 * across builds, the same reason [PreferenceKeys.AVATAR_DEFAULT_ID] stores a name.
 */
enum class AppIcon(
    /** Short variant name shown under the tile ("Gold", "Chalk"). */
    val label: String,
    /** Family the tile groups under; also the effect family the launch wordmark dispatches on. */
    val family: IconFamily,
    /** The flattened icon as a launcher shows it (both layers, the visible 72dp square). */
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
    // Kept in step with tools/app-icons/icons.mjs. Persistence is by [name], so this order is free to
    // change without migrating anyone's pick.
    Default("Pearl", IconFamily.Avex, R.drawable.app_icon_default, isDefault = true),
    AvexSignal("Signal", IconFamily.Avex, R.drawable.app_icon_avex_signal),
    SolidRed("Red", IconFamily.Solid, R.drawable.app_icon_solid_red,
        launchPalette = listOf(0xFF6E1414, 0xFFD93636, 0xFFFF8A8A)),
    SolidEmber("Ember", IconFamily.Solid, R.drawable.app_icon_solid_ember,
        launchPalette = listOf(0xFF5A2E0A, 0xFFD4761F, 0xFFF5B266)),
    SolidGold("Gold", IconFamily.Solid, R.drawable.app_icon_solid_gold,
        launchPalette = listOf(0xFF3A2E14, 0xFF8C7340, 0xFFE6CD8F)),
    SolidOlive("Olive", IconFamily.Solid, R.drawable.app_icon_solid_olive,
        launchPalette = listOf(0xFF1F2A19, 0xFF4D6040, 0xFFA9BE95)),
    SolidNavy("Navy", IconFamily.Solid, R.drawable.app_icon_solid_navy,
        launchPalette = listOf(0xFF283349, 0xFF3D4F73, 0xFF6B84AD)),
    SolidPaper("Paper", IconFamily.Solid, R.drawable.app_icon_solid_paper,
        launchPalette = listOf(0xFFB8B4A8, 0xFFFAF9F6, 0xFFFFFFFF)),
    MetalGold("Gold", IconFamily.Metal, R.drawable.app_icon_metal_gold,
        launchPalette = listOf(0xFF342D1F, 0xFFD4AF57, 0xFFFFF7DC)),
    MetalChrome("Chrome", IconFamily.Metal, R.drawable.app_icon_metal_chrome,
        launchPalette = listOf(0xFF212631, 0xFF8E9AAC, 0xFFFFFFFF)),
    MetalCopper("Copper", IconFamily.Metal, R.drawable.app_icon_metal_copper,
        launchPalette = listOf(0xFF2B231F, 0xFFC8845A, 0xFFFFE4B1)),
    StealthCrimson("Crimson", IconFamily.Stealth, R.drawable.app_icon_stealth_crimson,
        launchPalette = listOf(0xFF170A0E, 0xFFE0405E, 0xFFFF8BA3)),
    StealthCyan("Cyan", IconFamily.Stealth, R.drawable.app_icon_stealth_cyan,
        launchPalette = listOf(0xFF081517, 0xFF38C4DC, 0xFF90FFFF)),
    MoltenEmber("Ember", IconFamily.Molten, R.drawable.app_icon_molten_ember,
        launchPalette = listOf(0xFF47220C, 0xFFE07820, 0xFFFFD98C)),
    MoltenPlasma("Plasma", IconFamily.Molten, R.drawable.app_icon_molten_plasma,
        launchPalette = listOf(0xFF331A4D, 0xFFA855E8, 0xFFEFCBFF)),
    NebulaViolet("Violet", IconFamily.Nebula, R.drawable.app_icon_nebula_violet,
        launchPalette = listOf(0xFF301650, 0xFF7938B8, 0xFFF2E6FF)),
    NebulaTeal("Teal", IconFamily.Nebula, R.drawable.app_icon_nebula_teal,
        launchPalette = listOf(0xFF0C3C40, 0xFF209C99, 0xFFE2FFFB)),
    AuroraNorthern("Northern", IconFamily.Aurora, R.drawable.app_icon_aurora_northern,
        launchPalette = listOf(0xFF145247, 0xFF2FA57E, 0xFF8FE0AC)),
    AuroraDusk("Dusk", IconFamily.Aurora, R.drawable.app_icon_aurora_dusk,
        launchPalette = listOf(0xFF6B3F7E, 0xFFC0619B, 0xFFE2743C)),
    GemEmerald("Emerald", IconFamily.Gem, R.drawable.app_icon_gem_emerald,
        launchPalette = listOf(0xFF143325, 0xFF35B57A, 0xFFB9F0D4)),
    GemHolo("Holo", IconFamily.Gem, R.drawable.app_icon_gem_holo,
        launchPalette = listOf(0xFF241A3D, 0xFFA060F0, 0xFFFFA8D8)),
    GymChalk("Chalk", IconFamily.Gym, R.drawable.app_icon_gym_chalk,
        launchPalette = listOf(0xFF1C1F22, 0xFFD8D4CB, 0xFFFFFFFF)),
    GymIron("Iron", IconFamily.Gym, R.drawable.app_icon_gym_iron,
        launchPalette = listOf(0xFF161719, 0xFF8A8C91, 0xFFEDEAE3));

    /**
     * The icon's own colour as an accent ("#RRGGBB"): the middle of [launchPalette], which is the
     * mark's body colour (the first is the deep backdrop wash, the last the highlight glow). null
     * for the house icons, which have no colour of their own — "match accent to icon" then keeps
     * the accent the user picked.
     */
    val accentHex: String?
        get() = launchPalette?.let { p -> iconAccent(p[1], p.last()) }?.let { "#%06X".format(it) }

    /** Human name for the current-selection row: "Avex Pearl", "Nebula Violet". */
    val displayName: String get() = "$family $label"

    companion object {
        /**
         * Namespace the aliases live under. The manifest's `.icon.*` names resolve against the module
         * NAMESPACE (`com.forge.app`), which differs from the applicationId (`com.quietsoftware.avex`,
         * `+.debug`), so the class name is namespace-based while [ComponentName]'s package comes from
         * the running context.
         */
        const val NAMESPACE: String = "com.forge.app"

        /**
         * Icons cut in the 2026-09-26 redesign, each mapped to the nearest survivor. Their manifest
         * aliases stay (disabled, drawn with the survivor's art): a user whose ENABLED alias vanished
         * from the manifest would have no launcher entry at all after updating, so the old alias keeps
         * the app on the home screen until the next background swap moves them to the survivor.
         * Never delete a name from here or its alias from the manifest.
         */
        val RETIRED: Map<String, AppIcon> = mapOf(
            "SolidAmber" to SolidEmber,
            "SolidPrism" to SolidNavy,
            "MetalRosegold" to MetalCopper,
            "MetalGunmetal" to MetalChrome,
            "StealthAmber" to StealthCrimson,
            "StealthViolet" to StealthCyan,
            "MoltenCrimson" to MoltenEmber,
            "MoltenOcean" to MoltenPlasma,
            "NebulaCrimson" to NebulaViolet,
            "NebulaAmber" to NebulaViolet,
            "AuroraClassic" to AuroraDusk,
            "AuroraDawn" to AuroraDusk,
            "GemFrost" to GemEmerald,
        )

        /** Persisted key → enum. A retired key resolves to its survivor; empty/unknown ⇒ [Default]. */
        fun fromKey(key: String): AppIcon =
            entries.firstOrNull { it.name == key } ?: RETIRED[key] ?: Default

        /** Family headers in declaration order (Avex, Solid, Metal, Stealth, …), de-duped. */
        val families: List<IconFamily> = entries.map { it.family }.distinct()
    }
}

/** The app background the accent draws on (Pearl), as RGB. */
private const val ACCENT_BACKDROP_RGB = 0x110F0CL

/** Accent text and highlights must read on the background: WCAG's 3:1 for large text and UI marks. */
private const val ACCENT_MIN_CONTRAST = 3.0

/**
 * An icon's body colour, lifted toward its own highlight [glow] only as far as it takes to read on
 * the dark background. A navy or violet mark is a fine icon and an unreadable accent (a nav label in
 * #3D4F73 on near-black measures about 2.4:1), so the dark icons borrow some of their own glow
 * rather than a colour from outside the icon. Bright icons come back unchanged. Returns RGB.
 */
internal fun iconAccent(body: Long, glow: Long): Long {
    fun channel(c: Long, shift: Int) = ((c shr shift) and 0xFF).toDouble()
    fun luminance(rgb: Long): Double {
        fun lin(v: Double): Double = (v / 255).let { if (it <= 0.04045) it / 12.92 else Math.pow((it + 0.055) / 1.055, 2.4) }
        return 0.2126 * lin(channel(rgb, 16)) + 0.7152 * lin(channel(rgb, 8)) + 0.0722 * lin(channel(rgb, 0))
    }
    val backdrop = luminance(ACCENT_BACKDROP_RGB)
    fun mix(t: Double): Long {
        fun ch(shift: Int) = Math.round(channel(body, shift) + (channel(glow, shift) - channel(body, shift)) * t)
        return (ch(16) shl 16) or (ch(8) shl 8) or ch(0)
    }
    for (step in 0..10) {
        val rgb = mix(step / 10.0)
        if ((luminance(rgb) + 0.05) / (backdrop + 0.05) >= ACCENT_MIN_CONTRAST) return rgb
    }
    return glow and 0xFFFFFF
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
        val enabled = { alias: String ->
            pm.getComponentEnabledSetting(componentFor(alias)) == PackageManager.COMPONENT_ENABLED_STATE_ENABLED
        }
        return AppIcon.entries.firstOrNull { enabled(it.name) }
            ?: AppIcon.RETIRED.entries.firstOrNull { enabled(it.key) }?.value
            ?: AppIcon.Default
    }

    /** True while a retired alias is still the enabled launcher entry (a pick from before a redesign). */
    private fun retiredAliasEnabled(): Boolean {
        val pm = context.packageManager
        return AppIcon.RETIRED.keys.any {
            pm.getComponentEnabledSetting(componentFor(it)) == PackageManager.COMPONENT_ENABLED_STATE_ENABLED
        }
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
        // A retired alias reads back as its survivor, so check it explicitly: it must still be swapped
        // off for the survivor's own alias even when the two "match".
        if (currentIcon() != desired || retiredAliasEnabled()) applyIcon(desired)
        appliedIcon = desired
    }

    fun applyIcon(icon: AppIcon) {
        val pm = context.packageManager
        // API 33+ toggles all aliases in ONE binder call (enable target, disable the rest atomically —
        // no window with zero enabled launcher components, no 30-IPC storm on the background transition).
        if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.TIRAMISU) {
            pm.setComponentEnabledSettings(
                allAliases().map { alias ->
                    PackageManager.ComponentEnabledSetting(
                        componentFor(alias),
                        if (alias == icon.name) PackageManager.COMPONENT_ENABLED_STATE_ENABLED
                        else PackageManager.COMPONENT_ENABLED_STATE_DISABLED,
                        PackageManager.DONT_KILL_APP,
                    )
                }
            )
        } else {
            // Legacy: enable the target FIRST so there's never an instant with zero enabled.
            pm.setComponentEnabledSetting(
                componentFor(icon.name),
                PackageManager.COMPONENT_ENABLED_STATE_ENABLED,
                PackageManager.DONT_KILL_APP,
            )
            allAliases().forEach { other ->
                if (other != icon.name) pm.setComponentEnabledSetting(
                    componentFor(other),
                    PackageManager.COMPONENT_ENABLED_STATE_DISABLED,
                    PackageManager.DONT_KILL_APP,
                )
            }
        }
        appliedIcon = icon
    }

    /** Every launcher alias in the manifest: the live icons, then the retired ones (always disabled). */
    private fun allAliases(): List<String> = AppIcon.entries.map { it.name } + AppIcon.RETIRED.keys

    private fun componentFor(alias: String): ComponentName =
        // package = running applicationId (via context), class = namespace-qualified alias name.
        ComponentName(context, "${AppIcon.NAMESPACE}.icon.$alias")
}
