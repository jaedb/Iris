
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
			this.props.pusherActions.getConnectionList();
		}
	}

	componentWillReceiveProps(newProps){
		if( !this.props.connected && newProps.connected ){
			this.props.pusherActions.getConnectionList();
		}
	}

	render(){
		if( !this.props.connected ) return null;
		if( typeof(this.props.connections) == 'undefined' || this.props.connections.length <= 0 ) return null;

		return (
			<div className="pusher-connection-list">
				{
					this.props.connections.map( (connection, index) => {
						var is_me = false;
						if( connection.connectionid == this.props.connectionid ) is_me = true;
						return (
							<div className={ is_me ? 'connection cf me': 'connection cf'} key={connection.connectionid}>
								<div className="col w30">{ connection.username } { is_me ? <span>(you)</span> : null }</div>
								<div className="col w70">
									{ connection.ip }
									<span className="grey-text"> ({ connection.connectionid })</span>
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
		connectionid: state.pusher.connectionid,
		connections: state.pusher.connections
	}
}

const mapDispatchToProps = (dispatch) => {
	return {
		pusherActions: bindActionCreators(pusherActions, dispatch)
	}
}

export default connect(mapStateToProps, mapDispatchToProps)(PusherConnectionList)