
import React from 'react';
import { connect } from 'react-redux';
import { bindActionCreators } from 'redux';

import * as uiActions from '../services/ui/actions';
import * as mopidyActions from '../services/mopidy/actions';
import * as lastfmActions from '../services/lastfm/actions';

import GridItem from './GridItem';

class AlbumGrid extends React.Component {
  handleContextMenu(e, item) {
    e.preventDefault();
    const data = {
      e,
      context: 'album',
      uris: [item.uri],
      items: [item],
      tracklist_uri: item.uri,
    };
    this.props.uiActions.showContextMenu(data);
  }

  render() {
    if (this.props.albums) {
      let className = 'grid grid--albums';
      if (this.props.className) className += ` ${this.props.className}`;
      if (this.props.single_row) className += ' grid--single-row';
      if (this.props.mini) className += ' grid--mini';

      return (
        <div className={className}>
          {
						this.props.albums.map((album) => (
  <GridItem
    key={album.uri}
    type="album"
    item={album}
    lastfmActions={this.props.lastfmActions}
    mopidyActions={this.props.mopidyActions}
    show_source_icon={this.props.show_source_icon}
    onContextMenu={(e) => this.handleContextMenu(e, album)}
  />
						))
					}
        </div>
      );
    }
    return null;
  }
}

const mapStateToProps = (state, ownProps) => ({
  artists: state.core.artists,
});

const mapDispatchToProps = (dispatch) => ({
  uiActions: bindActionCreators(uiActions, dispatch),
  lastfmActions: bindActionCreators(lastfmActions, dispatch),
  mopidyActions: bindActionCreators(mopidyActions, dispatch),
});

export default connect(mapStateToProps, mapDispatchToProps)(AlbumGrid);
