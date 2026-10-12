package com.tally.app.data.sync

import android.content.Context
import androidx.test.core.app.ApplicationProvider
import com.tally.app.data.FixedClock
import com.tally.app.data.RoomTestDb
import com.tally.app.data.addAccount
import com.tally.app.data.addExpense
import com.tally.app.data.db.TallyDatabase
import com.tally.app.data.prefs.SettingsRepository
import com.tally.link.PairLink
import com.tally.link.RelayClient
import com.tally.link.RelayWire
import kotlinx.coroutines.runBlocking
import kotlinx.serialization.json.JsonArray
import kotlinx.serialization.json.JsonNull
import kotlinx.serialization.json.JsonObject
import kotlinx.serialization.json.boolean
import kotlinx.serialization.json.buildJsonArray
import kotlinx.serialization.json.buildJsonObject
import kotlinx.serialization.json.jsonArray
import kotlinx.serialization.json.jsonObject
import kotlinx.serialization.json.jsonPrimitive
import kotlinx.serialization.json.long
import kotlinx.serialization.json.put
import okhttp3.Response
import okhttp3.WebSocket
import okhttp3.WebSocketListener
import okhttp3.mockwebserver.MockResponse
import okhttp3.mockwebserver.MockWebServer
import org.junit.After
import org.junit.Assert.assertEquals
import org.junit.Assert.assertFalse
import org.junit.Assert.assertNull
import org.junit.Assert.assertTrue
import org.junit.Before
import org.junit.Test
import org.junit.runner.RunWith
import org.robolectric.RobolectricTestRunner
import java.time.LocalDate
import java.util.concurrent.CopyOnWriteArrayList

/**
 * Tally against a fake Relay door (the cloud copy on the owner's server answers as a PC's does):
 * the greeting, the pairing code once, a proof every time after, then one money.sync request per
 * connection, end to end over a real WebSocket.
 */
@RunWith(RobolectricTestRunner::class)
class RelaySyncTest {

    private val context: Context = ApplicationProvider.getApplicationContext()
    private val day = LocalDate.of(2026, 10, 5)
    private lateinit var db: TallyDatabase
    private lateinit var store: SyncStore
    private lateinit var settings: SettingsRepository
    private lateinit var sync: RelaySync
    private val door = FakeDoor()

    @Before fun open() = runBlocking<Unit> {
        db = RoomTestDb.create()
        store = SyncStore(context)
        store.forget()
        settings = SettingsRepository(context)
        sync = RelaySync(store, LedgerSync(db), settings, RelayClient())
        door.server.start()
    }

    @After fun close() {
        db.close()
        door.server.shutdown()
    }

    @Test fun pairingRunsAFirstSyncThatMergesNeverReplacing() = runBlocking<Unit> {
        val account = db.addAccount("Chequing")
        db.addExpense(42_50, day, account, note = "Metro")
        // The server already holds an account of its own (from the PC, or an earlier phone).
        door.whole = listOf(
            Change("accounts", "a-server", 1_700_000_000_000L, row = buildJsonObject {
                put("name", "Savings"); put("type", "SAVINGS"); put("openingBalance", 500_00); put("archived", false); put("sortOrder", 1)
            }),
        )
        door.connection()

        val outcome = sync.pair(PairLink.manual(door.address, "ABCD-EFGH")!!)

        assertTrue("$outcome", outcome is SyncOutcome.Done)
        val prefs = store.current()
        assertEquals("antho desktop", prefs.pc!!.name)
        assertEquals("dev-1", prefs.pc!!.deviceId)
        assertEquals("tok-1", prefs.pc!!.token)
        assertTrue(prefs.replaced)
        assertEquals(40L, prefs.cursor)
        assertNull(prefs.lastError)

        val sent = door.syncs.single()
        assertFalse("A first sync never empties the server's ledger", sent["replace"]!!.jsonPrimitive.boolean)
        assertEquals(0L, sent["since"]!!.jsonPrimitive.long)
        assertNull("A merge names no generation", sent["generation"])
        val tables = sent["changes"]!!.jsonArray.map { it.jsonObject["table"]!!.jsonPrimitive.content }
        assertEquals("Settings first, then the ledger in Relay's order", listOf("settings", "settings", "settings", "accounts", "transactions"), tables)
        assertEquals("Both sides' accounts are here", listOf("Chequing", "Savings"), db.accounts().all().map { it.name }.sorted())
        assertEquals(listOf("Metro"), db.transactions().all().map { it.note })
    }

