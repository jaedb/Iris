
from __future__ import unicode_literals
from mopidy.core import CoreListener

import mem
import pykka
import logging

from core import IrisCore
from raven import Client

# import logger
logger = logging.getLogger(__name__)

class IrisFrontend(pykka.ThreadingActor, CoreListener):

    def __init__(self, config, core):
        super(IrisFrontend, self).__init__()

        from mopidy_iris import __version__ as version

        # create our core instance
        mem.iris = IrisCore()
        mem.iris.version = version

        if not config['iris']['privacy']:
            # Connect to our Ravent Sentry error tracker
            mem.iris.raven_client = Client(
                dsn='https://023e3bf7721b48f29948545fc36a4621:ba30d29174ef4778ac4e141d445607a2@sentry.io/219026',
                release=version
            )

        mem.iris.core = core
        mem.iris.config = config

    def on_start(self):        
        logger.info('Starting Iris '+mem.iris.version)

        # Create our listening socket for Snapcast event notifications (only if enabled)
        #if mem.iris.config['iris'].get('snapcast_enabled'):
        #    mem.iris.create_snapcast_listener()

    def on_stop(self):
        mem.iris.snapcast_disconnect_listener()

    def track_playback_ended(self, tl_track, time_position):
        mem.iris.check_for_radio_update()

    def tracklist_changed(self):
        mem.iris.clean_queue_metadata()
        