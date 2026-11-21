package com.forge.app.data.db

import androidx.room.Room
import androidx.test.core.app.ApplicationProvider
import com.forge.app.data.db.entities.LoggedExercise
import com.forge.app.data.db.entities.LoggedSet
import com.forge.app.data.db.entities.Session

/**
 * An in-memory [ForgeDatabase] for JVM tests, plus the fixtures the DAO suites build on.
 *
 * Room runs perfectly well on the JVM under Robolectric, which matters because the DAO layer is
 * where this app keeps the user's training history and it had NO tests at all — 34 DAOs and 36
 * entities, guarded only by a migration suite that could run on an emulator and nowhere else.
 *
 * The queries worth testing are not the CRUD ones. They are the dozen aggregate queries that all
 * repeat the same four-clause exclusion contract (see [com.forge.app.data.db.dao.LoggedSetDao]):
 * untracked sessions, unfinished sessions, assisted sets and timed holds must never reach a
 * strength maximum. Every one of those clauses is a fix for a shipped bug, and dropping one from a
 * single query puts two screens into disagreement about the same lift — silently, and only for
 * users whose history happens to contain the excluded row.
 */
internal fun inMemoryForgeDb(): ForgeDatabase =
    Room.inMemoryDatabaseBuilder(
        ApplicationProvider.getApplicationContext(),
        ForgeDatabase::class.java
    )
        // The suites drive suspending DAO calls through runTest rather than a real dispatcher, so
        // main-thread queries are what actually happens here.
        .allowMainThreadQueries()
        .build()

/** A finished, tracked session — the shape every "best ever" query is supposed to count. */
internal fun session(
    id: Long = 0,
    startedAt: Long = 1_000_000L,
    finishedAt: Long? = 1_003_600_000L,
    untracked: Boolean = false,
    dayKey: String = "upper-a"
) = Session(
    id = id,
    dayKey = dayKey,
    startedAt = startedAt,
    finishedAt = finishedAt,
    isUntracked = untracked
)

internal fun loggedExercise(
    id: Long = 0,
    sessionId: Long,
    exerciseId: String = "bench",
    orderIndex: Int = 0,
    skipped: Boolean = false
) = LoggedExercise(
    id = id,
    sessionId = sessionId,
    exerciseId = exerciseId,
    orderIndex = orderIndex,
    skipped = skipped
)

internal fun loggedSet(
    id: Long = 0,
    loggedExerciseId: Long,
    setIndex: Int = 0,
    weightLb: Double? = 100.0,
    reps: Int = 5,
    assisted: Boolean = false,
    durationSeconds: Int? = null,
    completedAt: Long = 1_000_000L
) = LoggedSet(
    id = id,
    loggedExerciseId = loggedExerciseId,
    setIndex = setIndex,
    weightText = weightLb?.toString() ?: "BW",
    weightLb = weightLb,
    reps = reps,
    completedAt = completedAt,
    isAssisted = assisted,
    durationSeconds = durationSeconds
)
