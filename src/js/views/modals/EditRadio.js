
import React from 'react';
import { connect } from 'react-redux';
import { bindActionCreators } from 'redux';
import Modal from './Modal';
import Icon from '../../components/Icon';
import * as coreActions from '../../services/core/actions';
import * as uiActions from '../../services/ui/actions';
import * as mopidyActions from '../../services/mopidy/actions';
import * as spotifyActions from '../../services/spotify/actions';
import * as pusherActions from '../../services/pusher/actions';
import {
  uriSource,
  uriType,
} from '../../util/helpers';
import { i18n, I18n } from '../../locale';

class EditRadio extends React.Component {
  constructor(props) {
    super(props);
    this.state = {
      enabled: false,
      seeds: [],
      uri: '',
      error_message: null,
    };
  }

  componentDidMount() {
    this.props.uiActions.setWindowTitle(i18n('modal.edit_radio.title'));

    if (this.props.radio && this.props.radio.enabled) {
      this.loadRadio(this.props.radio);
    }
  }

  componentDidUpdate = ({ radio: prev_radio }) => {
    const { radio } = this.props;
    if (!prev_radio && radio) this.loadRadio(radio);
  }

  loadRadio(radio) {
    const seeds = [...radio.seed_tracks, ...radio.seed_artists, ...radio.seed_genres];
    this.setState({ seeds, enabled: radio.enabled });
  }

  handleStart(e) {
    e.preventDefault();

    let valid_seeds = true;
    const seeds = this.mapSeeds();
    for (let i = 0; i < seeds.length; i++) {
      if (seeds[i].unresolved !== undefined) {
        valid_seeds = false;
        continue;
      }
    }

    if (valid_seeds) {
      this.props.pusherActions.startRadio(this.state.seeds);
      window.history.back();
    } else {
      this.setState({ error_message: i18n('modal.edit_radio.invalid_seed_uri') });
    }
  }

  handleUpdate(e) {
    e.preventDefault();

    let valid_seeds = true;
    const seeds = this.mapSeeds();
    for (let i = 0; i < seeds.length; i++) {
      if (seeds[i].unresolved !== undefined) {
        valid_seeds = false;
        continue;
      }
    }

    if (valid_seeds) {
      this.props.pusherActions.updateRadio(this.state.seeds);
      window.history.back();
    } else {
      this.setState({ error_message: i18n('modal.edit_radio.invalid_seed_uri') });
    }
  }

  handleStop(e) {
    e.preventDefault();
    this.props.pusherActions.stopRadio();
    this.props.uiActions.closeModal();
  }

  addSeed(e) {
    e.preventDefault();

    if (this.state.uri == '') {
      this.setState({ error_message: i18n('errors.required') });
      return;
    }

    this.setState({ error_message: null });

    const seeds = Object.assign([], this.state.seeds);
    let uris = this.state.uri.split(',');

    if (uris.length >= 5) {
      uris = uris.slice(0, 5);
      this.setState({ error_message: i18n('modal.edit_radio.too_many_seeds') });
    }

    for (let i = 0; i < uris.length; i++) {
      if (uriSource(uris[i]) !== 'spotify') {
        this.setState({ error_message: i18n('modal.edit_radio.only_spotify_uris') });
        return;
      } if (seeds.indexOf(uris[i]) > -1) {
        this.setState({ error_message: i18n('modal.edit_radio.too_many_seeds') });
      } else {
        seeds.push(uris[i]);
      }

      // Resolve
      switch (uriType(uris[i])) {
        case 'track':
          this.props.spotifyActions.getTrack(uris[i]);
          break;

        case 'artist':
          this.props.spotifyActions.getArtist(uris[i]);
          break;
      }
    }

    // commit to state
    this.setState({
      seeds,
      uri: '',
    });
  }

  removeSeed(uri) {
    const seeds = [];
    for (let i = 0; i < this.state.seeds.length; i++) {
      if (this.state.seeds[i] != uri) {
        seeds.push(this.state.seeds[i]);
      }
    }
    this.setState({ seeds });
  }

