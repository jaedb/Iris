import React from 'react';
import { connect } from 'react-redux';
import { bindActionCreators } from 'redux';
import { withRouter } from 'react-router';

import Dropzone from './Dropzone';

import * as uiActions from '../../services/ui/actions';
import * as mopidyActions from '../../services/mopidy/actions';
import * as spotifyActions from '../../services/spotify/actions';
import { arrayOf } from '../../util/arrays';
import { i18n } from '../../locale';
import { encodeUri } from '../../util/format';

class Dropzones extends React.Component {
  constructor(props) {
    super(props);

    this._zones = [
      {
        title: i18n('actions.add_to_queue'),
        icon: 'play_arrow',
        action: 'enqueue',
      },
      {
        title: i18n('actions.play_next'),
        icon: 'play_arrow',
        action: 'enqueue_next',
      },
      {
        title: i18n('actions.add_to_playlist'),
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
        history.push(`/add-to-playlist/${encodeUri(uris.join(','))}`);
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

const mapStateToProps = (state) => ({
  dragger: state.ui.dragger,
});

const mapDispatchToProps = (dispatch) => ({
  uiActions: bindActionCreators(uiActions, dispatch),
  spotifyActions: bindActionCreators(spotifyActions, dispatch),
  mopidyActions: bindActionCreators(mopidyActions, dispatch),
});

export default withRouter(connect(mapStateToProps, mapDispatchToProps)(Dropzones));
