
import React from 'react';
import { connect } from 'react-redux';
import { bindActionCreators } from 'redux';
import * as uiActions from '../../services/ui/actions';
import * as lastfmActions from '../../services/lastfm/actions';
import { i18n } from '../../locale';

const FollowButton = ({
  uri,
  artist,
  track,
  addText,
  removeText,
  is_loved,
  className: classNameProp = '',
  lastfm_authorized,
  uiActions: {
    createNotification,
  },
  lastfmActions: {
    unloveTrack,
    loveTrack,
  },
}) => {
  const onRemove = () => unloveTrack(uri, artist, track);
  const onAdd = () => loveTrack(uri, artist, track);
  const onDisabledClick = () => {
    createNotification({
      content: i18n('errors.authorization_required', { provider: i18n('services.lastfm.title') }),
      level: 'warning',
    });
  };

  if (!uri) {
    return false;
  }

  const className = `button ${classNameProp}`;

  if (!lastfm_authorized) {
    return (
      <button
        className={`${className} button--disabled`}
        onClick={onDisabledClick}
        type="button"
      >
        {addText || i18n('services.lastfm.love')}
      </button>
    );
  } if (is_loved && is_loved !== '0') {
    return (
      <button
        className={`${className} button--destructive`}
        onClick={onRemove}
        type="button"
      >
        {removeText || i18n('services.lastfm.unlove')}
      </button>
    );
  }
  return (
    <button
      className={`${className} button--default`}
      onClick={onAdd}
      type="button"
    >
      {addText || i18n('services.lastfm.love')}
    </button>
  );
};

const mapStateToProps = (state) => ({
  load_queue: state.ui.load_queue,
  lastfm_authorized: state.lastfm.authorization,
});

const mapDispatchToProps = (dispatch) => ({
  uiActions: bindActionCreators(uiActions, dispatch),
  lastfmActions: bindActionCreators(lastfmActions, dispatch),
});

export default connect(mapStateToProps, mapDispatchToProps)(FollowButton);
