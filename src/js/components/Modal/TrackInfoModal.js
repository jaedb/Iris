
import React, { PropTypes } from 'react'
import FontAwesome from 'react-fontawesome'

import Icon from '../Icon'
import ArtistSentence from '../ArtistSentence'
import * as helpers from '../../helpers'

export default class TrackInfoModal extends React.Component{

	constructor(props){
		super(props)
	}

	componentDidMount(){
		if (this.props.current_track && !this.props.current_track.lyrics){
			this.props.geniusActions.getTrackLyrics(this.props.current_track);
		}
	}

	componentWillReceiveProps(nextProps){
		if (nextProps.current_track && nextProps.current_track.uri !== this.props.current_track.uri && !nextProps.current_track.lyrics){
			this.props.geniusActions.getTrackLyrics(nextProps.current_track);
		}
	}

	render(){
		if (this.props.current_track){
			var track = this.props.current_track;
		} else {
			var track = null
		}

		return (
			<div>
				<h1>Track info</h1>
				{track ? <h2 className="grey-text">{track.name} by <ArtistSentence artists={track.artists} /></h2> : null}
				{track && track.lyrics ? <div className="lyrics" dangerouslySetInnerHTML={{__html: track.lyrics}}></div> : "No lyrics"}
			</div>
		)
	}
}