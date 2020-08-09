import React, { memo } from 'react';
import Icon from './Icon';

export default memo(({
  className = '',
  onTrigger,
}) => {
  const handleClick = (e) => {
    e.preventDefault();
    e.stopPropagation();
    onTrigger(e);
  };

  return (
    <button
      className={`button button--icon button--default context-menu-trigger mouse-contextable touch-contextable ${className}`}
      onClick={handleClick}
      type="button"
    >
      <Icon name="more_horiz" />
    </button>
  );
});
