import React from 'react';
import { connect } from 'react-redux';
import { bindActionCreators } from 'redux';
import Header from '../../components/Header';
import DropdownField from '../../components/Fields/DropdownField';
import FilterField from '../../components/Fields/FilterField';
import { Grid } from '../../components/Grid';
import { List } from '../../components/List';
import Icon from '../../components/Icon';
import * as coreActions from '../../services/core/actions';
import * as uiActions from '../../services/ui/actions';
import * as mopidyActions from '../../services/mopidy/actions';
import * as googleActions from '../../services/google/actions';
import * as spotifyActions from '../../services/spotify/actions';
import { sortItems, applyFilter } from '../../util/arrays';
import Button from '../../components/Button';
import { i18n, I18n } from '../../locale';
import Loader from '../../components/Loader';
import {
  makeLibrarySelector,
  makeProcessProgressSelector,
  getLibrarySource,
} from '../../util/selectors';

const processKeys = [
  'MOPIDY_GET_LIBRARY_ALBUMS',
  'SPOTIFY_GET_LIBRARY_ALBUMS',
  'GOOGLE_GET_LIBRARY_ALBUMS',
];

class LibraryAlbums extends React.Component {
  constructor(props) {
    super(props);

    this.state = {
      filter: '',
    };
  }

  componentDidMount() {
    const {
      uiActions: {
        setWindowTitle,
      },
    } = this.props;

    setWindowTitle(i18n('library.albums.title'));
    this.getMopidyLibrary();
    this.getGoogleLibrary();
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

  getMopidyLibrary = (forceRefetch = false) => {
    const {
      source,
      coreActions: {
        loadLibrary,
      },
    } = this.props;

    if (source !== 'local' && source !== 'all') return;

    loadLibrary('mopidy:library:albums', { forceRefetch });
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

    loadLibrary('google:library:albums', { forceRefetch });
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

    loadLibrary('spotify:library:albums', { forceRefetch });
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
      library_albums_sort_reverse: reverse,
      library_albums_sort: value,
    });
  }

  renderView = () => {
    const {
      sort,
      sort_reverse,
      view,
      loading_progress,
    } = this.props;
    const {
      filter,
    } = this.state;
    let { albums } = this.props;

    if (loading_progress) {
      return <Loader body loading progress={loading_progress} />;
    }

    if (sort) {
      albums = sortItems(albums, sort, sort_reverse);
    }

    if (filter && filter !== '') {
      albums = applyFilter('name', filter, albums);
    }

    if (view === 'list') {
      return (
        <section className="content-wrapper">
          <List
            items={albums}
            details={['artists', 'tracks', 'followers']}
            right_column={['source']}
            thumbnail
          />
        </section>
      );
    }
    return (
      <section className="content-wrapper">
        <Grid items={albums} />
      </section>
    );
  }

  render = () => {
    const {
      spotify_available,
      google_available,
      sort,
      view,
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
        value: 'artist',
        label: i18n('fields.filters.artist'),
      },
      {
        value: 'last_modified',
        label: i18n('fields.filters.updated'),
      },
      {
        value: 'tracks',
        label: i18n('fields.filters.tracks'),
      },
      {
        value: 'uri',
        label: i18n('fields.filters.source'),
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
          icon="visibility"
          name={i18n('fields.view')}
          value={view}
          valueAsLabel
          options={view_options}
          handleChange={(val) => { uiActions.set({ library_albums_view: val }); uiActions.hideContextMenu(); }}
        />
        <DropdownField
          icon="cloud"
          name={i18n('fields.source')}
          value={source}
          valueAsLabel
          options={source_options}
          handleChange={(val) => { uiActions.set({ library_albums_source: val }); uiActions.hideContextMenu(); }}
        />
        <Button
          noHover
          discrete
          onClick={loading_progress ? this.cancelRefresh : this.refresh}
          tracking={{ category: 'LibraryAlbums', action: 'Refresh' }}
        >
          {loading_progress ? <Icon name="close" /> : <Icon name="refresh" /> }
          {loading_progress ? <I18n path="actions.cancel" /> : <I18n path="actions.refresh" /> }
        </Button>
      </>
    );

    return (
      <div className="view library-albums-view">
        <Header options={options} uiActions={uiActions}>
          <Icon name="album" type="material" />
          <I18n path="library.albums.title" />
        </Header>
        {this.renderView()}
      </div>
    );
  }
}

const librarySelector = makeLibrarySelector('albums');
const processProgressSelector = makeProcessProgressSelector(processKeys);
const mapStateToProps = (state) => ({
  loading_progress: processProgressSelector(state),
  mopidy_uri_schemes: state.mopidy.uri_schemes,
  albums: librarySelector(state, 'albums'),
  google_available: (state.mopidy.uri_schemes && state.mopidy.uri_schemes.includes('gmusic:')),
  spotify_available: state.spotify.access_token,
  view: state.ui.library_albums_view,
  source: getLibrarySource(state, 'albums'),
  sort: state.ui.library_albums_sort,
  sort_reverse: state.ui.library_albums_sort_reverse,
});
const mapDispatchToProps = (dispatch) => ({
  coreActions: bindActionCreators(coreActions, dispatch),
  uiActions: bindActionCreators(uiActions, dispatch),
  mopidyActions: bindActionCreators(mopidyActions, dispatch),
  googleActions: bindActionCreators(googleActions, dispatch),
  spotifyActions: bindActionCreators(spotifyActions, dispatch),
});

export default connect(mapStateToProps, mapDispatchToProps)(LibraryAlbums);
