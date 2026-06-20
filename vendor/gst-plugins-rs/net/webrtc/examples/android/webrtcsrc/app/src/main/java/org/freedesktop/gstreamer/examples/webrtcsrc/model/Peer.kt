package org.freedesktop.gstreamer.examples.webrtcsrc.model

import kotlinx.serialization.Serializable

@Serializable
data class Peer(val id: String, val meta: Map<String, String>?)
