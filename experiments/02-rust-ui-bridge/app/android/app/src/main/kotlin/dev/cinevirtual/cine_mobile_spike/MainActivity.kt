package dev.cinevirtual.cine_mobile_spike

import android.app.Activity
import android.content.Intent
import android.net.Uri
import android.os.Looper
import android.os.ParcelFileDescriptor
import android.os.SystemClock
import android.provider.OpenableColumns
import android.view.SurfaceView
import android.view.View
import androidx.media3.common.C
import androidx.media3.common.MediaItem
import androidx.media3.common.PlaybackException
import androidx.media3.common.PlaybackParameters
import androidx.media3.common.Player
import androidx.media3.exoplayer.SeekParameters
import androidx.media3.exoplayer.ExoPlayer
import androidx.media3.exoplayer.analytics.AnalyticsListener
import io.flutter.embedding.android.FlutterActivity
import io.flutter.embedding.engine.FlutterEngine
import io.flutter.plugin.common.MethodChannel
import io.flutter.plugin.common.StandardMessageCodec
import io.flutter.plugin.platform.PlatformView
import io.flutter.plugin.platform.PlatformViewFactory

/** Owns the SDK and SAF permission on main Looper. Rust receives observations only. */
class MainActivity : FlutterActivity() {
    companion object { init { System.loadLibrary("cine_ui_bridge") } }
    private external fun nativeDestroy(handle: Long)
    private external fun nativeClock(handle: Long): Long
    private external fun nativeDrive(handle: Long, sample: String): String
    private var rustAnchor = 0L
    private var sdkAnchor = 0L
    private var controlOffset = 0.0
    private var controlSequence = 0L
    private var seekDispatch = 0L
    private var seekTarget = 0L
    private var controlDeadline = 0L
    private var rustOwner = 0L
    private var driverRunning = false
    private var networkDriver = false
    private val ownedPersistedGrants = mutableSetOf<Uri>()
    private val driver = android.os.Handler(Looper.getMainLooper())
    private var drivePosted = 0L
    private var driveDue = 0L
    private var driveCount = 0L
    private var driveCostNs = 0L
    private var jniCostNs = 0L
    private var sourceCostNs = 0L
    private var diagnostic = false
    private var diagnosticBias = 0L
    private var biasUntil = 0L
    override fun onNewIntent(next: Intent) {
        super.onNewIntent(next)
        if (diagnostic && next.hasExtra("cine_fault_ms")) {
            diagnosticBias = next.getLongExtra("cine_fault_ms", 0)
            biasUntil = SystemClock.elapsedRealtime() + 10000
            trace("fault_begin", mapOf("bias_ms" to diagnosticBias, "duration_ms" to 10000))
        }
    }
    private var seekOperation: org.json.JSONObject? = null
    private val operations = mutableMapOf<String, org.json.JSONObject>()
    private var settledSample: Pair<Long, Long>? = null
    private var stableReported = true
    private var stableDelivered = true
    private var seekLoss: org.json.JSONObject? = null
    private var seekAdvanceReported = true
    private var seekLifecycle = "IDLE"
    private fun lifecycle(next: String) {
        seekLifecycle = next
        trace("seek_lifecycle", mapOf("state" to next), seekOperation)
    }
    private fun recordSeekLoss(loss: Long, source: String) {
        val operation = seekOperation ?: return
        seekLoss = org.json.JSONObject(mapOf("generation" to operation.optLong("generation"),
            "operation_id" to operation.optLong("operation_id"), "loss_ms" to loss.coerceAtLeast(0)))
        trace("seek_loss_measured", mapOf("loss_ms" to loss.coerceAtLeast(0), "source" to source), operation)
    }
    private fun trace(event: String, values: Map<String, Any?> = emptyMap(), operation: org.json.JSONObject? = null) {
        if (!diagnostic) return
        val fields = mutableMapOf<String, Any?>("event" to event, "sdk_ms" to SystemClock.elapsedRealtime())
        if (operation != null) for (key in listOf("operation_id", "generation", "sequence", "media_revision", "action", "reason")) fields[key] = operation.opt(key)
        fields.putAll(values)
        android.util.Log.i("CineRoom", org.json.JSONObject(fields).toString())
    }
    private fun postDrive(delay: Long = 0) {
        drivePosted = SystemClock.elapsedRealtime(); driveDue = drivePosted + delay
        driver.postDelayed(drive, delay)
    }
    private fun observeStable(sample: Map<String, Any>) {
        if (seeking || sample["loaded"] != true || sample["buffering"] == true) return
        val t = (sample["sample_monotonic_ms"] as Number).toLong()
        val pos = (sample["position_ms"] as Number).toLong()
        if (!seekAdvanceReported && sample["playing"] == true && pos > seekTarget) {
            recordSeekLoss(t - seekDispatch - ((pos - seekTarget) / (sample["rate"] as Double)).toLong(), "first_advance")
            seekAdvanceReported = true
        }
        // A paused stable position does not finish measuring a seek that will resume playing.
        if (stableReported) return
        val previous = settledSample
        if (previous != null && t > previous.first) {
            val expected = if (sample["playing"] == true) (t - previous.first) * (sample["rate"] as Double) else 0.0
            if (kotlin.math.abs((pos - previous.second) - expected) <= 35 && (sample["playing"] != true || pos > previous.second)) {
                trace("first_stable_position", mapOf("position_ms" to pos, "previous_position_ms" to previous.second,
                    "sample_delta_ms" to t - previous.first, "playing" to sample["playing"]), seekOperation)
                stableReported = true
                lifecycle("IDLE")
            }
        }
        settledSample = Pair(t, pos)
    }
    private val drive = object : Runnable {
        override fun run() {
            if (!alive || !driverRunning || rustOwner == 0L) return
            val startNs = SystemClock.elapsedRealtimeNanos()
            val execution = SystemClock.elapsedRealtime()
            try {
                // Bracket a Rust clock read. Origins are never assumed equal.
                val clockBefore = SystemClock.elapsedRealtime()
                val rustClock = nativeClock(rustOwner)
                val clockAfter = SystemClock.elapsedRealtime()
                val midpoint = (clockBefore + clockAfter) / 2
                val sourceStart = SystemClock.elapsedRealtimeNanos()
                val sdkState = state()
                sourceCostNs += SystemClock.elapsedRealtimeNanos() - sourceStart
                observeStable(sdkState)
                val observedAt = (sdkState["sample_monotonic_ms"] as Number).toLong()
                val before = SystemClock.elapsedRealtime()
                val mapped = rustClock + observedAt - midpoint
                if (diagnosticBias != 0L && before >= biasUntil) { diagnosticBias = 0; trace("fault_end") }
                val biasedPosition = ((sdkState["position_ms"] as Number).toLong() + diagnosticBias).coerceIn(0, (sdkState["duration_ms"] as Number).toLong())
                val observed = org.json.JSONObject(mapOf("sample" to mapOf("position_ms" to biasedPosition, "duration_ms" to sdkState["duration_ms"],
                    "playing" to sdkState["playing"], "loaded" to sdkState["loaded"], "buffering" to sdkState["buffering"], "seeking" to sdkState["seeking"], "age_ms" to 0),
                    "seek_loss" to seekLoss, "source_mapped_at_ms" to mapped, "supports_rate" to sdkState["supports_rate"], "rate" to sdkState["rate"], "failed" to (failure != null)))
                val jniStart = SystemClock.elapsedRealtimeNanos()
                val result = org.json.JSONObject(nativeDrive(rustOwner, observed.toString()))
                seekLoss = null
                val jniEnd = SystemClock.elapsedRealtime()
                val rustNow = result.optLong("now_ms")
                jniCostNs += SystemClock.elapsedRealtimeNanos() - jniStart
                if (stableReported && !stableDelivered) {
                    trace("stable_observation_delivered", mapOf("source_sdk_ms" to observedAt,
                        "source_mapped_at_ms" to mapped,"rust_delivered_ms" to rustNow,"jni_return_sdk_ms" to jniEnd), seekOperation)
                    stableDelivered = true
                }
                val list = result.getJSONArray("effects")
                rustAnchor = rustNow; sdkAnchor = before
                val records = result.optJSONArray("diagnostics")
                if (diagnostic && records != null) for (i in 0 until records.length()) android.util.Log.i("CineRoom", records.getJSONObject(i).toString())
                for (i in 0 until list.length()) {
                    val e = list.getJSONObject(i)
                    operations[e.getString("action")] = e
                    if (e.getString("action") == "seek") { seekOperation = e; stableReported = false; stableDelivered = false; settledSample = null; seekAdvanceReported = !e.optBoolean("advance_target"); lifecycle("SEEK_REQUESTED") }
                    controlOffset = e.optDouble("offset_ms"); controlSequence = e.optLong("sequence"); controlDeadline = e.optLong("deadline_server_ms")
                    trace("effect_received", mapOf("rust_received_ms" to rustNow, "enqueued_ms" to e.optLong("enqueued_ms"),
                        "received_at_ms" to e.optLong("received_at_ms"), "deadline_local_ms" to e.optDouble("deadline_local_ms"),
                        "wake_at_ms" to e.optLong("wake_at_ms"), "drive_post_ms" to drivePosted, "drive_due_ms" to driveDue,
                        "drive_execution_ms" to execution, "poll_lateness_ms" to execution - driveDue,
                        "jni_return_sdk_ms" to jniEnd, "sample_source_sdk_ms" to observedAt,
                        "source_mapped_at_ms" to mapped, "sample_assigned_at_ms" to result.optLong("sample_assigned_at_ms"),
                        "clock_bracket_ms" to clockAfter - clockBefore), e)
                    val dispatch = SystemClock.elapsedRealtime()
                    val value = if (e.getString("action") == "seek" && e.optBoolean("advance_target")) {
                        (e.getDouble("value") + (dispatch - jniEnd)).coerceAtMost((player?.duration ?: 0).toDouble())
                    } else e.getDouble("value")
                    trace("media3_call", mapOf("position_ms" to player?.currentPosition, "playback_state" to player?.playbackState,
                        "seeking" to seeking, "target_ms" to e.optLong("target_ms"), "value" to value, "seek_lead_ms" to e.optLong("seek_lead_ms")), e)
                    applyEffect(e.getString("action"), value, e.getLong("generation"))
                    trace("media3_return", mapOf("command_latency_ms" to SystemClock.elapsedRealtime() - dispatch), e)
                    val deadline = e.optLong("deadline_server_ms")
                    val dispatched = rustNow + dispatch - before
                    android.util.Log.i("CineRoom", org.json.JSONObject(mapOf("event" to "native_dispatch", "action" to e.getString("action"), "sequence" to e.optLong("sequence"),
                        "reason" to e.optString("reason"), "deadline_server_ms" to deadline, "dispatch_server_ms" to dispatched + e.optDouble("offset_ms"),
                        "lateness_ms" to if (deadline > 0) dispatched + e.optDouble("offset_ms") - deadline else 0, "target_ms" to e.optLong("target_ms"),
                        "operation_id" to e.optLong("operation_id"), "generation" to e.optLong("generation"))).toString())
                }
                if (diagnostic && driveCount % 25 == 0L) trace("driver_observation", mapOf("source_sdk_ms" to observedAt,
                    "rust_return_ms" to rustNow,"source_mapped_at_ms" to mapped,"assigned_at_ms" to result.optLong("sample_assigned_at_ms"),
                    "jni_roundtrip_ms" to jniEnd - before,"jni_to_lock_ms" to result.optDouble("jni_to_lock_ms"),
                    "clock_bracket_ms" to clockAfter - clockBefore, "poll_lateness_ms" to execution - driveDue,
                    "position_ms" to sdkState["position_ms"], "playing" to sdkState["playing"], "seeking" to seeking,
                    "buffering" to sdkState["buffering"], "diagnostics_dropped" to result.optLong("diagnostics_dropped")))
            } catch (_: Exception) { trace("driver_error") }
            driveCount++; driveCostNs += SystemClock.elapsedRealtimeNanos() - startNs
            postDrive(20)
        }
    }
    private fun applyEffect(action: String, value: Double, gen: Long) {
        val current = player ?: throw IllegalStateException("PLAYER_DESTROYED")
        require(gen >= generation) { "STALE_GENERATION" }; generation = gen
        when(action) {
            "play" -> current.play()
            "pause" -> current.pause()
            "rate" -> { require(value in 0.5..2.0) { "INVALID_RATE" }; current.playbackParameters = PlaybackParameters(value.toFloat(),1f) }
            "seek" -> { require(value >= 0 && value <= current.duration) { "SEEK_OUT_OF_RANGE" }; seeking = true; seekDispatch = SystemClock.elapsedRealtime(); seekTarget = value.toLong(); lifecycle("SEEK_DISPATCHED"); current.seekTo(value.toLong()) }
            else -> throw IllegalArgumentException("INVALID_EFFECT")
        }
    }
    @Volatile private var alive = true
    private var oracleWorker: Thread? = null
    private var player: ExoPlayer? = null
    private var selected: Uri? = null
    private var picker: MethodChannel.Result? = null
    private var seeking = false
    private var firstFrame = false
    private var decoder = "unknown"
    private var failure: String? = null
    private var loadStart = 0L
    private var loadMs = 0L
    private var hashStartPss = 0L
    private var generation = 1L
    private var fd: ParcelFileDescriptor? = null

