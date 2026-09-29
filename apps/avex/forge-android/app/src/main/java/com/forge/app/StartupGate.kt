package com.forge.app

import android.content.Context
import androidx.datastore.core.DataStore
import androidx.sqlite.db.SupportSQLiteDatabase
import androidx.sqlite.db.SupportSQLiteOpenHelper
import androidx.sqlite.db.framework.FrameworkSQLiteOpenHelperFactory
import java.io.IOException
import kotlinx.coroutines.CompletableDeferred
import kotlinx.coroutines.flow.Flow
import kotlinx.coroutines.flow.emitAll
import kotlinx.coroutines.flow.flow
import kotlinx.coroutines.runBlocking

/** No live storage may open until boot-time recovery has finished with a coherent dataset. */
internal class StartupGate {
    private val ready = CompletableDeferred<Unit>()
    suspend fun await() = ready.await()
    fun complete() { ready.complete(Unit) }
    fun fail(cause: Throwable) { ready.completeExceptionally(IOException("Startup recovery failed", cause)) }
}

internal suspend fun Context.awaitStorageReady() {
    (applicationContext as? ForgeApp)?.awaitStorageReady()
}

/** Defer even constructing the delegate until the barrier opens; covers edits as well as reads. */
internal class GatedDataStore<T>(
    private val awaitReady: suspend () -> Unit,
    private val delegate: () -> DataStore<T>
) : DataStore<T> {
    override val data: Flow<T> = flow { awaitReady(); emitAll(delegate().data) }
    override suspend fun updateData(transform: suspend (T) -> T): T {
        awaitReady()
        return delegate().updateData(transform)
    }
}

/** Room opens from its query/transaction executor. Validation uses the ordinary ungated factory. */
internal class StartupOpenHelperFactory(private val context: Context) : SupportSQLiteOpenHelper.Factory {
    override fun create(configuration: SupportSQLiteOpenHelper.Configuration): SupportSQLiteOpenHelper {
        val helper = FrameworkSQLiteOpenHelperFactory().create(configuration)
        return object : SupportSQLiteOpenHelper by helper {
            // The gate only ever opens once, so after the first successful wait every later access
            // skips the runBlocking (an event loop per database access). A failed gate never sets
            // this, so it keeps throwing.
            @Volatile private var gateOpen = false

            private fun awaitGate() {
                if (gateOpen) return
                runBlocking { context.awaitStorageReady() }
                gateOpen = true
            }

            override val writableDatabase: SupportSQLiteDatabase
                get() { awaitGate(); return helper.writableDatabase }
            override val readableDatabase: SupportSQLiteDatabase
                get() { awaitGate(); return helper.readableDatabase }
        }
    }
}
