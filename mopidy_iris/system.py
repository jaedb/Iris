from threading import Thread
import logging, pathlib, subprocess, json

# import logger
logger = logging.getLogger(__name__)

class IrisSystemThread(Thread):
    def __init__(self, action, callback):
        Thread.__init__(self)
        self.action = action
        self.callback = callback
        self.script_path = pathlib.Path(__file__).parent / "system.sh"

    ##
    # Run the defined action
    ##
    def run(self):
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

        logger.debug("sudo %s %s", self.script_path, self.action)

        proc = subprocess.Popen(["sudo", str(self.script_path), self.action],
            stdout=subprocess.PIPE, 
            stderr=subprocess.STDOUT)
        
        stdout,stderr = proc.communicate()

        if stderr:
            logger.error(stderr.decode())
            self.callback(None, { 'error': stderr.decode() })
        else:
            logger.info(stdout.decode())
            self.callback({ 'output': stdout.decode() }, None)



    ##
    # Check if we have access to the system script (system.sh)
    #
    # @return boolean or exception
    ##
    def can_run(self, *args, **kwargs):

        # Attempt an empty call to our system file
        process = subprocess.Popen("sudo -n %s check" % self.script_path, stdout=subprocess.PIPE, stderr=subprocess.PIPE, shell=True)
        result, error = process.communicate()
        exitCode = process.wait()

        # Some kind of failure, so we can't run any commands this way
        if exitCode > 0:
            raise Exception("Password-less access to %s was refused. Check your /etc/sudoers file." % self.script_path)
        else:
            return True
