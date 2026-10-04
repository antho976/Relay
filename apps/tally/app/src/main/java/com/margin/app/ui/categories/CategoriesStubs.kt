package com.margin.app.ui.categories

import androidx.compose.runtime.Composable
import com.margin.app.ui.common.PageTitle
import com.margin.app.ui.common.TopBar
import androidx.compose.foundation.layout.Column
import com.margin.app.ui.nav.AppNav

@Composable
fun CategoriesRoute(nav: AppNav) {
    Column { TopBar(onBack = nav::back); PageTitle("CategoriesRoute") }
}

@Composable
fun CategoryEditRoute(nav: AppNav) {
    Column { TopBar(onBack = nav::back); PageTitle("CategoryEditRoute") }
}

