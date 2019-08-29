
import React from 'react';
import { connect } from 'react-redux';
import { bindActionCreators } from 'redux';
import ReactGA from 'react-ga';

import Modal from './Modal';
import Link from '../../components/Link';
import Icon from '../../components/Icon';
import ColourField from '../../components/Fields/ColourField';
import IconField from '../../components/Fields/IconField';
import TextField from '../../components/Fields/TextField';

import * as pusherActions from '../../services/pusher/actions';
import * as uiActions from '../../services/ui/actions';
import * as helpers from '../../helpers';

class EditCommand extends React.Component{

	constructor(props){
		super(props)
		this.state = {
			id: helpers.generateGuid(),
			icon: 'power_settings_new',
			name: '',
			colour: '',
			url: "https://"+window.location.hostname+"/broadlink/sendCommand/power/",
			method: 'GET',
			post_data: "",
			additional_headers: ""
		}
	}

	componentDidMount(){
		if (this.props.command){
			this.props.uiActions.setWindowTitle("Edit command");
			this.setState(this.props.command);
		} else {
			this.props.uiActions.setWindowTitle("Create command");
		}
	}

	handleSubmit(e){		
		e.preventDefault();	

		this.props.pusherActions.setCommand(this.state);

		window.history.back();

		// A bit hacky, but wait for a moment to allow the back navigation
		// and then scroll down to our commands list
		setTimeout(() => {
				helpers.scrollTo("#commands-setup");
			},
			10
		);

		return false;
	}

	handleDelete(e){
		this.props.pusherActions.removeCommand(this.state.id);
		window.history.back();

		// A bit hacky, but wait for a moment to allow the back navigation
		// and then scroll down to our commands list
		setTimeout(() => {
				helpers.scrollTo("#commands-setup");
			},
			10
		);
	}

	render(){

		var icons = [
			"power_settings_new",
			"eject",
			"grade",
			"query_builder",
			"settings_input_component",
			"settings_input_hdmi",
			"settings_input_svideo",
			"forward_5",
			"forward_10",
			"forward_30",
			"replay",
			"replay_5",
			"replay_10",
			"replay_30",
			"skip_next",
			"skip_previous",
			"play_arrow",
			"pause",
			"stop",
			"shuffle",
			"snooze",
			"volume_off",
			"volume_down",
			"volume_up",
			"arrow_left",
			"arrow_drop_up",
			"arrow_right",
			"arrow_drop_down",
			"done",
			"done_all",
			"add",
			"remove",
			"clear",
			"cast",
			"speaker",
			"speaker_group",
			"audiotrack",
			"videogame_asset",
			"computer",
			"tv"
		];

		return (
			<Modal className="modal--create-command">
				<h1>{this.props.command ? "Edit" : "Create"} command</h1>
				<form onSubmit={(e) => this.handleSubmit(e)}>

					<div className="field textarea white">
						<div className="name">
							Name
						</div>
						<div className="input">
							<TextField 
								name="name"
								value={this.state.name}
								onChange={value => this.setState({ name: value })}
							/>
						</div>
					</div>

					<div className="field radio white">
						<div className="name">
							Colour
						</div>
						<div className="input">
							<ColourField
								colour={this.state.colour}
								onChange={colour => this.setState({colour: colour})}
							/>
						</div>
					</div>

					<div className="field radio white">
						<div className="name">
							Icon
						</div>
						<div className="input">
							<IconField
								icon={this.state.icon}
								icons={icons}
								onChange={icon => this.setState({icon: icon})}
							/>
						</div>
					</div>

					<div className="field textarea white">
						<div className="name">
							URL
						</div>
						<div className="input">
							<TextField 
								name="url"
								value={this.state.url}
								onChange={value => this.setState({ url: value })}
							/>
						</div>
					</div>

					<div className="field radio white">
						<div className="name">
							Method
						</div>
						<div className="input">
							<label>
								<input 
									type="radio"
									name="method"
									value="GET"
									checked={ this.state.method == 'GET' }
									onChange={ e => this.setState({ method: e.target.value })} />
								<span className="label">GET</span>
							</label>
							<label>
								<input 
									type="radio"
									name="method"
									value="POST"
									checked={ this.state.method == 'POST' }
									onChange={ e => this.setState({ method: e.target.value })} />
								<span className="label">POST</span>
							</label>
						</div>
					</div>

					{this.state.method == 'POST' && <div className="field textarea white">
						<div className="name">
							Data
						</div>
						<div className="input">
							<textarea 
								name="command"
								value={this.state.post_data}
								onChange={ e => this.setState({ post_data: e.target.value })}>
							</textarea>
						</div>
					</div>}

                    <div className="field textarea white">
					    <div className="name">
							Additional Headers
						</div>
						<div className="input">
							<textarea
								name="headers"
								value={this.state.additional_headers}
								onChange={ e => this.setState({ additional_headers: e.target.value })}>
							</textarea>
						</div>
					</div>


					<div className="actions centered-text">
						{this.props.command ? <button type="button" className="button button--destructive button--large" onClick={e => this.handleDelete(e)}>Delete</button> : null}
						<button type="submit" className="button button--primary button--large">Save</button>
					</div>

				</form>
			</Modal>
		)
	}
}

const mapStateToProps = (state, ownProps) => {
	var id = ownProps.match.params.id;
	return {
		id: id,
		command: (id && state.pusher.commands && state.pusher.commands[id] !== undefined ? state.pusher.commands[id] : null)
	}
}

const mapDispatchToProps = (dispatch) => {
	return {
		pusherActions: bindActionCreators(pusherActions, dispatch),
		uiActions: bindActionCreators(uiActions, dispatch)
	}
}

export default connect(mapStateToProps, mapDispatchToProps)(EditCommand)