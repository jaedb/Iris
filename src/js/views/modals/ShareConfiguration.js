
import React from 'react';
import { connect } from 'react-redux';
import { bindActionCreators } from 'redux';
import Modal from './Modal';
import * as uiActions from '../../services/ui/actions';
import * as pusherActions from '../../services/pusher/actions';
import { i18n, I18n } from '../../locale';

class ShareConfiguration extends React.Component {
  constructor(props) {
    super(props);

    this.state = {
      recipients: [],
      spotify: false,
      lastfm: false,
      genius: false,
      ui: false,
    };
  }

  componentDidMount() {
    this.props.uiActions.setWindowTitle(i18n('modal.share_configuration.title'));
  }

  toggleRecipient(id) {
    const { recipients } = this.state;
    if (recipients.includes(id)) {
      const index = recipients.indexOf(id);
      recipients.splice(index, 1);
    } else {
      recipients.push(id);
    }
    this.setState({ recipients });
  }

  handleSubmit(e) {
    e.preventDefault();

    const configuration = {};
    if (this.state.spotify) {
      configuration.spotify = {
        authorization: this.props.spotify_authorization,
        me: this.props.spotify_me,
      };
    }
    if (this.state.genius) {
      configuration.genius = {
        authorization: this.props.genius_authorization,
        me: this.props.genius_me,
      };
    }
    if (this.state.lastfm) {
      configuration.lastfm = {
        authorization: this.props.lastfm_authorization,
        me: this.props.lastfm_me,
      };
    }
    if (this.state.ui) {
      configuration.ui = this.props.ui;
    }
    if (this.state.snapcast) {
      configuration.snapcast = {
        enabled: this.props.snapcast.enabled,
        host: this.props.snapcast.host,
        port: this.props.snapcast.port,
      }
    }

    for (const recipient of this.state.recipients) {
      this.props.pusherActions.deliverMessage(
        recipient,
        'share_configuration_received',
        configuration,
      );
    }

    window.history.back();
  }

