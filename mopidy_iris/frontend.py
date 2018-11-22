
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
        mem.iris.start()

    def on_stop(self):
        mem.iris.stop()

    def track_playback_ended(self, tl_track, time_position):
        mem.iris.check_for_radio_update()

    def tracklist_changed(self):
        mem.iris.clean_queue_metadata()
        