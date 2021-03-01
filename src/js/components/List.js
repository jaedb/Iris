import React, { useState } from 'react';
import { ListItem } from './ListItem';
import ErrorBoundary from './ErrorBoundary';

const List = ({
  items,
  className,
  ...rest
}) => {
  if (!items || !items.length) return null;
  const [itemHeight, setItemHeight] = useState(0);

  return (
    <div className={`list ${className}`}>
      <ErrorBoundary>
        {
          items.map((item, index) => (
            <ListItem
              key={`${index}_${item.uri || item.name}`}
              item={item}
              itemHeight={itemHeight || '300px'}
              isFirst={index === 0}
              setItemHeight={setItemHeight}
              {...rest}
            />
          ))
        }
      </ErrorBoundary>
    </div>
  );
};

export default {
  List,
}

export {
  List,
};
