
import React from 'react';
import { connect } from 'react-redux';
import { bindActionCreators } from 'redux';
import Link from '../components/Link';
import ErrorMessage from '../components/ErrorMessage';
import Header from '../components/Header';
import Thumbnail from '../components/Thumbnail';
import LinksSentence from '../components/LinksSentence';
import LastfmLoveButton from '../components/Fields/LastfmLoveButton';
import { Dater } from '../components/Dater';
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
import { i18n, I18n } from '../locale';
import Button from '../components/Button';

class Track extends React.Component {
  componentDidMount() {
    const {
      uri,
      track,
      coreActions: { loadTrack },
      genius_authorized,
      geniusActions: { findTrackLyrics },
    } = this.props;

    loadTrack(uri);

    if (track) {
      this.setWindowTitle(track);

      if (genius_authorized && track.artists && !track.lyrics_results) {
        findTrackLyrics(track);
      }
    }
  }

  componentDidUpdate = ({
    uri: prevUri,
    track: prevTrack,
  }) => {
    const {
      uri,
      track,
      genius_authorized,
      lastfm_authorized,
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

    if (prevUri !== uri) {
      loadTrack(uri);

      if (genius_authorized && track.artists) {
        findTrackLyrics(track);
      }
    }

    // We have just received our full track or our track artists
    if ((!prevTrack && track) || (!prevTrack.artists && track.artists)) {
      this.setWindowTitle(track);
      if (lastfm_authorized) getTrack(track.uri);
      if (genius_authorized && !track.lyrics_results) findTrackLyrics(track);
    }

    if (!prevTrack && track) this.setWindowTitle(track);
  }

  setWindowTitle = (track = this.props.track) => {
    const {
      uiActions: { setWindowTitle },
    } = this.props;

    if (track) {
      let artists = '';
      for (let i = 0; i < track.artists.length; i++) {
        if (artists != '') {
          artists += ', ';
        }
        artists += track.artists[i].name;
      }
      setWindowTitle(i18n('track.title_window', { name: track.name, artists }));
    } else {
      setWindowTitle(i18n('track.title'));
    }
  }

  handleContextMenu = (e) => {
    const {
      uri,
      track,
      uiActions: { showContextMenu },
    } = this.props;

    showContextMenu({
      e,
      context: 'track',
      items: [track],
      uris: [uri],
    });
  }

  play = () => {
    const {
      uri,
      mopidyActions: { playURIs },
    } = this.props;

    playURIs([uri], uri);
  }

  renderLyricsSelector = () => {
    const {
      track,
      geniusActions: { getTrackLyrics },
    } = this.props;

    if (track.lyrics_results === undefined || track.lyrics_results === null) {
      return null;
    } if (track.lyrics_results.length <= 0) {
      return (
        <div className="field lyrics-selector">
          <div className="input">
            <input type="text" disabled="disabled" value="No results" />
            <div className="description">
              <I18n path="services.genius.switch_lyrics_result" />
            </div>
          </div>
        </div>
      );
    }

    return (
      <div className="field lyrics-selector">
        <div className="input">
          <SelectField
            onChange={(value) => getTrackLyrics(track.uri, value)}
            options={
              track.lyrics_results.map((result) => ({
                value: result.path,
                label: result.title,
                defaultValue: (result.path === track.lyrics_path),
              }))
            }
          />
          <div className="description">
            <I18n path="services.genius.switch_lyrics_result" />
          </div>
        </div>
      </div>
    );
  }

  renderLyrics = () => {
    const {
      load_queue,
      track: {
        lyrics,
        lyrics_path,
      } = {},
    } = this.props;

    if (isLoading(load_queue, ['genius_'])) {
      return (
        <div className="lyrics">
          <Loader body loading />
        </div>
      );
    } if (lyrics) {
      return (
        <div className="lyrics">
          <div className="content" dangerouslySetInnerHTML={{ __html: lyrics }} />
          <div className="origin mid_grey-text">
            <I18n path="track.lyrics_origin" />
            <a
              href={`https://genius.com${lyrics_path}`}
              target="_blank"
              rel="noreferrer noopener"
            >
              {`https://genius.com${lyrics_path}`}
            </a>
          </div>
        </div>
      );
    }
    return (
      <ErrorMessage type="not-found" title={i18n('errors.no_results')} />
    );
  }

  render = () => {
    const {
      uri,
      track,
      albums,
      load_queue,
      slim_mode,
      uiActions,
      genius_authorized,
    } = this.props;

    if (isLoading(load_queue, [`spotify_track/${getFromUri('trackid', uri)}`])) {
      return <Loader body loading />;
    }

    if (!track) return null;

    // Flatten our simple album so we can inherit artwork
    if (track.album) {
      const album = albums[track.album.uri];

      if (album && album.images) {
        track.images = album.images;
      }
    }


    return (
      <div className="view track-view content-wrapper">

        {slim_mode && (
          <Header
            icon="music"
            title="Track"
            handleContextMenuTrigger={this.handleContextMenu}
            uiActions={uiActions}
          />
        )}

        <div className="thumbnail-wrapper">
          <Thumbnail size="large" canZoom images={track.images} type="album" />
        </div>

        <div className="title">

          <h1>{track.name}</h1>
          <h2>
            {track.album && track.album.uri && <Link to={`/album/${track.album.uri}`}>{track.album.name}</Link>}
            {track.album && !track.album.uri ? track.album.name : null}
            {!track.album && <I18n path="track.unknown_album" />}
            <I18n path="common.by" />
            <LinksSentence items={track.artists} />
          </h2>

          <ul className="details">
            {!slim_mode && (
              <li className="source">
                <Icon type="fontawesome" name={sourceIcon(uri)} />
              </li>
            )}
            {track.date && <li><Dater type="date" data={track.date} /></li>}
            {track.explicit && (
              <li>
                <span className="flag flag--dark uppercase">
                  <I18n path="track.explicit" />
                </span>
              </li>
            )}
            <li>
              {track.disc_number > 0 && (
                <I18n path="track.disc_number" number={track.disc_number} />
              )}
              {track.disc_number > 0 && track.track_number > 0 && <span>,&nbsp;</span>}
              {track.track_number && (
                <I18n path="track.track_number" number={track.track_number} />
              )}
            </li>
            {track.duration && <li><Dater type="length" data={track.duration} /></li>}
            {track.popularity && (
              <li>
                <I18n path="stats.popularity" percent={track.popularity} />
              </li>
            )}
          </ul>
        </div>

        <div className="actions">
          <Button
            type="primary"
            onClick={this.play}
            tracking={{ category: 'Track', action: 'Play' }}
          >
            <I18n path="actions.play" />
          </Button>
          <LastfmLoveButton
            uri={uri}
            artist={(track.artists ? track.artists[0].name : null)}
            track={track.name}
            is_loved={track.userloved}
          />
          <ContextMenuTrigger onTrigger={this.handleContextMenu} />
        </div>

        {!genius_authorized && (
          <p className="no-results">
            <I18n path="track.want_lyrics" />
            <Link to="/settings/services/genius" scrollTo="#services-menu">
              <I18n path="settings.title" />
            </Link>
            .
          </p>
        )}
        {genius_authorized && this.renderLyricsSelector()}
        {genius_authorized && this.renderLyrics()}

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
