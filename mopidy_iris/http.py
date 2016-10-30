from __future__ import unicode_literals

import logging, json, urllib, urllib2
import tornado.web
from spotipy import Spotify

# import logger
logger = logging.getLogger(__name__)
        
class RequestHandler(tornado.web.RequestHandler):

    def set_default_headers(self):
        self.set_header("Access-Control-Allow-Origin", "*")

    def initialize(self, core, config):
        self.core = core
        self.config = config
    
    def get(self, slug=None):

        if( slug == 'refresh_spotify_token' ):
            self.write( self.refresh_spotify_token() )
            return

        else:
            self.write('Invalid request')
            return


    ##
    # Get a new spotify authentication token
    #
    # Uses the Client Credentials Flow, so is invisible to the user. We need this token for
    # any backend spotify requests (we don't tap in to Mopidy-Spotify, yet). Also used for
    # passing token to frontend for javascript requests without use of the Authorization Code Flow.
    ##
    def refresh_spotify_token( self ):
    
        url = 'https://accounts.spotify.com/api/token'
        authorization = 'YTg3ZmI0ZGJlZDMwNDc1YjhjZWMzODUyM2RmZjUzZTI6ZDdjODlkMDc1M2VmNDA2OGJiYTE2NzhjNmNmMjZlZDY='

        headers = {'Authorization' : 'Basic ' + authorization}
        data = {'grant_type': 'client_credentials'}
        data_encoded = urllib.urlencode( data )
        req = urllib2.Request(url, data_encoded, headers)

        try:
            response = urllib2.urlopen(req, timeout=30).read()
            response = json.loads(response)
            return response
        except urllib2.HTTPError as e:
            return e
        
