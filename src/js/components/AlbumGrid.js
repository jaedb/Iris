
import React from 'react';
import { connect } from 'react-redux';
import { bindActionCreators } from 'redux';

import * as uiActions from '../services/ui/actions';
import * as mopidyActions from '../services/mopidy/actions';
import * as lastfmActions from '../services/lastfm/actions';

import GridItem from './GridItem';

class AlbumGrid extends React.Component {
  handleContextMenu = (e, item) => {
    const { uiActions: { showContextMenu } } = this.props;
    e.preventDefault();
    showContextMenu({
      e,
      context: 'album',
      uris: [item.uri],
      items: [item],
      tracklist_uri: item.uri,
    });
  }

  render = () => {
    const {
      albums,
      className,
      mini,
      lastfmActions,
      mopidyActions,
      show_source_icon,
    } = this.props;

    if (!albums) return null;

    return (
      <div className={`grid grid--albums ${className} ${mini ? ' grid--mini' : ''}`}>
        {
          albums.map((album) => (
            <GridItem
              key={album.uri}
              type="album"
              item={album}
              lastfmActions={lastfmActions}
              mopidyActions={mopidyActions}
              show_source_icon={show_source_icon}
              onContextMenu={(e) => this.handleContextMenu(e, album)}
            />
          ))
        }
      </div>
    );
  }
}

const mapStateToProps = (state) => ({
  artists: state.core.artists,
});

const mapDispatchToProps = (dispatch) => ({
  uiActions: bindActionCreators(uiActions, dispatch),
  lastfmActions: bindActionCreators(lastfmActions, dispatch),
  mopidyActions: bindActionCreators(mopidyActions, dispatch),
});

export default connect(mapStateToProps, mapDispatchToProps)(AlbumGrid);
