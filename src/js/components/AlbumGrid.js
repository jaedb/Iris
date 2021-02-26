import React from 'react';
import { connect } from 'react-redux';
import { bindActionCreators } from 'redux';

import * as uiActions from '../services/ui/actions';
import * as mopidyActions from '../services/mopidy/actions';
import * as lastfmActions from '../services/lastfm/actions';

import { Grid } from '.';

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
    return <Grid items={albums} />;
  }
}

const mapStateToProps = () => ({});

const mapDispatchToProps = (dispatch) => ({
  uiActions: bindActionCreators(uiActions, dispatch),
  lastfmActions: bindActionCreators(lastfmActions, dispatch),
  mopidyActions: bindActionCreators(mopidyActions, dispatch),
});

export default connect(mapStateToProps, mapDispatchToProps)(AlbumGrid);
