
import React from 'react'

import Icon from '../Icon'
import * as helpers from '../../helpers'

export default class EditPlaylistModal extends React.Component{

	constructor(props){
		super(props)
		this.state = {
			error: null,
			name: this.props.data.name,
			description: (this.props.data.description ? this.props.data.description : ''),
			is_public: this.props.data.is_public
		}
	}

	savePlaylist(e){		
		e.preventDefault();

		if (!this.state.name || this.state.name == ''){
			this.setState({error: 'Name is required'})
			return false
		} else {
			this.props.uiActions.savePlaylist(this.props.data.uri, this.state.name, this.state.is_public, this.state.description)
			this.props.uiActions.closeModal()
			return false
		}
	}

	renderFields(){
		switch (helpers.uriSource(this.props.data.uri)){

			case 'spotify':
				return (
					<div>
						<div className="field">
							<input 
								type="text"
								placeholder="Name"
								onChange={ e => this.setState({ name: e.target.value })} 
								value={ this.state.name } />
						</div>
						<div className="field">
							<input 
								type="text"
								placeholder="Description"
								onChange={ e => this.setState({ description: e.target.value })} 
								value={ this.state.description } />
						</div>
						<div className="field checkbox white">
							<label>
								<input 
									type="checkbox"
									name="playlist_private"
									checked={ this.state.is_public }
									onChange={ e => this.setState({ is_public: !this.state.is_public })} />
								<span className="label">Public</span>
							</label>
						</div>
					</div>
				)
				break

			default:
				return (
					<div>
						<div className="field">
							<input 
								type="text"
								placeholder="Name"
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
				<h1>Edit playlist</h1>
				{this.state.error ? <h3 className="red-text">{this.state.error}</h3> : null}
				<form onSubmit={(e) => this.savePlaylist(e)}>

					{this.renderFields()}

					<div className="actions centered-text">
						<button type="submit" className="primary wide">Save</button>
					</div>
				</form>
			</div>
		)
	}
}
