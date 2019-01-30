
import React, { PropTypes } from 'react';
import ReactDOM from 'react-dom';
import { createStore, bindActionCreators } from 'redux';
import { BrowserRouter, Route, Switch, Link } from "react-router-dom";
import { connect } from 'react-redux';
import ReactGA from 'react-ga';

import Sidebar from './components/Sidebar';;
import PlaybackControls from './components/PlaybackControls';
import ContextMenu from './components/ContextMenu';
import Dragger from './components/Dragger';
import Notifications from './components/Notifications';
import DebugInfo from './components/DebugInfo';
import ErrorBoundary from './components/ErrorBoundary';
import ErrorMessage from './components/ErrorMessage';

import Album from './views/Album';
import Artist from './views/Artist';
import Playlist from './views/Playlist';
import User from './views/User';
import Track from './views/Track';
import Queue from './views/Queue';
import QueueHistory from './views/QueueHistory';
import Debug from './views/Debug';
import Search from './views/Search';
import Settings from './views/Settings';

import DiscoverRecommendations from './views/discover/DiscoverRecommendations';
import DiscoverFeatured from './views/discover/DiscoverFeatured';
import DiscoverCategories from './views/discover/DiscoverCategories';
import DiscoverCategory from './views/discover/DiscoverCategory';
import DiscoverNewReleases from './views/discover/DiscoverNewReleases';

import LibraryArtists from './views/library/LibraryArtists';
import LibraryAlbums from './views/library/LibraryAlbums';
import LibraryTracks from './views/library/LibraryTracks';
import LibraryPlaylists from './views/library/LibraryPlaylists';
import LibraryBrowse from './views/library/LibraryBrowse';
import LibraryBrowseDirectory from './views/library/LibraryBrowseDirectory';

import EditPlaylist from './views/modals/EditPlaylist';
import CreatePlaylist from './views/modals/CreatePlaylist';
import EditRadio from './views/modals/EditRadio';
import AddToQueue from './views/modals/AddToQueue';
import InitialSetup from './views/modals/InitialSetup';
import KioskMode from './views/modals/KioskMode';
import ShareConfiguration from './views/modals/ShareConfiguration';
import AddToPlaylist from './views/modals/AddToPlaylist';
import ImageZoom from './views/modals/ImageZoom';
import EditCommand from './views/modals/EditCommand';

import * as helpers from './helpers';
import * as coreActions from './services/core/actions';
import * as uiActions from './services/ui/actions';
import * as pusherActions from './services/pusher/actions';
import * as mopidyActions from './services/mopidy/actions';
import * as spotifyActions from './services/spotify/actions';
import * as lastfmActions from './services/lastfm/actions';
import * as geniusActions from './services/genius/actions';
import * as snapcastActions from './services/snapcast/actions';

class App extends React.Component{

	constructor(props){
		super(props);
		this.handleKeyUp = this.handleKeyUp.bind(this);
		this.handleKeyDown = this.handleKeyDown.bind(this);
		this.handleWindowResize = this.handleWindowResize.bind(this);
		this.handleInstallPrompt = this.handleInstallPrompt.bind(this);
		this.handleFocusAndBlur = this.handleFocusAndBlur.bind(this);
	}

	componentWillMount(){
		window.addEventListener("keyup", this.handleKeyUp, false);
		window.addEventListener("keydown", this.handleKeyDown, false);
		window.addEventListener("resize", this.handleWindowResize, false);
		window.addEventListener("beforeinstallprompt", this.handleInstallPrompt, false);
		window.addEventListener("focus", this.handleFocusAndBlur, false);
		window.addEventListener("blur", this.handleFocusAndBlur, false);
	}

	componentWillUnmount(){
		window.removeEventListener("keyup", this.handleKeyUp, false);
		window.removeEventListener("keydown", this.handleKeyDown, false);
		window.removeEventListener("resize", this.handleWindowResize, false);
		window.removeEventListener("beforeinstallprompt", this.handleInstallPrompt, false);
		window.removeEventListener("focus", this.handleFocusAndBlur, false);
		window.removeEventListener("blur", this.handleFocusAndBlur, false);
	}

	componentDidMount(){

		if (this.props.allow_reporting){
			ReactGA.initialize('UA-64701652-3');
		}
		
		// Fire up our services
		this.props.mopidyActions.connect();
		this.props.pusherActions.connect();
		this.props.coreActions.getBroadcasts();

		// Check our slim_mode
		this.handleWindowResize(null);

		// Check for url-parsed configuration values
		var url_vars = this.props.location.query;
		if (url_vars){
			var has_values = false;
			var values = {};
			if (url_vars.host !== undefined){
				has_values = true;
				values.host = url_vars.host;
			}
			if (url_vars.port !== undefined){
				has_values = true;
				values.port = url_vars.port;
			}

			if (has_values){
				this.props.mopidyActions.set(values);

				// Allow 100ms for the action above to complete before we re-route
				setTimeout(
					() => {
						this.props.history.push('/');
					},
					100
				);
			}
		}

		// show initial setup if required
		if (!this.props.initial_setup_complete){
			this.props.history.push('/initial-setup');
		}
	}

