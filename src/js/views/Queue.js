import React from "react";
import { connect } from "react-redux";
import { bindActionCreators } from "redux";

import Link from "../components/Link";
import Icon from "../components/Icon";
import Parallax from "../components/Parallax";
import TrackList from "../components/TrackList";
import Dater from "../components/Dater";
import ArtistSentence from "../components/ArtistSentence";
import Thumbnail from "../components/Thumbnail";
import Header from "../components/Header";
import URILink from "../components/URILink";
import LazyLoadListener from "../components/LazyLoadListener";

import * as helpers from "../helpers";
import * as coreActions from "../services/core/actions";
import * as uiActions from "../services/ui/actions";
import * as pusherActions from "../services/pusher/actions";
import * as spotifyActions from "../services/spotify/actions";
import * as mopidyActions from "../services/mopidy/actions";

class Queue extends React.Component {
  constructor(props) {
    super(props);

    this.state = {
      limit: 50,
      per_page: 50
    };
  }

  componentWillMount() {
    // Before we mount, restore any limit defined in our location state
    var state = this.props.location.state ? this.props.location.state : {};
    if (state.limit) {
      this.setState({
        limit: state.limit
      });
    }
  }

  componentDidMount() {
    this.props.uiActions.setWindowTitle("Now playing");
  }

  shouldComponentUpdate(nextProps) {
    return nextProps !== this.props;
  }

  componentWillReceiveProps(nextProps) {
    const { added_from_uri } = nextProps;

    if (added_from_uri && this.props.added_from_uri !== added_from_uri) {
      const item_type = helpers.uriType(added_from_uri);
      switch (item_type) {
        case "album":
          this.props.coreActions.loadAlbum(added_from_uri);
          break;
        case "artist":
          this.props.coreActions.loadArtist(added_from_uri);
          break;
        case "playlist":
          this.props.coreActions.loadPlaylist(added_from_uri);
          break;
      }
    }
  }

  loadMore() {
    var new_limit = this.state.limit + this.state.per_page;

    this.setState({ limit: new_limit });

    // Set our pagination to location state
    var state =
      this.props.location && this.props.location.state
        ? this.props.location.state
        : {};
    state.limit = new_limit;
    this.props.history.replace({ state: state });
  }

