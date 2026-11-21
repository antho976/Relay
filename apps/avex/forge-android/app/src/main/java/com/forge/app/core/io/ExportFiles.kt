package com.forge.app.core.io

import android.content.Context
import java.io.File

/**
 * Where every file Avex hands to the system share sheet is written.
 *
 * The FileProvider used to be rooted at `filesDir` itself (`<files-path path="." />`), so its
 * addressable surface was the whole of app-private storage: `progress_photos/`,
 * `datastore/forge_settings.preferences_pb`, `avatar.jpg` and `crashes/` alongside the exports the
 * root's name implied. Nothing granted a URI to any of those — the provider is not exported and
 * every grant is explicit — but the blast radius of a future mistake (a share feature building a
 * URI from a caller-influenced filename) was every private file the app owns rather than the
 * handful of artifacts it means to share.
 *
 * Exports live under one directory now and the provider maps that directory only. It also gives
 * Auto Backup a path it can exclude cleanly.
 */
fun exportsDir(context: Context): File =
    File(context.filesDir, EXPORTS_DIR).apply { mkdirs() }

/** An export artifact by name, with its directory created. */
fun exportFile(context: Context, name: String): File = File(exportsDir(context), name)

const val EXPORTS_DIR = "exports"
