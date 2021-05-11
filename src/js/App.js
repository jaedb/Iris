import React from 'react';
import { bindActionCreators } from 'redux';
import { Route, Switch } from 'react-router-dom';
import { connect } from 'react-redux';
import ReactGA from 'react-ga';
import * as Sentry from '@sentry/browser';

import Sidebar from './components/Sidebar';
import PlaybackControls from './components/PlaybackControls';
import ContextMenu from './components/ContextMenu';
import Dragger from './components/Dragger';
import Notifications from './components/Notifications';
import ResizeListener from './components/ResizeListener';
import Hotkeys from './components/Hotkeys';
import DebugInfo from './components/DebugInfo';
import ErrorMessage from './components/ErrorMessage';
import Stream from './components/Stream';

import Album from './views/Album';
import Artist from './views/Artist';
import Playlist from './views/Playlist';
import User from './views/User';
import Track from './views/Track';
import UriRedirect from './views/UriRedirect';
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
import HotkeysInfo from './views/modals/HotkeysInfo';
import EditCommand from './views/modals/EditCommand';
import Reset from './views/modals/Reset';

import { scrollTo, isTouchDevice } from './util/helpers';
import storage from './util/storage';
import * as coreActions from './services/core/actions';
import * as uiActions from './services/ui/actions';
import * as pusherActions from './services/pusher/actions';
import * as mopidyActions from './services/mopidy/actions';
import * as spotifyActions from './services/spotify/actions';
import * as lastfmActions from './services/lastfm/actions';
import * as geniusActions from './services/genius/actions';
import * as snapcastActions from './services/snapcast/actions';
import MediaSession from './components/MediaSession';
import ErrorBoundary from './components/ErrorBoundary';

export class App extends React.Component {
  constructor(props) {
    super(props);
    const { allow_reporting } = this.props;
    this.state = {
      userHasInteracted: false,
    };

    if (allow_reporting) {
      ReactGA.initialize('UA-64701652-3');
      Sentry.init({
        dsn: 'https://ca99fb6662fe40ae8ec4c18a466e4b4b@o99789.ingest.sentry.io/219026',
        sampleRate: 0.25,
        beforeSend: (event, hint) => {
          const {
            originalException: {
              message,
            } = {},
          } = hint;

          // Filter out issues that destroy our quota and that are not informative enough to
          // actually resolve.
          if (
            message
            && (
              message.match(/Websocket/i)
              || message.match(/NotSupportedError/i)
              || message.match(/Non-Error promise rejection captured with keys: call, message, value/i)
              || message.match(/Cannot read property 'addChunk' of undefined/i)
            )
          ) {
            return null;
          }

          return event;
        },
      });
    }

    // Load query param settings
    const configs = ['ui', 'spotify', 'pusher', 'snapcast', 'mopidy', 'google', 'lastfm', 'genius'];
    const params = new URLSearchParams(window.location.search);
    const changed = [];
    const urlParams = params.forEach((v, k) => {
      if (!configs.includes(k)) return;
      try {
        storage.set(k, JSON.parse(v));
        changed.push(k);
      } catch (error) {
        console.error(error);
      }
    });
    if (changed.length > 0) {
      changed.forEach((k) => params.delete(k));
      const url = window.location.toString().replace(window.location.search, params.toString());
      console.log(`Settings changed, redirecting to ${url}`, changed);
      window.location.assign(url);
    }

    this.handleInstallPrompt = this.handleInstallPrompt.bind(this);
    this.handleFocusAndBlur = this.handleFocusAndBlur.bind(this);
    window.language = props.language;
  }

  componentDidMount() {
    const {
      history,
      snapcast_enabled,
      initial_setup_complete,
      mopidyActions,
      pusherActions,
      snapcastActions,
    } = this.props;

    window.addEventListener(
      'beforeinstallprompt',
      this.handleInstallPrompt,
      false,
    );
    window.addEventListener('focus', this.handleFocusAndBlur, false);
    window.addEventListener('blur', this.handleFocusAndBlur, false);

    // Fire up our services
    mopidyActions.connect();
    pusherActions.connect();
    if (snapcast_enabled) {
      snapcastActions.connect();
    }
    uiActions.getBroadcasts();

    if (!initial_setup_complete) {
      history.push('/initial-setup');
    }
  }

