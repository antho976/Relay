package com.quietsoftware.relaycode;

import static org.junit.Assert.assertEquals;
import static org.junit.Assert.assertNotNull;
import static org.junit.Assert.assertNull;

import android.app.PendingIntent;
import android.content.Context;
import android.content.Intent;

import org.junit.Test;
import org.junit.runner.RunWith;
import org.robolectric.RobolectricTestRunner;
import org.robolectric.RuntimeEnvironment;
import org.robolectric.Shadows;
import org.robolectric.annotation.Config;

/**
 * D21: the Reply's PendingIntent is mutable (Android writes the words into it), so whoever holds it
 * may fill in extras. What the receiver does comes from the action, which a fill-in cannot change,
 * so a Reply dressed up as an Allow allows nothing.
 */
@RunWith(RobolectricTestRunner.class)
@Config(sdk = 34)
public class ActionReceiverTest {
    private static Intent saved(PendingIntent p) {
        return new Intent(Shadows.shadowOf(p).getSavedIntent());
    }

    @Test
    public void a_reply_filled_in_as_an_allow_with_a_hold_allows_nothing() {
        Context context = RuntimeEnvironment.getApplication();
        PendingIntent p = ActionReceiver.intent(context, 42, "reply", "umber-zebra", 0, true);
        Intent reply = saved(p);
        // What a holder of the mutable intent may send: other extras and another action. Android
        // fills it in with the flags its creator gave (PendingIntentRecord: fillIn(intent,
        // key.flags)), never the sender's, so this is what the receiver would get.
        Intent evil = new Intent("com.quietsoftware.relaycode.ACT.allow").putExtra("kind", "allow").putExtra("hold", 7L);
        reply.fillIn(evil, Shadows.shadowOf(p).getFlags());
        ActionReceiver.Act a = ActionReceiver.read(reply);
        assertNotNull(a);
        assertEquals("still a reply: the action set is kept", "reply", a.kind);
        assertEquals("a reply carries no hold", 0L, a.hold);
        // With no words a reply sends nothing either (Rust's Act::read refuses an empty reply).
        assertEquals("", a.text);
    }

    @Test
    public void allow_and_deny_carry_their_hold_and_nothing_else_is_read() {
        Context context = RuntimeEnvironment.getApplication();
        ActionReceiver.Act allow = ActionReceiver.read(saved(ActionReceiver.intent(context, 42, "allow", "umber-zebra", 7, false)));
        assertEquals("allow", allow.kind);
        assertEquals(7L, allow.hold);
        ActionReceiver.Act deny = ActionReceiver.read(saved(ActionReceiver.intent(context, 42, "deny", "umber-zebra", 7, false)));
        assertEquals("deny", deny.kind);
        assertEquals(7L, deny.hold);
        // Anything this app did not send is ignored.
        assertNull(ActionReceiver.read(new Intent("something.else")));
        assertNull(ActionReceiver.read(new Intent("com.quietsoftware.relaycode.ACT.shrug")));
    }
}
