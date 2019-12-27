
from __future__ import unicode_literals
from mopidy.core import CoreListener
from .core import IrisCore

import pykka
import logging
from .mem import iris

# import logger
logger = logging.getLogger(__name__)

class IrisFrontend(pykka.ThreadingActor, CoreListener):

    def __init__(self, config, core):
        super(IrisFrontend, self).__init__()

        # Pass our Mopidy config and core to the IrisCore instance
        iris.config = config
        iris.core = core

    def on_start(self):
        iris.start()

    def on_stop(self):
        iris.stop()

    def track_playback_ended(self, tl_track, time_position):
        iris.check_for_radio_update()

    def tracklist_changed(self):
        iris.clean_queue_metadata()
