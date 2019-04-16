
/**
 * Actions and Action Creators
 **/

export function setPort(port){
	return {
		type: 'PUSHER_SET_PORT',
		port: port
	}
}

export function setUsername(username, force = false){
	return {
		type: 'PUSHER_SET_USERNAME',
		username: username.replace(/[\W_]+/g,''),
		force: force
	}
}

export function connect(){
	return {
		type: 'PUSHER_CONNECT'
	}
}

export function disconnect(){
	return {
		type: 'PUSHER_DISCONNECT'
	}
}

export function upgrade(){
	return {
		type: 'PUSHER_UPGRADE'
	}
}

export function reload(){
	return {
		type: 'PUSHER_RELOAD'
	}
}

export function restart(){
	return {
		type: 'PUSHER_RESTART'
	}
}

export function localScan(){
	return {
		type: 'PUSHER_LOCAL_SCAN'
	}
}

export function getConnections(){
	return {
		type: 'PUSHER_GET_CONNECTIONS'
	}
}

export function connectionAdded(connection){
	return {
		type: 'PUSHER_CONNECTION_ADDED',
		connection: connection
	}
}

export function connectionChanged(connection){
	return {
		type: 'PUSHER_CONNECTION_CHANGED',
		connection: connection
	}
}

export function connectionRemoved(connection){
	return {
		type: 'PUSHER_CONNECTION_REMOVED',
		connection: connection
	}
}

export function request(method, params = null, response_callback = null, error_callback = null){
	return {
		type: 'PUSHER_REQUEST',
		method: method,
		params: params,
		response_callback: response_callback,
		error_callback: error_callback
	}
}

export function getConfig(){
	return {
		type: 'PUSHER_GET_CONFIG'
	}
}

export function getVersion(){
	return {
		type: 'PUSHER_GET_VERSION'
	}
}

export function deliverBroadcast(method, params){
	return {
		type: 'PUSHER_DELIVER_BROADCAST',
		data: {
			method: method,
			params: params
		}
	}
}

export function deliverMessage(recipient, method, params){
	return {
		type: 'PUSHER_DELIVER_MESSAGE',
		data: {
			recipient: recipient,
			method: method,
			params: params
		}
	}
}

export function getRadio(){
	return {
		type: 'PUSHER_GET_RADIO'
	}
}

export function startRadio(uris){
	return {
		type: 'PUSHER_START_RADIO',
		uris: uris
	}
}

export function updateRadio(uris){
	return {
		type: 'PUSHER_UPDATE_RADIO',
		uris: uris
	}
}

export function stopRadio(){
	return {
		type: 'PUSHER_STOP_RADIO'
	}
}

export function radioStarted(radio){
	return {
		type: 'PUSHER_RADIO_STARTED',
		radio: radio
	}
}

export function radioChanged(radio){
	return {
		type: 'PUSHER_RADIO_CHANGED',
		radio: radio
	}
}

export function radioStopped(){
	return {
		type: 'PUSHER_RADIO_STOPPED'
	}
}

export function debug(message = null){
	return {
		type: 'PUSHER_DEBUG',
		message: message
	}
}

export function getQueueMetadata(){
	return {
		type: 'PUSHER_GET_QUEUE_METADATA'
	}
}

export function queueMetadataChanged(queue_metadata){
	return {
		type: 'PUSHER_QUEUE_METADATA_CHANGED',
		queue_metadata: queue_metadata
	}
}

export function addQueueMetadata(tlids = [], from_uri = null){
	return {
		type: 'PUSHER_ADD_QUEUE_METADATA',
		tlids: tlids,
		from_uri: from_uri
	}
}


/**
 * Commands (buttons)
 **/

export function getCommands(){
    return {
        type: 'PUSHER_GET_COMMANDS'
    }
}

export function setCommand(command){
    return {
        type: 'PUSHER_SET_COMMAND',
        command: command
    }
}

export function setCommands(commands){
    return {
        type: 'PUSHER_SET_COMMANDS',
        commands: commands
    }
}

export function removeCommand(id){
    return {
        type: 'PUSHER_REMOVE_COMMAND',
        id: id
    }
}

export function runCommand(id, notify = false){
    return {
        type: 'PUSHER_RUN_COMMAND',
        id: id,
        notify: notify
    }
}

export function commandsUpdated(commands){
    return {
        type: 'PUSHER_COMMANDS_UPDATED',
        commands: commands
    }
}