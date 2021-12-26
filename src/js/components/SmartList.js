import React, { memo, useState, useEffect } from 'react';
import handleViewport from 'react-in-viewport';
import { chunk } from 'lodash';
import ErrorBoundary from './ErrorBoundary';
import { ReactSortable, Sortable } from 'react-sortablejs';
import { MultiDrag } from "sortablejs";

// mount whatever plugins you'd like to. These are the only current options.
Sortable.mount(new MultiDrag());

const SmartListBatch = handleViewport(
  ({
    items,
    inViewport,
    forwardedRef,
    setItemHeight,
    itemHeight,
    itemComponent: ItemComponent,
    itemProps,
    batchIndex,
    chunkSize,
    className = '',
    isFirst,
    isLast,
    sortable,
    onSort,
  }) => {
    const [list, setList] = useState(items);
    const onChange = (nextList) => {
      setList(nextList);
    }
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
    };

    const handleSort = ({ newIndex, oldIndex, oldIndicies }) => {
      if (oldIndicies?.length > 0) {
        return onSort(oldIndicies.map(({ index }) => index), newIndex);
      }
      return onSort([oldIndex], newIndex);
    }

    const Container = sortable ? ReactSortable : 'div';

    return (
      <div className={`smart-list__batch ${className}`} ref={forwardedRef}>
        {inViewport || isFirst || isLast ? (
          <Container
            list={list}
            setList={onChange}
            onSort={handleSort}
            className="smart-list__batch__inner"
            style={isFirst || isLast ? {} : { minHeight: itemHeight }}
            selectedClass="list__item--selected"
            multiDragKey="CTRL"
            multiDrag
          >
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
          </Container>
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
  sortable,
  onSort,
}) => {
  if (!items || !items.length) return null;
  if (!itemComponent) return null;

  const [itemHeight, setItemHeight] = useState(initialHeight);
  const chunks = chunk(items, chunkSize);

  return (
    <ErrorBoundary>
      {
        chunks.map((chunked, index) => (
          <SmartListBatch
            key={`smart-list__batch-${index}`} // Yeah yeah, I know; TODO
            className={className}
            items={chunked}
            itemHeight={itemHeight}
            itemComponent={itemComponent}
            itemProps={itemProps}
            batchIndex={index}
            chunkSize={chunkSize}
            setItemHeight={setItemHeight}
            isFirst={index === 0}
            isLast={index === chunks.length - 1}
            sortable={sortable}
            onSort={onSort}
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
