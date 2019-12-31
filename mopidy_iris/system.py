
from threading import Thread
import os, logging, subprocess, json, asyncio

# import logger
logger = logging.getLogger(__name__)

class IrisSystemThread:
    def __init__(self, action):
        self.action = action
        self.path = os.path.dirname(__file__)

    ##
    # Run the defined action
    ##
    async def run(self):
        logger.info("Running system action '"+self.action+"'")

        try:
            self.can_run()
        except Exception as e:
            logger.error(e)

            error = {
                'message': "Permission denied",
                'description': str(e)
            }
            
            return {
                'error': error
            }

        logger.debug("sudo "+ self.path +"/system.sh "+ self.action)

        proc = subprocess.Popen(["sudo", self.path+"/system.sh", self.action], 
            stdout=subprocess.PIPE, 
            stderr=subprocess.STDOUT)
        
        stdout,stderr = proc.communicate()

        if stderr:
            logger.error(stderr.decode())
            return {
                'error': stderr.decode()
            }
        else:
            logger.info(stdout.decode())
            return {
                'output': stdout.decode()
            }


    ##
    # Check if we have access to the system script (system.sh)
    #
    # @return boolean or exception
    ##
    def can_run(self, *args, **kwargs):

        # Attempt an empty call to our system file
        process = subprocess.Popen("sudo -n "+self.path+"/system.sh", stdout=subprocess.PIPE, stderr=subprocess.PIPE, shell=True)
        result, error = process.communicate()
        exitCode = process.wait()

        # Some kind of failure, so we can't run any commands this way
        if exitCode > 0:
            raise Exception("Password-less access to "+self.path+"/system.sh was refused. Check your /etc/sudoers file.")
        else:
            return True
