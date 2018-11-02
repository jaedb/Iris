
from threading import Thread
import os, logging, subprocess

# import logger
logger = logging.getLogger(__name__)

class IrisSystemThread(Thread):
    def __init__(self, action, callback):
        Thread.__init__(self)
        self.action = action
        self.callback = callback

    ##
    # Run the defined action
    ##
    def run(self):
        logger.info("Running system action: "+self.action)

        try:
            self.check_system_access()
        except Exception, e:
            logger.error(e)

            # And then, when complete, return to our callback
            if self.callback:
                self.callback(False, "Permission denied")
            

        # Run the actual task (this is the process-blocking instruction)
        path = os.path.dirname(__file__)        
        response = subprocess.check_output(["sudo "+path+"/system.sh "+self.action], shell=True)

        # And then, when complete, return to our callback
        if self.callback:
            self.callback(response, False)


    ##
    # Check if we have access to the system script (system.sh)
    #
    # @return boolean or exception
    ##
    def check_system_access(self, *args, **kwargs):
        path = os.path.dirname(__file__)

        # Attempt the upgrade
        process = subprocess.Popen("sudo -n "+path+"/system.sh", stdout=subprocess.PIPE, stderr=subprocess.PIPE, shell=True)
        result, error = process.communicate()
        exitCode = process.wait()

        if exitCode > 0:
            raise Exception("Password-less access to "+path+"/system.sh was refused. Check your /etc/sudoers file.")
        else:
            return True
