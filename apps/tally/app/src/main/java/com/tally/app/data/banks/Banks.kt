package com.tally.app.data.banks

import com.tally.app.BuildConfig
import com.tally.app.data.sync.RelaySync
import com.tally.link.RelayFailure
import kotlinx.coroutines.CancellationException
import kotlinx.serialization.json.JsonArray
import kotlinx.serialization.json.JsonElement
import kotlinx.serialization.json.JsonObject
import kotlinx.serialization.json.JsonPrimitive
import kotlinx.serialization.json.booleanOrNull
import kotlinx.serialization.json.buildJsonArray
import kotlinx.serialization.json.buildJsonObject
import kotlinx.serialization.json.contentOrNull
import kotlinx.serialization.json.longOrNull
import kotlinx.serialization.json.put
import javax.inject.Inject
import javax.inject.Singleton

/** One account a connected bank holds, and whether Tally follows it into the ledger. */
data class BankAccount(
    val id: String,
    val name: String,
    val mask: String,
    /** Plaid's subtype in words: "checking", "credit card", "tfsa". */
    val kind: String,
    val follow: Boolean,
    /** The Tally account it fills (its uid), or null for "a new account named after it". */
    val into: String?,
)

/** A bank connected through Plaid, as the server keeps it. */
data class Bank(
    val id: String,
    val institution: String,
    val accounts: List<BankAccount>,
    /** When the server last read new transactions from it, epoch millis; null before the first. */
    val lastSync: Long?,
    /** The connection's own state in words when it needs the person ("Sign in again"); null when fine. */
    val problem: String?,
)

/** What the server said about bank connections. */
sealed interface BanksAnswer {
    data class Ready(val banks: List<Bank>) : BanksAnswer
    /** The server's Relay has no bank connections yet (docs/PLAID.md). */
    data object NotOnServer : BanksAnswer
    data object NotPaired : BanksAnswer
    data class Failed(val reason: String) : BanksAnswer
}

/**
 * Bank connections through Plaid (docs/PLAID.md). The phone never holds Plaid's secret or a bank's
 * access token: it asks the owner's server for a link token, opens Plaid's own sign-in with it, and
 * hands the one-time public token back. The server reads transactions from then on, read only, and
 * they arrive here as ledger rows by the ordinary sync, deduplicated by their `plaid:` uid.
 */
@Singleton
class Banks @Inject constructor(private val relay: RelaySync) {

    suspend fun list(): BanksAnswer = ask { BanksAnswer.Ready(parseBanks(relay.call("money.bank.list", JsonObject(emptyMap())))) }

    /** A link token for Plaid's sign-in on this phone, or why there is none. */
    suspend fun linkToken(): Result<String> = runCatchingRelay {
        val r = relay.call("money.bank.link_token", buildJsonObject {
            put("android_package_name", BuildConfig.APPLICATION_ID)
            put("client_name", "Tally")
        })
        r.str("link_token") ?: throw RelayFailure("Your server did not hand back a link token")
    }

    /**
     * Trades Plaid's one-time public token for a connection the server keeps, and answers the bank
     * as the server now holds it (its accounts, none followed yet).
     */
    suspend fun exchange(publicToken: String, institutionId: String?, institutionName: String?): Result<Bank> = runCatchingRelay {
        val r = relay.call("money.bank.exchange", buildJsonObject {
            put("public_token", publicToken)
            institutionId?.let { put("institution_id", it) }
            institutionName?.let { put("institution_name", it) }
        }, timeoutMs = 60_000)
        parseBank((r as? JsonObject)?.get("bank")) ?: throw RelayFailure("Your server did not say what it connected")
    }

    /** Which accounts to follow and where each lands; [since] is the first day to bring in (ISO date). */
    suspend fun follow(bankId: String, accounts: List<BankAccount>, since: String?): Result<Bank> = runCatchingRelay {
        val r = relay.call("money.bank.accounts", buildJsonObject {
            put("bank_id", bankId)
            since?.let { put("since", it) }
            put("accounts", buildJsonArray {
                accounts.forEach { a ->
                    add(buildJsonObject {
                        put("id", a.id)
                        put("follow", a.follow)
                        a.into?.let { put("into", it) }
                    })
                }
            })
        }, timeoutMs = 60_000)
        parseBank((r as? JsonObject)?.get("bank")) ?: throw RelayFailure("Your server did not say what it follows")
    }

    /** Asks the server to read every connected bank now, rather than at its next round. */
    suspend fun refresh(): Result<Unit> = runCatchingRelay { relay.call("money.bank.refresh", JsonObject(emptyMap())); Unit }

    /** Disconnects a bank: the server drops its access token; entries already in the ledger stay. */
    suspend fun remove(bankId: String): Result<Unit> = runCatchingRelay {
        relay.call("money.bank.remove", buildJsonObject { put("bank_id", bankId) })
        Unit
    }

    private suspend fun ask(block: suspend () -> BanksAnswer): BanksAnswer = try {
        block()
    } catch (e: CancellationException) {
        throw e
    } catch (e: RelayFailure) {
        when {
            e.reason == RelaySync.NOT_CONNECTED -> BanksAnswer.NotPaired
            e.code in UNKNOWN -> BanksAnswer.NotOnServer
            else -> BanksAnswer.Failed(e.reason)
        }
    } catch (e: Exception) {
        BanksAnswer.Failed("Your server could not be reached")
    }

    private suspend fun <T> runCatchingRelay(block: suspend () -> T): Result<T> = try {
        Result.success(block())
    } catch (e: CancellationException) {
        throw e
    } catch (e: RelayFailure) {
        Result.failure(if (e.code in UNKNOWN) RelayFailure(NOT_ON_SERVER, code = e.code) else e)
    } catch (e: Exception) {
        Result.failure(RelayFailure("Your server could not be reached"))
    }

    companion object {
        /** The codes a Relay without bank connections answers with. */
        val UNKNOWN = setOf("bus.unknown_op", "remote.op")
        const val NOT_ON_SERVER = "Your server does not connect banks yet. It needs the Plaid update."

        fun parseBanks(json: JsonElement?): List<Bank> =
            ((json as? JsonObject)?.get("banks") as? JsonArray)?.mapNotNull { parseBank(it) } ?: emptyList()

        fun parseBank(json: JsonElement?): Bank? {
            val o = json as? JsonObject ?: return null
            val id = o.str("id") ?: return null
            return Bank(
                id = id,
                institution = o.str("institution") ?: "A bank",
                accounts = (o["accounts"] as? JsonArray)?.mapNotNull { a ->
                    val ao = a as? JsonObject ?: return@mapNotNull null
                    BankAccount(
                        id = ao.str("id") ?: return@mapNotNull null,
                        name = ao.str("name") ?: "Account",
                        mask = ao.str("mask") ?: "",
                        kind = ao.str("kind") ?: "",
                        follow = (ao["follow"] as? JsonPrimitive)?.booleanOrNull ?: false,
                        into = ao.str("into"),
                    )
                } ?: emptyList(),
                lastSync = (o["last_sync"] as? JsonPrimitive)?.longOrNull,
                problem = o.str("problem"),
            )
        }

        private fun JsonElement?.str(key: String): String? =
            ((this as? JsonObject)?.get(key) as? JsonPrimitive)?.contentOrNull?.takeIf { it.isNotBlank() }
    }
}
