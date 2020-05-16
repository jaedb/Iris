
import React from 'react';
import { connect } from 'react-redux';
import { bindActionCreators } from 'redux';
import { withRouter } from 'react-router';

import Dropzone from './Dropzone';

import * as uiActions from '../../services/ui/actions';
import * as mopidyActions from '../../services/mopidy/actions';
import * as spotifyActions from '../../services/spotify/actions';
import { arrayOf } from '../../util/arrays';
import { decodeMopidyUri } from '../../util/helpers';

class Dropzones extends React.Component {
  constructor(props) {
    super(props);

    this._zones = [
      {
        title: 'Add to queue',
        icon: 'play_arrow',
        action: 'enqueue',
      },
      {
        title: 'Play next',
        icon: 'play_arrow',
        action: 'enqueue_next',
      },
      {
        title: 'Add to playlist',
        icon: 'playlist_add',
        action: 'add_to_playlist',
        accepts: ['tltrack', 'track', 'album', 'playlist', 'artist'],
      },
    ];
  }

  handleMouseMove = (e) => {
    const { dragger = {}, uiActions: { dragMove } } = this.props;
    if (!dragger || !dragger.active) return null;
    return dragMove(e);
  }

  handleMouseUp = (zone) => {
    const {
      dragger: {
        victims = [],
        from_uri,
      },
      mopidyActions: {
        enqueueURIs,
      },
      history,
    } = this.props;
    const uris = arrayOf('uri', victims);

    switch (zone.action) {
      case 'enqueue':
        enqueueURIs(uris, from_uri);
        break;
      case 'enqueue_next':
        enqueueURIs(uris, from_uri, true);
        break;
      case 'add_to_playlist':
        history.push(`/add-to-playlist/${encodeURIComponent(uris.join(','))}`);
        // uris
        break;
      default:
        break;
    }
  }

  render = () => {
    const {
      dragger = {},
    } = this.props;
    if (!dragger || !dragger.active) return null;

    return (
      <div className="dropzones">
        {
          this._zones.map((zone) => (
            <Dropzone
              key={zone.action}
              data={zone}
              handleMouseUp={() => this.handleMouseUp(zone)}
            />
          ))
        }
      </div>
    );
  }
}

const mapStateToProps = (state, ownProps) => ({
  dragger: state.ui.dragger,
});

const mapDispatchToProps = (dispatch) => ({
  uiActions: bindActionCreators(uiActions, dispatch),
  spotifyActions: bindActionCreators(spotifyActions, dispatch),
  mopidyActions: bindActionCreators(mopidyActions, dispatch),
});

export default withRouter(connect(mapStateToProps, mapDispatchToProps)(Dropzones));
