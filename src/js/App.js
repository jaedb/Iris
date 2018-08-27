
import React, { PropTypes } from 'react';
import ReactDOM from 'react-dom';
import { createStore, bindActionCreators } from 'redux';
import { hashHistory, Link } from 'react-router';
import { connect } from 'react-redux';
import ReactGA from 'react-ga';

import Sidebar from './components/Sidebar';;
import PlaybackControls from './components/PlaybackControls';
import ContextMenu from './components/ContextMenu';
import Dragger from './components/Dragger';
import Notifications from './components/Notifications';
import DebugInfo from './components/DebugInfo';

import * as helpers from './helpers';
import * as coreActions from './services/core/actions';
import * as uiActions from './services/ui/actions';
import * as pusherActions from './services/pusher/actions';
import * as mopidyActions from './services/mopidy/actions';
import * as spotifyActions from './services/spotify/actions';


/**
 * Root level application
 **/
class App extends React.Component{

	constructor(props){
		super(props);
		this.handleKeyUp = this.handleKeyUp.bind(this);
		this.handleKeyDown = this.handleKeyDown.bind(this);
		this.handleWindowResize = this.handleWindowResize.bind(this);
		this.handleInstallPrompt = this.handleInstallPrompt.bind(this);
	}

	componentWillMount(){
		window.addEventListener("keyup", this.handleKeyUp, false);
		window.addEventListener("keydown", this.handleKeyDown, false);
		window.addEventListener("resize", this.handleWindowResize, false);
		window.addEventListener("beforeinstallprompt", this.handleInstallPrompt, false);
	}

	componentWillUnmount(){
		window.removeEventListener("keyup", this.handleKeyUp, false);
		window.removeEventListener("keydown", this.handleKeyDown, false);
		window.removeEventListener("resize", this.handleWindowResize, false);
		window.removeEventListener("beforeinstallprompt", this.handleInstallPrompt, false);
	}

	componentDidMount(){

		if (this.props.allow_reporting){
			ReactGA.initialize('UA-64701652-3');

			/*
			if (Raven !== undefined){
				Raven.config('https://ca99fb6662fe40ae8ec4c18a466e4b4b@sentry.io/219026').install();
			}
			*/
		}
		
		// Fire up our services
		this.props.mopidyActions.connect();
		this.props.pusherActions.connect();
		this.props.coreActions.getBroadcasts();

		// when we navigate to a new route
		hashHistory.listen(location => {
			
	    	// Log our pageview
			if (this.props.allow_reporting){
				ReactGA.set({ page: window.location.hash });
				ReactGA.pageview(window.location.hash);
			}

			// Hide our sidebar
			this.props.uiActions.toggleSidebar(false )

			// Unselect any tracks
			this.props.uiActions.setSelectedTracks([]);

			// Close context menu
			if (this.props.context_menu){
				this.props.uiActions.hideContextMenu();
			}
		});

		// Check our slim_mode
		this.handleWindowResize(null);

		// Check for url-parsed configuration values
		var url_vars = this.props.location.query;
		if (url_vars !== undefined && url_vars.host !== undefined && url_vars.port !== undefined){
			this.props.mopidyActions.set({
				host: url_vars.host,
				port: url_vars.port
			});
			hashHistory.push(global.baseURL);
		}

		// show initial setup if required
		if (!this.props.initial_setup_complete){
			hashHistory.push(global.baseURL+'initial-setup');
		}
	}

	componentWillReceiveProps(nextProps){

		// We've navigated to a new location
	    if (this.props.location.pathname !== nextProps.location.pathname){

			// Scroll to bottom, only if we've PUSHed to a new route
			// We also prevent scroll reset for any sub_view routes (like tabs, services, etc)
			if (nextProps.location.action == 'PUSH' && nextProps.params.sub_view === undefined){
				window.scrollTo(0, 0);
			}
		}
	}

