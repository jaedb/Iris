import React from 'react';
import { connect } from 'react-redux';
import { bindActionCreators } from 'redux';
import Link from '../components/Link';
import Icon from '../components/Icon';
import Parallax from '../components/Parallax';
import TrackList from '../components/TrackList';
import Dater from '../components/Dater';
import LinksSentence from '../components/LinksSentence';
import Thumbnail from '../components/Thumbnail';
import Header from '../components/Header';
import URILink from '../components/URILink';
import LazyLoadListener from '../components/LazyLoadListener';
import * as coreActions from '../services/core/actions';
import * as uiActions from '../services/ui/actions';
import * as pusherActions from '../services/pusher/actions';
import * as spotifyActions from '../services/spotify/actions';
import * as mopidyActions from '../services/mopidy/actions';
import {
  getFromUri,
  uriType,
} from '../util/helpers';

class Queue extends React.Component {
  constructor(props) {
    super(props);

    this.state = {
      limit: 50,
      per_page: 50,
    };
  }

  componentDidMount() {
    // Restore any limit defined in our location state
    const state = this.props.location.state ? this.props.location.state : {};
    if (state.limit) {
      this.setState({
        limit: state.limit,
      });
    }

    this.props.uiActions.setWindowTitle('Now playing');
  }

  shouldComponentUpdate(nextProps) {
    return nextProps !== this.props;
  }

  componentDidUpdate = ({
    added_from_uri: prev_added_from_uri,
  }) => {
    const {
      coreActions: {
        loadAlbum,
        loadArtist,
        loadPlaylist,
      },
      added_from_uri,
    } = this.props;

    if (added_from_uri && added_from_uri !== prev_added_from_uri) {
      const item_type = uriType(added_from_uri);
      switch (item_type) {
        case 'album':
          loadAlbum(added_from_uri);
          break;
        case 'artist':
          loadArtist(added_from_uri);
          break;
        case 'playlist':
          loadPlaylist(added_from_uri);
          break;
        default:
          break;
      }
    }
  }

  loadMore() {
    const { limit, per_page } = this.state;
    const { location, history } = this.props;
    const new_limit = limit + per_page;

    this.setState({ limit: new_limit });

    // Set our pagination to location state
    const state = location && location.state
      ? location.state
      : {};
    state.limit = new_limit;
    history.replace({ state });
  }

  removeTracks(track_indexes) {
    const { queue_tracks, mopidyActions } = this.props;
    const tlids = [];
    for (let i = 0; i < track_indexes.length; i++) {
      const track = queue_tracks[track_indexes[i]];
      if (track.tlid !== undefined) {
        tlids.push(track.tlid);
      }
    }

    if (tlids.length > 0) {
      mopidyActions.removeTracks(tlids);
    }
  }

  playTrack(track) {
    this.props.mopidyActions.changeTrack(track.tlid);
  }

  playTracks(tracks) {
    this.props.mopidyActions.changeTrack(tracks[0].tlid);
  }

  reorderTracks(indexes, index) {
    this.props.mopidyActions.reorderTracklist(indexes, index);
  }

  renderArtwork(image) {
    const { current_track } = this.props;
    const uri = (current_track && current_track.album && current_track.album.uri) ? current_track.album.uri : null;

    if (!image) {
      return (
        <div className="current-track__artwork">
          <Thumbnail glow type="track" />
        </div>
      );
    }

    return (
      <div className="current-track__artwork">
        <Thumbnail glow image={image} type="track">
          <URILink uri={uri} className="thumbnail__actions__item">
            <Icon name="art_track" />
          </URILink>
          <Link to="/kiosk-mode" className="thumbnail__actions__item">
            <Icon name="expand" type="fontawesome" />
          </Link>
        </Thumbnail>
      </div>
    );
  }

  renderAddedFrom() {
    const { added_from_uri } = this.props;
    if (!added_from_uri) return null;

    const uri_type = uriType(added_from_uri);
    const items = [];

    // Radio nests it's seed URIs in an encoded URI format
    switch (uri_type){
      case 'radio':
        const radio_seeds = getFromUri('seeds', added_from_uri);

        for (let seed of radio_seeds) {
          let item_type = uriType(seed);
          let item_library = this.props[`${item_type}s`];
          if (item_library && item_library[seed]) {
            items.push(item_library[seed]);
          }
        }
        break;

      case 'search':
        items.push({
          uri: added_from_uri,
          name: `"${getFromUri('searchterm', added_from_uri)}" search`,
        })
        break;

      default:
        const item_library = this.props[`${uri_type}s`];
        if (item_library && item_library[added_from_uri]) {
          items.push(item_library[added_from_uri]);
        }
        break;
    }

    if (items.length <= 0) return null;

    return (
      <div className="current-track__added-from">
        {items[0].images && (
          <URILink
            uri={items[0].uri}
            className="current-track__added-from__thumbnail"
          >
            <Thumbnail
              images={items[0].images}
              size="small"
              circle={uriType(items[0].uri) === 'artist'}
              type="artist"
            />
          </URILink>
        )}
        <div className="current-track__added-from__text">
          {'Playing from '}
          <LinksSentence items={items} />
          {uri_type === 'radio' && <span className="flag flag--blue">Radio</span>}
        </div>
      </div>
    );
  }

