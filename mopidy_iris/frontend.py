
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

        #if mem.iris.config['iris']['snapcast_enabed']:
        try:
            mem.iris.snapcast_listener = mem.iris.new_snapcast_socket()
            logger.info("Iris connected to Snapcast")
        except e:
            logger.error("Iris could not connect to Snapcast: %s" % e)

    def on_stop(self):
        mem.iris.snapcast_disconnect()

    def track_playback_ended(self, tl_track, time_position):
        mem.iris.check_for_radio_update()

    def tracklist_changed(self):
        mem.iris.clean_queue_metadata()
        