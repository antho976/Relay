package com.forge.app.ui.onboarding

import androidx.compose.animation.AnimatedContent
import androidx.compose.animation.slideInHorizontally
import androidx.compose.animation.slideOutHorizontally
import androidx.compose.animation.togetherWith
import androidx.compose.foundation.background
import androidx.compose.foundation.clickable
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.shape.CircleShape
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.foundation.text.KeyboardOptions
import androidx.compose.material3.Button
import androidx.compose.material3.FilterChip
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.OutlinedTextField
import androidx.compose.material3.Switch
import androidx.compose.material3.Text
import androidx.compose.material3.TextButton
import androidx.compose.runtime.Composable
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableIntStateOf
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.clip
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.text.input.KeyboardCapitalization
import androidx.compose.ui.text.input.KeyboardType
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp
import androidx.hilt.navigation.compose.hiltViewModel
import androidx.lifecycle.ViewModel
import androidx.lifecycle.viewModelScope
import com.forge.app.data.prefs.SettingsRepository
import com.forge.app.data.repo.BodyweightRepository
import dagger.hilt.android.lifecycle.HiltViewModel
import kotlinx.coroutines.launch
import javax.inject.Inject

@HiltViewModel
class OnboardingViewModel @Inject constructor(
    private val settingsRepo: SettingsRepository,
    private val bodyweightRepo: BodyweightRepository,
    private val sampleDataSeeder: com.forge.app.data.repo.SampleDataSeeder
) : ViewModel() {
    fun complete(name: String, useKg: Boolean, goal: String, bodyweightLb: Double?, withSampleData: Boolean = false) {
        viewModelScope.launch {
            settingsRepo.completeOnboarding(name, useKg, goal, bodyweightLb)
            bodyweightLb?.let { bodyweightRepo.log(it) }
            if (withSampleData) sampleDataSeeder.seed()
        }
    }
}

private val GOAL_OPTIONS = listOf(
    "build_muscle" to "Build muscle",
    "get_stronger" to "Get stronger",
    "lose_weight" to "Lose weight",
    "general_fitness" to "General fitness"
)

@Composable
fun OnboardingScreen(
    onFinished: () -> Unit,
    viewModel: OnboardingViewModel = hiltViewModel()
) {
    var step by remember { mutableIntStateOf(0) }
    var name by remember { mutableStateOf("") }
    var useKg by remember { mutableStateOf(false) }
    var goal by remember { mutableStateOf("get_stronger") }
    var bodyweightInput by remember { mutableStateOf("") }
    var loadSampleData by remember { mutableStateOf(false) }

    Box(
        modifier = Modifier
            .fillMaxSize()
            .background(MaterialTheme.colorScheme.background)
            .padding(horizontal = 24.dp, vertical = 48.dp)
    ) {
        Column(modifier = Modifier.fillMaxSize(), verticalArrangement = Arrangement.SpaceBetween) {
            // Step indicator
            Row(horizontalArrangement = Arrangement.spacedBy(8.dp)) {
                repeat(4) { i ->
                    Box(
                        Modifier
                            .size(8.dp)
                            .clip(CircleShape)
                            .background(
                                if (i == step) MaterialTheme.colorScheme.primary
                                else MaterialTheme.colorScheme.outline.copy(alpha = 0.4f)
                            )
                    )
                }
            }

            // Step content
            AnimatedContent(
                targetState = step,
                transitionSpec = {
                    val dir = if (targetState > initialState) 1 else -1
                    slideInHorizontally { it * dir } togetherWith slideOutHorizontally { -it * dir }
                },
                label = "onboarding_step"
            ) { s ->
                Column(
                    modifier = Modifier.fillMaxWidth(),
                    verticalArrangement = Arrangement.spacedBy(24.dp)
                ) {
                    when (s) {
                        0 -> StepName(name = name, onNameChange = { name = it })
                        1 -> StepUnits(useKg = useKg, onToggle = { useKg = it })
                        2 -> StepGoal(selected = goal, onSelect = { goal = it })
                        3 -> StepBodyweight(
                            input = bodyweightInput,
                            useKg = useKg,
                            onInputChange = { bodyweightInput = it },
                            sampleData = loadSampleData,
                            onSampleDataToggle = { loadSampleData = it }
                        )
                    }
                }
            }

            // Navigation
            Row(
                modifier = Modifier.fillMaxWidth(),
                horizontalArrangement = Arrangement.SpaceBetween,
                verticalAlignment = Alignment.CenterVertically
            ) {
                if (step > 0) {
                    TextButton(onClick = { step-- }) { Text("Back") }
                } else {
                    TextButton(onClick = {
                        viewModel.complete("", false, goal, null)
                        onFinished()
                    }) { Text("Skip") }
                }
                Button(
                    onClick = {
                        if (step < 3) {
                            step++
                        } else {
                            val bwLb = bodyweightInput.toDoubleOrNull()?.let { raw ->
                                if (useKg) raw * 2.20462 else raw
                            }
                            viewModel.complete(name, useKg, goal, bwLb, loadSampleData)
                            onFinished()
                        }
                    }
                ) {
                    Text(if (step < 3) "Next" else "Let's go")
                }
            }
        }
    }
}

