
import React, { PropTypes } from 'react';
import { connect } from 'react-redux';
import Link from '../../components/Link'
import { createStore, bindActionCreators } from 'redux';

import Modal from './Modal';
import Thumbnail from '../../components/Thumbnail';
import ArtistSentence from '../../components/ArtistSentence';
import Dater from '../../components/Dater';
import Icon from '../../components/Icon';
import ProgressSlider from '../../components/Fields/ProgressSlider';

import * as helpers from '../../helpers';
import * as uiActions from '../../services/ui/actions';
import * as mopidyActions from '../../services/mopidy/actions';

class KioskMode extends React.Component{

	constructor(props){
		super(props)
	}

	componentDidMount(){
		this.setWindowTitle();
	}

	componentWillReceiveProps(nextProps){
		if (!this.props.current_track && nextProps.current_track){
			this.setWindowTitle(nextProps.current_track);
		}
	}

	setWindowTitle(current_track = this.props.current_track){		
		if (current_track){
			var artists = "";
			for (var i = 0; i < current_track.artists.length; i++){
				if (artists != ""){
					artists += ", ";
				}
				artists += current_track.artists[i].name;
			}
			this.props.uiActions.setWindowTitle(current_track.name+" by "+artists+" (now playing)");
		} else{
			this.props.uiActions.setWindowTitle("Now playing");
		}
	}

	togglePlay(e){
		if (this.props.play_state == 'playing'){
			this.props.mopidyActions.pause();
		} else {
			this.props.mopidyActions.play();
		}
	}

	render(){
		if (this.props.current_track && this.props.current_track.images){
			var images = this.props.current_track.images
		} else {
			var images = []
		}

		return (
			<Modal className="modal--kiosk-mode">
			
				<Thumbnail className="background" images={images} />

				<div className="artwork" onClick={e => this.togglePlay(e)}>
					<Thumbnail images={images} />
					{this.props.play_state == 'playing' ? <Icon name="pause_circle_filled" /> : <Icon name="play_circle_filled" />}
				</div>

				<div className="player">
					<div className="current-track">
						<div className="title">{ this.props.current_track ? this.props.current_track.name : <span>-</span> }</div>
						{ this.props.current_track ? <ArtistSentence nolinks artists={ this.props.current_track.artists } /> : <ArtistSentence /> }
					</div>

					<div className="progress-wrapper">
						<ProgressSlider />
					</div>
				</div>
			</Modal>
		)
	}
}

const mapStateToProps = (state) => {
	return {
		play_state: state.mopidy.play_state,
		current_track: (state.core.current_track && state.core.tracks[state.core.current_track.uri] !== undefined ? state.core.tracks[state.core.current_track.uri] : null),
	}
}

const mapDispatchToProps = (dispatch) => {
	return {
		uiActions: bindActionCreators(uiActions, dispatch),
		mopidyActions: bindActionCreators(mopidyActions, dispatch)
	}
}

export default connect(mapStateToProps, mapDispatchToProps)(KioskMode)