    @Test fun aNewPhoneTakesTheServersSettingsAsWellAsItsLedger() = runBlocking<Unit> {
        settings.setCurrency("CAD")
        val stamp = 1_700_000_000_000L
        door.whole = listOf(
            Change("settings", "month_start_day", stamp, row = buildJsonObject { put("value", "15") }),
            Change("accounts", "a-server", stamp, row = buildJsonObject {
                put("name", "Chequing"); put("type", "CHEQUING"); put("openingBalance", 0); put("archived", false); put("sortOrder", 0)
            }),
        )
        door.connection()

        sync.pair(PairLink.manual(door.address, "ABCD-EFGH")!!)

        assertEquals(15, settings.current().monthStartDay)
        assertEquals(listOf("a-server"), db.accounts().all().map { it.uid })
        settings.setMonthStartDay(1)
    }

    @Test fun replacingTheServersCopyIsAskedForAndHappensOnce() = runBlocking<Unit> {
        db.addAccount("Chequing")
        door.connection()
        sync.pair(PairLink.manual(door.address, "ABCD-EFGH")!!)
        store.requestReplace()
        door.connection()

        sync.syncNow()

        val replaced = door.syncs.last()
        assertTrue(replaced["replace"]!!.jsonPrimitive.boolean)
        assertEquals(0L, replaced["since"]!!.jsonPrimitive.long)
        assertNull(replaced["generation"])
        assertEquals("Everything here goes", listOf("settings", "settings", "settings", "accounts"), replaced["changes"]!!.jsonArray.map { it.jsonObject["table"]!!.jsonPrimitive.content })
        assertFalse(store.current().replaceNext)

        door.connection()
        sync.syncNow()
        assertFalse(door.syncs.last()["replace"]!!.jsonPrimitive.boolean)
    }

    @Test fun laterSyncsProveTheKeySendWhatChangedAndTakeThePcsChanges() = runBlocking<Unit> {
        val account = db.addAccount("Chequing")
        door.connection()
        sync.pair(PairLink.manual(door.address, "ABCD-EFGH")!!)
        val accountUid = db.accounts().get(account)!!.uid
        Thread.sleep(5)
        db.addExpense(4_00, day, account, note = "Coffee")
        // The PC logged an entry of its own, and moved the month to start on the 15th.
        door.answer = { _ ->
            buildJsonObject {
                put("cursor", 12)
                put("changes", buildJsonArray {
                    add(Change("transactions", "pc-1", System.currentTimeMillis() + 60_000, row = buildJsonObject {
                        put("type", "EXPENSE"); put("amount", 9_00); put("date", "2026-10-06"); put("account", accountUid)
                        put("toAccount", JsonNull); put("category", JsonNull); put("note", "Lunch"); put("recurring", JsonNull); put("createdAt", 1)
                    }).toJson())
                    add(Change("settings", "month_start_day", System.currentTimeMillis() + 60_000, row = buildJsonObject { put("value", "15") }).toJson())
                })
                put("replaced", false)
            }
        }
        door.connection()

        val outcome = sync.syncNow()

        assertTrue("$outcome", outcome is SyncOutcome.Done)
        val sent = door.syncs.last()
        assertFalse(sent["replace"]!!.jsonPrimitive.boolean)
        assertEquals("The server's cursor goes back as since", 40L, sent["since"]!!.jsonPrimitive.long)
        val notes = sent["changes"]!!.jsonArray.map { it.jsonObject["row"]!!.jsonObject["note"]?.jsonPrimitive?.content }
        assertEquals("Only the new entry goes", listOf("Coffee"), notes)
        assertTrue("The proof checked out", door.proven)

        assertEquals(12L, store.current().cursor)
        assertEquals(listOf("Coffee", "Lunch"), db.transactions().all().map { it.note }.sorted())
        assertEquals(15, settings.current().monthStartDay)
        settings.setMonthStartDay(1)
    }

