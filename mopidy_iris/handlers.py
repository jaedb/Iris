
from __future__ import unicode_literals
from datetime import datetime
from tornado.escape import json_encode, json_decode
import tornado.ioloop, tornado.web, tornado.websocket, tornado.template
import random, string, logging, uuid, subprocess, pykka, ast, logging, json, urllib, urllib2, mem, requests, time

logger = logging.getLogger(__name__)

class WebsocketHandler(tornado.websocket.WebSocketHandler):
    
    # initiate (not the actual object __init__, but run shortly after)
    def initialize(self, core, config):
        self.core = core  
        self.config = config  

    def check_origin(self, origin):
        return True
        
    def select_subprotocol(self, subprotocols):

        # select one of our subprotocol elements and return it. This confirms the connection has been accepted.
        protocols = mem.iris.digest_protocol(subprotocols)

        # if we've auto-generated some ids, the provided subprotocols was a string, so just return it right back
        # this allows a connection to be completed
        if protocols['generated']:
            return subprotocols[0]
            
        # otherwise, just return one of the supplied subprotocols
        else:
            return protocols['client_id']

    def open(self):
    
        # decode our connection protocol value (which is a payload of id/name from javascript)
        protocolElements = mem.iris.digest_protocol(self.request.headers.get('Sec-Websocket-Protocol', []))

        connection_id = protocolElements['connection_id']
        client_id = protocolElements['client_id']
        self.connection_id = connection_id
        username = protocolElements['username']
        created = datetime.strftime(datetime.now(), '%Y-%m-%d %H:%M:%S')

        # get our IP
        # if it's local, then check for proxy origin
        ip = self.request.remote_ip
        if (ip == '127.0.0.1' and hasattr(self.request.headers,'X-Forwarded-For')):
            ip = self.request.headers['X-Forwarded-For']

        # construct our client object, and add to our list of connections
        client = {
            'client_id': client_id,
            'connection_id': connection_id,
            'username': username,
            'ip': ip,
            'created': created
        }

        # add to connections
        mem.iris.add_connection(connection_id=connection_id, connection=self, client=client)
 

    def on_message(self, message):
        logger.debug("Iris websocket message received: "+message)
        
        message = json_decode(message)

        if 'id' in message:
            id = message['id']
        else:
            id = None

        if 'jsonrpc' not in message:
            self.handle_result(id=id, error={'id': id, 'code': 32602, 'message': 'Invalid JSON-RPC request (missing property "jsonrpc")'})
        
        if 'params' in message:
            params = message['params']

            # Handle hard-coded connection_id in messages
            # Otherwise include the origin connection of this message
            if 'connection_id' not in params:
                message['params']['connection_id'] = self.connection_id
        else:
            params = {}

        # call the method, as specified in payload
        if 'method' in message:

            # make sure the method exists
            if hasattr(mem.iris, message['method']):
                getattr(mem.iris, message['method'])(data=params, callback=lambda response, error=False: self.handle_result(id=id, method=message['method'], response=response, error=error))

            else:
                self.handle_result(error={'id': id, 'code': 32601, 'message': 'Method "'+message['method']+'" does not exist'}, id=id)
                return
        else:
            self.handle_result(error={'id': id, 'code': 32602, 'message': 'Method key missing from request'}, id=id)
            return


    def on_close(self):
        mem.iris.remove_connection(connection_id=self.connection_id)

    ##
    # Handle a response from our core
    # This is just our callback from an Async request
    ##
    def handle_result(self, *args, **kwargs):
        id = kwargs.get('id', False)
        method = kwargs.get('method', None)
        response = kwargs.get('response', None)
        error = kwargs.get('error', None)
        request_response = {
            'id': id,
            'jsonrpc': '2.0',
            'method': method
        }

        # We've been given an error
        if error:
            error['id'] = id
            request_response['error'] = error

        # We've been handed an AsyncHTTPClient callback. This is the case
        # when our request calls subsequent external requests (eg Spotify, Genius)
        elif isinstance(response, tornado.httpclient.HTTPResponse):
            request_response['result'] = response.body

        # Just a regular json object, so not an external request
        else:
            request_response['result'] = response
        
        # Respond to the original request
        data = request_response
        data['recipient'] = self.connection_id
        mem.iris.send_message(data=data)





        
class HttpHandler(tornado.web.RequestHandler):

    def set_default_headers(self):
        self.set_header("Access-Control-Allow-Origin", "*")
        self.set_header("Access-Control-Allow-Headers", "Origin, X-Requested-With, Content-Type, Accept, Authorization, Client-Security-Token, Accept-Encoding")        

    def initialize(self, core, config):
        self.core = core
        self.config = config

    # Options request
    # This is a preflight request for CORS requests
    def options(self, slug=None):
        self.set_status(204)
        self.finish()
    
    @tornado.web.asynchronous
    def get(self, slug=None):

        id = int(time.time())

        # make sure the method exists
        if hasattr(mem.iris, slug):
            getattr(mem.iris, slug)(request=self, callback=lambda response, error=False: self.handle_result(id=id, method=slug, response=response, error=error))

        else:
            self.handle_result(id=id, error={'code': 32601, 'message': "Method "+slug+" does not exist"})
            return

    @tornado.web.asynchronous
    def post(self, slug=None):

        id = int(time.time())

        try:
            params = json.loads(self.request.body.decode('utf-8'))
        except:            
            self.handle_result(id=id, error={'code': 32700, 'message': "Missing or invalid payload"})
            return

        # make sure the method exists
        if hasattr(mem.iris, slug):
            try:
                getattr(mem.iris, slug)(data=params, request=self.request, callback=lambda response=False, error=False: self.handle_result(id=id, method=slug, response=response, error=error))

            except urllib2.HTTPError as e:
                self.handle_result(id=id, error={'code': 32601, 'message': "Invalid JSON payload"})
                return

        else:
            self.handle_result(id=id, error={'code': 32601, 'message': "Method "+slug+" does not exist"})
            return

    ##
    # Handle a response from our core
    # This is just our callback from an Async request
    ##
    def handle_result(self, *args, **kwargs):
        id = kwargs.get('id', None)
        method = kwargs.get('method', None)
        response = kwargs.get('response', None)
        error = kwargs.get('error', None)
        request_response = {
            'id': id,
            'jsonrpc': '2.0',
            'method': method
        }

        if error:
            request_response['error'] = error
            self.set_status(400)


        # We've been handed an AsyncHTTPClient callback. This is the case
        # when our request calls subsequent external requests (eg Spotify, Genius).
        # We don't need to wrap non-HTTPResponse responses as these are dicts
        elif isinstance(response, tornado.httpclient.HTTPResponse):

            # Digest JSON responses into JSON
            content_type = response.headers.get('Content-Type')
            if content_type.startswith('application/json') or content_type.startswith('text/json'):
                body = json.loads(response.body)

            # Non-JSON so just copy as-is
            else:
                body = response.body

            request_response['result'] = body

        # Regular ol successful response
        else:
            request_response['result'] = response


        # Write our response

        self.write(request_response)
        self.finish()


        
