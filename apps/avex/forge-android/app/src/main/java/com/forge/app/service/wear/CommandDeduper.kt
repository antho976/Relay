package com.forge.app.service.wear

import javax.inject.Inject
import javax.inject.Singleton

/**
 * Command idempotency (W1 protocol rule): every wrist command carries a UUID; a re-send after a
 * BT flap or a double-tap must never run twice. A small synchronized LRU is enough — commands
 * arrive one at a time over the Data Layer and the window only needs to cover retries.
 */
@Singleton
class CommandDeduper @Inject constructor() {

    private val seen = object : LinkedHashMap<String, Unit>(CAPACITY, 0.75f, false) {
        override fun removeEldestEntry(eldest: MutableMap.MutableEntry<String, Unit>?) = size > CAPACITY
    }

    /** True the FIRST time [commandId] is offered; false on any replay. */
    @Synchronized
    fun isNew(commandId: String): Boolean =
        if (seen.containsKey(commandId)) false else { seen[commandId] = Unit; true }

    private companion object { const val CAPACITY = 128 }
}
