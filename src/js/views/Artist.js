
import React from 'react';
import { connect } from 'react-redux';
import { bindActionCreators } from 'redux';
import { Route, Switch } from 'react-router-dom';
import ErrorMessage from '../components/ErrorMessage';
import Link from '../components/Link';
import TrackList from '../components/TrackList';
import AlbumGrid from '../components/AlbumGrid';
import Thumbnail from '../components/Thumbnail';
import Parallax from '../components/Parallax';
import ArtistGrid from '../components/ArtistGrid';
import RelatedArtists from '../components/RelatedArtists';
import FollowButton from '../components/Fields/FollowButton';
import ContextMenuTrigger from '../components/ContextMenuTrigger';
import DropdownField from '../components/Fields/DropdownField';
import FilterField from '../components/Fields/FilterField';
import Icon from '../components/Icon';
import Loader from '../components/Loader';
import * as coreActions from '../services/core/actions';
import * as uiActions from '../services/ui/actions';
import * as mopidyActions from '../services/mopidy/actions';
import {
  uriSource,
  sourceIcon,
  titleCase,
} from '../util/helpers';
import { collate } from '../util/format';
import { sortItems, applyFilter, arrayOf } from '../util/arrays';
import { i18n, I18n } from '../locale';
import Button from '../components/Button';
import { trackEvent } from '../components/Trackable';
import {
  makeItemSelector,
  makeLoadingSelector,
} from '../util/selectors';

class Artist extends React.Component {
  constructor(props) {
    super(props);

    this.state = {
      albumsFilter: '',
      tracksFilter: '',
    };
  }

  componentDidMount() {
    const {
      uri,
      coreActions: {
        loadItem,
      },
    } = this.props;

    this.setWindowTitle();
    loadItem(uri, { full: true });
  }

  componentDidUpdate = ({
    uri: prevUri,
    artist: prevArtist,
  }) => {
    const {
      uri,
      artist,
      coreActions: {
        loadItem,
      },
    } = this.props;

    if (uri !== prevUri) {
      loadItem(uri, { full: true });
    }

    if (!prevArtist && artist) this.setWindowTitle(artist);
    if (prevUri !== uri && artist) this.setWindowTitle(artist);
  }

  onChangeTracksFilter = (value) => {
    this.onChangeFilter('tracks', value);
  }

  onChangeAlbumsFilter = (value) => {
    this.onChangeFilter('albums', value);
  }

  onChangeFilter = (type, value) => {
    const { uiActions: { set, hideContextMenu } } = this.props;
    set({ [`artist_${type}_filter`]: value });
    hideContextMenu();
    trackEvent({ category: 'Artist', action: `Filter${type}`, label: value });
  }

  onChangeTracksSort = (value) => {
    this.onChangeSort('tracks', value);
  }

  onChangeAlbumsSort = (value) => {
    this.onChangeSort('albums', value);
  }

  onChangeSort = (type, value) => {
    const {
      [`${type}_sort`]: sort,
      [`${type}_sort_reverse`]: sort_reverse,
      uiActions: {
        set,
        hideContextMenu,
      },
    } = this.props;

    let reverse = false;
    if (value !== null && sort === value) {
      reverse = !sort_reverse;
    }

    set({
      [`artist_${type}_sort_reverse`]: reverse,
      [`artist_${type}_sort`]: value,
    });
    hideContextMenu();
    trackEvent({ category: 'Artist', action: `Sort${type}`, label: `${value} ${reverse ? 'DESC' : 'ASC'}` });
  }

