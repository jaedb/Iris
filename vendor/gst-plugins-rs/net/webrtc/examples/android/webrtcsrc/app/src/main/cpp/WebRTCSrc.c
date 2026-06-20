/* Android GStreamer WebRTCSrc native code.
 *
 * The following file implements the native logic for an Android application to be able to
 * to start an audio / video stream from a producer using GStreamer WebRTCSrc. This is designed
 * to work in tandem with the managed Kotlin class `WebRTCSrc`.
 *
 * - The managed class is in charge of spawning an event loop, initializing the native libraries and
 *   scheduling calls to native functions (such as `startPipeline`, `stopPipeline`, `setSurface`, ...).
 *   The native implementation of these functions expects the calls to be handled one at a time,
 *   which is the case when scheduling is handled by an `android.os.Handler`.
 * - See the `native_methods[]` for the mapping between the managed class and the native functions.
 * - See also the comment for `attach_current_thread()` regarding calling managed call from a thread
 *   spawned by C code.
 */

#include <string.h>
#include <stdint.h>
#include <jni.h>
#include <android/log.h>
#include <android/native_window.h>
#include <android/native_window_jni.h>

#include <gst/video/videooverlay.h>

GST_DEBUG_CATEGORY_STATIC (debug_category);
#define GST_CAT_DEFAULT debug_category

/* The WebRTCSrc Context which gathers objects needed between native calls for a streaming session */
typedef struct _WebRTCSrcCtx {
    jobject managed_webrtcsrc;
    GstElement *pipe;
    GstBus *bus;
    GMutex video_pad_mutex;
    ANativeWindow *native_window;
    GstElement *webrtcsrc, *video_sink;
} WebRTCSrcCtx;

static JavaVM *java_vm;
/* The identifier of the field of the managed class where the WebRTCSrc Context from which
 * current streaming session can be retrieved */
static jfieldID webrtcsrc_ctx_field_id;
static jmethodID on_error_method_id;
static jmethodID on_latency_message_method_id;

/* Retrieves the WebRTCSrc Context for current streaming session
 * from the managed WebRTCSrc object `self` */
static WebRTCSrcCtx *
get_webrtcsrc_ctx(JNIEnv *env, jobject self) {
    jobject field = (*env)->GetObjectField(env, self, webrtcsrc_ctx_field_id);
    return (WebRTCSrcCtx *) (field != NULL ? (*env)->GetDirectBufferAddress(env, field) : NULL);
}

/* Registers this thread with the VM
 *
 * - Calls from the managed code originate from a JVM thread which don't need to be attached.
 * - Calls from threads spawned by C code must be attached before calling managed code.
 *   Ex.: `on_error()` is called from the `bus` sync handler, which is triggered on the thread
 *        on which the error originated.
 *
 * The attached thread must be detached when no more managed code is to be called.
 * See `detach_current_thread()`.
 */
static JNIEnv *
attach_current_thread() {
    JNIEnv *env;
    JavaVMAttachArgs args;

    GST_LOG("Attaching thread %p", g_thread_self());
    args.version = JNI_VERSION_1_4;
    args.name = NULL;
    args.group = NULL;

    if ((*java_vm)->AttachCurrentThread(java_vm, &env, &args) < 0) {
        GST_ERROR ("Failed to attach current thread");
        return NULL;
    }

    return env;
}

/* Unregisters this thread from the VM
 *
 * See `attach_current_thread()` for details.
 */
static void
detach_current_thread() {
    GST_LOG("Detaching thread %p", g_thread_self());
    (*java_vm)->DetachCurrentThread(java_vm);
}

