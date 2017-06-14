
import React, { PropTypes } from 'react'
import { connect } from 'react-redux'
import { Link } from 'react-router'
import { createStore, bindActionCreators } from 'redux'

import * as uiActions from '../../services/ui/actions'
import * as helpers from '../../helpers'

import Icon from '../Icon'
import Thumbnail from '../Thumbnail'
import ArtistSentence from '../ArtistSentence'

export default class KioskModeModal extends React.Component{

	constructor(props){
		super(props)
	}

	handleClick(e, connectionid){		
		e.preventDefault()
		this.props.uiActions.closeModal()
		return false;
	}

	render(){
		if (this.props.current_track && this.props.current_track.album && this.props.current_track.album.images){
			var images = this.props.current_track.album.images
		} else {
			var images = []
		}

		return (
			<div className="kisk-mode-modal">
			
				<Thumbnail className="background" images={images} />
				<Thumbnail className="foreground" images={images} />

				<div className="player">
					<div className="current-track">
						<div className="title">{ this.props.current_track ? this.props.current_track.name : <span>-</span> }</div>
						{ this.props.current_track ? <ArtistSentence artists={ this.props.current_track.artists } /> : <ArtistSentence /> }
					</div>
				</div>
			</div>
		)
	}
}