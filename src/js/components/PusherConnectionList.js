
import React, { PropTypes } from 'react'
import { connect } from 'react-redux'
import { Link } from 'react-router'
import { createStore, bindActionCreators } from 'redux'

import FontAwesome from 'react-fontawesome'

import * as pusherActions from '../services/pusher/actions'

class PusherConnectionList extends React.Component{

	constructor(props) {
		super(props);
	}

	componentDidMount(){
		if( this.props.connected ){
			this.props.pusherActions.getConnections();
		}
	}

	componentWillReceiveProps(newProps){
		if( !this.props.connected && newProps.connected ){
			this.props.pusherActions.getConnections();
		}
	}

	render(){
		if( !this.props.connected ) return <div className="pusher-connection-list grey-text">Not connected</div>

		var connections = []
		for (var connection_id in this.props.connections){
			if (this.props.connections.hasOwnProperty(connection_id)) {
				connections.push(this.props.connections[connection_id])
			}
		}

		if (connections.length <= 0) return <div className="pusher-connection-list grey-text">No connections</div>;

		return (
			<div className="pusher-connection-list">
				{
					connections.map( (connection, index) => {
						var is_me = false;
						if( connection.connection_id == this.props.connection_id ) is_me = true;
						return (
							<div className={ is_me ? 'connection cf me': 'connection cf'} key={connection.connection_id}>
								<div className="col w30">{ connection.username } { is_me ? <span>(you)</span> : null }</div>
								<div className="col w70">
									{ connection.ip }
									<span className="grey-text"> ({ connection.connection_id })</span>
								</div>
							</div>
						);
					})
				}
			</div>
		);
	}
}

const mapStateToProps = (state, ownProps) => {
	return {
		connected: state.pusher.connected,
		connection_id: state.pusher.connection_id,
		connections: state.pusher.connections
	}
}

const mapDispatchToProps = (dispatch) => {
	return {
		pusherActions: bindActionCreators(pusherActions, dispatch)
	}
}

export default connect(mapStateToProps, mapDispatchToProps)(PusherConnectionList)