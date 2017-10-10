
import React, { PropTypes } from 'react'

import Icon from '../Icon'
import * as helpers from '../../helpers'

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
							<span className="label">Name</span>
							<input 
								type="text"
								onChange={ e => this.setState({ name: e.target.value })} 
								value={ this.state.name } />
						</div>

						<div className="field text">
							<span className="label">Description</span>
							<input 
								type="text"
								onChange={ e => this.setState({ description: e.target.value })} 
								
								value={ this.state.description } />
						</div>

						<div className="field checkbox white">
							<span className="label">Options</span>
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
				)

			default:
				return (
					<div>
						<div className="field text">
							<span className="label">Name</span>
							<input 
								type="text"
								onChange={ e => this.setState({ name: e.target.value })} 
								value={ this.state.name } />
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
						<span className="label">Provider</span>
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
					
					{this.renderFields()}

					<div className="actions centered-text">
						<button type="submit" className="primary wide">Create</button>
					</div>

				</form>
			</div>
		)
	}
}