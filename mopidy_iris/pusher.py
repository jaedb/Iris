import tornado.ioloop, tornado.web, tornado.websocket, tornado.template
import logging, uuid, subprocess, pykka
from datetime import datetime
from tornado.escape import json_encode, json_decode

logger = logging.getLogger(__name__)

# container for all current pusher connections
connections = {}
frontend = {}
  
  
##
# Send a message to an individual connection
#
# @param recipient_connection_ids = array
# @param action = string (action method of this message)
# @param request_id = string (used for callbacks)
# @param data = array (any data required to include in our message)
##
def send_message( recipient_connection_id, action, request_id, data ):          
    message = {
        'action': action,
        'request_id': request_id,
        'data': data
    }
    connections[recipient_connection_id]['connection'].write_message( json_encode(message) )
        
##
# Broadcast a message to all recipients
#
# @param action = string
# @param data = array (the body of our message to send)
##
def broadcast( type, data ):    
    for connection in connections.itervalues():
        message = {
            'action': 'broadcast',
            'type': type,
            'data': data
        }
        connection['connection'].write_message( json_encode(message) )
        
        
# digest a protocol header into it's id/name parts
def digest_protocol( protocol ):
    
    # if we're a string, split into list
    # this handles the different ways we get this passed (select_subprotocols gives string, headers.get gives list)
    if isinstance(protocol, basestring):
    
        # make sure we strip any spaces (IE gives "element,element", proper browsers give "element, element")
        protocol = [i.strip() for i in protocol.split(',')]
    
    # if we've been given a valid array
    try:
		clientid = protocol[0]
		connectionid = protocol[1]
		username = protocol[2]
		generated = False
      
    # invalid, so just create a default connection, and auto-generate an ID
    except:
		clientid = str(uuid.uuid4().hex)
		connectionid = str(uuid.uuid4().hex)
		username = str(uuid.uuid4().hex)
		generated = True
    
    # construct our protocol object, and return
    return {"clientid": clientid, "connectionid": connectionid, "username": username, "generated": generated}

    
