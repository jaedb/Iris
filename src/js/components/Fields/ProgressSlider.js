import React from 'react';
import { useSelector, useDispatch } from 'react-redux';
import { setTimePosition } from '../../services/mopidy/actions';
import { throttle } from '../../util/helpers';

const ProgressSlider = ({
  size,
}) => {
  const dispatch = useDispatch();
  const connected = useSelector((state) => state.mopidy.connected);
  const time_position = useSelector((state) => state.mopidy.time_position);
  const play_state = useSelector((state) => state.mopidy.play_state);
  const current_track = useSelector((state) => state.core.current_track);

  const onChangeThrottled = (e) => {
    const value = parseInt(e.target.value, 10);
    dispatch(setTimePosition(current_track?.duration * (value / 100)));
  };
  const onChange = throttle(onChangeThrottled, 250);

  let percent = 0;
  if (connected && time_position && current_track && current_track.duration) {
    percent = time_position / current_track.duration;
    percent *= 100;
    if (percent > 1000) {
      percent = 100;
    }
  }

  return (
    <div className={`slider slider--playback-progress slider--${play_state}`}>
      <input
        type="range"
        min="0"
        max="100"
        value={percent}
        className="slider__input"
        onChange={onChange}
      />
      <div className={`slider__track slider__track--${size}`}>
        <div
          className={`slider__track__progress slider__track__progress--${size}`}
          style={{ width: `${percent}%` }}
        />
      </div>
    </div>
  );
}

export default ProgressSlider;
