
from __future__ import unicode_literals
from mopidy.core import CoreListener

import mem
import pykka
import logging

# import logger
logger = logging.getLogger(__name__)

class IrisFrontend(pykka.ThreadingActor, CoreListener):

    def __init__(self, config, core):
        super(IrisFrontend, self).__init__()
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
        