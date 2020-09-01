
import React from 'react';
import { connect } from 'react-redux';
import { bindActionCreators } from 'redux';
import Modal from './Modal';
import * as coreActions from '../../services/core/actions';
import * as uiActions from '../../services/ui/actions';
import * as pusherActions from '../../services/pusher/actions';
import * as mopidyActions from '../../services/mopidy/actions';
import { queryString, isHosted } from '../../util/helpers';
import { I18n, i18n } from '../../locale';
import Button from '../../components/Button';

class InitialSetup extends React.Component {
  constructor(props) {
    super(props);

    const {
      host,
      port,
      username,
      allow_reporting,
    } = props;

    this.state = {
      username,
      allow_reporting,
      host,
      port,
    };
  }

  componentDidMount() {
    const {
      location: {
        search,
      },
      uiActions,
    } = this.props;

    // Check for url-parsed configuration values
    const customHost = queryString('host', search);
    const customPort = queryString('port', search);
    if (customHost) this.setState({ host: customHost });
    if (customPort) this.setState({ port: customPort });

    uiActions.setWindowTitle(i18n('modal.initial_setup.title'));
  }

  handleSubmit(e) {
    e.preventDefault();
    const self = this;

    // Only if we've changed the username do we set it
    if (this.props.username !== this.state.username) {
      this.props.pusherActions.setUsername(this.state.username);
    }

    this.props.uiActions.set({
      initial_setup_complete: true,
      allow_reporting: this.state.allow_reporting,
    });

    this.props.mopidyActions.set({
      host: this.state.host,
      port: this.state.port,
    });

    this.setState({ saving: true });

    // Wait a jiffy to allow changes to apply to store
    setTimeout(() => {
      // We've changed a connection setting, so need to reload
      if (self.state.host !== self.props.host || self.state.port !== self.props.port) {
        window.location = '/';

        // Safe to just close modal
      } else {
        self.props.history.push('/');
      }
    },
    200);

    return false;
  }

  render() {
    return (
      <Modal className="modal--initial-setup" noclose>
        <h1>
          <I18n path="modal.initial_setup.title" />
        </h1>
        <form onSubmit={(e) => this.handleSubmit(e)}>

          <div className="field text">
            <div className="name">
							<I18n path="settings.server.username.label" />
            </div>
            <div className="input">
              <input
                type="text"
                onChange={(e) => this.setState({ username: e.target.value.replace(/\W/g, '') })}
                value={this.state.username}
              />
              <div className="description">
                <I18n path="settings.server.username.description" />
              </div>
            </div>
          </div>

          <div className="field">
            <div className="name">
              <I18n path="settings.servers.host" />
            </div>
            <div className="input">
              <input
                type="text"
                onChange={(e) => this.setState({ host: e.target.value })}
                value={this.state.host}
              />
            </div>
          </div>
          <div className="field">
            <div className="name">
              <I18n path="settings.servers.port" />
            </div>
            <div className="input">
              <input
                type="text"
                onChange={(e) => this.setState({ port: e.target.value })}
                value={this.state.port}
              />
            </div>
          </div>

          {isHosted() ? null : (
            <div className="field checkbox">
              <div className="input">
                <label>
                  <input
                    type="checkbox"
                    name="allow_reporting"
                    checked={this.state.allow_reporting}
                    onChange={(e) => this.setState({ allow_reporting: !this.state.allow_reporting })}
                  />
                  <span className="label">
                    <I18n path="settings.interface.reporting.sublabel" />
                  </span>
                </label>
                <p className="description">
                  <I18n path="settings.interface.reporting.description" />
                  <a href="https://github.com/jaedb/Iris/wiki/Terms-of-use#privacy-policy" target="_blank">
                    <I18n path="settings.interface.reporting.privacy_policy" />
                  </a>.
                  {!this.state.allow_reporting && (
                    <span className="red-text">
                      <br />
                      <I18n path="settings.interface.reporting.are_you_sure" />
                    </span>
                  )}
                </p>
              </div>
            </div>
          )}

          <div className="actions centered-text">
            <Button
              type="primary"
              size="large"
              working={this.state.saving}
              onClick={(e) => this.handleSubmit(e)}
              tracking={{ category: 'InitialSetup', action: 'Submit' }}
            >
              <I18n path="actions.save" />
            </Button>
          </div>

        </form>
      </Modal>
    );
  }
}

const mapStateToProps = (state, ownProps) => ({
  allow_reporting: state.ui.allow_reporting,
  username: (state.pusher && state.pusher.username ? state.pusher.username : null),
  host: state.mopidy.host,
  port: state.mopidy.port,
});

const mapDispatchToProps = (dispatch) => ({
  coreActions: bindActionCreators(coreActions, dispatch),
  uiActions: bindActionCreators(uiActions, dispatch),
  pusherActions: bindActionCreators(pusherActions, dispatch),
  mopidyActions: bindActionCreators(mopidyActions, dispatch),
});

export default connect(mapStateToProps, mapDispatchToProps)(InitialSetup);
