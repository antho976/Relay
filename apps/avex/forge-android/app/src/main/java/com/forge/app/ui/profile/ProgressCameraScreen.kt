package com.forge.app.ui.profile

import android.Manifest
import android.content.pm.PackageManager
import androidx.activity.compose.rememberLauncherForActivityResult
import androidx.activity.result.contract.ActivityResultContracts
import androidx.camera.core.CameraSelector
import androidx.camera.core.ImageCapture
import androidx.camera.core.ImageCaptureException
import androidx.camera.view.CameraController
import androidx.camera.view.LifecycleCameraController
import androidx.camera.view.PreviewView
import androidx.compose.foundation.Canvas
import androidx.compose.foundation.background
import androidx.compose.foundation.border
import androidx.compose.foundation.horizontalScroll
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
import androidx.compose.foundation.layout.statusBarsPadding
import androidx.compose.foundation.layout.width
import androidx.compose.foundation.rememberScrollState
import androidx.compose.foundation.shape.CircleShape
import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.filled.Close
import androidx.compose.material3.Icon
import androidx.compose.material3.IconButton
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableIntStateOf
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.rememberCoroutineScope
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.clip
import androidx.compose.ui.geometry.Offset
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.layout.ContentScale
import androidx.compose.ui.platform.LocalContext
import androidx.compose.ui.text.style.TextAlign
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp
import androidx.compose.ui.viewinterop.AndroidView
import androidx.hilt.navigation.compose.hiltViewModel
import androidx.lifecycle.compose.LocalLifecycleOwner
import androidx.lifecycle.compose.collectAsStateWithLifecycle
import androidx.core.content.ContextCompat
import com.forge.app.domain.photo.PhotoPose
import com.forge.app.ui.common.ForgeOutlineCapsule
import com.forge.app.ui.common.SegmentPill
import com.forge.app.ui.common.bounceClick
import kotlinx.coroutines.delay
import kotlinx.coroutines.launch
import java.io.File

/**
 * The guided progress-photo camera. A live preview with a faint ghost of your last same-pose shot so
 * you line up identical framing; pose chips retag the ghost, a rule-of-thirds grid and a 3-second
 * self-timer help you frame a solo mirror shot. The capture is written straight to app-private
 * storage (never the camera roll) via [ProgressCameraViewModel]. If camera permission is refused, the
 * screen falls back to a note pointing at the gallery import.
 */