	componentDidUpdate(prevProps){

		// When we have navigated to a new route
		if (this.props.location !== prevProps.location){

	    	// Log our pageview
			if (this.props.allow_reporting){
				ReactGA.set({ page: this.props.location.pathname });
				ReactGA.pageview(this.props.location.pathname);
			}

			// Scroll to top of <main>
			// This doesn't know the difference between forward or backward navigation
			// so isn't quite a right fit
			//document.getElementById('main').scrollTo(0, 0);

			// Hide our sidebar
			this.props.uiActions.toggleSidebar(false);

			// Unselect any tracks
			this.props.uiActions.setSelectedTracks([]);

			// Close context menu
			if (this.props.context_menu){
				this.props.uiActions.hideContextMenu();
			}
		};
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

	/**
	 * Using Visibility API, detect whether the browser is in focus or not
	 *
	 * This is used to keep background requests lean, preventing a queue of requests building up
	 * for when focus is retained. Seems most obvious on mobile devices with Chrome as it has throttled
	 * quota significantly: https://developers.google.com/web/updates/2017/03/background_tabs
	 *
	 * @param e Event
	 **/
	handleFocusAndBlur(e){
		this.props.uiActions.setWindowFocus(document.hasFocus());
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
					window.history.push('/modal/kiosk-mode');
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
		if (this.props.context_menu){
			className += ' context-menu-open';
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

					<Switch>

						<Route path="/initial-setup" component={InitialSetup} />
						<Route path="/kiosk-mode" component={KioskMode} />
						<Route path="/add-to-playlist/:uris" component={AddToPlaylist} />
						<Route path="/image-zoom" component={ImageZoom} />
						<Route path="/share-configuration" component={ShareConfiguration} />
						<Route path="/edit-command/:id?" component={EditCommand} />
						
						<Route path="/queue/radio" component={EditRadio} />
						<Route path="/queue/add-uri" component={AddToQueue} />
						<Route path="/playlist/create" component={CreatePlaylist} />
						<Route path="/playlist/:uri/edit" component={EditPlaylist} />

						<Route>
							<div>
								<Sidebar />		        
						        <PlaybackControls history={this.props.history} />
						        <main id="main" className="smooth-scroll">
									<Switch>
										<Route exact path="/" component={Queue} />

										<Route exact path="/queue" component={Queue} />
										<Route exact path="/queue/history" component={QueueHistory} />
										<Route exact path="/settings/debug" component={Debug} />
										<Route path="/settings" component={Settings} />
										
										<Route exact path="/search/(:type/:term)?" component={Search} />
										<Route exact path="/album/:uri" component={Album} />
										<Route exact path="/artist/:uri/:sub_view?" component={Artist} />
										<Route exact path="/playlist/:uri" component={Playlist} />
										<Route exact path="/user/:uri" component={User} />
										<Route exact path="/track/:uri" component={Track} />
							
										<Route exact path="/discover/recommendations/:seeds?" component={DiscoverRecommendations} />
										<Route exact path="/discover/featured" component={DiscoverFeatured} />
										<Route exact path="/discover/categories/:id" component={DiscoverCategory} />
										<Route exact path="/discover/categories" component={DiscoverCategories} />
										<Route exact path="/discover/new-releases" component={DiscoverNewReleases} />

										<Route exact path="/library/artists" component={LibraryArtists} />
										<Route exact path="/library/albums" component={LibraryAlbums} />
										<Route exact path="/library/tracks" component={LibraryTracks} />
										<Route exact path="/library/playlists" component={LibraryPlaylists} />
										<Route exact path="/library/browse" component={LibraryBrowse} />
										<Route exact path="/library/browse/:uri" component={LibraryBrowseDirectory} />

										<Route>
											<ErrorMessage type="not-found" title="Not found">
												<p>Oops, that link could not be found</p>
											</ErrorMessage>
										</Route>

					        		</Switch>
						        </main>
					        </div>
						</Route>
					</Switch>
			        
		        </div>

		        <ContextMenu />
		        <Dragger />
		        <Notifications 
		        	uiActions={this.props.uiActions} 
		        	spotifyActions={this.props.spotifyActions} 
		        	geniusActions={this.props.geniusActions} 
		        	lastfmActions={this.props.lastfmActions} 
		        	snapcastActions={this.props.snapcastActions} 
		        	notifications={this.props.notifications} 
		        	processes={this.props.processes}
		        	broadcasts={this.props.broadcasts}
		        />

		        {this.props.debug_info ? <DebugInfo /> : null}

	        </div>
		);
	}
}

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
		spotifyActions: bindActionCreators(spotifyActions, dispatch),
		lastfmActions: bindActionCreators(lastfmActions, dispatch),
		geniusActions: bindActionCreators(geniusActions, dispatch),
		snapcastActions: bindActionCreators(snapcastActions, dispatch)
	}
}

export default connect(mapStateToProps, mapDispatchToProps)(App)
