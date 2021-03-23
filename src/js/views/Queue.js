import React from 'react';
import { connect } from 'react-redux';
import { bindActionCreators } from 'redux';
import Link from '../components/Link';
import Icon from '../components/Icon';
import Parallax from '../components/Parallax';
import TrackList from '../components/TrackList';
import { Dater } from '../components/Dater';
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
import { i18n, I18n } from '../locale';
import Button from '../components/Button';
import { indexToArray } from '../util/arrays';

const Artwork = ({
  image,
  album_uri,
}) => {
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
        <Link to="/kiosk-mode" className="thumbnail__actions__item">
          <Icon name="expand" type="fontawesome" />
        </Link>
        <URILink type="album" uri={album_uri} className="thumbnail__actions__item">
          <Icon name="album" />
        </URILink>
      </Thumbnail>
    </div>
  );
};

class Queue extends React.Component {
  componentDidMount() {
    const {
      uiActions: {
        setWindowTitle,
      },
      location: {
        limit,
      } = {},
    } = this.props;

    if (limit) this.setState({ limit });
    setWindowTitle(i18n('now_playing.title'));
  }

  shouldComponentUpdate(nextProps) {
    return nextProps !== this.props;
  }

  componentDidUpdate = ({
    added_from_uri: prev_added_from_uri,
  }) => {
    const {
      coreActions: {
        loadUri,
      },
      added_from_uri,
    } = this.props;

    if (added_from_uri && added_from_uri !== prev_added_from_uri) {
      loadUri(added_from_uri);
    }
  }

  removeTracks = (track_indexes) => {
    const {
      queue_tracks,
      mopidyActions: {
        removeTracks: doRemoveTracks,
      },
    } = this.props;
    const tlids = [];
    for (let i = 0; i < track_indexes.length; i++) {
      const track = queue_tracks[track_indexes[i]];
      if (track.tlid !== undefined) {
        tlids.push(track.tlid);
      }
    }

    if (tlids.length > 0) {
      doRemoveTracks(tlids);
    }
  }

  playTrack = (track) => {
    const { mopidyActions: { changeTrack } } = this.props;
    changeTrack(track.tlid);
  }

  playTracks = (tracks) => {
    const { mopidyActions: { changeTrack } } = this.props;
    changeTrack(tracks[0].tlid);
  }

  reorderTracks = (indexes, index) => {
    const { mopidyActions: { reorderTracklist } } = this.props;
    reorderTracklist(indexes, index);
  }

  renderAddedFrom = () => {
    const {
      items,
      added_from_uri,
    } = this.props;
    if (!added_from_uri) return null;

    const uri_type = uriType(added_from_uri);
    let addedFromItems = [];

    // Radio nests it's seed URIs in an encoded URI format
    switch (uri_type) {
      case 'radio':
        addedFromItems = indexToArray(items, getFromUri('seeds', added_from_uri));
        break;
      case 'search':
        addedFromItems = [{
          uri: added_from_uri,
          name: `"${getFromUri('searchterm', added_from_uri)}" search`,
        }];
        break;
      default:
        addedFromItems = indexToArray(items, [added_from_uri]);
        break;
    }

    if (!addedFromItems.length) return null;

    return (
      <div className="current-track__added-from">
        {addedFromItems[0].images && addedFromItems[0].uri && (
          <URILink
            uri={addedFromItems[0].uri}
            type={addedFromItems[0].type}
            className="current-track__added-from__thumbnail"
          >
            <Thumbnail
              images={addedFromItems[0].images}
              size="small"
              circle={uriType(addedFromItems[0].uri) === 'artist'}
              type="artist"
            />
          </URILink>
        )}
        <div className="current-track__added-from__text">
          {'Playing from '}
          <LinksSentence
            items={addedFromItems}
            type={addedFromItems[0].type}
          />
          {uri_type === 'radio' && (
            <span className="flag flag--blue">
              {i18n('now_playing.current_track.radio')}
            </span>
          )}
        </div>
      </div>
    );
  }

