
import React, { PropTypes } from 'react'
import FontAwesome from 'react-fontawesome'

import Icon from '../Icon'
import * as helpers from '../../helpers'

export default class AddToQueueModal extends React.Component{

	constructor(props){
		super(props)
		this.state = {
			uris: '',
			next: false
		}
	}

	handleSubmit(e){
		var uris = this.state.uris.split(',')
		console.log(this.state)
		this.props.mopidyActions.enqueueURIs(uris, null, this.state.next)
		this.props.uiActions.closeModal()
	}

	render(){
		return (
			<div>
				<h4>Add URI(s) to queue</h4>

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
								name="next"
								checked={!this.state.next}
								onChange={e => this.setState({next: false})} />
							<span className="label">Add to end</span>
						</label>
						<label>
							<input 
								type="radio"
								name="next"
								checked={this.state.next}
								onChange={e => this.setState({next: true})} />
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