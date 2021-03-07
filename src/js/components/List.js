import React from 'react';
import { SmartList } from './SmartList';
import { ListItem } from './ListItem'

const List = ({
  items,
  className,
  ...rest
}) => {
  if (!items || !items.length) return null;

  return (
    <div className={`list ${className}`}>
      <SmartList
        items={items}
        itemComponent={ListItem}
        itemProps={rest}
      />
    </div>
  );
};

export default {
  List,
};

export {
  List,
};
