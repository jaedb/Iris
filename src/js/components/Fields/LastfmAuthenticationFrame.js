
import React from 'react';
import { connect } from 'react-redux';
import { bindActionCreators } from 'redux';
import * as uiActions from '../../services/ui/actions';
import * as lastfmActions from '../../services/lastfm/actions';
import { toJSON } from '../../util/format';
import { I18n, i18n } from '../../locale';
import Button from '../Button';

class LastfmAuthenticationFrame extends React.Component {
  constructor(props) {
    super(props);

    this.state = {
      authorizing: false,
    };

    this.handleMessage = this.handleMessage.bind(this);
  }

  componentDidMount() {
    window.addEventListener('message', this.handleMessage, false);
  }

  componentWillUnmount() {
    window.removeEventListener('message', this.handleMessage, false);
  }

  handleMessage(event) {
    const data = toJSON(event.data);

    // Only digest messages relevant to us
    if (data.origin != 'auth_lastfm') {
      return;
    }

    // Only allow incoming data from our authorized authenticator proxy
    const authorization_domain = this.props.authorization_url.substring(0, this.props.authorization_url.indexOf('/', 8));
    if (event.origin != authorization_domain) {
      this.props.uiActions.createNotification({
        content: i18n('authentication.failed', { origin: event.origin }),
        level: 'error',
      });
      return false;
    }

    // Bounced with an error
    if (data.error !== undefined) {
      this.props.uiActions.createNotification({ content: data.message, level: 'error' });

      // No errors? We're in!
    } else {
      this.props.lastfmActions.authorizationGranted(data);
      this.props.lastfmActions.getMe();
    }

    // Turn off our authorizing switch
    this.setState({ authorizing: false });
  }

  startAuthorization() {
    const self = this;
    this.setState({ authorizing: true });

    // Open an authentication request window
    const url = `${this.props.authorization_url}?action=authorize`;
    const popup = window.open(url, 'popup', 'height=580,width=350');
    popup.name = 'LastfmAuthenticationWindow';

    // Start timer to check our popup's state
    const timer = setInterval(checkPopup, 1000);
    function checkPopup() {
        	// Popup has been closed
      if (typeof (popup) !== 'undefined' && popup) {
        if (popup.closed) {
          self.setState({ authorizing: false });
          clearInterval(timer);
        }

        // Popup does not exist, so must have been blocked
      } else {
        self.props.uiActions.createNotification({
          content: i18n('authentication.popup_blocked'),
          level: 'error',
        });
        self.setState({ authorizing: false });
        clearInterval(timer);
      }
    }
  }

  render() {
    const { authorizing } = this.state;

    if (this.props.authorization) {
      return (
        <Button
          type="destructive"
          working={authorizing}
          onClick={() => this.props.lastfmActions.revokeAuthorization()}
          tracking={{ category: 'Lastfm', label: 'Logout' }}
        >
          <I18n path="authentication.log_out" />
        </Button>
      );
    }
    return (
      <Button
        type="primary"
        working={authorizing}
        onClick={() => this.startAuthorization()}
        tracking={{ category: 'Lastfm', label: 'Login' }}
      >
        <I18n path="authentication.log_in" />
      </Button>
    );
  }
}

const mapStateToProps = (state, ownProps) => ({
  authorization_url: state.lastfm.authorization_url,
  authorization: state.lastfm.authorization,
  authorizing: state.lastfm.authorizing,
});

const mapDispatchToProps = (dispatch) => ({
  uiActions: bindActionCreators(uiActions, dispatch),
  lastfmActions: bindActionCreators(lastfmActions, dispatch),
});

export default connect(mapStateToProps, mapDispatchToProps)(LastfmAuthenticationFrame);
