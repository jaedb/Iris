
import React, { PropTypes } from 'react';
import { connect } from 'react-redux';
import { bindActionCreators } from 'redux';
import ReactGA from 'react-ga';

import Modal from './Modal';
import Link from '../../components/Link';
import Icon from '../../components/Icon';
import ColourField from '../../components/Fields/ColourField';
import IconField from '../../components/Fields/IconField';

import * as pusherActions from '../../services/pusher/actions';
import * as uiActions from '../../services/ui/actions';
import * as helpers from '../../helpers';

class EditCommand extends React.Component{

	constructor(props){
		super(props)
		this.state = {
			id: helpers.generateGuid(),
			icon: 'power_settings_new',
			colour: '',
			command: '{"url":"https://'+window.location.hostname+'/broadlink/sendCommand/power/"}'
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

		this.props.pusherActions.setCommand({
			id: this.state.id,
			icon: this.state.icon,
			colour: this.state.colour,
			command: this.state.command
		});

		window.history.back();

		// A bit hacky, but wait for a moment to allow the back navigation
		// and then scroll down to our commands list
		setTimeout(() => {
				helpers.scrollTo("commands-setup");
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
				helpers.scrollTo("commands-setup");
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
							Command
						</div>
						<div className="input">
							<textarea 
								name="command"
								value={this.state.command}
								onChange={ e => this.setState({ command: e.target.value })}>
							</textarea>
							<div className="description">
								Ajax request settings. See <a href="http://api.jquery.com/jquery.ajax/" target="_blank" noopener="true"><code>jquery.ajax</code> documentation</a>.
							</div>
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