@Composable
private fun StepName(name: String, onNameChange: (String) -> Unit) {
    Column(verticalArrangement = Arrangement.spacedBy(16.dp)) {
        Text("FORGE", style = MaterialTheme.typography.labelLarge,
            color = MaterialTheme.colorScheme.primary, fontWeight = FontWeight.Black)
        Text("What do you go by?", style = MaterialTheme.typography.headlineLarge,
            fontWeight = FontWeight.Black, lineHeight = 38.sp)
        OutlinedTextField(
            value = name,
            onValueChange = onNameChange,
            modifier = Modifier.fillMaxWidth(),
            placeholder = { Text("Your name") },
            keyboardOptions = KeyboardOptions(capitalization = KeyboardCapitalization.Words),
            singleLine = true
        )
    }
}

@Composable
private fun StepUnits(useKg: Boolean, onToggle: (Boolean) -> Unit) {
    Column(verticalArrangement = Arrangement.spacedBy(16.dp)) {
        Text("Weight units", style = MaterialTheme.typography.headlineLarge,
            fontWeight = FontWeight.Black, lineHeight = 38.sp)
        Text("All weights in the app use this unit. You can change it later in Settings.",
            style = MaterialTheme.typography.bodyMedium,
            color = MaterialTheme.colorScheme.onSurfaceVariant)
        Row(
            modifier = Modifier
                .fillMaxWidth()
                .background(MaterialTheme.colorScheme.surfaceVariant, RoundedCornerShape(12.dp))
                .padding(16.dp),
            horizontalArrangement = Arrangement.SpaceBetween,
            verticalAlignment = Alignment.CenterVertically
        ) {
            Text(if (useKg) "Kilograms (kg)" else "Pounds (lb)",
                style = MaterialTheme.typography.bodyLarge, fontWeight = FontWeight.Medium)
            Switch(checked = useKg, onCheckedChange = onToggle)
        }
    }
}

@Composable
private fun StepGoal(selected: String, onSelect: (String) -> Unit) {
    Column(verticalArrangement = Arrangement.spacedBy(12.dp)) {
        Text("What's your main goal?", style = MaterialTheme.typography.headlineLarge,
            fontWeight = FontWeight.Black, lineHeight = 38.sp)
        GOAL_OPTIONS.forEach { (key, label) ->
            val isSelected = selected == key
            Box(
                modifier = Modifier
                    .fillMaxWidth()
                    .clip(RoundedCornerShape(12.dp))
                    .background(
                        if (isSelected) MaterialTheme.colorScheme.primary.copy(alpha = 0.15f)
                        else MaterialTheme.colorScheme.surfaceVariant
                    )
                    .clickable { onSelect(key) }
                    .padding(16.dp),
                contentAlignment = Alignment.CenterStart
            ) {
                Row(
                    modifier = Modifier.fillMaxWidth(),
                    horizontalArrangement = Arrangement.SpaceBetween,
                    verticalAlignment = Alignment.CenterVertically
                ) {
                    Text(label, style = MaterialTheme.typography.bodyLarge,
                        fontWeight = if (isSelected) FontWeight.SemiBold else FontWeight.Normal,
                        color = if (isSelected) MaterialTheme.colorScheme.primary
                                else MaterialTheme.colorScheme.onSurface)
                    if (isSelected) {
                        Text("✓", style = MaterialTheme.typography.bodyLarge,
                            color = MaterialTheme.colorScheme.primary, fontWeight = FontWeight.Bold)
                    }
                }
            }
        }
    }
}

@Composable
private fun StepBodyweight(
    input: String,
    useKg: Boolean,
    onInputChange: (String) -> Unit,
    sampleData: Boolean,
    onSampleDataToggle: (Boolean) -> Unit
) {
    val unitLabel = if (useKg) "kg" else "lb"
    Column(verticalArrangement = Arrangement.spacedBy(16.dp)) {
        Text("Starting bodyweight", style = MaterialTheme.typography.headlineLarge,
            fontWeight = FontWeight.Black, lineHeight = 38.sp)
        Text("Optional — used for relative strength comparisons.",
            style = MaterialTheme.typography.bodyMedium,
            color = MaterialTheme.colorScheme.onSurfaceVariant)
        OutlinedTextField(
            value = input,
            onValueChange = onInputChange,
            modifier = Modifier.fillMaxWidth(),
            placeholder = { Text("e.g. 170") },
            suffix = { Text(unitLabel) },
            keyboardOptions = KeyboardOptions(keyboardType = KeyboardType.Decimal),
            singleLine = true
        )
        Row(
            modifier = Modifier
                .fillMaxWidth()
                .clip(RoundedCornerShape(12.dp))
                .background(MaterialTheme.colorScheme.surfaceVariant)
                .padding(16.dp),
            horizontalArrangement = Arrangement.SpaceBetween,
            verticalAlignment = Alignment.CenterVertically
        ) {
            Column(modifier = Modifier.weight(1f)) {
                Text("Load 8 weeks of sample data",
                    style = MaterialTheme.typography.bodyMedium, fontWeight = FontWeight.Medium)
                Text("Fills the app with realistic fake sessions for a trial run",
                    style = MaterialTheme.typography.labelSmall,
                    color = MaterialTheme.colorScheme.onSurfaceVariant)
            }
            Switch(checked = sampleData, onCheckedChange = onSampleDataToggle)
        }
    }
}
