
import React from 'react';
import { connect } from 'react-redux';
import { bindActionCreators } from 'redux';
import Link from '../components/Link';
import ErrorMessage from '../components/ErrorMessage';
import Header from '../components/Header';
import Thumbnail from '../components/Thumbnail';
import LinksSentence from '../components/LinksSentence';
import LastfmLoveButton from '../components/Fields/LastfmLoveButton';
import Dater from '../components/Dater';
import SelectField from '../components/Fields/SelectField';
import ContextMenuTrigger from '../components/ContextMenuTrigger';
import Icon from '../components/Icon';
import Loader from '../components/Loader';
import * as coreActions from '../services/core/actions';
import * as uiActions from '../services/ui/actions';
import * as mopidyActions from '../services/mopidy/actions';
import * as spotifyActions from '../services/spotify/actions';
import * as lastfmActions from '../services/lastfm/actions';
import * as geniusActions from '../services/genius/actions';
import {
  isLoading,
  getFromUri,
  sourceIcon,
  uriSource,
  uriType,
} from '../util/helpers';

class Track extends React.Component {
  componentDidMount() {
    this.props.coreActions.loadTrack(this.props.uri);

    if (this.props.track) {
      this.setWindowTitle(this.props.track);

      if (this.props.genius_authorized && this.props.track.artists && !this.props.track.lyrics_results) {
        this.props.geniusActions.findTrackLyrics(this.props.track);
      }
    }
  }

  handleContextMenu(e) {
    e.preventDefault();
    const data = { uris: [this.props.uri] };
    this.props.uiActions.showContextMenu(e, data, 'track', 'click');
  }

  componentDidUpdate = ({
    uri: prevUri,
    track: prevTrack,
    mopidy_connected: prev_mopidy_connected,
  }) => {
    const {
      uri,
      track,
      genius_authorized,
      lastfm_authorized,
      mopidy_connected,
      coreActions: {
        loadTrack,
      },
      geniusActions: {
        findTrackLyrics,
      },
      lastfmActions: {
        getTrack,
      },
    } = this.props;
    // if our URI has changed, fetch new track
    if (prevUri !== uri) {
      loadTrack(uri);
      if (genius_authorized && track.artists) findTrackLyrics(track);

      // if mopidy has just connected AND we're not a Spotify track, go get
    } else if (!prev_mopidy_connected && mopidy_connected) {
      if (uriSource(uri) !== 'spotify') loadTrack(uri);
    }

    // We have just received our full track or our track artists
    if ((!prevTrack && track) || (!prevTrack.artists && track.artists)) {
      this.setWindowTitle(track);
      if (lastfm_authorized) getTrack(track.uri);
      if (genius_authorized && !track.lyrics_results) findTrackLyrics(track);
    }

    if (!prevTrack && track) this.setWindowTitle(track);
  }

  setWindowTitle(track = this.props.track) {
    if (track) {
      let artists = '';
      for (let i = 0; i < track.artists.length; i++) {
        if (artists != '') {
          artists += ', ';
        }
        artists += track.artists[i].name;
      }
      this.props.uiActions.setWindowTitle(`${track.name} by ${artists} (track)`);
    } else {
      this.props.uiActions.setWindowTitle('Track');
    }
  }

  handleContextMenu(e) {
    const data = {
      e,
      context: 'track',
      items: [this.props.track],
      uris: [this.props.uri],
    };
    this.props.uiActions.showContextMenu(data);
  }

  play() {
    this.props.mopidyActions.playURIs([this.props.uri], this.props.uri);
  }

  renderLyricsSelector() {
    const {
      track,
      geniusActions,
    } = this.props;

    if (track.lyrics_results === undefined || track.lyrics_results === null) {
      return null;
    } if (track.lyrics_results.length <= 0) {
      return (
        <div className="field lyrics-selector">
          <div className="input">
            <input type="text" disabled="disabled" value="No results" />
            <div className="description">
              Switch to another lyrics seach result
            </div>
          </div>
        </div>
      );
    }

    return (
      <div className="field lyrics-selector">
        <div className="input">
          <SelectField
            onChange={(value) => geniusActions.getTrackLyrics(track.uri, value)}
            options={
              track.lyrics_results.map((result) => ({
                value: result.path,
                label: result.title,
                defaultValue: (result.path === track.lyrics_path),
              }))
            }
          />
          <div className="description">
            Switch to another lyrics seach result
          </div>
        </div>
      </div>
    );
  }

  renderLyrics() {
    if (isLoading(this.props.load_queue, ['genius_'])) {
      return (
        <div className="lyrics">
          <Loader body loading />
        </div>
      );
    } if (this.props.track.lyrics) {
      return (
        <div className="lyrics">
          <div className="content" dangerouslySetInnerHTML={{ __html: this.props.track.lyrics }} />
          <div className="origin mid_grey-text">
            Origin:
            {' '}
            <a href={`https://genius.com${this.props.track.lyrics_path}`} target="_blank">{`https://genius.com${this.props.track.lyrics_path}`}</a>
          </div>
        </div>
      );
    }
    return (
      <ErrorMessage type="not-found" title="Could not load lyrics" />
    );
  }

