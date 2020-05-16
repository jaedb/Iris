
import React from 'react';
import { connect } from 'react-redux';
import { bindActionCreators } from 'redux';
import AlbumGrid from '../../components/AlbumGrid';
import List from '../../components/List';
import Header from '../../components/Header';
import DropdownField from '../../components/Fields/DropdownField';
import FilterField from '../../components/Fields/FilterField';
import LazyLoadListener from '../../components/LazyLoadListener';
import Icon from '../../components/Icon';
import * as coreActions from '../../services/core/actions';
import * as uiActions from '../../services/ui/actions';
import * as mopidyActions from '../../services/mopidy/actions';
import * as googleActions from '../../services/google/actions';
import * as spotifyActions from '../../services/spotify/actions';
import {
  uriSource,
} from '../../util/helpers';
import { sortItems, applyFilter } from '../../util/arrays';
import { collate } from '../../util/format';

class LibraryAlbums extends React.Component {
  constructor(props) {
    super(props);

    this.state = {
      filter: '',
      limit: 50,
      per_page: 50,
    };
  }

  componentDidMount() {
    // Restore any limit defined in our location state
    const state = (this.props.location.state ? this.props.location.state : {});
    if (state.limit) {
      this.setState({
        limit: state.limit,
      });
    }

    this.props.uiActions.setWindowTitle('Albums');

    if (this.props.mopidy_connected && this.props.mopidy_library_albums_status != 'finished' && this.props.mopidy_library_albums_status != 'started' && (this.props.source == 'all' || this.props.source == 'local')) {
      this.props.mopidyActions.getLibraryAlbums();
    }

    if (this.props.google_available && this.props.google_library_albums_status != 'finished' && this.props.google_library_albums_status != 'started' && (this.props.source == 'all' || this.props.source == 'google')) {
      this.props.googleActions.getLibraryAlbums();
    }

    if (this.props.spotify_available && this.props.spotify_library_albums_status != 'finished' && this.props.spotify_library_albums_status != 'started' && (this.props.source == 'all' || this.props.source == 'spotify')) {
      this.props.spotifyActions.getLibraryAlbums();
    }
  }

  componentDidUpdate = ({
    mopidy_connected: prev_mopidy_connected,
  }) => {
    const {
      mopidy_connected,
      google_available,
      spotify_available,
      source,
      mopidyActions,
      googleActions,
      spotifyActions,
      mopidy_library_albums_status,
      google_library_albums_status,
      spotify_library_albums_status,
    } = this.props;

    if (mopidy_connected && (source == 'all' || source == 'local')) {
      if (!prev_mopidy_connected) mopidyActions.getLibraryAlbums();

      // Filter changed, but we haven't got this provider's library yet
      if (source !== 'all' && source !== 'local' && mopidy_library_albums_status !== 'finished' && mopidy_library_albums_status !== 'started') {
        mopidyActions.getLibraryAlbums();
      }
    }

    if (google_available && (source == 'all' || source == 'google')) {
      // Filter changed, but we haven't got this provider's library yet
      if (source !== 'all' && source !== 'google' && google_library_albums_status !== 'finished' && google_library_albums_status !== 'started') {
        googleActions.getLibraryAlbums();
      }
    }

    if (spotify_available && (source === 'all' || source === 'spotify')) {
      // Filter changed, but we haven't got this provider's library yet
      if (spotify_library_albums_status !== 'finished' && spotify_library_albums_status !== 'started') {
        spotifyActions.getLibraryAlbums();
      }
    }
  }

  handleContextMenu(e, item) {
    const data = {
      e,
      context: 'album',
      uris: [item.uri],
      items: [item],
    };
    this.props.uiActions.showContextMenu(data);
  }

  moreURIsToLoad() {
    const uris = [];
    if (this.props.albums && this.props.library_albums) {
      for (let i = 0; i < this.props.library_albums.length; i++) {
        const uri = this.props.library_albums[i];
        if (!this.props.albums.hasOwnProperty(uri) && uriSource(uri) == 'local') {
          uris.push(uri);
        }

        // limit each lookup to 50 URIs
        if (uris.length >= 50) break;
      }
    }

    return uris;
  }

