package com.quietsoftware.relay.ui.term

import android.os.Looper
import android.text.InputType
import android.view.KeyEvent
import android.view.inputmethod.EditorInfo
import androidx.activity.ComponentActivity
import androidx.activity.compose.setContent
import androidx.compose.ui.viewinterop.AndroidView
import com.quietsoftware.relay.core.term.Keys
import com.quietsoftware.relay.core.term.Terminal
import org.junit.Assert.assertEquals
import org.junit.Assert.assertTrue
import org.junit.Before
import org.junit.Test
import org.junit.runner.RunWith
import org.robolectric.Robolectric
import org.robolectric.RobolectricTestRunner
import org.robolectric.Shadows.shadowOf

/** Direct typing takes a keyboard's keys as a terminal must: each at once, delete and Enter as keys. */
@RunWith(RobolectricTestRunner::class)
class DirectInputTest {
    private val typed = ArrayList<String>()
    private val terminal = Terminal(80, 24)
    // Hosted as the terminal screen hosts it, in Compose, and focused: a keyboard's key events come
    // into the window and must get through Compose to it.
    private val activity = Robolectric.buildActivity(ComponentActivity::class.java).setup().get()
    private lateinit var view: DirectInput

    @Before fun host() {
        activity.setContent {
            AndroidView(factory = { DirectInput(it, terminal, Any(), onText = { t -> typed += "text:$t" }, onKey = { k -> typed += "key:$k" }).also { v -> view = v } })
        }
        shadowOf(Looper.getMainLooper()).idle()
        view.requestFocus()
        shadowOf(Looper.getMainLooper()).idle()
    }

    /** A key as the window receives one from the keyboard. */
    private fun press(code: Int) {
        val window = activity.window.decorView
        window.dispatchKeyEvent(KeyEvent(KeyEvent.ACTION_DOWN, code))
        window.dispatchKeyEvent(KeyEvent(KeyEvent.ACTION_UP, code))
        shadowOf(Looper.getMainLooper()).idle()
    }

    @Test fun `the keyboard is asked for no suggestions and each commit goes at once`() {
        val info = EditorInfo()
        val ic = view.onCreateInputConnection(info)
        assertTrue(info.inputType and InputType.TYPE_TEXT_FLAG_NO_SUGGESTIONS != 0)
        assertTrue(info.inputType and InputType.TYPE_TEXT_VARIATION_VISIBLE_PASSWORD != 0)
        ic.commitText("l", 1)
        ic.commitText("s", 1)
        // A keyboard that composed after all: sent when it finishes, not twice.
        ic.setComposingText("-l", 1)
        ic.finishComposingText()
        assertEquals(listOf("text:l", "text:s", "text:-l"), typed)
    }

    @Test fun `delete with nothing held is the terminal's backspace, once per character asked`() {
        val ic = view.onCreateInputConnection(EditorInfo())
        ic.deleteSurroundingText(1, 0)
        ic.deleteSurroundingText(3, 0)
        assertEquals(List(4) { "key:${Keys.BACKSPACE}" }, typed)
    }

    @Test fun `enter, arrows and delete arrive as the program expects them`() {
        assertTrue("the input view has focus", view.isFocused)
        press(KeyEvent.KEYCODE_ENTER)
        press(KeyEvent.KEYCODE_DPAD_UP)
        press(KeyEvent.KEYCODE_DEL)
        view.onCreateInputConnection(EditorInfo()).performEditorAction(EditorInfo.IME_ACTION_DONE)
        // Application cursor mode (vim, less) changes what an arrow sends.
        terminal.feed("\u001b[?1h")
        press(KeyEvent.KEYCODE_DPAD_UP)
        assertEquals(listOf("key:\r", "key:\u001b[A", "key:\u007f", "key:\r", "key:\u001bOA"), typed)
    }

    @Test fun `a hardware keyboard's ctrl reaches the program as a control character`() {
        view.onKeyDown(KeyEvent.KEYCODE_C, KeyEvent(0, 0, KeyEvent.ACTION_DOWN, KeyEvent.KEYCODE_C, 0, KeyEvent.META_CTRL_ON or KeyEvent.META_CTRL_LEFT_ON))
        view.onKeyDown(KeyEvent.KEYCODE_A, KeyEvent(KeyEvent.ACTION_DOWN, KeyEvent.KEYCODE_A))
        assertEquals(listOf("key:\u0003", "text:a"), typed)
    }
}