  render() {
    const connections = [];
    for (const connection_id in this.props.connections) {
      if (this.props.connections.hasOwnProperty(connection_id) && connection_id != this.props.connection_id) {
        connections.push(this.props.connections[connection_id]);
      }
    }

    if (connections.length > 0) {
      var recipients = (
        <div className="input checkbox-group">
          {
            connections.map((connection, index) => (
              <div key={connection.connection_id} className="checkbox-group__item">
                <label>
                  <input
                    type="checkbox"
                    name={`connection_${connection.connection_id}`}
                    checked={this.state.recipients.includes(connection.connection_id)}
                    onChange={(e) => this.toggleRecipient(connection.connection_id)}
                  />
                  <div className="label">
                    <div>
                      <div className="title">{connection.username}</div>
                      <div className="description mid_grey-text">
                        ({connection.ip})
                      </div>
                    </div>
                  </div>
                </label>
              </div>
            ))
          }
        </div>
      );
    } else {
      var recipients = (
        <div className="input text">
          <span className="mid_grey-text">
            <I18n path="modal.share_configuration.no_peers" />
          </span>
        </div>
      );
    }

    return (
      <Modal className="modal--share-configuration">

        <h1>
          <I18n path="modal.share_configuration.title" />
        </h1>
        <h2>
          <I18n path="modal.share_configuration.subtitle" />
        </h2>

        <form onSubmit={(e) => this.handleSubmit(e)}>
          <div className="field checkbox white">
            <div className="name">
              <I18n path="modal.share_configuration.recipients" />
            </div>
            {recipients}
          </div>

          <div className="field checkbox checkbox--block">
            <div className="name">
              <I18n path="modal.share_configuration.configurations" />
            </div>
            <div className="input">

              {this.props.spotify_me && this.props.spotify_authorization && (
                <div className="checkbox-group__item">
                  <label>
                    <input
                      type="checkbox"
                      name="spotify"
                      checked={this.state.spotify}
                      onChange={(e) => this.setState({ spotify: !this.state.spotify })}
                    />
                    <div className="label">
                      <div>
                        <div className="title">
                          <I18n
                            path="modal.share_configuration.authorization"
                            service={i18n('services.spotify.title')}
                          />
                        </div>
                        <div className="description mid_grey-text">
                          <I18n
                            path="modal.share_configuration.logged_in_as"
                            name={this.props.spotify_me.name}
                          />
                        </div>
                      </div>
                    </div>
                  </label>
                </div>
              )}

              {this.props.lastfm_me && this.props.lastfm_authorization && (
                <div className="checkbox-group__item">
                  <label>
                    <input
                      type="checkbox"
                      name="lastfm_authorization"
                      checked={this.state.lastfm}
                      onChange={(e) => this.setState({ lastfm: !this.state.lastfm })}
                    />
                    <div className="label">
                      <div>
                        <div className="title">
                          <I18n
                            path="modal.share_configuration.authorization"
                            service={i18n('services.lastfm.title')}
                          />
                        </div>
                        <div className="description mid_grey-text">
                          <I18n
                            path="modal.share_configuration.logged_in_as"
                            name={this.props.lastfm_me.name}
                          />
                        </div>
                      </div>
                    </div>
                  </label>
                </div>
              )}

              {this.props.genius_me && this.props.genius_authorization && (
                <div className="checkbox-group__item">
                  <label>
                    <input
                      type="checkbox"
                      name="genius_authorization"
                      checked={this.state.genius}
                      onChange={(e) => this.setState({ genius: !this.state.genius })}
                    />
                    <div className="label">
                      <div>
                        <div className="title">
                          <I18n
                            path="modal.share_configuration.authorization"
                            service={i18n('services.genius.title')}
                          />
                        </div>
                        <div className="description mid_grey-text">
                          <I18n
                            path="modal.share_configuration.logged_in_as"
                            name={this.props.genius_me.name}
                          />
                        </div>
                      </div>
                    </div>
                  </label>
                </div>
              )}

              <div className="checkbox-group__item">
                <label>
                  <input
                    type="checkbox"
                    name="snapcast"
                    checked={this.state.snapcast}
                    onChange={() => this.setState({ snapcast: !this.state.snapcast })}
                  />
                  <div className="label">
                    <div>
                      <div className="title">
                        <I18n path="services.snapcast.title" />
                      </div>
                      <div className="description mid_grey-text">
                        <I18n path="modal.share_configuration.snapcast_description" />
                      </div>
                    </div>
                  </div>
                </label>
              </div>

              <div className="checkbox-group__item">
                <label>
                  <input
                    type="checkbox"
                    name="interface"
                    checked={this.state.ui}
                    onChange={(e) => this.setState({ ui: !this.state.ui })}
                  />
                  <div className="label">
                    <div>
                      <div className="title">
                        <I18n path="modal.share_configuration.interface" />
                      </div>
                      <div className="description mid_grey-text">
                        <I18n path="modal.share_configuration.interface_description" />
                      </div>
                    </div>
                  </div>
                </label>
              </div>
            </div>
          </div>

          <div className="actions centered-text">
            {this.state.recipients.length > 0 ? (
              <button className="button button--primary button--large" onClick={(e) => this.handleSubmit(e)}>
                <I18n path="actions.send" />
              </button> 
            ) : (
              <button className="button button--primary button--large" disabled="disabled" onClick={(e) => this.handleSubmit(e)}>
                <I18n path="actions.send" />
              </button>
            )}
          </div>
        </form>
      </Modal>
    );
  }
}

const mapStateToProps = (state, ownProps) => ({
  spotify_authorization: state.spotify.authorization,
  spotify_me: state.spotify.me,
  genius_authorization: state.genius.authorization,
  genius_me: state.genius.me,
  lastfm_authorization: state.lastfm.authorization,
  lastfm_me: state.lastfm.me,
  ui: state.ui,
  snapcast: state.snapcast,
  connection_id: state.pusher.connection_id,
  connections: state.pusher.connections,
});

const mapDispatchToProps = (dispatch) => ({
  pusherActions: bindActionCreators(pusherActions, dispatch),
  uiActions: bindActionCreators(uiActions, dispatch),
});

export default connect(mapStateToProps, mapDispatchToProps)(ShareConfiguration);
