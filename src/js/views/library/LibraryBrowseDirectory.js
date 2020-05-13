
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

    this.props.uiActions.setWindowTitle('Browse');
    this.loadDirectory();
  }

  componentDidUpdate = ({
    mopidy_connected: prev_mopidy_connected,
    uri: prevUri,
  }) => {
    const {
      uri,
      mopidy_connected,
    } = this.props;

    if (!prev_mopidy_connected && mopidy_connected) this.loadDirectory();
    if (uri && uri !== prevUri) this.loadDirectory();
  }

  loadDirectory(props = this.props) {
    if (props.mopidy_connected) {
      let uri = null;
      if (props.uri !== undefined) {
        uri = props.uri;
      }
      this.props.mopidyActions.getDirectory(uri);
    }
  }

  loadMore() {
    const new_limit = this.state.limit + this.state.per_page;

    this.setState({ limit: new_limit });

    // Set our pagination to location state
    const state = (this.props.location && this.props.location.state ? this.props.location.state : {});
    state.limit = new_limit;
    this.props.history.replace({ state });
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
    const { limit } = this.state;

    let title = 'Directory';

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

    const total_items = (tracks ? tracks.length : 0) + (subdirectories ? subdirectories.length : 0);
    subdirectories = subdirectories.slice(0, this.state.limit);
    let all_tracks = null;
    let tracks = null;
    let limit_remaining = this.state.limit - subdirectories;
    if (limit_remaining > 0) {
      all_tracks = (directory.tracks && directory.tracks.length > 0 ? directory.tracks : null);
      all_tracks = sortItems(all_tracks, 'name');
      tracks = all_tracks.slice(0, limit_remaining);
    }

    const view_options = [
      {
        label: 'Thumbnails',
        value: 'thumbnails',
      },
      {
        label: 'List',
        value: 'list',
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
          <a className="button button--no-hover" onClick={(e) => { uiActions.hideContextMenu(); this.playAll(e, all_tracks); }}>
            <Icon name="play_circle_filled" />
            Play all
          </a>
        )}
        <a className="button button--no-hover" onClick={(e) => { uiActions.hideContextMenu(); this.goBack(e); }}>
          <Icon name="keyboard_backspace" />
          Back
        </a>
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
              loadMore={() => this.loadMore()}
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
    mopidy_connected: state.mopidy.connected,
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
