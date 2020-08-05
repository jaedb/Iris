
import React from 'react';
import { connect } from 'react-redux';
import { bindActionCreators } from 'redux';
import Header from '../../components/Header';
import ArtistGrid from '../../components/ArtistGrid';
import List from '../../components/List';
import DropdownField from '../../components/Fields/DropdownField';
import FilterField from '../../components/Fields/FilterField';
import LazyLoadListener from '../../components/LazyLoadListener';
import Icon from '../../components/Icon';
import * as uiActions from '../../services/ui/actions';
import * as mopidyActions from '../../services/mopidy/actions';
import * as spotifyActions from '../../services/spotify/actions';
import * as googleActions from '../../services/google/actions';
import {
  uriSource,
} from '../../util/helpers';
import { sortItems, applyFilter } from '../../util/arrays';
import { I18n, i18n } from '../../locale';

class LibraryArtists extends React.Component {
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

    this.props.uiActions.setWindowTitle(i18n('library.artists.title'));

    if (!this.props.mopidy_library_artists && this.props.mopidy_connected && (this.props.source == 'all' || this.props.source == 'local')) {
      this.props.mopidyActions.getLibraryArtists();
    }

    if (this.props.google_available && !this.props.google_library_artists && this.props.mopidy_connected && (this.props.source == 'all' || this.props.source == 'google')) {
      this.props.googleActions.getLibraryArtists();
    }

    if (this.props.spotify_available && this.props.spotify_library_artists_status != 'finished' && (this.props.source == 'all' || this.props.source == 'spotify')) {
      this.props.spotifyActions.getLibraryArtists();
    }
  }

  componentDidUpdate = ({
    mopidy_connected: prev_mopidy_connected,
  }) => {
    const {
      source,
      mopidy_connected,
      google_available,
      spotify_available,
      mopidyActions,
      googleActions,
      spotifyActions,
      mopidy_library_artists,
      google_library_artists,
      spotify_library_artists_status,
    } = this.props;

    if (mopidy_connected && (source === 'all' || source === 'local')) {
      if (!prev_mopidy_connected) mopidyActions.getLibraryArtists();

      if (source !== 'all' && source !== 'local' && !mopidy_library_artists) {
        mopidyActions.getLibraryArtists();
      }
    }

    if (mopidy_connected && google_available && (source === 'all' || source === 'google')) {
      if (source !== 'all' && source !== 'google' && !google_library_artists) {
        googleActions.getLibraryArtists();
      }
    }

    if (spotify_available && (source === 'all' || source === 'spotify')) {
      if (spotify_library_artists_status !== 'finished' && spotify_library_artists_status !== 'started') {
        spotifyActions.getLibraryArtists();
      }
    }
  }

  handleContextMenu(e, item) {
    const data = {
      e,
      context: 'artist',
      uris: [item.uri],
      items: [item],
    };
    this.props.uiActions.showContextMenu(data);
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
      library_artists_sort_reverse: reverse,
      library_artists_sort: value,
    };
    this.props.uiActions.set(data);
  }

  renderView() {
    let artists = [];

    // Mopidy library items
    if (this.props.mopidy_library_artists && (this.props.source == 'all' || this.props.source == 'local')) {
      for (uri of this.props.mopidy_library_artists) {
        // Construct item placeholder. This is used as Mopidy needs to
        // lookup ref objects to get the full object which can take some time
        var source = uriSource(uri);
        var artist = {
          uri,
          source,
        };

        if (this.props.artists.hasOwnProperty(uri)) {
          artist = this.props.artists[uri];
        }

        artists.push(artist);
      }
    }

    // Google library items
    if (this.props.google_library_artists && (this.props.source == 'all' || this.props.source == 'google')) {
      for (uri of this.props.google_library_artists) {
        // Construct item placeholder. This is used as Mopidy needs to
        // lookup ref objects to get the full object which can take some time
        var source = uriSource(uri);
        var artist = {
          uri,
          source,
        };

        if (this.props.artists.hasOwnProperty(uri)) {
          artist = this.props.artists[uri];
        }

        artists.push(artist);
      }
    }

    // Spotify library items
    if (this.props.spotify_library_artists && (this.props.source == 'all' || this.props.source == 'spotify')) {
      for (let i = 0; i < this.props.spotify_library_artists.length; i++) {
        var uri = this.props.spotify_library_artists[i];
        if (this.props.artists.hasOwnProperty(uri)) {
          artists.push(this.props.artists[uri]);
        }
      }
    }

    if (this.props.sort) {
      artists = sortItems(artists, this.props.sort, this.props.sort_reverse);
    }

    if (this.state.filter !== '') {
      artists = applyFilter('name', this.state.filter, artists);
    }

    // Apply our lazy-load-rendering
    const total_artists = artists.length;
    artists = artists.slice(0, this.state.limit);

    if (this.props.view == 'list') {
      return (
        <section className="content-wrapper">
          <List
            handleContextMenu={(e, item) => this.handleContextMenu(e, item)}
            rows={artists}
            thumbnail
            details={['followers']}
            middle_column={['source']}
            className="artists"
            link_prefix="/artist/"
          />
          <LazyLoadListener
            loadKey={total_artists > this.state.limit ? this.state.limit : total_artists}
            showLoader={this.state.limit < total_artists}
            loadMore={() => this.loadMore()}
          />
        </section>
      );
    }
    return (
      <section className="content-wrapper">
        <ArtistGrid
          handleContextMenu={(e, item) => this.handleContextMenu(e, item)}
          artists={artists}
        />
        <LazyLoadListener
          loadKey={total_artists > this.state.limit ? this.state.limit : total_artists}
          showLoader={this.state.limit < total_artists}
          loadMore={() => this.loadMore()}
        />
      </section>
    );
  }

  render() {
    const source_options = [
      {
        value: 'all',
        label: i18n('fields.filters.all'),
      },
      {
        value: 'local',
        label: i18n('services.mopidy.local'),
      },
    ];

    if (this.props.spotify_available) {
      source_options.push({
        value: 'spotify',
        label: i18n('services.spotify.title'),
      });
    }

    if (this.props.google_available) {
      source_options.push({
        value: 'google',
        label: i18n('services.google.title'),
      });
    }

    const view_options = [
      {
        value: 'thumbnails',
        label: i18n('fields.filters.thumbnails'),
      },
      {
        value: 'list',
        label: i18n('fields.filters.list'),
      },
    ];

    const sort_options = [
      {
        value: null,
        label: i18n('fields.filters.as_loaded'),
      },
      {
        value: 'name',
        label: i18n('fields.filters.name'),
      },
      {
        value: 'followers',
        label: i18n('fields.filters.followers'),
      },
      {
        value: 'popularity',
        label: i18n('fields.filters.popularity'),
      },
    ];

    const options = (
      <span>
        <FilterField
          initialValue={this.state.filter}
          handleChange={(value) => this.setState({ filter: value, limit: this.state.per_page })}
          onSubmit={e => this.props.uiActions.hideContextMenu()}
        />
        <DropdownField
          icon="swap_vert"
          name={i18n('fields.sort')}
          value={this.props.sort}
          valueAsLabel
          options={sort_options}
          selected_icon={this.props.sort ? (this.props.sort_reverse ? 'keyboard_arrow_up' : 'keyboard_arrow_down') : null}
          handleChange={(value) => { this.setSort(value); this.props.uiActions.hideContextMenu(); }}
        />
        <DropdownField
          icon="visibility"
          name={i18n('fields.view')}
          value={this.props.view}
          valueAsLabel
          options={view_options}
          handleChange={(value) => { this.props.uiActions.set({ library_artists_view: value }); this.props.uiActions.hideContextMenu(); }}
        />
        <DropdownField
          icon="cloud"
          name={i18n('fields.source')}
          value={this.props.source}
          valueAsLabel
          options={source_options}
          handleChange={(value) => { this.props.uiActions.set({ library_artists_source: value }); this.props.uiActions.hideContextMenu(); }}
        />
      </span>
    );

    return (
      <div className="view library-artists-view">
        <Header options={options} uiActions={this.props.uiActions}>
          <Icon name="recent_actors" type="material" />
					<I18n path="library.artists.title" />
        </Header>
        {this.renderView()}
      </div>
    );
  }
}

