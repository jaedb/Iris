import React from 'react';
import { connect } from 'react-redux';
import { bindActionCreators } from 'redux';
import Header from '../../components/Header';
import DropdownField from '../../components/Fields/DropdownField';
import FilterField from '../../components/Fields/FilterField';
import { Grid } from '../../components/Grid';
import { List } from '../../components/List';
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
  getLibrarySource,
  makeProvidersSelector,
} from '../../util/selectors';

const processKeys = [
  'MOPIDY_GET_LIBRARY_ARTISTS',
  'SPOTIFY_GET_LIBRARY_ARTISTS',
];

class LibraryArtists extends React.Component {
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

    setWindowTitle(i18n('library.artists.title'));
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
    uris.forEach((uri) => loadLibrary(uri, 'artists', { forceRefetch }));
  };

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

    if (view === 'list') {
      return (
        <section className="content-wrapper">
          <List
            items={artists}
            details={['albums', 'followers']}
            right_column={['source']}
            thumbnail
          />
        </section>
      );
    }
    return (
      <section className="content-wrapper">
        <Grid items={artists} />
      </section>
    );
  }

  render = () => {
    const {
      loading_progress,
      providers,
    } = this.props;

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
          onSubmit={(e) => this.props.uiActions.hideContextMenu()}
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
          options={[
            {
              value: 'all',
              label: i18n('fields.filters.all'),
            },
            ...providers.map((p) => ({ value: p.uri, label: p.title })),
          ]}
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

const librarySelector = makeLibrarySelector('artists');
const processProgressSelector = makeProcessProgressSelector(processKeys);
const providersSelector = makeProvidersSelector('artists');
const mapStateToProps = (state) => ({
  loading_progress: processProgressSelector(state),
  uri_schemes: state.mopidy.uri_schemes,
  providers: providersSelector(state),
  artists: librarySelector(state, 'artists'),
  source: getLibrarySource(state, 'artists'),
  sort: (state.ui.library_artists_sort ? state.ui.library_artists_sort : null),
  sort_reverse: (state.ui.library_artists_sort_reverse ? state.ui.library_artists_sort_reverse : false),
  view: state.ui.library_artists_view,
});

const mapDispatchToProps = (dispatch) => ({
  uiActions: bindActionCreators(uiActions, dispatch),
  coreActions: bindActionCreators(coreActions, dispatch),
});

export default connect(mapStateToProps, mapDispatchToProps)(LibraryArtists);
