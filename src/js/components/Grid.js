import React, { memo } from 'react';
import { SmartList } from './SmartList';
import { GridItem } from './GridItem';

const Grid = memo(({
  items,
  className = '',
  mini = false,
  getLink,
}) => {
  if (!items || !items.length) return null;

  return (
    <div className={`grid grid--${items[0].type}s ${className} ${mini ? 'grid--mini' : ''}`}>
      <SmartList
        items={items}
        itemComponent={GridItem}
        itemProps={{ getLink }}
      />
    </div>
  );
});

export default {
  Grid,
};

export {
  Grid,
};
