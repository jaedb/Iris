
import React, { PropTypes } from 'react'
import { connect } from 'react-redux'
import { Link } from 'react-router'
import { createStore, bindActionCreators } from 'redux'

import Modal from './Modal';
import Icon from '../../components/Icon';

import * as uiActions from '../../services/ui/actions';
import * as pusherActions from '../../services/pusher/actions';
import * as helpers from '../../helpers';

class ShareAuthorization_Send extends React.Component {

	constructor(props){
		super(props)
	}

	componentDidMount(){
		this.props.uiActions.setWindowTitle("Share Spotify authorization");
	}

	handleClick(e, connection_id){		
		e.preventDefault()
		this.props.pusherActions.deliverMessage(
			connection_id,
			'spotify_authorization_received',
			{
				authorization: this.props.authorization, 
				user: this.props.me
			}
		);
		window.history.back();
		return;
	}

	renderConnectionsList(){
		var connections = []
		for (var connection_id in this.props.connections){
			if (this.props.connections.hasOwnProperty(connection_id) && connection_id != this.props.connection_id){
				connections.push(this.props.connections[connection_id])
			}
		}

		if (connections.length <= 0){
			return <div className="no-results">No peer connections available</div>
		} else {
			return (
				<div className="list small pusher-connection-list">
					{
						connections.map((connection, index) => {
							return (
								<div className='list__item connection' key={connection.connection_id} onClick={ e => this.handleClick(e, connection.connection_id) }>
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
			<Modal className="modal--share-authorization">
				<h1>Share Spotify authentication</h1>
				<h2 className="grey-text">Send your authentication tokens to another client. When the recipient client imports this, their Iris will have full access to your Spotify account ({this.props.me.id}).</h2>
				{this.renderConnectionsList()}
			</Modal>
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
		pusherActions: bindActionCreators(pusherActions, dispatch),
		uiActions: bindActionCreators(uiActions, dispatch)
	}
}

export default connect(mapStateToProps, mapDispatchToProps)(ShareAuthorization_Send)