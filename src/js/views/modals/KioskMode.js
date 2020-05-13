
import React from 'react';
import { connect } from 'react-redux';
import { bindActionCreators } from 'redux';
import Link from '../../components/Link';
import Modal from './Modal';
import Thumbnail from '../../components/Thumbnail';
import LinksSentence from '../../components/LinksSentence';
import Loader from '../../components/Loader';
import Icon from '../../components/Icon';
import ProgressSlider from '../../components/Fields/ProgressSlider';
import * as uiActions from '../../services/ui/actions';
import * as mopidyActions from '../../services/mopidy/actions';
import * as geniusActions from '../../services/genius/actions';
import { isLoading } from '../../util/helpers';

const LyricsScroller = ({ content = '', time_position = 1, duration = 100 }) => {
  const percent = ((time_position / duration) * 110).toFixed(4);

  return (
    <div className="lyrics">
      <div
        className="lyrics__content"
        dangerouslySetInnerHTML={{ __html: content }}
        style={{ transform: `translateY(-${percent}%)` }}
      />
    </div>
  );
}

class KioskMode extends React.Component {

  componentDidMount() {
    const {
      current_track,
      genius_authorized,
      show_lyrics,
      geniusActions,
    } = this.props;
    this.setWindowTitle();

    if (show_lyrics && genius_authorized && current_track && current_track.artists && !current_track.lyrics_results) {
      geniusActions.findTrackLyrics(current_track);
    }
  }

  componentDidUpdate = ({
    current_track: prev_current_track,
  }) => {
    const {
      current_track,
      show_lyrics,
      genius_authorized,
      geniusActions: {
        findTrackLyrics,
      },
    } = this.props;

    if (!prev_current_track && current_track) {
      this.setWindowTitle(current_track);

      if (show_lyrics && genius_authorized && current_track && current_track.artists && !current_track.lyrics_results) {
        findTrackLyrics(current_track);
      }
    } else if (show_lyrics !== show_lyrics && show_lyrics && current_track) {
      if (genius_authorized && current_track && current_track.artists && !current_track.lyrics_results) {
        findTrackLyrics(current_track);
      }
    }
  }

  setWindowTitle(current_track = this.props.current_track) {
    if (current_track) {
      let artists = '';
      for (let i = 0; i < current_track.artists.length; i++) {
        if (artists != '') {
          artists += ', ';
        }
        artists += current_track.artists[i].name;
      }
      this.props.uiActions.setWindowTitle(`${current_track.name} by ${artists} (now playing)`);
    } else {
      this.props.uiActions.setWindowTitle('Now playing');
    }
  }

  togglePlay(e) {
    if (this.props.play_state == 'playing') {
      this.props.mopidyActions.pause();
    } else {
      this.props.mopidyActions.play();
    }
  }

  toggleLyrics = () => {
    const {
      show_lyrics,
      uiActions,
      genius_authorized,
      current_track,
    } = this.props;

    uiActions.set({ show_lyrics: !show_lyrics });
    if (
      !show_lyrics
      && this.props.genius_authorized
      && current_track
      && current_track.artists
      && !current_track.lyrics_results) {
      this.props.geniusActions.findTrackLyrics(current_track);
    }
  }

  renderLyrics = () => {
    const {
      load_queue,
      genius_authorized,
      time_position = null,
      current_track,
    } = this.props;

    const { lyrics, duration } = current_track || {};

    if (isLoading(load_queue, ['genius_'])) {
      return (
        <div className="lyrics">
          <Loader body loading />
        </div>
      );
    } else if (!genius_authorized) {

      return (
        <p className="no-results">
          Want track lyrics? Authorize Genius under
          {' '}
          <Link to="/settings/genius" scrollTo="#services-menu">Settings</Link>.
        </p>
      );

    } else if (lyrics) {
      return (
        <LyricsScroller
          content={lyrics}
          time_position={time_position}
          duration={duration}
        />
      );
    };
    return null;
  }

  renderPlayButton() {
    let button = <button className="control play" onClick={() => this.props.mopidyActions.play()}><Icon name="play_circle_filled" type="material" /></button>;
    if (this.props.play_state == 'playing') {
      button = <button className="control play" onClick={() => this.props.mopidyActions.pause()}><Icon name="pause_circle_filled" type="material" /></button>;
    }
    return button;
  }

  render() {
    const {
      show_lyrics,
      current_track,
    } = this.props;
    if (current_track && current_track.images) {
      var { images } = current_track;
    } else {
      var images = [];
    }

    const extraControls = (
      <div className="control" onClick={this.toggleLyrics}>
        {show_lyrics ? <Icon name="toggle_on" className="turquoise-text" />
          : <Icon name="toggle_off" />}
        <div style={{ paddingLeft: '6px', fontWeight: 'bold' }}>
          Lyrics
        </div>
      </div>
    );

    return (
      <Modal
        className="modal--kiosk-mode"
        extraControls={extraControls}
      >
        <Thumbnail className="background" images={images} placeholder={false} />

        <div className={`player player--${show_lyrics ? 'with' : 'without'}-lyrics`}>

          <div className="track">
            <div className="track__artwork">
              <Thumbnail images={images} useImageTag />
            </div>
            <div className="track__info">
              <div className="title">{ current_track ? current_track.name : <span>-</span> }</div>
              { current_track ? <LinksSentence nolinks items={current_track.artists} /> : <LinksSentence /> }
            </div>
          </div>

          <div className="playback">
            <div className="playback__controls">
              <button className="control previous" onClick={() => this.props.mopidyActions.previous()}>
                <Icon name="navigate_before" type="material" />
              </button>
              { this.renderPlayButton() }
              <button className="control next" onClick={() => this.props.mopidyActions.next()}>
                <Icon name="navigate_next" type="material" />
              </button>
            </div>
            <div className="playback__progress">
              <ProgressSlider />
            </div>
          </div>

        </div>

        {show_lyrics && this.renderLyrics()}
      </Modal>
    );
  }
}

const mapStateToProps = (state) => ({
  play_state: state.mopidy.play_state,
  current_track: (state.core.current_track && state.core.tracks[state.core.current_track.uri] !== undefined ? state.core.tracks[state.core.current_track.uri] : null),
  time_position: state.mopidy.time_position,
  load_queue: state.ui.load_queue,
  show_lyrics: state.ui.show_lyrics,
  genius_authorized: state.genius.authorization,
});

const mapDispatchToProps = (dispatch) => ({
  uiActions: bindActionCreators(uiActions, dispatch),
  mopidyActions: bindActionCreators(mopidyActions, dispatch),
  geniusActions: bindActionCreators(geniusActions, dispatch),
});

export default connect(mapStateToProps, mapDispatchToProps)(KioskMode);
