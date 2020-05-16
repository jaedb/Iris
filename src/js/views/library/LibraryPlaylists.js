
import React from 'react';
import { connect } from 'react-redux';
import { bindActionCreators } from 'redux';
import Link from '../../components/Link';
import PlaylistGrid from '../../components/PlaylistGrid';
import List from '../../components/List';
import DropdownField from '../../components/Fields/DropdownField';
import Header from '../../components/Header';
import FilterField from '../../components/Fields/FilterField';
import LazyLoadListener from '../../components/LazyLoadListener';
import Icon from '../../components/Icon';
import * as coreActions from '../../services/core/actions';
import * as uiActions from '../../services/ui/actions';
import * as mopidyActions from '../../services/mopidy/actions';
import * as spotifyActions from '../../services/spotify/actions';
import { applyFilter, removeDuplicates, sortItems } from '../../util/arrays';

class LibraryPlaylists extends React.Component {
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

    this.props.uiActions.setWindowTitle('Playlists');

    if (!this.props.mopidy_library_playlists && this.props.mopidy_connected && (this.props.source == 'all' || this.props.source == 'local')) {
      this.props.mopidyActions.getLibraryPlaylists();
    }

    if (this.props.spotify_available && this.props.spotify_library_playlists_status !== 'finished' && (this.props.source == 'all' || this.props.source == 'spotify')) {
      this.props.spotifyActions.getLibraryPlaylists();
    }
  }

  componentDidUpdate = ({
    mopidy_connected: prev_mopidy_connected,
  }) => {
    const {
      source,
      mopidy_connected,
      mopidy_library_playlists,
      spotify_available,
      spotify_library_playlists_status,
      mopidyActions,
      spotifyActions,
    } = this.props;

    if (mopidy_connected && (source == 'all' || source == 'local')) {
      if (!prev_mopidy_connected) mopidyActions.getLibraryPlaylists();

      if (source !== 'all' && source !== 'local' && !mopidy_library_playlists) {
        this.props.mopidyActions.getLibraryPlaylists();
      }
    }

    if (spotify_available && (source === 'all' || source === 'spotify')) {
      if (spotify_library_playlists_status !== 'finished' && spotify_library_playlists_status !== 'started') {
        spotifyActions.getLibraryPlaylists();
      }
    }
  }

  handleContextMenu(e, item) {
    const data = {
      e,
      context: 'playlist',
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
      library_playlists_sort_reverse: reverse,
      library_playlists_sort: value,
    };
    this.props.uiActions.set(data);
  }

  renderView() {
    let playlists = [];

    // Spotify library items
    if (this.props.spotify_library_playlists && (this.props.source == 'all' || this.props.source == 'spotify')) {
      for (var i = 0; i < this.props.spotify_library_playlists.length; i++) {
        var uri = this.props.spotify_library_playlists[i];
        if (this.props.playlists.hasOwnProperty(uri)) {
          playlists.push(this.props.playlists[uri]);
        }
      }
    }

    // Mopidy library items
    if (this.props.mopidy_library_playlists && (this.props.source == 'all' || this.props.source == 'local')) {
      for (var i = 0; i < this.props.mopidy_library_playlists.length; i++) {
        var uri = this.props.mopidy_library_playlists[i];
        if (this.props.playlists.hasOwnProperty(uri)) {
          playlists.push(this.props.playlists[uri]);
        }
      }
    }

    if (this.props.sort) {
      playlists = sortItems(playlists, this.props.sort, this.props.sort_reverse);
    }
    playlists = removeDuplicates(playlists);

    if (this.state.filter !== '') {
      playlists = applyFilter('name', this.state.filter, playlists);
    }

    // Apply our lazy-load-rendering
    const total_playlists = playlists.length;
    playlists = playlists.slice(0, this.state.limit);

    if (this.props.view == 'list') {
      return (
        <section className="content-wrapper">
          <List
            handleContextMenu={(e, item) => this.handleContextMenu(e, item)}
            rows={playlists}
            thumbnail
            details={['owner', 'tracks_total', 'last_modified']}
            right_column={['source']}
            className="playlists"
            link_prefix="/playlist/"
          />
          <LazyLoadListener
            loadKey={total_playlists > this.state.limit ? this.state.limit : total_playlists}
            loading={this.state.limit < total_playlists}
            loadMore={() => this.loadMore()}
          />
        </section>
      );
    }
    return (
      <section className="content-wrapper">
        <PlaylistGrid
          handleContextMenu={(e, item) => this.handleContextMenu(e, item)}
          playlists={playlists}
        />
        <LazyLoadListener
          loadKey={total_playlists > this.state.limit ? this.state.limit : total_playlists}
          loading={this.state.limit < total_playlists}
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
        value: 'last_modified',
        label: 'Updated',
      },
      {
        value: 'can_edit',
        label: 'Editable',
      },
      {
        value: 'owner.id',
        label: 'Owner',
      },
      {
        value: 'tracks_total',
        label: 'Tracks',
      },
      {
        value: 'source',
        label: 'Source',
      },
    ];

    const options = (
      <span>
        <FilterField
          initialValue={this.state.filter}
          handleChange={(value) => this.setState({ filter: value })}
          onSubmit={e => this.props.uiActions.hideContextMenu()}
        />
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
          icon="visibility"
          name="View"
          valueAsLabel
          value={this.props.view}
          options={view_options}
          handleChange={(value) => { this.props.uiActions.set({ library_playlists_view: value }); this.props.uiActions.hideContextMenu(); }}
        />
        <DropdownField
          icon="cloud"
          name="Source"
          valueAsLabel
          value={this.props.source}
          options={source_options}
          handleChange={(value) => { this.props.uiActions.set({ library_playlists_source: value }); this.props.uiActions.hideContextMenu(); }}
        />
        <Link className="button button--no-hover" to="/playlist/create">
          <Icon name="add_box" />
          New
        </Link>
      </span>
    );

    return (
      <div className="view library-playlists-view">
        <Header options={options} uiActions={this.props.uiActions}>
          <Icon name="queue_music" type="material" />
					My playlists
        </Header>
        { this.renderView() }
      </div>
    );
  }
}

