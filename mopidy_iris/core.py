
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

if sys.platform == 'win32':
    import ctypes

# import logger
logger = logging.getLogger(__name__)

class IrisCore(object):

    version = 0
    if sys.platform == 'win32':
        is_root = ctypes.windll.shell32.IsUserAnAdmin() != 0
    else:
       is_root = os.geteuid() == 0
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


    ##
    # Generate a random string
    #
    # Used for connection_ids where none is provided by client
    # @return string
    ##
    def generateGuid(self, length):
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
            clientid = protocol[0]
            connection_id = protocol[1]
            username = protocol[2]
            generated = False
          
        # invalid, so just create a default connection, and auto-generate an ID
        except:
            clientid = self.generateGuid(12)
            connection_id = self.generateGuid(12)
            username = 'Anonymous'
            generated = True
        
        # construct our protocol object, and return
        return {
            "clientid": clientid,
            "connection_id": connection_id,
            "username": username,
            "generated": generated
        }


    def send_message(self, *args, **kwargs):        
        connection_id = kwargs.get('connection_id', None)
        data = kwargs.get('data', {})

        try:
            self.connections[connection_id]['connection'].write_message( json_encode(data) )
        except:
            logger.error('Failed to send message to '+ connection_id)


    def broadcast(self, *args, **kwargs):
        data = kwargs.get('data', {})
        callback = kwargs.get('callback', None)

        for connection in self.connections.itervalues():
            connection['connection'].write_message( json_encode(data) )

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
            'connection': connection
        }
        self.connections[connection_id] = new_connection

        self.send_message(
            connection_id=connection_id,
            data={
                'type': 'connected',
                'connection_id': connection_id,
                'username': client['username']
            }
        )

        self.broadcast(
            data={
                'type': 'connection_added',
                'connection': client
            }
        )
    
    def remove_connection(self, connection_id):
        if connection_id in self.connections:
            try:
                client = self.connections[connection_id]['client']  
                del self.connections[connection_id]
                self.broadcast(
                    data={
                        'type': 'connection_removed',
                        'connection': client
                    }
                )
            except:
                logger.error('Failed to close connection to '+ connection_id)           

    def set_username(self, *args, **kwargs):
        callback = kwargs.get('callback', None)
        data = kwargs.get('data', {})
        connection_id = data['connection_id']

        if connection_id in self.connections:
            self.connections[connection_id]['client']['username'] = data['username']
            self.broadcast(
                data={
                    'type': 'connection_updated',
                    'connection': self.connections[connection_id]['client']
                }
            )
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

    def deliver_message(self, *args, **kwargs):
        callback = kwargs.get('callback', False)
        data = kwargs.get('data', {})

        if data['connection_id'] in self.connections:
            self.send_message(connection_id=data['connection_id'], data=data['message'])
            response = {
                'message': 'Sent message to '+data['connection_id']
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
                "spotify_username": spotify_username,
                "country": self.config['iris']['country'],
                "locale": self.config['iris']['locale'],
                "spotify_authorization_url": self.config['iris']['spotify_authorization_url'],
                "lastfm_authorization_url": self.config['iris']['lastfm_authorization_url']
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
                'is_root': self.is_root,
                'upgrade_available': upgrade_available
            }
        }
        if (callback):
            callback(response)
        else:
            return response

    def perform_upgrade(self, *args, **kwargs):
        callback = kwargs.get('callback', False)

        try:
            subprocess.check_call(["pip", "install", "--upgrade", "Mopidy-Iris"])
            response = {
                'message': "Upgrade started"
            }
            if (callback):
                callback(response)
            else:
                return response

        except subprocess.CalledProcessError as e:
            error = {
                'message': "Could not start upgrade"
            }
            if (callback):
                callback(False, error)
            else:
                return error
        
    def restart(self, *args, **kwargs):
        os.execl(sys.executable, *([sys.executable]+sys.argv))


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

        # figure out if we're starting or updating radio mode
        if data['update'] and self.radio['enabled']:
            starting = False
            self.initial_consume = self.core.tracklist.get_consume()
        else:
            starting = True
        
        # fetch more tracks from Mopidy-Spotify
        self.radio = data
        self.radio['enabled'] = 1;
        self.radio['results'] = [];
        uris = self.load_more_tracks()

        # make sure we got recommendations
        if uris:
            if starting:
                self.core.tracklist.clear()

            self.core.tracklist.set_consume(True)

            # We only want to play the first batch
            added = self.core.tracklist.add(uris = uris[0:3])

            # Save results (minus first batch) for later use
            self.radio['results'] = uris[3:]

            if added.get():
                if starting:
                    self.core.playback.play()
                    self.broadcast(
                        data={
                            'type': 'radio_started',
                            'radio': self.radio
                        }
                    )
                else:
                    self.broadcast(
                        data={
                            'type': 'radio_changed',
                            'radio': self.radio
                        }
                    )

                self.get_radio(callback=callback)
                return
        
        # failed fetching/adding tracks, so no-go
        self.radio['enabled'] = 0;
        error = {
            'message': 'Could not start radio',
            'radio': self.radio
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

        self.broadcast(
            data={
                'type': 'radio_stopped',
                'radio': self.radio
            }
        )
        
        response = {
            'message': 'Stopped radio'
        }
        if (callback):
            callback(response)
        else:
            return response


    def load_more_tracks(self, *args, **kwargs):
        
        # this is crude, but it means we don't need to handle expired tokens
        # TODO: address this when it's clear what Jodal and the team want to do with Pyspotify
        self.refresh_spotify_token()
        
        try:
            token = self.spotify_token
            token = token['access_token']
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
            req.add_header('Authorization', 'Bearer '+self.spotify_token['access_token'])

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
                'added_from': data['added_from'],
                'added_by': data['added_by']
            }
            self.queue_metadata['tlid_'+str(tlid)] = item

        self.broadcast(
            data={
                'type': 'queue_metadata_changed',
                'queue_metadata': self.queue_metadata
            }
        )
        
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

        self.broadcast(
            data={
                'type': 'queue_metadata_changed',
                'queue_metadata': self.queue_metadata
            }
        )
        
        response = {
            'message': 'Cleaned queue metadata'
        }
        if (callback):
            callback(response)
        else:
            return response


    ##
    # Spotify authentication
    #
    # Uses the Client Credentials Flow, so is invisible to the user. We need this token for
    # any backend spotify requests (we don't tap in to Mopidy-Spotify, yet). Also used for
    # passing token to frontend for javascript requests without use of the Authorization Code Flow.
    ##

    def get_spotify_token(self, *args, **kwargs):
        callback = kwargs.get('callback', False)
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

            self.broadcast(
                data={
                    'type': 'spotify_token_changed',
                    'spotify_token': self.spotify_token
                }
            )

            token = json.loads(response.body)
            self.spotify_token = token
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
    # Proxy a request to an external provider
    #
    # This is required when requesting to non-CORS providers. We simply make the request
    # server-side and pass that back. All we change is the response's Access-Control-Allow-Origin
    # to prevent CORS-blocking by the browser.
    ##

    def proxy_request(self, *args, **kwargs):
        callback = kwargs.get('callback', False)
        origin_request = kwargs.get('request', None)
        
        try:
            data = kwargs.get('data', {})
        except:
            callback(False, {
                'message': 'Malformed data',
                'source': 'proxy_request'
            })
            return

        # Our request includes data, so make sure we POST the data
        if 'url' not in data:
            callback(False, {
                'message': 'Malformed data (missing URL)',
                'source': 'proxy_request',
                'original_request': data
            })
            return

        # Construct request headers
        # If we have an original request, pass through it's headers
        if origin_request:
            headers = origin_request.headers
        else:
            headers = {}

        # Adjust headers
        headers["Accept-Language"] = "*" 
        headers["Accept-Encoding"] = "deflate" 
        if "Content-Type" in headers:
            del headers["Content-Type"]
        if "Host" in headers:
            del headers["Host"]
        if "X-Requested-With" in headers:
            del headers["X-Requested-With"]
        if "X-Forwarded-Server" in headers:
            del headers["X-Forwarded-Server"]
        if "X-Forwarded-Host" in headers:
            del headers["X-Forwarded-Host"]
        if "X-Forwarded-For" in headers:
            del headers["X-Forwarded-For"]
        if "Referrer" in headers:
            del headers["Referrer"]

        # Our request includes data, so make sure we POST the data
        if ('data' in data and data['data']):
            http_client = tornado.httpclient.AsyncHTTPClient()
            request = tornado.httpclient.HTTPRequest(data['url'], method='POST', body=json.dumps(data['data']), headers=headers, validate_cert=False)
            http_client.fetch(request, callback=callback)

        # No data, so just a simple GET request
        else:

            # Strip out our origin content-length otherwise this confuses
            # the target server as content-length doesn't apply to GET requests
            if "Content-Length" in headers:
                del headers["Content-Length"]

            http_client = tornado.httpclient.AsyncHTTPClient()
            request = tornado.httpclient.HTTPRequest(data['url'], headers=headers, validate_cert=False)
            http_client.fetch(request, callback=callback)


    ##
    # Simple test method
    ##
    def test(self, *args, **kwargs):
        callback = kwargs.get('callback', None)
        data = kwargs.get('data', {})

        if data and 'force_error' in data:
            callback(False, {'message': "Could not sleep, forced error"})
            return
        else:
            time.sleep(1)
            callback({'message': "Slept for one second"}, False)
            return
