
import React from 'react';
import { connect } from 'react-redux';
import { bindActionCreators } from 'redux';
import Modal from './Modal';
import * as coreActions from '../../services/core/actions';
import * as uiActions from '../../services/ui/actions';
import * as pusherActions from '../../services/pusher/actions';
import * as mopidyActions from '../../services/mopidy/actions';
import { queryString, isHosted } from '../../util/helpers';

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

    uiActions.setWindowTitle('Welcome to Iris');
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
        <h1>Get started</h1>
        <form onSubmit={(e) => this.handleSubmit(e)}>

          <div className="field text">
            <div className="name">
							Username
            </div>
            <div className="input">
              <input
                type="text"
                onChange={(e) => this.setState({ username: e.target.value.replace(/\W/g, '') })}
                value={this.state.username}
              />
              <div className="description">
								A non-unique string used to identify this client (no special characters)
              </div>
            </div>
          </div>

          <div className="field">
            <div className="name">Host</div>
            <div className="input">
              <input
                type="text"
                onChange={(e) => this.setState({ host: e.target.value })}
                value={this.state.host}
              />
            </div>
          </div>
          <div className="field">
            <div className="name">Port</div>
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
									Allow reporting of anonymous usage statistics
                  </span>
                </label>
                <p className="description">
This anonymous usage data is important in identifying errors and potential features that make Iris better for everyone. Want to know more? Read the
                  <a href="https://github.com/jaedb/Iris/wiki/Terms-of-use#privacy-policy" target="_blank">privacy policy</a>
                  {!this.state.allow_reporting ? <span className="red-text"> Are you sure you don't want to support this?</span> : null}
.
                  {' '}
                </p>
              </div>
            </div>
          )}

          <div className="actions centered-text">
            <button className={`button button--primary button--large${this.state.saving ? ' button--working' : ''}`} onClick={(e) => this.handleSubmit(e)}>
              {this.state.saving ? 'Saving' : 'Save'}
            </button>
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
