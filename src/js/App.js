
import React, { PropTypes } from 'react';
import ReactDOM from 'react-dom';
import { createStore, bindActionCreators } from 'redux'
import { hashHistory, Link } from 'react-router'
import { connect } from 'react-redux'

import Sidebar from './components/Sidebar'
import PlaybackControls from './components/PlaybackControls'
import SidebarToggleButton from './components/SidebarToggleButton'
import ContextMenu from './components/ContextMenu'
import Dragger from './components/Dragger'
import Modal from './components/Modal/Modal'
import Notifications from './components/Notifications'
import DebugInfo from './components/DebugInfo'

import * as helpers from './helpers'
import * as coreActions from './services/core/actions'
import * as uiActions from './services/ui/actions'
import * as pusherActions from './services/pusher/actions'
import * as mopidyActions from './services/mopidy/actions'
import * as spotifyActions from './services/spotify/actions'


/**
 * Root level application
 **/
class App extends React.Component{

	constructor(props){
		super(props);
		this.handleKeyUp = this.handleKeyUp.bind(this);
		this.handleKeyDown = this.handleKeyDown.bind(this);
		this.handleWindowResize = this.handleWindowResize.bind(this);
	}

	componentWillMount(){
		window.addEventListener("keyup", this.handleKeyUp, false);
		window.addEventListener("keydown", this.handleKeyDown, false);
		window.addEventListener("resize", this.handleWindowResize, false);
	}

	componentWillUnmount(){
		window.removeEventListener("keyup", this.handleKeyUp, false);
		window.removeEventListener("keydown", this.handleKeyDown, false);
		window.removeEventListener("resize", this.handleWindowResize, false);
	}

	componentDidMount(){
		
		// Fire up our services
		this.props.coreActions.startServices()
		this.props.coreActions.getBroadcasts()

		// when we navigate to a new route
		hashHistory.listen( location => {

			// Hide our sidebar
			this.props.uiActions.toggleSidebar( false )

			// Unselect any tracks
			this.props.uiActions.setSelectedTracks([])
		});

		// Check our slim_mode
		this.handleWindowResize(null)
	}

	shouldTriggerShortcut(e){

		// When we're focussed on certian elements, don't fire any shortcuts
		// Typically form inputs
		var ignoreNodes = ['INPUT', 'TEXTAREA']
		if (ignoreNodes.indexOf(e.target.nodeName) > -1){
			return false
		}

		// Listen for standalone key codes
		let keyCodes = [27,32,191]
		if (keyCodes.indexOf(e.keyCode) > -1){
			e.preventDefault()
			return true
		}

		// Listen for key codes that require ctrl to be held		
		let keyCodesWithCtrl = [37,38,39,40]
		if (e.ctrlKey && keyCodesWithCtrl.indexOf(e.keyCode) > -1){
			e.preventDefault()
			return true
		}

		// Listen for key codes that require ctrl to be held		
		let keyCodesWithCtrlShift = [70]
		if (e.ctrlKey && e.shiftKey && keyCodesWithCtrlShift.indexOf(e.keyCode) > -1){
			e.preventDefault()
			return true
		}
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
				if (this.props.play_state == 'playing'){
					this.props.mopidyActions.pause()
					this.props.uiActions.createNotification('pause', 'shortcut', 'shortcut')
				}else{
					this.props.mopidyActions.play()
					this.props.uiActions.createNotification('play', 'shortcut', 'shortcut')
				}
				break

			case 27: // esc
				if (this.props.dragger && this.props.dragger.dragging){
					this.props.uiActions.dragEnd()
				}
				if (this.props.modal){
					this.props.uiActions.closeModal()
				}
				break

			case 40: // down
				if (e.ctrlKey && e.shiftKey){
					this.props.mopidyActions.setMute(true)
					this.props.uiActions.createNotification('volume-off', 'shortcut', 'shortcut')
				} else if (e.ctrlKey){
					var volume = this.props.volume
					if (volume !== 'false'){
						volume -= 5
						if (volume < 0) volume = 0
						this.props.mopidyActions.setVolume(volume)
						if (this.props.mute) this.props.mopidyActions.setMute(false)
						this.props.uiActions.createNotification('volume-down', 'shortcut', 'shortcut')
					}
				}
				break

			case 38: // up
				if (e.ctrlKey && e.shiftKey){
					this.props.mopidyActions.setVolume(100)
					if (this.props.mute) this.props.mopidyActions.setMute(false)
					this.props.uiActions.createNotification('volume-up', 'shortcut', 'shortcut')
				} else if (e.ctrlKey){
					var volume = this.props.volume
					if (volume !== 'false'){
						volume += 5
						if (volume > 100) volume = 100
						this.props.mopidyActions.setVolume(volume)
						if (this.props.mute) this.props.mopidyActions.setMute(false)
						this.props.uiActions.createNotification('volume-up', 'shortcut', 'shortcut')
					}
				}
				break

			case 37: // left
				if (e.ctrlKey && e.shiftKey){
					var new_position = this.props.play_time_position - 30000
					if (new_position < 0) new_position = 0
					this.props.mopidyActions.seek(new_position)
					this.props.uiActions.createNotification('fast-backward', 'shortcut', 'shortcut')
				} else if (e.ctrlKey){
					this.props.mopidyActions.previous()
					this.props.uiActions.createNotification('step-backward', 'shortcut', 'shortcut')
				}
				break

			case 39: // right
				if (e.ctrlKey && e.shiftKey){
					this.props.mopidyActions.seek(this.props.play_time_position + 30000)
					this.props.uiActions.createNotification('fast-forward', 'shortcut', 'shortcut')
				} else if (e.ctrlKey){
					this.props.mopidyActions.next()
					this.props.uiActions.createNotification('step-forward', 'shortcut', 'shortcut')
				}
				break

			case 70: // F
				if (e.ctrlKey && e.shiftKey){
					if (this.props.modal){
						this.props.uiActions.closeModal()
					} else {
						this.props.uiActions.openModal('kiosk_mode')
					}
				}
				break
		}
	}

	render(){
		var className = '';
		if (this.props.dragger && this.props.dragger.active) className += ' dragging'
		if (this.props.sidebar_open) className += ' sidebar-open'
		if (this.props.modal) className += ' modal-open'
		if (this.props.touch_dragging) className += ' touch-dragging'
		if (this.props.slim_mode) className += ' slim-mode'
		if (helpers.isTouchDevice()){
			className += ' touch'
		} else {
			className += ' notouch'
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
		        <Modal />
		        <Notifications 
		        	uiActions={this.props.uiActions} 
		        	notifications={this.props.notifications} 
		        	processes={this.props.processes}
		        	broadcasts={this.props.broadcasts} />
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
		touch_dragging: state.ui.touch_dragging,
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
		modal: state.ui.modal,
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
