import React from 'react';
import Link from './Link';
import URILink from './URILink';
import { trackEvent } from './Trackable';

const Button = ({
  className = '',
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
  submit,
  tracking,
  href,
  onClick: onClickProp,
  to,
  uri,
  uriType,
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

  let Element = 'button';
  if (to) Element = Link;
  if (uri) Element = URILink;
  if (href) Element = 'a';

  const onClick = (e) => {
    if (onClickProp) onClickProp(e);

    if (tracking) {
      trackEvent({
        category: 'Button',
        action: 'Click',
        ...tracking,
      });
    }
  };

  return (
    <Element
      type={uri ? uriType : (submit ? 'submit' : 'button')}
      className={`button ${className} ${classNames.map((s) => ` button--${s}`).join(' ')}`}
      disabled={disabled}
      to={to}
      href={href}
      uri={uri}
      onClick={onClick}
      {...rest}
    />
  );
};

export default Button;

export {
  Button,
};