  loadMore() {
    const new_limit = this.state.limit + this.state.per_page;

    this.setState({ limit: new_limit });

    // Set our pagination to location state
    const state = (this.props.location && this.props.location.state ? this.props.location.state : {});
    state.limit = new_limit;
    this.props.history.replace({ state });
  }

  setSort(value) {
    let reverse = false;
    if (this.props.sort == value) reverse = !this.props.sort_reverse;

    const data = {
      library_albums_sort_reverse: reverse,
      library_albums_sort: value,
    };
    this.props.uiActions.set(data);
  }

  renderView() {
    let albums = [];

    // Spotify library items
    if (this.props.spotify_library_albums && (this.props.source == 'all' || this.props.source == 'spotify')) {
      for (var uri of this.props.spotify_library_albums) {
        if (this.props.albums.hasOwnProperty(uri)) {
          albums.push(this.props.albums[uri]);
        }
      }
    }

    // Mopidy library items
    if (this.props.mopidy_library_albums && (this.props.source == 'all' || this.props.source == 'local')) {
      for (var uri of this.props.mopidy_library_albums) {
        // Construct item placeholder. This is used as Mopidy needs to
        // lookup ref objects to get the full object which can take some time
        var source = uriSource(uri);
        var album = {
          uri,
          source,
        };

        if (this.props.albums.hasOwnProperty(uri)) {
          albums.push(this.props.albums[uri]);
        }
      }
    }

    // Google library items
    if (this.props.google_library_albums && (this.props.source == 'all' || this.props.source == 'google')) {
      for (var uri of this.props.google_library_albums) {
        // Construct item placeholder. This is used as Mopidy needs to
        // lookup ref objects to get the full object which can take some time
        var source = uriSource(uri);
        var album = {
          uri,
          source,
        };

        if (this.props.albums.hasOwnProperty(uri)) {
          album = this.props.albums[uri];
        }

        albums.push(album);
      }
    }

    // Collate each album into it's full object (including nested artists)
    for (let i = 0; i < albums.length; i++) {
      albums[i] = collate(albums[i], { artists: this.props.artists });
    }

    if (this.props.sort) {
      albums = sortItems(albums, this.props.sort, this.props.sort_reverse);
    }

    if (this.state.filter && this.state.filter !== '') {
      albums = applyFilter('name', this.state.filter, albums);
    }

    // Apply our lazy-load-rendering
    const total_albums = albums.length;
    albums = albums.slice(0, this.state.limit);

    if (this.props.view == 'list') {
      return (
        <section className="content-wrapper">
          <List
            handleContextMenu={(e, item) => this.handleContextMenu(e, item)}
            rows={albums}
            thumbnail
            details={['artists', 'tracks_uris.length', 'last_modified']}
            right_column={['added_at']}
            className="albums"
            link_prefix="/album/"
          />
          <LazyLoadListener
            loadKey={total_albums > this.state.limit ? this.state.limit : total_albums}
            showLoader={this.state.limit < total_albums}
            loadMore={() => this.loadMore()}
          />
        </section>
      );
    }
    return (
      <section className="content-wrapper">
        <AlbumGrid
          handleContextMenu={(e, item) => this.handleContextMenu(e, item)}
          albums={albums}
        />
        <LazyLoadListener
          loadKey={total_albums > this.state.limit ? this.state.limit : total_albums}
          showLoader={this.state.limit < total_albums}
          loadMore={() => this.loadMore()}
        />
      </section>
    );
  }

