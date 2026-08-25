package com.forge.app.di

import com.forge.app.core.time.Clock
import com.forge.app.core.time.ElapsedClock
import com.forge.app.core.time.SystemClock
import com.forge.app.core.time.SystemElapsedClock
import dagger.Binds
import dagger.Module
import dagger.hilt.InstallIn
import dagger.hilt.components.SingletonComponent
import javax.inject.Singleton

@Module
@InstallIn(SingletonComponent::class)
abstract class ClockModule {
    @Binds
    @Singleton
    abstract fun bindClock(impl: SystemClock): Clock

    @Binds
    @Singleton
    abstract fun bindElapsedClock(impl: SystemElapsedClock): ElapsedClock
}
