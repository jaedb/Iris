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
  getLibrarySource,
  makeProvidersSelector,
  getSortSelector,
} from '../../util/selectors';

const SORT_KEY = 'library_tracks';
const processKeys = [
  'MOPIDY_GET_LIBRARY_TRACKS',
  'SPOTIFY_GET_LIBRARY_TRACKS',
];

class Tracks extends React.Component {
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

    setWindowTitle(i18n('library.tracks.title'));
    this.getLibraries();
  }

  componentDidUpdate = ({ source: prevSource }) => {
    const { source } = this.props;

    if (source !== prevSource) {
      this.getLibraries();
    }
  }

  refresh = () => {
    const { uiActions: { hideContextMenu } } = this.props;

    hideContextMenu();
    this.getLibraries(true);
  }

  cancelRefresh = () => {
    const { uiActions: { hideContextMenu, cancelProcess } } = this.props;

    hideContextMenu();
    cancelProcess(processKeys);
  }

  getLibraries = (forceRefetch = false) => {
    const {
      source,
      providers,
      coreActions: {
        loadLibrary,
      },
    } = this.props;

    let uris = [];
    if (source === 'all') {
      uris = providers.map((p) => p.uri);
    } else {
      uris.push(source);
    }
    uris.forEach((uri) => loadLibrary(uri, 'tracks', { forceRefetch }));
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

  onSortChange = (field) => {
    const {
      sortField,
      sortReverse,
      uiActions: {
        setSort,
        hideContextMenu,
      },
    } = this.props;

    let reverse = false;
    if (field !== null && sortField === field) {
      reverse = !sortReverse;
    }

    setSort(SORT_KEY, field, reverse);
    hideContextMenu();
  }

  playAll = () => {
    const {
      sortField,
      sortReverse,
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

    if (sortField) {
      tracks = sortItems(tracks, sortField, sortReverse);
    }

    if (filter && filter !== '') {
      tracks = applyFilter('name', filter, tracks);
    }

    playURIs(arrayOf('uri', tracks));
    hideContextMenu();
  }

  renderView = () => {
    const {
      sortField,
      sortReverse,
      loading_progress,
    } = this.props;
    const {
      filter,
    } = this.state;
    let { tracks } = this.props;

    if (loading_progress) {
      return <Loader body loading progress={loading_progress} />;
    }

    if (sortField) {
      tracks = sortItems(tracks, sortField, sortReverse);
    }

    if (filter && filter !== '') {
      tracks = applyFilter('name', filter, tracks);
    }

    return (
      <section className="content-wrapper">
        <TrackList
          context={{
            name: 'Tracks',
          }}
          tracks={tracks}
        />
      </section>
    );
  }

  render = () => {
    const {
      source,
      providers,
      sortField,
      sortReverse,
      uiActions,
      loading_progress,
    } = this.props;
    const {
      filter,
    } = this.state;

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
          handleChange={(value) => this.setState({ filter: value })}
          onSubmit={() => uiActions.hideContextMenu()}
        />
        <DropdownField
          icon="swap_vert"
          name={i18n('fields.sort')}
          value={sortField}
          valueAsLabel
          options={sort_options}
          selected_icon={sortField ? (sortReverse ? 'keyboard_arrow_up' : 'keyboard_arrow_down') : null}
          handleChange={this.onSortChange}
        />
        <DropdownField
          icon="cloud"
          name={i18n('fields.source')}
          value={source}
          valueAsLabel
          options={[
            {
              value: 'all',
              label: i18n('fields.filters.all'),
            },
            ...providers.map((p) => ({ value: p.uri, label: p.title })),
          ]}
          handleChange={
            (val) => {
              uiActions.set({ library_tracks_source: val });
              uiActions.hideContextMenu();
            }
          }
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

const librarySelector = makeLibrarySelector('tracks');
const processProgressSelector = makeProcessProgressSelector(processKeys);
const providersSelector = makeProvidersSelector('tracks');
const mapStateToProps = (state) => {
  const [sortField, sortReverse] = getSortSelector(state, SORT_KEY, null);

  return {
    loading_progress: processProgressSelector(state),
    mopidy_uri_schemes: state.mopidy.uri_schemes,
    tracks: librarySelector(state, 'tracks'),
    view: state.ui.library_tracks_view,
    source: getLibrarySource(state, 'tracks'),
    providers: providersSelector(state),
    sortField,
    sortReverse,
  };
};

const mapDispatchToProps = (dispatch) => ({
  coreActions: bindActionCreators(coreActions, dispatch),
  uiActions: bindActionCreators(uiActions, dispatch),
  mopidyActions: bindActionCreators(mopidyActions, dispatch),
  spotifyActions: bindActionCreators(spotifyActions, dispatch),
});

export default connect(mapStateToProps, mapDispatchToProps)(Tracks);