  render() {
    const source_options = [
      {
        value: 'all',
        label: 'All',
      },
      {
        value: 'local',
        label: 'Local',
      },
    ];

    if (this.props.spotify_available) {
      source_options.push({
        value: 'spotify',
        label: 'Spotify',
      });
    }

    if (this.props.google_available) {
      source_options.push({
        value: 'google',
        label: 'Google',
      });
    }

    const view_options = [
      {
        value: 'thumbnails',
        label: 'Thumbnails',
      },
      {
        value: 'list',
        label: 'List',
      },
    ];

    const sort_options = [
      {
        value: null,
        label: 'As loaded',
      },
      {
        value: 'name',
        label: 'Name',
      },
      {
        value: 'artists.first.name',
        label: 'Artist',
      },
      {
        value: 'last_modified',
        label: 'Updated',
      },
      {
        value: 'tracks_uris.length',
        label: 'Tracks',
      },
      {
        value: 'uri',
        label: 'Source',
      },
    ];

    const options = (
      <div className="header__options__wrapper">
        <FilterField
          initialValue={this.state.filter}
          handleChange={(value) => this.setState({ filter: value, limit: this.state.per_page })}
          onSubmit={e => this.props.uiActions.hideContextMenu()}
        />
        <DropdownField
          icon="swap_vert"
          name="Sort"
          value={this.props.sort}
          valueAsLabel
          options={sort_options}
          selected_icon={this.props.sort ? (this.props.sort_reverse ? 'keyboard_arrow_up' : 'keyboard_arrow_down') : null}
          handleChange={(val) => { this.setSort(val); this.props.uiActions.hideContextMenu(); }}
        />
        <DropdownField
          icon="visibility"
          name="View"
          value={this.props.view}
          valueAsLabel
          options={view_options}
          handleChange={(val) => { this.props.uiActions.set({ library_albums_view: val }); this.props.uiActions.hideContextMenu(); }}
        />
        <DropdownField
          icon="cloud"
          name="Source"
          value={this.props.source}
          valueAsLabel
          options={source_options}
          handleChange={(val) => { this.props.uiActions.set({ library_albums_source: val }); this.props.uiActions.hideContextMenu(); }}
        />
      </div>
    );

    return (
      <div className="view library-albums-view">
        <Header options={options} uiActions={this.props.uiActions}>
          <Icon name="album" type="material" />
					My albums
        </Header>
        {this.renderView()}
      </div>
    );
  }
}

const mapStateToProps = (state, ownProps) => ({
  mopidy_connected: state.mopidy.connected,
  mopidy_uri_schemes: state.mopidy.uri_schemes,
  load_queue: state.ui.load_queue,
  artists: state.core.artists,
  albums: state.core.albums,
  mopidy_library_albums: state.mopidy.library_albums,
  mopidy_library_albums_status: (state.ui.processes.MOPIDY_LIBRARY_ALBUMS_PROCESSOR !== undefined ? state.ui.processes.MOPIDY_LIBRARY_ALBUMS_PROCESSOR.status : null),
  google_available: (state.mopidy.uri_schemes && state.mopidy.uri_schemes.includes('gmusic:')),
  google_library_albums: state.google.library_albums,
  google_library_albums_status: (state.ui.processes.GOOGLE_LIBRARY_ALBUMS_PROCESSOR !== undefined ? state.ui.processes.GOOGLE_LIBRARY_ALBUMS_PROCESSOR.status : null),
  spotify_available: state.spotify.access_token,
  spotify_library_albums: state.spotify.library_albums,
  spotify_library_albums_status: (state.ui.processes.SPOTIFY_GET_LIBRARY_ALBUMS_PROCESSOR !== undefined ? state.ui.processes.SPOTIFY_GET_LIBRARY_ALBUMS_PROCESSOR.status : null),
  view: state.ui.library_albums_view,
  source: (state.ui.library_albums_source ? state.ui.library_albums_source : 'all'),
  sort: (state.ui.library_albums_sort ? state.ui.library_albums_sort : null),
  sort_reverse: (state.ui.library_albums_sort_reverse ? state.ui.library_albums_sort_reverse : false),
});

const mapDispatchToProps = (dispatch) => ({
  coreActions: bindActionCreators(coreActions, dispatch),
  uiActions: bindActionCreators(uiActions, dispatch),
  mopidyActions: bindActionCreators(mopidyActions, dispatch),
  googleActions: bindActionCreators(googleActions, dispatch),
  spotifyActions: bindActionCreators(spotifyActions, dispatch),
});

export default connect(mapStateToProps, mapDispatchToProps)(LibraryAlbums);
