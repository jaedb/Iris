
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
			public: this.props.data.public,
			collaborative: this.props.data.collaborative
		}
	}

	savePlaylist(e){		
		e.preventDefault();

		if (!this.state.name || this.state.name == ''){
			this.setState({error: 'Name is required'})
			return false
		} else {
			this.props.coreActions.savePlaylist(this.props.data.uri, this.state.name, this.state.description, this.state.public, this.state.collaborative)
			this.props.uiActions.closeModal()
			return false
		}
	}

	renderFields(){
		switch (helpers.uriSource(this.props.data.uri)){

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
				)
				break

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
