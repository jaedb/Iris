
import React, { PropTypes } from 'react'

import Icon from '../Icon'
import SpotifyAuthenticationFrame from '../Fields/SpotifyAuthenticationFrame'
import LastfmAuthenticationFrame from '../Fields/LastfmAuthenticationFrame'

import * as helpers from '../../helpers'

export default class InitialSetupModal extends React.Component{
	constructor(props){
		super(props);

		this.state = {
			username: 'Anonymous',
			allow_reporting: this.props.allow_reporting,
			host: this.props.host,
			port: this.props.port,
			ssl: this.props.ssl
		}
	}

	handleSubmit(e){
		e.preventDefault();
		var self = this;

		// Force local username change, even if remote connection absent/failed
		this.props.pusherActions.setUsername(this.state.username, true);

		this.props.uiActions.set({
			initial_setup_complete: true,
			allow_reporting: this.state.allow_reporting
		});
		this.props.mopidyActions.setConfig({
			host: this.state.host,
			port: this.state.port,
			ssl: this.state.ssl
		});

		this.setState({saving: true});

		// Wait a second to allow changes to apply to store
		setTimeout(function(){

			// We've changed a connection setting, so need to reload
			if (self.state.host !== self.props.host || self.state.port !== self.props.port || self.state.ssl !== self.props.ssl){

				window.location.reload(true);

			// Safe to just close modal
			} else {
				self.props.uiActions.closeModal();
			}
		}, 1000);

		return false;
	}

	render(){
		return (
			<div>
				<h1>Get started</h1>
				<form onSubmit={(e) => this.handleSubmit(e)}>

					<div className="field text">
						<div className="name">
							Username
						</div>
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

					<div className="field">
						<div className="name">Host</div>
						<div className="input">
							<input 
								type="text"
								onChange={ e => this.setState({host: e.target.value})}
								value={ this.state.host } />
						</div>
					</div>
					<div className="field">
						<div className="name">Port</div>
						<div className="input">
							<input 
								type="text"
								onChange={ e => this.setState({port: e.target.value})}
								value={ this.state.port } />
						</div>
					</div>
					<div className="field checkbox">
						<div className="input">
							<label>
								<input 
									type="checkbox"
									name="ssl"
									checked={this.state.ssl}
									onChange={e => this.setState({ssl: !this.state.ssl})} />
								<span className="label has-tooltip">
									Enable SSL
									<span className="tooltip">Requires SSL proxy</span>
								</span>
							</label>
						</div>
					</div>
					
					<div className="field checkbox">
						<div className="input">
							<label>
								<input 
									type="checkbox"
									name="allow_reporting"
									checked={ this.state.allow_reporting }
									onChange={ e => this.setState({ allow_reporting: !this.state.allow_reporting })} />
								<span className="label">
									Allow reporting of anonymous usage statistics
								</span>
							</label>
						</div>
					</div>

					{!this.state.allow_reporting ? <p className="description">This anonymous usage data is important in identifying errors and potential features that make Iris better for everyone. Want to know more? Read the <a href="https://github.com/jaedb/Iris/wiki/Terms-of-use#privacy-policy" target="_blank">privacy policy</a>.</p> : null}

					<div className="actions centered-text">
						<button className={"primary large"+(this.state.saving ? " working" : "")} onClick={e => this.handleSubmit(e)}>
							{this.state.saving ? "Saving" : "Save"}
						</button>
					</div>

				</form>
			</div>
		)
	}
}