static void
on_incoming_stream(__attribute__((unused)) GstElement *webrtcsrc, GstPad *pad, WebRTCSrcCtx *ctx) {
    GstPadLinkReturn ret;
    const gchar *name = gst_pad_get_name(pad);

    if (g_str_has_prefix(name, "video")) {
        g_mutex_lock(&ctx->video_pad_mutex);

        if (ctx->video_sink == NULL) {
            GstElement *queue;
            GstPad *queue_pad;

            GST_DEBUG("Handling video pad %s", name);

            ctx->video_sink = gst_element_factory_make("glimagesink", NULL);
            g_assert_nonnull(ctx->video_sink);
            if (ctx->native_window)
                gst_video_overlay_set_window_handle(GST_VIDEO_OVERLAY(ctx->video_sink),
                                                    (guintptr) ctx->native_window);

            queue = gst_element_factory_make("queue", NULL);
            g_assert_nonnull(queue);

            gst_bin_add_many(GST_BIN(ctx->pipe), queue, ctx->video_sink, NULL);
            gst_element_link_many(queue, ctx->video_sink, NULL);
            queue_pad = gst_element_get_static_pad(queue, "sink");
            ret = gst_pad_link(pad, queue_pad);
            gst_object_unref(queue_pad);
            g_assert(ret == GST_PAD_LINK_OK);

            gst_element_sync_state_with_parent(queue);
            gst_element_sync_state_with_parent(ctx->video_sink);
        } else {
            GstElement *sink;
            GstPad *sinkpad;

            GST_INFO("Ignoring additional video pad %s", name);
            sink = gst_element_factory_make("fakesink", NULL);
            g_assert_nonnull(sink);

            gst_bin_add(GST_BIN(ctx->pipe), sink);

            sinkpad = gst_element_get_static_pad(sink, "sink");
            ret = gst_pad_link(pad, sinkpad);
            gst_object_unref(sinkpad);
            g_assert(ret == GST_PAD_LINK_OK);

            gst_element_sync_state_with_parent(sink);
        }

        g_mutex_unlock(&ctx->video_pad_mutex);
    } else if (g_str_has_prefix(name, "audio")) {
        GstElement *conv, *resample, *queue, *sink;
        GstPad *conv_pad;

        GST_DEBUG("Handling audio stream %s", name);

        conv = gst_element_factory_make("audioconvert", NULL);
        g_assert_nonnull(conv);
        resample = gst_element_factory_make("audioresample", NULL);
        g_assert_nonnull(resample);
        queue = gst_element_factory_make("queue", NULL);
        g_assert_nonnull(queue);
        sink = gst_element_factory_make("autoaudiosink", NULL);
        g_assert_nonnull(sink);

        gst_bin_add_many(GST_BIN(ctx->pipe), conv, resample, queue, sink, NULL);
        gst_element_link_many(conv, resample, queue, sink, NULL);
        conv_pad = gst_element_get_static_pad(conv, "sink");
        ret = gst_pad_link(pad, conv_pad);
        gst_object_unref(conv_pad);
        g_assert(ret == GST_PAD_LINK_OK);

        gst_element_sync_state_with_parent(conv);
        gst_element_sync_state_with_parent(queue);
        gst_element_sync_state_with_parent(resample);
        gst_element_sync_state_with_parent(sink);
    } else {
        GST_ERROR("Ignoring unknown pad %s", name);
    }
}

static void
on_error(JNIEnv *env, WebRTCSrcCtx *ctx, GError *err) {
    jstring error_msg = (*env)->NewStringUTF(env, err->message);
    (*env)->CallVoidMethod(env, ctx->managed_webrtcsrc, on_error_method_id, error_msg);

    if ((*env)->ExceptionCheck(env)) {
        (*env)->ExceptionDescribe(env);
        (*env)->ExceptionClear(env);
    }

    (*env)->DeleteLocalRef(env, error_msg);
}

static GstBusSyncReply
bus_sync_handler(__attribute__((unused)) GstBus *bus, GstMessage *message, gpointer user_data) {
    /* Ideally, we would pass the `GstMessage *` to the managed code, which would act
     * as an asynchronous handler and decide what to do about it. However, passing a C pointer
     * to managed code is a bit clumsy. For now, we handle the few cases on a case by case basis.
     *
     * Possible solution would be to use: https://kotlinlang.org/docs/native-c-interop.html
     */

    WebRTCSrcCtx *ctx = user_data;

    switch (GST_MESSAGE_TYPE(message)) {
        case GST_MESSAGE_ERROR: {
            JNIEnv *env;
            GError *error = NULL;
            gchar *debug = NULL;

            gst_message_parse_error(message, &error, &debug);
            GST_ERROR("Error on bus: %s (debug: %s)", error->message, debug);

            /* Pass error message to app
             * This function is called from a thread spawned by C code so we need to attach it
             * before calling managed code.
             */
            env = attach_current_thread();
            g_assert(env != NULL);
            on_error(env, ctx, error);
            detach_current_thread();

            g_error_free(error);
            g_free(debug);
            break;
        }
        case GST_MESSAGE_WARNING: {
            GError *error = NULL;
            gchar *debug = NULL;

            gst_message_parse_warning(message, &error, &debug);
            GST_WARNING("Warning on bus: %s (debug: %s)", error->message, debug);
            g_error_free(error);
            g_free(debug);
            break;
        }
        case GST_MESSAGE_LATENCY: {
            GST_LOG("Got Latency message");
            JNIEnv *env;
            /* Notify the app a Latency message was posted
             * This will cause the managed WebRTCSrc to trigger `recalculateLatency()`,
             * which we couldn't trigger call immediately from here nor via `gst_element_call_async`
             * without causing deadlocks.
             */
            env = attach_current_thread();
            g_assert(env != NULL);
            (*env)->CallVoidMethod(env, ctx->managed_webrtcsrc, on_latency_message_method_id);
            detach_current_thread();

            break;
        }
        default:
            break;
    }

    return GST_BUS_DROP;
}

