import React, { memo } from 'react';
import { FixedSizeGrid } from 'react-window';
import AutoSizer from 'react-virtualized-auto-sizer';
import GridItem from './GridItem';
import { getGridItem } from '../util/helpers';

const Grid = memo(({ items }) => {
  if (!items) return null;
  return (
    <AutoSizer>
      {({ height, width }) => {
        const columnCount = 5;
        const columnWidth = (width / columnCount) - (20 / columnCount); // 20px for scrollbars
        const rowHeight = columnWidth * 1.2;

        const itemKey = ({ columnIndex, data, rowIndex }) => {
          const item = getGridItem({ rowIndex, columnIndex, columnCount, data }) || {};
          return `${item.uri}-${columnIndex}-${rowIndex}`;
        };

        return (
          <FixedSizeGrid
            columnCount={columnCount}
            columnWidth={columnWidth}
            height={height}
            rowCount={items.length / columnCount}
            rowHeight={rowHeight}
            width={width}
            itemData={items}
            itemKey={itemKey}
          >
            {GridItem}
          </FixedSizeGrid>
        );
      }}
    </AutoSizer>
  );
});

export default {
  Grid,
};

export {
  Grid,
};