  render() {
    if (isLoading(this.props.load_queue, [`spotify_track/${getFromUri('trackid', this.props.uri)}`])) {
      return <Loader body loading />
    }

    if (!this.props.track) {
      return null;
    }
    const { track } = this.props;

    // Flatten our simple album so we can inherit artwork
    if (track.album) {
      const album = this.props.albums[track.album.uri];

      if (album && album.images) {
        track.images = album.images;
      }
    }


    return (
      <div className="view track-view content-wrapper">

        {this.props.slim_mode ? (
          <Header
            icon="music"
            title="Track"
            handleContextMenuTrigger={(e) => this.handleContextMenu(e)}
            uiActions={this.props.uiActions}
          />
        ) : null}

        <div className="thumbnail-wrapper">
          <Thumbnail size="large" canZoom images={track.images} type="album" />
        </div>

        <div className="title">

          <h1>{track.name}</h1>
          <h2>
            {track.album && track.album.uri && <Link to={`/album/${track.album.uri}`}>{track.album.name}</Link>}
            {track.album && !track.album.uri ? track.album.name : null}
            {!track.album ? 'Unknown album' : null}
            {' by '}
            <LinksSentence items={track.artists} />
          </h2>

          <ul className="details">
            {!this.props.slim_mode ? <li className="source"><Icon type="fontawesome" name={sourceIcon(this.props.uri)} /></li> : null}
            {track.date ? <li><Dater type="date" data={track.date} /></li> : null}
            {track.explicit ? <li><span className="flag flag--dark">EXPLICIT</span></li> : null}
            <li>
              {track.disc_number > 0 && <span>Disc {track.disc_number}</span>}
              {track.disc_number > 0 && track.track_number > 0 && <span>,&nbsp;</span>}
              {track.track_number && <span>Track {track.track_number}</span>}
            </li>
            {track.duration && <li><Dater type="length" data={track.duration} /></li>}
            {track.popularity && <li>{`${track.popularity}% popularity`}</li>}
          </ul>
        </div>

        <div className="actions">
          <button className="button button--primary" onClick={(e) => this.play()}>Play</button>
          <LastfmLoveButton uri={this.props.uri} artist={(this.props.track.artists ? this.props.track.artists[0].name : null)} track={this.props.track.name} addText="Love" removeText="Unlove" is_loved={this.props.track.userloved} />
          <ContextMenuTrigger onTrigger={(e) => this.handleContextMenu(e)} />
        </div>

        {!this.props.genius_authorized ? (
          <p className="no-results">
Want track lyrics? Authorize Genius under
            <Link to="/settings/genius" scrollTo="#services-menu">Settings</Link>
.
          </p>
        ) : null}
        {this.props.genius_authorized ? this.renderLyricsSelector() : null}
        {this.props.genius_authorized ? this.renderLyrics() : null}

      </div>
    );
  }
}

/**
 * Rebuild a track URI with some ugly-ass handling of encoding.
 *
 * Basically the ID part of a Mopidy URI needs to be encoded, but the rest of the URI can't be.
 * This means we need to break down the URI (decoded) and then reconstruct with an encoded ID.
 * This is all required because he URI is passed to us *from* a URL which has been encoded for
 * obvious reasons.
 *
 * @param uri String
 * @return String
 * */
const rebuildUri = (uri) => {
  const rebuilt_uri = `${uriSource(uri)}:${uriType(uri)}:`;

  // Escape unreserved characters (RFC 3986)
  // https://stackoverflow.com/questions/18251399/why-doesnt-encodeuricomponent-encode-single-quotes-apostrophes
  let id = getFromUri('trackid', uri);
  id = encodeURIComponent(id).replace(/[!'()*]/g, escape);

  // Reinstate slashes for the Mopidy-Local structure
  id = id.replace(/%2F/g, '/');

  return rebuilt_uri + id;
};

const mapStateToProps = (state, ownProps) => {
  let uri = decodeURIComponent(ownProps.match.params.uri);
  uri = rebuildUri(uri);

  return {
    uri,
    slim_mode: state.ui.slim_mode,
    load_queue: state.ui.load_queue,
    track: (state.core.tracks && state.core.tracks[uri] !== undefined ? state.core.tracks[uri] : false),
    tracks: state.core.tracks,
    artists: state.core.artists,
    albums: state.core.albums,
    spotify_library_albums: state.spotify.library_albums,
    local_library_albums: state.mopidy.library_albums,
    lastfm_authorized: state.lastfm.authorization,
    spotify_authorized: state.spotify.authorization,
    genius_authorized: state.genius.authorization,
    mopidy_connected: state.mopidy.connected,
  };
};

const mapDispatchToProps = (dispatch) => ({
  coreActions: bindActionCreators(coreActions, dispatch),
  uiActions: bindActionCreators(uiActions, dispatch),
  mopidyActions: bindActionCreators(mopidyActions, dispatch),
  lastfmActions: bindActionCreators(lastfmActions, dispatch),
  spotifyActions: bindActionCreators(spotifyActions, dispatch),
  geniusActions: bindActionCreators(geniusActions, dispatch),
});

export default connect(mapStateToProps, mapDispatchToProps)(Track);