  componentDidUpdate({
    location: {
      pathname: prevPathname,
    },
  }) {
    const {
      location: {
        pathname,
        state: {
          scroll_position,
        } = {},
      } = {},
      allow_reporting,
      uiActions,
      context_menu,
    } = this.props;

    // When we have navigated to a new route
    if (pathname !== prevPathname) {
      // Log our pageview
      if (allow_reporting) {
        ReactGA.set({ page: pathname });
        ReactGA.pageview(pathname);
      }

      // If the location has a "scroll_position" state variable, scroll to it.
      // This is invisibly injected to the history by the Link component when navigating, so
      // hitting back in the browser allows us to restore the position
      if (scroll_position) {
        scrollTo(parseInt(scroll_position, 10), false);
      }

      uiActions.toggleSidebar(false);
      uiActions.setSelectedTracks([]);
      if (context_menu) uiActions.hideContextMenu();
    }
  }

  componentWillUnmount() {
    window.removeEventListener(
      'beforeinstallprompt',
      this.handleInstallPrompt,
      false,
    );
    window.removeEventListener('focus', this.handleFocusAndBlur, false);
    window.removeEventListener('blur', this.handleFocusAndBlur, false);
  }

  /**
   * Using Visibility API, detect whether the browser is in focus or not
   *
   * This is used to keep background requests lean, preventing a queue of requests building up for
   * when focus is retained. Seems most obvious on mobile devices with Chrome as it has throttled
   * quota significantly: https://developers.google.com/web/updates/2017/03/background_tabs
   *
   * @param e Event
   * */
  handleFocusAndBlur() {
    const { uiActions: { setWindowFocus } } = this.props;
    setWindowFocus(document.hasFocus());
  }

  handleInstallPrompt(e) {
    const { uiActions: { installPrompt } } = this.props;
    e.preventDefault();
    console.log('Install prompt detected');
    installPrompt(e);
  }

  userInteracted = () => {
    const { userHasInteracted } = this.state;

    if (userHasInteracted) return;
    this.setState({ userHasInteracted: true });
  }

