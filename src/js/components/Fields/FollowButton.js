import React from 'react';
import { createNotification } from '../../services/ui/actions';
import { following } from '../../services/spotify/actions';
import { i18n } from '../../locale';
import { Button } from '../Button';
import { makeLoadingSelector } from '../../util/selectors';
import { useSelector } from 'react-redux';

const loadingSelector = makeLoadingSelector(['(.*)(follow)|(me\/albums)(.*)']);

const FollowButton = ({
  uri,
  addText,
  removeText,
  is_following,
}) => {
  const loading = useSelector(loadingSelector);
  const spotify_authorized = useSelector((state) => state.spotify.authorization);
  const remove = () => dispatch(following(uri, 'DELETE'));
  const add = () => dispatch(following(uri, 'PUT'));
  const unauthorized = () => dispatch(
    createNotification({
      content: i18n('errors.authorization_required', { provider: i18n('services.spotify.title') }),
      level: 'warning',
    }),
  );

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

export default FollowButton;
