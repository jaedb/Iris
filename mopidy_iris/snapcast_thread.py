
from threading import Thread
import os, logging, subprocess, socket, json, random, string
from .snapcast import IrisSnapcast

# import logger
logger = logging.getLogger(__name__)

class IrisSnapcastThread(Thread):

    def __init__(self, config, broadcast):
        Thread.__init__(self)
        self.config = config
        self.broadcast = broadcast


    ##
    # This thread has been started
    ##
    def run(self):

        socket = IrisSnapcast(self.config)
        socket.connect()
        socket.listen(self.broadcast)
        