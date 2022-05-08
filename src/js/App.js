import React, { useState, useEffect } from 'react';
import { Route, Switch, useLocation, useHistory } from 'react-router-dom';
import { useSelector, useDispatch } from 'react-redux';
import ReactGA from 'react-ga';
import { DndProvider } from 'react-dnd';
import { HTML5Backend } from 'react-dnd-html5-backend';
import { TouchBackend } from 'react-dnd-touch-backend';
import * as Sentry from '@sentry/browser';

import Sidebar from './components/Sidebar';
import PlaybackControls from './components/PlaybackControls';
import ContextMenu from './components/ContextMenu/ContextMenu';
import Notifications from './components/Notifications';
import ResizeListener from './components/ResizeListener';
import Hotkeys from './components/Hotkeys';
import DebugInfo from './components/DebugInfo';
import ErrorMessage from './components/ErrorMessage';
import Stream from './components/Stream';

import Album from './views/Album';
import Artist from './views/Artist/Artist';
import Playlist from './views/Playlist';
import User from './views/User';
import Track from './views/Track';
import UriRedirect from './views/UriRedirect';
import Queue from './views/Queue';
import QueueHistory from './views/QueueHistory';
import Debug from './views/Debug';
import Search from './views/Search';
import Settings from './views/Settings';
import Discover from './views/Discover/Discover';
import Library from './views/Library/Library';
import ModalStateListener from './components/ModalStateListener';
import Modals from './views/Modals/Modals';

import { scrollTo, isTouchDevice } from './util/helpers';
import * as uiActions from './services/ui/actions';
import * as pusherActions from './services/pusher/actions';
import * as mopidyActions from './services/mopidy/actions';
import * as spotifyActions from './services/spotify/actions';
import * as snapcastActions from './services/snapcast/actions';
import MediaSession from './components/MediaSession';
import ErrorBoundary from './components/ErrorBoundary';

