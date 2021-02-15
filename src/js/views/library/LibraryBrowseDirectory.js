import React from 'react';
import { connect } from 'react-redux';
import { bindActionCreators } from 'redux';
import Loader from '../../components/Loader';
import Header from '../../components/Header';
import List from '../../components/List';
import TrackList from '../../components/TrackList';
import GridItem from '../../components/GridItem';
import DropdownField from '../../components/Fields/DropdownField';
import FilterField from '../../components/Fields/FilterField';
import Icon from '../../components/Icon';
import URILink from '../../components/URILink';
import ErrorBoundary from '../../components/ErrorBoundary';
import LazyLoadListener from '../../components/LazyLoadListener';
import * as uiActions from '../../services/ui/actions';
import * as mopidyActions from '../../services/mopidy/actions';
import * as spotifyActions from '../../services/spotify/actions';
import { arrayOf, sortItems, applyFilter } from '../../util/arrays';
import { i18n, I18n } from '../../locale';
import Button from '../../components/Button';
import { encodeUri, decodeUri } from '../../util/format';
import { makeLoadingSelector } from '../../util/selectors';
import ErrorMessage from '../../components/ErrorMessage';

const Breadcrumbs = ({ uri }) => {
  let parent_uri = uri || null;
  if (!parent_uri.startsWith('file://')) return null;

  parent_uri = parent_uri.substring(0, parent_uri.lastIndexOf('/')).replace('file://', '');

  return (
    <h4>
      {decodeURI(parent_uri)}
    </h4>
  );
};

const Subdirectories = ({ items, view }) => {
  if (!items.length) return null;

  const link = (item) => `/library/browse/${encodeURIComponent(item.name)}/${encodeUri(item.uri)}`;

  if (view === 'list') {
    return (
      <List
        rows={items}
        className="library-local-directory-list"
        link={link}
        nocontext
      />
    );
  }

  return (
    <div className="grid category-grid">
      {
        items.map((item) => (
          <GridItem
            key={item.uri}
            type="directory"
            link={link(item)}
            item={item}
            nocontext
          />
        ))
      }
    </div>
  );
};