  removeTracks(track_indexes) {
    var tlids = [];
    for (var i = 0; i < track_indexes.length; i++) {
      var track = this.props.queue_tracks[track_indexes[i]];
      if (track.tlid !== undefined) {
        tlids.push(track.tlid);
      }
    }

    if (tlids.length > 0) {
      this.props.mopidyActions.removeTracks(tlids);
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

  renderQueueStats() {
    var total_time = 0;

    return (
      <div className="queue-stats mid_grey-text">
        <span>{this.props.current_tracklist.length} tracks</span>
        &nbsp;&nbsp;|&nbsp;&nbsp;
        {this.props.current_tracklist.length > 0 ? (
          <Dater type="total-time" data={this.props.current_tracklist} />
        ) : (
          <span>0 mins</span>
        )}
      </div>
    );
  }

  renderArtwork(image) {
    if (!image) {
      return (
        <div
          className={`current-track__artwork ${
            this.props.radio_enabled
              ? "current-track__artwork--radio-enabled"
              : ""
          }`}
        >
          {this.props.radio_enabled ? (
            <img
              className="radio-overlay"
              src="/iris/assets/radio-overlay.png"
            />
          ) : null}
          <Thumbnail glow circle={this.props.radio_enabled} />
        </div>
      );
    }

    var uri = null;
    if (this.props.current_track.album && this.props.current_track.album.uri) {
      uri = this.props.current_track.album.uri;
    }
    return (
      <div
        className={`current-track__artwork ${
          this.props.radio_enabled
            ? "current-track__artwork--radio-enabled"
            : ""
        }`}
      >
        <URILink type="album" uri={uri}>
          {this.props.radio_enabled ? (
            <img
              className="radio-overlay"
              src="/iris/assets/radio-overlay.png"
            />
          ) : null}
          <Thumbnail glow image={image} circle={this.props.radio_enabled} />
        </URILink>
      </div>
    );
  }

  renderAddedFrom() {
    const { added_from_uri } = this.props;
    if (!added_from_uri) return null;

    const item_type = helpers.uriType(added_from_uri);
    const item_library = this.props[item_type + "s"];
    if (!item_library) return null;

    const item = item_library[added_from_uri];
    if (!item) return null;

    return (
      <div className="current-track__added-from">
        <URILink
          type={item_type}
          uri={item.uri}
          className="current-track__added-from__thumbnail"
        >
          <Thumbnail
            images={item.images}
            size="small"
            circle={item_type == "artist"}
          />
        </URILink>
        <div className="current-track__added-from__text">
          Playing from{" "}
          <URILink type={item_type} uri={item.uri}>
            {item.name}
          </URILink>
        </div>
      </div>
    );
  }

  render() {
    const { current_track, queue_tracks } = this.props;
    const total_queue_tracks = queue_tracks.length;
    const tracks = queue_tracks.slice(0, this.state.limit);

    var current_track_image = null;
    if (current_track && this.props.current_track_uri) {
      if (current_track.images !== undefined && current_track.images) {
        current_track_image = current_track.images.large;
      }
    }

    var options = (
      <span>
        {this.props.spotify_enabled ? (
          <Link className="button button--no-hover" to={"/queue/radio"}>
            <Icon name="radio" />
            Radio
            {this.props.radio && this.props.radio.enabled ? (
              <span className="flag blue">On</span>
            ) : null}
          </Link>
        ) : null}
        <Link className="button button--no-hover" to={"/queue/history"}>
          <Icon name="history" />
          History
        </Link>
        <a
          className="button button--no-hover"
          onClick={e => {
            this.props.mopidyActions.clearTracklist();
            this.props.uiActions.hideContextMenu();
          }}
        >
          <Icon name="delete_sweep" />
          Clear
        </a>
        <Link className="button button--no-hover" to={"/queue/add-uri"}>
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
        <Parallax blur image={current_track_image} />
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
                <ArtistSentence
                  className="current-track__artists"
                  artists={current_track.artists}
                />
              ) : (
                <ArtistSentence className="current-track__artists" />
              )}

              {this.renderAddedFrom()}
            </div>
          </div>

          <section className="list-wrapper">
            <TrackList
              show_source_icon
              track_context="queue"
              className="queue-track-list"
              tracks={tracks}
              removeTracks={track_indexes => this.removeTracks(track_indexes)}
              playTracks={tracks => this.playTracks(tracks)}
              playTrack={track => this.playTrack(track)}
              reorderTracks={(indexes, index) =>
                this.reorderTracks(indexes, index)
              }
            />
          </section>

          <LazyLoadListener
            loadKey={
              total_queue_tracks > this.state.limit
                ? this.state.limit
                : total_queue_tracks
            }
            showLoader={this.state.limit < total_queue_tracks}
            loadMore={() => this.loadMore()}
          />
        </div>
      </div>
    );
  }
}

const mapStateToProps = (state, ownProps) => {
  let current_track = state.core.current_track;
  let queue_tracks = [];

  if (state.core.queue && state.core.tracks) {
    for (let queue_track of state.core.queue) {
      let track = Object.assign({}, queue_track);

      // If we have the track in our index, merge it in.
      // We prioritise queue track over index track as queue has unique data, like which track
      // is playing and tlids.
      if (state.core.tracks.hasOwnProperty(track.uri)) {
        track = Object.assign({}, state.core.tracks[track.uri], track, {
          playing: current_track && current_track.tlid == track.tlid
        });
      }

      // Now merge in our queue metadata
      if (state.core.queue_metadata["tlid_" + track.tlid] !== undefined) {
        track = Object.assign(
          {},
          track,
          state.core.queue_metadata["tlid_" + track.tlid]
        );
      }

      // Siphon off this track if it's a full representation of our current track (by tlid)
      if (current_track && current_track.uri == track.uri) {
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
    radio_enabled: state.core.radio && state.core.radio.enabled ? true : false,
    artists: state.core.artists,
    albums: state.core.albums,
    playlists: state.core.playlists,
    queue_tracks: queue_tracks,
    current_track_uri: state.core.current_track_uri,
    current_track: current_track,
    added_from_uri:
      current_track && current_track.added_from
        ? current_track.added_from
        : null
  };
};

const mapDispatchToProps = dispatch => {
  return {
    coreActions: bindActionCreators(coreActions, dispatch),
    uiActions: bindActionCreators(uiActions, dispatch),
    pusherActions: bindActionCreators(pusherActions, dispatch),
    spotifyActions: bindActionCreators(spotifyActions, dispatch),
    mopidyActions: bindActionCreators(mopidyActions, dispatch)
  };
};

export default connect(
  mapStateToProps,
  mapDispatchToProps
)(Queue);
