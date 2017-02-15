
import React, { PropTypes } from 'react'
import { connect } from 'react-redux'
import { bindActionCreators } from 'redux'

import Dropzone from './Dropzone'

import * as uiActions from '../services/ui/actions'
import * as mopidyActions from '../services/mopidy/actions'
import * as spotifyActions from '../services/spotify/actions'

class Dropzones extends React.Component{

	constructor(props) {
		super(props);

		this._zones = [
			{
				title: 'Add to queue',
				icon: 'play',
				action: 'enqueue'
			},
			{
				title: 'Play next',
				icon: 'play',
				action: 'enqueue_next'
			},
			{
				title: 'Add to playlist',
				icon: 'playlist',
				action: 'add_to_playlist',
				accepts: ['tltrack','track','album','playlist','artist']
			}
		]
	}

	handleMouseMove(e){
		if( !this.props.dragger || !this.props.dragger.active ) return null;
		this.props.uiActions.dragMove( e )
	}

	handleMouseUp(e, index){
		var target = this._zones[index]
		var victims = this.props.dragger.victims
		var uris = []
		for( var i = 0; i < victims.length; i++ ){
			uris.push( victims[i].uri )
		}

		switch( target.action ){
			case 'enqueue':
				this.props.mopidyActions.enqueueURIs( uris, this.props.dragger.from_uri )
				break

			case 'enqueue_next':
				this.props.mopidyActions.enqueueURIsNext( uris, this.props.dragger.from_uri )
				break

			case 'add_to_playlist':
				this.props.uiActions.openModal( 'add_to_playlist', { tracks_uris: uris } )
				break
		}
	}

	render(){
		if( !this.props.dragger || !this.props.dragger.active ) return null

		return (
			<div className="dropzones">
				{
					this._zones.map( (zone, index) => {
						return <Dropzone key={index} data={zone} handleMouseUp={ e => this.handleMouseUp(e, index) }/>
					})
				}
			</div>
		);
	}
}

const mapStateToProps = (state, ownProps) => {
	return {
		dragger: state.ui.dragger
	}
}

const mapDispatchToProps = (dispatch) => {
	return {
		uiActions: bindActionCreators(uiActions, dispatch),
		spotifyActions: bindActionCreators(spotifyActions, dispatch),
		mopidyActions: bindActionCreators(mopidyActions, dispatch)
	}
}

export default connect(mapStateToProps, mapDispatchToProps)(Dropzones)