import React, { memo } from 'react';
import FontAwesome from 'react-fontawesome';
import { sourceIcon } from '../util/helpers';

const Icon = memo(({
  name,
  type,
  className,
  onClick,
}) => {
  if (!name || name === '') return null;

  const fullClassName = `icon icon--${type || 'material'} ${className || ''}`;

  switch (type) {
    case 'svg':
      return (
        <span className={fullClassName}>
          <img
            src={`/iris/assets/icons/${name}.svg`}
            onClick={onClick}
            alt={`${name} icon`}
          />
        </span>
      );

    case 'gif':
      return (
        <span className={fullClassName}>
          <img
            className={fullClassName}
            src={`/iris/assets/icons/${name}.gif`}
            onClick={onClick}
            alt={`${name} icon`}
          />
        </span>
      );

    case 'fontawesome':
      return (
        <FontAwesome
          className={fullClassName}
          type="fontawesome"
          name={name}
          onClick={onClick}
        />
      );

    case 'css':
      if (name === 'playing') {
        return (
          <i className={`${fullClassName} icon--playing`}>
            <span />
            <span />
            <span />
          </i>
        );
      }
      break;

    default:
      return <i className={fullClassName} onClick={onClick}>{name}</i>;
  }
});

const SourceIcon = ({
  uri,
  ...rest
}) => {
  const source = sourceIcon(uri);

  switch (source) {
    case 'genius':
    case 'tidal':
      return <Icon name={source} type="svg" className="source" {...rest} />;
    default:
      return <Icon name={source} type="fontawesome" className="source" {...rest} />;
  }
};

export default Icon;

export {
  Icon,
  SourceIcon,
};
