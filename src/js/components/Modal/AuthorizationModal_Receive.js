
import React, { PropTypes } from 'react';
import { connect } from 'react-redux';
import { Link } from 'react-router';
import { createStore, bindActionCreators } from 'redux';

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
		console.log(this.props.data)
		if (!this.props.data || !this.props.data.user || !this.props.data.authorization){
			return null
		}

		var user = this.props.data.user

		return (
			<div>
				<h1>Spotify authorization received</h1>
				<h2 className="grey-text">{user.display_name ? user.display_name : user.id} has shared their Spotify authorization with you. Importing this will overwrite your existing Spotify Authorization.</h2>
				
				<div className="user-details">
					<Thumbnail images={user.images} circle />
					<h2>{user.display_name ? user.display_name : user.id}</h2>
					<ul className="details">
						<li>{user.id}</li>
						<li>{user.followers.total} followers</li>
					</ul>
				</div>

				<div className="actions centered-text">
					<button className="large" onClick={e => this.props.uiActions.closeModal()}>Ignore</button>
					<button className="primary large" onClick={e => this.handleImport(e)}>Import authorization</button>
				</div>
			</div>
		)
	}
}