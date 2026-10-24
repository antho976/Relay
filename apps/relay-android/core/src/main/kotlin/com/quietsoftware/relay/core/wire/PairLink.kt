package com.quietsoftware.relay.core.wire

import java.net.URLDecoder

/**
 * What the phone scans: `relay://pair?v=1&code=…&host=…&id=…&instance=…&direct=…&via=…`
 * (crates/relay-remote/src/pairlink.rs). `direct` is a comma-joined list of `ws://` addresses
 * on the PC's networks; `via` is the join address of the person's own rendezvous, if any.
 */
data class PairLink(
    val code: String,
    val host: String,
    val hostId: String,
    val instance: String,
    val direct: List<String>,
    val via: String?,
) {
    /** Direct addresses first: they keep the lines on the person's own network. */
    val routes: List<Route> get() = direct.map { Route(it, Route.Kind.of(it)) } + listOfNotNull(via?.let { Route(it, Route.Kind.Rendezvous) })

    companion object {
        fun parse(text: String): PairLink? {
            val trimmed = text.trim()
            if (!trimmed.startsWith("relay://pair?")) return null
            val query = trimmed.substringAfter('?')
            // A link cut mid-escape when copied (`%3`) or a bad `%` in a scanned code is no link.
            val params = query.split('&').mapNotNull { part ->
                val eq = part.indexOf('=')
                if (eq <= 0) null else part.substring(0, eq) to (decode(part.substring(eq + 1)) ?: return null)
            }.toMap()
            if (params["v"] != "1") return null
            val code = params["code"]?.takeIf { it.isNotBlank() } ?: return null
            val direct = params["direct"].orEmpty().split(',').map { it.trim() }.filter { it.startsWith("ws://") || it.startsWith("wss://") }
            val via = params["via"]?.takeIf { it.startsWith("ws://") || it.startsWith("wss://") }
            if (direct.isEmpty() && via == null) return null
            return PairLink(
                code = code,
                host = params["host"].orEmpty(),
                hostId = params["id"].orEmpty(),
                instance = params["instance"].orEmpty(),
                direct = direct,
                via = via,
            )
        }

        /** Typed by hand: an address the PC printed and its eight-character code. */
        fun manual(address: String, code: String): PairLink? {
            val url = address(address) ?: return null
            if (code.isBlank()) return null
            return PairLink(code.trim(), host = "", hostId = "", instance = "", direct = listOf(url), via = null)
        }

        /** `192.168.1.20`, `host:port`, `ws(s)://…` or `http(s)://…`, as a WebSocket address. */
        fun address(raw: String): String? {
            val s = raw.trim().trimEnd('/')
            if (s.isEmpty() || s.any { it.isWhitespace() }) return null
            return when {
                s.startsWith("ws://") || s.startsWith("wss://") -> s
                s.startsWith("http://") -> "ws://" + s.removePrefix("http://")
                s.startsWith("https://") -> "wss://" + s.removePrefix("https://")
                hasPort(s) -> "ws://$s"
                else -> "ws://$s:${Wire.DEFAULT_PORT}"
            }
        }

        private fun hasPort(s: String): Boolean {
            // `[::1]:7420`, `host:7420`; a bare IPv6 address has several colons and no brackets.
            if (s.startsWith("[")) return s.substringAfter(']', "").startsWith(":")
            return s.count { it == ':' } == 1 && s.substringAfter(':').all { it.isDigit() }
        }

        private fun decode(s: String): String? = runCatching { URLDecoder.decode(s.replace("+", "%2B"), Charsets.UTF_8) }.getOrNull()
    }
}

/** One way to reach a PC. */
data class Route(val url: String, val kind: Kind) {
    enum class Kind {
        /** A private LAN address: the phone is on the same network. */
        Lan,

        /** A Tailscale address (100.64.0.0/10 or a `.ts.net` name): end-to-end encrypted, from anywhere. */
        Tailscale,

        /** Any other direct address the person typed. */
        Other,

        /** The person's own rendezvous server, which can read the lines (docs/MOBILE.md §6). */
        Rendezvous;

        companion object {
            fun of(url: String): Kind {
                val host = url.substringAfter("://").substringBefore('/').let {
                    if (it.startsWith("[")) it.substringAfter('[').substringBefore(']') else it.substringBeforeLast(':', it)
                }
                if (host.endsWith(".ts.net")) return Tailscale
                val parts = host.split('.').mapNotNull { it.toIntOrNull() }
                if (parts.size == 4) {
                    val (a, b) = parts
                    if (a == 100 && b in 64..127) return Tailscale
                    if (a == 10 || (a == 172 && b in 16..31) || (a == 192 && b == 168) || a == 127) return Lan
                }
                if (host.endsWith(".local") || host == "localhost") return Lan
                return Other
            }
        }
    }

    val private: Boolean get() = kind != Kind.Rendezvous
}

/** Which routes a PC card lets the link try (docs/MOBILE.md §5, "Routes"). */
enum class RoutePolicy { Auto, DirectOnly, ServerOnly }

/** The order routes are tried in: on the home network first, Tailscale next, the server last. */
fun orderRoutes(routes: List<Route>, policy: RoutePolicy, preferred: String? = null): List<Route> {
    val allowed = routes.distinctBy { it.url }.filter {
        when (policy) {
            RoutePolicy.Auto -> true
            RoutePolicy.DirectOnly -> it.kind != Route.Kind.Rendezvous
            RoutePolicy.ServerOnly -> it.kind == Route.Kind.Rendezvous
        }
    }
    val rank = { r: Route ->
        when (r.kind) {
            Route.Kind.Lan -> 0
            Route.Kind.Other -> 1
            Route.Kind.Tailscale -> 2
            Route.Kind.Rendezvous -> 3
        }
    }
    // The route that worked last time leads its own rank, so a phone at home does not wait on a
    // stale LAN address from another network before reaching the one that answers.
    return allowed.sortedWith(compareBy({ rank(it) }, { if (it.url == preferred) 0 else 1 }))
}
