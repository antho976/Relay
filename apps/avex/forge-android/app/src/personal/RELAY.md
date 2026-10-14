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
  `avex`): the server DMs him Allow on Discord, and the phone collects its pairing link. The
  server's address lives here and nowhere else.
- `relay/RelaySyncActivity.kt`: Connect, see the last send, send now, forget.

## Building and pairing

```sh
./gradlew -p forge-android :app:assemblePersonal
```

The build signs with the release key when `keystore.properties` is complete, like `release`. It
keeps the release application id, so it installs over a sideloaded release build. It does not
install over the Play Store install, which Play signs with its own key. To move from that, take a
backup in Avex (Settings, Backup), install this build, and restore.

In "Avex → Relay", tap Connect to my server, then Allow on the Discord message the server sends
(it shows the same four-character code). That is all: the phone pairs with the cloud copy and
sends the history from then on (antho976/Relay, docs/CLOUD.md, "Connecting through your server").

## Not built here

Written in an agent session with no Android SDK: this source set has not been compiled.
