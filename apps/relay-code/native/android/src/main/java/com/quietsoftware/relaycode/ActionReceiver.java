package com.quietsoftware.relaycode;

import android.app.PendingIntent;
import android.app.RemoteInput;
import android.content.BroadcastReceiver;
import android.content.Context;
import android.content.Intent;
import android.os.Bundle;
import android.os.Handler;
import android.os.Looper;

import androidx.core.content.ContextCompat;

/**
 * What is done from a notification in the shade (D18): a reply to an agent that asks you, or a held
 * command allowed once or denied. It hands the act to Rust (LinkService.nativeAct), whose loop asks
 * the PC on the link and then updates the notification ("Sent", "Allowed") and clears it.
 *
 * What it does is read from the intent's action, never from its extras (D21): the Reply's
 * PendingIntent must be mutable for Android to write the words into it, and whoever holds a mutable
 * PendingIntent (a notification listener, a watch's bridge) may fill in extras, but not an action
 * already set. So a Reply filled in with another kind or a hold stays a reply, and a hold's id is
 * read only for Allow once and Deny, whose intents are immutable.
 *
 * Nothing here may throw: a receiver that throws takes the process down with it.
 */
public class ActionReceiver extends BroadcastReceiver {
    /** The RemoteInput key a reply's words come under. */
    static final String REPLY = "reply";
    private static final String HOLD = "hold";
    private static final String ACT = "com.quietsoftware.relaycode.ACT";

    /** What a tap in the shade asks: its kind, the agent, the hold (allow and deny only), the words. */
    static final class Act {
        final String kind;
        final String agent;
        final long hold;
        final String text;

        Act(String kind, String agent, long hold, String text) {
            this.kind = kind;
            this.agent = agent;
            this.hold = hold;
            this.text = text;
        }
    }

    /**
     * The intent a notification's action sends here. A reply's must be mutable (Android writes the
     * words into it); the others are not.
     */
    static PendingIntent intent(Context context, int id, String kind, String agent, long hold, boolean mutable) {
        Intent i = new Intent(context, ActionReceiver.class)
                .setAction(ACT + "." + kind)
                .putExtra(LinkService.AGENT, agent);
        // Only an immutable intent carries a hold: a mutable one's extras can be changed by others.
        if (!mutable) i.putExtra(HOLD, hold);
        int flags = PendingIntent.FLAG_UPDATE_CURRENT | (mutable ? PendingIntent.FLAG_MUTABLE : PendingIntent.FLAG_IMMUTABLE);
        // One per notification and act, so a reply and an Allow never share an intent.
        int act = "reply".equals(kind) ? 1 : "allow".equals(kind) ? 2 : 3;
        return PendingIntent.getBroadcast(context, id * 4 + act, i, flags);
    }

    /**
     * What the intent asks, or null for anything this app did not send. The kind is the action's;
     * a reply has no hold and its words only; allow and deny have their hold and no words.
     */
    static Act read(Intent intent) {
        String action = intent.getAction();
        if (action == null || !action.startsWith(ACT + ".")) return null;
        String kind = action.substring(ACT.length() + 1);
        String agent = intent.getStringExtra(LinkService.AGENT);
        if (agent == null) agent = "";
        switch (kind) {
            case "reply": {
                String text = "";
                Bundle words = RemoteInput.getResultsFromIntent(intent);
                if (words != null) {
                    CharSequence said = words.getCharSequence(REPLY);
                    if (said != null) text = said.toString();
                }
                return new Act(kind, agent, 0, text);
            }
            case "allow":
            case "deny":
                return new Act(kind, agent, intent.getLongExtra(HOLD, 0), "");
            default:
                return null;
        }
    }

    /** How long the process is kept awake for an act to reach the PC (a receiver may take ~10 s). */
    private static final long KEEP_AWAKE_MS = 8_000;

    @Override
    public void onReceive(Context context, Intent intent) {
        // Android 14 and later freeze a cached process within seconds of onReceive returning: an
        // act handed to Rust's loop could sit there unsent (D22). So the process is kept awake for
        // a few seconds (goAsync), and LinkService, which keeps the link, is started in the
        // foreground, which a tap on a notification's action is allowed to do.
        final PendingResult pending = goAsync();
        try {
            if (!LinkService.loaded()) return;
            Act a = read(intent);
            if (a == null) return;
            try {
                ContextCompat.startForegroundService(context, new Intent(context, LinkService.class));
            } catch (RuntimeException ignored) {
                // Refused here: goAsync alone keeps the process up while the act goes.
            }
            // The link and its loop run for the process (once; later calls do nothing), so an act
            // from a process Android just started for this has somewhere to go.
            LinkService.nativeStart(context.getApplicationContext(), context.getFilesDir().getAbsolutePath());
            LinkService.nativeAct(a.kind, a.agent, a.hold, a.text);
        } catch (Throwable ignored) {
            // Nothing done: the notification stays, and the chat answers it.
        } finally {
            if (pending != null) {
                new Handler(Looper.getMainLooper()).postDelayed(pending::finish, KEEP_AWAKE_MS);
            }
        }
    }
}
