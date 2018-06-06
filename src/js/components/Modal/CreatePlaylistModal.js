
import React, { PropTypes } from 'react';;

import Icon from '../Icon';
import * as helpers from '../../helpers';

export default class CreatePlaylistModal extends React.Component{

	constructor(props){
		super(props)
		this.state = {
			name: '',
			description: '',
			scheme: 'spotify',
			is_public: true,
			is_collaborative: false
		}
	}

	createPlaylist(e){		
		e.preventDefault();	

		if (!this.state.name || this.state.name == ''){
			this.setState({error: 'Name is required'})
			return false
		} else {	
			this.props.coreActions.createPlaylist(this.state.scheme, this.state.name, this.state.description, this.state.is_public, this.state.is_collaborative )
			this.props.uiActions.closeModal()
		}

		return false
	}

	renderFields(){
		switch (this.state.scheme){

			case 'spotify':
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

						<div className="field text">
							<div className="name">Description</div>
							<div className="input">
								<input 
									type="text"
									onChange={ e => this.setState({ description: e.target.value })} 
									
									value={ this.state.description } />
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
										name="is_public"
										checked={ this.state.is_public }
										onChange={ e => this.setState({ is_public: !this.state.is_public })} />
									<span className="label">Public</span>
								</label>
								<label>
									<input 
										type="checkbox"
										name="is_collaborative"
										checked={ this.state.is_collaborative }
										onChange={ e => this.setState({ is_collaborative: !this.state.is_collaborative })} />
									<span className="label">Collaborative</span>
								</label>
							</div>
						</div>
					</div>
				)

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
			<div>
				<h1>Create playlist</h1>
				<form onSubmit={(e) => this.createPlaylist(e)}>

					<div className="field radio white">
						<div className="name">
							Provider
						</div>
						<div className="input">
							<label>
								<input 
									type="radio"
									name="scheme"
									value="spotify"
									checked={ this.state.scheme == 'spotify' }
									onChange={ e => this.setState({ scheme: e.target.value })} />
								<span className="label">Spotify</span>
							</label>
							<label>
								<input 
									type="radio"
									name="scheme"
									value="m3u"
									checked={ this.state.scheme == 'm3u' }
									onChange={ e => this.setState({ scheme: e.target.value })} />
								<span className="label">Mopidy</span>
							</label>
						</div>
					</div>
					
					{this.renderFields()}

					<div className="actions centered-text">
						<button type="submit" className="primary large">Create playlist</button>
					</div>

				</form>
			</div>
		)
	}
}