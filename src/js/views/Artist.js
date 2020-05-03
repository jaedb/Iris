
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
      if (uriSource(uri) != 'spotify') {
        loadArtist(uri);
      }
    }

    if (!prevArtist && artist) this.setWindowTitle(artist);
    if (prevUri !== uri && artist) this.setWindowTitle(artist);
  }

  setWindowTitle(artist = this.props.artist) {
    if (artist) {
      this.props.uiActions.setWindowTitle(`${artist.name} (artist)`);
    } else {
      this.props.uiActions.setWindowTitle('Artist');
    }
  }

  handleContextMenu(e) {
    const data = {
      e,
      context: 'artist',
      items: [this.props.artist],
      uris: [this.props.uri],
    };
    this.props.uiActions.showContextMenu(data);
  }

  loadMore() {
    this.props.spotifyActions.getMore(
      this.props.artist.albums_more,
      {
        parent_type: 'artist',
        parent_key: this.props.uri,
        records_type: 'album',
      },
    );
  }

  inLibrary() {
    const library = `${uriSource(this.props.uri)}_library_artists`;
    return (this.props[library] && this.props[library].indexOf(this.props.uri) > -1);
  }

  setSort(value) {
    let reverse = false;
    if (value !== null && this.props.sort == value) {
      reverse = !this.props.sort_reverse;
    }

    const data = {
      artist_albums_sort_reverse: reverse,
      artist_albums_sort: value,
    };
    this.props.uiActions.set(data);
  }

  setFilter(value) {
    this.props.uiActions.set({ artist_albums_filter: value });
  }

  renderOverview() {
    const { uiActions } = this.props;
    const artist = collate(
      this.props.artist,
      {
        artists: this.props.artists,
        albums: this.props.albums,
        tracks: this.props.tracks,
      },
    );

    if (this.props.sort && artist.albums) {
      artist.albums = sortItems(artist.albums, this.props.sort, this.props.sort_reverse);
    }

    if (this.props.filter && artist.albums) {
      artist.albums = applyFilter('type', this.props.filter, artist.albums);
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

    if (!artist.tracks_uris || (artist.tracks_uris && !artist.tracks) || (artist.tracks_uris.length !== artist.tracks.length)) {
      var is_loading_tracks = true;
    } else {
      var is_loading_tracks = false;
    }

    return (
      <div className="body overview">
        <div className={`top-tracks col col--w${artist.related_artists && artist.related_artists.length > 0 ? '70' : '100'}`}>
          {artist.tracks ? <h4>Top tracks</h4> : null}
          <div className="list-wrapper">
            <TrackList className="artist-track-list" uri={artist.uri} tracks={artist.tracks ? artist.tracks.splice(0, 10) : []} />
            <LazyLoadListener showLoader={is_loading_tracks} />
          </div>
        </div>

        <div className="col col--w5" />

        {artist.related_artists && artist.related_artists.length > 0 && (
          <div className="col col--w25 related-artists">
            <h4>Related artists</h4>
            <div className="list-wrapper">
              <RelatedArtists
                artists={artist.related_artists.slice(0, 6)}
                uiActions={uiActions}
              />
            </div>
            <Link
              to={`/artist/${encodeURIComponent(this.props.uri)}/related-artists`}
              scrollTo="#sub-views-menu"
              className="button button--default"
            >
              All related artists
            </Link>
          </div>
        )}

        <div className="cf" />

        {artist.albums && (
          <div className="albums">
            <h4>
						  <div>Albums</div>
              <DropdownField
                icon="swap_vert"
                name="Sort"
                value={this.props.sort}
                valueAsLabel
                options={sort_options}
                selected_icon={this.props.sort ? (this.props.sort_reverse ? 'keyboard_arrow_up' : 'keyboard_arrow_down') : null}
                handleChange={(value) => { this.setSort(value); this.props.uiActions.hideContextMenu(); }}
              />
              <DropdownField
                icon="filter_list"
                name="Filter"
                value={this.props.filter}
                valueAsLabel
                options={filter_options}
                handleChange={(value) => { this.setFilter(value); this.props.uiActions.hideContextMenu(); }}
              />
              {(this.props.sort || this.props.filter) && (
                <a className="button button--discrete button--small" onClick={(e) => { this.setFilter(null); this.setSort(null); }}>
                  <Icon name="clear" />
                  Reset
                </a>
              )}
            </h4>

            <section className="grid-wrapper no-top-padding">
              <AlbumGrid albums={artist.albums} />
              <LazyLoadListener
                loadKey={artist.albums_more}
                showLoader={artist.albums_more}
                loadMore={() => this.loadMore()}
              />
            </section>
          </div>
        )}
      </div>
    );
  }

  renderTracks() {
    const artist = collate(
      this.props.artist,
      {
        artists: this.props.artists,
        tracks: this.props.tracks,
      },
    );

    if (!artist.tracks_uris || (artist.tracks_uris && !artist.tracks) || (artist.tracks_uris.length !== artist.tracks.length)) {
      var is_loading_tracks = true;
    } else {
      var is_loading_tracks = false;
    }

    return (
      <div className="body related-artists">
        <section className="list-wrapper no-top-padding">
          <TrackList className="artist-track-list" uri={artist.uri} tracks={artist.tracks} />
          <LazyLoadListener showLoader={is_loading_tracks} />
        </section>
      </div>
    );
  }

  renderRelatedArtists() {
    const artist = collate(
      this.props.artist,
      {
        artists: this.props.artists,
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

  renderAbout() {
    const artist = collate(
      this.props.artist,
      {
        artists: this.props.artists,
      },
    );

    const thumbnails = [];
    if (artist.images && artist.images.length > 0) {
      for (const images of artist.images) {
        // Capture potential 'null' images
        // TODO: Prevent these from existing in the first place
        if (images.huge) {
          thumbnails.push(
            <div className="tile thumbnail-wrapper" key={images.huge}>
              <Thumbnail size="huge" canZoom images={images} />
            </div>,
          );
        }
      }
    }

    return (
      <div className="body about">

        <div className="col col--w40 tiles artist-stats">
          {thumbnails}
          <div className="tile">
            <span className="content">
              <Icon type="fontawesome" name={sourceIcon(artist.uri)} />
              {`${titleCase(uriSource(artist.uri))} artist`}
            </span>
          </div>
          {artist.followers && (
            <div className="tile">
              <span className="content">
                <Icon type="fontawesome" name="users" />
                {`${artist.followers.toLocaleString()} followers`}
              </span>
            </div>
          )}
          {artist.popularity && (
            <div className="tile">
              <span className="content">
                <Icon type="fontawesome" name="fire" />
                {`${artist.popularity} % popularity`}
              </span>
            </div>
          )}
          {artist.listeners && (
            <div className="tile">
              <span className="content">
                <Icon type="fontawesome" name="headphones" />
                {`${artist.listeners.toLocaleString()} listeners`}
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
                  {`Published: ${artist.biography_publish_date}`}
                </div>
                <div className="mid_grey-text">
                  {`Origin: `}
                  <a href={artist.biography_link} target="_blank">{ artist.biography_link }</a>
                </div>
              </div>
            )}
          </section>
        </div>
      </div>
    );
  }

  render() {
    const scheme = uriSource(this.props.uri);

    if (!this.props.artist) {
      if (isLoading(this.props.load_queue, [`spotify_artists/${getFromUri('artistid', this.props.uri)}`, 'lastfm_method=artist.getInfo'])) {
        return <Loader body loading />;
      }
      return (
        <ErrorMessage type="not-found" title="Not found">
          <p>
            {`Could not find artist with URI "${encodeURIComponent(this.props.uri)}"`}
          </p>
        </ErrorMessage>
      );
    }

    if (this.props.artist && this.props.artist.images && this.props.artist.images.length > 0) {
      var image = this.props.artist.images[0].huge;
    } else {
      var image = null;
    }

    const is_spotify = (scheme == 'spotify');

    if (this.props.artist.tracks_uris && this.props.artist.tracks_uris.length > 0) {
      var uris_to_play = this.props.artist.tracks_uris;
    } else {
      var uris_to_play = this.props.artist.albums_uris;
    }

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
                  <button className="button button--primary" onClick={(e) => this.props.mopidyActions.playURIs(uris_to_play, this.props.artist.uri)}>Play</button>
                  {is_spotify ? <FollowButton uri={this.props.uri} removeText="Remove from library" addText="Add to library" is_following={this.inLibrary()} /> : null}
                  <ContextMenuTrigger className="white" onTrigger={(e) => this.handleContextMenu(e)} />
                </div>
              </div>
            </div>
            <div className="sub-views" id="sub-views-menu">
              <Link
                exact
                history={this.props.history}
                activeClassName="sub-views__option--active"
                className="sub-views__option"
                to={`/artist/${encodeURIComponent(this.props.uri)}`}
                scrollTo="#sub-views-menu"
              >
                <h4>Overview</h4>
              </Link>
              {this.props.artist.tracks_uris && this.props.artist.tracks_uris.length > 10 && (
                <Link
                  exact
                  history={this.props.history}
                  activeClassName="sub-views__option--active"
                  className="sub-views__option"
                  to={`/artist/${encodeURIComponent(this.props.uri)}/tracks`}
                  scrollTo="#sub-views-menu"
                >
                  <h4>Tracks</h4>
                </Link>
              )}
              {this.props.artist.related_artists_uris && (
                <Link
                  exact
                  history={this.props.history}
                  activeClassName="sub-views__option--active"
                  className="sub-views__option"
                  to={`/artist/${encodeURIComponent(this.props.uri)}/related-artists`}
                  scrollTo="#sub-views-menu"
                >
                  <h4>Related artists</h4>
                </Link>
              )}
              <Link
                exact
                history={this.props.history}
                activeClassName="sub-views__option--active"
                className="sub-views__option"
                to={`/artist/${encodeURIComponent(this.props.uri)}/about`}
                scrollTo="#sub-views-menu"
              >
                <h4>About</h4>
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
