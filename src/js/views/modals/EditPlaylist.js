
import React, { PropTypes } from 'react'
import { connect } from 'react-redux'
import { bindActionCreators } from 'redux'
import { Link } from 'react-router'
import ReactGA from 'react-ga'

import Modal from './Modal';
import Icon from '../../components/Icon';
import * as coreActions from '../../services/core/actions'
import * as uiActions from '../../services/ui/actions'
import * as mopidyActions from '../../services/mopidy/actions'
import * as spotifyActions from '../../services/spotify/actions'
import * as helpers from '../../helpers';

class EditPlaylist extends React.Component{

	constructor(props){
		super(props)
		this.state = {
			error: null,
			name: '',
			description: '',
			image: null,
			public: false,
			collaborative: false
		}
	}

	componentDidMount(){
		if (this.props.playlist){
			this.setState({
				name: this.props.playlist.name,
				description: this.props.playlist.description,
				public: (this.props.playlist.public == true),
				collaborative: (this.props.playlist.collaborative == true)
			});
		} else {
			switch (helpers.uriSource(this.props.params.uri)){

				case 'spotify':
					this.props.spotifyActions.getPlaylist(this.props.params.uri);
					this.props.spotifyActions.following(this.props.params.uri);
					break

				default:
					if (props.mopidy_connected){
						this.props.mopidyActions.getPlaylist(this.props.params.uri);
					}
					break
			}
		}
	}

	componentWillReceiveProps(nextProps){
		if (!this.props.playlist && nextProps.playlist){
			this.setState({
				name: nextProps.playlist.name,
				description: nextProps.playlist.description,
				public: (nextProps.playlist.public == true),
				collaborative: (nextProps.playlist.collaborative == true)
			});
		}
	}

	savePlaylist(e){	
		e.preventDefault();

		if (!this.state.name || this.state.name == ''){
			this.setState({error: 'Name is required'})
			return false
		} else {
			this.props.coreActions.savePlaylist(this.props.params.uri, this.state.name, this.state.description, this.state.public, this.state.collaborative, this.state.image);
			this.props.uiActions.closeModal();
			return false;
		}
	}

	setImage(e){
		var self = this;

		// Create a file-reader to import the selected image as a base64 string
		var file_reader = new FileReader();
    	
    	// Once the image is loaded, convert the result
		file_reader.addEventListener("load", function(e){
			var image_base64 = e.target.result.replace('data:image/jpeg;base64,','');
			self.setState({image: image_base64});
		}); 
		
		// This calls the filereader to load the file
		file_reader.readAsDataURL(e.target.files[0]);
	}

	renderFields(){
		switch (helpers.uriSource(this.props.params.uri)){

			case 'spotify':
				return (
					<div>
						<div className="field text">
							<div className="name">Name</div>
							<div className="input">
								<input 
									type="text"
									onChange={ e => this.setState({ name: e.target.value })} 
									value={ this.state.name }
								/>
							</div>
						</div>
						<div className="field text">
							<div className="name">Description</div>
							<div className="input">
								<input 
									type="text"
									onChange={ e => this.setState({ description: e.target.value })} 
									value={ this.state.description }
								/>
							</div>
						</div>
						<div className="field file">
							<div className="name">Cover image</div>
							<div className="input">
								<input 
									type="file"
									placeholder="Leave empty to keep existing image"
									onChange={e => this.setImage(e)}
								/>
								<div className="description">
									JPEG only, 256kB max. Leave empty to keep cover image unchanged.
								</div>
							</div>
						</div>
						<div className="field checkbox white">
							<div className="name">
								Options
							</div>
							<div className="input">
								<label>
									<input 
										type="checkbox"
										name="playlist_private"
										checked={ this.state.public }
										onChange={ e => this.setState({ public: !this.state.public })} />
									<span className="label">Public</span>
								</label>
								<label>
									<input 
										type="checkbox"
										name="collaborative"
										checked={ this.state.collaborative }
										onChange={ e => this.setState({ collaborative: !this.state.collaborative })} />
									<span className="label">Collaborative</span>
								</label>
							</div>
						</div>
					</div>
				)
				break

			default:
				return (
					<div>
						<div className="field text">
							<div className="name">Name</div>
							<div className="input">
								<input 
									type="text"
									onChange={ e => this.setState({ name: e.target.value })} 
									value={ this.state.name } />
							</div>
						</div>
					</div>
				)
		}
	}

	render(){
		return (
			<Modal className="edit-playlist-modal">
				<h1>Edit playlist</h1>
				{this.state.error ? <h3 className="red-text">{this.state.error}</h3> : null}
				<form onSubmit={(e) => this.savePlaylist(e)}>

					{this.renderFields()}

					<div className="actions centered-text">
						<button type="submit" className="primary large">Save</button>
					</div>
				</form>
			</Modal>
		)
	}
}

const mapStateToProps = (state, ownProps) => {
	return {
		mopidy_connected: state.mopidy.connected,
		playlist: (state.core.playlists[ownProps.params.uri] !== undefined ? state.core.playlists[ownProps.params.uri] : null),
		playlists: state.core.playlists
	}
}

const mapDispatchToProps = (dispatch) => {
	return {
		coreActions: bindActionCreators(coreActions, dispatch),
		uiActions: bindActionCreators(uiActions, dispatch),
		mopidyActions: bindActionCreators(mopidyActions, dispatch),
		spotifyActions: bindActionCreators(spotifyActions, dispatch)
	}
}

export default connect(mapStateToProps, mapDispatchToProps)(EditPlaylist)
