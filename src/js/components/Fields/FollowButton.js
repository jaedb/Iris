import React from 'react';
import { connect } from 'react-redux';
import { bindActionCreators } from 'redux';
import * as uiActions from '../../services/ui/actions';
import * as spotifyActions from '../../services/spotify/actions';
import { isLoading, getFromUri } from '../../util/helpers';
import { i18n } from '../../locale';
import { Button } from '../Button';
import { makeLoadingSelector } from '../../util/selectors';

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
      loading,
    } = this.props;

    if (!uri) return null;

    if (!spotify_authorized) {
      return (
        <Button
          disabled
          working={loading}
          onClick={this.unauthorized}
          tracking={{ category: 'FollowButton', action: 'Add (disabled)' }}
        >
          {addText || i18n('actions.add_to_library')}
        </Button>
      );
    } if (is_following === true) {
      return (
        <Button
          type="destructive"
          working={loading}
          onClick={this.remove}
          tracking={{ category: 'FollowButton', action: 'Remove' }}
        >
          {removeText || i18n('actions.remove_from_library')}
        </Button>
      );
    }
    return (
      <Button
        onClick={this.add}
        working={loading}
        tracking={{ category: 'FollowButton', action: 'Add' }}
      >
        {addText || i18n('actions.add_to_library')}
      </Button>
    );
  }
}

const mapStateToProps = (state, ownProps) => {
  const {
    uri,
  } = ownProps;

  const loadingSelector = makeLoadingSelector([
    'spotify_me/tracks?',
    'spotify_me/albums?',
    'spotify_me/following?',
    `spotify_playlists/${getFromUri('playlistid', uri)}/followers?`,
  ]);

  return {
    loading: loadingSelector(state),
    spotify_authorized: state.spotify.authorization,
  };
};

const mapDispatchToProps = (dispatch) => ({
  uiActions: bindActionCreators(uiActions, dispatch),
  spotifyActions: bindActionCreators(spotifyActions, dispatch),
});

export default connect(mapStateToProps, mapDispatchToProps)(FollowButton);
