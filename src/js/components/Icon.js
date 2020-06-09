
import React, { memo } from 'react';
import FontAwesome from 'react-fontawesome';

export default memo(({ name, type, className, onClick }) => {
  if (!name || name === '') return null;

  const fullClassName = `icon icon--${type || 'material'} ${className || ''}`;

  switch (type) {
    case 'svg':
      return <img className={fullClassName} src={`/iris/assets/icons/${name}.svg`} onClick={onClick} />;

    case 'gif':
      return <img className={fullClassName} src={`/iris/assets/icons/${name}.gif`} onClick={onClick} />;

    case 'fontawesome':
      return <FontAwesome className={fullClassName} type="fontawesome" name={name} onClick={onClick} />;

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
