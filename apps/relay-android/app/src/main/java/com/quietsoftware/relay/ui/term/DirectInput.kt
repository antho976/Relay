package com.quietsoftware.relay.ui.term

import android.annotation.SuppressLint
import android.content.Context
import android.text.InputType
import android.view.KeyEvent
import android.view.View
import android.view.inputmethod.BaseInputConnection
import android.view.inputmethod.EditorInfo
import android.view.inputmethod.InputConnection
import android.view.inputmethod.InputMethodManager
import com.quietsoftware.relay.core.term.Keys
import com.quietsoftware.relay.core.term.Terminal

/**
 * What the phone's keyboard types, sent to the terminal key by key as Termux takes it: no
 * suggestions, no autocorrect, each character as it is typed, Backspace as DEL, Enter as CR, and
 * a hardware keyboard's arrows, Home, End and Page keys in the program's cursor mode. Text goes
 * through [onText], where the screen's sticky Ctrl and Alt apply; keys that are already sequences
 * go through [onKey] as they are. [terminal] is read under [lock] for the cursor mode.
 */
@SuppressLint("ViewConstructor")
internal class DirectInput(
    context: Context,
    private val terminal: Terminal,
    private val lock: Any,
    var onText: (String) -> Unit,
    var onKey: (String) -> Unit,
) : View(context) {
    init {
        isFocusable = true
        isFocusableInTouchMode = true
    }

    override fun onCheckIsTextEditor() = true

    override fun onCreateInputConnection(outAttrs: EditorInfo): InputConnection {
        // A visible password is what keyboards honour as "no predictions": each key commits at once.
        outAttrs.inputType = InputType.TYPE_CLASS_TEXT or InputType.TYPE_TEXT_VARIATION_VISIBLE_PASSWORD or InputType.TYPE_TEXT_FLAG_NO_SUGGESTIONS
        outAttrs.imeOptions = EditorInfo.IME_FLAG_NO_FULLSCREEN or EditorInfo.IME_FLAG_NO_EXTRACT_UI or EditorInfo.IME_ACTION_NONE
        return object : BaseInputConnection(this, true) {
            override fun commitText(text: CharSequence?, newCursorPosition: Int): Boolean {
                super.commitText(text, newCursorPosition)
                flush()
                return true
            }

            override fun finishComposingText(): Boolean {
                super.finishComposingText()
                flush()
                return true
            }

            override fun deleteSurroundingText(beforeLength: Int, afterLength: Int): Boolean {
                // Nothing is kept here, so a keyboard's delete is the terminal's: one DEL per character it asked for.
                val held = editable?.length ?: 0
                if (held == 0) repeat(beforeLength.coerceAtLeast(1)) { onKey(Keys.BACKSPACE) }
                return super.deleteSurroundingText(beforeLength, afterLength)
            }

            override fun performEditorAction(editorAction: Int): Boolean {
                onKey(Keys.ENTER)
                return true
            }

            private fun flush() {
                val e = editable ?: return
                if (e.isEmpty()) return
                val text = e.toString()
                e.clear()
                onText(text)
            }
        }
    }

    override fun onKeyDown(keyCode: Int, event: KeyEvent): Boolean {
        val seq = synchronized(lock) {
            when (keyCode) {
                KeyEvent.KEYCODE_ENTER, KeyEvent.KEYCODE_NUMPAD_ENTER -> Keys.ENTER
                KeyEvent.KEYCODE_DEL -> Keys.BACKSPACE
                KeyEvent.KEYCODE_FORWARD_DEL -> Keys.DELETE
                KeyEvent.KEYCODE_TAB -> if (event.isShiftPressed) Keys.SHIFT_TAB else Keys.TAB
                KeyEvent.KEYCODE_ESCAPE -> Keys.ESC
                KeyEvent.KEYCODE_DPAD_UP -> Keys.arrowUp(terminal)
                KeyEvent.KEYCODE_DPAD_DOWN -> Keys.arrowDown(terminal)
                KeyEvent.KEYCODE_DPAD_LEFT -> Keys.arrowLeft(terminal)
                KeyEvent.KEYCODE_DPAD_RIGHT -> Keys.arrowRight(terminal)
                KeyEvent.KEYCODE_MOVE_HOME -> Keys.home(terminal)
                KeyEvent.KEYCODE_MOVE_END -> Keys.end(terminal)
                KeyEvent.KEYCODE_PAGE_UP -> Keys.PAGE_UP
                KeyEvent.KEYCODE_PAGE_DOWN -> Keys.PAGE_DOWN
                else -> null
            }
        }
        if (seq != null) {
            onKey(seq)
            return true
        }
        // A letter sent as a key event (a hardware keyboard, or a keyboard in this mode): with its own Ctrl and Alt.
        val ch = event.getUnicodeChar(event.metaState and (KeyEvent.META_CTRL_MASK or KeyEvent.META_ALT_MASK).inv())
        if (ch <= 0) return super.onKeyDown(keyCode, event)
        val c = ch.toChar()
        val typed = Keys.typed(c.toString(), ctrl = event.isCtrlPressed, alt = event.isAltPressed)
        if (event.isCtrlPressed || event.isAltPressed) onKey(typed) else onText(typed)
        return true
    }

    /** Take the keyboard: focus, and raise it, once this is in the window. */
    fun raise() {
        post {
            requestFocus()
            context.getSystemService(InputMethodManager::class.java)?.showSoftInput(this, 0)
        }
    }

    /** Give the keyboard back. */
    fun lower() {
        context.getSystemService(InputMethodManager::class.java)?.hideSoftInputFromWindow(windowToken, 0)
        clearFocus()
    }
}
