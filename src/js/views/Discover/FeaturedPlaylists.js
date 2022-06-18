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
import * as spotifyActions from '../../services/spotify/actions';
import { sortItems, applyFilter } from '../../util/arrays';
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
import { encodeUri } from '../../util/format';

const SORT_KEY = 'library_featured_playlists';
const processKeys = [
  'MOPIDY_GET_LIBRARY_FEATURED_PLAYLISTS',
  'SPOTIFY_GET_LIBRARY_FEATURED_PLAYLISTS',
];

class FeaturedPlaylists extends React.Component {
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

    setWindowTitle(i18n('discover.featured_playlists.title'));
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
    uris.forEach((uri) => loadLibrary(uri, 'featuredPlaylists', { forceRefetch }));
  };

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

  renderView = () => {
    const {
      sortField,
      sortReverse,
      view,
      loading_progress,
    } = this.props;
    const {
      filter,
    } = this.state;
    let { featured_playlists } = this.props;

    if (loading_progress) {
      return <Loader body loading progress={loading_progress} />;
    }

    if (sortField) {
      featured_playlists = sortItems(featured_playlists, sortField, sortReverse);
    }

    if (filter && filter !== '') {
      featured_playlists = applyFilter('name', filter, featured_playlists);
    }

    if (view === 'list') {
      return (
        <section className="content-wrapper">
          <List
            items={featured_playlists}
            details={['playlists']}
            right_column={['source']}
            getLink={(item) => `/discover/featured-playlists/${item.type}/${encodeUri(item.uri)}/${item.name}`}
            thumbnail
          />
        </section>
      );
    }
    return (
      <section className="content-wrapper">
        <Grid
          items={featured_playlists}
          getLink={(item) => `/discover/featured-playlists/${item.type}/${encodeUri(item.uri)}/${item.name}`}
        />
      </section>
    );
  }

  render = () => {
    const {
      view,
      source,
      providers,
      sortField,
      sortReverse,
      uiActions,
      loading_progress,
    } = this.props;
    const {
      filter,
      per_page,
    } = this.state;

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
        value: 'uri',
        label: i18n('fields.filters.source'),
      },
    ];

    console.debug({ source, view })

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
          value={sortField}
          valueAsLabel
          options={sort_options}
          selected_icon={sortField ? (sortReverse ? 'keyboard_arrow_up' : 'keyboard_arrow_down') : null}
          handleChange={this.onSortChange}
        />
        <DropdownField
          icon="visibility"
          name={i18n('fields.view')}
          value={view}
          valueAsLabel
          options={view_options}
          handleChange={(val) => { uiActions.set({ library_featured_playlists_view: val }); uiActions.hideContextMenu(); }}
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
          handleChange={(val) => { uiActions.set({ library_featured_playlists_source: val }); uiActions.hideContextMenu(); }}
        />
        <Button
          noHover
          discrete
          onClick={loading_progress ? this.cancelRefresh : this.refresh}
          tracking={{ category: 'FeaturedPlaylists', action: 'Refresh' }}
        >
          {loading_progress ? <Icon name="close" /> : <Icon name="refresh" /> }
          {loading_progress ? <I18n path="actions.cancel" /> : <I18n path="actions.refresh" /> }
        </Button>
      </>
    );

    return (
      <div className="view library-featured-playlists-view">
        <Header options={options} uiActions={uiActions}>
          <Icon name="album" type="material" />
          <I18n path="discover.featured_playlists.title" />
        </Header>
        {this.renderView()}
      </div>
    );
  }
}

const librarySelector = makeLibrarySelector('featured_playlists');
const processProgressSelector = makeProcessProgressSelector(processKeys);
const providersSelector = makeProvidersSelector('featured_playlists');
const mapStateToProps = (state) => {
  const [sortField, sortReverse] = getSortSelector(state, SORT_KEY, null);

  return {
    loading_progress: processProgressSelector(state),
    uri_schemes: state.mopidy.uri_schemes,
    featured_playlists: librarySelector(state, 'featured_playlists'),
    providers: providersSelector(state),
    view: state.ui.library_featured_playlists_view,
    source: getLibrarySource(state, 'featured_playlists'),
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

export default connect(mapStateToProps, mapDispatchToProps)(FeaturedPlaylists);