    override fun configureFlutterEngine(engine: FlutterEngine) {
        super.configureFlutterEngine(engine)
        val sdk = ExoPlayer.Builder(this).setLooper(Looper.getMainLooper()).build()
        sdk.setSeekParameters(SeekParameters.EXACT)
        player = sdk
        sdk.addListener(object : Player.Listener {
            override fun onPlayerError(error: PlaybackException) { if (alive) failure = "PLAYER_LOAD_ERROR" }
            override fun onIsPlayingChanged(isPlaying: Boolean) {
                if (!alive) return
                val operation = if (isPlaying || !sdk.playWhenReady) operations.remove(if (isPlaying) "play" else "pause") else null
                trace(if (operation != null) "playing_completed" else "playing_changed",
                    mapOf("playing" to isPlaying,"position_ms" to sdk.currentPosition,"play_when_ready" to sdk.playWhenReady,
                        "playback_state" to sdk.playbackState), operation ?: seekOperation)
                android.util.Log.i("CineRoom", org.json.JSONObject(mapOf("event" to "native_playing_changed", "playing" to isPlaying, "sequence" to controlSequence,
                    "position_ms" to sdk.currentPosition, "server_ms" to rustAnchor + SystemClock.elapsedRealtime() - sdkAnchor + controlOffset,
                    "deadline_server_ms" to controlDeadline)).toString())
            }
            override fun onPositionDiscontinuity(old: Player.PositionInfo, new: Player.PositionInfo, reason: Int) {
                if (reason == Player.DISCONTINUITY_REASON_SEEK && seeking) lifecycle("SEEK_IN_PROGRESS")
                trace("position_discontinuity", mapOf("reason_code" to reason,"old_position_ms" to old.positionMs,"position_ms" to new.positionMs), seekOperation)
            }
            override fun onPlaybackStateChanged(state: Int) {
                trace("playback_state_changed", mapOf("playback_state" to state,"position_ms" to sdk.currentPosition), seekOperation)
            }
            override fun onPlaybackParametersChanged(parameters: PlaybackParameters) {
                trace("rate_completed", mapOf("rate" to parameters.speed), operations["rate"])
            }
            override fun onTimelineChanged(timeline: androidx.media3.common.Timeline, reason: Int) {
                trace("timeline_changed", mapOf("reason_code" to reason,"window_count" to timeline.windowCount))
            }
            override fun onEvents(p: Player, events: Player.Events) {
                if (!alive) return
                if (p.playbackState == Player.STATE_READY) {
                    if (loadMs == 0L) loadMs = SystemClock.elapsedRealtime() - loadStart
                    if (seeking) {
                        trace("seek_ready_candidate", mapOf("playback_state" to p.playbackState,"position_ms" to p.currentPosition, "latency_ms" to SystemClock.elapsedRealtime() - seekDispatch,
                            "events" to (0 until events.size()).map { events.get(it) }), seekOperation)
                        seeking = false
                        lifecycle("SEEK_SETTLED")
                        if (seekOperation?.optBoolean("advance_target") != true) recordSeekLoss(SystemClock.elapsedRealtime() - seekDispatch, "paused_ready")
                        android.util.Log.i("CineRoom", org.json.JSONObject(mapOf("event" to "native_seek_ready", "sequence" to controlSequence,
                            "latency_ms" to SystemClock.elapsedRealtime() - seekDispatch, "target_ms" to seekTarget, "position_ms" to p.currentPosition,
                            "server_ms" to rustAnchor + SystemClock.elapsedRealtime() - sdkAnchor + controlOffset)).toString())
                    }
                }
            }
        })
        sdk.addAnalyticsListener(object : AnalyticsListener {
            override fun onVideoDecoderInitialized(eventTime: AnalyticsListener.EventTime,
                decoderName: String, initializedTimestampMs: Long, initializationDurationMs: Long) { if (alive) decoder = decoderName }
            override fun onRenderedFirstFrame(eventTime: AnalyticsListener.EventTime, output: Any, renderTimeMs: Long) { if (alive) firstFrame = true }
        })
        engine.platformViewsController.registry.registerViewFactory("cine.mobile/surface", object : PlatformViewFactory(StandardMessageCodec.INSTANCE) {
            override fun create(context: android.content.Context, id: Int, args: Any?): PlatformView {
                check(alive && player != null) { "PLAYER_DESTROYED" }
                val surface = SurfaceView(context)
                surface.keepScreenOn = true
                sdk.setVideoSurfaceView(surface)
                return object : PlatformView {
                    override fun getView(): View = surface
                    override fun dispose() { player?.clearVideoSurfaceView(surface) }
                }
            }
        })
        MethodChannel(engine.dartExecutor.binaryMessenger, "cine.mobile/player").setMethodCallHandler { call, reply ->
            if (!alive) { reply.error("PLAYER_DESTROYED", "PLAYER_DESTROYED", null); return@setMethodCallHandler }
            check(Looper.myLooper() == Looper.getMainLooper())
            try {
                when (call.method) {
                    "bindOwner" -> {
                        val next = (call.arguments as Number).toLong()
                        require(rustOwner == 0L || rustOwner == next) { "OWNER_ALREADY_BOUND" }
                        rustOwner = next; reply.success(null)
                    }
                    "startNetworkDriver" -> { diagnostic = intent.getBooleanExtra("cine_diagnostic", false); networkDriver = true; driverRunning = true; driver.removeCallbacks(drive); postDrive(); reply.success(null) }
                    "roomConfig" -> reply.success(mapOf("server" to (intent.getStringExtra("cine_server") ?: ""), "invite" to (intent.getStringExtra("cine_invite") ?: ""), "run_id" to (intent.getStringExtra("cine_run_id") ?: "manual")))
                    "select" -> {
                        if (picker != null) reply.error("PICKER_BUSY", "PICKER_BUSY", null)
                        else {
                            picker = reply
                            startActivityForResult(Intent(Intent.ACTION_OPEN_DOCUMENT).apply {
                                type = "video/*"; addCategory(Intent.CATEGORY_OPENABLE)
                                if (android.os.Build.VERSION.SDK_INT >= 26) putExtra(android.provider.DocumentsContract.EXTRA_INITIAL_URI,
                                    android.provider.DocumentsContract.buildDocumentUri("com.android.externalstorage.documents", "primary:Download"))
                                addFlags(Intent.FLAG_GRANT_READ_URI_PERMISSION or Intent.FLAG_GRANT_PERSISTABLE_URI_PERMISSION)
                            }, 41)
                        }
                    }
                    "openHashFd" -> {
                        hashStartPss = android.os.Debug.getPss()
                        val uri = selected ?: throw IllegalStateException("INVALID_URI")
                        fd?.close()
                        fd = contentResolver.openFileDescriptor(uri, "r") ?: throw IllegalStateException("READ_FAILED")
                        reply.success(fd!!.fd)
                    }
                    "closeHashFd" -> { fd?.close(); fd = null; reply.success(null) }
                    "verifyHash" -> {
                        val uri = selected ?: throw IllegalStateException("INVALID_URI")
                        val expected = call.arguments as String
                        oracleWorker = Thread {
                            val matches = try {
                                val hash = java.security.MessageDigest.getInstance("SHA-256")
                                contentResolver.openInputStream(uri)!!.use { input ->
                                    val block = ByteArray(1048576)
                                    while (!Thread.currentThread().isInterrupted) { val n = input.read(block); if (n < 0) break; hash.update(block,0,n) }
                                }
                                hash.digest().joinToString("") { "%02x".format(it) } == expected
                            } catch (_: Exception) { false }
                            android.os.Handler(Looper.getMainLooper()).post { if (alive) reply.success(matches) }
                        }.also { it.start() }
                    }
                    "resources" -> reply.success(resources())
                    "load" -> {
                        val uri = selected ?: throw IllegalStateException("INVALID_URI")
                        require(uri.scheme == "content") { "INVALID_URI" }
                        failure = null; firstFrame = false; seekLoss = null; seekOperation = null; seekAdvanceReported = true; stableReported = true; lifecycle("IDLE"); loadMs = 0; loadStart = SystemClock.elapsedRealtime()
                        sdk.trackSelectionParameters = sdk.trackSelectionParameters.buildUpon()
                            .setTrackTypeDisabled(C.TRACK_TYPE_AUDIO, call.argument<Boolean>("disable_audio") == true).build()
                        sdk.pause(); sdk.setMediaItem(MediaItem.fromUri(uri)); sdk.prepare(); reply.success(null)
                    }
                    "state" -> reply.success(state())
                    "effect" -> {
                        applyEffect(call.argument<String>("action")!!, call.argument<Number>("value")!!.toDouble(), call.argument<Number>("generation")!!.toLong())
                        reply.success(null)
                    }
                    "requestLifecycleTest" -> {
                        reply.success(null)
                        android.util.Log.i("CineSpike", "CINE_SPIKE_LIFECYCLE_BACKGROUND")
                        moveTaskToBack(true)
                    }
                    "errorTests" -> {
                        var uriRejected = false
                        try { contentResolver.openFileDescriptor(Uri.parse("content://invalid.cine.spike/missing"), "r")?.close() }
                        catch (_: Exception) { uriRejected = true }
                        val old = player; player = null
                        var destroyedRejected = false
                        try { state() } catch (_: IllegalStateException) { destroyedRejected = true }
                        player = old
                        reply.success(mapOf("invalid_uri_rejected" to uriRejected, "destroyed_access_rejected" to destroyedRejected))
                    }
                    else -> reply.notImplemented()
                }
            } catch (_: SecurityException) { reply.error("PERMISSION_DENIED", "PERMISSION_DENIED", null) }
              catch (e: IllegalArgumentException) { reply.error(e.message ?: "INVALID_ARGUMENT", "INVALID_ARGUMENT", null) }
              catch (e: IllegalStateException) { reply.error(e.message ?: "PLAYER_ERROR", "PLAYER_ERROR", null) }
              catch (_: Exception) { reply.error("LOCAL_MEDIA_ERROR", "LOCAL_MEDIA_ERROR", null) }
        }
    }
    private fun resources(): Map<String, Any> {
        val stat = java.io.File("/proc/self/stat").readText().substringAfterLast(") ").split(" ")
        return mapOf("sample_monotonic_ms" to SystemClock.elapsedRealtime(), "cpu_ticks" to stat[11].toLong() + stat[12].toLong(),
            "clock_ticks_per_second" to android.system.Os.sysconf(android.system.OsConstants._SC_CLK_TCK),
            "pss_before_hash_kib" to hashStartPss, "pss_after_hash_and_load_kib" to android.os.Debug.getPss().toLong(),
            "threads" to java.io.File("/proc/self/task").list()!!.size.toLong(), "fds" to java.io.File("/proc/self/fd").list()!!.size.toLong(),
            "thread_cpu_groups" to java.io.File("/proc/self/task").listFiles()!!.mapNotNull { task ->
                try {
                    val text = java.io.File(task, "stat").readText()
                    val fields = text.substringAfterLast(") ").split(" ")
                    val name = text.substringAfter("(").substringBeforeLast(")")
                    val group = when {
                        task.name == android.os.Process.myPid().toString() -> "main"
                        name.contains("raster") -> "flutter_raster"
                        name.contains(".ui") -> "flutter_ui"
                        name.contains("ExoPlayer") -> "media3"
                        name.contains("Codec") || name.contains("OMX") || name.contains("Audio") -> "codec_audio"
                        name.contains("cine-") -> "rust_owner"
                        else -> "other"
                    }
                    Pair(group, fields[11].toLong() + fields[12].toLong())
                } catch (_: Exception) { null }
            }.groupBy({ it.first }, { it.second }).mapValues { it.value.sum() }, "driver_count" to driveCount,"driver_cost_ns" to driveCostNs,"jni_cost_ns" to jniCostNs,"source_cost_ns" to sourceCostNs)
    }
    private fun state(): Map<String, Any> {
        val sdk = player ?: throw IllegalStateException("PLAYER_DESTROYED")
        val duration = if (sdk.duration == C.TIME_UNSET) 0 else sdk.duration.coerceAtLeast(0)
        return mapOf("sample_monotonic_ms" to SystemClock.elapsedRealtime(), "supports_rate" to sdk.isCommandAvailable(Player.COMMAND_SET_SPEED_AND_PITCH), "position_ms" to sdk.currentPosition.coerceIn(0, duration), "duration_ms" to duration,
            "playing" to sdk.isPlaying, "loaded" to (sdk.playbackState == Player.STATE_READY && failure == null && duration > 0),
            "buffering" to (sdk.playbackState == Player.STATE_BUFFERING), "seeking" to seeking,
            "rate" to sdk.playbackParameters.speed.toDouble(), "decoder" to decoder,
            "rendered_first_frame" to firstFrame, "load_ms" to loadMs)
    }
    override fun onActivityResult(requestCode: Int, resultCode: Int, data: Intent?) {
        super.onActivityResult(requestCode, resultCode, data)
        if (requestCode != 41 || !alive) return
        val result = picker; picker = null
        if (resultCode != Activity.RESULT_OK) { result?.success(null); return }
        val uri = data?.data
        if (uri == null || uri.scheme != "content") { result?.error("INVALID_URI", "INVALID_URI", null); return }
        selected = uri
        if ((data.flags and Intent.FLAG_GRANT_PERSISTABLE_URI_PERMISSION) != 0) {
            val existed = contentResolver.persistedUriPermissions.any { it.uri == uri && it.isReadPermission }
            try {
                contentResolver.takePersistableUriPermission(uri, Intent.FLAG_GRANT_READ_URI_PERMISSION)
                if (!existed) ownedPersistedGrants.add(uri)
            }
            catch (_: SecurityException) { /* Session read permission still works. */ }
        }
        var title = "Selected video"
        contentResolver.query(uri, arrayOf(OpenableColumns.DISPLAY_NAME), null, null, null)?.use { c ->
            if (c.moveToFirst()) title = c.getString(0)
        }
        result?.success(mapOf("title" to title, "source_type" to "content_uri"))
    }
    override fun onStart() {
        super.onStart()
        if (rustOwner != 0L && alive && networkDriver) { driverRunning = true; driver.removeCallbacks(drive); postDrive() }
    }
    override fun onStop() {
        driverRunning = false; driver.removeCallbacks(drive)
        player?.pause(); player?.playbackParameters = PlaybackParameters.DEFAULT
        super.onStop()
    }
    override fun onDestroy() {
        alive = false
        driverRunning = false; driver.removeCallbacks(drive)
        val owned = rustOwner; rustOwner = 0L
        if (owned != 0L) Thread {
            nativeDestroy(owned)
            android.util.Log.i("CineRoom", org.json.JSONObject(resources() + mapOf("event" to "resources_after_rust_destroy")).toString())
            android.util.Log.i("CineSpike", "CINE_SPIKE_RUST_RELEASED")
        }.start()
        val oracle = oracleWorker; oracle?.interrupt(); oracleWorker = null
        if (oracle != null) Thread { oracle.join(2000) }.start()
        ownedPersistedGrants.forEach { uri ->
            try { contentResolver.releasePersistableUriPermission(uri, Intent.FLAG_GRANT_READ_URI_PERMISSION) } catch (_: SecurityException) { }
        }
        ownedPersistedGrants.clear(); selected = null
        fd?.close(); fd = null
        picker?.error("CANCELLED", "CANCELLED", null); picker = null
        player?.release(); player = null
        android.util.Log.i("CineSpike", "CINE_SPIKE_PLAYER_RELEASED")
        super.onDestroy()
    }
}
