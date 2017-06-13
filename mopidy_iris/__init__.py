
from __future__ import unicode_literals

import logging, os, json
import tornado.web
import tornado.websocket
import handlers

from mopidy import config, ext
from frontend import IrisFrontend
from handlers import WebsocketHandler, HttpHandler
from core import IrisCore

logger = logging.getLogger(__name__)
__version__ = '2.14.4'

##
# Core extension class
#
# Loads config and gets the party started. Initiates any additional frontends, etc.
##
class Extension( ext.Extension ):

    dist_name = 'Mopidy-Iris'
    ext_name = 'iris'
    version = __version__

    def get_default_config(self):
        conf_file = os.path.join(os.path.dirname(__file__), 'ext.conf')
        return config.read(conf_file)

    def get_config_schema(self):
        schema = config.ConfigSchema(self.ext_name)
        schema['enabled'] = config.Boolean()
        schema['country'] = config.String()
        schema['locale'] = config.String()
        schema['authorization_url'] = config.String()
        return schema

    def setup(self, registry):
        
        # Add web extension
        registry.add('http:app', {
            'name': self.ext_name,
            'factory': iris_factory
        })

        # create our core instance
        mem.iris = IrisCore()
        mem.iris.version = self.version

        # Add our frontend
        registry.add('frontend', IrisFrontend)


def iris_factory(config, core):

    path = os.path.join( os.path.dirname(__file__), 'static')
    
    return [
        (r"/images/(.*)", tornado.web.StaticFileHandler, {
            'path': config['local-images']['image_dir']
        }),
        (r'/http/([^/]*)', handlers.HttpHandler, {
            'core': core,
            'config': config
        }),
        (r'/ws/?', handlers.WebsocketHandler, { 
            'core': core,
            'config': config
        }),
        (r'/(.*)', tornado.web.StaticFileHandler, {
            'path': path,
            'default_filename': 'index.html'
        }),
    ]
