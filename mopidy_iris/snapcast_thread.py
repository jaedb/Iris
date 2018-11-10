
from threading import Thread
import os, logging, subprocess, socket, json, random, string
from .snapcast import IrisSnapcast

# import logger
logger = logging.getLogger(__name__)

class IrisSnapcastThread(Thread):

    sock = None

    def __init__(self, config, broadcast):
        Thread.__init__(self)
        self.config = config
        self.broadcast = broadcast


    ##
    # This thread has been started
    ##
    def run(self):

        self.sock = IrisSnapcast(self.config)
        self.sock.listen(self.broadcast)


    ##
    # Instruction to stop this thread
    ##
    def close(self):
        self.sock.stop_listening()
