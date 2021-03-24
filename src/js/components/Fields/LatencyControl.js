import React from 'react';
import { throttle } from '../../util/helpers';

export default class LatencyControl extends React.Component {
  constructor(props) {
    super(props);
    this.handleChange = throttle(this.handleChange.bind(this), 100);
  }

  handleChange(value) {
    this.props.onChange(value, this.props.value);
  }

  render() {
    // Zero, or positive value
    if (this.props.value >= 0) {
      var { value } = this.props;
      if (value > this.props.max) {
        value = this.props.max;
      }
      var percentage = Math.round((value / this.props.max) * 100 / 2);
      var left = 50;
      var width = percentage;
      var negative = false;

      if (width > (this.props.max / 2)) width = this.props.max / 2;

      // Negative value
      // We reverse it to a positive for easier maths and style rules
    } else {
      var value = -this.props.value;
      if (value < -this.props.max) {
        value = -this.props.max;
      }
      var percentage = Math.round((value / this.props.max) * 100 / 2);
      var left = 50 - percentage;
      var width = percentage;
      var negative = true;

      if (left < 0) left = 0;
      if (width > (this.props.max / 2)) width = this.props.max / 2;
    }

    return (
      <span className="latency-control">
        <div className="slider__wrapper">
          <div className="slider slider--latency">
            <input
              type="range"
              min={-(this.props.max)}
              max={this.props.max}
              className="slider__input"
              value={this.props.value}
              onChange={(e) => this.handleChange(parseInt(e.target.value))}
            />
            <div className="zero" />
            <div className="slider__track">
              <div className={`slider__track__progress slider__track__progress--${negative ? 'negative' : 'positive'}`} style={{ width: `${width}%`, left: `${left}%` }} />
            </div>
          </div>
        </div>
      </span>
    );
  }
}
