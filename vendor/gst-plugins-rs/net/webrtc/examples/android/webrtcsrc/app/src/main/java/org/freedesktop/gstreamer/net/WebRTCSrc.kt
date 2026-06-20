package org.freedesktop.gstreamer.net

import android.content.Context
import android.os.Handler
import android.os.HandlerThread
import android.util.Log
import android.view.Surface
import org.freedesktop.gstreamer.GStreamer
import java.io.Closeable
import java.nio.ByteBuffer

// This Managed class allows running a native GStreamer pipeline containing a `webrtcsrc` element.
//
// This class is responsible for creating and managing the pipeline event loop.
//
// Operations on the pipeline are guaranteed to be synchronized as long as they are posted
// to the `Handler` via `handler.post()`.
class WebRTCSrc(private val errorListener: ErrorListener) :
    HandlerThread("GStreamer WebRTCSrc pipeline"), Closeable {

    private lateinit var handler: Handler

    init {
        start()
    }

    override fun onLooperPrepared() {
        handler = Handler(this.looper)
    }

    // This is a pointer to the native struct needed by C code
    // to keep track of its context between calls.
    //
    // FIXME we would love to use `CPointer<ByteVar>`, but despite our attempts,
    //       we were not able to import that type and resorting to code generation
    //       for this seemed overkill.
    //
    // See https://kotlinlang.org/docs/native-c-interop.html
    private var nativeWebRTCSrcCtx: ByteBuffer? = null

    private external fun nativeSetSurface(surface: Surface?)
    fun setSurface(surface: Surface?) {
        handler.post {
            nativeSetSurface(surface)
        }
    }

    private external fun nativeStartPipeline(
        signallerUri: String,
        peerId: String,
        preferOpusHardwareDec: Boolean
    )

    fun startPipeline(signallerUri: String, peerId: String, preferOpusHardwareDec: Boolean) {
        handler.post {
            nativeStartPipeline(signallerUri, peerId, preferOpusHardwareDec)
        }
    }

    private external fun nativeStopPipeline()

    fun stopPipeline() {
        handler.post {
            nativeStopPipeline()
        }
    }

    private external fun nativeFreeWebRTCSrcContext()
    override fun close() {
        handler.post {
            nativeFreeWebRTCSrcContext()
        }
    }

    interface ErrorListener {
        fun onWebRTCSrcError(errorMessage: String)
    }

    private fun onError(errorMessage: String) {
        handler.post {
            Log.d("WebRTCSrc", "Dispatching GStreamer error $errorMessage")
            errorListener.onWebRTCSrcError(errorMessage)
            nativeStopPipeline()
        }
    }

    private external fun nativeRecalculateLatency()

    fun onLatencyMessage() {
        handler.post {
            nativeRecalculateLatency()
        }
    }

    companion object {
        @JvmStatic
        private external fun nativeClassInit()

        @JvmStatic
        fun init(context: Context) {
            Log.d("WebRTCSrc", "Init")

            System.loadLibrary("gstreamer_android")
            GStreamer.init(context)

            System.loadLibrary("gstreamer_webrtcsrc")
            nativeClassInit()
        }
    }
}
