
import React from 'react';
import { connect } from 'react-redux';
import { bindActionCreators } from 'redux';
import Button from '../../components/Button';
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
import { I18n, i18n } from '../../locale';

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

    this.props.uiActions.setWindowTitle(i18n('library.playlists.title'));

    this.getMopidyLibrary();
    this.getSpotifyLibrary();
  }

  getMopidyLibrary = () => {
    const {
      source,
      mopidy_library_playlists,
      mopidyActions: {
        getLibraryPlaylists,
      },
    } = this.props;

    if (source !== 'local' && source !== 'all') return;
    if (mopidy_library_playlists) return;

    getLibraryPlaylists();
  };

  getSpotifyLibrary = () => {
    const {
      source,
      spotify_library_playlists_status,
      spotifyActions: {
        getLibraryPlaylists,
      },
    } = this.props;

    if (source !== 'spotify' && source !== 'all') return;
    if (spotify_library_playlists_status === 'finished') return;
    if (spotify_library_playlists_status === 'started') return;

    getLibraryPlaylists();
  };

  componentDidUpdate = ({ source: prevSource }) => {
    const { source } = this.props;

    if (source !== prevSource) {
      this.getMopidyLibrary();
      this.getSpotifyLibrary();
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
        value: 'last_modified',
        label: i18n('fields.filters.updated'),
      },
      {
        value: 'can_edit',
        label: i18n('fields.filters.editable'),
      },
      {
        value: 'owner.id',
        label: i18n('fields.filters.owner'),
      },
      {
        value: 'tracks_total',
        label: i18n('fields.filters.tracks'),
      },
      {
        value: 'source',
        label: i18n('fields.filters.source'),
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
          valueAsLabel
          value={this.props.view}
          options={view_options}
          handleChange={(value) => { this.props.uiActions.set({ library_playlists_view: value }); this.props.uiActions.hideContextMenu(); }}
        />
        <DropdownField
          icon="cloud"
          name={i18n('fields.source')}
          valueAsLabel
          value={this.props.source}
          options={source_options}
          handleChange={(value) => { this.props.uiActions.set({ library_playlists_source: value }); this.props.uiActions.hideContextMenu(); }}
        />
        <Button
          to="/playlist/create"
          noHover
          discrete
          tracking={{ category: 'Playlist', action: 'Create' }}
        >
          <Icon name="add_box" />
          <I18n path="actions.add" />
        </Button>
      </span>
    );

    return (
      <div className="view library-playlists-view">
        <Header options={options} uiActions={this.props.uiActions}>
          <Icon name="queue_music" type="material" />
          <I18n path="library.playlists.title" />
        </Header>
        { this.renderView() }
      </div>
    );
  }
}

const mapStateToProps = (state) => ({
  slim_mode: state.ui.slim_mode,
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
