
import React from 'react';
import { connect } from 'react-redux';
import { createStore, bindActionCreators } from 'redux';
import Link from '../../components/Link';

import Modal from './Modal';
import Icon from '../../components/Icon';

import * as uiActions from '../../services/ui/actions';
import * as pusherActions from '../../services/pusher/actions';
import * as helpers from '../../helpers';

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
    this.props.uiActions.setWindowTitle('Share configuration');
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
        <div className="input">
          {
						connections.map((connection, index) => (
  <label key={connection.connection_id}>
    <input
      type="checkbox"
      name={`connection_${connection.connection_id}`}
      checked={this.state.recipients.includes(connection.connection_id)}
      onChange={(e) => this.toggleRecipient(connection.connection_id)}
    />
    <span className="label">
      <div className="title">{ connection.username }</div>
      <div className="description mid_grey-text">
											(
        { connection.ip }
)
      </div>
    </span>
  </label>
						))
					}
        </div>
      );
    } else {
      var recipients = (
        <div className="input text">
          <span className="mid_grey-text">
						No peer connections
          </span>
        </div>
      );
    }

    return (
      <Modal className="modal--share-configuration">

        <h1>Share configuration</h1>
        <h2>Push your authorizations and interface settings to another, connected user</h2>

        <form onSubmit={(e) => this.handleSubmit(e)}>
          <div className="field checkbox white">
            <div className="name">Recipients</div>
            {recipients}
          </div>

          <div className="field checkbox checkbox--block">
            <div className="name">Configurations</div>
            <div className="input">
              <label>
                <input
                  type="checkbox"
                  name="interface"
                  checked={this.state.ui}
                  onChange={(e) => this.setState({ ui: !this.state.ui })}
                />
                <span className="label">
                  <span className="title">UI customisation (theme, sorting, filters)</span>
                </span>
              </label>

              {this.props.spotify_me && this.props.spotify_authorization ? (
                <label>
                  <input
                    type="checkbox"
                    name="spotify"
                    checked={this.state.spotify}
                    onChange={(e) => this.setState({ spotify: !this.state.spotify })}
                  />
                  <span className="label">
                    <div className="title">Spotify authorization</div>
                    <div className="description mid_grey-text">
Logged in as
                      {this.props.spotify_me.name}
                    </div>
                  </span>
                </label>
              ) : null}

              {this.props.lastfm_me && this.props.lastfm_authorization ? (
                <label>
                  <input
                    type="checkbox"
                    name="lastfm_authorization"
                    checked={this.state.lastfm}
                    onChange={(e) => this.setState({ lastfm: !this.state.lastfm })}
                  />
                  <span className="label">
                    <div className="title">LastFM authorization</div>
                    <div className="description mid_grey-text">
Logged in as
                      {this.props.lastfm_me.name}
                    </div>
                  </span>
                </label>
              ) : null}

              {this.props.genius_me && this.props.genius_authorization ? (
                <label>
                  <input
                    type="checkbox"
                    name="genius_authorization"
                    checked={this.state.genius}
                    onChange={(e) => this.setState({ genius: !this.state.genius })}
                  />
                  <span className="label">
                    <div className="title">Genius authorization</div>
                    <div className="description mid_grey-text">
Logged in as
                      {this.props.genius_me.name}
                    </div>
                  </span>
                </label>
              ) : null}
            </div>
          </div>

          <div className="actions centered-text">
            {this.state.recipients.length > 0 ? <button className="button button--primary button--large" onClick={(e) => this.handleSubmit(e)}>Send</button> : <button className="button button--primary button--large" disabled="disabled" onClick={(e) => this.handleSubmit(e)}>Send</button>}
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
  connection_id: state.pusher.connection_id,
  connections: state.pusher.connections,
});

const mapDispatchToProps = (dispatch) => ({
  pusherActions: bindActionCreators(pusherActions, dispatch),
  uiActions: bindActionCreators(uiActions, dispatch),
});

export default connect(mapStateToProps, mapDispatchToProps)(ShareConfiguration);
