from threading import Thread
import logging, pathlib, subprocess, json

# import logger
logger = logging.getLogger(__name__)


class IrisSystemError(Exception):
    pass


class IrisSystemPermissionError(IrisSystemError):
    reason = "Permission denied"

    def __init__(self, path):
        message = "Password-less access to %s was refused. Check your /etc/sudoers file." % path.as_uri()
        super().__init__(message)


class IrisSystemMissingError(IrisSystemError):
    reason = "Not found"

    def __init__(self, path):
        message = "Unable to access %s." % path.as_uri()
        super().__init__(message)


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
        except IrisSystemError as e:
            logger.error(e)

            error = {
                'message': e.reason,
                'description': e.message
            }
            
            return {
                'error': error
            }

        logger.debug("sudo %s %s", self.script_path.as_uri(), self.action)

        proc = subprocess.Popen([b"sudo", bytes(self.script_path), self.action.encode()],
            stdout=subprocess.PIPE, 
            stderr=subprocess.PIPE)
        
        stdout,stderr = proc.communicate()

        if stderr:
            error_string = os.fsdecode(stderr)
            logger.error(error_string)
            self.callback(None, {'error': error_string})
        else:
            response_string = os.fsdecode(stdout)
            logger.info(response_string)
            self.callback({'output': response_string}, None)



    ##
    # Check if we have access to the system script (system.sh)
    #
    # @return boolean or exception
    ##
    def can_run(self, *args, **kwargs):
        if not self.script_path.is_file():
            raise IrisSystemMissingError(self.script_path)

        # Attempt an empty call to our system file
        process = subprocess.Popen(b"sudo -n %s check" % bytes(self.script_path), stdout=subprocess.PIPE, stderr=subprocess.PIPE, shell=True)
        result, error = process.communicate()
        exitCode = process.wait()

        # Some kind of failure, so we can't run any commands this way
        if exitCode > 0:
            raise IrisSystemPermissionError(self.script_path)
        else:
            return True
