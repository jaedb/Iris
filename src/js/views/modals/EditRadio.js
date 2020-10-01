
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
import TextField from '../../components/Fields/TextField';
import Button from '../../components/Button';

const SeedListItem = ({ seed, remove }) => (
  <div className="list__item list__item--no-interaction" key={seed.uri}>
    {seed.unresolved ? (
      <span className="mid_grey-text">{seed.uri}</span>
    ) : (
      <span>{seed.name}</span>
    )}
    {!seed.unresolved && (
      <span className="mid_grey-text">
        {` (${uriType(seed.uri)})`}
      </span>
    )}
    <Button
      type="destructive"
      size="tiny"
      className="pull-right"
      discrete
      onClick={() => remove(seed.uri)}
      tracking={{ category: 'Directory', action: 'Play' }}
    >
      <Icon name="delete" />
      <I18n path="actions.remove" />
    </Button>
  </div>
);

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
    const {
      uiActions: {
        setWindowTitle,
      },
      radio,
    } = this.props;

    setWindowTitle(i18n('modal.edit_radio.title'));

    if (radio && radio.enabled) {
      this.loadRadio(radio);
    }
  }

  componentDidUpdate = ({ radio: prev_radio }) => {
    const { radio } = this.props;
    if (!prev_radio && radio) this.loadRadio(radio);
  }

  onUriChange = (uri) => {
    this.setState({ uri, error_message: null });
  }

  loadRadio = (radio) => {
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

  addSeed = (e) => {
    const {
      uri,
      seeds,
    } = this.state;
    const {
      coreActions: {
        loadItem,
      },
    } = this.props;

    e.preventDefault();

    if (uri === '') {
      this.setState({ error_message: i18n('errors.required') });
      return;
    }

    this.setState({ error_message: null });

    const validatedSeeds = Object.assign([], seeds);
    let uris = uri.split(',');

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
        validatedSeeds.push(uris[i]);
        loadItem(uris[i]);
      }
    }

    this.setState({
      seeds: validatedSeeds,
      uri: '',
    });
  }

  removeSeed = (uri) => {
    const {
      seeds: prevSeeds,
    } = this.state;

    const seeds = prevSeeds.filter((seed) => seed !== uri);
    this.setState({ seeds });
  }

  mapSeeds = () => {
    const { seeds } = this.state;
    const { items } = this.props;

    if (!seeds) {
      return [];
    }

    return seeds.slice(0, 5).map((uri) => items[uri] || { uri, unresolved: true });
  }

  render = () => {
    const {
      enabled,
      uri,
      error_message,
    } = this.state;

    const seeds = this.mapSeeds();

    return (
      <Modal className="modal--edit-radio">
        <h1>
          <I18n path="modal.edit_radio.title" />
        </h1>
        <h2 className="mid_grey-text">
          <I18n path="modal.edit_radio.subtitle" />
        </h2>

        <form onSubmit={(e) => { (enabled ? this.handleUpdate(e) : this.handleStart(e)); }}>

          <div className="field text">
            <div className="name">
              <I18n path="fields.uri" />
            </div>
            <div className="input">
              <TextField
                onChange={this.onUriChange}
                value={uri}
              />
              <span className="button discrete add-uri no-hover" onClick={(e) => this.addSeed(e)}>
                <Icon name="add" />
                <I18n path="actions.add" />
              </span>
              {error_message && <span className="description error">{error_message}</span>}
            </div>
          </div>

          <div className="field text">
            <div className="name">
              <I18n path="fields.items_to_add.label" />
            </div>
            <div className="input">
              {seeds.length ? (
                <div className="list">
                  {seeds.map((seed, index) => (
                    <SeedListItem
                      key={`${seed}_${index}`}
                      seed={seed}
                      remove={this.removeSeed}
                    />
                  ))}
                </div>
              ) : (
                <div className="text mid_grey-text">
                  <I18n path="fields.items_to_add.placeholder" />
                </div>
              )}
            </div>
          </div>

          <div className="actions centered-text">
            {enabled && (
              <Button
                type="destructive"
                size="large"
                onClick={(e) => this.handleStop(e)}
                tracking={{ category: 'Radio', action: 'Stop' }}
              >
                <I18n path="actions.add" />
              </Button>
            )}

            {enabled ? (
              <Button
                type="primary"
                size="large"
                onClick={(e) => this.handleUpdate(e)}
                tracking={{ category: 'Radio', action: 'Update' }}
              >
                <I18n path="actions.save" />
              </Button>
            ) : (
              <Button
                type="primary"
                size="large"
                onClick={(e) => this.handleStart(e)}
                tracking={{ category: 'Radio', action: 'Start' }}
              >
                <I18n path="actions.start" />
              </Button>
            )}
          </div>
        </form>
      </Modal>
    );
  }
}

const mapStateToProps = (state) => ({
  radio: state.core.radio,
  items: state.core.items,
});

const mapDispatchToProps = (dispatch) => ({
  coreActions: bindActionCreators(coreActions, dispatch),
  uiActions: bindActionCreators(uiActions, dispatch),
  mopidyActions: bindActionCreators(mopidyActions, dispatch),
  pusherActions: bindActionCreators(pusherActions, dispatch),
  spotifyActions: bindActionCreators(spotifyActions, dispatch),
});

export default connect(mapStateToProps, mapDispatchToProps)(EditRadio);