const App = () => {
  const [isReady, setIsReady] = useState(false);
  const [hasInteracted, setHasInteracted] = useState(false);
  const { pathname, state: { scroll_position } = {} } = useLocation();
  const dispatch = useDispatch();
  const history = useHistory();

  const language = useSelector((state) => state.ui.language);
  window.language = language;
  const theme = useSelector((state) => state.ui.theme);
  const wide_scrollbar_enabled = useSelector((state) => state.ui.wide_scrollbar_enabled);
  const hide_scrollbars = useSelector((state) => state.ui.hide_scrollbars);
  const smooth_scrolling_enabled = useSelector((state) => state.ui.smooth_scrolling_enabled);
  const hotkeys_enabled = useSelector((state) => state.ui.hotkeys_enabled);
  const allow_reporting = useSelector((state) => state.ui.allow_reporting);
  const context_menu = useSelector((state) => state.ui.context_menu);
  const initial_setup_complete = useSelector((state) => state.ui.initial_setup_complete);
  const slim_mode = useSelector((state) => state.ui.slim_mode);
  const sidebar_open = useSelector((state) => state.ui.sidebar_open);
  const debug_info = useSelector((state) => state.ui.debug_info);
  const snapcast_enabled = useSelector((state) => state.snapcast.enabled);

  // Accept incoming preconfiguration via URL parameters and inject into application state.
  // For example: iris?snapcast={"enabled":true,"host":"myserver.local"}
  useEffect(() => {
    const params = new URLSearchParams(window.location.search);
    if (params) {
      params.forEach((value, key) => {
        try {
          const json = JSON.parse(value);
          switch (key) {
            case 'ui':
              dispatch(uiActions.set(json));
              console.info(`Applying preconfiguration for ${key}:`, value)
              break;
            case 'spotify':
              dispatch(spotifyActions.set(json));
              console.info(`Applying preconfiguration for ${key}:`, value)
              break;
            case 'mopidy':
              dispatch(mopidyActions.set(json));
              console.info(`Applying preconfiguration for ${key}:`, value)
              break;
            case 'pusher':
              dispatch(pusherActions.set(json));
              console.info(`Applying preconfiguration for ${key}:`, value)
              break;
            case 'snapcast':
              dispatch(snapcastActions.set(json));
              console.info(`Applying preconfiguration for ${key}:`, value)
              break;
            default:
              break;
          }
        } catch (e) {
          console.error('Preconfiguration failed', { e, key, value })
          return;
        }
      });
      // Wait a moment to allow actions to complete before we proceed
      setTimeout(
        () => setIsReady(true),
        250,
      );
    } else {
      setIsReady(true);
    }
  }, []);

  useEffect(() => {
    window.addEventListener('beforeinstallprompt', handleInstallPrompt, false);
    window.addEventListener('focus', handleFocusAndBlur, false);
    window.addEventListener('blur', handleFocusAndBlur, false);

    return () => {
      window.removeEventListener('beforeinstallprompt', handleInstallPrompt, false);
      window.removeEventListener('focus', handleFocusAndBlur, false);
      window.removeEventListener('blur', handleFocusAndBlur, false);
    };
  }, []);

  // Primary engines
  useEffect(() => {
    if (isReady) {
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
                || message.match(/NotSupportedError: The element has no supported sources./i)
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

      dispatch(mopidyActions.connect());
      dispatch(pusherActions.connect());
      dispatch(uiActions.getBroadcasts());
      if (snapcast_enabled) {
        dispatch(snapcastActions.connect());
      }
      if (!initial_setup_complete) {
        history.push('/modal/initial-setup');
      }
    }
  }, [isReady]);

  // Path changed (aka app navigation)
  useEffect(() => {
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

    dispatch(uiActions.toggleSidebar(false));
    dispatch(uiActions.setSelectedTracks([]));
    if (context_menu) {
      dispatch(uiActions.hideContextMenu());
    }
  }, [pathname]);

  /**
   * Using Visibility API, detect whether the browser is in focus or not
   *
   * This is used to keep background requests lean, preventing a queue of requests building up for
   * when focus is retained. Seems most obvious on mobile devices with Chrome as it has throttled
   * quota significantly: https://developers.google.com/web/updates/2017/03/background_tabs
   *
   * @param e Event
   * */
  const handleFocusAndBlur = () => {
    dispatch(uiActions.setWindowFocus(document.hasFocus()));
  };

  const handleInstallPrompt = (e) => {
    e.preventDefault();
    console.log('Install prompt detected');
    dispatch(uiActions.installPrompt(e));
  };

  const handleInteraction = () => {
    if (hasInteracted) return;
    setHasInteracted(true);
  };

  let className = `${theme}-theme app-inner`;
  className += ` ${navigator.onLine ? 'online' : 'offline'}`;
  if (wide_scrollbar_enabled) {
    className += ' wide-scrollbar';
  }
  if (sidebar_open) {
    className += ' sidebar-open';
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
      onClick={handleInteraction}
      onKeyDown={handleInteraction}
    >
      <DndProvider
        backend={isTouchDevice() ? TouchBackend : HTML5Backend}
        options={{ enableMouseEvents: true }}
      >
        <div className="body">
          <ModalStateListener />
          <Switch>
            <Route
              path="/modal"
              component={Modals}
            />
            <Route>
              <div>
                <Sidebar tabIndex="3" />
                <PlaybackControls tabIndex="2"/>

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
                    <Route
                      exact
                      path="/artist/:uri/:sub_view?"
                      component={Artist}
                    />
                    <Route exact path="/album/:uri/:name?" component={Album} />
                    <Route exact path="/playlist/:uri/:name?" component={Playlist} />
                    <Route exact path="/user/:uri/:name?" component={User} />
                    <Route exact path="/track/:uri/:name?" component={Track} />
                    <Route exact path="/uri/:uri/:name?" component={UriRedirect} />

                    <Route
                      path="/discover"
                      component={Discover}
                    />

                    <Route
                      path="/library"
                      component={Library}
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

        <ResizeListener />
        {hotkeys_enabled && <Hotkeys />}
        <ContextMenu />
        <Notifications />
        {hasInteracted && <ErrorBoundary silent><Stream /></ErrorBoundary>}
        {hasInteracted && ('mediaSession' in navigator) && <MediaSession />}
        {debug_info && <DebugInfo />}
      </DndProvider>
    </div>
  );
};

export default App;
