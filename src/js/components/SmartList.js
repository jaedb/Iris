import React, { memo, useState, useEffect } from 'react';
import handleViewport from 'react-in-viewport';
import { chunk } from 'lodash';
import ErrorBoundary from './ErrorBoundary';

const SmartListBatch = handleViewport(
  ({
    items,
    isFirst,
    inViewport,
    forwardedRef,
    setItemHeight,
    itemHeight,
    itemComponent: ItemComponent,
    itemProps,
    batchIndex,
    chunkSize,
    className = '',
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

    const itemIndex = (bi = 0, ii = 0) => {
      if (bi === 0) return ii;
      return ii + (chunkSize * bi);
    }

    return (
      <div className={`smart-list__batch ${className}`} ref={forwardedRef}>
        {inViewport || isFirst ? (
          <div className="smart-list__batch__inner" style={isFirst ? {} : { minHeight: itemHeight }}>
            {
              items.map((item, index) => (
                <ItemComponent
                  key={`${index}_${item.uri || item.name}`}
                  item={item}
                  getItemIndex={() => itemIndex(batchIndex, index)}
                  {...itemProps}
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

const SmartList = memo(({
  items,
  itemComponent,
  chunkSize = 20,
  initialHeight = '50vh',
  itemProps,
  className,
}) => {
  if (!items || !items.length) return null;
  if (!itemComponent) return null;

  const [itemHeight, setItemHeight] = useState(initialHeight);

  return (
    <ErrorBoundary>
      {
        chunk(items, chunkSize).map((chunked, index) => (
          <SmartListBatch
            key={`smart-list__batch-${index}`} // Yeah yeah, I know; TODO
            className={className}
            items={chunked}
            itemHeight={itemHeight}
            itemComponent={itemComponent}
            itemProps={itemProps}
            batchIndex={index}
            chunkSize={chunkSize}
            isFirst={index === 0}
            setItemHeight={setItemHeight}
          />
        ))
      }
    </ErrorBoundary>
  );
});

export default {
  SmartList,
};

export {
  SmartList,
};
