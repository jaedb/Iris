from __future__ import unicode_literals

import logging, json, pykka, pylast, pusher, urllib, urllib2, os, sys, mopidy_spotmop, subprocess
import tornado.web
import tornado.websocket
import tornado.ioloop
from mopidy import config, ext
from mopidy.core import CoreListener
from pkg_resources import parse_version
from spotipy import Spotify

# import logger
logger = logging.getLogger(__name__)

    
###
# Spotmop supporting frontend
#
# This provides a wrapping thread for the Pusher websocket, as well as the radio infrastructure
##
class IrisFrontend(pykka.ThreadingActor, CoreListener):

    def __init__(self, config, core):
        global spotmop
        super(IrisFrontend, self).__init__()
        self.config = config
        self.core = core
        self.version = mopidy_spotmop.__version__
        self.is_root = ( os.geteuid() == 0 )
        self.spotify_token = False
        self.radio = {
            "enabled": 0,
            "seed_artists": [],
            "seed_genres": [],
            "seed_tracks": []
        }

    def on_start(self):
        
        logger.info('Starting Iris '+self.version)
        
        # try and start a pusher server
        port = str(self.config['iris']['pusherport'])
        try:
            self.pusher = tornado.web.Application([( '/pusher', pusher.PusherWebsocketHandler, { 'frontend': self } )])
            self.pusher.listen(port)
            logger.info('Pusher server running at [0.0.0.0]:'+port)
            
        except( pylast.NetworkError, pylast.MalformedResponseError, pylast.WSError ) as e:
            logger.error('Error starting Pusher: %s', e)
            self.stop()
        
        # get a fresh spotify authentication token and store for future use
        # self.refresh_spotify_token()
    
    ##
    # Listen for core events, and update our frontend as required
    ##
    def track_playback_ended( self, tl_track, time_position ):
        self.check_for_radio_update()        
        
        
    ##
    # See if we need to perform updates to our radio
    #
    # We see if we've got one or two tracks left, if so, go get some more
    ##
    def check_for_radio_update( self ):
        try:
            tracklistLength = self.core.tracklist.length.get()        
            if( tracklistLength <= 5 and self.radio['enabled'] == 1 ):
                self.load_more_tracks()
                
        except RuntimeError:
            logger.warning('RadioHandler: Could not fetch tracklist length')
            pass


    ##
    # Load some more radio tracks
    #
    # We need to build a Spotify authentication token first, and then fetch recommendations
    ##
    def load_more_tracks( self ):
        
        # this is crude, but it means we don't need to handle expired tokens
        # TODO: address this when it's clear what Jodal and the team want to do with Pyspotify
        self.refresh_spotify_token()
        
        try:
            token = self.spotify_token
            token = token['access_token']
        except:
            logger.error('IrisFrontend: access_token missing or invalid')
            
        try:
            spotify = Spotify( auth = token )
            response = spotify.recommendations(seed_artists = self.radio['seed_artists'], seed_genres = self.radio['seed_genres'], seed_tracks = self.radio['seed_tracks'], limit = 5)
            
            uris = []
            for track in response['tracks']:
                uris.append( track['uri'] )
            
            self.core.tracklist.add( uris = uris )
        except:
            logger.error('IrisFrontend: Failed to fetch recommendations from Spotify')
            
    
    ##
    # Start radio
    #
    # Take the provided radio details, and start a new radio process
    ##
    def start_radio( self, new_state ):
        
        # TODO: validate payload has the required seed values
        
        # set our new radio state
        self.radio = new_state
        self.radio['enabled'] = 1;
        
        # clear all tracks
        self.core.tracklist.clear()
        
        # explicitly set consume, to ensure we don't end up with a huge tracklist (and it's how a radio should 'feel')
        self.core.tracklist.set_consume( True )
        
        # load me some tracks, and start playing!
        self.load_more_tracks()
        self.core.playback.play()
        
        # notify clients
        pusher.broadcast( 'radio_started', { 'radio': self.radio })
        
        # return new radio state to initial call
        return self.radio
        
    ##
    # Stop radio
    ##
    def stop_radio( self ):
        
        # reset radio
        self.radio = {
            "enabled": 0,
            "seed_artists": [],
            "seed_genres": [],
            "seed_tracks": []
        }
        
        # stop track playback
        self.core.playback.stop()

        # notify clients
        pusher.broadcast( 'radio_stopped', { 'radio': self.radio })
        
        # return new radio state to initial call
        return self.radio
        
   
    # get our spotify token
    def get_spotify_token( self ):
        return self.spotify_token
        
        
    ##
    # Get Spotmop version, and check for updates
    #
    # We compare our version with the latest available on PyPi
    ##
    def get_version( self ):
        
        url = 'https://pypi.python.org/pypi/Mopidy-Spotmop/json'
        req = urllib2.Request(url)
        
        try:
            response = urllib2.urlopen(req, timeout=30).read()
            response = json.loads(response)
            latest_version = response['info']['version']
        except urllib2.HTTPError as e:
            latest_version = False
        
        # compare our versions, and convert result to boolean
        upgrade_available = cmp( parse_version( latest_version ), parse_version( self.version ) )
        upgrade_available = ( upgrade_available == 1 )
        
        # prepare our response
        data = {
            'current': self.version,
            'latest': latest_version,
            'is_root': self.is_root,
            'upgrade_available': upgrade_available
        }
        return data
        
        
    ##
    # Upgrade Spotmop module
    #
    # Upgrade myself to the latest version available on PyPi
    ##
    def perform_upgrade( self ):
        try:
            subprocess.check_call(["pip", "install", "--upgrade", "Mopidy-Iris"])
            return True
        except subprocess.CalledProcessError:
            return False
        
    ##
    # Restart Mopidy
    #
    # This is untested and may require installation of an upstart script to properly restart
    ##
    def restart( self ):
        os.execl(sys.executable, *([sys.executable]+sys.argv))
        