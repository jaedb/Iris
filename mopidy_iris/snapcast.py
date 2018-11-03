
from threading import Thread
import os, logging, subprocess, socket, json, random, string

# import logger
logger = logging.getLogger(__name__)

class IrisSnapcast(object):

    socket = None
    listen = False

    def __init__(self, config):
        self.config = config
        self.path = os.path.dirname(__file__)


    ##
    # This thread has been started
    ##
    def connect(self):

        host = str(self.config['iris']['snapcast_host'])
        port = int(self.config['iris']['snapcast_port'])

        try:
            self.socket = socket.socket(socket.AF_INET, socket.SOCK_STREAM)
            self.socket.settimeout(10)
            self.socket.connect((host, port))
            logger.debug("Snapcast connection established on "+host+":"+str(port))

        except socket.gaierror, e:
            raise Exception(e);

        except socket.error, e:
            raise Exception(e);


    ##
    # Listen for socket activity
    # Broadcast events to the piped broadcaster
    #
    # @param broadcast = Method
    ##
    def listen(self, broadcast):
        logger.info("Established Snapcast listener")

        broadcast(data={'method':'snapcast_connected'})

        self.listen = True

        data = ""

        while self.listen:

            # TODO: This works great, but when no data over X seconds we hit the timeout
            # Possibly stop timeout? seems to lock things up
            data = data + self.socket.recv(1024)
            print data
            if data.endswith(u"\r\n"):

                message = data
                data = ""

                logger.debug("Incoming Snapcast message: "+message)

                try:
                    message = json.loads(message)
                    broadcast(data=message)
                except:
                    logger.error("Malformed Snapcast message: "+message)


    ##
    # Stop our listener
    #
    # @param broadcast = Method
    ##
    def stop_listening(self):
        self.listen = False


    ##
    # Send a single request to Snapcast
    #
    # @param data = Dict
    ##
    def request(self, data):

        # No socket to make request on
        if self.socket == None:
            raise Exception("Socket not established");

        # Construct our request, based on the provided data
        request = {
            'id': self.generateGuid(),
            'jsonrpc': '2.0',
            'method': data['method'],
            'params': data['params'] if 'params' in data else {}
        }

        logger.debug("Outgoing Snapcast message")
        logger.debug(request)

        # Convert to string. For some really nuts reason we need an extra trailing curly brace...
        request = json.dumps(request)+'}'

        # Attempt to send the request
        try:
            self.socket.send(request.encode('ascii')+b"\n")

        except socket.error, e:
            logger.error("Iris could not send request to Snapcast: %s" % e)

            socket_response = {
                    'error': {
                        'message': "Failed to send request to Snapcast",
                        'data': str(e)
                    }
                }

        # Wait for response
        while True:

            try:
                response = self.socket.recv(8192)

            except socket.error, e:
                logger.error("Iris failed to receive Snapcast response: %s" % e)
                socket_response = {
                        'error': {
                            'message': "Failed to receive Snapcast response",
                            'data': str(e)
                        }
                    }

            if not len(response):
                break

            try:
                response = json.loads(response)
                logger.debug("Incoming Snapcast message")
                logger.debug(response)

                if 'result' in response:
                    socket_response = {
                        'response': response['result']
                    }
                else:
                    socket_response = {
                        'error': response['error']
                    }

            except:
                logger.error("Iris received malformed Snapcast response: "+response)
                socket_response = {
                        'error': {
                            'message': "Malformed Snapcast response",
                            'data': response
                        }
                    }

            return socket_response


    ##
    # Generate a random string
    #
    # Used for connection_ids where none is provided by client
    # @return string
    ##
    def generateGuid(self):
        length = 12
        return ''.join(random.choice(string.lowercase) for i in range(length))

