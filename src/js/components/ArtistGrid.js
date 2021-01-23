
import React from 'react';
import { connect } from 'react-redux';
import { bindActionCreators } from 'redux';
import GridItem from './GridItem';
import { collate, encodeUri } from '../util/format';
import * as uiActions from '../services/ui/actions';
import * as lastfmActions from '../services/lastfm/actions';
import * as spotifyActions from '../services/spotify/actions';

class ArtistGrid extends React.Component {
  handleContextMenu = (e, item) => {
    const { uiActions: { showContextMenu } } = this.props;
    e.preventDefault();
    showContextMenu({
      e,
      context: 'artist',
      uris: [item.uri],
      items: [item],
    });
  }

  render = () => {
    const {
      artists,
      albums,
      className,
      single_row,
      mini,
      show_source_icon,
      history,
      spotifyActions,
      spotifyAvailable,
      lastfmActions,
    } = this.props;

    if (!artists) return null;

    return (
      <div className={`grid grid--artists ${className} ${mini ? 'grid--mini' : ''}`}>
        {
          artists.map((item) => {
            const artist = collate(item, { albums });
            return (
              <GridItem
                key={artist.uri}
                type="artist"
                item={artist}
                show_source_icon={show_source_icon}
                onClick={() => { history.push(`/artist/${encodeUri(artist.uri)}`); }}
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
