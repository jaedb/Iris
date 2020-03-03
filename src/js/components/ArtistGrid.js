
import React from 'react';
import { connect } from 'react-redux';
import { bindActionCreators } from 'redux';
import GridItem from './GridItem';
import { collate } from '../util/format';
import * as uiActions from '../services/ui/actions';
import * as lastfmActions from '../services/lastfm/actions';
import * as spotifyActions from '../services/spotify/actions';

class ArtistGrid extends React.Component {
  handleContextMenu(e, item) {
    e.preventDefault();
    const data = {
      e,
      context: 'artist',
      uris: [item.uri],
      items: [item],
    };
    this.props.uiActions.showContextMenu(data);
  }

  render() {
    const {
      artists,
      albums,
      className: classNameProp,
      single_row,
      mini,
      show_source_icon,
      history,
      spotifyActions,
      spotifyAvailable,
      lastfmActions,
    } = this.props;

    if (artists) {
      let className = 'grid grid--artists';
      if (classNameProp) className += ` ${classNameProp}`;
      if (single_row) className += ' grid--single-row';
      if (mini) className += ' grid--mini';

      return (
        <div className={className}>
          {
						artists.map((item) => {
						  const artist = collate(item, { albums });
						  return (
                <GridItem
                  key={artist.uri}
                  type="artist"
                  item={artist}
                  show_source_icon={show_source_icon}
                  onClick={(e) => { history.push(`/artist/${encodeURIComponent(artist.uri)}`); }}
                  lastfmActions={lastfmActions}
                  spotifyActions={spotifyActions}
                  spotifyAvailable={spotifyAvailable}
                  onContextMenu={(e) => this.handleContextMenu(e, artist)}
                />
						  );
						})
					}
        </div>
      );
    }
    return null;
  }
}

const mapStateToProps = (state, ownProps) => ({
  albums: state.core.albums,
  spotifyAvailable: state.spotify.enabled,
});

const mapDispatchToProps = (dispatch) => ({
  uiActions: bindActionCreators(uiActions, dispatch),
  lastfmActions: bindActionCreators(lastfmActions, dispatch),
  spotifyActions: bindActionCreators(spotifyActions, dispatch),
});

export default connect(mapStateToProps, mapDispatchToProps)(ArtistGrid);
