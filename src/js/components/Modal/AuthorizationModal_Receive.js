
import React, { PropTypes } from 'react'
import { connect } from 'react-redux'
import { Link } from 'react-router'
import { createStore, bindActionCreators } from 'redux'

import * as uiActions from '../../services/ui/actions'
import * as helpers from '../../helpers'

import Icon from '../Icon'
import Thumbnail from '../Thumbnail'

export default class extends React.Component{

	constructor(props){
		super(props)
	}

	handleImport(e){
		this.props.spotifyActions.importAuthorization(this.props.data)
		this.props.uiActions.closeModal()
	}

	render(){
		if (!this.props.data || !this.props.data.user || !this.props.data.authorization){
			return null
		}

		var user = this.props.data.user

		return (
			<div>
				<h4 className="no-bottom-padding">Spotify authorization received</h4>
				<h3 className="grey-text">{user.display_name ? user.display_name : user.id} has shared their Spotify authorization with you. Importing this will overwrite your existing Spotify Authorization.</h3>
				
				<div className="user-view">
					<div className="intro">
						<Thumbnail images={user.images} circle />
						<h1>{user.display_name ? user.display_name : user.id}</h1>
						<ul className="details">
							<li>{user.id}</li>
							<li>{user.followers.total} followers</li>
						</ul>
					</div>
				</div>

				<div className="actions centered-text">
					<button onClick={e => this.props.uiActions.closeModal()}>Ignore</button>
					<button className="primary" onClick={e => this.handleImport(e)}>Import</button>
				</div>
			</div>
		)
	}
}