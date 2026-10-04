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
    private var hashEndPss = 0L
    private var generation = 1L
    private var fd: ParcelFileDescriptor? = null

    override fun configureFlutterEngine(engine: FlutterEngine) {
        super.configureFlutterEngine(engine)
        val sdk = ExoPlayer.Builder(this).setLooper(Looper.getMainLooper()).build()
        sdk.setSeekParameters(SeekParameters.EXACT)
        player = sdk
        sdk.addListener(object : Player.Listener {
            override fun onPlayerError(error: PlaybackException) { failure = "PLAYER_LOAD_ERROR" }
            override fun onEvents(p: Player, events: Player.Events) {
                if (p.playbackState == Player.STATE_READY) {
                    if (loadMs == 0L) loadMs = SystemClock.elapsedRealtime() - loadStart
                    if (seeking) seeking = false
                }
            }
        })
        sdk.addAnalyticsListener(object : AnalyticsListener {
            override fun onVideoDecoderInitialized(eventTime: AnalyticsListener.EventTime,
                decoderName: String, initializedTimestampMs: Long, initializationDurationMs: Long) { decoder = decoderName }
            override fun onRenderedFirstFrame(eventTime: AnalyticsListener.EventTime, output: Any, renderTimeMs: Long) { firstFrame = true }
        })
        engine.platformViewsController.registry.registerViewFactory("cine.mobile/surface", object : PlatformViewFactory(StandardMessageCodec.INSTANCE) {
            override fun create(context: android.content.Context, id: Int, args: Any?): PlatformView {
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
            check(Looper.myLooper() == Looper.getMainLooper())
            try {
                when (call.method) {
                    "select" -> {
                        if (picker != null) reply.error("PICKER_BUSY", "PICKER_BUSY", null)
                        else {
                            picker = reply
                            startActivityForResult(Intent(Intent.ACTION_OPEN_DOCUMENT).apply {
                                type = "video/*"; addCategory(Intent.CATEGORY_OPENABLE)
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
                    "resources" -> { hashEndPss = android.os.Debug.getPss(); reply.success(mapOf("pss_before_hash_kib" to hashStartPss, "pss_after_hash_and_load_kib" to hashEndPss, "threads" to java.io.File("/proc/self/task").list()!!.size)) }
                    "load" -> {
                        val uri = selected ?: throw IllegalStateException("INVALID_URI")
                        require(uri.scheme == "content") { "INVALID_URI" }
                        failure = null; firstFrame = false; loadMs = 0; loadStart = SystemClock.elapsedRealtime()
                        sdk.trackSelectionParameters = sdk.trackSelectionParameters.buildUpon()
                            .setTrackTypeDisabled(C.TRACK_TYPE_AUDIO, call.argument<Boolean>("disable_audio") == true).build()
                        sdk.pause(); sdk.setMediaItem(MediaItem.fromUri(uri)); sdk.prepare(); reply.success(null)
                    }
                    "state" -> reply.success(state())
                    "effect" -> {
                        val current = player ?: throw IllegalStateException("PLAYER_DESTROYED")
                        val gen = call.argument<Number>("generation")!!.toLong()
                        require(gen >= generation) { "STALE_GENERATION" }; generation = gen
                        val value = call.argument<Number>("value")!!.toDouble()
                        when (call.argument<String>("action")) {
                            "play" -> current.play()
                            "pause" -> current.pause()
                            "rate" -> { require(value in 0.5..2.0) { "INVALID_RATE" }; current.playbackParameters = PlaybackParameters(value.toFloat(), 1f) }
                            "seek" -> {
                                require(value >= 0 && value <= current.duration) { "SEEK_OUT_OF_RANGE" }
                                seeking = true; current.seekTo(value.toLong())
                            }
                            else -> throw IllegalArgumentException("INVALID_EFFECT")
                        }
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
        if (requestCode != 41) return
        val result = picker; picker = null
        if (resultCode != Activity.RESULT_OK) { result?.success(null); return }
        val uri = data?.data
        if (uri == null || uri.scheme != "content") { result?.error("INVALID_URI", "INVALID_URI", null); return }
        selected = uri
        if ((data.flags and Intent.FLAG_GRANT_PERSISTABLE_URI_PERMISSION) != 0) {
            try { contentResolver.takePersistableUriPermission(uri, Intent.FLAG_GRANT_READ_URI_PERMISSION) }
            catch (_: SecurityException) { /* Session read permission still works. */ }
        }
        var title = "Selected video"
        contentResolver.query(uri, arrayOf(OpenableColumns.DISPLAY_NAME), null, null, null)?.use { c ->
            if (c.moveToFirst()) title = c.getString(0)
        }
        result?.success(mapOf("title" to title, "source_type" to "content_uri"))
    }
    override fun onStop() {
        player?.pause(); player?.playbackParameters = PlaybackParameters.DEFAULT
        super.onStop()
    }
    override fun onDestroy() {
        alive = false
        oracleWorker?.interrupt(); oracleWorker = null
        fd?.close(); fd = null
        picker?.error("CANCELLED", "CANCELLED", null); picker = null
        player?.release(); player = null
        android.util.Log.i("CineSpike", "CINE_SPIKE_PLAYER_RELEASED")
        super.onDestroy()
    }
}
