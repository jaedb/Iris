import React from 'react';
import { connect } from 'react-redux';
import { bindActionCreators } from 'redux';
import * as uiActions from '../../services/ui/actions';
import * as spotifyActions from '../../services/spotify/actions';
import { getFromUri } from '../../util/helpers';
import { i18n } from '../../locale';
import { Button } from '../Button';
import { makeLoadingSelector } from '../../util/selectors';

const FollowButton = ({
  spotifyActions: {
    following,
  },
  uiActions: {
    createNotification,
  },
  uri,
  addText,
  removeText,
  spotify_authorized,
  is_following,
  loading,
}) => {
  const remove = () => following(uri, 'DELETE');
  const add = () => following(uri, 'PUT');
  const unauthorized = () => createNotification({
    content: i18n('errors.authorization_required', { provider: i18n('services.spotify.title') }),
    level: 'warning',
  });

  if (!uri) return null;

  if (!spotify_authorized) {
    return (
      <Button
        disabled
        working={loading}
        onClick={unauthorized}
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
        onClick={remove}
        tracking={{ category: 'FollowButton', action: 'Remove' }}
      >
        {removeText || i18n('actions.remove_from_library')}
      </Button>
    );
  }
  return (
    <Button
      onClick={add}
      working={loading}
      tracking={{ category: 'FollowButton', action: 'Add' }}
    >
      {addText || i18n('actions.add_to_library')}
    </Button>
  );
}

const mapStateToProps = (state) => {
  const loadingSelector = makeLoadingSelector(['(.*)(follow)|(me\/albums)(.*)']);

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
