package com.quietsoftware.relaycode;

import android.app.PendingIntent;
import android.app.RemoteInput;
import android.content.BroadcastReceiver;
import android.content.Context;
import android.content.Intent;
import android.os.Bundle;

/**
 * What is done from a notification in the shade (D18): a reply to an agent that asks you, or a held
 * command allowed once or denied. It hands the act to Rust (LinkService.nativeAct), whose loop asks
 * the PC on the link and then updates the notification ("Sent", "Allowed") and clears it.
 *
 * Nothing here may throw: a receiver that throws takes the process down with it.
 */
public class ActionReceiver extends BroadcastReceiver {
    /** The RemoteInput key a reply's words come under. */
    static final String REPLY = "reply";
    private static final String KIND = "kind";
    private static final String HOLD = "hold";
    private static final String ACT = "com.quietsoftware.relaycode.ACT";

    /**
     * The intent a notification's action sends here. A reply's must be mutable (Android writes the
     * words into it); the others are not.
     */
    static PendingIntent intent(Context context, int id, String kind, String agent, long hold, boolean mutable) {
        Intent i = new Intent(context, ActionReceiver.class)
                .setAction(ACT + "." + kind)
                .putExtra(KIND, kind)
                .putExtra(LinkService.AGENT, agent)
                .putExtra(HOLD, hold);
        int flags = PendingIntent.FLAG_UPDATE_CURRENT | (mutable ? PendingIntent.FLAG_MUTABLE : PendingIntent.FLAG_IMMUTABLE);
        // One per notification and act, so a reply and an Allow never share an intent.
        int act = "reply".equals(kind) ? 1 : "allow".equals(kind) ? 2 : 3;
        return PendingIntent.getBroadcast(context, id * 4 + act, i, flags);
    }

    @Override
    public void onReceive(Context context, Intent intent) {
        try {
            if (!LinkService.loaded()) return;
            String kind = intent.getStringExtra(KIND);
            String agent = intent.getStringExtra(LinkService.AGENT);
            long hold = intent.getLongExtra(HOLD, 0);
            String text = "";
            Bundle words = RemoteInput.getResultsFromIntent(intent);
            if (words != null) {
                CharSequence said = words.getCharSequence(REPLY);
                if (said != null) text = said.toString();
            }
            // The link and its loop run for the process (once; later calls do nothing), so an act
            // from a process Android just started for this has somewhere to go.
            LinkService.nativeStart(context.getApplicationContext(), context.getFilesDir().getAbsolutePath());
            LinkService.nativeAct(kind, agent, hold, text);
        } catch (Throwable ignored) {
            // Nothing done: the notification stays, and the chat answers it.
        }
    }
}
