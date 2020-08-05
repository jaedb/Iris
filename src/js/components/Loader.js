import React, { memo } from 'react';
import Icon from './Icon';
import { I18n } from '../locale';

const ProgressRing = ({ radius, stroke, progress }) => {
  const normalizedRadius = radius - stroke * 2;
  const circumference = normalizedRadius * 2 * Math.PI;
  const strokeDashoffset = circumference - progress * circumference;

  return (
    <svg
      className="loader__spinner__progress"
      height={radius * 2}
      width={radius * 2}
    >
      <circle
        className="loader__spinner__progress__circle"
        stroke="white"
        fill="transparent"
        strokeWidth={stroke}
        strokeDasharray={`${circumference} ${circumference}`}
        strokeDashoffset={strokeDashoffset}
        r={normalizedRadius}
        cx={radius}
        cy={radius}
      />
    </svg>
  );
};

export default memo((props) => {
  const {
    body,
    loading,
    finished = false,
    mini,
    lazy,
    white,
    className = '',
    progress = null,
  } = props;

  if (!loading && !finished) {
    return null;
  }

  let classNameString = 'loader';
  if (className) {
    classNameString += ` ${className}`;
  }
  if (mini) {
    classNameString += ' loader--mini';
  }
  if (body) {
    classNameString += ' loader--body';
  }
  if (lazy) {
    classNameString += ' loader--lazy';
  }
  if (white) {
    classNameString += ' loader--white';
  }

  if (!navigator.onLine) {
    return (
      <div className={classNameString}>
        <div className="loader__offline">
          <Icon name="wifi_off" />
          <p><I18n path="errors.need_to_be_online" /></p>
        </div>
      </div>
    );
  }

  if (finished) {
    classNameString += ' loader--finished';
    return (
      <div className={classNameString}>
        <div className="loader__spinner">
          <Icon name="check" />
          <div className="loader__spinner__background" />
          <div className="loader__spinner__foreground" />
        </div>
      </div>
    );
  }

  if (progress) {
    classNameString += ' loader--progress';
    return (
      <div className={classNameString}>
        <div className="loader__spinner">
          <ProgressRing
            stroke="2"
            radius={mini ? '13' : '60'}
            progress={progress}
          />
          <div className="loader__spinner__foreground" />
        </div>
      </div>
    );
  }

  return (
    <div className={classNameString}>
      <div className="loader__spinner">
        <div className="loader__spinner__background" />
        <div className="loader__spinner__foreground" />
      </div>
    </div>
  );
});
