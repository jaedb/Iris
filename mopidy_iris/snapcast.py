
from threading import Thread
import os, logging, subprocess, socket, json, random, string, select

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
            self.sock = socket.socket(socket.AF_INET, socket.SOCK_STREAM)
            self.sock.settimeout(10)
            self.sock.connect((host, port))
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

        self.connect()
        self.listen = True

        logger.info("Established Snapcast listener")
        broadcast(data={'method':'snapcast_connected'})

        messages = []
        data = ""
        select.select([], [self.sock], [])

        while self.listen:

            # This loop will run constantly in the background, so we need a relatively short
            # timeout as this would otherwise block the shutdown sequence
            timeout = 1

            readlist, writelist, exceptionlist = select.select([self.sock], [], [], timeout)

            # Check if we've got any lists
            if [readlist, writelist, exceptionlist] != [[], [], []]:

                # Rread and print the available data on any of the read list
                for socket in readlist:

                    # Allow a relatively large buffer size to handle large JSON payloads
                    message = socket.recv(8192)

                    try:
                        message = json.loads(message)

                        # Prefix with snapcast
                        message['method'] = "snapcast_"+message['method']

                        # Broadcast to all clients
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
        if self.sock == None:
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
            self.sock.send(request.encode('ascii')+b"\n")

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
                response = self.sock.recv(8192)

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
                    socket_response = response['result']
                else:
                    socket_response = response['error']

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

