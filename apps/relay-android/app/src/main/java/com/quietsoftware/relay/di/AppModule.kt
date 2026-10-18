package com.quietsoftware.relay.di

import android.content.Context
import androidx.room.Room
import com.quietsoftware.relay.data.Relay
import com.quietsoftware.relay.data.Settings
import com.quietsoftware.relay.data.db.RelayDb
import com.quietsoftware.relay.data.link.CloudStore
import com.quietsoftware.relay.data.link.PcStore
import dagger.Module
import dagger.Provides
import dagger.hilt.InstallIn
import dagger.hilt.android.qualifiers.ApplicationContext
import dagger.hilt.components.SingletonComponent
import kotlinx.coroutines.CoroutineScope
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.SupervisorJob
import javax.inject.Qualifier
import javax.inject.Singleton

/** The app's own scope: the link and the replica outlive any one screen. */
@Qualifier
@Retention(AnnotationRetention.BINARY)
annotation class AppScope

@Module
@InstallIn(SingletonComponent::class)
object AppModule {
    @Provides
    @Singleton
    @AppScope
    fun appScope(): CoroutineScope = CoroutineScope(SupervisorJob() + Dispatchers.Default)

    @Provides
    @Singleton
    fun db(@ApplicationContext context: Context): RelayDb =
        Room.databaseBuilder(context, RelayDb::class.java, "relay.db").build()

    @Provides
    @Singleton
    fun pcStore(@ApplicationContext context: Context): PcStore = PcStore(context)

    @Provides
    @Singleton
    fun cloudStore(@ApplicationContext context: Context): CloudStore = CloudStore(context)

    @Provides
    @Singleton
    fun settings(@ApplicationContext context: Context): Settings = Settings(context)

    @Provides
    @Singleton
    fun relay(@ApplicationContext context: Context, db: RelayDb, pc: PcStore, cloud: CloudStore, @AppScope scope: CoroutineScope): Relay =
        Relay(context, db, pc, scope, cloud)
}