  onPlay = () => {
    const {
      artist: {
        uri,
        tracks,
        albums_uris,
      },
      mopidyActions: {
        playURIs,
      },
    } = this.props;
    playURIs(arrayOf('uri', tracks) || albums_uris, uri);
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

  inLibrary = () => {
    const { uri } = this.props;
    const libraryName = `${uriSource(uri)}_library_artists`;
    const { [libraryName]: { items_uris } } = this.props;
    return items_uris.indexOf(uri) > -1;
  }

  renderOverview = () => {
    const {
      uri,
      uiActions,
      artist,
      albums_sort: sort,
      albums_sort_reverse: sort_reverse,
      filterType,
    } = this.props;
    const {
      albumsFilter: filter,
    } = this.state;
    let {
      tracks,
      related_artists,
    } = artist;
    let { albums } = this.props;

    if (sort && albums) {
      albums = sortItems(albums, sort, sort_reverse);
    }

    if (filterType && albums) {
      albums = applyFilter('type', filterType, albums);
    }

    if (filter && filter !== '') {
      albums = applyFilter('name', filter, albums);
    }

    const sort_options = [
      {
        value: null,
        label: i18n('artist.albums.sort.default'),
      },
      {
        value: 'name',
        label: i18n('artist.albums.sort.name'),
      },
      {
        value: 'release_date',
        label: i18n('artist.albums.sort.release_date'),
      },
      {
        value: 'tracks',
        label: i18n('artist.albums.sort.track_count'),
      },
    ];

    const filter_type_options = [
      {
        value: null,
        label: i18n('artist.albums.filter.all'),
      },
      {
        value: 'album',
        label: i18n('artist.albums.filter.albums'),
      },
      {
        value: 'single',
        label: i18n('artist.albums.filter.singles'),
      },
    ];

    return (
      <div className="body overview">
        <div className={`top-tracks col col--w${related_artists && related_artists.length > 0 ? '70' : '100'}`}>
          {tracks && <h4><I18n path="artist.overview.top_tracks" /></h4>}
          <div className="list-wrapper">
            <TrackList className="artist-track-list" uri={uri} tracks={tracks ? tracks.slice(0, 10) : []} />
          </div>
        </div>

        <div className="col col--w5" />

        {related_artists && related_artists.length > 0 && (
          <div className="col col--w25 related-artists">
            <h4><I18n path="artist.overview.related_artists.title" /></h4>
            <div className="list-wrapper">
              <RelatedArtists
                artists={related_artists.slice(0, 6)}
                uiActions={uiActions}
              />
            </div>
            <Button
              to={`/artist/${encodeURIComponent(uri)}/related-artists`}
              scrollTo="#sub-views-menu"
            >
              <I18n path="artist.overview.related_artists.more" />
            </Button>
          </div>
        )}

        <div className="cf" />

        <div className="albums">
          <h4>
            <I18n path="artist.overview.albums" />
            <div className="actions-wrapper">
              <FilterField
                initialValue={filter}
                handleChange={(value) => this.setState({ albumsFilter: value })}
                onSubmit={() => uiActions.hideContextMenu()}
              />
              <DropdownField
                icon="swap_vert"
                name="Sort"
                value={sort}
                valueAsLabel
                options={sort_options}
                selected_icon={sort ? (sort_reverse ? 'keyboard_arrow_up' : 'keyboard_arrow_down') : null}
                handleChange={this.onChangeAlbumsSort}
              />
              <DropdownField
                icon="filter_list"
                name="Filter"
                value={filterType}
                valueAsLabel
                options={filter_type_options}
                handleChange={this.onChangeAlbumsFilter}
              />
            </div>
          </h4>

          <section className="grid-wrapper no-top-padding">
            <AlbumGrid albums={albums} />
          </section>
        </div>
      </div>
    );
  }

  renderTracks = () => {
    const {
      artist: {
        uri,
      },
      tracks_sort: sort,
      tracks_sort_reverse: sort_reverse,
    } = this.props;
    const { tracksFilter: filter } = this.state;
    let { artist: { tracks } } = this.props;

    if (sort && tracks) {
      tracks = sortItems(tracks, sort, sort_reverse);
    }

    if (filter && filter !== '') {
      tracks = applyFilter('name', filter, tracks);
    }

    const sort_options = [
      {
        value: 'disc_track',
        label: i18n('album.tracks.sort.disc_track'),
      },
      {
        value: 'name',
        label: i18n('album.tracks.sort.name'),
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
                value={sort}
                valueAsLabel
                options={sort_options}
                selected_icon={sort ? (sort_reverse ? 'keyboard_arrow_up' : 'keyboard_arrow_down') : null}
                handleChange={this.onChangeTracksSort}
              />
            </div>
          </h4>
          <TrackList
            className="artist-track-list"
            uri={uri}
            tracks={tracks}
          />
        </section>
      </div>
    );
  }

  renderRelatedArtists = () => {
    const { artist } = this.props;

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

    const thumbnails = artist.images && Array.isArray(artist.images) && artist.images.map(
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
      loading,
      artist,
      history,
    } = this.props;

    if (loading) {
      return <Loader body loading />;
    } else if (!artist) {
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
                <h1>{artist && artist.name}</h1>
                <div className="actions">
                  <Button
                    type="primary"
                    onClick={this.onPlay}
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
              {artist.tracks && artist.tracks.length > 10 && (
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
              {artist.related_artists && (
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
  const loadingSelector = makeLoadingSelector([`(.*)${uri}(.*)`]);
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
    filterType: state.ui.artist_albums_filter,
    albums_sort: state.ui.artist_albums_sort,
    albums_sort_reverse: (!!state.ui.artist_albums_sort_reverse),
    tracks_sort: state.ui.artist_tracks_sort,
    tracks_sort_reverse: (!!state.ui.artist_tracks_sort_reverse),
    spotify_authorized: state.spotify.authorization,
  };
};

const mapDispatchToProps = (dispatch) => ({
  coreActions: bindActionCreators(coreActions, dispatch),
  uiActions: bindActionCreators(uiActions, dispatch),
  mopidyActions: bindActionCreators(mopidyActions, dispatch),
});

export default connect(mapStateToProps, mapDispatchToProps)(Artist);
