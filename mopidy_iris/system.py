
import logging, pykka, subprocess, os

# import logger
logger = logging.getLogger(__name__)

class IrisSystemActor(pykka.ThreadingActor):

    def __init__(self):
        super(IrisSystemActor, self).__init__()

    ##
    # Someone is telling us to do something
    ##
    def run(self, action):
        path = os.path.dirname(__file__)
        return subprocess.check_output(["sudo "+path+"/system.sh "+action], shell=True)

actor_ref = IrisSystemActor.start()