  render = () => {
    const {
      current_track,
      queue_tracks,
      stream_title,
      theme,
      current_track_uri,
      spotify_enabled,
      uiActions,
      mopidyActions: {
        clearTracklist,
        shuffleTracklist,
      },
    } = this.props;

    let current_track_image = null;
    if (current_track && current_track_uri) {
      if (current_track.images !== undefined && current_track.images) {
        current_track_image = current_track.images.large;
      }
    }

    const options = (
      <>
        {spotify_enabled && (
          <Button noHover discrete to="/queue/radio">
            <Icon name="radio" />
            <I18n path="now_playing.context_actions.radio" />
          </Button>
        )}
        <Button noHover discrete to="/queue/history">
          <Icon name="history" />
          <I18n path="now_playing.context_actions.history" />
        </Button>
        <Button noHover discrete to="/queue/add-uri">
          <Icon name="playlist_add" />
          <I18n path="actions.add" />
        </Button>
      </>
    );

    return (
      <div className="view queue-view preserve-3d">
        <Header options={options} uiActions={uiActions}>
          <Icon name="play_arrow" type="material" />
          <I18n path="now_playing.title" />
        </Header>
        {theme === 'dark' && <Parallax image={current_track_image} blur />}
        <div className="content-wrapper">
          <div className="current-track">
            <Artwork
              image={current_track_image}
              album_uri={current_track && current_track.album && current_track.album.uri}
            />
            <div className="current-track__details">
              <div className="current-track__title">
                {stream_title && (
                  <span>{stream_title}</span>
                )}
                {!stream_title && current_track && (
                  <URILink type="track" uri={current_track.uri}>
                    {current_track.name}
                  </URILink>
                )}
                {!stream_title && !current_track && (<span>-</span>)}
              </div>

              <LinksSentence
                className="current-track__artists"
                items={current_track ? current_track.artists : null}
              />

              {this.renderAddedFrom()}

              <div className="current-track__queue-details">
                <ul className="details">
                  <li>{`${queue_tracks.length} tracks`}</li>
                  <li><Dater type="total-time" data={queue_tracks} /></li>
                  {queue_tracks.length > 0 && (
                    <li>
                      <a onClick={shuffleTracklist}>
                        <Icon name="shuffle" />
                        <I18n path="now_playing.current_track.shuffle" />
                      </a>
                    </li>
                  )}
                  {queue_tracks.length > 0 && (
                    <li>
                      <a onClick={clearTracklist}>
                        <Icon name="delete_sweep" />
                        <I18n path="now_playing.current_track.clear" />
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
              tracks={queue_tracks}
              removeTracks={this.removeTracks}
              playTracks={this.playTracks}
              playTrack={this.playTrack}
              reorderTracks={this.reorderTracks}
            />
          </section>
        </div>
      </div>
    );
  }
}

const mapStateToProps = (state) => {
  const {
    current_track: core_current_track,
    stream_title,
    items,
    queue,
    queue_metadata,
  } = state.core;
  const queue_tracks = [];
  let current_track = {};

  if (queue && items) {
    queue.forEach((queueTrack) => {
      let track = {
        ...queueTrack,
        playing: core_current_track && core_current_track.tlid === queueTrack.tlid,
      };

      // If we have the track in our index, merge it in.
      // We prioritise queue track over index track as queue has unique data, like which track
      // is playing and tlids.
      if (items[track.uri]) {
        track = {
          ...items[track.uri],
          ...track,
        };
      }

      // Now merge in our queue metadata
      if (queue_metadata[`tlid_${track.tlid}`] !== undefined) {
        track = {
          ...track,
          ...queue_metadata[`tlid_${track.tlid}`],
        };
      }

      // Siphon off this track if it's a full representation of our current track
      if (track.playing) {
        current_track = track;
      }

      // Now add our compiled track for our tracklist
      queue_tracks.push(track);
    });
  }

  return {
    theme: state.ui.theme,
    spotify_enabled: state.spotify.enabled,
    radio: state.core.radio,
    radio_enabled: !!(state.core.radio && state.core.radio.enabled),
    items,
    queue_tracks,
    current_track_uri: state.core.current_track_uri,
    current_track,
    added_from_uri:
      current_track && current_track.added_from
        ? current_track.added_from
        : null,
    stream_title,
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
