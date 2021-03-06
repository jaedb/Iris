import React, { useState, useEffect } from 'react';
import { chunk } from 'lodash';
import handleViewport from 'react-in-viewport';
import { ListItem } from './ListItem';
import ErrorBoundary from './ErrorBoundary';

const ListItemBatch = handleViewport(
  ({
    items,
    isFirst,
    inViewport,
    forwardedRef,
    setItemHeight,
    itemHeight,
    middle_column,
    right_column,
    details,
    thumbnail,
    nocontext,
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
      <div className="list__item-batch" ref={forwardedRef}>
        {inViewport || isFirst ? (
          <div style={isFirst ? {} : { minHeight: itemHeight }}>
            {
              items.map((item, index) => (
                <ListItem
                  key={`${index}_${item.uri || item.name}`}
                  item={item}
                  middle_column={middle_column}
                  right_column={right_column}
                  details={details}
                  thumbnail={thumbnail}
                  nocontext={nocontext}
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

const List = ({
  items,
  className,
  middle_column,
  right_column,
  details,
  thumbnail,
  nocontext,
  getLink,
}) => {
  if (!items || !items.length) return null;
  const [itemHeight, setItemHeight] = useState('300px');

  return (
    <div className={`list ${className}`}>
      <ErrorBoundary>
        {
          chunk(items, 20).map((chunked, index) => (
            <ListItemBatch
              key={`${index}`}
              items={chunked}
              itemHeight={itemHeight}
              isFirst={index === 0}
              setItemHeight={setItemHeight}
              middle_column={middle_column}
              right_column={right_column}
              details={details}
              thumbnail={thumbnail}
              nocontext={nocontext}
              getLink={getLink}
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
