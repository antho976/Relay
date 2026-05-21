package com.forge.app.di

import android.content.Context
import androidx.room.Room
import com.forge.app.data.db.ForgeDatabase
import com.forge.app.data.db.dao.AppMetaDao
import dagger.Module
import dagger.Provides
import dagger.hilt.InstallIn
import dagger.hilt.android.qualifiers.ApplicationContext
import dagger.hilt.components.SingletonComponent
import javax.inject.Singleton

@Module
@InstallIn(SingletonComponent::class)
object DatabaseModule {

    @Provides
    @Singleton
    fun provideDatabase(@ApplicationContext context: Context): ForgeDatabase =
        Room.databaseBuilder(context, ForgeDatabase::class.java, "forge.db")
            // Phase 2 will define explicit migrations; until then, schema is single-version.
            .build()

    @Provides
    fun provideAppMetaDao(db: ForgeDatabase): AppMetaDao = db.appMetaDao()
}
