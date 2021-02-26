import React from 'react';
import { connect } from 'react-redux';
import { bindActionCreators } from 'redux';
import { FixedSizeGrid as Grid } from 'react-window';
import AutoSizer from 'react-virtualized-auto-sizer';
import * as uiActions from '../services/ui/actions';
import GridItem from './GridItem';

class PlaylistGrid extends React.Component {
  handleContextMenu(e, item) {
    const { uiActions: { showContextMenu } } = this.props;

    e.preventDefault();
    showContextMenu({
      e,
      context: 'playlist',
      uris: [item.uri],
      items: [item],
      tracklist_uri: item.uri,
    });
  }

  render = () => {
    const {
      playlists,
      single_row,
      mini,
      show_source_icon,
    } = this.props;
    let {
      className = '',
    } = this.props;

    if (!playlists) return null;

    className += 'grid grid--playlists';
    if (single_row) className += ' grid--single-row';
    if (mini) className += ' grid--mini';

    return (
      <AutoSizer>
        {({ height, width }) => {
          const columnCount = 5;
          const columnWidth = (width / columnCount) - (20 / columnCount); // 20px for scrollbars
          const rowHeight = columnWidth * 1.2;

          return (
            <Grid
              columnCount={columnCount}
              columnWidth={columnWidth}
              height={height}
              rowCount={playlists.length / columnCount}
              rowHeight={rowHeight}
              width={width}
              itemData={playlists}
            >
              {GridItem}
            </Grid>
          );
        }}
      </AutoSizer>
    );
  }
}

const mapStateToProps = () => ({});

const mapDispatchToProps = (dispatch) => ({
  uiActions: bindActionCreators(uiActions, dispatch),
});

export default connect(mapStateToProps, mapDispatchToProps)(PlaylistGrid);
