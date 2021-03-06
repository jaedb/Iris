import React, { useEffect } from 'react';
import handleViewport from 'react-in-viewport';
import ErrorBoundary from './ErrorBoundary';
import { GridItem } from './GridItem';

const GridItemBatchIndex = ({
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
    <div className="grid__item-batch" ref={forwardedRef} style={{ minHeight: itemHeight }}>
      {inViewport || isFirst ? (
        <ErrorBoundary>
          {
            items.map((item, index) => (
              <GridItem
                key={`${index}_${item.uri || item.name}`}
                item={item}
                getLink={getLink}
              />
            ))
          }
        </ErrorBoundary>
      ) : (
        <div style={{ height: itemHeight }} />
      )}
    </div>
  );
};

const GridItemBatch = handleViewport(GridItemBatchIndex);

export {
  GridItemBatch,
};

export default {
  GridItemBatch,
};
