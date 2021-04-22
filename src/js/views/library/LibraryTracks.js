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

    return (
      <section className="content-wrapper">
        <TrackList tracks={tracks} />
      </section>
    );
  }

  render = () => {
    const {
      sort,
      source,
      providers,
      sort_reverse,
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
const mapStateToProps = (state) => ({
  loading_progress: processProgressSelector(state),
  mopidy_uri_schemes: state.mopidy.uri_schemes,
  tracks: librarySelector(state, 'tracks'),
  view: state.ui.library_tracks_view,
  source: getLibrarySource(state, 'tracks'),
  providers: providersSelector(state),
  sort: state.ui.library_tracks_sort,
  sort_reverse: state.ui.library_tracks_sort_reverse,
});

const mapDispatchToProps = (dispatch) => ({
  coreActions: bindActionCreators(coreActions, dispatch),
  uiActions: bindActionCreators(uiActions, dispatch),
  mopidyActions: bindActionCreators(mopidyActions, dispatch),
  spotifyActions: bindActionCreators(spotifyActions, dispatch),
});

export default connect(mapStateToProps, mapDispatchToProps)(LibraryTracks);
