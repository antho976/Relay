package com.quietsoftware.relay.ui.threads

import kotlinx.serialization.json.Json
import kotlinx.serialization.json.JsonPrimitive
import kotlinx.serialization.json.jsonObject
import org.junit.Assert.assertEquals
import org.junit.Assert.assertFalse
import org.junit.Assert.assertNull
import org.junit.Assert.assertTrue
import org.junit.Test

/** Which models are offered pictures, and the pictures a message names. */
class PicturesTest {
    private fun obj(json: String) = Json.parseToJsonElement(json).jsonObject

    @Test
    fun claudesModelsReadPicturesAndThePcsOwnDoNot() {
        assertTrue(seesPictures(""))
        assertTrue(seesPictures(null))
        assertTrue(seesPictures("claude-sonnet-5-5"))
        assertTrue(seesPictures("opus"))
        assertTrue(seesPictures("sonnet[1m]"))
        assertFalse(seesPictures("qwen35-9b"))
        assertFalse(seesPictures("qwen35-4b"))
    }

    @Test
    fun aMessageBodyAndAPayloadNameTheirPictures() {
        val body = picturesOf(obj("""{"text": "", "images": [{"id": "a1", "media_type": "image/jpeg", "width": 2048, "height": 1024}, {"id": 7}]}"""))
        assertEquals(listOf(JsonPrimitive("a1"), JsonPrimitive(7)), body.map { it.id })
        assertEquals(2f, body[0].ratio!!, .001f)
        assertNull(body[1].ratio)
        assertEquals(listOf(JsonPrimitive("b2")), picturesOf(obj("""{"id": 3, "text": "", "images": ["b2", null]}""")).map { it.id })
        assertTrue(picturesOf(obj("""{"text": "hi"}""")).isEmpty())
        assertTrue(picturesOf(null).isEmpty())
    }
}
