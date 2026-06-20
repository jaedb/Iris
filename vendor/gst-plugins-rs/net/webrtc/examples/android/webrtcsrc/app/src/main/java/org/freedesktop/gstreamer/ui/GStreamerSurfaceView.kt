/* kotlin port of:
 https://gitlab.freedesktop.org/gstreamer/gstreamer/-/blob/subprojects/gst-examples/webrtc/android/app/src/main/java/org/freedesktop/gstreamer/webrtc/GStreamerSurfaceView.java
 */

package org.freedesktop.gstreamer.ui

import android.annotation.SuppressLint
import android.content.Context
import android.util.AttributeSet
import android.util.Log
import android.view.SurfaceView
import kotlin.math.max


class GStreamerSurfaceView(context: Context, attrs: AttributeSet) : SurfaceView(context, attrs) {
    var mediaWidth: Int = 320
    var mediaHeight: Int = 240

    // Called by the layout manager to find out our size and give us some rules.
    // We will try to maximize our size, and preserve the media's aspect ratio if
    // we are given the freedom to do so.
    override fun onMeasure(widthMeasureSpec: Int, heightMeasureSpec: Int) {
        if (mediaWidth == 0 || mediaHeight == 0) {
            super.onMeasure(widthMeasureSpec, heightMeasureSpec)
            return
        }

        var width = 0
        var height = 0
        val wMode = MeasureSpec.getMode(widthMeasureSpec)
        val hMode = MeasureSpec.getMode(heightMeasureSpec)
        val wSize = MeasureSpec.getSize(widthMeasureSpec)
        val hSize = MeasureSpec.getSize(heightMeasureSpec)

        Log.i("GStreamer", "onMeasure called with $mediaWidth x $mediaHeight")
        when (wMode) {
            MeasureSpec.AT_MOST -> {
                width = if (hMode == MeasureSpec.EXACTLY) {
                    (hSize * mediaWidth / mediaHeight).coerceAtMost(wSize)
                } else {
                    wSize
                }
            }
            MeasureSpec.EXACTLY -> width = wSize
            MeasureSpec.UNSPECIFIED -> width = mediaWidth
        }
        when (hMode) {
            MeasureSpec.AT_MOST -> {
                height = if (wMode == MeasureSpec.EXACTLY) {
                    (wSize * mediaHeight / mediaWidth).coerceAtMost(hSize)
                } else {
                    hSize
                }
            }
            MeasureSpec.EXACTLY -> height = hSize
            MeasureSpec.UNSPECIFIED -> height = mediaHeight
        }
        // Finally, calculate best size when both axis are free
        if (hMode == MeasureSpec.AT_MOST && wMode == MeasureSpec.AT_MOST) {
            val correctHeight: Int = width * mediaHeight / mediaWidth
            val correctWidth: Int = height * mediaWidth / mediaHeight

            if (correctHeight < height) height = correctHeight
            else width = correctWidth
        }

        // Obey minimum size
        width = max(suggestedMinimumWidth.toDouble(), width.toDouble()).toInt()
        height = max(suggestedMinimumHeight.toDouble(), height.toDouble()).toInt()
        setMeasuredDimension(width, height)
    }
}