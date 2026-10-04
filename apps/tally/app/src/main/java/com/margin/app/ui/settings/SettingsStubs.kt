package com.margin.app.ui.settings

import androidx.compose.runtime.Composable
import com.margin.app.ui.common.PageTitle
import com.margin.app.ui.common.TopBar
import androidx.compose.foundation.layout.Column
import com.margin.app.ui.nav.AppNav

@Composable
fun SettingsRoute(nav: AppNav) {
    Column { TopBar(onBack = nav::back); PageTitle("SettingsRoute") }
}

@Composable
fun AppearanceRoute(nav: AppNav) {
    Column { TopBar(onBack = nav::back); PageTitle("AppearanceRoute") }
}

@Composable
fun FormatRoute(nav: AppNav) {
    Column { TopBar(onBack = nav::back); PageTitle("FormatRoute") }
}

@Composable
fun DataRoute(nav: AppNav) {
    Column { TopBar(onBack = nav::back); PageTitle("DataRoute") }
}

