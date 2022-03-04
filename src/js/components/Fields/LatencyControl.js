import React from 'react';
import TextField from './TextField';
import { throttle } from '../../util/helpers';

const LatencyControl = ({
  onChange,
  value: valueProp,
  max,
}) => {
  let value = valueProp;
  const throttledChange = (nextValue) => onChange(nextValue, value);
  const handleChange = throttle(throttledChange, 100);
  let percentage = Math.round((value / max) * 100 / 2);
  let left = 50;
  let width = percentage;
  let negative = false;

  // Zero, or positive value
  if (value >= 0) {
    if (value > max) {
      value = max;
    }
    if (width > (max / 2)) width = max / 2;

    // Negative value
    // We reverse it to a positive for easier maths and style rules
  } else {
    value = -value;
    if (value < -max) {
      value = -max;
    }
    percentage = Math.round((value / max) * 100 / 2);
    left = 50 - percentage;
    width = percentage;
    negative = true;

    if (left < 0) left = 0;
    if (width > (max / 2)) width = max / 2;
  }

  return (
    <span className="latency-control">
      <div className="slider__wrapper">
        <div className="slider slider--latency">
          <input
            type="range"
            min={-(max)}
            max={max}
            className="slider__input"
            value={value}
            onChange={(e) => handleChange(parseInt(e.target.value, 10))}
          />
          <div className="zero" />
          <div className="slider__track">
            <div className={`slider__track__progress slider__track__progress--${negative ? 'negative' : 'positive'}`} style={{ width: `${width}%`, left: `${left}%` }} />
          </div>
        </div>
      </div>
      <TextField
        className="tiny"
        type="number"
        onChange={handleChange}
        value={String(value)}
        autosave
      />
    </span>
  );
}

export default LatencyControl;
