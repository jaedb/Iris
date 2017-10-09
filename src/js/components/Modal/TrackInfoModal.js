
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

	renderLyrics(track){
		if (helpers.isLoading(this.props.load_queue,['genius_'])){
			return (
				<div className="body-loader loading">
					<div className="loader"></div>
				</div>
			);
		} else if (!track){
			return <div className="lyrics">Could not load track</div>
		} else if (!track.lyrics){
			return <div className="lyrics">No lyrics available</div>
		} else {
			return <div className="lyrics" dangerouslySetInnerHTML={{__html: track.lyrics}}></div>
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
				{this.renderLyrics(track)}
			</div>
		)
	}
}