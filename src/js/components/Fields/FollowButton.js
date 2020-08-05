
import React from 'react';
import { connect } from 'react-redux';
import { bindActionCreators } from 'redux';
import * as uiActions from '../../services/ui/actions';
import * as spotifyActions from '../../services/spotify/actions';
import { isLoading, getFromUri } from '../../util/helpers';
import { i18n } from '../../locale';

class FollowButton extends React.Component {
  remove = () => {
    const { spotifyActions: actions, uri } = this.props;
    actions.following(uri, 'DELETE');
  }

  add = () => {
    const { spotifyActions: actions, uri } = this.props;
    actions.following(uri, 'PUT');
  }

  unauthorized = () => {
    const { uiActions: { createNotification } } = this.props;

    createNotification({
      content: i18n('errors.authorization_required', { provider: i18n('services.spotify.title') }),
      level: 'warning',
    });
  }

  render = () => {
    const {
      uri,
      addText,
      removeText,
      spotify_authorized,
      is_following,
      load_queue,
    } = this.props;

    let { className } = this.props;

    if (!uri) return null;

    className += ' button';
    if (isLoading(load_queue, [
      'spotify_me/tracks?',
      'spotify_me/albums?',
      'spotify_me/following?',
      `spotify_playlists/${getFromUri('playlistid', uri)}/followers?`,
    ])) {
      className += ' button--working';
    }

    if (!spotify_authorized) {
      return (
        <button
          type="button"
          className={`${className} button--disabled`}
          onClick={this.unauthorized}
        >
          {addText || i18n('actions.add_to_library')}
        </button>
      );
    } if (is_following === true) {
      return (
        <button
          type="button"
          className={`${className} button--destructive`}
          onClick={this.remove}
        >
          {removeText || i18n('actions.remove_from_library')}
        </button>
      );
    }
    return (
      <button
        type="button"
        className={`${className} button--default`}
        onClick={this.add}
      >
        {addText || i18n('actions.add_to_library')}
      </button>
    );
  }
}

const mapStateToProps = (state) => ({
  load_queue: state.ui.load_queue,
  spotify_authorized: state.spotify.authorization,
});

const mapDispatchToProps = (dispatch) => ({
  uiActions: bindActionCreators(uiActions, dispatch),
  spotifyActions: bindActionCreators(spotifyActions, dispatch),
});

export default connect(mapStateToProps, mapDispatchToProps)(FollowButton);