  render() {
    const { current_track, queue_tracks, theme } = this.props;
    const { limit } = this.state;
    const total_queue_tracks = queue_tracks.length;
    const tracks = queue_tracks.slice(0, limit);

    let current_track_image = null;
    if (current_track && this.props.current_track_uri) {
      if (current_track.images !== undefined && current_track.images) {
        current_track_image = current_track.images.large;
      }
    }

    const options = (
      <span>
        {this.props.spotify_enabled && (
          <Link className="button button--no-hover" to="/queue/radio">
            <Icon name="radio" />
            Radio
          </Link>
        )}
        <Link className="button button--no-hover" to="/queue/history">
          <Icon name="history" />
          History
        </Link>
        <Link className="button button--no-hover" to="/queue/add-uri">
          <Icon name="playlist_add" />
          Add URI
        </Link>
      </span>
    );

    return (
      <div className="view queue-view preserve-3d">
        <Header options={options} uiActions={this.props.uiActions}>
          <Icon name="play_arrow" type="material" />
          Now playing
        </Header>
        {theme === 'dark' && <Parallax image={current_track_image} blur />}
        <div className="content-wrapper">
          <div className="current-track">
            {this.renderArtwork(current_track_image)}
            <div className="current-track__details">
              <div className="current-track__title">
                {current_track ? (
                  <URILink type="track" uri={current_track.uri}>
                    {current_track.name}
                  </URILink>
                ) : (
                  <span>-</span>
                )}
              </div>

              {current_track ? (
                <LinksSentence
                  className="current-track__artists"
                  items={current_track.artists}
                />
              ) : (
                <LinksSentence className="current-track__artists" />
              )}

              {this.renderAddedFrom()}

              <div className="current-track__queue-details">
                <ul className="details">
                  <li>{`${queue_tracks.length} tracks`}</li>
                  <li><Dater type="total-time" data={queue_tracks} /></li>
                  {queue_tracks.length > 0 && (
                    <li>
                      <a onClick={this.props.mopidyActions.shuffleTracklist}>
                        <Icon name="shuffle" />
                        Shuffle
                      </a>
                    </li>
                  )}
                  {queue_tracks.length > 0 && (
                    <li>
                      <a onClick={this.props.mopidyActions.clearTracklist}>
                        <Icon name="delete_sweep" />
                        Clear
                      </a>
                    </li>
                  )}
                </ul>
              </div>
            </div>
          </div>

          <section className="list-wrapper">
            <TrackList
              uri="iris:queue"
              show_source_icon
              track_context="queue"
              className="queue-track-list"
              tracks={tracks}
              removeTracks={(track_indexes) => this.removeTracks(track_indexes)}
              playTracks={(tracks) => this.playTracks(tracks)}
              playTrack={(track) => this.playTrack(track)}
              reorderTracks={(indexes, index) => this.reorderTracks(indexes, index)}
            />
          </section>

          <LazyLoadListener
            loadKey={
              total_queue_tracks > limit
                ? limit
                : total_queue_tracks
            }
            showLoader={limit < total_queue_tracks}
            loadMore={() => this.loadMore()}
          />
        </div>
      </div>
    );
  }
}

const mapStateToProps = (state, ownProps) => {
  let { current_track } = state.core;
  const queue_tracks = [];

  if (state.core.queue && state.core.tracks) {
    for (const queue_track of state.core.queue) {
      let track = {
        ...queue_track,
        playing: current_track && current_track.tlid == queue_track.tlid,
      };

      // If we have the track in our index, merge it in.
      // We prioritise queue track over index track as queue has unique data, like which track
      // is playing and tlids.
      if (state.core.tracks.hasOwnProperty(track.uri)) {
        track = {
          ...state.core.tracks[track.uri],
          ...track,
        };
      }

      // Now merge in our queue metadata
      if (state.core.queue_metadata[`tlid_${track.tlid}`] !== undefined) {
        track = {
          ...track,
          ...state.core.queue_metadata[`tlid_${track.tlid}`],
        };
      }

      // Siphon off this track if it's a full representation of our current track
      if (track.playing) {
        current_track = track;
      }

      // Now add our compiled track for our tracklist
      queue_tracks.push(track);
    }
  }

  return {
    theme: state.ui.theme,
    spotify_enabled: state.spotify.enabled,
    radio: state.core.radio,
    radio_enabled: !!(state.core.radio && state.core.radio.enabled),
    artists: state.core.artists,
    albums: state.core.albums,
    playlists: state.core.playlists,
    tracks: state.core.tracks,
    queue_tracks,
    current_track_uri: state.core.current_track_uri,
    current_track,
    added_from_uri:
      current_track && current_track.added_from
        ? current_track.added_from
        : null,
  };
};

const mapDispatchToProps = (dispatch) => ({
  coreActions: bindActionCreators(coreActions, dispatch),
  uiActions: bindActionCreators(uiActions, dispatch),
  pusherActions: bindActionCreators(pusherActions, dispatch),
  spotifyActions: bindActionCreators(spotifyActions, dispatch),
  mopidyActions: bindActionCreators(mopidyActions, dispatch),
});

export default connect(
  mapStateToProps,
  mapDispatchToProps,
)(Queue);