    @Test fun aRevokedPhoneIsToldInWordsAndNothingIsThrown() = runBlocking<Unit> {
        door.connection()
        sync.pair(PairLink.manual(door.address, "ABCD-EFGH")!!)
        door.token = "another"
        door.connection()

        val outcome = sync.syncNow()

        assertTrue(outcome is SyncOutcome.Failed)
        assertEquals("Your server no longer accepts this phone's key. Connect it again.", store.current().lastError)
        assertTrue("CloudConnect asks again from here", store.current().unpaired)
    }

    @Test fun aWrongCodePairsNothing() = runBlocking<Unit> {
        door.connection()
        val outcome = sync.pair(PairLink.manual(door.address, "WRONG-CODE")!!)
        assertTrue(outcome is SyncOutcome.Failed)
        assertFalse(store.current().paired)
    }

    @Test fun anUnreachablePcFailsQuietly() = runBlocking<Unit> {
        door.connection()
        sync.pair(PairLink.manual(door.address, "ABCD-EFGH")!!)
        door.server.shutdown()
        val outcome = sync.syncNow()
        assertTrue(outcome is SyncOutcome.Failed)
        assertEquals("Your server could not be reached", store.current().lastError)
    }

    @Test fun theGenerationIsKeptAndSentBack() = runBlocking<Unit> {
        door.connection()
        sync.pair(PairLink.manual(door.address, "ABCD-EFGH")!!)
        assertEquals("gen-1", store.current().generation)
        assertNull("The first sync names no generation", door.syncs.single()["generation"])
        door.connection()
        sync.syncNow()
        assertEquals("gen-1", door.syncs.last()["generation"]!!.jsonPrimitive.content)
    }

    /**
     * The PC's ledger was restored since the last sync: it refuses to merge, and the phone takes
     * its ledger whole, stamps and uids kept, with nothing of its own left behind.
     */
    @Test fun aStalePhoneTakesThePcsLedgerWhole() = runBlocking<Unit> {
        val mine = db.addAccount("Old chequing")
        db.addExpense(4_00, day, mine, note = "Coffee")
        door.connection()
        sync.pair(PairLink.manual(door.address, "ABCD-EFGH")!!)
        Thread.sleep(5)
        db.addExpense(9_00, day, mine, note = "Made since")

        val stamp = 1_700_000_000_000L
        door.generation = "gen-2"
        door.whole = listOf(
            Change("settings", "month_start_day", stamp, row = buildJsonObject { put("value", "15") }),
            Change("accounts", "a-pc", stamp, row = buildJsonObject {
                put("name", "Restored chequing"); put("type", "CHEQUING"); put("openingBalance", 100); put("archived", false); put("sortOrder", 0)
            }),
            Change("transactions", "t-pc", stamp, row = buildJsonObject {
                put("type", "EXPENSE"); put("amount", 12_00); put("date", "2026-10-04"); put("account", "a-pc")
                put("toAccount", JsonNull); put("category", JsonNull); put("note", "Restored"); put("recurring", JsonNull); put("createdAt", 1)
            }),
            Change("transactions", "gone", stamp, deleted = true),
            Change("transactions", "bill:r1:2026-10-01", stamp, deleted = true),
        )
        door.connection()

        val outcome = sync.syncNow()

        assertTrue("$outcome", outcome is SyncOutcome.Done)
        val refused = door.syncs[door.syncs.size - 2]
        assertEquals("gen-1", refused["generation"]!!.jsonPrimitive.content)
        val take = door.syncs.last()
        assertEquals(0L, take["since"]!!.jsonPrimitive.long)
        assertNull(take["generation"])
        assertFalse(take["replace"]!!.jsonPrimitive.boolean)
        assertTrue("Nothing of the phone's goes to a ledger it no longer shares", take["changes"]!!.jsonArray.isEmpty())

        assertEquals(listOf("a-pc"), db.accounts().all().map { it.uid })
        val tx = db.transactions().all().single()
        assertEquals("t-pc", tx.uid)
        assertEquals("The PC's stamps are kept", stamp, tx.updatedAt)
        assertEquals("No tombstones for the rows it replaced, only the PC's deleted bill", listOf("bill:r1:2026-10-01"), db.sync().tombstones().map { it.uid })
        assertEquals(15, settings.current().monthStartDay)

        val prefs = store.current()
        assertEquals(40L, prefs.cursor)
        assertEquals("gen-2", prefs.generation)
        assertEquals(RelaySync.TOOK_SERVER_LEDGER, prefs.note)
        assertTrue("Merging starts from now", prefs.watermark >= tx.updatedAt)

        // From here the two merge again, under the new generation.
        door.connection()
        sync.syncNow()
        assertEquals("gen-2", door.syncs.last()["generation"]!!.jsonPrimitive.content)
        assertEquals(40L, door.syncs.last()["since"]!!.jsonPrimitive.long)
    }

