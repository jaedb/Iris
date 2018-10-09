
import React, { PropTypes } from 'react'
import { connect } from 'react-redux'
import { bindActionCreators } from 'redux'
import { Link } from 'react-router'
import ReactGA from 'react-ga'

import Modal from './Modal';
import Icon from '../../components/Icon';
import * as pusherActions from '../../services/pusher/actions';
import * as uiActions from '../../services/ui/actions';
import * as helpers from '../../helpers';

class EditCommand extends React.Component{

	constructor(props){
		super(props)
		this.state = {
			id: helpers.generateGuid(),
			icon: 'power_settings_new',
			command: '{"url":"https://myserver.local:8080/sendCommand/power"}'
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
			command: this.state.command
		});

		window.history.back();

		return false;
	}

	render(){
		return (
			<Modal className="modal--create-command">
				<h1>Create command</h1>
				<form onSubmit={(e) => this.handleSubmit(e)}>

					<div className="field text white">
						<div className="name">
							Icon
						</div>
						<div className="input">
							<input 
								type="text"
								name="icon"
								value={this.state.icon}
								onChange={ e => this.setState({ icon: e.target.value })}
							/>
							<div className="description">
								Material icon, see <a href="https://material.io/tools/icons/?style=baseline" target="_blank" noopener="true">docs</a>
							</div>
						</div>
					</div>

					<div className="field textarea white">
						<div className="name tooltip">
							Command
							<div className="tooltip__content">
								Ajax configuration (JSON string)
							</div>
						</div>
						<div className="input">
							<textarea 
								name="command"
								value={this.state.command}
								onChange={ e => this.setState({ command: e.target.value })}>
							</textarea>
						</div>
					</div>

					<div className="actions centered-text">
						<button type="submit" className="primary large">Save</button>
					</div>

				</form>
			</Modal>
		)
	}
}

const mapStateToProps = (state, ownProps) => {
	var id = ownProps.params.id;
	return {
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