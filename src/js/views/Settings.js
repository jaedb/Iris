
import React, { PropTypes } from 'react'
import { connect } from 'react-redux'
import { bindActionCreators } from 'redux'

import FontAwesome from 'react-fontawesome'
import SpotifyAuthenticationFrame from '../components/SpotifyAuthenticationFrame'
import ConfirmationButton from '../components/ConfirmationButton'
import Header from '../components/Header'

import * as mopidyActions from '../services/mopidy/actions'
import * as spotifyActions from '../services/spotify/actions'

class Settings extends React.Component{

	constructor(props) {
		super(props);

		this.state = {
			mopidy_host: this.props.mopidy.host,
			mopidy_port: this.props.mopidy.port,
			spotify_country: this.props.spotify.country,
			spotify_locale: this.props.spotify.locale
		};
	}

	resetAllSettings(){
		localStorage.clear();
		window.location.reload(true);
	}

	setMopidyConfig(){
		this.props.mopidyActions.setConfig({ host: this.state.mopidy_host, port: this.state.mopidy_port });
		window.location.reload(true);
	}

	setSpotifyConfig(){
		this.props.spotifyActions.setConfig({ country: this.state.spotify_country, locale: this.state.spotify_locale });
	}

	render(){
		return (
			<div className="view settings-view">
				<Header
					icon="cog"
					title="Settings"
					/>

				<section>

					<h3 className="underline">Mopidy</h3>
					<form onSubmit={() => this.setMopidyConfig()}>
						<label>
							<span className="label">Host</span>
							<input onChange={ e => this.setState({ mopidy_host: e.target.value })} value={ this.state.mopidy_host } />
						</label>
						<label>
							<span className="label">Port</span>
							<input onChange={ e => this.setState({ mopidy_port: e.target.value })} value={ this.state.mopidy_port } />
						</label>
						<button type="submit">Apply</button>
					</form>

					<h3 className="underline">Spotify</h3>
					<form onSubmit={() => this.setSpotifyConfig()}>
						<label>
							<span className="label">Country</span>
							<input onChange={ e => this.setState({ spotify_country: e.target.value })} value={ this.state.spotify_country } />
						</label>
						<label>
							<span className="label">Locale</span>
							<input onChange={ e => this.setState({ spotify_locale: e.target.value })} value={ this.state.spotify_locale } />
						</label>
						<button type="submit">Apply</button>
					</form>

			        <SpotifyAuthenticationFrame />

			        <ConfirmationButton content="Reset all settings" confirmingContent="Are you sure?" onConfirm={() => this.resetAllSettings()} />

		        </section>
			</div>
		);
	}
}


/**
 * Export our component
 *
 * We also integrate our global store, using connect()
 **/

const mapStateToProps = (state, ownProps) => {
	return state;
}

const mapDispatchToProps = (dispatch) => {
	return {
		mopidyActions: bindActionCreators(mopidyActions, dispatch),
		spotifyActions: bindActionCreators(spotifyActions, dispatch)
	}
}

export default connect(mapStateToProps, mapDispatchToProps)(Settings)