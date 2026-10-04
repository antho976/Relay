package com.margin.app.ui.accounts

import androidx.compose.runtime.Composable
import com.margin.app.ui.common.PageTitle
import com.margin.app.ui.common.TopBar
import androidx.compose.foundation.layout.Column
import com.margin.app.ui.nav.AppNav

@Composable
fun AccountsRoute(nav: AppNav) {
    Column { TopBar(onBack = nav::back); PageTitle("AccountsRoute") }
}

@Composable
fun AccountEditRoute(nav: AppNav) {
    Column { TopBar(onBack = nav::back); PageTitle("AccountEditRoute") }
}

