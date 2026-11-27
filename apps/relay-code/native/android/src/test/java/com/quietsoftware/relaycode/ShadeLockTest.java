package com.quietsoftware.relaycode;

import static org.junit.Assert.assertEquals;
import static org.junit.Assert.assertFalse;
import static org.junit.Assert.assertNotNull;
import static org.junit.Assert.assertNull;
import static org.junit.Assert.assertTrue;

import android.app.Notification;
import android.app.NotificationManager;
import android.content.Context;

import org.junit.Test;
import org.junit.runner.RunWith;
import org.robolectric.RobolectricTestRunner;
import org.robolectric.RuntimeEnvironment;
import org.robolectric.Shadows;
import org.robolectric.annotation.Config;

/**
 * What a notification lets someone holding the locked phone do (D18): nothing that acts as the
 * owner. Reply, Allow once and Deny need the phone unlocked; on the lock screen only that something
 * waits shows, with no question, no command and no actions.
 */
@RunWith(RobolectricTestRunner.class)
public class ShadeLockTest {
    private static Notification posted(int kind) {
        Context context = RuntimeEnvironment.getApplication();
        LinkService.alert(context, 42, "umber-zebra needs your OK", "git push --force origin main", "umber-zebra", kind, 7L);
        NotificationManager nm = context.getSystemService(NotificationManager.class);
        Notification n = Shadows.shadowOf(nm).getNotification(42);
        assertNotNull(n);
        return n;
    }

    private static Notification.Action action(Notification n, String title) {
        for (Notification.Action a : n.actions) {
            if (title.contentEquals(a.title)) return a;
        }
        throw new AssertionError("no action " + title);
    }

    @Test
    @Config(sdk = 34)
    public void every_action_that_acts_needs_the_phone_unlocked() {
        Notification hold = posted(LinkService.KIND_HOLD);
        assertTrue(action(hold, "Allow once").isAuthenticationRequired());
        assertTrue(action(hold, "Deny").isAuthenticationRequired());
        Notification ask = posted(LinkService.KIND_REPLY);
        assertTrue(action(ask, "Reply").isAuthenticationRequired());
        // Open goes through the app, which a locked phone unlocks first.
        assertFalse(action(ask, "Open").isAuthenticationRequired());
    }

    @Test
    @Config(sdk = 34)
    public void the_lock_screen_shows_that_something_waits_not_what() {
        for (int kind : new int[] {LinkService.KIND_OPEN, LinkService.KIND_REPLY, LinkService.KIND_HOLD, LinkService.KIND_QUIET}) {
            Notification n = posted(kind);
            assertEquals(Notification.VISIBILITY_PRIVATE, n.visibility);
            Notification shown = n.publicVersion;
            assertNotNull(shown);
            assertNull("no actions on the lock screen", shown.actions);
            String text = String.valueOf(shown.extras.getCharSequence(Notification.EXTRA_TEXT));
            assertFalse("no command on the lock screen", text.contains("git push"));
            assertEquals(LinkService.lockedWords(kind), text);
        }
    }

    @Test
    @Config(sdk = 28)
    public void before_android_12_the_lock_screen_still_has_no_actions() {
        Notification n = posted(LinkService.KIND_HOLD);
        assertEquals(Notification.VISIBILITY_PRIVATE, n.visibility);
        assertNull(n.publicVersion.actions);
    }
}