    /**
     * A phone that synced before it could read the investment tables asks once for everything the
     * PC has, so the rows the PC numbered meanwhile arrive; from then on it asks from its cursor.
     */
    @Test fun anUpdatedPhoneAsksOnceForEverything() = runBlocking<Unit> {
        db.addAccount("Chequing")
        door.connection()
        sync.pair(PairLink.manual(door.address, "ABCD-EFGH")!!)
        assertEquals(SyncStore.LEDGER_VERSION, store.current().ledgerVersion)
        // As the build before investments left it.
        store.restore(store.current().copy(ledgerVersion = 0))
        assertTrue(store.current().catchingUp)
        door.answer = { _ ->
            buildJsonObject {
                put("cursor", 9)
                put("generation", "gen-1")
                put("changes", buildJsonArray {
                    add(Change("securities", "sec:XEQT", 1_700_000_000_000L, row = buildJsonObject {
                        put("symbol", "XEQT"); put("name", ""); put("currency", "CAD"); put("kind", "ETF"); put("exchange", "TSX")
                    }).toJson())
                })
                put("replaced", false)
            }
        }
        door.connection()

        sync.syncNow()

        val caughtUp = door.syncs.last()
        assertEquals("From the start", 0L, caughtUp["since"]!!.jsonPrimitive.long)
        assertEquals("gen-1", caughtUp["generation"]!!.jsonPrimitive.content)
        assertFalse(caughtUp["replace"]!!.jsonPrimitive.boolean)
        assertEquals("XEQT", db.invest().security("sec:XEQT")?.symbol)
        assertEquals(SyncStore.LEDGER_VERSION, store.current().ledgerVersion)

        door.connection()
        sync.syncNow()
        assertEquals("Once", 9L, door.syncs.last()["since"]!!.jsonPrimitive.long)
    }

    @Test fun settingStampsNeverGoBackAndACurrencyWithOtherDecimalsStaysOnThePhone() = runBlocking<Unit> {
        settings.setCurrency("CAD")
        val future = System.currentTimeMillis() + 3_600_000L
        // The PC, its clock ahead, set the month to start on the 15th.
        assertTrue(settings.applySynced("month_start_day", "15", future))
        settings.setMonthStartDay(1)
        val mine = settings.synced().all.single { it.key == "month_start_day" }
        assertTrue("A later edit here outranks the PC's even with a slower clock", mine.updatedAt > future)
        assertFalse(settings.applySynced("month_start_day", "20", future + 1))

        assertTrue("Same decimals: taken", settings.applySynced("currency", "USD", future + 10))
        assertFalse("Other decimals: the PC cannot have converted, so it is refused", settings.applySynced("currency", "JPY", future + 20))
        assertEquals("USD", settings.current().currency)
        assertTrue("With the PC's whole ledger it comes along", settings.applySynced("currency", "JPY", future + 30, force = true))
        settings.applySynced("currency", "CAD", future + 40, force = true)
    }
}

