
from __future__ import unicode_literals

import random, string, logging, json, pykka, pylast, urllib, urllib2, os, sys, mopidy_iris, subprocess
import tornado.web
import tornado.websocket
import tornado.ioloop
from mopidy import config, ext
from mopidy.core import CoreListener
from pkg_resources import parse_version
from tornado.escape import json_encode, json_decode
from spotipy import Spotify

# import logger
logger = logging.getLogger(__name__)

class IrisCore(object):

    version = 0
    is_root = ( os.geteuid() == 0 )
    spotify_token = False
    queue_metadata = {}
    connections = {}
    initial_consume = False
    radio = {
        "enabled": 0,
        "seed_artists": [],
        "seed_genres": [],
        "seed_tracks": []
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
        return {"clientid": clientid, "connection_id": connection_id, "username": username, "generated": generated}


    def send_message(self, to, data):
        self.connections[to]['connection'].write_message( json_encode(data) )


    def broadcast(self, data):
        for connection in self.connections.itervalues():
            connection['connection'].write_message( json_encode(data) )
        return {
            'status': 1,
            'message': 'Broadcast to '+str(len(self.connections))+' connections'
        }
    
    ##
    # Connections
    #
    # Contains all our connections and client details. This requires updates
    # when new clients connect, and old ones disconnect. These events are broadcast
    # to all current connections
    ##

    def get_connections(self, data):        
        connections = []
        for connection in self.connections.itervalues():
            connections.append(connection['client'])
        
        return {
            'status': 1,
            'connections': connections
        }

    def add_connection(self, connection_id, connection, client):
        new_connection = {
            'client': client,
            'connection': connection
        }
        self.connections[connection_id] = new_connection

        self.send_message(
            connection_id, {
                'type': 'connected',
                'connection_id': connection_id,
                'username': client['username']
            }
        )

        self.broadcast({
            'type': 'connection_added',
            'connection': client
        })
    
    def remove_connection(self, connection_id):
        if connection_id in self.connections:
            try:
                client = self.connections[connection_id]['client']  
                del self.connections[connection_id]
                self.broadcast({
                    'type': 'connection_removed',
                    'connection': client
                })
            except:
                logger.error('Failed to close connection to '+ connection_id)           

    def set_username(self, data):
        connection_id = data['connection_id']
        if connection_id in self.connections:
            self.connections[connection_id]['client']['username'] = data['username']
            self.broadcast({
                'type': 'connection_updated',
                'connection': self.connections[connection_id]['client']
            })
            return {
                'status': 1,
                'connection_id': connection_id,
                'username': data['username']
            }

        else:
            error = 'Connection "'+data['connection_id']+'" not found'
            logger.error(error)
            return {
                'status': 0,
                'message': error
            }         

    def deliver_message(self, data):
        to = data['to']
        if to in self.connections:

            self.send_message(to, data['message'])

            return {
                'status': 1
            }

        else:
            error = 'Connection "'+data['connection_id']+'" not found'
            logger.error(error)
            return {
                'status': 0,
                'message': error
            }    
            



    ##
    # System controls
    #
    # Faciitates upgrades and configuration fetching
    ##  

    def get_config(self, data):

        # handle config setups where there is no username/password
        # Iris won't work properly anyway, but at least we won't get server errors
        if 'spotify' in self.config and 'username' in self.config['spotify']:
            spotify_username = self.config['spotify']['username']
        else:        
            spotify_username = False

        config = {
            "spotify_username": spotify_username,
            "country": self.config['iris']['country'],
            "locale": self.config['iris']['locale'],
            "authorization_url": self.config['iris']['authorization_url']
        }
        return {
            'config': config
        }

    def get_version(self, data):

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
        
        return {
            'status': 1,
            'version': {
                'current': self.version,
                'latest': latest_version,
                'is_root': self.is_root,
                'upgrade_available': upgrade_available
            }
        }

    def perform_upgrade( self ):
        try:
            subprocess.check_call(["pip", "install", "--upgrade", "Mopidy-Iris"])
            return True
        except subprocess.CalledProcessError:
            return False
        
    def restart( self ):
        os.execl(sys.executable, *([sys.executable]+sys.argv))


    ##
    # Spotify Radio
    #
    # Accepts seed URIs and creates radio-like experience. When our tracklist is nearly
    # empty, we fetch more recommendations. This can result in duplicates. We keep the
    # recommendations limit low to avoid timeouts and slow UI
    ##

    def get_radio(self, data):
        return {
            'status': 1,
            'radio': self.radio
        }

    def change_radio(self, data):

        # figure out if we're starting or updating radio mode
        if data['update'] and self.radio['enabled']:
            starting = False
            self.initial_consume = self.core.tracklist.get_consume()
        else:
            starting = True
        
        # fetch more tracks from Mopidy-Spotify
        self.radio = data
        self.radio['enabled'] = 1;
        uris = self.load_more_tracks()

        # make sure we got recommendations
        if uris:
            if starting:
                self.core.tracklist.clear()

            self.core.tracklist.set_consume(True)
            added = self.core.tracklist.add(uris = uris)

            if added.get():
                if starting:
                    self.core.playback.play()
                    self.broadcast({
                        'type': 'radio_started',
                        'radio': self.radio
                    })
                else:
                    self.broadcast({
                        'type': 'radio_changed',
                        'radio': self.radio
                    })

                return self.get_radio({})

        # failed fetching/adding tracks, so no-go
        self.radio['enabled'] = 0;
        return {
            'status': 0,
            'message': 'Could not start radio',
            'radio': self.radio
        }


    def stop_radio(self, data):

        self.radio = {
            "enabled": 0,
            "seed_artists": [],
            "seed_genres": [],
            "seed_tracks": []
        }

        # restore initial consume state
        self.core.tracklist.set_consume(self.initial_consume)
        self.core.playback.stop()        

        self.broadcast({
            'type': 'radio_stopped',
            'radio': self.radio
        })
        
        return {
            'status': 1
        }


    def load_more_tracks( self ):
        
        # this is crude, but it means we don't need to handle expired tokens
        # TODO: address this when it's clear what Jodal and the team want to do with Pyspotify
        self.refresh_spotify_token({})
        
        try:
            token = self.spotify_token
            token = token['access_token']
        except:
            logger.error('IrisFrontend: access_token missing or invalid')
            self.broadcast({
                'type': 'error',
                'message': 'Could not get radio tracks: access_token missing or invalid',
                'source': 'load_more_tracks'
            })
            
        try:
            spotify = Spotify( auth = token )
            response = spotify.recommendations(seed_artists = self.radio['seed_artists'], seed_genres = self.radio['seed_genres'], seed_tracks = self.radio['seed_tracks'], limit = 5)
            
            uris = []
            for track in response['tracks']:
                uris.append( track['uri'] )
            
            return uris
        except:
            logger.error('IrisFrontend: Failed to fetch Spotify recommendations')
            self.broadcast({
                'type': 'error',
                'message': 'Could not get radio tracks',
                'source': 'load_more_tracks'
            })
            return False


    def check_for_radio_update( self ):
        tracklistLength = self.core.tracklist.length.get()        
        if( tracklistLength <= 5 and self.radio['enabled'] == 1 ):
            
            uris = self.load_more_tracks()

            if not uris:
                self.broadcast({
                    'type': 'error',
                    'message': 'Could not fetch tracklist length',
                    'source': 'check_for_radio_update'
                })
                logger.warning('IrisFrontend: Could not fetch tracklist length')

            else:
                self.core.tracklist.add(uris = uris)
                


    ##
    # Additional queue metadata
    #
    # This maps tltracks with extra info for display in Iris, including
    # added_by and from_uri.
    ##

    def get_queue_metadata(self, data):
        return {
            'status': 1,
            'queue_metadata': self.queue_metadata
        }

    def add_queue_metadata(self, data):

        for tlid in data['tlids']:
            item = {
                'tlid': tlid,
                'added_from': data['added_from'],
                'added_by': data['added_by']
            }
            self.queue_metadata['tlid_'+str(tlid)] = item

        self.broadcast({
            'type': 'queue_metadata_changed',
            'queue_metadata': self.queue_metadata
        })

        return {
            'status': 1
        }

    def clean_queue_metadata( self ):
        cleaned_queue_metadata = {}

        for tltrack in self.core.tracklist.get_tl_tracks().get():

            # if we have metadata for this track, push it through to cleaned dictionary
            if 'tlid_'+str(tltrack.tlid) in self.queue_metadata:
                cleaned_queue_metadata['tlid_'+str(tltrack.tlid)] = self.queue_metadata['tlid_'+str(tltrack.tlid)]

        self.queue_metadata = cleaned_queue_metadata

        self.broadcast({
            'type': 'queue_metadata_changed',
            'queue_metadata': self.queue_metadata
        })

        return {
            'status': 1
        }


    ##
    # Spotify authentication
    #
    # Uses the Client Credentials Flow, so is invisible to the user. We need this token for
    # any backend spotify requests (we don't tap in to Mopidy-Spotify, yet). Also used for
    # passing token to frontend for javascript requests without use of the Authorization Code Flow.
    ##

    def get_spotify_token(self, data):
        return {
            'spotify_token': self.spotify_token
        }

    def refresh_spotify_token(self, data):

        ## TODO: See if we can use the credentials used by Mopidy-Spotify 3.1.0
        # Currently this returns 'invalid_client' error
        
        # Use client_id and client_secret from config
        # This was introduced in Mopidy-Spotify 3.1.0
        #if 'spotify' in self.config and 'client_id' in self.config['spotify']:
        #    authorization = str(self.config['spotify']['client_id']+':'+self.config['spotify']['client_secret'])
        #    authorization = authorization.encode('base64').replace('\n', '')

        # Mopidy-Spotify is old version, so just use basic authorization code
        #else:
        #    authorization = 'YTg3ZmI0ZGJlZDMwNDc1YjhjZWMzODUyM2RmZjUzZTI6ZDdjODlkMDc1M2VmNDA2OGJiYTE2NzhjNmNmMjZlZDY='

        authorization = 'YTg3ZmI0ZGJlZDMwNDc1YjhjZWMzODUyM2RmZjUzZTI6ZDdjODlkMDc1M2VmNDA2OGJiYTE2NzhjNmNmMjZlZDY='
        url = 'https://accounts.spotify.com/api/token'
        headers = {'Authorization' : 'Basic ' + authorization}
        data = {'grant_type': 'client_credentials'}

        data_encoded = urllib.urlencode( data )
        req = urllib2.Request(url, data_encoded, headers)

        try:
            response = urllib2.urlopen(req, timeout=30).read()
            response_dict = json.loads(response)
            self.spotify_token = response_dict

            self.broadcast({
                'type': 'spotify_token_changed',
                'spotify_token': self.spotify_token
            })

            return self.get_spotify_token({})

        except urllib2.HTTPError as e:
            error = json.loads(e.read())

            return {
                'type': 'error',
                'message': 'Could not refresh token: '+error['error'],
                'source': 'refresh_spotify_token'
            }
