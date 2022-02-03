import React from 'react';
import { connect } from 'react-redux';
import { bindActionCreators } from 'redux';
import Modal from './Modal';
import ColourField from '../../components/Fields/ColourField';
import IconField from '../../components/Fields/IconField';
import TextField from '../../components/Fields/TextField';
import * as pusherActions from '../../services/pusher/actions';
import * as uiActions from '../../services/ui/actions';
import { scrollTo, generateGuid } from '../../util/helpers';
import { i18n, I18n } from '../../locale';
import Button from '../../components/Button';

class EditCommand extends React.Component {
  constructor(props) {
    super(props);
    this.state = {
      id: generateGuid(),
      icon: 'power_settings_new',
      name: '',
      colour: '',
      url: `https://${window.location.hostname}/broadlink/sendCommand/power/`,
      method: 'GET',
      post_data: '',
      additional_headers: '',
    };
  }

  componentDidMount() {
    if (this.props.command) {
      this.props.uiActions.setWindowTitle(i18n('modal.edit_command.title'));
      this.setState(this.props.command);
    } else {
      this.props.uiActions.setWindowTitle(i18n('modal.edit_command.title_create'));
    }
  }

  handleSubmit(e) {
    e.preventDefault();

    this.props.pusherActions.setCommand(this.state);

    window.history.back();

    // A bit hacky, but wait for a moment to allow the back navigation
    // and then scroll down to our commands list
    setTimeout(() => {
      scrollTo('#commands-setup');
    },
    10);

    return false;
  }

  handleDelete(e) {
    this.props.pusherActions.removeCommand(this.state.id);
    window.history.back();

    // A bit hacky, but wait for a moment to allow the back navigation
    // and then scroll down to our commands list
    setTimeout(() => {
      scrollTo('#commands-setup');
    },
    10);
  }

  render() {
    const { command } = this.props;
    const {
      name,
      colour,
      icon,
      url,
      method,
      post_data,
      additional_headers,
    } = this.state;
    const icons = [
      'power_settings_new',
      'eject',
      'grade',
      'query_builder',
      'settings_input_component',
      'settings_input_hdmi',
      'settings_input_svideo',
      'forward_5',
      'forward_10',
      'forward_30',
      'replay',
      'replay_5',
      'replay_10',
      'replay_30',
      'skip_next',
      'skip_previous',
      'play_arrow',
      'pause',
      'stop',
      'shuffle',
      'snooze',
      'volume_off',
      'volume_down',
      'volume_up',
      'arrow_left',
      'arrow_drop_up',
      'arrow_right',
      'arrow_drop_down',
      'done',
      'done_all',
      'add',
      'remove',
      'clear',
      'cast',
      'speaker',
      'speaker_group',
      'audiotrack',
      'videogame_asset',
      'computer',
      'tv',
    ];

    return (
      <Modal className="modal--create-command">
        <h1>
          <I18n path={`modal.edit_command.title${command ? '' : '_create'}`} />
        </h1>
        <form onSubmit={(e) => this.handleSubmit(e)}>

          <div className="field textarea white">
            <div className="name">
              <I18n path="modal.edit_command.name" />
            </div>
            <div className="input">
              <TextField
                name="name"
                value={name}
                onChange={(value) => this.setState({ name: value })}
              />
            </div>
          </div>

          <div className="field radio white">
            <div className="name">
              <I18n path="modal.edit_command.colour" />
            </div>
            <div className="input">
              <ColourField
                colour={colour}
                onChange={(colour) => this.setState({ colour })}
              />
            </div>
          </div>

          <div className="field radio white">
            <div className="name">
              <I18n path="modal.edit_command.icon" />
            </div>
            <div className="input">
              <IconField
                icon={icon}
                icons={icons}
                onChange={(icon) => this.setState({ icon })}
              />
            </div>
          </div>

          <div className="field textarea white">
            <div className="name">
              <I18n path="modal.edit_command.url" />
            </div>
            <div className="input">
              <TextField
                name="url"
                value={url}
                onChange={(url) => this.setState({ url })}
              />
            </div>
          </div>

          <div className="field radio white">
            <div className="name">
              <I18n path="modal.edit_command.method.label" />
            </div>
            <div className="input">
              <label>
                <input
                  type="radio"
                  name="method"
                  value="GET"
                  checked={method === 'GET'}
                  onChange={(e) => this.setState({ method: e.target.value })}
                />
                <span className="label">
                  <I18n path="modal.edit_command.method.get" />
                </span>
              </label>
              <label>
                <input
                  type="radio"
                  name="method"
                  value="POST"
                  checked={method === 'POST'}
                  onChange={(e) => this.setState({ method: e.target.value })}
                />
                <span className="label">
                  <I18n path="modal.edit_command.method.post" />
                </span>
              </label>
            </div>
          </div>

          {method === 'POST' && (
          <div className="field textarea white">
            <div className="name">
              <I18n path="modal.edit_command.data" />
            </div>
            <div className="input">
              <textarea
                name="command"
                value={post_data}
                onChange={(e) => this.setState({ post_data: e.target.value })}
              />
            </div>
          </div>
          )}

          <div className="field textarea white">
            <div className="name">
              <I18n path="modal.edit_command.headers" />
            </div>
            <div className="input">
              <textarea
                name="headers"
                value={additional_headers}
                onChange={(e) => this.setState({ additional_headers: e.target.value })}
              />
            </div>
          </div>

          <div className="actions centered-text">
            {command && (
              <Button
                type="destructive"
                size="large"
                onClick={(e) => this.handleDelete(e)}
                tracking={{ category: 'EditCommand', action: 'Delete' }}
              >
                <I18n path="actions.delete" />
              </Button>
            )}
            <Button
              type="primary"
              size="large"
              submit
              tracking={{ category: 'EditCommand', action: 'Submit' }}
            >
              <I18n path="actions.save" />
            </Button>
          </div>

        </form>
      </Modal>
    );
  }
}

const mapStateToProps = (state, ownProps) => {
  const { id } = ownProps.match.params;
  return {
    id,
    command: (id && state.pusher.commands && state.pusher.commands[id] !== undefined ? state.pusher.commands[id] : null),
  };
};

const mapDispatchToProps = (dispatch) => ({
  pusherActions: bindActionCreators(pusherActions, dispatch),
  uiActions: bindActionCreators(uiActions, dispatch),
});

export default connect(mapStateToProps, mapDispatchToProps)(EditCommand);