	shouldTriggerShortcut(e){

		if (!this.props.shortkeys_enabled){
			return false;
		}

		// When we're focussed on certian elements, don't fire any shortcuts
		// Typically form inputs
		var ignoreNodes = ['INPUT', 'TEXTAREA'];
		if (ignoreNodes.indexOf(e.target.nodeName) > -1){
			return false;
		}

		// Listen for standalone key codes
		let keyCodes = [27,32,191];
		if (keyCodes.indexOf(e.keyCode) > -1){
			e.preventDefault();
			return true;
		}

		// Listen for key codes that require ctrl to be held		
		let keyCodesWithCtrl = [37,38,39,40];
		if ((e.ctrlKey || e.metaKey) && keyCodesWithCtrl.indexOf(e.keyCode) > -1){
			e.preventDefault();
			return true;
		}

		// Listen for key codes that require ctrl to be held		
		let keyCodesWithCtrlShift = [70];
		if ((e.ctrlKey || e.metaKey) && e.shiftKey && keyCodesWithCtrlShift.indexOf(e.keyCode) > -1){
			e.preventDefault();
			return true;
		}
	}

	handleInstallPrompt(e){
		e.preventDefault();
		console.log("Install prompt detected");
		this.props.uiActions.installPrompt(e);
	}

	handleWindowResize(e){
		var width = Math.max(document.documentElement.clientWidth, window.innerWidth || 0)
		var height = Math.max(document.documentElement.clientHeight, window.innerHeight || 0)

		if (width <= 800){
			if (!this.props.slim_mode){
				this.props.uiActions.setSlimMode(true)
			}
		} else {
			if (this.props.slim_mode){
				this.props.uiActions.setSlimMode(false)
			}
		}
	}

	handleKeyDown(e){
		this.shouldTriggerShortcut(e)
	}

	handleKeyUp(e){
		if (!this.shouldTriggerShortcut(e)){
			return
		}

		switch(e.keyCode){

			case 32: // spacebar
				if (e.ctrlKey || e.metaKey){
					this.props.mopidyActions.stop();
					this.props.uiActions.createNotification({content: 'stop', type: 'shortcut', key: 'shortcut', duration: 1});
				} else if (this.props.play_state == 'playing'){
					this.props.mopidyActions.pause();
					this.props.uiActions.createNotification({content: 'pause', type: 'shortcut', key: 'shortcut', duration: 1});
				} else {
					this.props.mopidyActions.play();
					this.props.uiActions.createNotification({content: 'play', type: 'shortcut', key: 'shortcut', duration: 1});
				}
				break;

			case 27: // esc
				if (this.props.dragger && this.props.dragger.dragging){
					this.props.uiActions.dragEnd();
				}
				break;

			case 40: // down
				if ((e.ctrlKey || e.metaKey) && e.shiftKey){
					this.props.mopidyActions.setMute(true);
					this.props.uiActions.createNotification({content: 'volume-off', type: 'shortcut', key: 'shortcut', duration: 1});
				} else if (e.ctrlKey){
					var volume = this.props.volume;
					if (volume !== 'false'){
						volume -= 5;
						if (volume < 0){
							volume = 0;
						}
						this.props.mopidyActions.setVolume(volume);
						if (this.props.mute){
							this.props.mopidyActions.setMute(false);
						}
					this.props.uiActions.createNotification({content: 'volume-down', type: 'shortcut', key: 'shortcut', duration: 1});
					}
				}
				break;

			case 38: // up
				if ((e.ctrlKey || e.metaKey) && e.shiftKey){
					this.props.mopidyActions.setVolume(100)
					if (this.props.mute){
						this.props.mopidyActions.setMute(false);
					}
					this.props.uiActions.createNotification({content: 'volume-up', type: 'shortcut', key: 'shortcut', duration: 1});
				} else if (e.ctrlKey || e.metaKey){
					var volume = this.props.volume
					if (volume !== 'false'){
						volume += 5;
						if (volume > 100){
							volume = 100
						}
						this.props.mopidyActions.setVolume(volume);
						if (this.props.mute){
							this.props.mopidyActions.setMute(false);
						}
					this.props.uiActions.createNotification({content: 'volume-up', type: 'shortcut', key: 'shortcut', duration: 1});
					}
				}
				break;

			case 37: // left
				if ((e.ctrlKey || e.metaKey) && e.shiftKey){
					var new_position = this.props.play_time_position - 30000;
					if (new_position < 0){
						new_position = 0;;
					}
					this.props.mopidyActions.seek(new_position);
					this.props.uiActions.createNotification({content: 'fast-backward', type: 'shortcut', key: 'shortcut', duration: 1});
				} else if (e.ctrlKey || e.metaKey){
					this.props.mopidyActions.previous();
					this.props.uiActions.createNotification({content: 'step-backward', type: 'shortcut', key: 'shortcut', duration: 1});
				}
				break;

			case 39: // right
				if ((e.ctrlKey || e.metaKey) && e.shiftKey){
					this.props.mopidyActions.seek(this.props.play_time_position + 30000);
					this.props.uiActions.createNotification({content: 'fast-forward', type: 'shortcut', key: 'shortcut', duration: 1});
				} else if (e.ctrlKey || e.metaKey){
					this.props.mopidyActions.next();
					this.props.uiActions.createNotification({content: 'step-forward', type: 'shortcut', key: 'shortcut', duration: 1});
				}
				break;

			case 70: // F
				if ((e.ctrlKey || e.metaKey) && e.shiftKey){
					hashHistory.push(global.baseURL+'modal/kiosk-mode');
				}
				break;
		}
	}

