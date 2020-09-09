
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
import { collate } from '../../util/format';

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
    const {
      location: {
        state: {
          limit,
        } = {},
      } = {},
      uiActions: {
        setWindowTitle,
      },
    } = this.props;
    if (limit) {
      this.setState({
        limit,
      });
    }

    setWindowTitle(i18n('library.playlists.title'));

    this.getMopidyLibrary();
    this.getSpotifyLibrary();
  }

  onRefresh = () => {
    const { uiActions: { hideContextMenu } } = this.props;

    hideContextMenu();
    this.getMopidyLibrary(true);
    this.getSpotifyLibrary(true);
  }

  getMopidyLibrary = (forceRefetch = false) => {
    const {
      source,
      coreActions: {
        loadLibrary,
      },
    } = this.props;

    if (source !== 'local' && source !== 'all') return;

    loadLibrary('mopidy:library:playlists', { forceRefetch });
  };

  getSpotifyLibrary = (forceRefetch = false) => {
    const {
      source,
      spotify_available,
      coreActions: {
        loadLibrary,
      },
    } = this.props;

    if (!spotify_available) return;
    if (source !== 'spotify' && source !== 'all') return;

    loadLibrary('spotify:library:playlists', { forceRefetch });
  };

  componentDidUpdate = ({ source: prevSource }) => {
    const { source } = this.props;

    if (source !== prevSource) {
      this.getMopidyLibrary();
      this.getSpotifyLibrary();
    }
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

  renderView = () => {
    const {
      spotify_library,
      mopidy_library,
      items,
      sort,
      sort_reverse,
      view,
      source,
    } = this.props;
    const {
      filter,
      limit,
    } = this.state;

    let playlists = [
      ...(source === 'all' || source === 'spotify' ? collate(spotify_library, { items }).items : []),
      ...(source === 'all' || source === 'local' ? collate(mopidy_library, { items }).items : []),
    ];

    if (sort) {
      playlists = sortItems(playlists, sort, sort_reverse);
    }
    playlists = removeDuplicates(playlists);

    if (filter !== '') {
      playlists = applyFilter('name', filter, playlists);
    }

    // Apply our lazy-load-rendering
    const total_playlists = playlists.length;
    playlists = playlists.slice(0, limit);

    if (view === 'list') {
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
            loadKey={total_playlists > limit ? limit : total_playlists}
            loading={limit < total_playlists}
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
          loadKey={total_playlists > limit ? limit : total_playlists}
          loading={limit < total_playlists}
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
        <Button
          noHover
          onClick={this.onRefresh}
          tracking={{ category: 'LibraryAlbums', action: 'Refresh' }}
        >
          <Icon name="refresh" />
          <I18n path="actions.refresh" />
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

const mapStateToProps = (state) => {
  return {
    slim_mode: state.ui.slim_mode,
    mopidy_uri_schemes: state.mopidy.uri_schemes,
    spotify_available: state.spotify.access_token,
    items: state.core.items,
    mopidy_library: state.core.libraries['mopidy:library:playlists'] || { items_uris: [] },
    spotify_library: state.core.libraries['spotify:library:playlists'] || { items_uris: [] },
    load_queue: state.ui.load_queue,
    me_id: (state.spotify.me ? state.spotify.me.id : false),
    view: state.ui.library_playlists_view,
    source: (state.ui.library_playlists_source ? state.ui.library_playlists_source : 'all'),
    sort: (state.ui.library_playlists_sort ? state.ui.library_playlists_sort : null),
    sort_reverse: (state.ui.library_playlists_sort_reverse ? state.ui.library_playlists_sort_reverse : false),
    playlists: state.core.playlists,
  };
};

const mapDispatchToProps = (dispatch) => ({
  coreActions: bindActionCreators(coreActions, dispatch),
  uiActions: bindActionCreators(uiActions, dispatch),
  mopidyActions: bindActionCreators(mopidyActions, dispatch),
  spotifyActions: bindActionCreators(spotifyActions, dispatch),
});

export default connect(mapStateToProps, mapDispatchToProps)(LibraryPlaylists);
