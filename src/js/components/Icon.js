
import React, { memo } from 'react';
import FontAwesome from 'react-fontawesome';

export default memo((props) => {
  if (!props.name || props.name === '') {
    return null;
  }

  let className = `icon icon--${props.type ? props.type : 'material'}`;
  if (props.className) {
    className += ` ${props.className}`;
  }

  switch (props.type) {
    case 'svg':
      return <img className={className} src={`/iris/assets/icons/${props.name}.svg`} onClick={(e) => (props.onClick ? props.onClick(e) : null)} />;

    case 'gif':
      return <img className={className} src={`/iris/assets/icons/${props.name}.gif`} onClick={(e) => (props.onClick ? props.onClick(e) : null)} />;

    case 'fontawesome':
      return <FontAwesome className={className} type="fontawesome" name={props.name} onClick={(e) => (props.onClick ? props.onClick(e) : null)} />;

    case 'css':
      switch (props.name) {
        case 'playing':
          return (
            <i className={`${className} icon--playing`}>
              <span />
              <span />
              <span />
            </i>
          );
      }

    default:
      return <i className={className} onClick={(e) => (props.onClick ? props.onClick(e) : null)}>{props.name}</i>;
  }
});
