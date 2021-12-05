import React, { Fragment, useEffect } from 'react';
import { connect, useSelector, useDispatch } from 'react-redux';
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
import * as coreActions from '../services/core/actions';
import * as uiActions from '../services/ui/actions';
import * as pusherActions from '../services/pusher/actions';
import * as spotifyActions from '../services/spotify/actions';
import * as mopidyActions from '../services/mopidy/actions';
import { i18n, I18n } from '../locale';
import Button from '../components/Button';
import { makeItemSelector } from '../util/selectors';
import { uriType } from '../util/helpers';

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
        <Link to="/modal/kiosk-mode" className="thumbnail__actions__item">
          <Icon name="expand" type="fontawesome" />
        </Link>
        <URILink type="album" uri={album_uri} className="thumbnail__actions__item">
          <Icon name="album" />
        </URILink>
      </Thumbnail>
    </div>
  );
};

const AddedFromItem = ({ item: { uri, type, name } }) => {
  const dispatch = useDispatch();
  const itemSelector = makeItemSelector(uri);
  const item = useSelector(itemSelector);

  useEffect(() => {
    if (uri && !item) {
      dispatch(coreActions.loadUri(uri));
    }
  }, [uri]);

  return (
    <URILink uri={uri} type={type}>{name || item?.name}</URILink>
  );
}

const AddedFrom = ({ from }) => {
  const items = from?.type === 'radio' && from?.seeds
    ? from.seeds.map((uri) => ({
      uri,
      type: uriType(uri),
    }))
    : [from];
  const { uri, type } = items[0];
  const itemSelector = makeItemSelector(uri);
  const item = useSelector(itemSelector);
  const { images } = item || {};

  return (
    <div className="current-track__added-from">
      {images && uri && (
        <URILink
          uri={uri}
          type={type}
          className="current-track__added-from__thumbnail"
        >
          <Thumbnail
            images={images}
            size="small"
            circle={items[0].type === 'artist'}
            type="artist"
          />
        </URILink>
      )}
      <div className="current-track__added-from__text">
        <I18n path="now_playing.current_track.playing_from" />
        <span>&nbsp;</span>
        {items.map((i, index) => (
          <Fragment key={i.uri}>
            {index > 0 && <span>,&nbsp;</span>}
            <AddedFromItem item={i} />
          </Fragment>
        ))}
        {from?.type === 'radio' && (
          <span className="flag flag--blue">
            <I18n path="now_playing.current_track.radio" />
          </span>
        )}
      </div>
    </div>
  );
};

const Queue = ({
  queue_tracks,
  added_from,
  current_track,
  stream_title,
  theme,
  current_track_uri,
  spotify_enabled,
  uiActions: uiActionsProp, // TODO: Remove <Header>'s dependency on passing this
  mopidyActions: {
    removeTracks,
    changeTrack,
    reorderTracklist,
    clearTracklist,
    shuffleTracklist,
  },
}) => {
  useEffect(() => uiActionsProp.setWindowTitle(i18n('now_playing.title')), []);
  const onRemoveTracks = (track_indexes) => {
    const tlids = [];
    for (let i = 0; i < track_indexes.length; i++) {
      const track = queue_tracks[track_indexes[i]];
      if (track.tlid !== undefined) {
        tlids.push(track.tlid);
      }
    }

    if (tlids.length > 0) {
      removeTracks(tlids);
    }
  }

  const onPlayTrack = (track) => changeTrack(track.tlid);
  const onPlayTracks = (tracks) => changeTrack(tracks[0].tlid);
  const onReorderTracks = (indexes, index) => reorderTracklist(indexes, index);

  let current_track_image = null;
  if (current_track && current_track_uri) {
    if (current_track.images !== undefined && current_track.images) {
      current_track_image = current_track.images.large;
    }
  }

  const options = (
    <>
      {spotify_enabled && (
        <Button noHover discrete to="/modal/radio">
          <Icon name="radio" />
          <I18n path="now_playing.context_actions.radio" />
        </Button>
      )}
      <Button noHover discrete to="/queue/history">
        <Icon name="history" />
        <I18n path="now_playing.context_actions.history" />
      </Button>
      <Button noHover discrete to="/modal/add-uri">
        <Icon name="playlist_add" />
        <I18n path="actions.add" />
      </Button>
    </>
  );

  return (
    <div className="view queue-view preserve-3d">
      <Header options={options} uiActions={uiActionsProp}>
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

            {added_from && <AddedFrom from={added_from} />}

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
            context={{
              uri: 'iris:queue',
              name: 'Queue',
              type: 'track',
              context: 'queue',
            }}
            show_source_icon
            className="queue-track-list"
            tracks={queue_tracks}
            removeTracks={onRemoveTracks}
            playTracks={onPlayTracks}
            playTrack={onPlayTrack}
            reorderTracks={onReorderTracks}
          />
        </section>
      </div>
    </div>
  );
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
    queue_tracks,
    current_track_uri: state.core.current_track_uri,
    current_track,
    added_from:
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
