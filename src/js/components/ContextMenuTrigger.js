import React, { memo } from 'react';
import Icon from './Icon';
import Button from './Button';
import { trackEvent } from './Trackable';

export default memo(({ onTrigger }) => {
  const handleClick = (e) => {
    e.preventDefault();
    e.stopPropagation();
    onTrigger(e);

    trackEvent({
      category: 'ContextMenu',
      action: 'Open',
    });
  };

  return (
    <Button
      className="context-menu-trigger mouse-contextable touch-contextable"
      onClick={handleClick}
      icon
    >
      <Icon name="more_horiz" />
    </Button>
  );
});
