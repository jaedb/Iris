
import React from 'react';
import { connect } from 'react-redux';
import { bindActionCreators } from 'redux';
import Loader from '../../components/Loader';
import Header from '../../components/Header';
import List from '../../components/List';
import TrackList from '../../components/TrackList';
import GridItem from '../../components/GridItem';
import DropdownField from '../../components/Fields/DropdownField';
import Icon from '../../components/Icon';
import URILink from '../../components/URILink';
import ErrorBoundary from '../../components/ErrorBoundary';
import LazyLoadListener from '../../components/LazyLoadListener';
import * as uiActions from '../../services/ui/actions';
import * as mopidyActions from '../../services/mopidy/actions';
import * as spotifyActions from '../../services/spotify/actions';
import {
  isLoading,
} from '../../util/helpers';
import { arrayOf, sortItems } from '../../util/arrays';
import { i18n, I18n } from '../../locale';
import Button from '../../components/Button';

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
    // Restore any limit defined in our location state
    const state = (this.props.location.state ? this.props.location.state : {});
    if (state.limit) {
      this.setState({
        limit: state.limit,
      });
    }

    this.props.uiActions.setWindowTitle(i18n('library.browse_directory.title'));
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

  playAll(e, tracks) {
    const tracks_uris = arrayOf('uri', tracks);
    this.props.mopidyActions.playURIs(tracks_uris, `iris:browse:${this.props.uri}`);
    this.props.uiActions.hideContextMenu();
  }

  goBack(e) {
    window.history.back();
    this.props.uiActions.hideContextMenu();
  }

  renderBreadcrumbs() {
    if (this.props.uri) {
      var parent_uri = this.props.uri;
    } else {
      return null;
    }

    if (parent_uri.startsWith('file://')) {
      parent_uri = parent_uri.substring(0, parent_uri.lastIndexOf('/'));

      return (
        <h4 className="breadcrumbs">
          <Icon type="fontawesome" name="angle-left" />
          <URILink type="browse" uri={parent_uri}>
            {decodeURI(parent_uri)}
          </URILink>
        </h4>
      );
    }

    return null;
  }

  renderSubdirectories(subdirectories) {
    if (this.props.view === 'list') {
      return (
        <List
          nocontext
          rows={subdirectories}
          className="library-local-directory-list"
          link_prefix="/library/browse/"
          nocontext
        />
      );
    }
    return (
      <div className="grid category-grid">
        {
          subdirectories.map((subdirectory) => (
            <GridItem
              key={subdirectory.uri}
              type="directory"
              link={`/library/browse/${encodeURIComponent(subdirectory.uri)}`}
              item={subdirectory}
              nocontext
            />
          ))
        }
      </div>
    );
  }

  render = () => {
    const {
      uri,
      directory,
      load_queue,
      uiActions,
      view,
    } = this.props;
    const {
      limit,
    } = this.state;

    let title = i18n('library.browse_directory.title');

    if (!directory || isLoading(load_queue, ['mopidy_browse'])) {
      return (
        <div className="view library-local-view">
          <Header icon="music" title={title} uiActions={uiActions} />
          <Loader body loading />
        </div>
      );
    }

    if (directory.name) {
      title = directory.name;
    } else {
      const uri_exploded = uri.split(':');
      if (uri_exploded.length > 0) {
        title = uri_exploded[0];
        title = title.charAt(0).toUpperCase() + title.slice(1);
      }
    };

    let subdirectories = (directory.subdirectories && directory.subdirectories.length > 0 ? directory.subdirectories : null);
    subdirectories = sortItems(subdirectories, 'name');

    const total_items = (directory.tracks ? directory.tracks.length : 0) + (subdirectories ? subdirectories.length : 0);
    subdirectories = subdirectories.slice(0, limit);
    let all_tracks = null;
    let tracks = null;
    const limit_remaining = limit - subdirectories;
    if (limit_remaining > 0) {
      all_tracks = (directory.tracks && directory.tracks.length > 0 ? directory.tracks : null);
      all_tracks = sortItems(all_tracks, 'name');
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
      <span>
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
            onClick={(e) => { uiActions.hideContextMenu(); this.playAll(e, all_tracks); }}
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
      </span>
    );

    return (
      <div className="view library-local-view">
        <Header options={options} uiActions={uiActions}>
          <Icon name="folder" type="material" />
          {title}
        </Header>
        <section className="content-wrapper">
          <ErrorBoundary>

            {this.renderBreadcrumbs()}

            {subdirectories ? this.renderSubdirectories(subdirectories) : null}

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
  // Decode the URI, and then re-encode selected characters
  // This is needed as Mopidy encodes *some* characters in URIs (but not other characters)
  // We need to retain ":" because this a reserved URI separator
  let uri = decodeURIComponent(ownProps.match.params.uri);
  uri = uri.replace(/\s/g, '%20');	// space
  uri = uri.replace(/&/g, '%26');		// &
  uri = uri.replace(/\[/g, '%5B');	// [
  uri = uri.replace(/\]/g, '%5D');	// ]
  uri = uri.replace(/\(/g, '%28');	// (
  uri = uri.replace(/\)/g, '%29');	// )
  uri = uri.replace(/\#/g, '%23');	// #

  return {
    uri,
    load_queue: state.ui.load_queue,
    directory: state.mopidy.directory,
    view: state.ui.library_directory_view,
  };
};

const mapDispatchToProps = (dispatch) => ({
  uiActions: bindActionCreators(uiActions, dispatch),
  mopidyActions: bindActionCreators(mopidyActions, dispatch),
  spotifyActions: bindActionCreators(spotifyActions, dispatch),
});

export default connect(mapStateToProps, mapDispatchToProps)(LibraryBrowseDirectory);
