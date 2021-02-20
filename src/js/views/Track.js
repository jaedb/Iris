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
import { sourceIcon } from '../util/helpers';
import { i18n, I18n } from '../locale';
import Button from '../components/Button';
import { makeLoadingSelector, makeItemSelector } from '../util/selectors';
import { decodeUri } from '../util/format';

const LyricsSelector = ({
  track: {
    uri,
    lyrics_results,
    lyrics_path,
  } = {},
  getTrackLyrics,
}) => {

  if (lyrics_results === undefined || lyrics_results === null) return null;
  if (lyrics_results.length <= 0) {
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
          onChange={(value) => getTrackLyrics(uri, value)}
          options={
            lyrics_results.map((result) => ({
              value: result.path,
              label: result.title,
              defaultValue: (result.path === lyrics_path),
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

const LyricsContent = ({
  authorized,
  loading,
  track: {
    lyrics,
    lyrics_path,
  } = {},
}) => {
  if (loading) return <Loader mini />;
  if (!lyrics && !authorized) {
    return (
      <p className="no-results">
        <I18n path="track.want_lyrics" />
        <Link to="/settings/services/genius" scrollTo="#services-menu">
          <I18n path="settings.title" />
        </Link>
        .
      </p>
    );
  };
  if (!lyrics) return null;

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

const Lyrics = ({
  loading,
  authorized,
  track,
  getTrackLyrics,
}) => {
  return (
    <>
      <h4>
        <I18n path="track.lyrics" />
        {loading && <Loader loading mini />}
      </h4>

      <LyricsSelector
        getTrackLyrics={getTrackLyrics}
        authorized={authorized}
        track={track}
      />
      <LyricsContent
        track={track}
        authorized={authorized}
      />
    </>
  );
}

class Track extends React.Component {
  componentDidMount() {
    const {
      uri,
      track,
      coreActions: {
        loadTrack,
      },
    } = this.props;

    loadTrack(decodeUri(uri), { full: true, lyrics: true });

    if (track) {
      this.setWindowTitle(track);
    }
  }

  componentDidUpdate = ({
    uri: prevUri,
    track: prevTrack,
  }) => {
    const {
      uri,
      track,
      coreActions: {
        loadTrack,
      },
    } = this.props;

    if (prevUri !== uri) {
      loadTrack(decodeUri(uri), { full: true, lyrics: true });
    }

    if (!prevTrack && track) this.setWindowTitle(track);
  }

  setWindowTitle = (track = this.props.track) => {
    const {
      uiActions: { setWindowTitle },
    } = this.props;

    if (track) {
      let artists = '';
      if (artists) {
        for (let i = 0; i < track.artists.length; i++) {
          if (artists != '') {
            artists += ', ';
          }
          artists += track.artists[i].name;
        }
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

  render = () => {
    const {
      uri,
      track,
      loading,
      slim_mode,
      uiActions,
      genius_authorized,
      geniusActions: {
        getTrackLyrics
      },
      loadingLyrics,
    } = this.props;

    if (!track) {
      if (loading) {
        return <Loader body loading />;
      }
      return (
        <ErrorMessage type="not-found" title="Not found">
          <p>
            {i18n('errors.uri_not_found', { uri })}
          </p>
        </ErrorMessage>
      );
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
            {track.artists && <LinksSentence items={track.artists} type="artist" />}
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
                <I18n path="specs.popularity" percent={track.popularity} />
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

        <Lyrics
          loading={loadingLyrics}
          authorized={genius_authorized}
          getTrackLyrics={getTrackLyrics}
          track={track}
        />

      </div>
    );
  }
}

const mapStateToProps = (state, ownProps) => {
  const uri = decodeUri(ownProps.match.params.uri);
  const loadingSelector = makeLoadingSelector([`(.*)${uri}(.*)`, '^((?!genius).)*$', '^((?!contains).)*$']);
  const loadingLyricsSelector = makeLoadingSelector([`^genius_(.*)lyrics_${uri}$`]);
  const trackSelector = makeItemSelector(uri);

  return {
    uri,
    slim_mode: state.ui.slim_mode,
    loading: loadingSelector(state),
    loadingLyrics: loadingLyricsSelector(state),
    track: trackSelector(state),
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
