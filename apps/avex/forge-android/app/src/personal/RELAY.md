# The personal build: Avex → Relay

Antho's own Avex sends its training history to his Relay server, so the PC has every workout
without an export by hand (antho976/Relay, `docs/CLOUD.md`). Only this build type does. The public
Avex, the one on the Play Store, is built from `src/main` and `release` alone, holds no INTERNET
permission, and keeps its promise that it "physically cannot upload anything".

## What is here

Everything lives in this source set, and nothing in `src/main` refers to it:

- `AndroidManifest.xml`: the INTERNET permission, a second launcher entry ("Avex → Relay"), and a
  provider that starts the uploader when the app starts.
- `relay/RelayWire.kt`, `relay/RelayClient.kt`: Relay's door, wire v1, over an OkHttp WebSocket
  (the same handshake Tally uses). OkHttp is a `personalImplementation` dependency only.
- `relay/RelayUploader.kt`: pairing, and `gym.upload` of the whole `avex_export.json`.
- `relay/RelayUploadWorker.kt`: a WorkManager job that writes the export
  (`BackupRepository.exportFullDataJson`, the same file Settings, Export, Training history writes)
  and sends it once the phone is online. A change to a workout, its sets, cardio or coach goals
  schedules it 3 s later. A newer change replaces a send still waiting, and a send is skipped while
  a workout is in progress, because finishing the workout schedules the next one.
- `relay/RelayConnect.kt`: asking Antho's server to let this phone in (`/api/connect`, kind
  `avex`): the phone first checks the address answers as his bot (`GET /api/connect`, with `avex`
  among its kinds and the cloud copy on), the server DMs him Allow on Discord, and the phone
  collects its pairing link. No address is built in: the one typed on the screen is kept in the
  app's private preferences.
- `relay/RelaySyncActivity.kt`: the bot's address, Connect, the last send, send now, forget.

## Building and pairing

```sh
./gradlew -p forge-android :app:assemblePersonal
```

The build signs with the release key when `keystore.properties` is complete, like `release`. It
keeps the release application id, so it installs over a sideloaded release build. It does not
install over the Play Store install, which Play signs with its own key. To move from that, take a
backup in Avex (Settings, Backup), install this build, and restore.

In "Avex → Relay", type your bot's address (its `PUBLIC_URL`, e.g. `bot.<domain>`), tap Connect,
then Allow on the Discord message the bot sends (it shows the same four-character code). The
address reads like Tally 2's: a bare host, `https://…`, `wss://…/relay` or a pasted `/api/connect`
all work. It is kept for next time, and Forget keeps it too. That is all: the phone pairs with the cloud copy and
sends the history from then on (antho976/Relay, docs/CLOUD.md, "Connecting through your server").

## Checking it

`./gradlew -p forge-android :app:compilePersonalKotlin` compiles this source set. AGP gives the
`personal` build type no unit-test task, so it has no tests of its own.
