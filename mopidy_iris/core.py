
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
    radio = {
        "enabled": 0,
        "seed_artists": [],
        "seed_genres": [],
        "seed_tracks": []
    }


    def on_start(self):        
        logger.info('--- Starting Iris core '+self.version)


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

    ##
    # Send a message to an individual connection
    #
    # @param to = recipient's connection_id
    # @param data = array (any data required to include in our message)
    ##
    def send_message(self, to, data):
        self.connections[to]['connection'].write_message( json_encode(data) )


    def broadcast(self, data):
        for connection in self.connections.itervalues():
            connection['connection'].write_message( json_encode(data) )
        return {}
    
    ##
    # Add a new connection
    ##
    def add_connection(self, connection_id, connection, client):
        new_connection = {
            'client': client,
            'connection': connection
        }
        self.connections[connection_id] = new_connection

        self.broadcast({
            'action': 'client_connected',
            'client': client
        })
    
    ##
    # Add a new connection
    ##
    def remove_connection(self, connection_id):
        if connection_id in self.connections:
            try:
                del self.connections[connection_id]
                self.broadcast(self.get_connections())
            except:
                print 'Failed to close connection to '+ connection_id              
            
            self.broadcast({
                'action': 'client_disconnected',
                'client': client
            })
  



    def get_config(self, data):
        config = {
            "spotify_username": self.config['spotify']['username'],
            "country": self.config['iris']['country'],
            "locale": self.config['iris']['locale']
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
            'version': {
                'current': self.version,
                'latest': latest_version,
                'is_root': self.is_root,
                'upgrade_available': upgrade_available
            }
        }

    def get_connections(self, data):        
        connections = []
        for connection in self.connections.itervalues():
            connections.append(connection['client'])
        
        return {
            'connections': connections
        }

    def get_radio(self, data):
        return {
            'radio': self.radio
        }

    def stop_radio(self, data):

        self.radio = {
            "enabled": 0,
            "seed_artists": [],
            "seed_genres": [],
            "seed_tracks": []
        }
        
        self.core.playback.stop()

        self.broadcast({
            'action': 'radio_stopped',
            'radio': self.radio
        })
        
        return {}

    def get_queue_metadata(self, data):
        return {
            'queue_metadata': self.queue_metadata
        }