  render = () => {
    const {
      uiActions,
      location,
      history,
      debug_info,
      theme,
      dragging,
      touch_dragging,
      context_menu,
      sidebar_open,
      slim_mode,
      wide_scrollbar_enabled,
      hide_scrollbars,
      smooth_scrolling_enabled,
      hotkeys_enabled,
    } = this.props;
    const {
      userHasInteracted,
    } = this.state;

    let className = `${theme}-theme app-inner`;
    className += ` ${navigator.onLine ? 'online' : 'offline'}`;
    if (wide_scrollbar_enabled) {
      className += ' wide-scrollbar';
    }
    if (dragging) {
      className += ' dragging';
    }
    if (sidebar_open) {
      className += ' sidebar-open';
    }
    if (touch_dragging) {
      className += ' touch-dragging';
    }
    if (context_menu) {
      className += ' context-menu-open';
    }
    if (slim_mode) {
      className += ' slim-mode';
    }
    if (smooth_scrolling_enabled) {
      className += ' smooth-scrolling-enabled';
    }
    if (hide_scrollbars) {
      className += ' hide-scrollbars';
    }
    if (isTouchDevice()) {
      className += ' touch';
    } else {
      className += ' notouch';
    }

    return (
      <div
        className={className}
        onClick={userHasInteracted ? null : this.userInteracted}
        onKeyDown={userHasInteracted ? null : this.userInteracted}
      >
        <div className="body">
          <Switch>
            <Route path="/initial-setup" component={InitialSetup} />
            <Route path="/kiosk-mode" component={KioskMode} />
            <Route path="/add-to-playlist/:uris" component={AddToPlaylist} />
            <Route path="/image-zoom" component={ImageZoom} />
            <Route path="/hotkeys" component={HotkeysInfo} />
            <Route path="/share-configuration" component={ShareConfiguration} />
            <Route path="/reset" component={Reset} />
            <Route path="/edit-command/:id?" component={EditCommand} />

            <Route path="/queue/radio" component={EditRadio} />
            <Route path="/queue/add-uri" component={AddToQueue} />
            <Route path="/playlist/create/:uris?" component={CreatePlaylist} />
            <Route path="/playlist/:uri/edit" component={EditPlaylist} />

            <Route>
              <div>
                <Sidebar
                  location={location}
                  history={history}
                  tabIndex="3"
                />
                <PlaybackControls
                  history={history}
                  slim_mode={slim_mode}
                  tabIndex="2"
                />

                <main id="main" className="smooth-scroll" tabIndex="1">
                  <Switch>
                    <Route exact path="/" component={Queue} />

                    <Route exact path="/queue" component={Queue} />
                    <Route
                      exact
                      path="/queue/history"
                      component={QueueHistory}
                    />
                    <Route exact path="/settings/debug" component={Debug} />
                    <Route path="/settings" component={Settings} />

                    <Route
                      exact
                      path="/search/:type?/:term?"
                      component={Search}
                    />
                    <Route exact path="/album/:uri" component={Album} />
                    <Route
                      exact
                      path="/artist/:uri/:sub_view?"
                      component={Artist}
                    />
                    <Route exact path="/playlist/:uri" component={Playlist} />
                    <Route exact path="/user/:uri" component={User} />
                    <Route exact path="/track/:uri" component={Track} />
                    <Route exact path="/uri/:uri" component={UriRedirect} />

                    <Route
                      exact
                      path="/discover/recommendations/:uri?"
                      component={DiscoverRecommendations}
                    />
                    <Route
                      exact
                      path="/discover/featured"
                      component={DiscoverFeatured}
                    />
                    <Route
                      exact
                      path="/discover/categories/:uri"
                      component={DiscoverCategory}
                    />
                    <Route
                      exact
                      path="/discover/categories"
                      component={DiscoverCategories}
                    />
                    <Route
                      exact
                      path="/discover/new-releases"
                      component={DiscoverNewReleases}
                    />

                    <Route
                      exact
                      path="/library/artists"
                      component={LibraryArtists}
                    />
                    <Route
                      exact
                      path="/library/albums"
                      component={LibraryAlbums}
                    />
                    <Route
                      exact
                      path="/library/tracks"
                      component={LibraryTracks}
                    />
                    <Route
                      exact
                      path="/library/playlists"
                      component={LibraryPlaylists}
                    />
                    <Route
                      exact
                      path="/library/browse"
                      component={LibraryBrowse}
                    />
                    <Route
                      exact
                      path="/library/browse/:name/:uri"
                      component={LibraryBrowseDirectory}
                    />

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

        <ResizeListener
          uiActions={uiActions}
          slim_mode={slim_mode}
        />
        {hotkeys_enabled && <Hotkeys history={history} />}
        <ContextMenu />
        <Dragger />
        <Notifications />
        {userHasInteracted && <ErrorBoundary silent><Stream /></ErrorBoundary>}
        {userHasInteracted && ('mediaSession' in navigator) && <MediaSession />}
        {debug_info && <DebugInfo />}
      </div>
    );
  }
}

const mapStateToProps = (state) => {
  const {
    ui: {
      language,
      theme,
      wide_scrollbar_enabled,
      hide_scrollbars,
      smooth_scrolling_enabled,
      hotkeys_enabled,
      allow_reporting,
      touch_dragging,
      initial_setup_complete,
      slim_mode,
      sidebar_open,
      dragger: {
        active: dragging,
      } = {},
      context_menu,
      debug_info,
      test_mode,
    },
    snapcast: {
      enabled: snapcast_enabled,
    },
    spotify: {
      authorization: spotify_authorized,
    },
  } = state;

  return {
    language,
    theme,
    wide_scrollbar_enabled,
    hide_scrollbars,
    smooth_scrolling_enabled,
    hotkeys_enabled,
    allow_reporting,
    touch_dragging,
    initial_setup_complete,
    slim_mode,
    sidebar_open,
    dragging,
    context_menu,
    debug_info,
    test_mode,
    snapcast_enabled,
    spotify_authorized,
  };
};

const mapDispatchToProps = (dispatch) => ({
  coreActions: bindActionCreators(coreActions, dispatch),
  uiActions: bindActionCreators(uiActions, dispatch),
  pusherActions: bindActionCreators(pusherActions, dispatch),
  mopidyActions: bindActionCreators(mopidyActions, dispatch),
  spotifyActions: bindActionCreators(spotifyActions, dispatch),
  lastfmActions: bindActionCreators(lastfmActions, dispatch),
  geniusActions: bindActionCreators(geniusActions, dispatch),
  snapcastActions: bindActionCreators(snapcastActions, dispatch),
});

export default connect(
  mapStateToProps,
  mapDispatchToProps,
)(App);