const mapStateToProps = (state, ownProps) => ({
  mopidy_connected: state.mopidy.connected,
  mopidy_uri_schemes: state.mopidy.uri_schemes,
  mopidy_library_artists: state.mopidy.library_artists,
  google_available: (state.mopidy.uri_schemes && state.mopidy.uri_schemes.includes('gmusic:')),
  google_library_artists: state.google.library_artists,
  spotify_available: state.spotify.access_token,
  spotify_library_artists: state.spotify.library_artists,
  spotify_library_artists_status: (state.ui.processes.SPOTIFY_GET_LIBRARY_ARTISTS_PROCESSOR !== undefined ? state.ui.processes.SPOTIFY_GET_LIBRARY_ARTISTS_PROCESSOR.status : null),
  artists: state.core.artists,
  source: (state.ui.library_artists_source ? state.ui.library_artists_source : 'all'),
  sort: (state.ui.library_artists_sort ? state.ui.library_artists_sort : null),
  sort_reverse: (state.ui.library_artists_sort_reverse ? state.ui.library_artists_sort_reverse : false),
  view: state.ui.library_artists_view,
});

const mapDispatchToProps = (dispatch) => ({
  uiActions: bindActionCreators(uiActions, dispatch),
  mopidyActions: bindActionCreators(mopidyActions, dispatch),
  spotifyActions: bindActionCreators(spotifyActions, dispatch),
  googleActions: bindActionCreators(googleActions, dispatch),
});

export default connect(mapStateToProps, mapDispatchToProps)(LibraryArtists);