  mapSeeds() {
    const seeds = [];

    if (this.state.seeds) {
      for (let i = 0; i < this.state.seeds.length && i < 5; i++) {
        const uri = this.state.seeds[i];
        if (uri) {
          if (uriType(uri) == 'artist') {
            if (this.props.artists && this.props.artists.hasOwnProperty(uri)) {
              seeds.push(this.props.artists[uri]);
            } else {
              seeds.push({
                unresolved: true,
                uri,
              });
            }
          } else if (uriType(uri) == 'track') {
            if (this.props.tracks && this.props.tracks.hasOwnProperty(uri)) {
              seeds.push(this.props.tracks[uri]);
            } else {
              seeds.push({
                unresolved: true,
                uri,
              });
            }
          }
        }
      }
    }

    return seeds;
  }

  renderSeeds() {
    const seeds = this.mapSeeds();

    if (seeds.length > 0) {
      return (
        <div>
          <div className="list">
            {
							seeds.map((seed, index) => (
  <div className="list__item" key={seed.uri}>
    {seed.unresolved ? <span className="mid_grey-text">{seed.uri}</span> : <span>{seed.name}</span> }
    {!seed.unresolved ? (
      <span className="mid_grey-text">
&nbsp;(
        {uriType(seed.uri)}
)
      </span>
    ) : null}
    <span className="button discrete remove-uri no-hover" onClick={(e) => this.removeSeed(seed.uri)}>
      <Icon name="delete" />
      <I18n path="actions.delete" />
    </span>
  </div>
							))
						}
          </div>
        </div>
      );
    }
    return (
      <div className="no-results">
        <I18n path="modal.edit_radio.no_seeds" />
      </div>
    );
  }

  render() {
    return (
      <Modal className="modal--edit-radio">
        <h1>
          <I18n path="modal.edit_radio.title" />
        </h1>
        <h2 className="mid_grey-text">
          <I18n path="modal.edit_radio.subtitle" />
        </h2>

        <form onSubmit={(e) => { (this.state.enabled ? this.handleUpdate(e) : this.handleStart(e)); }}>

          {this.renderSeeds()}

          <div className="field text">
            <div className="name">
              <I18n path="fields.uri" />
            </div>
            <div className="input">
              <input
                type="text"
                onChange={(e) => this.setState({ uri: e.target.value, error_message: null })}
                value={this.state.uri}
              />
              <span className="button discrete add-uri no-hover" onClick={(e) => this.addSeed(e)}>
                <Icon name="add" />
                <I18n path="actions.add" />
              </span>
              {this.state.error_message ? <span className="description error">{this.state.error_message}</span> : null}
            </div>
          </div>

          <div className="actions centered-text">
            {this.state.enabled ? (
              <span className="button button--destructive button--large" onClick={(e) => this.handleStop(e)}>
                <I18n path="actions.add" />
              </span>
            ) : null}

            {this.state.enabled ? (
              <button
                className="button button--primary button--large"
                onClick={(e) => this.handleUpdate(e)}
                type="button"
              >
                <I18n path="actions.save" />
              </button>
            ) : (
              <button
                className="button button--primary button--large"
                onClick={(e) => this.handleStart(e)}
                type="button"
              >
                <I18n path="actions.start" />
              </button>
            )}
          </div>
        </form>
      </Modal>
    );
  }
}

const mapStateToProps = (state, ownProps) => ({
  mopidy_connected: state.mopidy.connected,
  radio: state.core.radio,
  artists: state.core.artists,
  tracks: state.core.tracks,
});

const mapDispatchToProps = (dispatch) => ({
  coreActions: bindActionCreators(coreActions, dispatch),
  uiActions: bindActionCreators(uiActions, dispatch),
  mopidyActions: bindActionCreators(mopidyActions, dispatch),
  pusherActions: bindActionCreators(pusherActions, dispatch),
  spotifyActions: bindActionCreators(spotifyActions, dispatch),
});

export default connect(mapStateToProps, mapDispatchToProps)(EditRadio);