const mapStateToProps = (state, ownProps) => ({
  slim_mode: state.ui.slim_mode,
  mopidy_connected: state.mopidy.connected,
  mopidy_uri_schemes: state.mopidy.uri_schemes,
  mopidy_library_playlists: state.mopidy.library_playlists,
  mopidy_library_playlists_status: (state.ui.processes.MOPIDY_LIBRARY_PLAYLISTS_PROCESSOR !== undefined ? state.ui.processes.MOPIDY_LIBRARY_PLAYLISTS_PROCESSOR.status : null),
  spotify_available: state.spotify.access_token,
  spotify_library_playlists: state.spotify.library_playlists,
  spotify_library_playlists_status: (state.ui.processes.SPOTIFY_GET_LIBRARY_PLAYLISTS_PROCESSOR !== undefined ? state.ui.processes.SPOTIFY_GET_LIBRARY_PLAYLISTS_PROCESSOR.status : null),
  load_queue: state.ui.load_queue,
  me_id: (state.spotify.me ? state.spotify.me.id : false),
  view: state.ui.library_playlists_view,
  source: (state.ui.library_playlists_source ? state.ui.library_playlists_source : 'all'),
  sort: (state.ui.library_playlists_sort ? state.ui.library_playlists_sort : null),
  sort_reverse: (state.ui.library_playlists_sort_reverse ? state.ui.library_playlists_sort_reverse : false),
  playlists: state.core.playlists,
});

const mapDispatchToProps = (dispatch) => ({
  coreActions: bindActionCreators(coreActions, dispatch),
  uiActions: bindActionCreators(uiActions, dispatch),
  mopidyActions: bindActionCreators(mopidyActions, dispatch),
  spotifyActions: bindActionCreators(spotifyActions, dispatch),
});

export default connect(mapStateToProps, mapDispatchToProps)(LibraryPlaylists);