##
# Websocket server
#
# This is the actual websocket thread that accepts, digests and emits messages.
# TODO: Figure out how to merge this into the main Mopidy websocket to avoid needing two websocket servers
##    
class PusherWebsocketHandler(tornado.websocket.WebSocketHandler):
    
    def initialize(self, frontend):
        self.frontend = frontend

    def check_origin(self, origin):
        return True
  
    # when a new connection is opened
    def open(self):
    
        # decode our connection protocol value (which is a payload of id/name from javascript)
        protocolElements = digest_protocol(self.request.headers.get('Sec-Websocket-Protocol', []))

        connectionid = protocolElements['connectionid']
        clientid = protocolElements['clientid']
        self.connectionid = connectionid
        username = protocolElements['username']
        created = datetime.strftime(datetime.now(), '%Y-%m-%d %H:%M:%S')

        # construct our client object, and add to our list of connections
        client = {
            'clientid': clientid,
            'connectionid': connectionid,
            'username': username,
            'ip': self.request.remote_ip,
            'created': created
        }
        connections[connectionid] = {
            'client': client,
            'connection': self
        }

        logger.info( 'Pusher connection established: '+ connectionid +' ('+ clientid +'/'+ username +')' )

        # broadcast to all connections that a new user has connected
        broadcast( 'new_connection', client )
  
    def select_subprotocol(self, subprotocols):
        # select one of our subprotocol elements and return it. This confirms the connection has been accepted.
        protocols = digest_protocol( subprotocols )

        # if we've auto-generated some ids, the provided subprotocols was a string, so just return it right back
        # this allows a connection to be completed
        if protocols['generated']:
            return subprotocols[0]
            
        # otherwise, just return one of the supplied subprotocols
        else:
            return protocols['clientid']
  
    # server received a message
    def on_message(self, message):
        messageJson = json_decode(message)

        # construct the origin client info
        messageJson['origin'] = {
            'connectionid' : self.connectionid,
            'clientid': connections[self.connectionid]['client']['clientid'],
            'ip': self.request.remote_ip,
            'username': connections[self.connectionid]['client']['username']
        }
        
        logger.debug('Pusher message received: '+message)
    
        # broadcast message to other connections (except for self)
        if messageJson['action'] == 'broadcast':

            # respond to request with status update
            send_message(
                self.connectionid, 
                'response', 
                messageJson['request_id'], 
                { 'status': 'Ok' }
            )

            for connection in connections.itervalues():
                if connection['client']['connectionid'] != self.connectionid:
                    connection['connection'].write_message(messageJson)
    
        # send authroization details
        elif messageJson['action'] == 'send_authorization':

            # make sure we actually have a connection matching the provided connectionid
            if messageJson['recipient_connectionid'] in connections:

                # send payload to recipient
                authorization_message = {
                    'type': 'broadcast',
                    'action': 'received_authorization',
                    'authorization': messageJson['authorization'],
                    'me': messageJson['me'],
                    'origin': messageJson['origin']
                }
                connections[messageJson['recipient_connectionid']]['connection'].write_message(authorization_message)

                # respond to request with status update
                send_message(
                    self.connectionid, 
                    'response', 
                    messageJson['request_id'], 
                    { 'status': 'Ok' }
                )
            else:
                # respond to request with status update
                send_message(
                    self.connectionid, 
                    'response', 
                    messageJson['request_id'], 
                    { 'error': 'Could not send to that connection, does not exist' }
                )

        # fetch our pusher connections
        elif messageJson['action'] == 'get_config':                
            send_message(
                self.connectionid, 
                'response', 
                messageJson['request_id'], 
                { 'config': self.frontend.get_config() }
            )

        # fetch our pusher connections
        elif messageJson['action'] == 'get_connections':
        
            connectionsDetailsList = []
            for connection in connections.itervalues():
                connectionsDetailsList.append(connection['client'])
                
            send_message(
                self.connectionid, 
                'response', 
                messageJson['request_id'], 
                { 'connections': connectionsDetailsList }
            )
        
        # change connection's client username
        elif messageJson['action'] == 'set_username':
            
            # username is the only value we allow clients to change
            connections[messageJson['origin']['connectionid']]['client']['username'] = messageJson['username']
            
            # respond to request
            send_message(
                self.connectionid, 
                'response',
                messageJson['request_id'],
                { 'username': messageJson['username'] }
            )
            
            # notify all clients of this change
            broadcast( 'connection_updated', { 'connection': connections[messageJson['origin']['connectionid']]['client'] })
    
        # start radio
        elif messageJson['action'] == 'start_radio':
        
            # pull out just the radio data (we don't want all the request_id guff)
            radio = {
                'enabled': 1,
                'seed_artists': messageJson['seed_artists'],
                'seed_genres': messageJson['seed_genres'],
                'seed_tracks': messageJson['seed_tracks']
            }
            radio = self.frontend.start_radio( radio )
            send_message( 
                self.connectionid, 
                'response',
                messageJson['request_id'], 
                { 'radio': radio }
            )
    
        # stop radio
        elif messageJson['action'] == 'stop_radio':
            radio = self.frontend.stop_radio()
            send_message( 
                self.connectionid, 
                'response', 
                messageJson['request_id'], 
                { 'radio': self.frontend.radio }
            )
        
        # fetch our current radio state
        elif messageJson['action'] == 'get_radio':
            send_message( 
                self.connectionid, 
                'response',
                messageJson['request_id'],
                { 'radio': self.frontend.radio }
            )
    
        # get system version and check for upgrade
        elif messageJson['action'] == 'get_version':
            version = self.frontend.get_version()
            send_message( 
                self.connectionid, 
                'response',
                messageJson['request_id'], 
                { 'version': version } 
            )
    
        # perform upgrade
        elif messageJson['action'] == 'upgrade':
            version = self.frontend.get_version()
            upgrade_successful = self.frontend.perform_upgrade()
            send_message( 
                self.connectionid, 
                'response',
                messageJson['request_id'], 
                { 'upgrade_successful': upgrade_successful, 'version': version } 
            )
    
        # restart mopidy
        elif messageJson['action'] == 'restart':
            self.frontend.restart()            
        
        # not an action we recognise!
        else:
            send_message( 
                self.connectionid, 
                'response', 
                messageJson['request_id'], 
                { 'error': 'Unhandled action' } 
            )
                        
        logger.debug( 'Pusher: Unhandled message received from '+ self.connectionid )
  
    # connection closed
    def on_close(self):
        if self.connectionid in connections:
            
            clientRemoved = connections[self.connectionid]['client']
            logger.debug( 'Spotmop Pusher connection to '+ self.connectionid +' closed' )
            
            # now actually remove it
            try:
                del connections[self.connectionid]
            except:
                logger.info( 'Failed to close connection to '+ self.connectionid )                
            
            broadcast( 'client_disconnected', clientRemoved )
        
        
        
  