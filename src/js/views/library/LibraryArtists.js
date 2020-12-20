
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
import * as coreActions from '../../services/core/actions';
import { sortItems, applyFilter } from '../../util/arrays';
import { I18n, i18n } from '../../locale';
import Button from '../../components/Button';
import Loader from '../../components/Loader';
import {
  makeLibrarySelector,
  makeProcessProgressSelector,
} from '../../util/selectors';

const processKeys = [
  'MOPIDY_GET_LIBRARY_ARTISTS',
  'SPOTIFY_GET_LIBRARY_ARTISTS',
  'GOOGLE_GET_LIBRARY_ARTISTS',
];

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
    const {
      uiActions: {
        setWindowTitle,
      },
    } = this.props;

    // Restore any limit defined in our location state
    const state = (this.props.location.state ? this.props.location.state : {});
    if (state.limit) {
      this.setState({
        limit: state.limit,
      });
    }

    setWindowTitle(i18n('library.artists.title'));

    this.getMopidyLibrary();
    this.getSpotifyLibrary();
  }

  componentDidUpdate = ({ source: prevSource }) => {
    const { source } = this.props;

    if (source !== prevSource) {
      this.getMopidyLibrary();
      this.getGoogleLibrary();
      this.getSpotifyLibrary();
    }
  }

  refresh = () => {
    const { uiActions: { hideContextMenu } } = this.props;

    hideContextMenu();
    this.getMopidyLibrary(true);
    this.getGoogleLibrary(true);
    this.getSpotifyLibrary(true);
  }

  cancelRefresh = () => {
    const { uiActions: { hideContextMenu, cancelProcess } } = this.props;

    hideContextMenu();
    cancelProcess(processKeys);
  }

  setSort = (value) => {
    const { sort, sort_reverse, uiActions: { set } } = this.props;
    let reverse = false;
    if (sort === value) reverse = !sort_reverse;
    set({
      library_artists_sort_reverse: reverse,
      library_artists_sort: value,
    });
  }

  handleContextMenu = (e, item) => {
    const { uiActions: { showContextMenu } } = this.props;
    showContextMenu({
      e,
      context: 'artist',
      uris: [item.uri],
      items: [item],
    });
  }

  getMopidyLibrary = (forceRefetch = false) => {
    const {
      source,
      coreActions: {
        loadLibrary,
      },
    } = this.props;

    if (source !== 'local' && source !== 'all') return;

    loadLibrary('mopidy:library:artists', { forceRefetch });
  };

  getGoogleLibrary = (forceRefetch = false) => {
    const {
      source,
      google_available,
      coreActions: {
        loadLibrary,
      },
    } = this.props;

    if (!google_available) return;
    if (source !== 'google' && source !== 'all') return;

    loadLibrary('google:library:artists', { forceRefetch });
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

    loadLibrary('spotify:library:artists', { forceRefetch });
  };

  loadMore = () => {
    const new_limit = this.state.limit + this.state.per_page;

    this.setState({ limit: new_limit });

    // Set our pagination to location state
    const state = (this.props.location && this.props.location.state ? this.props.location.state : {});
    state.limit = new_limit;
    this.props.history.replace({ state });
  }

  renderView = () => {
    const {
      sort,
      sort_reverse,
      view,
      loading_progress,
    } = this.props;
    const {
      limit,
      filter,
    } = this.state;
    let { artists } = this.props;

    if (loading_progress) {
      return (
        <Loader body loading progress={loading_progress} />
      );
    }

    if (sort) {
      artists = sortItems(artists, sort, sort_reverse);
    }

    if (filter !== '') {
      artists = applyFilter('name', filter, artists);
    }

    // Apply our lazy-load-rendering
    const total_artists = artists.length;
    artists = artists.slice(0, limit);

    if (view === 'list') {
      return (
        <section className="content-wrapper">
          <List
            handleContextMenu={(e, item) => this.handleContextMenu(e, item)}
            rows={artists}
            thumbnail
            details={['followers', 'listeners']}
            right_column={['source', 'albums']}
            className="artists"
            link_prefix="/artist/"
          />
          <LazyLoadListener
            loadKey={total_artists > limit ? limit : total_artists}
            showLoader={limit < total_artists}
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
          loadKey={total_artists > limit ? limit : total_artists}
          showLoader={limit < total_artists}
          loadMore={() => this.loadMore()}
        />
      </section>
    );
  }

  render = () => {
    const {
      spotify_available,
      google_available,
      loading_progress,
    } = this.props;

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

    if (google_available) {
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
      <>
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
        <Button
          noHover
          discrete
          onClick={loading_progress ? this.cancelRefresh : this.refresh}
          tracking={{ category: 'LibraryArtists', action: 'Refresh' }}
        >
          {loading_progress ? <Icon name="close" /> : <Icon name="refresh" /> }
          {loading_progress ? <I18n path="actions.cancel" /> : <I18n path="actions.refresh" /> }
        </Button>
      </>
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

const mapStateToProps = (state) => {
  const source = state.ui.library_artists_source || 'all';

  const libraryUris = [];
  if (source === 'all' || source === 'local') libraryUris.push('mopidy:library:artists');
  if (source === 'all' || source === 'spotify') libraryUris.push('spotify:library:artists');
  if (source === 'all' || source === 'google') libraryUris.push('google:library:artists');
  const librarySelector = makeLibrarySelector(libraryUris);
  const processProgressSelector = makeProcessProgressSelector(processKeys);

  return {
    loading_progress: processProgressSelector(state),
    mopidy_uri_schemes: state.mopidy.uri_schemes,
    google_available: (state.mopidy.uri_schemes && state.mopidy.uri_schemes.includes('gmusic:')),
    spotify_available: (state.spotify.access_token),
    artists: librarySelector(state),
    source,
    sort: (state.ui.library_artists_sort ? state.ui.library_artists_sort : null),
    sort_reverse: (state.ui.library_artists_sort_reverse ? state.ui.library_artists_sort_reverse : false),
    view: state.ui.library_artists_view,
  };
};

const mapDispatchToProps = (dispatch) => ({
  uiActions: bindActionCreators(uiActions, dispatch),
  coreActions: bindActionCreators(coreActions, dispatch),
});

export default connect(mapStateToProps, mapDispatchToProps)(LibraryArtists);
