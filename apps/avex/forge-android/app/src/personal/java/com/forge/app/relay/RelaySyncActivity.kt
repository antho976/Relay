package com.forge.app.relay

import android.os.Bundle
import androidx.activity.ComponentActivity
import androidx.activity.compose.setContent
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.padding
import androidx.compose.material3.Button
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.OutlinedButton
import androidx.compose.material3.Surface
import androidx.compose.material3.Text
import androidx.compose.material3.darkColorScheme
import androidx.compose.runtime.Composable
import androidx.compose.runtime.collectAsState
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.rememberCoroutineScope
import androidx.compose.runtime.setValue
import androidx.compose.ui.Modifier
import androidx.compose.ui.unit.dp
import kotlinx.coroutines.launch

/**
 * PERSONAL BUILD ONLY. "Avex → Relay": connect to Antho's server with one tap on Discord, see how
 * the last send went, send now, or forget the server.
 */
class RelaySyncActivity : ComponentActivity() {
    override fun onCreate(savedInstanceState: Bundle?) {
        super.onCreate(savedInstanceState)
        val store = RelayStore(this)
        setContent {
            MaterialTheme(colorScheme = darkColorScheme()) {
                Surface(Modifier.fillMaxSize()) { RelaySyncScreen(store) }
            }
        }
    }
}

@Composable
private fun RelaySyncScreen(store: RelayStore) {
    val scope = rememberCoroutineScope()
    val tick by RelayStore.changes.collectAsState()
    var busy by remember { mutableStateOf(false) }
    var code by remember { mutableStateOf<String?>(null) }
    var message by remember { mutableStateOf<String?>(null) }
    val paired = remember(tick) { store.paired }
    val status = remember(tick) { store.status() }
    val context = androidx.compose.ui.platform.LocalContext.current

    Column(Modifier.padding(24.dp), verticalArrangement = Arrangement.spacedBy(16.dp)) {
        Text("Avex → Relay", style = MaterialTheme.typography.headlineSmall)
        Text(
            "This build sends your training history to your own Relay server after each workout, " +
                "and your PC takes it from there. The public Avex never does.",
            style = MaterialTheme.typography.bodyMedium,
        )
        Text(status, style = MaterialTheme.typography.bodyMedium)
        if (paired == null) {
            code?.let { Text("Code $it", style = MaterialTheme.typography.headlineMedium) }
            Button(
                enabled = !busy,
                onClick = {
                    busy = true
                    code = null
                    message = "Asking your server…"
                    scope.launch {
                        message = try {
                            connect(store) { asked ->
                                code = asked.code
                                message = if (asked.notified) "Your bot sent you a message on Discord. Tap Allow if it shows this code."
                                else "Your bot could not message you. Allow the request on its dashboard, Devices, where it shows this code."
                            }
                            RelayUploadWorker.schedule(context, delaySeconds = 0, byHand = true)
                            code = null
                            "Connected. Sending your history now."
                        } catch (e: RelayFailure) {
                            e.reason
                        }
                        busy = false
                    }
                },
            ) { Text(if (busy) "Waiting for you on Discord…" else "Connect to my server") }
        } else {
            Button(onClick = {
                RelayUploadWorker.schedule(context, delaySeconds = 0, byHand = true)
                message = "Sending as soon as the phone is online."
            }) { Text("Send now") }
            OutlinedButton(onClick = {
                store.forget()
                message = "Forgotten. Revoke this phone on the server too: relay remote revoke ${paired.device}"
            }) { Text("Forget this server") }
        }
        message?.let { Text(it, style = MaterialTheme.typography.bodySmall) }
    }
}

/**
 * Asks the server, waits for Antho's tap (showing the code through [onAsked]), collects the pairing
 * link and pairs with the cloud copy. Throws [RelayFailure] with what to tell the person.
 */
private suspend fun connect(store: RelayStore, onAsked: (ConnectRequest) -> Unit) {
    val api = RelayConnect()
    val asked = api.ask("Avex on ${android.os.Build.MODEL}".take(48))
    onAsked(asked)
    var troubles = 0
    while (true) {
        kotlinx.coroutines.delay(2_000)
        when (val reply = api.claim(asked)) {
            ConnectReply.Waiting -> {}
            is ConnectReply.Ready -> {
                RelayUploader.pair(store, reply.link)
                return
            }
            ConnectReply.Denied -> throw RelayFailure("The request was denied on Discord.")
            ConnectReply.Expired -> throw RelayFailure("The request expired. Ask again.")
            ConnectReply.Spent -> throw RelayFailure("The request was used up. Ask again.")
            is ConnectReply.Trouble -> if (++troubles >= 10) throw RelayFailure(reply.reason)
        }
    }
}
