
import React, { PropTypes } from 'react'

import Icon from '../Icon'
import * as helpers from '../../helpers'

export default class InitialSetupModal extends React.Component{
	constructor(props){
		super(props);

		this.state = {
			username: 'Anonymous'
		}
	}

	handleSubmit(e){
		// save and falseify show_initial_setup
	}

	handleCancel(e){
		// falseify show_initial_setup
	}

	render(){
		return (
			<div>
				<h1>Welcome to Iris</h1>
				<form onSubmit={(e) => this.handleSubmit(e)}>

					<div className="field">
						<div className="name">Username</div>
						<div className="input">
							<input 
								type="text"
								onChange={e => this.setState({username: e.target.value.replace(/\W/g, '')})}
								value={this.state.username } />
							<div className="description">
								A non-unique string used to identify this client (no special characters)
							</div>
						</div>
					</div>

					<div className="actions centered-text">
						<button className="large" onClick={e => this.handleCancel(e)}>Skip</button>
						<button className="primary large" onClick={e => this.handleSubmit(e)}>Save</button>
					</div>

				</form>
			</div>
		)
	}
}