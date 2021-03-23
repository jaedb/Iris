import React from 'react';
import { throttle } from '../../util/helpers';

export default class VolumeControl extends React.Component {
  constructor(props) {
    super(props);

    this.handleChange = throttle(this.handleChange.bind(this), 100);
  }

  handleChange(value) {
    this.props.onVolumeChange(value, this.props.volume);
  }

  render() {
    const {
      className,
      mute,
      volume,
      vertical,
    } = this.props;

    let sliderClassName = 'slider slider--volume';
    if (mute) sliderClassName += ' slider--muted';

    return (
      <div className={`slider__wrapper slider__wrapper--${vertical ? 'vertical' : 'horiztonal'} ${className}`}>
        <div className={sliderClassName}>
          <input
            className="slider__input"
            type="range"
            min="0"
            max="25"
            orient={vertical ? 'vertical' : 'horizontal'}
            value={volume / 4}
            onChange={(e) => this.handleChange(parseInt(e.target.value) * 4)}
          />
          <div className="slider__track">
            <div className="slider__track__progress" style={vertical ? { height: `${volume}%` } : { width: `${volume}%` }} />
          </div>
        </div>
      </div>
    );
  }
}
