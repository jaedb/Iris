import React, { useState, useEffect } from 'react';
import { connect } from 'react-redux';
import { bindActionCreators } from 'redux';
import { Route, Switch } from 'react-router-dom';
import sanitizeHtml from 'sanitize-html';
import ErrorMessage from '../components/ErrorMessage';
import Link from '../components/Link';
import TrackList from '../components/TrackList';
import Thumbnail from '../components/Thumbnail';
import Parallax from '../components/Parallax';
import { Grid } from '../components/Grid';
import RelatedArtists from '../components/RelatedArtists';
import FollowButton from '../components/Fields/FollowButton';
import ContextMenuTrigger from '../components/ContextMenuTrigger';
import DropdownField from '../components/Fields/DropdownField';
import FilterField from '../components/Fields/FilterField';
import ArtistAbout from './subviews/ArtistAbout';
import ArtistOverview from './subviews/ArtistOverview';
import Icon, { SourceIcon } from '../components/Icon';
import Loader from '../components/Loader';
import * as coreActions from '../services/core/actions';
import * as uiActions from '../services/ui/actions';
import * as mopidyActions from '../services/mopidy/actions';
import {
  uriSource,
  titleCase,
} from '../util/helpers';
import { collate, encodeUri, decodeUri } from '../util/format';
import { sortItems, applyFilter, arrayOf } from '../util/arrays';
import { i18n, I18n } from '../locale';
import Button from '../components/Button';
import { trackEvent } from '../components/Trackable';
import {
  makeItemSelector,
  makeLoadingSelector,
  getSortSelector,
} from '../util/selectors';
import { nice_number } from '../components/NiceNumber';

const ALBUM_SORT_KEY = 'artist_albums';
const TRACK_SORT_KEY = 'artist_tracks';

const Artist = ({
  uri,
  loading,
  history,
  artist: artistProp,
  coreActions: {
    loadArtist,
  },
  uiActions: {
    set,
    setSort,
    setWindowTitle,
    hideContextMenu,
    createNotification,
  },
  mopidyActions: {
    playURIs,
  },
  ...props
}) => {
  const [artist, setArtist] = useState({});

  useEffect(
    () => {
      if (uri) {
        loadArtist(uri, { full: true });
      }
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

  const onChangeFilter = (type, value) => {
    set({ [`artist_${type}_filter`]: value });
    hideContextMenu();
    trackEvent({ category: 'Artist', action: `Filter${type}`, label: value });
  }

  const onChangeSort = (key, field) => {
    const prefix = key.replace('artist_');
    const {
      [`${prefix}SortField`]: sortField,
      [`${prefix}SortReverse`]: sortReverse,
    } = props;

    let reverse = false;
    if (field !== null && sortField === field) {
      reverse = !sortReverse;
    }

    setSort(key, field, reverse);
    hideContextMenu();
  }

  const onPlayAll = () => {
    const {
      uri,
      tracks,
      albums_uris,
    } = album;

    if ((!albums_uris || !albums_uris.length) && (!tracks || !tracks.length)) {
      createNotification({ content: i18n('errors.no_results'), level: 'warning' });
      return;
    }

    playURIs(arrayOf('uri', tracks) || albums_uris, uri);
  }

  const handleContextMenu = (e) => {
    showContextMenu({
      e,
      context: 'artist',
      items: [artist],
      uris: [uri],
      tracklist_uri: uri,
    });
  }


  const renderTracks = () => {
    const {
      artist: {
        uri,
      },
      trackSortField,
      trackSortReverse,
    } = this.props;
    const { tracksFilter: filter } = this.state;
    let { artist: { tracks } } = this.props;

    if (trackSortField && tracks) {
      tracks = sortItems(tracks, trackSortField, trackSortReverse);
    }

    if (filter && filter !== '') {
      tracks = applyFilter('name', filter, tracks);
    }

    const sort_options = [
      {
        value: 'name',
        label: i18n('artist.tracks.sort.name'),
      },
      {
        value: 'album',
        label: i18n('artist.tracks.sort.album'),
      },
    ];

    return (
      <div className="body related-artists">
        <section className="list-wrapper no-top-padding">
          <h4 className="no-bottom-margin">
            <I18n path="artist.tracks.title" />
            <div className="actions-wrapper">
              <FilterField
                initialValue={filter}
                handleChange={(value) => this.setState({ tracksFilter: value })}
                onSubmit={() => uiActions.hideContextMenu()}
              />
              <DropdownField
                icon="swap_vert"
                name="Sort"
                value={trackSortField}
                valueAsLabel
                options={sort_options}
                selected_icon={trackSortField ? (trackSortReverse ? 'keyboard_arrow_up' : 'keyboard_arrow_down') : null}
                handleChange={this.onChangeTracksSort}
              />
            </div>
          </h4>
          <TrackList
            className="artist-track-list"
            uri={uri}
            tracks={tracks}
            track_context="artist"
          />
        </section>
      </div>
    );
  }

  const renderRelatedArtists = () => {
    const { artist } = this.props;

    return (
      <div className="body related-artists">
        <section className="grid-wrapper no-top-padding">
          <Grid items={artist.related_artists} />
        </section>
      </div>
    );
  }



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
            {/* {this.renderRelatedArtists()} */}
          </Route>
          <Route exact path="/artist/:id/tracks">
            {/* {this.renderTracks()} */}
          </Route>
          <Route exact path="/artist/:id/about">
            <ArtistAbout
              artist={artist}
            />
          </Route>
          <Route exact path="/artist/:id/:name?">
            <ArtistOverview
              artist={artist}
            />
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
  const [albumSortField, albumSortReverse] = getSortSelector(state, ALBUM_SORT_KEY, null);

  return {
    uri,
    artist,
    albums,
    loading: loadingSelector(state),
    theme: state.ui.theme,
    slim_mode: state.ui.slim_mode,
    filterType: state.ui.artist_albums_filter,
    albumSortField,
    albumSortReverse,
    spotify_authorized: state.spotify.authorization,
  };
};

const mapDispatchToProps = (dispatch) => ({
  coreActions: bindActionCreators(coreActions, dispatch),
  uiActions: bindActionCreators(uiActions, dispatch),
  mopidyActions: bindActionCreators(mopidyActions, dispatch),
});

export default connect(mapStateToProps, mapDispatchToProps)(Artist);
