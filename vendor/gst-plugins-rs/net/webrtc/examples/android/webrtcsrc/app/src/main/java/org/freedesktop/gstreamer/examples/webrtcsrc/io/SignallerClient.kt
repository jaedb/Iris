package org.freedesktop.gstreamer.examples.webrtcsrc.io

import android.util.Log
import io.ktor.client.HttpClient
import io.ktor.client.plugins.websocket.WebSockets
import io.ktor.client.plugins.websocket.wss
import io.ktor.websocket.Frame
import io.ktor.websocket.readText
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.channels.ReceiveChannel
import kotlinx.coroutines.withContext
import kotlinx.serialization.encodeToString
import kotlinx.serialization.json.Json
import kotlinx.serialization.json.JsonElement
import kotlinx.serialization.json.JsonObject
import kotlinx.serialization.json.add
import kotlinx.serialization.json.put
import kotlinx.serialization.json.buildJsonObject
import kotlinx.serialization.json.decodeFromJsonElement
import kotlinx.serialization.json.putJsonArray

import org.freedesktop.gstreamer.examples.webrtcsrc.model.Peer

class SignallerClient(private val uri: String) {
    private suspend fun getNextMessage(incoming: ReceiveChannel<Frame>, expected: String): JsonObject {
        while (true) {
            for (frame in incoming) {
                if (frame is Frame.Text) {
                    val msg = Json.decodeFromString<JsonObject>(frame.readText())
                    val type = Json.decodeFromJsonElement<String>(
                        msg["type"] ?: throw Exception("message missing 'type'")
                    )
                    if (type == expected) {
                        Log.d(CAT, "Got $expected message")
                        return msg
                    } else {
                        Log.d(CAT, "Expected $expected, got $type => ignoring")
                    }
                }
            }
        }
    }

    suspend fun retrieveProducers(): Result<List<Peer>> {
        var res: Result<List<Peer>>? = null

        val client = HttpClient {
            install(WebSockets)
        }

        withContext(Dispatchers.IO) {
            Log.d(CAT, "Connecting to $uri")

            try {
                client.wss(uri) {
                    // Register as listener
                    val welcome = getNextMessage(incoming, "welcome")
                    val ourId = Json.decodeFromJsonElement<String>(
                        welcome["peerId"] ?: throw Exception("welcome missing 'peerId'")
                    )
                    Log.i(CAT, "Our id: $ourId")

                    val setStatusMsg = buildJsonObject {
                        put("type", "setPeerStatus")
                        putJsonArray("roles") { add("listener") }
                    }
                    outgoing.send(Frame.Text(Json.encodeToString(setStatusMsg)))

                    var registered = false
                    while (!registered) {
                        val peerStatus = getNextMessage(incoming, "peerStatusChanged")
                        val peerId = Json.decodeFromJsonElement<String>(
                            peerStatus["peerId"]
                                ?: throw Exception("peerStatusChanged missing 'peerId'")
                        )
                        if (peerId == ourId) {
                            registered = true
                        }
                    }

                    // Query producer list
                    val listMsg = buildJsonObject {
                        put("type", "list")
                    }
                    outgoing.send(Frame.Text(Json.encodeToString(listMsg)))

                    val peerStatus = getNextMessage(incoming, "list")
                    val list = Json.decodeFromJsonElement<Array<JsonElement>>(
                        peerStatus["producers"]
                            ?: throw Exception("list message missing 'producers'")
                    )
                    res = Result.success(list.map {
                        Log.i(CAT, "Got Producer $it")
                        Json.decodeFromJsonElement<Peer>(it)
                    })
                }

                Log.d(CAT, "Disconnecting")
                client.close()
            } catch (e: Exception) {
                Log.e(CAT, "Error retrieving producers: $e")
                res = Result.failure(e)
            }
        }
        return res!!
    }

    companion object {
        private const val CAT = "SignallerClient"
    }
}