package com.forge.app.relay

import android.os.Bundle
import androidx.activity.ComponentActivity
import androidx.activity.compose.setContent
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.padding
import androidx.compose.material3.Button
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.OutlinedButton
import androidx.compose.material3.OutlinedTextField
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
 * PERSONAL BUILD ONLY. "Avex → Relay": pair with Relay's cloud copy by pasting the link the server
 * printed, see how the last send went, send now, or forget the server.
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
    var link by remember { mutableStateOf("") }
    var busy by remember { mutableStateOf(false) }
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
            OutlinedTextField(
                value = link,
                onValueChange = { link = it },
                label = { Text("relay://pair… link") },
                singleLine = true,
                modifier = Modifier.fillMaxWidth(),
            )
            Button(
                enabled = !busy && link.isNotBlank(),
                onClick = {
                    busy = true
                    message = "Waiting for the server to approve this phone…"
                    scope.launch {
                        message = try {
                            RelayUploader.pair(store, link)
                            RelayUploadWorker.schedule(context, delaySeconds = 0, byHand = true)
                            "Paired. Sending your history now."
                        } catch (e: RelayFailure) {
                            e.reason
                        }
                        busy = false
                    }
                },
            ) { Text("Pair") }
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
