import React, { useState, useEffect } from 'react';
import { connect } from 'react-redux';
import { bindActionCreators } from 'redux';
import { Route, Switch } from 'react-router-dom';
import ErrorMessage from '../components/ErrorMessage';
import Link from '../components/Link';
import Thumbnail from '../components/Thumbnail';
import Parallax from '../components/Parallax';
import FollowButton from '../components/Fields/FollowButton';
import ContextMenuTrigger from '../components/ContextMenuTrigger';
import About from './Artist/About';
import Overview from './Artist/Overview';
import Tracks from './Artist/Tracks';
import Related from './Artist/Related';
import Loader from '../components/Loader';
import * as coreActions from '../services/core/actions';
import * as uiActions from '../services/ui/actions';
import * as mopidyActions from '../services/mopidy/actions';
import {
  uriSource,
} from '../util/helpers';
import { encodeUri, decodeUri } from '../util/format';
import { arrayOf } from '../util/arrays';
import { i18n, I18n } from '../locale';
import Button from '../components/Button';
import {
  makeItemSelector,
  makeLoadingSelector,
} from '../util/selectors';

const Artist = ({
  uri,
  loading,
  history,
  artist: artistProp,
  albums,
  coreActions: {
    loadArtist,
  },
  uiActions: {
    setWindowTitle,
    showContextMenu,
    createNotification,
  },
  mopidyActions: {
    playURIs,
  },
}) => {
  const [artist, setArtist] = useState({});

  useEffect(
    () => {
      if (uri) loadArtist(uri, { full: true });
    },
    [uri],
  );

  useEffect(() => {
    if (artistProp) {
      setWindowTitle(i18n('artist.title_window', { name: artistProp.name }));
    } else {
      setWindowTitle(i18n('artist.title'));
    }
    setArtist(artistProp);
  }, [artistProp]);

  const onPlayAll = () => {
    const {
      tracks,
      albums_uris,
    } = artist;

    if ((!albums_uris || !albums_uris.length) && (!tracks || !tracks.length)) {
      createNotification({ content: i18n('errors.no_results'), level: 'warning' });
      return;
    }

    playURIs(arrayOf('uri', tracks) || albums_uris, uri);
  }

  const handleContextMenu = (e) => showContextMenu({
    e,
    context: 'artist',
    items: [artist],
    uris: [uri],
    tracklist_uri: uri,
  });

  if (loading) {
    return <Loader body loading />;
  }
  if (!artist) {
    return (
      <ErrorMessage type="not-found" title="Not found">
        <p>
          <I18n path="errors.uri_not_found" uri={uri} />
        </p>
      </ErrorMessage>
    );
  }

  const scheme = uriSource(uri);
  const image = (artist.images && artist.images.length) ? artist.images[0].huge : null;
  const is_spotify = (scheme === 'spotify');

  return (
    <div className="view artist-view preserve-3d">
      <div className="intro preserve-3d">

        <Parallax image={image} />

        <div className="liner">
          <div className="heading">
            <div className="heading__thumbnail">
              <Thumbnail size="medium" circle canZoom type="artist" image={image} />
            </div>
            <div className="heading__content">
              <h1>{artist && artist.name}</h1>
              <div className="actions">
                <Button
                  type="primary"
                  onClick={onPlayAll}
                  tracking={{ category: 'Artist', action: 'Play' }}
                >
                  <I18n path="actions.play" />
                </Button>
                {is_spotify && (
                  <FollowButton
                    uri={uri}
                    is_following={artist.in_library}
                  />
                )}
                <ContextMenuTrigger className="white" onTrigger={handleContextMenu} />
              </div>
            </div>
          </div>
          <div className="sub-views" id="sub-views-menu">
            <Link
              exact
              history={history}
              activeClassName="sub-views__option--active"
              className="sub-views__option"
              to={`/artist/${encodeUri(uri)}`}
              scrollTo="#sub-views-menu"
            >
              <h4><I18n path="artist.overview.title" /></h4>
            </Link>
            {artist.tracks && artist.tracks.length > 10 && (
              <Link
                exact
                history={history}
                activeClassName="sub-views__option--active"
                className="sub-views__option"
                to={`/artist/${encodeUri(uri)}/tracks`}
                scrollTo="#sub-views-menu"
              >
                <h4><I18n path="artist.tracks.title" /></h4>
              </Link>
            )}
            {artist.related_artists && (
              <Link
                exact
                history={history}
                activeClassName="sub-views__option--active"
                className="sub-views__option"
                to={`/artist/${encodeUri(uri)}/related-artists`}
                scrollTo="#sub-views-menu"
              >
                <h4><I18n path="artist.related_artists.title" /></h4>
              </Link>
            )}
            <Link
              exact
              history={history}
              activeClassName="sub-views__option--active"
              className="sub-views__option"
              to={`/artist/${encodeUri(uri)}/about`}
              scrollTo="#sub-views-menu"
            >
              <h4><I18n path="artist.about.title" /></h4>
            </Link>
          </div>
        </div>
      </div>
      <div className="content-wrapper">
        <Switch>
          <Route exact path="/artist/:id/related-artists">
            <Related artist={artist} />
          </Route>
          <Route exact path="/artist/:id/tracks">
            <Tracks artist={artist} />
          </Route>
          <Route exact path="/artist/:id/about">
            <About artist={artist} />
          </Route>
          <Route exact path="/artist/:id/:name?">
            <Overview artist={artist} albums={albums} />
          </Route>
        </Switch>
      </div>
    </div>
  );
}

const mapStateToProps = (state, props) => {
  const uri = decodeUri(props.match.params.uri);
  const loadingSelector = makeLoadingSelector([`(.*)${uri}(.*)`, '^((?!contains).)*$', '^((?!albums).)*$', '^((?!related-artists).)*$', '^((?!top-tracks).)*$', '^((?!following).)*$']);
  const artistSelector = makeItemSelector(uri);
  const artist = artistSelector(state);
  let albums = null;
  if (artist && artist.albums_uris) {
    const albumsSelector = makeItemSelector(artist.albums_uris);
    albums = albumsSelector(state);
  }

  return {
    uri,
    artist,
    albums,
    loading: loadingSelector(state),
    theme: state.ui.theme,
    slim_mode: state.ui.slim_mode,
    spotify_authorized: state.spotify.authorization,
  };
};

const mapDispatchToProps = (dispatch) => ({
  coreActions: bindActionCreators(coreActions, dispatch),
  uiActions: bindActionCreators(uiActions, dispatch),
  mopidyActions: bindActionCreators(mopidyActions, dispatch),
});

export default connect(mapStateToProps, mapDispatchToProps)(Artist);