	render(){
		var className = this.props.theme+'-theme';
		if (this.props.dragger && this.props.dragger.active){
			className += ' dragging';
		}
		if (this.props.sidebar_open){
			className += ' sidebar-open';
		}
		if (this.props.touch_dragging){
			className += ' touch-dragging';
		}
		if (this.props.slim_mode){
			className += ' slim-mode';
		}
		if (helpers.isTouchDevice()){
			className += ' touch';
		} else {
			className += ' notouch';
		}

		return (
			<div className={className}>
			
				<div className="body">
			        <Sidebar />		        
			        <PlaybackControls />
			        <main>
			      		{this.props.children}
			        </main>
		        </div>

		        <ContextMenu />
		        <Dragger />
		        <Notifications 
		        	uiActions={this.props.uiActions} 
		        	spotifyActions={this.props.spotifyActions} 
		        	notifications={this.props.notifications} 
		        	processes={this.props.processes}
		        	broadcasts={this.props.broadcasts}
		        />

		        {this.props.debug_info ? <DebugInfo /> : null}
		        
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
	return {
		theme: state.ui.theme,
		shortkeys_enabled: state.ui.shortkeys_enabled,
		allow_reporting: state.ui.allow_reporting,
		touch_dragging: state.ui.touch_dragging,
		initial_setup_complete: state.ui.initial_setup_complete,
		slim_mode: state.ui.slim_mode,
		broadcasts: (state.ui.broadcasts ? state.ui.broadcasts : []),
		volume: (state.mopidy.volume ? state.mopidy.volume : false),
		notifications: (state.ui.notifications ? state.ui.notifications : []),
		processes: (state.ui.processes ? state.ui.processes : {}),
		load_queue: (state.ui.load_queue ? state.ui.load_queue : {}),
		mopidy_connected: state.mopidy.connected,
		spotify_authorized: state.spotify.authorization,
		play_state: state.mopidy.play_state,
		play_time_position: parseInt(state.mopidy.time_position),
		mute: state.mopidy.mute,
		sidebar_open: state.ui.sidebar_open,
		dragger: state.ui.dragger,
		context_menu: state.ui.context_menu,
		debug_info: state.ui.debug_info
	}
}

const mapDispatchToProps = (dispatch) => {
	return {
		coreActions: bindActionCreators(coreActions, dispatch),
		uiActions: bindActionCreators(uiActions, dispatch),
		pusherActions: bindActionCreators(pusherActions, dispatch),
		mopidyActions: bindActionCreators(mopidyActions, dispatch),
		spotifyActions: bindActionCreators(spotifyActions, dispatch)
	}
}

export default connect(mapStateToProps, mapDispatchToProps)(App)
