
import React, { PropTypes } from 'react';
import FontAwesome from 'react-fontawesome';

import Icon from '../Icon';
import * as helpers from '../../helpers';

export default class EditSnapcastClientModal extends React.Component{

	constructor(props){
		super(props);
		var client = this.props.clients[this.props.data.id];
		this.state = {
			id: client.id,
			name: client.config.name,
			latency: client.config.latency.toString()
		};
	}

	handleSave(e){
		this.props.pusherActions.setSnapcastClientName(this.state.id, this.state.name);
		this.props.pusherActions.setSnapcastClientLatency(this.state.id, parseInt(this.state.latency));
		this.props.uiActions.closeModal();
	}

	handleDelete(e){
		this.props.pusherActions.deleteSnapcastClient(this.state.id);
		this.props.uiActions.closeModal();
	}

	render(){
		return (
			<div>
				<h1>Edit speaker</h1>
				<h2 className="grey-text">Description</h2>

				<form>
					<div className="field text">
						<span className="label">Name</span>
						<input 
							type="text"
							onChange={e => this.setState({name: e.target.value})} 
							value={this.state.name} />
					</div>
					<div className="field text">
						<span className="label">Latency (ms)</span>
						<input 
							type="number"
							onChange={e => this.setState({latency: e.target.value})} 
							value={this.state.latency} />
					</div>
				</form>
				<form>
					<div className="actions centered-text">
						<button className="destructive large" onClick={e => this.handleDelete(e)}>Delete</button>
						<button className="primary large" onClick={e => this.handleSave(e)}>Save</button>
					</div>
				</form>
			</div>
		)
	}
}