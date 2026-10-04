package com.margin.app.ui.plan

import androidx.compose.runtime.Composable
import com.margin.app.ui.common.PageTitle
import com.margin.app.ui.common.TopBar
import androidx.compose.foundation.layout.Column
import com.margin.app.ui.nav.AppNav

@Composable
fun PlanTab(nav: AppNav) {
    Column { TopBar(onBack = nav::back); PageTitle("PlanTab") }
}

@Composable
fun BudgetEditRoute(nav: AppNav) {
    Column { TopBar(onBack = nav::back); PageTitle("BudgetEditRoute") }
}

@Composable
fun BillEditRoute(nav: AppNav) {
    Column { TopBar(onBack = nav::back); PageTitle("BillEditRoute") }
}

@Composable
fun GoalRoute(nav: AppNav) {
    Column { TopBar(onBack = nav::back); PageTitle("GoalRoute") }
}

@Composable
fun GoalEditRoute(nav: AppNav) {
    Column { TopBar(onBack = nav::back); PageTitle("GoalEditRoute") }
}

