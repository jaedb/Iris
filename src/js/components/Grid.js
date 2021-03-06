import React, { memo, useState, useEffect } from 'react';
import handleViewport from 'react-in-viewport';
import { chunk } from 'lodash';
import ErrorBoundary from './ErrorBoundary';
import { GridItem } from './GridItem';

const GridItemBatch = handleViewport(
  ({
    items,
    isFirst,
    inViewport,
    forwardedRef,
    setItemHeight,
    itemHeight,
    getLink,
  }) => {
    // Listen for changes to our height, and pass it up to our Grid. This is then used to build the
    // placeholder elements when out of viewport. We only care about the first item because this
    // represents the same heights for everything else (in almost all circumstances).
    const { current: { clientHeight } = {} } = forwardedRef;
    useEffect(() => {
      if (isFirst && clientHeight !== itemHeight) {
        setItemHeight(clientHeight);
      }
    }, [clientHeight]);

    return (
      <div className="grid__item-batch" ref={forwardedRef}>
        {inViewport || isFirst ? (
          <div style={isFirst ? {} : { minHeight: itemHeight }}>
            {
              items.map((item, index) => (
                <GridItem
                  key={`${index}_${item.uri || item.name}`}
                  item={item}
                  getLink={getLink}
                />
              ))
            }
          </div>
        ) : (
          <div style={{ height: itemHeight }} />
        )}
      </div>
    );
  },
);

const Grid = memo(({
  items,
  className = '',
  mini = false,
  getLink,
}) => {
  if (!items || !items.length) return null;
  const [itemHeight, setItemHeight] = useState('50vh');

  return (
    <div className={`grid grid--${items[0].type}s ${className} ${mini ? 'grid--mini' : ''}`}>
      <ErrorBoundary>
        {
          chunk(items, 20).map((chunked, index) => (
            <GridItemBatch
              key={`${index}`} // Yeah yeah, I know
              items={chunked}
              getLink={getLink}
              itemHeight={itemHeight}
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
