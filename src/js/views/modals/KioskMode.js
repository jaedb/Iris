
import React from 'react';
import { connect } from 'react-redux';
import { createStore, bindActionCreators } from 'redux';
import Link from '../../components/Link';

import Modal from './Modal';
import Thumbnail from '../../components/Thumbnail';
import LinksSentence from '../../components/LinksSentence';
import Loader from '../../components/Loader';
import Icon from '../../components/Icon';
import ProgressSlider from '../../components/Fields/ProgressSlider';

import * as helpers from '../../helpers';
import * as uiActions from '../../services/ui/actions';
import * as mopidyActions from '../../services/mopidy/actions';
import * as geniusActions from '../../services/genius/actions';

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
  constructor(props) {
    super(props);

    this.state = {
      showLyrics: false,
    }
  }

  componentDidMount() {
    this.setWindowTitle();
  }

  componentWillReceiveProps(nextProps) {
    if (!this.props.current_track && nextProps.current_track) {
      this.setWindowTitle(nextProps.current_track);

      if (this.state.showLyrics && nextProps.genius_authorized && nextProps.current_track && nextProps.current_track.artists && !nextProps.current_track.lyrics_results) {
        this.props.geniusActions.findTrackLyrics(nextProps.current_track);
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
      showLyrics,
    } = this.state;

    this.setState({ showLyrics: !showLyrics });
    if (
      !showLyrics
      && this.props.genius_authorized
      && this.props.current_track
      && this.props.current_track.artists
      && !this.props.current_track.lyrics_results) {
      this.props.geniusActions.findTrackLyrics(this.props.current_track);
    }
  }

  renderLyrics = () => {
    if (helpers.isLoading(this.props.load_queue, ['genius_'])) {
      return (
        <div className="lyrics">
          <Loader loading />
        </div>
      );
    } if (this.props.current_track && this.props.current_track.lyrics) {
      return (
        <LyricsScroller
          content={this.props.current_track.lyrics}
          time_position={this.props.time_position}
          duration={this.props.current_track ? this.props.current_track.duration : null}
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
      showLyrics,
    } = this.state;
    if (this.props.current_track && this.props.current_track.images) {
      var { images } = this.props.current_track;
    } else {
      var images = [];
    }

    const extraControls = (      
      <div className="control" onClick={this.toggleLyrics} style={showLyrics ? { opacity: 1 } : {}}>
        <Icon name="queue_music" className={showLyrics ? 'turquoise-text' : null} />
      </div>
    );

    return (
      <Modal className="modal--kiosk-mode" extraControls={extraControls}>
        <Thumbnail className="background" images={images} />

        <div className={`track-info track-info--${showLyrics ? 'with' : 'without'}-lyrics`}>
          <div className="artwork">
            <Thumbnail images={images} />
          </div>

          <div className="player">
            <div className="current-track">
              <div className="title">{ this.props.current_track ? this.props.current_track.name : <span>-</span> }</div>
              { this.props.current_track ? <LinksSentence nolinks items={this.props.current_track.artists} /> : <LinksSentence /> }
            </div>

            <div className="player__controls">
              <button className="control previous" onClick={() => this.props.mopidyActions.previous()}>
                <Icon name="navigate_before" type="material" />
              </button>
              { this.renderPlayButton() }
              <button className="control next" onClick={() => this.props.mopidyActions.next()}>
                <Icon name="navigate_next" type="material" />
              </button>
            </div>

            <div className="progress-wrapper">
              <ProgressSlider />
            </div>
          </div>
        </div>
        {showLyrics && this.renderLyrics()}
      </Modal>
    );
  }
}

const mapStateToProps = (state) => ({
  play_state: state.mopidy.play_state,
  current_track: (state.core.current_track && state.core.tracks[state.core.current_track.uri] !== undefined ? state.core.tracks[state.core.current_track.uri] : null),
  time_position: state.mopidy.time_position,
  load_queue: state.ui.load_queue,
  genius_authorized: state.genius.authorization,
});

const mapDispatchToProps = (dispatch) => ({
  uiActions: bindActionCreators(uiActions, dispatch),
  mopidyActions: bindActionCreators(mopidyActions, dispatch),
  geniusActions: bindActionCreators(geniusActions, dispatch),
});

export default connect(mapStateToProps, mapDispatchToProps)(KioskMode);
