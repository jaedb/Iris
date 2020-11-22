
import React from 'react';
import { connect } from 'react-redux';
import { bindActionCreators } from 'redux';
import * as uiActions from '../../services/ui/actions';
import * as lastfmActions from '../../services/lastfm/actions';
import { i18n } from '../../locale';
import Button from '../Button';
import { makeLoadingSelector } from '../../util/selectors';

const FollowButton = ({
  uri,
  artist,
  track,
  addText,
  removeText,
  is_loved,
  lastfm_authorized,
  uiActions: {
    createNotification,
  },
  lastfmActions: {
    unloveTrack,
    loveTrack,
  },
  loading,
}) => {
  const onRemove = () => unloveTrack(uri, artist, track);
  const onAdd = () => loveTrack(uri, artist, track);
  const onDisabledClick = () => {
    createNotification({
      content: i18n('errors.authorization_required', { provider: i18n('services.lastfm.title') }),
      level: 'warning',
    });
  };

  if (!uri) return false;

  if (!lastfm_authorized) {
    return (
      <Button
        disabled
        onClick={onDisabledClick}
        working={loading}
        tracking={{ category: 'Lastfm', action: 'Love', label: 'Disabled' }}
      >
        {addText || i18n('services.lastfm.love')}
      </Button>
    );
  } if (is_loved && is_loved !== '0') {
    return (
      <Button
        type="destructive"
        onClick={onRemove}
        working={loading}
        tracking={{ category: 'Lastfm', action: 'Unlove' }}
      >
        {removeText || i18n('services.lastfm.unlove')}
      </Button>
    );
  }
  return (
    <Button
      onClick={onAdd}
      working={loading}
      tracking={{ category: 'Lastfm', action: 'Love' }}
    >
      {addText || i18n('services.lastfm.love')}
    </Button>
  );
};

const mapStateToProps = (state) => {
  const loadingSelector = makeLoadingSelector(['love', 'unlove']);
  return {
    loading: loadingSelector(state),
    lastfm_authorized: state.lastfm.authorization,
  };
};

const mapDispatchToProps = (dispatch) => ({
  uiActions: bindActionCreators(uiActions, dispatch),
  lastfmActions: bindActionCreators(lastfmActions, dispatch),
});

export default connect(mapStateToProps, mapDispatchToProps)(FollowButton);
