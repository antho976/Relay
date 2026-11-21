package com.forge.app.ui.profile

import androidx.activity.compose.rememberLauncherForActivityResult
import androidx.activity.result.PickVisualMediaRequest
import androidx.activity.result.contract.ActivityResultContracts
import androidx.compose.runtime.Composable
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.getValue
import androidx.compose.runtime.key
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.setValue
import androidx.hilt.navigation.compose.hiltViewModel
import androidx.lifecycle.compose.collectAsStateWithLifecycle
import com.forge.app.domain.schedule.ScheduleHistory
import com.forge.app.domain.vacation.VacationCalendar
import com.forge.app.security.LocalAppLock
import com.forge.app.ui.common.DayLogSheet
import com.forge.app.ui.theme.LocalForgeSettings
import java.time.LocalDate

/**
 * Profile (2026-10-21 redesign): the page itself is [ProfilePage], drawn from plain data; this
 * wires it to its ViewModels, the gallery lock, and the sheets it opens (cover chooser, weigh-in,
 * body fat, sizes, and the day sheet behind a tapped day of the Consistency month).
 */
@Composable
fun ProfileScreen(
    // Null when shown without a back arrow; a real callback as a pushed route.
    onBack: (() -> Unit)? = null,
    onOpenPhotoGallery: () -> Unit = {},
    onOpenCamera: () -> Unit = {},
    onOpenMeasurements: () -> Unit = {},
    /** Drill-downs out of the day sheet — the same destinations History opens. */
    onOpenSession: (Long) -> Unit = {},
    onOpenCardio: (Long) -> Unit = {},
    viewModel: ProfileViewModel = hiltViewModel(),
    measurementsVm: BodyMeasurementsViewModel = hiltViewModel(),
    leanMassVm: LeanMassViewModel = hiltViewModel(),
) {
    val state by viewModel.state.collectAsStateWithLifecycle()
    // Sessions or cardio deleted from the day sheet's drill-down change the totals and lit days;
    // the ViewModel outlives that trip, so re-read them when the screen comes back.
    val lifecycleOwner = androidx.lifecycle.compose.LocalLifecycleOwner.current
    androidx.compose.runtime.DisposableEffect(lifecycleOwner) {
        val observer = androidx.lifecycle.LifecycleEventObserver { _, event ->
            if (event == androidx.lifecycle.Lifecycle.Event.ON_RESUME) {
                viewModel.refresh()
                leanMassVm.refreshConnection()
            }
        }
        lifecycleOwner.lifecycle.addObserver(observer)
        onDispose { lifecycleOwner.lifecycle.removeObserver(observer) }
    }
    val dayLog by viewModel.dayLog.collectAsStateWithLifecycle()
    val appLock = LocalAppLock.current
    val galleryLocked by appLock.galleryLocked.collectAsStateWithLifecycle()
    val scheduleHistory by viewModel.scheduleHistory.collectAsStateWithLifecycle()
    val vacations by viewModel.vacations.collectAsStateWithLifecycle()
    val bodyweight by viewModel.bodyweight.collectAsStateWithLifecycle()
    val bodyweightGoalLb by viewModel.bodyweightGoalLb.collectAsStateWithLifecycle()
    val weightConnected by viewModel.weightConnected.collectAsStateWithLifecycle()
    val bodyweightMessage by viewModel.bodyweightMessage.collectAsStateWithLifecycle()
    val bodyFat by viewModel.bodyFat.collectAsStateWithLifecycle()
    val bodyFatConnected by viewModel.bodyFatConnected.collectAsStateWithLifecycle()
    val bodyFatMessage by viewModel.bodyFatMessage.collectAsStateWithLifecycle()
    val measurements by measurementsVm.state.collectAsStateWithLifecycle()
    val leanMass by leanMassVm.state.collectAsStateWithLifecycle()
    var showWeightSheet by remember { mutableStateOf(false) }
    var showBodyFatSheet by remember { mutableStateOf(false) }
    var showSizeSheet by remember { mutableStateOf(false) }
    var showAvatarSheet by remember { mutableStateOf(false) }
    // One File for the screen's life: avatarFile() builds a new one per call.
    val avatarFile = remember(viewModel) { viewModel.avatarFile() }

    // Persist the one-time cover hint as soon as it surfaces — it stays visible this session, gone next.
    LaunchedEffect(state.showAvatarHint) { if (state.showAvatarHint) viewModel.markAvatarHintSeen() }

    val photoImport = rememberLauncherForActivityResult(ActivityResultContracts.PickMultipleVisualMedia()) { uris ->
        if (uris.isNotEmpty()) viewModel.importPhotos(uris)
    }
    val avatarPicker = rememberLauncherForActivityResult(ActivityResultContracts.PickVisualMedia()) { uri ->
        uri?.let { viewModel.setAvatar(it) }
    }
    val unlock = rememberGalleryUnlock(onUnlocked = { appLock.markAuthenticated() })

    val planOn = remember(scheduleHistory) { { d: LocalDate -> ScheduleHistory.planOn(scheduleHistory, d) } }
    val onVacation = remember(vacations) { VacationCalendar.onVacation(vacations) }
    val today = remember(state) { LocalDate.now() }
    val nowMs = remember(state, bodyweight) { System.currentTimeMillis() }

    ProfilePage(
        state = state,
        body = ProfileBody(
            weight = bodyweight,
            goalLb = bodyweightGoalLb,
            bodyFat = bodyFat,
            leanMass = leanMass.entries,
            leanMassConnected = leanMass.connected,
            measurements = measurements,
        ),
        weightUnit = LocalForgeSettings.current.weightUnit,
        galleryLocked = galleryLocked,
        today = today,
        nowMs = nowMs,
        planOn = planOn,
        onVacation = onVacation,
        actions = remember(viewModel, onBack, onOpenPhotoGallery, onOpenCamera, onOpenMeasurements, unlock.launch) {
            ProfileActions(
                onBack = onBack,
                onPickCover = { viewModel.dismissAvatarHint(); showAvatarSheet = true },
                onSetName = viewModel::setUserName,
                onDayTap = viewModel::openDay,
                onLogWeight = {
                    // Fresh sheet: drop any prior result line and re-check HC permission so a grant
                    // made in Settings since this screen opened surfaces the import option.
                    viewModel.clearBodyweightMessage()
                    viewModel.refreshWeightConnected()
                    showWeightSheet = true
                },
                onLogBodyFat = {
                    viewModel.clearBodyFatMessage()
                    viewModel.refreshBodyFatConnected()
                    showBodyFatSheet = true
                },
                onSyncMuscle = { leanMassVm.syncNow() },
                onLogSize = { showSizeSheet = true },
                onOpenSizes = onOpenMeasurements,
                onOpenGallery = onOpenPhotoGallery,
                onOpenCamera = onOpenCamera,
                onImportPhotos = { photoImport.launch(PickVisualMediaRequest(ActivityResultContracts.PickVisualMedia.ImageOnly)) },
                onUnlock = unlock.launch,
            )
        },
        cover = if (state.hasAvatar) { m ->
            // Re-key on the file's stamp so a freshly picked cover reloads (the decoder caches by path).
            key(state.avatarStamp) { ProgressPhotoImage(avatarFile, m, reqPx = 2560) }
        } else null,
        photo = { p, m -> ProgressPhotoImage(viewModel.fileFor(p), m) },
        unlockNote = unlock.note,
        showCoverHint = state.showAvatarHint,
    )

    // "What did I do that day?" — opened by tapping a trained day of the Consistency month. The same
    // sheet as everywhere a day is asked about, drilling into the screens History uses.
    dayLog?.let { log ->
        DayLogSheet(
            log = log,
            onOpenSession = { viewModel.closeDay(); onOpenSession(it) },
            onOpenCardio = { viewModel.closeDay(); onOpenCardio(it) },
            onDismiss = viewModel::closeDay
        )
    }

    if (showWeightSheet) {
        BodyweightLogSheet(
            entries = bodyweight,
            canImport = weightConnected,
            message = bodyweightMessage,
            lookupDay = viewModel::bodyweightOn,
            onSave = { lb, date, note ->
                viewModel.logBodyweight(lb, date, note)
                showWeightSheet = false
            },
            onImport = { viewModel.importBodyweight() },  // stays open so the result line shows
            onDismiss = {
                showWeightSheet = false
                viewModel.clearBodyweightMessage()
            }
        )
    }

    if (showBodyFatSheet) {
        BodyFatLogSheet(
            entries = bodyFat,
            canImport = bodyFatConnected,
            message = bodyFatMessage,
            onSave = { pct, date ->
                viewModel.logBodyFat(pct, date)
                showBodyFatSheet = false
            },
            onImport = { viewModel.importBodyFat() },  // stays open so the result line shows
            onDismiss = {
                showBodyFatSheet = false
                viewModel.clearBodyFatMessage()
            }
        )
    }

    if (showSizeSheet) {
        BodyMeasurementLogSheet(
            series = measurements.series,
            useCm = measurements.useCm,
            onSave = { values ->
                measurementsVm.log(values)
                showSizeSheet = false
            },
            onDismiss = { showSizeSheet = false }
        )
    }

    if (showAvatarSheet) {
        AvatarPickerSheet(
            selectedKey = state.avatarDefaultKey,
            // "Select your own" hands off to the system Photo Picker.
            onPickOwn = {
                showAvatarSheet = false
                avatarPicker.launch(PickVisualMediaRequest(ActivityResultContracts.PickVisualMedia.ImageOnly))
            },
            onSelectDefault = { showAvatarSheet = false; viewModel.setAvatarFromDefault(it) },
            onDismiss = { showAvatarSheet = false }
        )
    }
}