class LibraryBrowseDirectory extends React.Component {
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
      },
      uiActions: {
        setWindowTitle,
      },
    } = this.props;

    if (limit) this.setState({ limit });

    setWindowTitle(i18n('library.browse_directory.title'));
    this.loadDirectory();
  }

  componentDidUpdate = ({ uri: prevUri }) => {
    const { uri } = this.props;

    if (uri && uri !== prevUri) {
      this.loadDirectory();
    }
  }

  loadDirectory = () => {
    const {
      uri,
      mopidyActions: {
        getDirectory,
      },
    } = this.props;

    getDirectory(uri);
  }

  loadMore = () => {
    const {
      limit: prevLimit,
      per_page,
    } = this.state;
    const {
      history,
      location: {
        state: prevState = {},
      },
    } = this.props;

    const limit = prevLimit + per_page;
    this.setState({ limit });

    history.replace({
      ...prevState,
      limit,
    });
  }

  playAll = () => {
    const {
      uri,
      mopidyActions: {
        playURIs,
      },
      uiActions: {
        hideContextMenu,
      },
      directory: {
        tracks,
      } = {},
    } = this.props;

    if (!tracks || !tracks.length) return;

    playURIs(arrayOf('uri', sortItems(tracks, 'name')), `iris:browse:${uri}`);
    hideContextMenu();
  }

  goBack = () => {
    const { uiActions: { hideContextMenu } } = this.props;

    window.history.back();
    hideContextMenu();
  }

  render = () => {
    const {
      uri,
      directory,
      uiActions,
      loading,
      view,
      name,
    } = this.props;
    const {
      filter,
      per_page,
      limit,
    } = this.state;

    if (!directory || (!directory.subdirectories && !directory.tracks)) {
      if (loading) {
        return <Loader body loading />;
      }
      return (
        <ErrorMessage type="not-found" title="Not found">
          <p>
            <I18n path="errors.uri_not_found" uri={uri} />
          </p>
        </ErrorMessage>
      );
    }

    let subdirectories = (directory.subdirectories && directory.subdirectories.length > 0 ? directory.subdirectories : null);
    subdirectories = sortItems(subdirectories, 'name');
    if (filter && filter !== '') {
      subdirectories = applyFilter('name', filter, subdirectories);
    }

    const total_items = (directory.tracks ? directory.tracks.length : 0) + (subdirectories ? subdirectories.length : 0);
    subdirectories = subdirectories.slice(0, limit);
    let all_tracks = null;
    let tracks = null;
    const limit_remaining = limit - subdirectories.length;
    if (limit_remaining > 0 && directory.tracks && directory.tracks.length) {
      all_tracks = directory.tracks;
      all_tracks = sortItems(all_tracks, 'name');
      if (filter && filter !== '') {
        tracks = applyFilter('name', filter, tracks);
      }
      tracks = all_tracks.slice(0, limit_remaining);
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

    const options = (
      <>
        <FilterField
          initialValue={filter}
          handleChange={(value) => this.setState({ filter: value, limit: per_page })}
          onSubmit={() => uiActions.hideContextMenu()}
        />
        <DropdownField
          icon="visibility"
          name="View"
          value={view}
          valueAsLabel
          options={view_options}
          handleChange={(value) => { uiActions.set({ library_directory_view: value }); uiActions.hideContextMenu(); }}
        />
        {tracks && (
          <Button
            onClick={this.playAll}
            noHover
            discrete
            tracking={{ category: 'Directory', action: 'Play' }}
          >
            <Icon name="play_circle_filled" />
            <I18n path="actions.play_all" />
          </Button>
        )}
        <Button
          onClick={(e) => { uiActions.hideContextMenu(); this.goBack(e); }}
          noHover
          tracking={{ category: 'Directory', action: 'Back' }}
        >
          <Icon name="keyboard_backspace" />
          <I18n path="actions.back" />
        </Button>
      </>
    );

    return (
      <div className="view library-local-view">
        <Header options={options} uiActions={uiActions}>
          <Icon name="folder" type="material" />
          <div className="header__text">
            {name || i18n('library.browse_directory.title')}
            <Breadcrumbs uri={uri} />
          </div>
        </Header>
        <section className="content-wrapper">
          <ErrorBoundary>

            <Subdirectories items={subdirectories} view={view} />

            {tracks && (
              <TrackList
                tracks={tracks}
                uri={`iris:browse:${uri}`}
                className="library-local-track-list"
              />
            )}

            <LazyLoadListener
              loadKey={total_items > limit ? limit : total_items}
              showLoader={limit < total_items}
              loadMore={this.loadMore}
            />

          </ErrorBoundary>
        </section>
      </div>
    );
  }
}

const mapStateToProps = (state, ownProps) => {
  const {
    mopidy: {
      directory: _directory = {},
    },
    ui: {
      library_directory_view: view,
    },
  } = state;
  const uri = decodeUri(ownProps.match.params.uri);
  const uriMatcher = [uri, decodeURIComponent(uri)]; // Lenient matching due to encoding diffs
  const directory = _directory && uriMatcher.includes(_directory.uri)
    ? _directory
    : undefined;
  const loadingSelector = makeLoadingSelector(['mopidy_library.(browse|lookup)']);

  return {
    uri,
    name: decodeURIComponent(ownProps.match.params.name),
    loading: loadingSelector(state),
    directory,
    view,
  };
};

const mapDispatchToProps = (dispatch) => ({
  uiActions: bindActionCreators(uiActions, dispatch),
  mopidyActions: bindActionCreators(mopidyActions, dispatch),
  spotifyActions: bindActionCreators(spotifyActions, dispatch),
});

export default connect(mapStateToProps, mapDispatchToProps)(LibraryBrowseDirectory);
