import React, { memo, useState } from 'react';
import { GridItem } from './GridItem';
import ErrorBoundary from './ErrorBoundary';

const Grid = memo(({
  items,
  className = '',
  mini = false,
  getLink,
}) => {
  if (!items || !items.length) return null;
  const [itemHeight, setItemHeight] = useState(0);

  return (
    <div className={`grid grid--${items[0].type}s ${className} ${mini ? 'grid--mini' : ''}`}>
      <ErrorBoundary>
        {
          items.map((item, index) => (
            <GridItem
              key={`${index}_${item.uri || item.name}`}
              item={item}
              getLink={getLink}
              itemHeight={itemHeight || '300px'}
              isFirst={index === 0}
              setItemHeight={setItemHeight}
            />
          ))
        }
      </ErrorBoundary>
    </div>
  );
});

export default {
  Grid,
};

export {
  Grid,
};