/*
 * Java Bindings
 */

static void
stop_pipeline(JNIEnv *env, jobject self) {
    WebRTCSrcCtx *ctx = get_webrtcsrc_ctx(env, self);

    if (!ctx)
        return;

    if (!ctx->pipe)
        return;

    GST_DEBUG("Stopping pipeline");

    gst_element_set_state(GST_ELEMENT(ctx->pipe), GST_STATE_NULL);

    gst_bus_set_sync_handler(ctx->bus, NULL, NULL, NULL);
    gst_object_unref(ctx->bus);
    gst_object_unref(ctx->pipe);
    g_mutex_clear(&ctx->video_pad_mutex);

    ctx->webrtcsrc = NULL;
    ctx->video_sink = NULL;
    ctx->pipe = NULL;

    GST_DEBUG("Pipeline stopped");
}

static void
free_webrtcsrc_ctx(JNIEnv *env, jobject self) {
    WebRTCSrcCtx *ctx = get_webrtcsrc_ctx(env, self);

    if (!ctx)
        return;

    stop_pipeline(env, self);

    GST_DEBUG("Freeing Context");

    (*env)->DeleteGlobalRef(env, ctx->managed_webrtcsrc);

    g_free(ctx);
    (*env)->SetObjectField(env, self, webrtcsrc_ctx_field_id, NULL);
}

static void
start_pipeline(JNIEnv *env, jobject self, jstring j_signaller_uri, jstring j_peer_id,
               jboolean prefer_opus_hard_dec) {
    WebRTCSrcCtx *ctx = get_webrtcsrc_ctx(env, self);
    const gchar *signaller_uri, *peer_id;
    GObject *signaller;
    GstStateChangeReturn ret;

    g_assert(!ctx);
    ctx = g_new0(WebRTCSrcCtx, 1);
    (*env)->SetObjectField(env,
                           self,
                           webrtcsrc_ctx_field_id,
                           (*env)->NewDirectByteBuffer(env, (void *) ctx, sizeof(WebRTCSrcCtx)));
    ctx->managed_webrtcsrc = (*env)->NewGlobalRef(env, self);

    /* Preferred OPUS decoder */
    GstRegistry *reg = gst_registry_get();
    GstPluginFeature *opusdec = gst_registry_lookup_feature(reg, "opusdec");
    if (opusdec != NULL) {
        if (prefer_opus_hard_dec) {
            GST_INFO("Lowering rank for 'opusdec'");
            gst_plugin_feature_set_rank(opusdec, GST_RANK_PRIMARY - 1);
        } else {
            GST_INFO("Raising rank for 'opusdec'");
            gst_plugin_feature_set_rank(opusdec, GST_RANK_PRIMARY + 1);
        }

        gst_object_unref(opusdec);
    } else {
        GST_WARNING("Couldn't find element 'opusdec'");
    }

    GST_DEBUG("Preparing pipeline");

    g_mutex_init(&ctx->video_pad_mutex);
    ctx->video_sink = NULL;
    ctx->pipe = gst_pipeline_new(NULL);
    ctx->webrtcsrc = gst_element_factory_make("webrtcsrc", NULL);
    g_assert(ctx->webrtcsrc != NULL);
    gst_bin_add(GST_BIN(ctx->pipe), ctx->webrtcsrc);

    g_object_get(ctx->webrtcsrc, "signaller", &signaller, NULL);
    g_assert(signaller != NULL);

    /* Signaller Uri & Peer Id */
    signaller_uri = (*env)->GetStringUTFChars(env, j_signaller_uri, NULL);
    peer_id = (*env)->GetStringUTFChars(env, j_peer_id, NULL);

    GST_DEBUG("Setting producer id %s from signaller %s", peer_id, signaller_uri);
    g_object_set(signaller,
                 "uri", signaller_uri,
                 "producer-peer-id", peer_id, NULL);

    g_object_unref(signaller);

    (*env)->ReleaseStringUTFChars(env, j_signaller_uri, signaller_uri);
    (*env)->ReleaseStringUTFChars(env, j_peer_id, peer_id);

    /* Aiming for low latency by default. Could be an option along with pipeline latency */
    g_object_set(ctx->webrtcsrc, "do-retransmission", FALSE, NULL);

    g_signal_connect(ctx->webrtcsrc, "pad-added", G_CALLBACK(on_incoming_stream), ctx);

    ctx->bus = gst_pipeline_get_bus(GST_PIPELINE(ctx->pipe));
    gst_bus_set_sync_handler(ctx->bus, bus_sync_handler, ctx, NULL);

    GST_DEBUG("Starting pipeline");
    ret = gst_element_set_state(GST_ELEMENT(ctx->pipe), GST_STATE_PLAYING);
    if (ret == GST_STATE_CHANGE_FAILURE) {
        GST_ERROR("Failed to start the pipeline");
        stop_pipeline(env, self);
        return;
    }

    GST_DEBUG("Pipeline started");
}

