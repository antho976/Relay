@file:OptIn(androidx.compose.material3.ExperimentalMaterial3Api::class)

package com.forge.app.ui.academy

import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.PaddingValues
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.lazy.LazyColumn
import androidx.compose.foundation.lazy.rememberLazyListState
import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.automirrored.filled.ArrowBack
import androidx.compose.material3.Icon
import androidx.compose.material3.IconButton
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Scaffold
import androidx.compose.material3.Text
import androidx.compose.material3.TopAppBar
import androidx.compose.material3.TopAppBarDefaults
import androidx.compose.runtime.Composable
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.derivedStateOf
import androidx.compose.runtime.getValue
import androidx.compose.runtime.remember
import androidx.compose.runtime.snapshotFlow
import androidx.compose.ui.Modifier
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.text.font.FontStyle
import androidx.compose.ui.unit.dp
import androidx.hilt.navigation.compose.hiltViewModel
import androidx.lifecycle.compose.collectAsStateWithLifecycle
import com.forge.app.ui.common.InlineEmptyHint

/**
 * One Library article (DESIGN §3, detail archetype) — scoped to a single item, serif title and its
 * context, and nothing borrowed from the dashboard.
 *
 * There is no figure row, no chart and no lens pills here, because an article has exactly one thing
 * to say and reading it is the only interaction. The one piece of state the screen keeps is whether
 * the reader reached the end, which is recorded from a real scroll rather than from a dismissal:
 * counting a bounce as a read would corrupt the only signal the Library keeps.
 */
@Composable
fun ArticleScreen(
    onBack: () -> Unit,
    viewModel: ArticleViewModel = hiltViewModel()
) {
    val state by viewModel.state.collectAsStateWithLifecycle()
    val onBg = MaterialTheme.colorScheme.onBackground
    val muted = MaterialTheme.colorScheme.onSurfaceVariant

    val listState = rememberLazyListState()
    // The tail spacer is the last item, so seeing it means the sources have cleared the fold.
    val reachedEnd by remember {
        derivedStateOf {
            listState.layoutInfo.visibleItemsInfo.any { it.key == KEY_TAIL }
        }
    }
    LaunchedEffect(listState) {
        snapshotFlow { reachedEnd }.collect { if (it) viewModel.onReachedEnd() }
    }

    Scaffold(
        topBar = {
            TopAppBar(
                // §4.6: chrome only. The article names itself in its own serif hero below, and the
                // bell is Home-only so it is not repeated here.
                title = {},
                navigationIcon = {
                    IconButton(onClick = onBack) {
                        Icon(
                            Icons.AutoMirrored.Filled.ArrowBack,
                            contentDescription = "Back",
                            tint = muted
                        )
                    }
                },
                colors = TopAppBarDefaults.topAppBarColors(containerColor = Color.Transparent)
            )
        },
        containerColor = Color.Transparent
    ) { inner ->
        val article = state.article
        if (article == null) {
            // A retired or mistyped id. §12's error state: a quiet inline line wording the
            // consequence, never a dialog and never a crash.
            Column(Modifier.fillMaxSize().padding(inner).padding(horizontal = 24.dp)) {
                Spacer(Modifier.height(24.dp))
                InlineEmptyHint("That article is no longer in the library", muted.copy(alpha = 0.65f))
            }
            return@Scaffold
        }

        LazyColumn(
            state = listState,
            modifier = Modifier.fillMaxSize().padding(inner),
            contentPadding = PaddingValues(horizontal = 24.dp)
        ) {
            item("head") {
                Spacer(Modifier.height(8.dp))
                // §11: a serif title takes no terminal period.
                Text(article.title, style = MaterialTheme.typography.headlineSmall, color = onBg)
                Spacer(Modifier.height(8.dp))
                Text(
                    "${article.topic.displayName} · ${article.level.displayName} · ${article.readMinutes} MIN"
                        .uppercase(),
                    style = MaterialTheme.typography.labelMedium,
                    color = muted.copy(alpha = 0.65f)
                )
                Spacer(Modifier.height(14.dp))
                // §11: italic is the aside voice. The deck is the article's one-line answer, so it
                // sits above the body rather than repeating inside it.
                Text(
                    article.deck,
                    style = MaterialTheme.typography.bodyMedium.copy(fontStyle = FontStyle.Italic),
                    color = muted
                )
                Spacer(Modifier.height(20.dp))
            }

            item("body") {
                Column(Modifier.fillMaxWidth()) { ArticleBody(article) }
            }

            item(KEY_TAIL) { Spacer(Modifier.height(56.dp)) }
        }
    }
}

private const val KEY_TAIL = "article-tail"
