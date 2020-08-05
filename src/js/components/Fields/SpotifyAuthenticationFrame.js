
import React from 'react';
import { connect } from 'react-redux';
import { bindActionCreators } from 'redux';
import * as uiActions from '../../services/ui/actions';
import * as spotifyActions from '../../services/spotify/actions';
import { toJSON } from '../../util/format';
import { I18n, i18n } from '../../locale';

class SpotifyAuthenticationFrame extends React.Component {
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
    if (data.origin != 'auth_spotify') {
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

    // Spotify bounced with an error
    if (data.error !== undefined) {
      this.props.uiActions.createNotification({ content: data.error, level: 'error' });

      // No errors? We're in!
    } else {
      this.props.spotifyActions.authorizationGranted(data);
      this.props.spotifyActions.getMe();
    }

    // Turn off our authorizing switch
    this.setState({ authorizing: false });
  }

  startAuthorization() {
    const self = this;
    this.setState({ authorizing: true });

    // Open an authentication request window (to spotify)
    const url = `${this.props.authorization_url}?action=authorize`;
    const scopes = [
      'playlist-modify-private',
      'playlist-modify-public',
      'playlist-read-private',
      'playlist-modify-private',
      'user-library-read',
      'user-library-modify',
      'user-follow-modify',
      'user-follow-read',
      'user-read-email',
      'user-top-read',
      'user-read-currently-playing',
      'user-read-playback-state',
      'playlist-read-collaborative',
      'ugc-image-upload', // playlist image uploading
    ];
    const popup = window.open(`${url}&scope=${scopes.join('%20')}`, 'popup', 'height=580,width=350');

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
    const { authorized } = this.props;

    if (authorized) {
      return (
        <a className={`button button--destructive ${authorizing ? 'button--working' :''}`} onClick={(e) => this.props.spotifyActions.revokeAuthorization()}>
          <I18n path="authentication.log_out" />
        </a>
      );
    }
    return (
      <a className={`button button--primary ${authorizing ? 'button--working' :''}`} onClick={(e) => this.startAuthorization()}>
        <I18n path="authentication.log_in" />
      </a>
    );
  }
}

const mapStateToProps = (state, ownProps) => ({
  authorization_url: state.spotify.authorization_url,
  authorized: state.spotify.authorization,
  authorizing: state.spotify.authorizing,
});

const mapDispatchToProps = (dispatch) => ({
  uiActions: bindActionCreators(uiActions, dispatch),
  spotifyActions: bindActionCreators(spotifyActions, dispatch),
});

export default connect(mapStateToProps, mapDispatchToProps)(SpotifyAuthenticationFrame);
