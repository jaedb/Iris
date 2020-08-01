
import React from 'react';
import { connect } from 'react-redux';
import { bindActionCreators } from 'redux';
import { Route, Switch } from 'react-router-dom';
import ErrorMessage from '../components/ErrorMessage';
import Link from '../components/Link';
import LazyLoadListener from '../components/LazyLoadListener';
import TrackList from '../components/TrackList';
import AlbumGrid from '../components/AlbumGrid';
import Thumbnail from '../components/Thumbnail';
import Parallax from '../components/Parallax';
import ArtistGrid from '../components/ArtistGrid';
import RelatedArtists from '../components/RelatedArtists';
import FollowButton from '../components/Fields/FollowButton';
import ContextMenuTrigger from '../components/ContextMenuTrigger';
import DropdownField from '../components/Fields/DropdownField';
import Icon from '../components/Icon';
import Loader from '../components/Loader';
import * as coreActions from '../services/core/actions';
import * as uiActions from '../services/ui/actions';
import * as mopidyActions from '../services/mopidy/actions';
import * as pusherActions from '../services/pusher/actions';
import * as lastfmActions from '../services/lastfm/actions';
import * as spotifyActions from '../services/spotify/actions';
import {
  uriSource,
  getFromUri,
  isLoading,
  sourceIcon,
  titleCase,
} from '../util/helpers';
import { collate } from '../util/format';
import { sortItems, applyFilter } from '../util/arrays';
import { i18n, I18n } from '../locale';

class Artist extends React.Component {
  componentDidMount() {
    this.setWindowTitle();
    this.props.coreActions.loadArtist(this.props.uri);
  }

  componentDidUpdate = ({
    uri: prevUri,
    mopidy_connected: prev_mopidy_connected,
    artist: prevArtist,
  }) => {
    const {
      uri,
      mopidy_connected,
      artist,
      coreActions: {
        loadArtist,
      },
    } = this.props;

    if (uri !== prevUri) {
      loadArtist(uri);
    } else if (!prev_mopidy_connected && mopidy_connected) {
      if (uriSource(uri) !== 'spotify') {
        loadArtist(uri);
      }
    }

    if (!prevArtist && artist) this.setWindowTitle(artist);
    if (prevUri !== uri && artist) this.setWindowTitle(artist);
  }

  onResetFilters = () => {
    this.onChangeFilter(null);
    this.onChangeSort(null);
  }

  onChangeFilter = (value) => {
    const { uiActions: { set, hideContextMenu } } = this.props;
    set({ artist_albums_filter: value });
    hideContextMenu();
  }

  onChangeSort = (value) => {
    const {
      sort,
      sort_reverse,
      uiActions: {
        set,
        hideContextMenu,
      },
    } = this.props;

    let reverse = false;
    if (value !== null && sort == value) {
      reverse = !sort_reverse;
    }

    set({
      artist_albums_sort_reverse: reverse,
      artist_albums_sort: value,
    });
    hideContextMenu();
  }

  onPlay = () => {
    const {
      artist: {
        uri,
        tracks_uris,
        albums_uris,
      },
      mopidyActions: { playURIs },
    } = this.props;
    playURIs(tracks_uris || albums_uris, uri);
  }

  setWindowTitle = (artist = this.props.artist) => {
    const { uiActions: { setWindowTitle } } = this.props;

    if (artist) {
      setWindowTitle(i18n('artist.title_window', { name: artist.name }));
    } else {
      setWindowTitle(i18n('artist.title'));
    }
  }

  handleContextMenu = (e) => {
    const {
      artist,
      uri,
      uiActions: { showContextMenu },
    } = this.props;

    showContextMenu({
      e,
      context: 'artist',
      items: [artist],
      uris: [uri],
    });
  }

  loadMore = () => {
    const {
      spotifyActions: { getMore },
      artist: { albums_more },
      uri,
    } = this.props;

    getMore(
      albums_more,
      {
        parent_type: 'artist',
        parent_key: uri,
        records_type: 'album',
      },
    );
  }

  inLibrary = () => {
    const { uri } = this.props;
    const libraryName = `${uriSource(uri)}_library_artists`;
    const { [libraryName]: library = [] } = this.props;
    return library.indexOf(uri) > -1;
  }

