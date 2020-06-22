import random
import string
import logging
import json
import pykka
import urllib
import os
import sys
import tornado.web
import tornado.ioloop
import time
import pickle
from pkg_resources import parse_version
from tornado.escape import json_encode
from tornado.httpclient import AsyncHTTPClient, HTTPRequest

from . import Extension
from .system import IrisSystemThread

if sys.platform == "win32":
    import ctypes

# import logger
logger = logging.getLogger(__name__)


class IrisCore(pykka.ThreadingActor):
    version = ""
    spotify_token = False
    queue_metadata = {}
    connections = {}
    commands = {}
    initial_consume = False
    radio = {
        "enabled": 0,
        "seed_artists": [],
        "seed_genres": [],
        "seed_tracks": [],
        "results": [],
    }
    ioloop = None

    @classmethod
    async def do_fetch(cls, client, request):
        # This wrapper function exists to ease mocking.
        return await client.fetch(request)

    def setup(self, config, core):
        self.config = config
        self.core = core

    ##
    # Mopidy server is starting
    ##
    def start(self):
        logger.info("Starting Iris " + Extension.version)

        # Load our commands from file
        self.commands = self.load_from_file("commands")

    ##
    # Mopidy is shutting down
    ##
    def stop(self):
        logger.info("Stopping Iris")

    ##
    # Load a dict from disk
    #
    # @param name String
    # @return Dict
    ##
    def load_from_file(self, name):
        file_path = Extension.get_data_dir(self.config) / ("%s.pkl" % name)

        try:
            with file_path.open("rb") as f:
                content = pickle.load(f)
                f.close()
                return content
        except Exception:
            return {}

    ##
    # Save dict object to disk
    #
    # @param dict Dict
    # @param name String
    # @return void
    ##
    def save_to_file(self, dict, name):
        file_path = Extension.get_data_dir(self.config) / ("%s.pkl" % name)

        try:
            with file_path.open("wb") as f:
                pickle.dump(dict, f, pickle.HIGHEST_PROTOCOL)
                pickle.close()
        except Exception:
            return False

    ##
    # Generate a random string
    #
    # Used for connection_ids where none is provided by client
    # @return string
    ##
    def generateGuid(self):
        return "".join(
            random.choices(string.ascii_uppercase + string.digits, k=12)
        )

    ##
    # Digest a protocol header into it's id/name parts
    #
    # @return dict
    ##
    def digest_protocol(self, protocol):

        # if we're a string, split into list
        # this handles the different ways we get this passed
        # (select_subprotocols gives string, headers.get gives list)
        if isinstance(protocol, str):

            # make sure we strip any spaces (IE gives "element,element", proper
            # browsers give "element, element")
            protocol = [i.strip() for i in protocol.split(",")]

        # if we've been given a valid array
        try:
            client_id = protocol[0]
            connection_id = protocol[1]
            username = protocol[2]
            generated = False

        # invalid, so just create a default connection, and auto-generate an ID
        except BaseException:
            client_id = self.generateGuid()
            connection_id = self.generateGuid()
            username = "Anonymous"
            generated = True

        # construct our protocol object, and return
        return {
            "client_id": client_id,
            "connection_id": connection_id,
            "username": username,
            "generated": generated,
        }

    def send_message(self, *args, **kwargs):
        callback = kwargs.get("callback", None)
        data = kwargs.get("data", None)

        logger.debug(data)

        # Catch invalid recipient
        if data["recipient"] not in self.connections:
            error = 'Connection "' + data["recipient"] + '" not found'
            logger.error(error)

            error = {"message": error}
            if callback:
                callback(False, error)
            else:
                return error

        # Sending of an error
        if "error" in data:
            message = {"jsonrpc": "2.0", "error": data["error"]}

        # Sending of a regular message
        else:
            message = {
                "jsonrpc": "2.0",
                "method": data["method"] if "method" in data else None,
            }
            if "id" in data:
                message["id"] = data["id"]
            if "params" in data:
                message["params"] = data["params"]
            if "result" in data:
                message["result"] = data["result"]

        # Dispatch the message
        try:
            self.connections[data["recipient"]]["connection"].write_message(
                json_encode(message)
            )

            response = {"message": "Sent message to " + data["recipient"]}
            if callback:
                callback(response)
            else:
                return response
        except BaseException:
            error = "Failed to send message to " + data["recipient"]
            logger.error(error)

            error = {"message": error}
            if callback:
                callback(False, error)
            else:
                return error

    def broadcast(self, *args, **kwargs):
        callback = kwargs.get("callback", None)
        data = kwargs.get("data", None)

        logger.debug(data)

        if "error" in data:
            message = {"jsonrpc": "2.0", "error": data["error"]}
        else:
            message = {
                "jsonrpc": "2.0",
                "method": data["method"] if "method" in data else None,
                "params": data["params"] if "params" in data else None,
            }

        for connection in self.connections.values():

            send_to_this_connection = True

            # Don't send the broadcast to the origin, naturally
            if "connection_id" in data:
                if connection["connection_id"] == data["connection_id"]:
                    send_to_this_connection = False

            if send_to_this_connection:
                connection["connection"].write_message(json_encode(message))

        response = {
            "message": "Broadcast to "
            + str(len(self.connections))
            + " connections"
        }
        if callback:
            callback(response)
        else:
            return response

    ##
    # Connections
    #
    # Contains all our connections and client details. This requires
    # updates when new clients connect, and old ones disconnect. These
    # events are broadcast to all current connections
    ##

    def get_connections(self, *args, **kwargs):
        callback = kwargs.get("callback", None)

        connections = []
        for connection in self.connections.values():
            connections.append(connection["client"])

        response = {"connections": connections}
        if callback:
            callback(response)
        else:
            return response

    def add_connection(self, *args, **kwargs):
        connection = kwargs.get("connection", None)
        client = kwargs.get("client", None)

        logger.debug("Connection added")
        logger.debug(connection)

        self.connections[client["connection_id"]] = {
            "client": client,
            "connection_id": client["connection_id"],
            "connection": connection,
        }

        self.broadcast(
            data={
                "method": "connection_added",
                "params": {"connection": client},
            }
        )

    def update_connection(self, *args, **kwargs):
        callback = kwargs.get("callback", None)
        data = kwargs.get("data", {})
        connection_id = data["connection_id"]

        if connection_id in self.connections:
            username = data["username"]
            client_id = data["client_id"]
            self.connections[connection_id]["client"]["username"] = username
            self.connections[connection_id]["client"]["client_id"] = client_id
            self.broadcast(
                data={
                    "method": "connection_changed",
                    "params": {
                        "connection": self.connections[connection_id]["client"]
                    },
                }
            )
            response = {"connection": self.connections[connection_id]["client"]}
            if callback:
                callback(response)
            else:
                return response

        else:
            error = 'Connection "' + data["connection_id"] + '" not found'
            logger.error(error)

            error = {"message": error}
            if callback:
                callback(False, error)
            else:
                return error

    def remove_connection(self, connection_id):
        if connection_id in self.connections:
            try:
                client = self.connections[connection_id]["client"]
                del self.connections[connection_id]
                self.broadcast(
                    data={
                        "method": "connection_removed",
                        "params": {"connection": client},
                    }
                )
            except BaseException:
                logger.error("Failed to close connection to " + connection_id)

    def set_username(self, *args, **kwargs):
        callback = kwargs.get("callback", None)
        data = kwargs.get("data", {})
        connection_id = data["connection_id"]

        if connection_id in self.connections:
            username = data["username"]
            self.connections[connection_id]["client"]["username"] = username
            self.broadcast(
                data={
                    "method": "connection_changed",
                    "params": {
                        "connection": self.connections[connection_id]["client"]
                    },
                }
            )
            response = {
                "connection_id": connection_id,
                "username": data["username"],
            }
            if callback:
                callback(response)
            else:
                return response

        else:
            error = 'Connection "' + data["connection_id"] + '" not found'
            logger.error(error)

            error = {"message": error}
            if callback:
                callback(False, error)
            else:
                return error

    ##
    # System controls
    #
    # Faciitates upgrades and configuration fetching
    ##

    def get_config(self, *args, **kwargs):
        callback = kwargs.get("callback", False)

        # handle config setups where there is no username/password
        # Iris won't work properly anyway, but at least we won't get server
        # errors
        if "spotify" in self.config and "username" in self.config["spotify"]:
            spotify_username = self.config["spotify"]["username"]
        else:
            spotify_username = False

        response = {
            "config": {
                "is_root": self.is_root(),
                "spotify_username": spotify_username,
                "country": self.config["iris"]["country"],
                "locale": self.config["iris"]["locale"],
                "spotify_authorization_url": self.config["iris"][
                    "spotify_authorization_url"
                ],
                "lastfm_authorization_url": self.config["iris"][
                    "lastfm_authorization_url"
                ],
                "genius_authorization_url": self.config["iris"][
                    "genius_authorization_url"
                ],
            }
        }

        if callback:
            callback(response)
        else:
            return response

    async def get_version(self, *args, **kwargs):

        callback = kwargs.get("callback", False)
        url = "https://pypi.python.org/pypi/Mopidy-Iris/json"
        http_client = AsyncHTTPClient()

        try:
            http_response = await http_client.fetch(url)
            response_body = json.loads(http_response.body)
            latest_version = response_body["info"]["version"]
            current_version = Extension.version

            # compare our versions, and convert result to boolean
            upgrade_available = parse_version(latest_version) > parse_version(
                current_version
            )
            upgrade_available = upgrade_available == 1

        except (urllib.request.HTTPError, urllib.request.URLError):
            latest_version = "0.0.0"
            upgrade_available = False

        response = {
            "version": {
                "current": current_version,
                "latest": latest_version,
                "is_root": self.is_root(),
                "upgrade_available": upgrade_available,
            }
        }
        if callback:
            callback(response)
        else:
            return response

    ##
    # Restart Mopidy
    # This requires sudo access to system.sh
    ##
    def restart(self, *args, **kwargs):
        callback = kwargs.get("callback", False)
        ioloop = kwargs.get("ioloop", False)

        # Trigger the action
        IrisSystemThread("restart", ioloop, self.restart_callback).start()

        self.broadcast(data={"method": "restart_started"})

        response = {"message": "Restart started"}
        if callback:
            callback(response)
        else:
            return response

    def restart_callback(self, response, error, update):
        if error:
            self.broadcast(data={"method": "restart_error", "params": error})
        elif update:
            self.broadcast(data={"method": "restart_updated", "params": update})
        else:
            self.broadcast(
                data={"method": "restart_finished", "params": response}
            )

    ##
    # Run an upgrade of Iris
    ##
    def upgrade(self, *args, **kwargs):
        callback = kwargs.get("callback", False)
        ioloop = kwargs.get("ioloop", False)

        self.broadcast(data={"method": "upgrade_started"})

        # Trigger the action
        IrisSystemThread("upgrade", ioloop, self.upgrade_callback).start()

        response = {"message": "Upgrade started"}

        if callback:
            callback(response)
        else:
            return response

    def upgrade_callback(self, response, error, update):
        if error:
            self.broadcast(data={"method": "upgrade_error", "params": error})
        elif update:
            self.broadcast(data={"method": "upgrade_updated", "params": update})
        else:
            self.broadcast(
                data={"method": "upgrade_finished", "params": response}
            )
            self.restart()

    ##
    # Run a mopidy local scan
    # Essetially an alias to "mopidyctl local scan"
    ##
    def local_scan(self, *args, **kwargs):
        callback = kwargs.get("callback", False)
        ioloop = kwargs.get("ioloop", False)

        # Trigger the action
        IrisSystemThread("local_scan", ioloop, self.local_scan_callback).start()

        self.broadcast(data={"method": "local_scan_started"})

        response = {"message": "Local scan started"}
        if callback:
            callback(response)
        else:
            return response

    def local_scan_callback(self, response, error, update):
        if error:
            self.broadcast(data={"method": "local_scan_error", "params": error})
        elif update:
            self.broadcast(
                data={"method": "local_scan_updated", "params": update}
            )
        else:
            self.broadcast(
                data={"method": "local_scan_finished", "params": response}
            )

    ##
    # Spotify Radio
    #
    # Accepts seed URIs and creates radio-like experience. When our
    # tracklist is nearly empty, we fetch more recommendations. This
    # can result in duplicates. We keep the recommendations limit low
    # to avoid timeouts and slow UI
    ##

    def get_radio(self, *args, **kwargs):
        callback = kwargs.get("callback", False)

        response = {"radio": self.radio}
        if callback:
            callback(response)
        else:
            return response

    async def change_radio(self, *args, **kwargs):
        callback = kwargs.get("callback", False)
        data = kwargs.get("data", {})

        # We're starting a new radio (or forced restart)
        if data["reset"] or not self.radio["enabled"]:
            starting = True
            self.initial_consume = self.core.tracklist.get_consume().get()
        else:
            starting = False

        # fetch more tracks from Mopidy-Spotify
        self.radio = {
            "seed_artists": data["seed_artists"],
            "seed_genres": data["seed_genres"],
            "seed_tracks": data["seed_tracks"],
            "enabled": 1,
            "results": [],
        }
        uris = await self.load_more_tracks()

        # make sure we got recommendations
        if uris:
            if starting:
                self.core.tracklist.clear()

            self.core.tracklist.set_consume(True)

            # We only want to play the first batch
            added = self.core.tracklist.add(uris=uris[0:3])

            if not added.get():
                logger.error("No recommendations added to queue")

                self.radio["enabled"] = 0
                error = {
                    "message": "No recommendations added to queue",
                    "radio": self.radio,
                }
                if callback:
                    callback(False, error)
                else:
                    return error

            # Save results (minus first batch) for later use
            self.radio["results"] = uris[3:]

            self.add_radio_metadata(added)

            if starting:
                self.core.playback.play()
                self.broadcast(
                    data={
                        "method": "radio_started",
                        "params": {"radio": self.radio},
                    }
                )
            else:
                self.broadcast(
                    data={
                        "method": "radio_changed",
                        "params": {"radio": self.radio},
                    }
                )

            self.get_radio(callback=callback)
            return

        # Failed fetching/adding tracks, so no-go
        else:
            logger.error("No recommendations returned by Spotify")
            self.radio["enabled"] = 0
            error = {
                "code": 32500,
                "message": "Could not start radio",
                "data": {"radio": self.radio},
            }
            if callback:
                callback(False, error)
            else:
                return error

    def stop_radio(self, *args, **kwargs):
        callback = kwargs.get("callback", False)

        self.radio = {
            "enabled": 0,
            "seed_artists": [],
            "seed_genres": [],
            "seed_tracks": [],
            "results": [],
        }

        # restore initial consume state
        self.core.tracklist.set_consume(self.initial_consume)
        self.core.playback.stop()

        self.broadcast(
            data={"method": "radio_stopped", "params": {"radio": self.radio}}
        )

        response = {"message": "Stopped radio"}
        if callback:
            callback(response)
        else:
            return response

    async def load_more_tracks(self, *args, **kwargs):
        try:
            await self.get_spotify_token()
            spotify_token = self.spotify_token
            access_token = spotify_token["access_token"]
        except BaseException:
            error = "IrisFrontend: access_token missing or invalid"
            logger.error(error)
            return False

        url = "https://api.spotify.com/v1/recommendations/"
        url = (
            url
            + "?seed_artists="
            + (",".join(self.radio["seed_artists"])).replace(
                "spotify:artist:", ""
            )
        )
        url = (
            url
            + "&seed_genres="
            + (",".join(self.radio["seed_genres"])).replace(
                "spotify:genre:", ""
            )
        )
        url = (
            url
            + "&seed_tracks="
            + (",".join(self.radio["seed_tracks"])).replace(
                "spotify:track:", ""
            )
        )
        url = url + "&limit=50"
        http_client = AsyncHTTPClient()

        try:
            http_response = await http_client.fetch(
                url, "POST", headers={"Authorization": "Bearer " + access_token}
            )
            response_body = json.loads(http_response.body)

            uris = []
            for track in response_body["tracks"]:
                uris.append(track["uri"])

            return uris

        except (urllib.error.HTTPError, urllib.error.URLError) as e:
            error = json.loads(e.read())
            error = {
                "message": "Could not fetch Spotify recommendations: "
                + error["error_description"]
            }
            logger.error(
                "Could not fetch Spotify recommendations: "
                + error["error_description"]
            )
            logger.debug(error)
            return False

    def check_for_radio_update(self):
        tracklistLength = self.core.tracklist.get_length().get()
        if tracklistLength < 3 and self.radio["enabled"] == 1:

            # Grab our loaded tracks
            uris = self.radio["results"]

            # We've run out of pre-fetched tracks, so we need to get more
            # recommendations
            if len(uris) < 3:
                uris = self.load_more_tracks()

            # Remove the next batch, and update our results
            self.radio["results"] = uris[3:]

            # Only add the next set of uris
            uris = uris[0:3]

            added = self.core.tracklist.add(uris=uris)

            self.add_radio_metadata(added)

    def add_radio_metadata(self, added):
        seeds = ""
        if len(self.radio["seed_artists"]) > 0:
            seeds = seeds + (",".join(self.radio["seed_artists"])).replace(
                "spotify:artist:", "spotify_artist_"
            )
        if len(self.radio["seed_tracks"]) > 0:
            if seeds != "":
                seeds = seeds + ","
            seeds = seeds + (",".join(self.radio["seed_tracks"])).replace(
                "spotify:track:", "spotify_track_"
            )
        if len(self.radio["seed_genres"]) > 0:
            if seeds != "":
                seeds = seeds + ","
            seeds = seeds + (",".join(self.radio["seed_genres"])).replace(
                "spotify:genre:", "spotify_genre_"
            )

        metadata = {
            "tlids": [],
            "added_by": "Radio",
            "added_from": "iris:radio:" + seeds,
        }
        for added_tltrack in added.get():
            metadata["tlids"].append(added_tltrack.tlid)

        self.add_queue_metadata(data=metadata)

    ##
    # Additional queue metadata
    #
    # This maps tltracks with extra info for display in Iris, including
    # added_by and from_uri.
    ##

    def get_queue_metadata(self, *args, **kwargs):
        callback = kwargs.get("callback", False)

        response = {"queue_metadata": self.queue_metadata}
        if callback:
            callback(response)
        else:
            return response

    def add_queue_metadata(self, *args, **kwargs):
        callback = kwargs.get("callback", False)
        data = kwargs.get("data", {})

        for tlid in data["tlids"]:
            item = {
                "tlid": tlid,
                "added_from": data["added_from"]
                if "added_from" in data
                else None,
                "added_by": data["added_by"] if "added_by" in data else None,
            }
            self.queue_metadata["tlid_" + str(tlid)] = item

        self.broadcast(
            data={
                "method": "queue_metadata_changed",
                "params": {"queue_metadata": self.queue_metadata},
            }
        )

        response = {"message": "Added queue metadata"}
        if callback:
            callback(response)
        else:
            return response

    def clean_queue_metadata(self, *args, **kwargs):
        cleaned_queue_metadata = {}

        for tltrack in self.core.tracklist.get_tl_tracks().get():

            # if we have metadata for this track, push it through to cleaned
            # dictionary
            if "tlid_" + str(tltrack.tlid) in self.queue_metadata:
                cleaned_queue_metadata[
                    "tlid_" + str(tltrack.tlid)
                ] = self.queue_metadata["tlid_" + str(tltrack.tlid)]

        self.queue_metadata = cleaned_queue_metadata

    ##
    # Commands
    #
    # These are stored locally for all users to access
    ##

    def get_commands(self, *args, **kwargs):
        callback = kwargs.get("callback", False)

        response = {"commands": self.commands}
        if callback:
            callback(response)
        else:
            return response

    def set_commands(self, *args, **kwargs):
        callback = kwargs.get("callback", False)
        data = kwargs.get("data", {})

        # Update our temporary variable
        self.commands = data["commands"]

        # Save the new commands to file storage
        self.save_to_file(self.commands, "commands")

        self.broadcast(
            data={
                "method": "commands_changed",
                "params": {"commands": self.commands},
            }
        )

        response = {"message": "Commands saved"}
        if callback:
            callback(response)
        else:
            return response

    async def run_command(self, *args, **kwargs):
        callback = kwargs.get("callback", False)
        data = kwargs.get("data", {})
        error = False

        if str(data["id"]) not in self.commands:
            error = {
                "message": "Command failed",
                "description": "Could not find command by ID "
                + '"'
                + str(data["id"])
                + '"',
            }
        else:
            command = self.commands[str(data["id"])]
            if "method" not in command:
                error = {
                    "message": "Command failed",
                    "description": 'Missing required property "method"',
                }
            if "url" not in command:
                error = {
                    "message": "Command failed",
                    "description": 'Missing required property "url"',
                }

        logger.debug("Running command " + str(command))

        if error:
            if callback:
                callback(False, error)
                return
            else:
                return error

        # Build headers dict if additional headers are given
        headers = None
        if "additional_headers" in command:
            d = command["additional_headers"].split("\n")
            lines = list(filter(lambda x: x.find(":") > 0, d))
            fields = [
                (x.split(":", 1)[0].strip().lower(), x.split(":", 1)[1].strip())
                for x in lines
            ]
            headers = dict(fields)

        if command["method"] == "POST":
            if (
                "content-type" in headers
                and headers["content-type"].lower() != "application/json"
            ):
                post_data = command["post_data"]
            else:
                post_data = json.dumps(command["post_data"])
            request = HTTPRequest(
                command["url"],
                connect_timeout=5,
                method="POST",
                body=post_data,
                validate_cert=False,
                headers=headers,
            )
        else:
            request = HTTPRequest(
                command["url"],
                connect_timeout=5,
                validate_cert=False,
                headers=headers,
            )

        # Make the request, and handle any request errors
        try:
            http_client = AsyncHTTPClient()
            command_response = await http_client.fetch(request)
        except Exception as e:
            error = {"message": "Command failed", "description": str(e)}
            if callback:
                callback(False, error)
                return
            else:
                return error

        # Attempt to parse body as JSON
        try:
            command_response_body = json.loads(command_response.body)
        except BaseException:
            # Perhaps it requires unicode encoding?
            try:
                command_response_body = tornado.escape.to_unicode(
                    command_response.body
                )
            except BaseException:
                command_response_body = ""

        # Finally, return the result
        response = {"message": "Command run", "response": command_response_body}

        if callback:
            callback(response)
            return
        else:
            return response

    ##
    # Spotify authentication
    #
    # Uses the Client Credentials Flow, so is invisible to the user.
    # We need this token for any backend spotify requests (we don't tap in
    # to Mopidy-Spotify, yet). Also used for passing token to frontend for
    # javascript requests without use of the Authorization Code Flow.
    ##

    async def get_spotify_token(self, *args, **kwargs):
        callback = kwargs.get("callback", False)

        # Expired, so go get a new one
        if (
            not self.spotify_token
            or self.spotify_token["expires_at"] <= time.time()
        ):
            await self.refresh_spotify_token()

        response = {"spotify_token": self.spotify_token}

        if callback:
            callback(response)
        else:
            return response

    async def refresh_spotify_token(self, *args, **kwargs):
        callback = kwargs.get("callback", None)

        # Use client_id and client_secret from config
        # This was introduced in Mopidy-Spotify 3.1.0
        url = "https://auth.mopidy.com/spotify/token"
        data = {
            "client_id": self.config["spotify"]["client_id"],
            "client_secret": self.config["spotify"]["client_secret"],
            "grant_type": "client_credentials",
        }

        try:
            http_client = tornado.httpclient.AsyncHTTPClient()
            request = tornado.httpclient.HTTPRequest(
                url, method="POST", body=urllib.parse.urlencode(data)
            )
            response = await self.do_fetch(http_client, request)

            token = json.loads(response.body)
            token["expires_at"] = time.time() + token["expires_in"]
            self.spotify_token = token

            self.broadcast(
                data={
                    "method": "spotify_token_changed",
                    "params": {"spotify_token": self.spotify_token},
                }
            )

            response = {"spotify_token": token}
            if callback:
                callback(response)
            else:
                return response

        except (urllib.error.HTTPError, urllib.error.URLError) as e:
            error = json.loads(e.read())
            error = {
                "message": "Could not refresh token: "
                + error["error_description"]
            }

            if callback:
                callback(False, error)
            else:
                return error

    ##
    # Detect if we're running as root
    ##
    def is_root(self):
        if sys.platform == "win32":
            return ctypes.windll.shell32.IsUserAnAdmin() != 0
        else:
            return os.geteuid() == 0

    ##
    # Spotify authentication
    #
    # Uses the Client Credentials Flow, so is invisible to the user.
    # We need this token for any backend spotify requests (we don't tap in
    # to Mopidy-Spotify, yet). Also used for passing token to frontend for
    # javascript requests without use of the Authorization Code Flow.
    ##

    async def get_lyrics(self, *args, **kwargs):
        callback = kwargs.get("callback", False)
        request = kwargs.get("request", False)
        error = False
        url = ""

        try:
            path = request.get_argument("path")
            url = "https://genius.com" + path
        except Exception as e:
            logger.error(e)
            error = {"message": "Path not valid", "description": str(e)}

        try:
            connection_id = request.get_argument("connection_id")

            if connection_id not in self.connections:
                error = {
                    "message": "Unauthorized request",
                    "description": "Connection "
                    + connection_id
                    + " not connected",
                }

        except Exception as e:
            logger.error(e)
            error = {
                "message": "Unauthorized request",
                "description": "connection_id missing",
            }

        if error:
            return error

        try:
            http_client = AsyncHTTPClient()
            http_response = await http_client.fetch(url)
            callback(
                http_response.body.decode("utf-8", errors="replace"), False
            )

        except (urllib.error.HTTPError, urllib.error.URLError) as e:
            error = json.loads(e.read())
            error = {
                "message": "Could not fetch Spotify recommendations: "
                + error["error_description"]
            }
            logger.error(
                "Could not fetch Spotify recommendations: "
                + error["error_description"]
            )
            logger.debug(error)
            return error

    ##
    # Simple test method to debug access to system tasks
    ##
    def test(self, *args, **kwargs):
        callback = kwargs.get("callback", False)
        ioloop = kwargs.get("ioloop", False)

        self.broadcast(data={"method": "test_started"})

        response = {"message": "Running test... please wait"}

        if callback:
            callback(response)
        else:
            return response

        IrisSystemThread("test", ioloop, self.test_callback).run()

    def test_callback(self, response, error, update):
        if error:
            self.broadcast(data={"method": "test_error", "params": error})
        elif error:
            self.broadcast(data={"method": "test_updated", "params": update})
        else:
            self.broadcast(data={"method": "test_finished", "params": response})
