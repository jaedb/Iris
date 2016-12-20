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
# @param type = string (type of event, ie connection_opened)
# @param action = string (action method of this message)
# @param message_id = string (used for callbacks)
# @param data = array (any data required to include in our message)
##
def send_message( recipient_connection_id, type, action, message_id, data ):          
    message = {
        'type': type,
        'action': action,
        'message_id': message_id,
        'data': data
    }
    connections[recipient_connection_id]['connection'].write_message( json_encode(message) )
        
        
##
# Broadcast a message to all recipients
#
# @param action = string
# @param data = array (the body of our message to send)
##
def broadcast( action, data ):    
    for connection in connections.itervalues():
        message = {
            'type': 'broadcast',
            'action': action,
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
        broadcast( 'client_connected', client )
  
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
        
        # query-based message that is expecting a response
        if messageJson['type'] == 'query':
            
            # fetch our pusher connections
            if messageJson['action'] == 'get_connections':
            
                connectionsDetailsList = []
                for connection in connections.itervalues():
                    connectionsDetailsList.append(connection['client'])
                    
                send_message(
                    self.connectionid, 
                    'response', 
                    messageJson['action'], 
                    messageJson['message_id'], 
                    { 'connections': connectionsDetailsList }
                )
            
            # change connection's client username
            elif messageJson['action'] == 'change_username':
                
                # username is the only value we allow clients to change
                connections[messageJson['origin']['connectionid']]['client']['username'] = messageJson['username']
                
                # respond to request
                send_message(
                    self.connectionid, 
                    'response', 
                    messageJson['action'], 
                    messageJson['message_id'],
                    { 'connection': connections[messageJson['origin']['connectionid']]['client'] }
                )
                
                # notify all clients of this change
                broadcast( 'connection_updated', { 'connections': connections[messageJson['origin']['connectionid']]['client'] })
        
            # start radio
            elif messageJson['action'] == 'start_radio':
            
                # pull out just the radio data (we don't want all the message_id guff)
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
                    'radio',
                    messageJson['message_id'], 
                    { 'radio': radio }
                )
        
            # stop radio
            elif messageJson['action'] == 'stop_radio':
                radio = self.frontend.stop_radio()
                send_message( 
                    self.connectionid, 
                    'response', 
                    'radio', 
                    messageJson['message_id'], 
                    { 'radio': self.frontend.radio }
                )
            
            # fetch our current radio state
            elif messageJson['action'] == 'get_radio':
                send_message( 
                    self.connectionid, 
                    'response', 
                    'radio',
                    messageJson['message_id'],
                    { 'radio': self.frontend.radio }
                )

            # MOVED TO HTTP ENDPOINT FOR SIMPLICITY IN REACT+REDUX

            # # get our spotify authentication token
            # elif messageJson['action'] == 'get_spotify_token':
            #     send_message(
            #         self.connectionid,
            #         'response',
            #         messageJson['action'],
            #         messageJson['message_id'],
            #         { 'token': self.frontend.spotify_token } 
            #     )
        
            # # refresh our spotify authentication token
            # elif messageJson['action'] == 'refresh_spotify_token':
            #     token = self.frontend.refresh_spotify_token()
            #     send_message( 
            #         self.connectionid, 
            #         'response', 
            #         messageJson['action'], 
            #         messageJson['message_id'], 
            #         { 'token': token } 
            #     )
        
            # get system version and check for upgrade
            elif messageJson['action'] == 'get_version':
                version = self.frontend.get_version()
                send_message( 
                    self.connectionid, 
                    'response', 
                    messageJson['action'], 
                    messageJson['message_id'], 
                    { 'version': version } 
                )
        
            # get system version and check for upgrade
            elif messageJson['action'] == 'perform_upgrade':
                version = self.frontend.get_version()
                version['upgrade_successful'] = self.frontend.perform_upgrade()
                send_message( 
                    self.connectionid, 
                    'response', 
                    messageJson['action'], 
                    messageJson['message_id'], 
                    { 'version': version } 
                )
                
                # notify all clients of this change
                broadcast( 'upgraded', { 'version': version })
        
            # restart mopidy
            elif messageJson['action'] == 'restart':
                self.frontend.restart()
                
            
            # not an action we recognise!
            else:
                send_message( 
                    self.connectionid, 
                    'response', 
                    messageJson['action'], 
                    messageJson['message_id'], 
                    { 'error': 'Unhandled action' } 
                )
        
        # point-and-shoot one-way broadcast
        elif messageJson['type'] == 'broadcast':

            # recipients array has items, so only send to specific clients
            if messageJson.has_key('recipients'):  
                for connectionid in messageJson['recipients']:
                    connectionid = connectionid.encode("utf-8")
                    
                    # make sure we actually have a connection matching the provided connectionid
                    if connectionid in connections:
                        connections[connectionid]['connection'].write_message(messageJson)
                    else:
                        logger.warn('Pusher: Tried to broadcast to connectionid '+connectionid+' but it doesn\'t exist!');

            # empty, so send to all clients
            else:    
                for connection in connections.itervalues():
                
                    # if we've set ignore_self, then don't send message to originating connection
                    if messageJson.has_key('ignore_self'):
                        if connection['client']['connectionid'] != messageJson['origin']['connectionid']:
                            connection['connection'].write_message(messageJson)
                            
                    # send it to everyone
                    else:
                        connection['connection'].write_message(messageJson)
                        
        logger.debug( 'Pusher: Message received from '+ self.connectionid )
  
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
        
        
        
  