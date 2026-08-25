package com.forge.app.ui.profile

import androidx.compose.material3.MaterialTheme
import androidx.compose.ui.test.assertIsDisplayed
import androidx.compose.ui.test.junit4.createComposeRule
import androidx.compose.ui.test.onNodeWithText
import com.forge.app.data.repo.ProgressPhoto
import com.forge.app.ui.theme.ForgeTheme
import org.junit.Rule
import org.junit.Test
import org.junit.runner.RunWith
import org.robolectric.RobolectricTestRunner

@RunWith(RobolectricTestRunner::class)
class ProfileGalleryLockTest {

    @get:Rule
    val compose = createComposeRule()

    @Test
    fun lockedStripNeverResolvesPrivatePhotoFile() {
        compose.setContent {
            ForgeTheme {
                GalleryStrip(
                    photos = listOf(ProgressPhoto("private.jpg", takenAtMs = 0L)),
                    fileFor = { error("Locked Profile must not resolve photo files") },
                    onOpenGallery = {},
                    muted = MaterialTheme.colorScheme.onSurfaceVariant,
                    locked = true,
                )
            }
        }

        compose.onNodeWithText("UNLOCK PHOTOS").assertIsDisplayed()
        compose.onNodeWithText("Gallery locked").assertIsDisplayed()
    }
}
