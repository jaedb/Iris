
import React, { memo } from 'react';
import GridItem from './GridItem';

export default memo(({
  categories,
  className,
  mini,
}) => {
  if (!categories) return null;

  return (
    <div className={`grid grid--tiles ${className} ${mini ? 'grid--mini' : ''}`}>
      {
				categories.map((category) => (
          <GridItem
            key={category.uri}
            type="category"
            item={category}
            link={`/discover/categories/${encodeURIComponent(category.uri)}`}
          />
				))
			}
    </div>
  );
});