  renderOverview = () => {
    const {
      uri,
      uiActions,
      artist: artistProp,
      artists,
      albums,
      tracks,
      sort,
      sort_reverse,
      filter,
    } = this.props;
    const artist = collate(
      artistProp,
      {
        artists,
        albums,
        tracks,
      },
    );

    if (sort && artist.albums) {
      artist.albums = sortItems(artist.albums, sort, sort_reverse);
    }

    if (filter && artist.albums) {
      artist.albums = applyFilter('type', filter, artist.albums);
    }

    const sort_options = [
      {
        value: null,
        label: 'Default',
      },
      {
        value: 'name',
        label: 'Name',
      },
      {
        value: 'release_date',
        label: 'Date',
      },
      {
        value: 'tracks_uris.length',
        label: 'Tracks',
      },
    ];

    const filter_options = [
      {
        value: null,
        label: 'All',
      },
      {
        value: 'album',
        label: 'Albums',
      },
      {
        value: 'single',
        label: 'Singles',
      },
      {
        value: 'compilation',
        label: 'Compilations',
      },
    ];

    const is_loading_tracks = (
      !artist.tracks_uris
      || (artist.tracks_uris && !artist.tracks)
      || (artist.tracks_uris.length !== artist.tracks.length)
    );

    return (
      <div className="body overview">
        <div className={`top-tracks col col--w${artist.related_artists && artist.related_artists.length > 0 ? '70' : '100'}`}>
          {artist.tracks && <h4><I18n path="artist.overview.top_tracks" /></h4>}
          <div className="list-wrapper">
            <TrackList className="artist-track-list" uri={artist.uri} tracks={artist.tracks ? artist.tracks.splice(0, 10) : []} />
            <LazyLoadListener showLoader={is_loading_tracks} />
          </div>
        </div>

        <div className="col col--w5" />

        {artist.related_artists && artist.related_artists.length > 0 && (
          <div className="col col--w25 related-artists">
            <h4><I18n path="artist.overview.related_artists.title" /></h4>
            <div className="list-wrapper">
              <RelatedArtists
                artists={artist.related_artists.slice(0, 6)}
                uiActions={uiActions}
              />
            </div>
            <Link
              to={`/artist/${encodeURIComponent(uri)}/related-artists`}
              scrollTo="#sub-views-menu"
              className="button button--default"
            >
              <I18n path="artist.overview.related_artists.more" />
            </Link>
          </div>
        )}

        <div className="cf" />

        {artist.albums && (
          <div className="albums">
            <h4>
						  <div><I18n path="artist.overview.albums" /></div>
              <DropdownField
                icon="swap_vert"
                name="Sort"
                value={sort}
                valueAsLabel
                options={sort_options}
                selected_icon={sort ? (sort_reverse ? 'keyboard_arrow_up' : 'keyboard_arrow_down') : null}
                handleChange={this.onChangeSort}
              />
              <DropdownField
                icon="filter_list"
                name="Filter"
                value={filter}
                valueAsLabel
                options={filter_options}
                handleChange={this.onChangeFilter}
              />
              {(sort || filter) && (
                <a className="button button--discrete button--destructive button--small" onClick={this.onResetFilters}>
                  <Icon name="clear" />
                  <I18n path="actions.reset" />
                </a>
              )}
            </h4>

            <section className="grid-wrapper no-top-padding">
              <AlbumGrid albums={artist.albums} />
              <LazyLoadListener
                loadKey={artist.albums_more}
                showLoader={artist.albums_more}
                loadMore={this.loadMore}
              />
            </section>
          </div>
        )}
      </div>
    );
  }

  renderTracks = () => {
    const {
      artist: artistProp,
      artists,
      tracks,
    } = this.props;

    const artist = collate(
      artistProp,
      {
        artists,
        tracks,
      },
    );

    const is_loading_tracks = (
      !artist.tracks_uris
      || (artist.tracks_uris && !artist.tracks)
      || (artist.tracks_uris.length !== artist.tracks.length)
    );

    return (
      <div className="body related-artists">
        <section className="list-wrapper no-top-padding">
          <TrackList className="artist-track-list" uri={artist.uri} tracks={artist.tracks} />
          <LazyLoadListener showLoader={is_loading_tracks} />
        </section>
      </div>
    );
  }

  renderRelatedArtists = () => {
    const {
      artist: artistProp,
      artists,
    } = this.props;

    const artist = collate(
      artistProp,
      {
        artists,
      },
    );

    return (
      <div className="body related-artists">
        <section className="grid-wrapper no-top-padding">
          <ArtistGrid artists={artist.related_artists} />
        </section>
      </div>
    );
  }

  renderAbout = () => {
    const {
      artist: artistProp,
      artists,
    } = this.props;

    const artist = collate(
      artistProp,
      {
        artists,
      },
    );

    const thumbnails = artist.images && artist.images.map(
      (image) => {
        if (!image.huge) return null;
        return (
          <div className="tile thumbnail-wrapper" key={image.huge}>
            <Thumbnail size="huge" canZoom fill images={image} />
          </div>
        );
      }
    );

    return (
      <div className="body about">
        <div className="col col--w40 tiles artist-stats">
          {thumbnails}
          <div className="tile">
            <span className="content">
              <Icon type="fontawesome" name={sourceIcon(artist.uri)} />
              <I18n
                path="artist.about.source"
                source={titleCase(uriSource(artist.uri))}
              />
            </span>
          </div>
          {artist.followers && (
            <div className="tile">
              <span className="content">
                <Icon type="fontawesome" name="users" />
                <I18n path="specs.followers" count={artist.followers.toLocaleString()} />
              </span>
            </div>
          )}
          {artist.popularity && (
            <div className="tile">
              <span className="content">
                <Icon type="fontawesome" name="fire" />
                <I18n path="specs.popularity" percent={artist.popularity} />
              </span>
            </div>
          )}
          {artist.listeners && (
            <div className="tile">
              <span className="content">
                <Icon type="fontawesome" name="headphones" />
                <I18n path="specs.listeners" count={artist.listeners.toLocaleString()} />
              </span>
            </div>
          )}
        </div>

        <div className="col col--w60 biography">
          <section>
            <br />
            {artist.biography && (
              <div className="biography-text">
                <p>{artist.biography}</p>
                <br />
                <div className="mid_grey-text">
                  <I18n path="artist.about.wiki.published" date={artist.biography_publish_date} />
                </div>
                <div className="mid_grey-text">
                  <I18n path="artist.about.wiki.origin" />
                  <a
                    href={artist.biography_link}
                    target="_blank"
                    rel="noopener noreferrer"
                  >
                    {artist.biography_link}
                  </a>
                </div>
              </div>
            )}
          </section>
        </div>
      </div>
    );
  }

