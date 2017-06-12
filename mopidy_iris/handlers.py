
from __future__ import unicode_literals
from datetime import datetime
from tornado.escape import json_encode, json_decode
from spotipy import Spotify
import tornado.ioloop, tornado.web, tornado.websocket, tornado.template
import random, string, logging, uuid, subprocess, pykka, ast, logging, json, urllib, urllib2, mem

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
        protocols = mem.iris.digest_protocol( subprotocols )

        # if we've auto-generated some ids, the provided subprotocols was a string, so just return it right back
        # this allows a connection to be completed
        if protocols['generated']:
            return subprotocols[0]
            
        # otherwise, just return one of the supplied subprotocols
        else:
            return protocols['clientid']

    def open(self):
    
        # decode our connection protocol value (which is a payload of id/name from javascript)
        protocolElements = mem.iris.digest_protocol(self.request.headers.get('Sec-Websocket-Protocol', []))

        connection_id = protocolElements['connection_id']
        clientid = protocolElements['clientid']
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
            'clientid': clientid,
            'connection_id': connection_id,
            'username': username,
            'ip': ip,
            'created': created
        }

        # add to connections
        mem.iris.add_connection(connection_id, self, client)
 

    def on_message(self, message):
        
        message = json_decode(message)

        if 'data' in message:
            data = message['data']
        else:
            data = {}

        # Handle hard-coded connection_id in messages
        # Otherwise include the origin connection of this message
        if 'connection_id' not in data:
            data['connection_id'] = self.connection_id

        if 'request_id' in message:
            request_id = message['request_id']
        else:
            request_id = False

        # call the method, as specified in payload
        if 'method' in message:

            # make sure the method exists
            if hasattr(mem.iris, message['method']):

                # make the call, and return it's response
                response = getattr(mem.iris, message['method'])(data)
                if response:
                    response['request_id'] = request_id
                    mem.iris.send_message(self.connection_id, response)
            else:
                response = {
                    'status': 0,
                    'message': 'Method "'+message['method']+'" does not exist',
                    'request_id': request_id
                }
                mem.iris.send_message(self.connection_id, response)
        else:
            response = {
                'status': 0,
                'message': 'Method key missing from request',
                'request_id': request_id
            }
            mem.iris.send_message(self.connection_id, response)


    def on_close(self):
        mem.iris.remove_connection(self.connection_id)





        
class HttpHandler(tornado.web.RequestHandler):

    def set_default_headers(self):
        self.set_header("Access-Control-Allow-Origin", "*")

    def initialize(self, core, config):
        self.core = core
        self.config = config
    
    def get(self, slug=None):

        # make sure the method exists
        if hasattr(mem.iris, slug):

            # make the call, and return it's response
            self.write(getattr(mem.iris, slug)({}))
        else:
            self.write({
                'error': 'Method "'+slug+'" does not exist'
            })
        
