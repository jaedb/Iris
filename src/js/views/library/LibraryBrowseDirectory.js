import React from 'react';
import { connect } from 'react-redux';
import { bindActionCreators } from 'redux';
import Loader from '../../components/Loader';
import Header from '../../components/Header';
import TrackList from '../../components/TrackList';
import { Grid } from '../../components/Grid';
import { List } from '../../components/List';
import DropdownField from '../../components/Fields/DropdownField';
import FilterField from '../../components/Fields/FilterField';
import Icon from '../../components/Icon';
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
        items={items}
        className="library-local-directory-list"
        getLink={link}
        nocontext
      />
    );
  }

  return (
    <Grid
      items={items}
      getLink={link}
      nocontext
    />
  );
};

class LibraryBrowseDirectory extends React.Component {
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
    const { filter } = this.state;

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

    let subdirectories = directory?.subdirectories;
    let tracks = directory?.tracks;
    subdirectories = sortItems(subdirectories, 'name');
    tracks = sortItems(tracks, 'name');
    if (filter && filter !== '') {
      subdirectories = applyFilter('name', filter, subdirectories);
      tracks = applyFilter('name', filter, tracks);
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
          handleChange={(value) => this.setState({ filter: value })}
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

            <TrackList
              tracks={tracks}
              uri={`iris:browse:${uri}`}
              className="library-local-track-list"
            />

          </ErrorBoundary>
        </section>
      </div>
    );
  }
}

const loadingSelector = makeLoadingSelector(['mopidy_library.(browse|lookup)']);
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
