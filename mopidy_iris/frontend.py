
from __future__ import unicode_literals
from mopidy.core import CoreListener

import mem
import pykka

class IrisFrontend(pykka.ThreadingActor, CoreListener):

    def __init__(self, config, core):
        super(IrisFrontend, self).__init__()
        mem.iris.core = core
        mem.iris.config = config

    def on_start(self):        
        print '--- Starting IrisFrontend'

    def track_playback_started(self, tl_track):
        mem.iris.broadcast({
            'action': 'started_playback'
        })
        