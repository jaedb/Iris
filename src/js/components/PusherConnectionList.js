
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
		if( this.props.pusher.connected ){
			this.props.pusherActions.getConnectionList();
		}
	}

	componentWillReceiveProps(newProps){
		if( !this.props.pusher.connected && newProps.pusher.connected ){
			this.props.pusherActions.getConnectionList();
		}
	}

	render(){
		if( !this.props.pusher.connected ) return null;
		if( typeof(this.props.pusher.connections) == 'undefined' || this.props.pusher.connections.length <= 0 ) return null;

		return (
			<div className="pusher-connection-list">
				{
					this.props.pusher.connections.map( (connection, index) => {
						var isMe = false;
						if( connection.connectionid == this.props.pusher.connectionid ) isMe = true;
						return (
							<div className="connection cf" key={connection.connectionid}>
								<div className="col w20">{ connection.username } { isMe ? <span>(you)</span> : null }</div>
								<div className="col w20 one-liner">{ connection.ip }</div>
								<div className="col w20 one-liner">{ connection.connectionid }</div>
							</div>
						);
					})
				}
			</div>
		);
	}
}

const mapStateToProps = (state, ownProps) => {
	return state;
}

const mapDispatchToProps = (dispatch) => {
	return {
		pusherActions: bindActionCreators(pusherActions, dispatch)
	}
}

export default connect(mapStateToProps, mapDispatchToProps)(PusherConnectionList)