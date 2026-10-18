package com.quietsoftware.relay.data.link

import android.content.Context
import androidx.datastore.preferences.core.edit
import androidx.datastore.preferences.core.stringPreferencesKey
import androidx.datastore.preferences.preferencesDataStore
import com.quietsoftware.relay.core.link.Link
import com.quietsoftware.relay.core.link.PcProfile
import com.quietsoftware.relay.core.wire.Greeting
import com.quietsoftware.relay.core.wire.Route
import kotlinx.coroutines.flow.Flow
import kotlinx.coroutines.flow.first
import kotlinx.coroutines.flow.map
import kotlinx.serialization.Serializable
import kotlinx.serialization.encodeToString
import kotlinx.serialization.json.Json

private val Context.cloudStore by preferencesDataStore(name = "cloud")

/**
 * The server's copy this phone paired with (docs/CLOUD.md), kept as the PC is ([PcStore]): its
 * name and routes in a DataStore of its own, the device token sealed under its own keystore key.
 * Pairing or forgetting it never touches the PC's pairing or the phone's copy of the PC's data.
 */
class CloudStore(private val context: Context) : Link.ProfileSource {
    private val json = Json { ignoreUnknownKeys = true; encodeDefaults = true }

    @Serializable
    private data class Stored(
        val hostId: String,
        val name: String,
        val instance: String,
        val device: String,
        val routes: List<StoredRoute>,
        val lastRoute: String? = null,
        val pairedAt: Long = 0,
        val version: String = "",
        val lastSeen: Long = 0,
    )

    @Serializable
    private data class StoredRoute(val url: String, val kind: String)

    private fun Stored.toProfile(token: String) = PcProfile(
        hostId = hostId,
        name = name,
        instance = instance,
        device = device,
        token = token,
        routes = routes.map { Route(it.url, runCatching { Route.Kind.valueOf(it.kind) }.getOrDefault(Route.Kind.of(it.url))) },
        lastRoute = lastRoute,
        pairedAt = pairedAt,
        version = version,
    )

    private fun decode(text: String?): Stored? = text?.let { runCatching { json.decodeFromString<Stored>(it) }.getOrNull() }

    val profile: Flow<PcProfile?> = context.cloudStore.data.map { prefs ->
        val stored = decode(prefs[PROFILE]) ?: return@map null
        val token = prefs[TOKEN]?.let { Seal.unseal(KEY_ALIAS, it) } ?: return@map null
        stored.toProfile(token)
    }

    /** When the link last reached the server's copy (epoch ms). */
    val lastSeen: Flow<Long> = context.cloudStore.data.map { prefs -> decode(prefs[PROFILE])?.lastSeen ?: 0L }

    override suspend fun current(): PcProfile? = profile.first()

    override suspend fun connected(profile: PcProfile, route: Route, greeting: Greeting) {
        context.cloudStore.edit { prefs ->
            val stored = decode(prefs[PROFILE]) ?: return@edit
            prefs[PROFILE] = json.encodeToString(
                stored.copy(
                    name = greeting.host.ifEmpty { stored.name },
                    version = greeting.version,
                    lastRoute = route.url,
                    lastSeen = System.currentTimeMillis(),
                ),
            )
        }
    }

    suspend fun save(profile: PcProfile) {
        context.cloudStore.edit { prefs ->
            prefs[PROFILE] = json.encodeToString(
                Stored(
                    hostId = profile.hostId,
                    name = profile.name,
                    instance = profile.instance,
                    device = profile.device,
                    routes = profile.routes.map { StoredRoute(it.url, it.kind.name) },
                    lastRoute = profile.lastRoute,
                    pairedAt = profile.pairedAt,
                    version = profile.version,
                ),
            )
            prefs[TOKEN] = Seal.seal(KEY_ALIAS, profile.token)
        }
    }

    suspend fun forget() {
        context.cloudStore.edit { it.clear() }
    }

    private companion object {
        val PROFILE = stringPreferencesKey("profile")
        val TOKEN = stringPreferencesKey("token")
        const val KEY_ALIAS = "relay-cloud-token"
    }
}
