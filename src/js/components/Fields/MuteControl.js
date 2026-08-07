import React, { memo } from 'react';
import Icon from '../Icon';
import { I18n } from '../../locale';

export default memo((props) => {
  if (props.mute) {
    return (
      <button className={`control mute-control ${props.noTooltip ? '' : 'tooltip '}${props.className ? props.className : ''}`} onClick={() => props.onMuteChange(false)}>
        <Icon className="red-text" name="volume_off" />
        {props.noTooltip ? null : <span className="tooltip__content"><I18n path="playback_controls.unmute" /></span>}
      </button>
    );
  }
  return (
    <button className={`control mute-control ${props.noTooltip ? '' : 'tooltip '}${props.className ? props.className : ''}`} onClick={() => props.onMuteChange(true)}>
      <Icon className="muted" name="volume_mute" />
      {props.noTooltip ? null : <span className="tooltip__content"><I18n path="playback_controls.mute" /></span>}
    </button>
  );
});
