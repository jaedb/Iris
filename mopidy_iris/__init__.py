import logging, json, pathlib
import tornado.web
import tornado.websocket

import pkg_resources
from mopidy import config, ext
from .frontend import IrisFrontend
from .handlers import WebsocketHandler, HttpHandler
from .core import IrisCore
from .mem import iris

__version__ = pkg_resources.get_distribution("Mopidy-Iris").version

logger = logging.getLogger(__name__)


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
        return config.read(pathlib.Path(__file__).parent / "ext.conf")

    def get_config_schema(self):
        schema = config.ConfigSchema(self.ext_name)
        schema['enabled'] = config.Boolean()
        schema['country'] = config.String()
        schema['locale'] = config.String()
        schema['spotify_authorization_url'] = config.String()
        schema['lastfm_authorization_url'] = config.String()
        schema['genius_authorization_url'] = config.String()
        schema['data_dir'] = config.String()
        return schema

    def setup(self, registry):

        # Add web extension
        registry.add('http:app', {
            'name': self.ext_name,
            'factory': iris_factory
        })

        # Add our frontend
        registry.add('frontend', IrisFrontend)

##
# Customised handler for react router URLS
#
# This routes all URLs to the same path, so that React can handle the path etc
##
class ReactRouterHandler(tornado.web.StaticFileHandler):
    def initialize(self, path):
        self.path = path
        self.absolute_path = path
        self.dirname = path.parent
        self.filename = path.name
        super().initialize(self.dirname)

    def get(self, path=None, include_body=True):
        return super().get(self.path, include_body)

##
# Frontend factory
##
def iris_factory(config, core):

    path = pathlib.Path(__file__).parent / 'static'

    return [
        (
            r'/http/([^/]*)',
            handlers.HttpHandler,
            {
                'core': core,
                'config': config
            }
        ),
        (
            r'/ws/?',
            handlers.WebsocketHandler,
            {
                'core': core,
                'config': config
            }
        ),
        (
            r'/assets/(.*)',
            tornado.web.StaticFileHandler,
            {
                'path': path / 'assets'
            }
        ),
        (
            r'/((.*)(?:css|js|json|map)$)',
            tornado.web.StaticFileHandler,
            {
                'path': path
            }
        ),
        (
            r'/(.*)',
            ReactRouterHandler, {
                'path': path / 'index.html'
            }
        ),
    ]
