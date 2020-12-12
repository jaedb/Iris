
import React from 'react';
import { connect } from 'react-redux';
import { bindActionCreators } from 'redux';
import TrackList from '../../components/TrackList';
import Header from '../../components/Header';
import DropdownField from '../../components/Fields/DropdownField';
import FilterField from '../../components/Fields/FilterField';
import LazyLoadListener from '../../components/LazyLoadListener';
import Icon from '../../components/Icon';
import * as coreActions from '../../services/core/actions';
import * as uiActions from '../../services/ui/actions';
import * as mopidyActions from '../../services/mopidy/actions';
import * as spotifyActions from '../../services/spotify/actions';
import { sortItems, applyFilter, arrayOf } from '../../util/arrays';
import Button from '../../components/Button';
import { i18n, I18n } from '../../locale';
import Loader from '../../components/Loader';
import {
  makeLibrarySelector,
  makeProcessProgressSelector,
} from '../../util/selectors';

const processKeys = [
  'MOPIDY_GET_LIBRARY_TRACKS',
  'SPOTIFY_GET_LIBRARY_TRACKS',
];

class LibraryTracks extends React.Component {
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
        state = {},
      },
      uiActions: {
        setWindowTitle,
      },
    } = this.props;

    // Restore any limit defined in our location state
    if (state.limit) {
      this.setState({
        limit: state.limit,
      });
    }

    setWindowTitle(i18n('library.tracks.title'));
    this.getMopidyLibrary();
    this.getSpotifyLibrary();
  }

  componentDidUpdate = ({ source: prevSource }) => {
    const { source } = this.props;

    if (source !== prevSource) {
      this.getMopidyLibrary();
      this.getSpotifyLibrary();
    }
  }

  refresh = () => {
    const { uiActions: { hideContextMenu } } = this.props;

    hideContextMenu();
    this.getMopidyLibrary(true);
    this.getSpotifyLibrary(true);
  }

  cancelRefresh = () => {
    const { uiActions: { hideContextMenu, cancelProcess } } = this.props;

    hideContextMenu();
    cancelProcess(processKeys);
  }

  getMopidyLibrary = (forceRefetch = false) => {
    const {
      source,
      coreActions: {
        loadLibrary,
      },
    } = this.props;

    if (source !== 'local' && source !== 'all') return;

    loadLibrary('mopidy:library:tracks', { forceRefetch });
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

    loadLibrary('spotify:library:tracks', { forceRefetch });
  };

  handleContextMenu = (e, item) => {
    const {
      uiActions: {
        showContextMenu,
      },
    } = this.props;

    showContextMenu({
      e,
      context: 'album',
      uris: [item.uri],
      items: [item],
    });
  }

  loadMore = () => {
    const {
      limit,
      per_page,
    } = this.state;
    const {
      location: {
        state,
      },
      history,
    } = this.props;

    const new_limit = limit + per_page;
    this.setState({ limit: new_limit });
    history.replace({ state: { ...state, limit: new_limit } });
  }

  setSort = (value) => {
    const {
      sort,
      sort_reverse,
      uiActions: {
        set,
      },
    } = this.props;

    let reverse = false;
    if (sort === value) reverse = !sort_reverse;

    set({
      library_tracks_sort_reverse: reverse,
      library_tracks_sort: value,
    });
  }

  playAll = () => {
    const {
      sort,
      sort_reverse,
      mopidyActions: {
        playURIs,
      },
      uiActions: {
        hideContextMenu,
      },
    } = this.props;
    let { tracks } = this.props;
    const { filter } = this.state;

    if (!tracks || !tracks.length) return;

    if (sort) {
      tracks = sortItems(tracks, sort, sort_reverse);
    }

    if (filter && filter !== '') {
      tracks = applyFilter('name', filter, tracks);
    }

    playURIs(arrayOf('uri', tracks));
    hideContextMenu();
  }

  renderView = () => {
    const {
      sort,
      sort_reverse,
      loading_progress,
    } = this.props;
    const {
      limit,
      filter,
    } = this.state;
    let { tracks } = this.props;

    if (loading_progress) {
      return <Loader body loading progress={loading_progress} />;
    }

    if (sort) {
      tracks = sortItems(tracks, sort, sort_reverse);
    }

    if (filter && filter !== '') {
      tracks = applyFilter('name', filter, tracks);
    }

    // Apply our lazy-load-rendering
    const total_tracks = tracks.length;
    tracks = tracks.slice(0, limit);

    return (
      <section className="content-wrapper">
        <TrackList
          tracks={tracks}
        />
        <LazyLoadListener
          loadKey={total_tracks > limit ? limit : total_tracks}
          showLoader={limit < total_tracks}
          loadMore={this.loadMore}
        />
      </section>
    );
  }

  render = () => {
    const {
      spotify_available,
      sort,
      source,
      sort_reverse,
      uiActions,
      loading_progress,
    } = this.props;
    const {
      filter,
      per_page,
    } = this.state;

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

    if (spotify_available) {
      source_options.push({
        value: 'spotify',
        label: i18n('services.spotify.title'),
      });
    }

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
        value: 'artist',
        label: i18n('fields.filters.artist'),
      },
      {
        value: 'album',
        label: i18n('fields.filters.album'),
      },
    ];

    const options = (
      <>
        <FilterField
          initialValue={filter}
          handleChange={(value) => this.setState({ filter: value, limit: per_page })}
          onSubmit={() => uiActions.hideContextMenu()}
        />
        <DropdownField
          icon="swap_vert"
          name={i18n('fields.sort')}
          value={sort}
          valueAsLabel
          options={sort_options}
          selected_icon={sort ? (sort_reverse ? 'keyboard_arrow_up' : 'keyboard_arrow_down') : null}
          handleChange={(val) => { this.setSort(val); uiActions.hideContextMenu(); }}
        />
        <DropdownField
          icon="cloud"
          name={i18n('fields.source')}
          value={source}
          valueAsLabel
          options={source_options}
          handleChange={(val) => { uiActions.set({ library_tracks_source: val }); uiActions.hideContextMenu(); }}
        />
        <Button
          onClick={this.playAll}
          noHover
          discrete
          tracking={{ category: 'LibraryTracks', action: 'Play' }}
        >
          <Icon name="play_circle_filled" />
          <I18n path="actions.play_all" />
        </Button>
        <Button
          noHover
          discrete
          onClick={loading_progress ? this.cancelRefresh : this.refresh}
          tracking={{ category: 'LibraryTracks', action: 'Refresh' }}
        >
          {loading_progress ? <Icon name="close" /> : <Icon name="refresh" /> }
          {loading_progress ? <I18n path="actions.cancel" /> : <I18n path="actions.refresh" /> }
        </Button>
      </>
    );

    return (
      <div className="view library-tracks-view">
        <Header options={options} uiActions={uiActions}>
          <Icon name="album" type="material" />
          <I18n path="library.tracks.title" />
        </Header>
        {this.renderView()}
      </div>
    );
  }
}

const mapStateToProps = (state) => {
  const source = state.ui.library_tracks_source ? state.ui.library_tracks_source : 'all';

  const libraryUris = [];
  if (source === 'all' || source === 'local') libraryUris.push('mopidy:library:tracks');
  if (source === 'all' || source === 'spotify') libraryUris.push('spotify:library:tracks');
  const librarySelector = makeLibrarySelector(libraryUris);
  const processProgressSelector = makeProcessProgressSelector(processKeys);

  return {
    loading_progress: processProgressSelector(state),
    mopidy_uri_schemes: state.mopidy.uri_schemes,
    tracks: librarySelector(state),
    spotify_available: state.spotify.access_token,
    view: state.ui.library_tracks_view,
    source,
    sort: state.ui.library_tracks_sort,
    sort_reverse: state.ui.library_tracks_sort_reverse,
  };
};

const mapDispatchToProps = (dispatch) => ({
  coreActions: bindActionCreators(coreActions, dispatch),
  uiActions: bindActionCreators(uiActions, dispatch),
  mopidyActions: bindActionCreators(mopidyActions, dispatch),
  spotifyActions: bindActionCreators(spotifyActions, dispatch),
});

export default connect(mapStateToProps, mapDispatchToProps)(LibraryTracks);
