import React, { useState, useEffect } from 'react';
import { useDispatch, useSelector } from 'react-redux';
import { bindActionCreators } from 'redux';
import { Route, Switch, useParams, useHistory } from 'react-router-dom';
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
import { makeItemSelector } from '../util/selectors';

const Artist = () => {
  const history = useHistory();
  const dispatch = useDispatch();
  const { uri: encodedUri } = useParams();
  const uri = decodeUri(encodedUri);

  const artistSelector = makeItemSelector(uri);
  const artist = useSelector(artistSelector);
  const albumSelector = makeItemSelector(artist?.albums_uris || []);
  const albums = useSelector(albumSelector);

  const loading = artist?.loading && artist.loading !== 'albums';

  const { loadArtist } = coreActions;
  const { playURIs } = mopidyActions;
  const {
    setWindowTitle,
    showContextMenu,
    createNotification,
  } = uiActions;

  useEffect(
    () => {
      if (uri) dispatch(loadArtist(uri, { full: true }));
    },
    [uri],
  );

  useEffect(() => {
    if (artist) {
      dispatch(setWindowTitle(i18n('artist.title_window', { name: artist.name })));
    } else {
      dispatch(setWindowTitle(i18n('artist.title')));
    }
  }, [artist]);

  const onPlayAll = () => {
    const {
      tracks,
      albums_uris,
    } = artist;

    if ((!albums_uris || !albums_uris.length) && (!tracks || !tracks.length)) {
      dispatch(createNotification({ content: i18n('errors.no_results'), level: 'warning' }));
      return;
    }

    dispatch(playURIs(arrayOf('uri', tracks) || albums_uris, uri));
  };

  const handleContextMenu = (e) => dispatch(
    showContextMenu({
      e,
      context: 'artist',
      items: [artist],
      uris: [uri],
      tracklist_uri: uri,
    }),
  );

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

export default Artist;
