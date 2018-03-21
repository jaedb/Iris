
import React, { PropTypes } from 'react'
import { connect } from 'react-redux'
import { bindActionCreators } from 'redux'
import { Link, hashHistory } from 'react-router'
import FontAwesome from 'react-fontawesome'

import ArtistSentence from './ArtistSentence'
import * as helpers from '../helpers'
import * as coreActions from '../services/core/actions'
import * as uiActions from '../services/ui/actions'
import * as pusherActions from '../services/pusher/actions'

class Snapcast extends React.Component{

	constructor(props){
		super(props)
	}

	componentDidMount(){
		if (this.props.pusher_connected){
			this.props.pusherActions.getSnapcast();
		}
	}

	componentWillReceiveProps(newProps){
		if (!this.props.pusher_connected && newProps.pusher_connected){
			this.props.pusherActions.getSnapcast();
		}
	}

	render(){
		if (!this.props.snapcast){
			return null;
		}

		return (
			<div className="snapcast">
				{
					this.props.snapcast.groups.map(group => {
						return (
							<div className="group" key={group.id}>
								{group.muted ? <FontAwesome name="volume-off" /> : <FontAwesome name="volume-up" />} {group.name ? group.name : 'Untitled'}
								<div className="clients">
									{
										group.clients.map(client => {
											return (
												<div key={client.id}>
													{client.config.volume.muted ? <FontAwesome name="volume-off" onClick={e => this.props.pusherActions.setSnapcastClientVolume(client.id, false, client.config.volume.percent)} /> : <FontAwesome name="volume-up" onClick={e => this.props.pusherActions.setSnapcastClientVolume(client.id, true, client.config.volume.percent)} />} {client.config.name} {client.config.volume.percent}
												</div>
											);
										})
									}
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
		pusher_connected: state.pusher.connected,
		snapcast: state.pusher.snapcast
	}
}

const mapDispatchToProps = (dispatch) => {
	return {
		coreActions: bindActionCreators(coreActions, dispatch),
		uiActions: bindActionCreators(uiActions, dispatch),
		pusherActions: bindActionCreators(pusherActions, dispatch)
	}
}

export default connect(mapStateToProps, mapDispatchToProps)(Snapcast)