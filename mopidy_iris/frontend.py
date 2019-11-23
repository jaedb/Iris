
from __future__ import unicode_literals
from mopidy.core import CoreListener

import pykka
import logging
from .mem import getIris

# import logger
logger = logging.getLogger(__name__)

class IrisFrontend(pykka.ThreadingActor, CoreListener):

    def __init__(self, config, core):
        super(IrisFrontend, self).__init__()
        getIris().core = core
        getIris().config = config

    def on_start(self):
        getIris().start()

    def on_stop(self):
        getIris().stop()

    def track_playback_ended(self, tl_track, time_position):
        getIris().check_for_radio_update()

    def tracklist_changed(self):
        getIris().clean_queue_metadata()