static void
set_surface(JNIEnv *env, jobject self, jobject surface) {
    WebRTCSrcCtx *ctx = get_webrtcsrc_ctx(env, self);
    ANativeWindow *new_native_window;

    if (!ctx)
        return;

    new_native_window = surface ? ANativeWindow_fromSurface(env, surface)
                                : NULL;
    GST_DEBUG("Received surface %p (native window %p)", surface,
              new_native_window);

    if (ctx->native_window) {
        ANativeWindow_release(ctx->native_window);
    }

    ctx->native_window = new_native_window;

    if (ctx->video_sink)
        gst_video_overlay_set_window_handle(
                GST_VIDEO_OVERLAY(ctx->video_sink),
                (guintptr) new_native_window);
}

static void
recalculate_latency(JNIEnv *env, jobject self) {
    WebRTCSrcCtx *ctx = get_webrtcsrc_ctx(env, self);
    if (!ctx)
        return;

    GST_DEBUG("Recalculating latency");
    gst_bin_recalculate_latency(GST_BIN(ctx->pipe));
}

static void
class_init(JNIEnv *env, jclass klass) {
    guint major, minor, micro, nano;

    /* There are restrictions on using gst_bus_set_sync_handler prior to version 1.18 */
    gst_version(&major, &minor, &micro, &nano);
    if (major < 1 || (major == 1 && minor < 18)) {
        __android_log_print(ANDROID_LOG_ERROR, "WebRTCSrc (native)",
                            "Unsupported GStreamer version < 1.18");
        return;
    }

    webrtcsrc_ctx_field_id =
            (*env)->GetFieldID(env, klass, "nativeWebRTCSrcCtx", "Ljava/nio/ByteBuffer;");

    on_error_method_id =
            (*env)->GetMethodID(env, klass, "onError", "(Ljava/lang/String;)V");

    on_latency_message_method_id =
            (*env)->GetMethodID(env, klass, "onLatencyMessage", "()V");

    if (!webrtcsrc_ctx_field_id || !on_error_method_id || !on_latency_message_method_id) {
        /* We emit this message through the Android log instead of the GStreamer log because the later
         * has not been initialized yet.
         */
        __android_log_print(ANDROID_LOG_ERROR, "WebRTCSrc (native)",
                            "The calling class does not include necessary fields or methods");
        return;
    }

    GST_DEBUG_CATEGORY_INIT(debug_category, "WebRTCSrc", 0, "GStreamer WebRTCSrc Android example");
    gst_debug_set_threshold_for_name("WebRTCSrc", GST_LEVEL_DEBUG);
}

/* List of implemented native methods */
static JNINativeMethod native_methods[] = {
        {"nativeClassInit",            "()V", (void *) class_init},
        {"nativeStartPipeline",        "(Ljava/lang/String;Ljava/lang/String;Z)V",
                                              (void *) start_pipeline},
        {"nativeStopPipeline",         "()V",
                                              (void *) stop_pipeline},
        {"nativeSetSurface",           "(Landroid/view/Surface;)V",
                                              (void *) set_surface},
        {"nativeRecalculateLatency",   "()V",
                                              (void *) recalculate_latency},
        {"nativeFreeWebRTCSrcContext", "()V", (void *) free_webrtcsrc_ctx},
};

/* Library initializer */
jint
JNI_OnLoad(JavaVM *vm, __attribute__((unused)) void *reserved) {
    JNIEnv *env = NULL;

    java_vm = vm;

    if ((*vm)->GetEnv(vm, (void **) &env, JNI_VERSION_1_4) != JNI_OK) {
        __android_log_print(ANDROID_LOG_ERROR, "WebRTCSrc (native)", "Could not retrieve JNIEnv");
        return 0;
    }
    jclass klass = (*env)->FindClass(env, "org/freedesktop/gstreamer/net/WebRTCSrc");
    if (!klass) {
        __android_log_print(ANDROID_LOG_ERROR, "WebRTCSrc (native)",
                            "Could not retrieve class org.freedesktop.gstreamer.net.WebRTCSrc");
        return 0;
    }
    if ((*env)->RegisterNatives(env, klass, native_methods,
                                G_N_ELEMENTS(native_methods))) {
        __android_log_print(ANDROID_LOG_ERROR, "WebRTCSrc (native)",
                            "Could not register native methods for org.freedesktop.gstreamer.net.WebRTCSrc");
        return 0;
    }

    return JNI_VERSION_1_4;
}
