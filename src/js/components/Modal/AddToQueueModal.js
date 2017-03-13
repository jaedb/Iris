
import React, { PropTypes } from 'react'
import FontAwesome from 'react-fontawesome'

import Icon from '../Icon'
import * as helpers from '../../helpers'

export default class AddToQueueModal extends React.Component{

	constructor(props){
		super(props)
		this.state = {
			uris: '',
			at_position: 'end'
		}
	}

	handleSubmit(e){
		var uris = this.state.uris.split(',')
		if (this.state.at_position == 'next'){
			this.props.mopidyActions.enqueueURIsNext(uris)
		}else{
			this.props.mopidyActions.enqueueURIs(uris)
		}
		this.props.uiActions.closeModal()
	}

	render(){
		return (
			<div>
				<h4>Add URI(s) to tracklist</h4>

				<form onSubmit={e => this.handleSubmit(e)}>
					<div className="field text">
						<input 
							type="text"
							placeholder="Comma-separated URIs"
							onChange={e => this.setState({uris: e.target.value})} 
							value={this.state.uris} />
					</div>

					<div className="field radio white">
						<label>
							<input 
								type="radio"
								name="at_position"
								value="end"
								checked={ this.state.at_position == 'end' }
								onChange={ e => this.setState({ at_position: e.target.value })} />
							<span className="label">Add to end</span>
						</label>
						<label>
							<input 
								type="radio"
								name="at_position"
								value="next"
								checked={ this.state.at_position == 'next' }
								onChange={ e => this.setState({ at_position: e.target.value })} />
							<span className="label">Add after current track</span>
						</label>
					</div>

					<div className="actions centered-text">
						<button type="submit" className="primary wide">Add</button>
					</div>
				</form>
			</div>
		)
	}
}