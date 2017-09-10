
import React, { PropTypes } from 'react'
import { connect } from 'react-redux'
import { Link } from 'react-router'
import { createStore, bindActionCreators } from 'redux'

import * as uiActions from '../../services/ui/actions'
import * as helpers from '../../helpers'

import Icon from '../Icon'

class AuthorizationModal_Send extends React.Component{

	constructor(props){
		super(props)
	}

	handleClick(e, connection_id){		
		e.preventDefault()
		this.props.pusherActions.sendAuthorization( connection_id, this.props.authorization, this.props.me )
		this.props.uiActions.closeModal()
		return false;
	}

	renderConnectionsList(){
		var connections = []
		for (var connection_id in this.props.connections){
			if (this.props.connections.hasOwnProperty(connection_id) && connection_id !== this.props.connection_id){
				connections.push(this.props.connections[connection_id])
			}
		}

		if (connections.length <= 0){
			return <div className="no-results">No connections available</div>
		} else {
			return (
				<div className="list small pusher-connection-list">
					{
						connections.map( (connection, index) => {
							return (
								<div className='list-item connection' key={connection.connection_id} onClick={ e => this.handleClick(e, connection.connection_id) }>
									{ connection.username }
									&nbsp;
									<span className="grey-text">({ connection.ip })</span>
								</div>
							);
						})
					}
				</div>
			)
		}
	}

	render(){
		return (
			<div>
				<h1>Share Spotify authentication</h1>
				<h2 className="grey-text">Send your authentication tokens to another client. When the recipient client imports this, their Iris will have full access to your Spotify account ({this.props.me.id}).</h2>
				{this.renderConnectionsList()}
			</div>
		)
	}
}

const mapStateToProps = (state, ownProps) => {
	return {
		me: state.spotify.me,
		authorization: state.spotify.authorization,
		connection_id: state.pusher.connection_id,
		connections: state.pusher.connections
	}
}

const mapDispatchToProps = (dispatch) => {
	return {
		uiActions: bindActionCreators(uiActions, dispatch)
	}
}

export default connect(mapStateToProps, mapDispatchToProps)(AuthorizationModal_Send)