import logging
import pathlib

from mopidy import config, ext

__version__ = "3.51.0"

logger = logging.getLogger(__name__)


##
# Core extension class
#
# Loads config and gets the party started. Initiates any additional frontends, etc.
##
class Extension(ext.Extension):

    dist_name = "Mopidy-Iris"
    ext_name = "iris"
    version = __version__

    def get_default_config(self):
        return config.read(pathlib.Path(__file__).parent / "ext.conf")

    def get_config_schema(self):
        schema = config.ConfigSchema(self.ext_name)
        schema["enabled"] = config.Boolean()
        schema["country"] = config.String()
        schema["locale"] = config.String()
        schema["spotify_authorization_url"] = config.String()
        schema["lastfm_authorization_url"] = config.String()
        schema["genius_authorization_url"] = config.String()
        schema["data_dir"] = config.String()  # Deprecated
        return schema

    def setup(self, registry):
        from .frontend import IrisFrontend

        # Add web extension
        registry.add(
            "http:app", {"name": self.ext_name, "factory": iris_factory}
        )

        # Add our frontend
        registry.add("frontend", IrisFrontend)


##
# Frontend factory
##
def iris_factory(config, core):
    from tornado.web import StaticFileHandler
    from .handlers import HttpHandler, ReactRouterHandler, WebsocketHandler

    path = pathlib.Path(__file__).parent / "static"

    return [
        (r"/http/([^/]*)", HttpHandler, {"core": core, "config": config}),
        (r"/ws/?", WebsocketHandler, {"core": core, "config": config}),
        (r"/assets/(.*)", StaticFileHandler, {"path": path / "assets"}),
        (r"/((.*)(?:css|js|json|map)$)", StaticFileHandler, {"path": path}),
        (r"/(.*)", ReactRouterHandler, {"path": path / "index.html"}),
    ]
