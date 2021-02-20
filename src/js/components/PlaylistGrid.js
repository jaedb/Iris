import React from 'react';
import { connect } from 'react-redux';
import { bindActionCreators } from 'redux';
import * as uiActions from '../services/ui/actions';
import GridItem from './GridItem';
import { encodeUri } from '../util/format';

class PlaylistGrid extends React.Component {
  handleContextMenu(e, item) {
    const { uiActions: { showContextMenu } } = this.props;

    e.preventDefault();
    showContextMenu({
      e,
      context: 'playlist',
      uris: [item.uri],
      items: [item],
      tracklist_uri: item.uri,
    });
  }

  render = () => {
    const {
      playlists,
      single_row,
      mini,
      show_source_icon,
    } = this.props;
    let {
      className = '',
    } = this.props;

    if (!playlists) return null;

    className += 'grid grid--playlists';
    if (single_row) className += ' grid--single-row';
    if (mini) className += ' grid--mini';

    return (
      <div className={className}>
        {
          playlists.map((playlist) => (
            <GridItem
              key={playlist.uri}
              type="playlist"
              item={playlist}
              show_source_icon={show_source_icon}
              onContextMenu={(e) => this.handleContextMenu(e, playlist)}
            />
          ))
        }
      </div>
    );
  }
}

const mapStateToProps = () => ({});

const mapDispatchToProps = (dispatch) => ({
  uiActions: bindActionCreators(uiActions, dispatch),
});

export default connect(mapStateToProps, mapDispatchToProps)(PlaylistGrid);