@Composable
fun ProgressCameraScreen(
    onBack: () -> Unit,
    viewModel: ProgressCameraViewModel = hiltViewModel()
) {
    val context = LocalContext.current
    val lifecycleOwner = LocalLifecycleOwner.current
    val scope = rememberCoroutineScope()

    val latestByPose by viewModel.latestByPose.collectAsStateWithLifecycle()
    val newest by viewModel.newest.collectAsStateWithLifecycle()
    // A shot the repository refused to save. Its cache file is the only copy, so it is kept and the
    // shutter retries it rather than taking a new picture over it.
    val pending by viewModel.pending.collectAsStateWithLifecycle()

    var pose by remember { mutableStateOf(PhotoPose.FRONT) }
    var lensFront by remember { mutableStateOf(true) }
    var showGrid by remember { mutableStateOf(true) }
    var showGhost by remember { mutableStateOf(true) }
    var timerOn by remember { mutableStateOf(false) }
    var countdown by remember { mutableIntStateOf(0) }
    var capturing by remember { mutableStateOf(false) }
    var error by remember { mutableStateOf<String?>(null) }

    var hasPermission by remember {
        mutableStateOf(ContextCompat.checkSelfPermission(context, Manifest.permission.CAMERA) == PackageManager.PERMISSION_GRANTED)
    }
    var denied by remember { mutableStateOf(false) }
    val permLauncher = rememberLauncherForActivityResult(ActivityResultContracts.RequestPermission()) { granted ->
        hasPermission = granted; denied = !granted
    }
    LaunchedEffect(Unit) { if (!hasPermission) permLauncher.launch(Manifest.permission.CAMERA) }

    // The high-level controller binds the provider + use cases internally — no ListenableFuture to name.
    val previewView = remember { PreviewView(context).apply { scaleType = PreviewView.ScaleType.FILL_CENTER } }
    // Preview + image capture only (no image analysis) — two use cases bind on every device.
    val controller = remember { LifecycleCameraController(context).apply { setEnabledUseCases(CameraController.IMAGE_CAPTURE) } }
    val executor = remember(context) { ContextCompat.getMainExecutor(context) }
    LaunchedEffect(controller, previewView) { previewView.controller = controller }
    LaunchedEffect(hasPermission) { if (hasPermission) runCatching { controller.bindToLifecycle(lifecycleOwner) }.onFailure { error = "Couldn't open the camera." } }
    LaunchedEffect(lensFront) {
        controller.cameraSelector = if (lensFront) CameraSelector.DEFAULT_FRONT_CAMERA else CameraSelector.DEFAULT_BACK_CAMERA
    }

    // A failed repository save: the shot is kept (see `pending`), so the line names the way back.
    fun onSaveFailed() { capturing = false; error = "Couldn't save the photo. Tap the shutter to retry." }
    fun capture() {
        capturing = true
        error = null
        // Snapshot the pose WITH the shutter request. The CameraX callback below lands later, and a
        // pose tapped in between used to retag a shot that had already been framed as the old one.
        val shotPose = pose
        val temp = File(context.cacheDir, "cap_${System.nanoTime()}.jpg")
        val opts = ImageCapture.OutputFileOptions.Builder(temp).build()
        controller.takePicture(opts, executor, object : ImageCapture.OnImageSavedCallback {
            // Save first, navigate back only once the write lands (onBack clears the VM + cancels its
            // scope, so firing it before the save could drop the photo). Back fires ONLY on a real
            // save: a null repository result is a failure, and navigating on it presented a photo
            // that did not exist as saved.
            override fun onImageSaved(result: ImageCapture.OutputFileResults) {
                viewModel.addCaptured(temp, shotPose.name, onSaved = { onBack() }, onFailed = { onSaveFailed() })
            }
            override fun onError(exc: ImageCaptureException) { capturing = false; error = "Couldn't save the photo." }
        })
    }
    fun onShutter() {
        // Guard against a re-tap while a countdown runs or a capture is already in flight (a fresh shot
        // navigates away on success, so `capturing` only ever resets on the error path).
        if (countdown > 0 || capturing) return
        if (pending != null) {
            capturing = true
            error = null
            viewModel.retryPending(onSaved = { onBack() }, onFailed = { onSaveFailed() })
            return
        }
        if (timerOn) scope.launch {
            for (i in 3 downTo 1) { countdown = i; delay(1000) }
            countdown = 0; capture()
        } else capture()
    }

    val ghost = latestByPose[pose.name] ?: newest
    // Pose is fixed from the moment the shutter is asked for until the shot lands: a change during
    // the countdown or the in-flight capture would no longer describe the photo being taken.
    val poseLocked = countdown > 0 || capturing

    Box(Modifier.fillMaxSize().background(Color.Black)) {
        if (hasPermission) {
            AndroidView(factory = { previewView }, modifier = Modifier.fillMaxSize())
            if (showGhost && ghost != null) {
                // Crop-to-fill so the ghost frames the same way the FILL_CENTER preview does — a Fit
                // ghost would letterbox and no longer line up with the live subject it's an aid for.
                GalleryFullImage(
                    viewModel.fileFor(ghost), Modifier.fillMaxSize(),
                    reqPx = 900, alpha = 0.28f, contentScale = ContentScale.Crop
                )
            }
            if (showGrid) ThirdsGrid()
            if (countdown > 0) {
                Box(Modifier.fillMaxSize(), contentAlignment = Alignment.Center) {
                    Text("$countdown", style = MaterialTheme.typography.displayLarge, color = Color.White)
                }
            }
        } else {
            PermissionFallback(denied = denied, onAllow = { permLauncher.launch(Manifest.permission.CAMERA) }, onBack = onBack)
        }

        // ── Top chrome: close · pose lens · flip ──────────────────────────────
        Column(Modifier.fillMaxWidth().statusBarsPadding()) {
            Row(
                Modifier.fillMaxWidth().padding(horizontal = 6.dp, vertical = 4.dp),
                verticalAlignment = Alignment.CenterVertically, horizontalArrangement = Arrangement.SpaceBetween
            ) {
                IconButton(onClick = onBack) { Icon(Icons.Filled.Close, contentDescription = "Close camera", tint = Color.White) }
                if (hasPermission) Text(
                    if (lensFront) "Front cam" else "Rear cam",
                    style = MaterialTheme.typography.labelMedium, color = Color.White.copy(alpha = 0.85f),
                    modifier = Modifier.bounceClick { lensFront = !lensFront }.padding(horizontal = 12.dp, vertical = 8.dp)
                )
            }
            if (hasPermission) Row(
                Modifier.fillMaxWidth().horizontalScroll(rememberScrollState()).padding(horizontal = 12.dp),
                horizontalArrangement = Arrangement.spacedBy(8.dp)
            ) {
                PhotoPose.entries.forEach { p ->
                    SegmentPill(p.label, selected = pose == p, onClick = { if (!poseLocked) pose = p }, Color.White, Color.White, Color.White.copy(alpha = 0.6f), Color.White, 11.sp)
                }
            }
        }

        // ── Bottom chrome: ghost/grid/timer toggles + shutter ─────────────────
        if (hasPermission) Column(
            Modifier.align(Alignment.BottomCenter).fillMaxWidth().padding(bottom = 40.dp)
        ) {
            error?.let {
                Text(it, style = MaterialTheme.typography.bodySmall, color = MaterialTheme.colorScheme.error,
                    modifier = Modifier.fillMaxWidth().padding(bottom = 10.dp), textAlign = TextAlign.Center)
            }
            Row(
                Modifier.fillMaxWidth().padding(horizontal = 24.dp),
                verticalAlignment = Alignment.CenterVertically, horizontalArrangement = Arrangement.SpaceBetween
            ) {
                ToggleWord("Ghost", showGhost && ghost != null, enabled = ghost != null) { showGhost = !showGhost }
                ToggleWord("Grid", showGrid) { showGrid = !showGrid }
                ToggleWord("Timer", timerOn) { timerOn = !timerOn }
            }
            Spacer(Modifier.height(18.dp))
            Box(Modifier.fillMaxWidth(), contentAlignment = Alignment.Center) {
                // Shutter — a big ringed disc (§ live archetype: big target, function first).
                Box(
                    Modifier.size(74.dp).clip(CircleShape)
                        .border(3.dp, Color.White, CircleShape)
                        .padding(6.dp).clip(CircleShape).background(Color.White)
                        .bounceClick { onShutter() }
                )
            }
        }
    }
}