  render = () => {
    const {
      uri,
      load_queue,
      artist,
      history,
    } = this.props;

    if (!artist) {
      if (
        isLoading(
          load_queue,
          [`spotify_artists/${getFromUri('artistid', uri)}`, 'lastfm_method=artist.getInfo'],
        )
      ) {
        return <Loader body loading />;
      }
      return (
        <ErrorMessage type="not-found" title="Not found">
          <p>
            <I18n path="errors.uri_not_found" uri={encodeURIComponent(uri)} />
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
                <h1>{this.props.artist ? this.props.artist.name : null}</h1>
                <div className="actions">
                  <button className="button button--primary" onClick={this.onPlay}>
                    <I18n path="actions.play" />
                  </button>
                  {is_spotify && (
                    <FollowButton
                      uri={uri}
                      is_following={this.inLibrary()}
                    />
                  )}
                  <ContextMenuTrigger className="white" onTrigger={this.handleContextMenu} />
                </div>
              </div>
            </div>
            <div className="sub-views" id="sub-views-menu">
              <Link
                exact
                history={history}
                activeClassName="sub-views__option--active"
                className="sub-views__option"
                to={`/artist/${encodeURIComponent(uri)}`}
                scrollTo="#sub-views-menu"
              >
                <h4><I18n path="artist.overview.title" /></h4>
              </Link>
              {artist.tracks_uris && artist.tracks_uris.length > 10 && (
                <Link
                  exact
                  history={history}
                  activeClassName="sub-views__option--active"
                  className="sub-views__option"
                  to={`/artist/${encodeURIComponent(uri)}/tracks`}
                  scrollTo="#sub-views-menu"
                >
                  <h4><I18n path="artist.tracks.title" /></h4>
                </Link>
              )}
              {artist.related_artists_uris && (
                <Link
                  exact
                  history={history}
                  activeClassName="sub-views__option--active"
                  className="sub-views__option"
                  to={`/artist/${encodeURIComponent(uri)}/related-artists`}
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
                to={`/artist/${encodeURIComponent(uri)}/about`}
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
              {this.renderRelatedArtists()}
            </Route>
            <Route exact path="/artist/:id/tracks">
              {this.renderTracks()}
            </Route>
            <Route exact path="/artist/:id/about">
              {this.renderAbout()}
            </Route>
            <Route exact path="/artist/:id">
              {this.renderOverview()}
            </Route>
          </Switch>
        </div>
      </div>
    );
  }
}

const mapStateToProps = (state, ownProps) => {
  const uri = decodeURIComponent(ownProps.match.params.uri);
  return {
    uri,
    theme: state.ui.theme,
    slim_mode: state.ui.slim_mode,
    load_queue: state.ui.load_queue,
    artist: (state.core.artists[uri] !== undefined ? state.core.artists[uri] : false),
    tracks: state.core.tracks,
    artists: state.core.artists,
    spotify_library_artists: state.spotify.library_artists,
    local_library_artists: state.mopidy.library_artists,
    albums: (state.core.albums ? state.core.albums : []),
    filter: (state.ui.artist_albums_filter ? state.ui.artist_albums_filter : null),
    sort: (state.ui.artist_albums_sort ? state.ui.artist_albums_sort : null),
    sort_reverse: (!!state.ui.artist_albums_sort_reverse),
    spotify_authorized: state.spotify.authorization,
    mopidy_connected: state.mopidy.connected,
  };
};

const mapDispatchToProps = (dispatch) => ({
  coreActions: bindActionCreators(coreActions, dispatch),
  uiActions: bindActionCreators(uiActions, dispatch),
  mopidyActions: bindActionCreators(mopidyActions, dispatch),
  pusherActions: bindActionCreators(pusherActions, dispatch),
  lastfmActions: bindActionCreators(lastfmActions, dispatch),
  spotifyActions: bindActionCreators(spotifyActions, dispatch),
});

export default connect(mapStateToProps, mapDispatchToProps)(Artist);
