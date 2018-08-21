
from __future__ import unicode_literals

import random, string, logging, json, pykka, pylast, urllib, urllib2, os, sys, mopidy_iris, subprocess
import tornado.web
import tornado.websocket
import tornado.ioloop
import tornado.httpclient
import requests
import time
from mopidy import config, ext
from mopidy.core import CoreListener
from pkg_resources import parse_version
from tornado.escape import json_encode, json_decode

import socket

if sys.platform == 'win32':
    import ctypes

# import logger
logger = logging.getLogger(__name__)

class IrisCore(object):
    version = 0
    spotify_token = False
    queue_metadata = {}
    connections = {}
    initial_consume = False
    radio = {
        "enabled": 0,
        "seed_artists": [],
        "seed_genres": [],
        "seed_tracks": [],
        "results": []
    }
    snapcast_listener = False


    ##
    # Create a new snapcast TCP connection
    #
    # @return socket
    ##
    def new_snapcast_socket(self):

        if not self.config['iris'].get('snapcast_enabled'):
            logger.error("Iris Snapcast not enabled")
            raise Exception("Snapcast not enabled")

        try:
            snapcast = socket.socket(socket.AF_INET, socket.SOCK_STREAM)
            snapcast.settimeout(10)
            snapcast.connect((self.config['iris']['snapcast_host'], self.config['iris']['snapcast_port']))
        except socket.gaierror, e:
            logger.error("Iris could not connect to Snapcast: %s" % e)
            raise Exception(e);
        except socket.error, e:
            logger.error("Iris could not connect to Snapcast: %s" % e)
            raise Exception(e);

        return snapcast


    ##
    # Create our ongoing notification listener
    #
    # TODO: Make non-blocking
    ##
    def create_snapcast_listener(self):
        try:
            listener = self.new_snapcast_socket()
            self.snapcast_listener = listener
            logger.info("Iris connected to Snapcast")
        except Exception, e:
            logger.error("Iris could not connect to Snapcast: %s" % e)


    ##
    # Disconnect our Snapcast listener
    ##
    def snapcast_disconnect_listener(self):
        if self.snapcast_listener:
            self.snapcast_listener.close()
            self.snapcast_listener = None


    ##
    # Handle a message from the Snapcast Telnet API
    # We create a connection for this request, and then drop it once completed. This is because
    # we have a separate socket for monitoring notifications
    #
    # @param data = string
    ##
    def snapcast_handle_message(self, data):
        logger.debug("Iris received Snapcast message: "+data)

        try:
            data = json.loads(data)
        except:
            logger.error("Iris could not digest Snapcast message: "+data)

        print "broadcasting..."
        print data

        self.broadcast(data)


    ##
    # Send a request to Snapcast
    #
    # We create a connection for this request, and then drop it once completed. This is because
    # we have a separate socket for monitoring notifications
    ##
    def snapcast_instruct(self, *args, **kwargs):
        callback = kwargs.get('callback', None)
        request_id = kwargs.get('request_id', None)
        data = kwargs.get('data', {})

        request = {
            'id': self.generateGuid(),
            'jsonrpc': '2.0',
            'method': data['method'],
            'params': data['params'] if 'params' in data else {}
        }

        # Convert to string. For some really nuts reason we need an extra trailing curly brace...
        request = json.dumps(request)+'}'

        # Create our connection
        try:
            snapcast_socket = self.new_snapcast_socket()
        except Exception, e:
            callback(response=None, error={
                'message': "Could not connect to Snapcast",
                'data': str(e)
            })
            return

        # Attempt to send the request
        try:
            snapcast_socket.send(request.encode('ascii')+b"\n")
        except socket.error, e:
            logger.error("Iris could not send request to Snapcast: %s" % e)
            callback(response=None, error={
                'message': "Failed to send request to Snapcast",
                'data': str(e)
            })
            return

        # Wait for response
        while True:
            try:
                response = snapcast_socket.recv(8192)
            except socket.error, e:
                logger.error("Iris failed to receive Snapcast response: %s" % e)
                callback(response=None, error={
                    'message': "Failed to receive Snapcast response",
                    'data': str(e)
                })

            snapcast_socket.close()

            if not len(response):
                break

            try:
                response = json.loads(response)
                if 'result' in response:
                    callback(response=response['result'])
                else:
                    callback(error=response['error'])
            except:
                logger.error("Iris received malformed Snapcast response: "+response)
                callback(response=None, error={
                    'message': "Malformed Snapcast response",
                    'data': response
                })
                return

            return




    ##
    # Generate a random string
    #
    # Used for connection_ids where none is provided by client
    # @return string
    ##
    def generateGuid(self):
        length = 12
        return ''.join(random.choice(string.lowercase) for i in range(length))


    ##
    # Digest a protocol header into it's id/name parts
    #
    # @return dict
    ##
    def digest_protocol(self, protocol):

        # if we're a string, split into list
        # this handles the different ways we get this passed (select_subprotocols gives string, headers.get gives list)
        if isinstance(protocol, basestring):

            # make sure we strip any spaces (IE gives "element,element", proper browsers give "element, element")
            protocol = [i.strip() for i in protocol.split(',')]

        # if we've been given a valid array
        try:
            client_id = protocol[0]
            connection_id = protocol[1]
            username = protocol[2]
            generated = False

        # invalid, so just create a default connection, and auto-generate an ID
        except:
            client_id = self.generateGuid()
            connection_id = self.generateGuid()
            username = 'Anonymous'
            generated = True

        # construct our protocol object, and return
        return {
            "client_id": client_id,
            "connection_id": connection_id,
            "username": username,
            "generated": generated
        }


    def send_message(self, *args, **kwargs):
        callback = kwargs.get('callback', None)
        data = kwargs.get('data', None)

        logger.debug(data)


        # Catch invalid recipient
        if data['recipient'] not in self.connections:
            error = 'Connection "'+data['recipient']+'" not found'
            logger.error(error)

            error = {
                'message': error
            }
            if (callback):
                callback(False, error)
            else:
                return error

        # Sending of an error
        if 'error' in data:
            message = {
                'jsonrpc': '2.0',
                'error': data['error']
            }

        # Sending of a regular message
        else:
            message = {
                'jsonrpc': '2.0',
                'method': data['method'] if 'method' in data else None
            }
            if 'id' in data:
                message['id'] = data['id']
            if 'params' in data:
                message['params'] = data['params']
            if 'result' in data:
                message['result'] = data['result']

        # Dispatch the message
        try:
            self.connections[data['recipient']]['connection'].write_message(json_encode(message))

            response = {
                'message': 'Sent message to '+data['recipient']
            }
            if (callback):
                callback(response)
            else:
                return response
        except:
            error = 'Failed to send message to '+ data['recipient']
            logger.error(error)

            error = {
                'message': error
            }
            if (callback):
                callback(False, error)
            else:
                return error


    def broadcast(self, *args, **kwargs):
        callback = kwargs.get('callback', None)
        data = kwargs.get('data', None)

        logger.debug(data)

        if 'error' in data:
            message = {
                'jsonrpc': '2.0',
                'error': data['error']
            }
        else:
            message = {
                'jsonrpc': '2.0',
                'method': data['method'] if 'method' in data else None,
                'params': data['params'] if 'params' in data else None
            }

        for connection in self.connections.itervalues():

            send_to_this_connection = True

            # Don't send the broadcast to the origin, naturally
            if 'connection_id' in data:
                if connection['connection_id'] == data["connection_id"]:
                    send_to_this_connection = False

            if send_to_this_connection:
                connection['connection'].write_message(json_encode(message))

        response = {
            'message': 'Broadcast to '+str(len(self.connections))+' connections'
        }
        if (callback):
            callback(response)
        else:
            return response


    ##
    # Connections
    #
    # Contains all our connections and client details. This requires updates
    # when new clients connect, and old ones disconnect. These events are broadcast
    # to all current connections
    ##

    def get_connections(self, *args, **kwargs):
        callback = kwargs.get('callback', None)

        connections = []
        for connection in self.connections.itervalues():
            connections.append(connection['client'])

        response = {
            'connections': connections
        }
        if (callback):
            callback(response)
        else:
            return response

    def add_connection(self, *args, **kwargs):
        connection_id = kwargs.get('connection_id', None)
        connection = kwargs.get('connection', None)
        client = kwargs.get('client', None)

        new_connection = {
            'client': client,
            'connection_id': connection_id,
            'connection': connection
        }
        self.connections[connection_id] = new_connection

        self.send_message(data={
            'recipient': connection_id,
            'method': 'connection_added',
            'params': {
                'connection': {
                    'connection_id': connection_id,
                    'client_id': client['client_id'],
                    'username': client['username'],
                    'ip': client['ip']
                }
            }
        })

        self.broadcast(data={
            'method': 'connection_added',
            'params': {
                'connection': client
            }
        })

    def remove_connection(self, connection_id):
        if connection_id in self.connections:
            try:
                client = self.connections[connection_id]['client']
                del self.connections[connection_id]
                self.broadcast(data={
                    'method': "connection_removed",
                    'params': {
                        'connection': client
                    }
                })
            except:
                logger.error('Failed to close connection to '+ connection_id)

    def set_username(self, *args, **kwargs):
        callback = kwargs.get('callback', None)
        data = kwargs.get('data', {})
        connection_id = data['connection_id']

        if connection_id in self.connections:
            self.connections[connection_id]['client']['username'] = data['username']
            self.broadcast(data={
                'method': "connection_changed",
                'params': {
                    'connection': self.connections[connection_id]['client']
                }
            })
            response = {
                'connection_id': connection_id,
                'username': data['username']
            }
            if (callback):
                callback(response)
            else:
                return response

        else:
            error = 'Connection "'+data['connection_id']+'" not found'
            logger.error(error)

            error = {
                'message': error
            }
            if (callback):
                callback(False, error)
            else:
                return error




    ##
    # System controls
    #
    # Faciitates upgrades and configuration fetching
    ##

    def get_config(self, *args, **kwargs):
        callback = kwargs.get('callback', False)

        # handle config setups where there is no username/password
        # Iris won't work properly anyway, but at least we won't get server errors
        if 'spotify' in self.config and 'username' in self.config['spotify']:
            spotify_username = self.config['spotify']['username']
        else:
            spotify_username = False

        response = {
            'config': {
                "is_root": self.is_root(),
                "spotify_username": spotify_username,
                "country": self.config['iris']['country'],
                "locale": self.config['iris']['locale'],
                "spotify_authorization_url": self.config['iris']['spotify_authorization_url'],
                "lastfm_authorization_url": self.config['iris']['lastfm_authorization_url'],
                "genius_authorization_url": self.config['iris']['genius_authorization_url'],
                "snapcast_enabled": self.config['iris']['snapcast_enabled']
            }
        }

        if (callback):
            callback(response)
        else:
            return response


    def get_version(self, *args, **kwargs):
        callback = kwargs.get('callback', False)
        url = 'https://pypi.python.org/pypi/Mopidy-Iris/json'
        req = urllib2.Request(url)

        try:
            response = urllib2.urlopen(req, timeout=30).read()
            response = json.loads(response)
            latest_version = response['info']['version']

            # compare our versions, and convert result to boolean
            upgrade_available = cmp( parse_version( latest_version ), parse_version( self.version ) )
            upgrade_available = ( upgrade_available == 1 )

        except urllib2.HTTPError as e:
            latest_version = '0.0.0'
            upgrade_available = False

        response = {
            'version': {
                'current': self.version,
                'latest': latest_version,
                'is_root': self.is_root(),
                'upgrade_available': upgrade_available
            }
        }
        if (callback):
            callback(response)
        else:
            return response


    def upgrade(self, *args, **kwargs):
        logger.info("Upgrading")

        callback = kwargs.get('callback', False)

        try:
            self.check_system_access()
        except Exception, e:
            logger.error(e)

            error = {
                'message': "Permission denied",
                'description': str(e)
            }

            if (callback):
                callback(False, error)
                return
            else:
                return error

        self.broadcast(data={
            'method': "upgrade_started",
            'params': {}
        });

        # Run the system task
        path = os.path.dirname(__file__)

        # Attempt the upgrade
        upgrade_process = subprocess.Popen("sudo "+path+"/system.sh upgrade", stdout=subprocess.PIPE, stderr=subprocess.PIPE, shell=True)
        result, error = upgrade_process.communicate()
        exitCode = upgrade_process.wait()

        if exitCode > 0:
            output = error.decode('ascii')
            logger.error("Upgrade failed: "+output)

            error = {
                'message': "Upgrade failed",
                'data': {
                    'output': output
                }
            }

            if (callback):
                callback(False, error)
                return
            else:
                return error

        self.broadcast(data={
            'method': "upgrade_complete",
            'params': {}
        });

        output = result.decode();

        response = {
            'message': "Restart required to complete upgrade",
            'data': {
                'output': output
            }
        }

        if (callback):
            callback(response)
        else:
            return response


    def restart(self, *args, **kwargs):
        logger.info("Restarting")
        callback = kwargs.get('callback', False)

        try:
            self.check_system_access()
        except Exception, e:
            logger.error(e)

            error = {
                'message': "Permission denied",
                'description': str(e)
            }

            if (callback):
                callback(False, error)
                return
            else:
                return error


        self.broadcast(data={
            'method': "restarting",
            'params': {}
        })

        path = os.path.dirname(__file__)

        subprocess.Popen(["sudo "+path+"/system.sh restart 0"], shell=True)

        response = {
            'message': "Restarting... please wait"
        }
        if (callback):
            callback(response)
        else:
            return response


    ##
    # Spotify Radio
    #
    # Accepts seed URIs and creates radio-like experience. When our tracklist is nearly
    # empty, we fetch more recommendations. This can result in duplicates. We keep the
    # recommendations limit low to avoid timeouts and slow UI
    ##

    def get_radio(self, *args, **kwargs):
        callback = kwargs.get('callback', False)

        response = {
            'radio': self.radio
        }
        if (callback):
            callback(response)
        else:
            return response

    def change_radio(self, *args, **kwargs):
        callback = kwargs.get('callback', False)
        data = kwargs.get('data', {})

        # We're starting a new radio (or forced restart)
        if data['reset'] or not self.radio['enabled']:
            starting = True
            self.initial_consume = self.core.tracklist.get_consume().get()
        else:
            starting = False

        # fetch more tracks from Mopidy-Spotify
        self.radio = {
            'seed_artists': data['seed_artists'],
            'seed_genres': data['seed_genres'],
            'seed_tracks': data['seed_tracks'],
            'enabled': 1,
            'results': []
        }
        uris = self.load_more_tracks()

        # make sure we got recommendations
        if uris:
            if starting:
                self.core.tracklist.clear()

            self.core.tracklist.set_consume(True)

            # We only want to play the first batch
            added = self.core.tracklist.add(uris = uris[0:3])

            if (not added.get()):
                logger.error("No recommendations added to queue")

                self.radio['enabled'] = 0;
                error = {
                    'message': 'No recommendations added to queue',
                    'radio': self.radio
                }
                if (callback):
                    callback(False, error)
                else:
                    return error

            # Save results (minus first batch) for later use
            self.radio['results'] = uris[3:]

            if starting:
                self.core.playback.play()
                self.broadcast(data={
                    'method': "radio_started",
                    'params': {
                        'radio': self.radio
                    }
                })
            else:
                self.broadcast(data={
                    'method': "radio_changed",
                    'params': {
                        'radio': self.radio
                    }
                })

            self.get_radio(callback=callback)
            return

        # Failed fetching/adding tracks, so no-go
        else:
            logger.error("No recommendations returned by Spotify")
            self.radio['enabled'] = 0;
            error = {
                'code': 32500,
                'message': 'Could not start radio',
                'data': {
                    'radio': self.radio
                }
            }
            if (callback):
                callback(False, error)
            else:
                return error


    def stop_radio(self, *args, **kwargs):
        callback = kwargs.get('callback', False)

        self.radio = {
            "enabled": 0,
            "seed_artists": [],
            "seed_genres": [],
            "seed_tracks": [],
            "results": []
        }

        # restore initial consume state
        self.core.tracklist.set_consume(self.initial_consume)
        self.core.playback.stop()

        self.broadcast(data={
            'method': "radio_stopped",
            'params': {
                'radio': self.radio
            }
        })

        response = {
            'message': 'Stopped radio'
        }
        if (callback):
            callback(response)
        else:
            return response


    def load_more_tracks(self, *args, **kwargs):

        try:
            self.get_spotify_token()
            spotify_token = self.spotify_token
            access_token = spotify_token['access_token']
        except:
            error = 'IrisFrontend: access_token missing or invalid'
            logger.error(error)
            return False

        try:
            url = 'https://api.spotify.com/v1/recommendations/'
            url = url+'?seed_artists='+(",".join(self.radio['seed_artists'])).replace('spotify:artist:','')
            url = url+'&seed_genres='+(",".join(self.radio['seed_genres'])).replace('spotify:genre:','')
            url = url+'&seed_tracks='+(",".join(self.radio['seed_tracks'])).replace('spotify:track:','')
            url = url+'&limit=50'

            req = urllib2.Request(url)
            req.add_header('Authorization', 'Bearer '+access_token)

            response = urllib2.urlopen(req, timeout=30).read()
            response_dict = json.loads(response)

            uris = []
            for track in response_dict['tracks']:
                uris.append( track['uri'] )

            return uris

        except:
            logger.error('IrisFrontend: Failed to fetch Spotify recommendations')
            return False


    def check_for_radio_update( self ):
        tracklistLength = self.core.tracklist.length.get()
        if (tracklistLength < 3 and self.radio['enabled'] == 1):

            # Grab our loaded tracks
            uris = self.radio['results']

            # We've run out of pre-fetched tracks, so we need to get more recommendations
            if (len(uris) < 3):
                uris = self.load_more_tracks()

            # Remove the next batch, and update our results
            self.radio['results'] = uris[3:]

            # Only add the next set of uris
            uris = uris[0:3]

            self.core.tracklist.add(uris = uris)



    ##
    # Additional queue metadata
    #
    # This maps tltracks with extra info for display in Iris, including
    # added_by and from_uri.
    ##

    def get_queue_metadata(self, *args, **kwargs):
        callback = kwargs.get('callback', False)

        response = {
            'queue_metadata': self.queue_metadata
        }
        if (callback):
            callback(response)
        else:
            return response

    def add_queue_metadata(self, *args, **kwargs):
        callback = kwargs.get('callback', False)
        data = kwargs.get('data', {})

        for tlid in data['tlids']:
            item = {
                'tlid': tlid,
                'added_from': data['added_from'] if 'added_from' in data else None,
                'added_by': data['added_by'] if 'added_by' in data else None
            }
            self.queue_metadata['tlid_'+str(tlid)] = item

        self.broadcast(data={
            'method': 'queue_metadata_changed',
            'params': {
                'queue_metadata': self.queue_metadata
            }
        })

        response = {
            'message': 'Added queue metadata'
        }
        if (callback):
            callback(response)
        else:
            return response

    def clean_queue_metadata(self, *args, **kwargs):
        callback = kwargs.get('callback', False)
        cleaned_queue_metadata = {}

        for tltrack in self.core.tracklist.get_tl_tracks().get():

            # if we have metadata for this track, push it through to cleaned dictionary
            if 'tlid_'+str(tltrack.tlid) in self.queue_metadata:
                cleaned_queue_metadata['tlid_'+str(tltrack.tlid)] = self.queue_metadata['tlid_'+str(tltrack.tlid)]

        self.queue_metadata = cleaned_queue_metadata


    ##
    # Spotify authentication
    #
    # Uses the Client Credentials Flow, so is invisible to the user. We need this token for
    # any backend spotify requests (we don't tap in to Mopidy-Spotify, yet). Also used for
    # passing token to frontend for javascript requests without use of the Authorization Code Flow.
    ##

    def get_spotify_token(self, *args, **kwargs):
        callback = kwargs.get('callback', False)

        # Expired, so go get a new one
        if (not self.spotify_token or self.spotify_token['expires_at'] <= time.time()):
            self.refresh_spotify_token()

        response = {
            'spotify_token': self.spotify_token
        }

        if (callback):
            callback(response)
        else:
            return response

    def refresh_spotify_token(self, *args, **kwargs):
        callback = kwargs.get('callback', None)

        # Use client_id and client_secret from config
        # This was introduced in Mopidy-Spotify 3.1.0
        url = 'https://auth.mopidy.com/spotify/token'
        data = {
            'client_id': self.config['spotify']['client_id'],
            'client_secret': self.config['spotify']['client_secret'],
            'grant_type': 'client_credentials'
        }

        try:
            http_client = tornado.httpclient.HTTPClient()
            request = tornado.httpclient.HTTPRequest(url, method='POST', body=urllib.urlencode(data))
            response = http_client.fetch(request)

            token = json.loads(response.body)
            token['expires_at'] = time.time() + token['expires_in']
            self.spotify_token = token

            self.broadcast(data={
                'method': 'spotify_token_changed',
                'params': {
                    'spotify_token': self.spotify_token
                }
            })

            response = {
                'spotify_token': token
            }
            if (callback):
                callback(response)
            else:
                return response

        except urllib2.HTTPError as e:
            error = json.loads(e.read())
            error = {'message': 'Could not refresh token: '+error['error_description']}

            if (callback):
                callback(False, error)
            else:
                return error


    ##
    # Detect if we're running as root
    ##
    def is_root(self):
        if sys.platform == 'win32':
            return ctypes.windll.shell32.IsUserAnAdmin() != 0
        else:
            return os.geteuid() == 0


    ##
    # Check if we have access to the system script (system.sh)
    #
    # @return boolean or exception
    ##
    def check_system_access(self, *args, **kwargs):
        callback = kwargs.get('callback', None)

        # Run the system task
        path = os.path.dirname(__file__)

        # Attempt the upgrade
        process = subprocess.Popen("sudo -n "+path+"/system.sh", stdout=subprocess.PIPE, stderr=subprocess.PIPE, shell=True)
        result, error = process.communicate()
        exitCode = process.wait()

        if exitCode > 0:
            raise Exception("Password-less access to "+path+"/system.sh was refused. Check your /etc/sudoers file.")
        else:
            return True


    ##
    # Spotify authentication
    #
    # Uses the Client Credentials Flow, so is invisible to the user. We need this token for
    # any backend spotify requests (we don't tap in to Mopidy-Spotify, yet). Also used for
    # passing token to frontend for javascript requests without use of the Authorization Code Flow.
    ##

    def get_lyrics(self, *args, **kwargs):
        callback = kwargs.get('callback', False)
        request = kwargs.get('request', False)
        error = False
        url = ""

        try:
            path = request.get_argument('path')
            url = 'https://genius.com'+path
        except Exception, e:
            logger.error(e)
            error = {
                'message': "Path not valid",
                'description': str(e)
            }

        try:
            connection_id = request.get_argument('connection_id')

            if connection_id not in self.connections:
                error = {
                    'message': 'Unauthorized request',
                    'description': 'Connection '+connection_id+' not connected'
                }

        except Exception, e:
            logger.error(e)
            error = {
                'message': "Unauthorized request",
                'description': "connection_id missing"
            }

        if error:
            if (callback):
                callback(False, error)
                return
            else:
                return error

        http_request = tornado.httpclient.HTTPRequest(url)
        http_client = tornado.httpclient.HTTPClient()
        http_client.fetch(http_request, callback=callback)


    ##
    # Simple test method. Not for use in production for any purposes.
    ##
    def test(self, *args, **kwargs):
        callback = kwargs.get('callback', None)

        try:
            self.check_system_access()
        except Exception, e:
            logger.error(e)

            error = {
                'message': "Permission denied",
                'description': str(e)
            }

            if (callback):
                callback(False, error)
                return
            else:
                return error

        response = {
            'message': "Permission granted"
        }

        if (callback):
            callback(response)
            return
        else:
            return response
