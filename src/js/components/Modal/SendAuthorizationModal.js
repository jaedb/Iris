
import React, { PropTypes } from 'react'
import { connect } from 'react-redux'
import { Link } from 'react-router'
import { createStore, bindActionCreators } from 'redux'

import * as uiActions from '../../services/ui/actions'
import * as helpers from '../../helpers'

import Icon from '../Icon'

class SendAuthorizationModal extends React.Component{

	constructor(props){
		super(props)
	}

	handleClick(e, connectionid){		
		e.preventDefault()
		this.props.pusherActions.sendAuthorization( connectionid, this.props.authorization, this.props.me )
		this.props.uiActions.closeModal()
		return false;
	}

	render(){
		return (
			<div>
				<h4 className="no-bottom-padding">Share Spotify authentication</h4>
				<h3 className="grey-text">Send your authentication tokens to another client. When the recipient client imports this, their Iris will have full access to your Spotify account ({this.props.me.id}).</h3>
				<div className="list pusher-connection-list">
					{
						this.props.connections.map( (connection, index) => {
							
							// don't list OUR connection
							if (connection.connectionid == this.props.connectionid) return null

							return (
								<div className='list-item connection' key={connection.connectionid} onClick={ e => this.handleClick(e, connection.connectionid) }>
									{ connection.username }
									&nbsp;
									<span className="grey-text">({ connection.ip })</span>
								</div>
							);
						})
					}
				</div>
			</div>
		)
	}
}

const mapStateToProps = (state, ownProps) => {
	return {
		me: state.spotify.me,
		authorization: state.spotify.authorization,
		connectionid: state.pusher.connectionid,
		connections: state.pusher.connections
	}
}

const mapDispatchToProps = (dispatch) => {
	return {
		uiActions: bindActionCreators(uiActions, dispatch)
	}
}

export default connect(mapStateToProps, mapDispatchToProps)(SendAuthorizationModal)