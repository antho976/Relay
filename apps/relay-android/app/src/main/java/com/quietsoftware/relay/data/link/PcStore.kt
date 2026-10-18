package com.quietsoftware.relay.data.link

import android.content.Context
import androidx.datastore.preferences.core.edit
import androidx.datastore.preferences.core.stringPreferencesKey
import androidx.datastore.preferences.preferencesDataStore
import com.quietsoftware.relay.core.link.Link
import com.quietsoftware.relay.core.link.PcProfile
import com.quietsoftware.relay.core.wire.Greeting
import com.quietsoftware.relay.core.wire.Route
import com.quietsoftware.relay.core.wire.RoutePolicy
import com.quietsoftware.relay.core.wire.WakeTarget
import kotlinx.coroutines.flow.Flow
import kotlinx.coroutines.flow.first
import kotlinx.coroutines.flow.map
import kotlinx.serialization.Serializable
import kotlinx.serialization.encodeToString
import kotlinx.serialization.json.Json

private val Context.pcStore by preferencesDataStore(name = "pc")

/**
 * The paired PC: its name, routes and the phone's device id in a private DataStore, and the
 * device token sealed with a key that never leaves the phone's keystore. Excluded from backups
 * and device transfers (res/xml): a new phone pairs for itself.
 */
class PcStore(private val context: Context) : Link.ProfileSource {
    private val json = Json { ignoreUnknownKeys = true; encodeDefaults = true }

    @Serializable
    private data class Stored(
        val hostId: String,
        val name: String,
        val instance: String,
        val device: String,
        val routes: List<StoredRoute>,
        val policy: String = RoutePolicy.Auto.name,
        val lastRoute: String? = null,
        val wake: List<StoredWake> = emptyList(),
        val pairedAt: Long = 0,
        val version: String = "",
        val lastSeen: Long = 0,
    )

    @Serializable
    private data class StoredRoute(val url: String, val kind: String, val added: Boolean = false)

    @Serializable
    private data class StoredWake(val mac: String, val broadcast: String? = null)

    val profile: Flow<PcProfile?> = context.pcStore.data.map { prefs ->
        val stored = prefs[PROFILE]?.let { runCatching { json.decodeFromString<Stored>(it) }.getOrNull() } ?: return@map null
        val token = prefs[TOKEN]?.let { Seal.unseal(KEY_ALIAS, it) } ?: return@map null
        stored.toProfile(token)
    }

    /** When the link last reached the PC (epoch ms), for "last seen" on the PC card. */
    val lastSeen: Flow<Long> = context.pcStore.data.map { prefs ->
        prefs[PROFILE]?.let { runCatching { json.decodeFromString<Stored>(it) }.getOrNull() }?.lastSeen ?: 0L
    }

    override suspend fun current(): PcProfile? = profile.first()

    override suspend fun connected(profile: PcProfile, route: Route, greeting: Greeting) {
        context.pcStore.edit { prefs ->
            val stored = prefs[PROFILE]?.let { runCatching { json.decodeFromString<Stored>(it) }.getOrNull() } ?: return@edit
            prefs[PROFILE] = json.encodeToString(
                stored.copy(
                    name = greeting.host.ifEmpty { stored.name },
                    instance = greeting.instance.ifEmpty { stored.instance },
                    version = greeting.version,
                    lastRoute = route.url,
                    wake = if (greeting.wake.isNotEmpty()) greeting.wake.map { StoredWake(it.mac, it.broadcast) } else stored.wake,
                    lastSeen = System.currentTimeMillis(),
                ),
            )
        }
    }

    suspend fun save(profile: PcProfile) {
        context.pcStore.edit { prefs ->
            prefs[PROFILE] = json.encodeToString(storedFrom(profile))
            prefs[TOKEN] = Seal.seal(KEY_ALIAS, profile.token)
        }
    }

    suspend fun update(transform: (PcProfile) -> PcProfile) {
        val current = current() ?: return
        val next = transform(current)
        context.pcStore.edit { prefs ->
            val old = prefs[PROFILE]?.let { runCatching { json.decodeFromString<Stored>(it) }.getOrNull() }
            prefs[PROFILE] = json.encodeToString(storedFrom(next).copy(lastSeen = old?.lastSeen ?: 0))
        }
    }

    suspend fun forget() {
        context.pcStore.edit { it.clear() }
    }

    private fun Stored.toProfile(token: String) = PcProfile(
        hostId = hostId,
        name = name,
        instance = instance,
        device = device,
        token = token,
        routes = routes.map { Route(it.url, runCatching { Route.Kind.valueOf(it.kind) }.getOrDefault(Route.Kind.of(it.url))) },
        policy = runCatching { RoutePolicy.valueOf(policy) }.getOrDefault(RoutePolicy.Auto),
        lastRoute = lastRoute,
        wake = wake.map { WakeTarget(it.mac, it.broadcast) },
        pairedAt = pairedAt,
        version = version,
    )

    private companion object {
        val PROFILE = stringPreferencesKey("profile")
        val TOKEN = stringPreferencesKey("token")
        const val KEY_ALIAS = "relay-pc-token"

        fun storedFrom(p: PcProfile) = Stored(
            hostId = p.hostId,
            name = p.name,
            instance = p.instance,
            device = p.device,
            routes = p.routes.map { StoredRoute(it.url, it.kind.name) },
            policy = p.policy.name,
            lastRoute = p.lastRoute,
            wake = p.wake.map { StoredWake(it.mac, it.broadcast) },
            pairedAt = p.pairedAt,
            version = p.version,
        )
    }
}