/** A white on/off word-toggle for the camera's bottom controls. */
@Composable
private fun ToggleWord(label: String, on: Boolean, enabled: Boolean = true, onClick: () -> Unit) {
    Text(
        label,
        style = MaterialTheme.typography.labelMedium,
        color = when {
            !enabled -> Color.White.copy(alpha = 0.3f)
            on -> Color.White
            else -> Color.White.copy(alpha = 0.5f)
        },
        modifier = Modifier.let { if (enabled) it.bounceClick { onClick() } else it }.padding(horizontal = 10.dp, vertical = 8.dp)
    )
}

/** A faint rule-of-thirds overlay to help line up a solo shot. */
@Composable
private fun ThirdsGrid() {
    Canvas(Modifier.fillMaxSize()) {
        val c = Color.White.copy(alpha = 0.18f)
        val w = size.width; val h = size.height
        drawLine(c, Offset(w / 3f, 0f), Offset(w / 3f, h), strokeWidth = 1f)
        drawLine(c, Offset(2 * w / 3f, 0f), Offset(2 * w / 3f, h), strokeWidth = 1f)
        drawLine(c, Offset(0f, h / 3f), Offset(w, h / 3f), strokeWidth = 1f)
        drawLine(c, Offset(0f, 2 * h / 3f), Offset(w, 2 * h / 3f), strokeWidth = 1f)
    }
}

@Composable
private fun PermissionFallback(denied: Boolean, onAllow: () -> Unit, onBack: () -> Unit) {
    Column(
        Modifier.fillMaxSize().padding(32.dp),
        verticalArrangement = Arrangement.Center, horizontalAlignment = Alignment.CenterHorizontally
    ) {
        Text(
            if (denied) "Camera access is off. Turn it on to take a shot, or import from your gallery instead."
            else "Waiting for camera permission…",
            style = MaterialTheme.typography.bodyMedium, color = Color.White.copy(alpha = 0.85f), textAlign = TextAlign.Center
        )
        if (denied) {
            Spacer(Modifier.height(20.dp))
            ForgeOutlineCapsule("Allow camera", onClick = onAllow, contentColor = Color.White)
            Spacer(Modifier.height(10.dp))
            Text("Back", style = MaterialTheme.typography.labelMedium, color = Color.White.copy(alpha = 0.6f),
                modifier = Modifier.bounceClick { onBack() }.padding(12.dp))
        }
    }
}
