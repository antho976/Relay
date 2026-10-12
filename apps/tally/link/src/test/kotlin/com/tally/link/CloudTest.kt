package com.tally.link

import kotlinx.coroutines.test.runTest
import okhttp3.mockwebserver.MockResponse
import okhttp3.mockwebserver.MockWebServer
import org.junit.After
import org.junit.Assert.assertEquals
import org.junit.Assert.assertFalse
import org.junit.Assert.assertTrue
import org.junit.Assert.fail
import org.junit.Before
import org.junit.Test

/** The request that lets this phone sync, against what antho-s-bot's /api/tally answers. */
class CloudTest {

    private lateinit var server: MockWebServer
    private val api = CloudApi()

    @Before fun start() { server = MockWebServer().also { it.start() } }
    @After fun stop() = server.shutdown()

    private fun base() = server.url("/").toString()

    @Test fun anAddressIsReadHoweverItIsTyped() {
        assertEquals("https://antho-s-bot-8dve.onrender.com", CloudApi.normalize("antho-s-bot-8dve.onrender.com"))
        assertEquals("https://antho-s-bot-8dve.onrender.com", CloudApi.normalize(" https://antho-s-bot-8dve.onrender.com/ "))
        assertEquals("https://antho-s-bot-8dve.onrender.com", CloudApi.normalize("wss://antho-s-bot-8dve.onrender.com/relay"))
        assertEquals("http://10.0.2.2:3000", CloudApi.normalize("http://10.0.2.2:3000/api/tally"))
        assertEquals("https://antho-s-bot-8dve.onrender.com/api/tally", CloudApi.api("antho-s-bot-8dve.onrender.com"))
    }

    @Test fun theCodeIsTheChallengesFirstFourCharactersAsTheBotShowsThem() {
        val verifier = CloudApi.newVerifier()
        assertEquals(64, verifier.length)
        assertTrue(verifier.all { it in "0123456789abcdef" })
        val challenge = CloudApi.challengeOf(verifier)
        // printf 'abc' | sha256sum: the digest node's createHash("sha256").update("abc") gives.
        assertEquals("ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad", CloudApi.challengeOf("abc"))
        assertEquals(challenge.take(4).uppercase(), CloudApi.codeOf(challenge))
        assertFalse(CloudApi.newVerifier() == verifier)
    }

    @Test fun helloSaysWhetherTheCloudCopyRuns() = runTest {
        server.enqueue(MockResponse().setBody("""{"service":"tally-cloud","v":1,"cloud":true,"approval":"discord"}"""))
        assertTrue(api.hello(base()).cloud)
        assertEquals("/api/tally", server.takeRequest().path)
        server.enqueue(MockResponse().setBody("<html>a dashboard</html>"))
        try {
            api.hello(base())
            fail("a page that is not the bot's must not read as a server")
        } catch (e: CloudFailure) {
            assertFalse(e.cloudOff)
        }
    }

    @Test fun enrollSendsOnlyTheChallengeAndKeepsTheVerifier() = runTest {
        server.enqueue(MockResponse().setResponseCode(201).setBody("""{"id":"0123456789abcdef0123456789abcdef","code":"ABCD","expiresAt":1900000000000,"notified":true}"""))
        val e = api.enroll(base(), "Tally on Pixel 8")
        val sent = server.takeRequest()
        assertEquals("/api/tally/enroll", sent.path)
        val body = sent.body.readUtf8()
        assertTrue(body, body.contains("\"challenge\":\"${CloudApi.challengeOf(e.verifier)}\""))
        assertFalse("the verifier never leaves before the owner allows", body.contains(e.verifier))
        assertTrue(body.contains("\"device\":\"Tally on Pixel 8\""))
        assertEquals("0123456789abcdef0123456789abcdef", e.id)
        assertEquals("ABCD", e.code)
        assertTrue(e.notified)
    }

    @Test fun aServerWithoutRelaySaysSo() = runTest {
        server.enqueue(MockResponse().setResponseCode(503).setBody("""{"error":"cloud_off"}"""))
        try {
            api.enroll(base(), "Tally on Pixel 8")
            fail("expected a refusal")
        } catch (e: CloudFailure) {
            assertTrue(e.cloudOff)
        }
    }

    @Test fun claimReadsEveryAnswer() = runTest {
        val e = Enrollment(base().trimEnd('/'), "0123456789abcdef0123456789abcdef", CloudApi.newVerifier(), "ABCD", 0, true)
        server.enqueue(MockResponse().setBody("""{"status":"waiting"}"""))
        server.enqueue(MockResponse().setBody("""{"status":"denied"}"""))
        server.enqueue(MockResponse().setBody("""{"status":"expired"}"""))
        server.enqueue(MockResponse().setBody("""{"status":"ready","link":"relay://pair?v=1&code=87H4-KKJF&host=vm&id=133a&instance=stable&via=wss%3A%2F%2Fbot.example%2Frelay%2Fjoin%2Fabc"}"""))
        server.enqueue(MockResponse().setResponseCode(503).setBody("""{"status":"error","reason":"The server could not open a pairing window. Try again in a minute."}"""))
        assertEquals(ClaimReply.Waiting, api.claim(e))
        assertTrue(server.takeRequest().body.readUtf8().contains("\"verifier\":\"${e.verifier}\""))
        assertEquals(ClaimReply.Denied, api.claim(e))
        assertEquals(ClaimReply.Expired, api.claim(e))
        val ready = api.claim(e) as ClaimReply.Ready
        assertEquals("87H4-KKJF", ready.link.code)
        assertEquals(listOf("wss://bot.example/relay/join/abc"), ready.link.routes)
        val trouble = api.claim(e) as ClaimReply.Trouble
        assertTrue(trouble.reason.startsWith("The server could not open"))
    }
}
