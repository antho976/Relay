package com.tally.app.ui.common

import androidx.lifecycle.SavedStateHandle
import kotlinx.serialization.KSerializer
import kotlinx.serialization.json.Json

/*
 * Editor drafts outlive the process. Each editor keeps its working copy in its SavedStateHandle
 * as one JSON string, on every change, and reads it back before it loads anything from the
 * database: the system can kill a backgrounded app while the owner checks a balance in their
 * bank's app, and coming back must not mean typing it all again. A draft that no longer decodes
 * (a shape from an older version) is dropped, and the editor starts as it would have.
 */

/** The SavedStateHandle key every editor keeps its draft under. No route argument uses it. */
const val DRAFT_KEY = "draft"

private val draftJson = Json { ignoreUnknownKeys = true }

/** The draft kept under [key], or null when there is none or it no longer reads. */
fun <T> SavedStateHandle.savedDraft(serializer: KSerializer<T>, key: String = DRAFT_KEY): T? {
    val text = get<String>(key) ?: return null
    return runCatching { draftJson.decodeFromString(serializer, text) }.getOrNull()
}

/** Keeps [draft] under [key], replacing the one there. */
fun <T> SavedStateHandle.keepDraft(serializer: KSerializer<T>, draft: T, key: String = DRAFT_KEY) {
    set(key, draftJson.encodeToString(serializer, draft))
}
