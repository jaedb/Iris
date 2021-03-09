import React from 'react';
import { SmartList } from './SmartList';
import { ListItem } from './ListItem'

const List = ({
  items,
  className = '',
  ...rest
}) => {
  if (!items || !items.length) return null;

  return (
    <SmartList
      className={`list ${className}`}
      items={items}
      itemComponent={ListItem}
      itemProps={rest}
      chunkSize={10}
    />
  );
};

export default {
  List,
};

export {
  List,
};
