
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
		var data = {
			recipients: [connectionid],
			action: 'send_authorization',
			data: this.props.authorization
		}
		this.props.pusherActions.instruct( 'broadcast', data )
		this.props.uiActions.closeModal()
		return false;
	}

	render(){
		return (
			<div>
				<h4>Share Spotify authentication</h4>
				<div className="list pusher-connection-list">
					{
						this.props.connections.map( (connection, index) => {
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
		authorization: state.spotify.authorization,
		connections: state.pusher.connections
	}
}

const mapDispatchToProps = (dispatch) => {
	return {
		uiActions: bindActionCreators(uiActions, dispatch)
	}
}

export default connect(mapStateToProps, mapDispatchToProps)(SendAuthorizationModal)