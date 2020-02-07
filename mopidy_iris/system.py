from threading import Thread
import logging
import os
import pathlib
import subprocess

# import logger
logger = logging.getLogger(__name__)


class IrisSystemError(Exception):
    pass


class IrisSystemPermissionError(IrisSystemError):
    reason = "Permission denied"

    def __init__(self, path):
        message = (
            "Password-less access to %s was refused. "
            "Check your /etc/sudoers file." % path.as_uri()
        )
        logger.error(message)
        super().__init__(message)


class IrisSystemThread(Thread):
    _USE_SUDO = True

    def __init__(self, action, ioloop, callback):
        Thread.__init__(self)
        self.action = action
        self.callback = callback
        self.ioloop = ioloop
        self.script_path = pathlib.Path(__file__).parent / "system.sh"

    def get_command(self, action=None, *, non_interactive=False):
        if self._USE_SUDO:
            if non_interactive:
                args = [b"sudo -n"]
            else:
                args = [b"sudo"]
        else:
            args = []

        if action is None:
            action = self.action

        args = args + [bytes(self.script_path), action.encode()]
        return args

    ##
    # Run the defined action
    ##
    def run(self):
        logger.info("Running system action '" + self.action + "'")

        try:
            self.can_run()
        except IrisSystemError as e:
            logger.error(e)

            error = {"message": e.reason, "description": str(e)}

            return {"error": error}

        command = self.get_command()
        logger.debug("Running '%s'", os.fsdecode(b" ".join(command)))
        process = subprocess.Popen(
            command, stdout=subprocess.PIPE, encoding="utf8"
        )

        lines = ""
        while True:
            line = process.stdout.readline()
            if process.poll() is not None:
                break
            if line:
                logger.info(line)
                lines = lines + "\n" + line

        if process.returncode == 0:
            self.ioloop.add_callback(
                lambda: self.callback({"output": lines}, None, None)
            )
        else:
            self.ioloop.add_callback(
                lambda: self.callback(None, {"error": lines}, None)
            )

    ##
    # Check if we have access to the system script (system.sh)
    #
    # @return boolean or exception
    ##
    def can_run(self, *args, **kwargs):
        # Attempt an empty call to our system file
        command_bytes = b" ".join(
            self.get_command("check", non_interactive=True)
        )
        process = subprocess.Popen(
            command_bytes,
            stdout=subprocess.PIPE,
            stderr=subprocess.PIPE,
            shell=True,
        )
        result, error = process.communicate()
        exitCode = process.wait()

        # Some kind of failure, so we can't run any commands this way
        if exitCode > 0:
            raise IrisSystemPermissionError(self.script_path)
        else:
            return True
