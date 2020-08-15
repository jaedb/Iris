import React from 'react';
import Link from './Link';
import { trackEvent } from './Trackable';

const Button = ({
  className,
  primary,
  type,
  size,
  icon,
  pullRight,
  colour,
  confirming,
  timingOut,
  destructiveHover,
  discrete,
  working,
  disabled,
  noHover,
  trackingLabel,
  onClick: onClickProp,
  to,
  ...rest
}) => {
  const classNames = [];
  if (type) {
    classNames.push(type);
  } else {
    classNames.push('default');
  }
  if (size) classNames.push(size);
  if (colour) classNames.push(colour);
  if (icon) classNames.push('icon');
  if (pullRight) classNames.push('pull-right');
  if (confirming) classNames.push('confirming');
  if (timingOut) classNames.push('timing-out');
  if (destructiveHover) classNames.push('destructive-hover');
  if (discrete) classNames.push('discrete');
  if (working) classNames.push('working');
  if (disabled) classNames.push('disabled');
  if (noHover) classNames.push('no-hover');

  const Element = to ? Link : 'button';

  const onClick = (e) => {
    if (onClickProp) onClickProp(e);

    if (trackingLabel) {
      trackEvent({
        category: 'Button',
        action: 'Click',
        label: trackingLabel,
      });
    }
  };

  return (
    <Element
      type="button"
      className={`button ${className} ${classNames.map((s) => ` button--${s}`).join(' ')}`}
      disabled={disabled}
      to={to}
      onClick={onClick}
      {...rest}
    />
  );
};

export default Button;

export {
  Button,
};
