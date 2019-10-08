
import React, { memo } from 'react';
import Icon from './Icon';

export default memo((props) => {
  const handleClick = (e) => {
    e.preventDefault();
    e.stopPropagation();
    props.onTrigger(e);
  };

  let className = 'context-menu-trigger mouse-contextable touch-contextable';
  if (props.className) {
    className += ` ${props.className}`;
  }

  return (
    <span className={className} onClick={(e) => handleClick(e)}>
      <Icon name="more_horiz" />
    </span>
  );
});
