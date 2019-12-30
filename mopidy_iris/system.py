
from threading import Thread
import os, logging, subprocess, json

# import logger
logger = logging.getLogger(__name__)

class IrisSystemThread(Thread):
    def __init__(self, action, callback):
        Thread.__init__(self)
        self.action = action
        self.callback = callback
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

            if self.callback:
                self.callback(False, error)
            return

        # Create subprocess
        process = await asyncio.create_subprocess_shell(
            self.path+"/system.sh "+self.action,
            stdout=asyncio.subprocess.PIPE,
            stderr=asyncio.subprocess.PIPE
        )

        # Status
        print("Started:", command, "(pid = " + str(process.pid) + ")", flush=True)

        # Wait for the subprocess to finish
        stdout, stderr = await process.communicate()

        logger.debug("System action '"+self.action+"' completed with output:")
        logger.debug(stdout)

        # Some kind of failure, so we can't run any commands this way
        if exitCode > 0:
            raise Exception("System task '"+self.action+"' failed")
        else:
            if self.callback:
                response = {
                    'output': str(stdout)
                }
                self.callback(response, False)


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
