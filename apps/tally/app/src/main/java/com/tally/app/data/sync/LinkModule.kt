package com.tally.app.data.sync

import com.tally.link.CloudApi
import com.tally.link.RelayClient
import dagger.Module
import dagger.Provides
import dagger.hilt.InstallIn
import dagger.hilt.components.SingletonComponent
import okhttp3.OkHttpClient
import javax.inject.Singleton

/**
 * The link to Relay on the owner's server (the :link module): one HTTP client for both halves, the
 * request to connect and the door. The only network code in the app lives in data/sync and :link.
 */
@Module
@InstallIn(SingletonComponent::class)
object LinkModule {
    @Provides @Singleton fun http(): OkHttpClient = OkHttpClient()
    @Provides @Singleton fun relay(http: OkHttpClient): RelayClient = RelayClient(http)
    @Provides @Singleton fun cloud(http: OkHttpClient): CloudApi = CloudApi(http)
}
