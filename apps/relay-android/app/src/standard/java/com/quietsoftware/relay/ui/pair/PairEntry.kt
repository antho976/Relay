package com.quietsoftware.relay.ui.pair

import androidx.compose.runtime.Composable
import com.quietsoftware.relay.ui.Nav

/**
 * Pairing in the builds anyone can install (debug and release): scan, paste or type what
 * `relay remote pair` printed on the PC. The personal build pairs through its owner's server
 * instead (src/personal).
 */
@Composable
fun PairEntry(link: String?, nav: Nav) = PairScreen(link, nav)