/** Relay's phone door as bridge.rs speaks it, one scripted connection per [connection]. */
private class FakeDoor {
    val server = MockWebServer()
    @Volatile var token = "tok-1"
    @Volatile var proven = false
    /** Which of its ledgers the PC holds; a sync naming another is stale. */
    @Volatile var generation = "gen-1"
    /** What the PC answers a phone that takes its ledger whole. */
    @Volatile var whole: List<Change> = emptyList()
    val syncs = CopyOnWriteArrayList<JsonObject>()
    @Volatile var answer: (JsonObject) -> JsonObject = { payload ->
        buildJsonObject {
            put("cursor", 7)
            put("generation", generation)
            put("changes", JsonArray(emptyList()))
            put("replaced", payload["replace"]?.jsonPrimitive?.boolean ?: false)
            put("applied", payload["changes"]!!.jsonArray.size)
            put("skipped", 0)
        }
    }

    val address: String get() = "${server.hostName}:${server.port}"

    fun connection() {
        server.enqueue(MockResponse().withWebSocketUpgrade(object : WebSocketListener() {
            val challenge = "challenge-${System.nanoTime()}"
            var admitted = false

            override fun onClosing(webSocket: WebSocket, code: Int, reason: String) {
                webSocket.close(1000, null)
            }

            override fun onOpen(webSocket: WebSocket, response: Response) {
                webSocket.send("""{"v":1,"relay":"remote","host":"antho desktop","host_id":"h1","instance":"dev","version":"0.4","challenge":"$challenge"}""")
            }

            override fun onMessage(webSocket: WebSocket, text: String) {
                val o = RelayWire.parse(text)!!
                if (!admitted) {
                    val pair = o["pair"]?.jsonPrimitive?.content
                    val proof = o["proof"]?.jsonPrimitive?.content
                    val provenNow = pair == null && proof == RelayWire.proof(challenge, token) && o["device"]?.jsonPrimitive?.content == "dev-1"
                    when {
                        pair == "ABCD-EFGH" -> webSocket.send("""{"v":1,"ok":true,"device":"dev-1","token":"$token"}""")
                        pair != null -> webSocket.send("""{"v":1,"ok":false,"error":"pair.invalid"}""")
                        provenNow -> {
                            proven = true
                            webSocket.send("""{"v":1,"ok":true,"device":"dev-1"}""")
                        }
                        else -> webSocket.send("""{"v":1,"ok":false,"error":"auth.bad_proof"}""")
                    }
                    admitted = pair == "ABCD-EFGH" || provenNow
                    return
                }
                val id = o["id"]!!.jsonPrimitive.content
                if (o["op"]!!.jsonPrimitive.content != "money.sync" || o["actor"]!!.jsonPrimitive.content != "user") {
                    webSocket.send("""{"v":1,"id":"$id","ok":false,"error":{"code":"remote.op","message":"refused"}}""")
                    return
                }
                val payload = o["payload"]!!.jsonObject
                syncs += payload
                val theirs = payload["generation"]?.jsonPrimitive?.content
                if (theirs != null && theirs != generation) {
                    webSocket.send("""{"v":1,"id":"$id","ok":false,"error":{"code":"money.sync_stale","kind":"conflict","message":"stale"}}""")
                    return
                }
                if (theirs == null && payload["since"]!!.jsonPrimitive.long == 0L && payload["replace"]?.jsonPrimitive?.boolean != true) {
                    val result = buildJsonObject {
                        put("cursor", 40)
                        put("generation", generation)
                        put("changes", JsonArray(whole.map { it.toJson() }))
                        put("replaced", false)
                    }
                    webSocket.send(buildJsonObject { put("v", 1); put("id", id); put("ok", true); put("result", result) }.toString())
                    return
                }
                // An event first: the client must wait for its own answer.
                webSocket.send("""{"v":1,"ev":"money.changed","ts":"2026-10-07T12:00:00Z","actor":"user","payload":{}}""")
                webSocket.send(buildJsonObject { put("v", 1); put("id", id); put("ok", true); put("result", answer(payload)) }.toString())
            }
        }))
    }
}
