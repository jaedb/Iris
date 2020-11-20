import React, { memo } from 'react';
import Icon from './Icon';
import { I18n } from '../locale';

const LoaderRing = ({ radius, stroke, progress }) => {
  const normalizedRadius = radius - stroke * 2;
  const circumference = normalizedRadius * 2 * Math.PI;
  const strokeDashoffset = circumference - progress * circumference;

  return (
    <svg
      className="loader__ring"
      height={radius * 2}
      width={radius * 2}
    >
      {progress === null ? (
        <circle
          className="loader__ring__background"
          stroke="transparent"
          fill="transparent"
          strokeWidth={stroke}
          r={normalizedRadius}
          cx={radius}
          cy={radius}
        />
      ) : (
        <circle
          className="loader__ring__progress"
          stroke="transparent"
          fill="transparent"
          strokeWidth={stroke}
          strokeDasharray={`${circumference} ${circumference}`}
          strokeDashoffset={strokeDashoffset || 0}
          r={normalizedRadius}
          cx={radius}
          cy={radius}
        />
      )}
      <circle
        className="loader__ring__foreground"
        stroke="transparent"
        fill="transparent"
        strokeWidth={stroke}
        strokeDasharray={`${circumference} ${circumference}`}
        strokeDashoffset={(circumference - 0.25 * circumference) || 0}
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
    finished,
    mini,
    lazy,
    white,
    className = '',
    progress,
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

  classNameString += ' loader--progress';
  return (
    <div className={classNameString}>
      <LoaderRing
        stroke={mini ? 2 : 3}
        radius={mini ? 12 : 60}
        progress={progress}
      />
    </div>
  );
});
