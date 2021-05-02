import React from 'react';
import { connect } from 'react-redux';
import { bindActionCreators } from 'redux';
import ErrorMessage from '../components/ErrorMessage';
import Thumbnail from '../components/Thumbnail';
import { Grid } from '../components/Grid';
import FollowButton from '../components/Fields/FollowButton';
import Parallax from '../components/Parallax';
import { nice_number } from '../components/NiceNumber';
import Loader from '../components/Loader';
import { SourceIcon } from '../components/Icon';
import * as coreActions from '../services/core/actions';
import * as uiActions from '../services/ui/actions';
import * as spotifyActions from '../services/spotify/actions';
import {
  getFromUri,
} from '../util/helpers';
import { decodeUri } from '../util/format';
import { i18n, I18n } from '../locale';
import {
  makeItemSelector,
  makeLoadingSelector,
} from '../util/selectors';
import ContextMenuTrigger from '../components/ContextMenuTrigger';

class User extends React.Component {
  componentDidMount() {
    const {
      uri,
      coreActions: {
        loadUser,
      },
    } = this.props;

    this.setWindowTitle();
    loadUser(uri, { full: true });
  }

  componentDidUpdate = ({
    uri: prevUri,
    user: prevUser,
  }) => {
    const {
      uri,
      user,
      coreActions: {
        loadUser,
        loadUserPlaylists,
      },
    } = this.props;

    if (prevUri !== uri) {
      loadUser(uri);
      loadUserPlaylists(uri);
    }

    if (!prevUser && user) this.setWindowTitle(user);
  }

  handleContextMenu = (e) => {
    const { user, uri, uiActions: { showContextMenu } } = this.props;

    showContextMenu({
      e,
      context: 'user',
      items: [user],
      uris: [uri],
    });
  }

  setWindowTitle = (user = this.props.user) => {
    const {
      uiActions: { setWindowTitle },
    } = this.props;
    if (user) {
      setWindowTitle(i18n('user.title_window', { name: user.name }));
    } else {
      setWindowTitle(i18n('user.title'));
    }
  }

  loadMore = () => {
    const {
      uri,
      spotifyActions: { getMore },
      user: { playlists_more } = {},
    } = this.props;

    getMore(
      playlists_more,
      {
        parent_type: 'user',
        parent_key: uri,
        records_type: 'playlist',
      },
    );
  }

  isMe = () => {
    const {
      uri,
      me: { id } = {},
    } = this.props;

    return (id && id === getFromUri('userid', uri));
  }

  render = () => {
    const {
      uri,
      loading,
      user,
      playlists,
      slim_mode,
    } = this.props;

    if (loading) {
      return <Loader body loading />;
    }
    if (!user) {
      return (
        <ErrorMessage type="not-found" title="Not found">
          <p>
            <I18n path="errors.uri_not_found" uri={uri} />
          </p>
        </ErrorMessage>
      );
    }

    const image = user.images ? user.images.huge : null;

    return (
      <div className="view user-view preserve-3d">
        <div className="intro preserve-3d">

          <Parallax image={image} blur />

          <div className="liner">
            <div className="heading">

              <div className="heading__thumbnail">
                <Thumbnail size="medium" circle canZoom image={image} type="user" />
              </div>

              <div className="heading__content">
                <h1>{user.name}</h1>
                <div className="heading__content__details">
                  <div className="actions">
                    <FollowButton
                      className="primary"
                      uri={user.uri}
                    />
                    <ContextMenuTrigger onTrigger={this.handleContextMenu} />
                  </div>
                  <h2>
                    <ul className="details details--one-line">
                      {!slim_mode && (
                        <li className="source">
                          <SourceIcon uri={user.uri} />
                        </li>
                      )}
                      {playlists && (
                        <li>
                          <I18n path="specs.playlists" count={nice_number(playlists.length)} />
                        </li>
                      )}
                      {user.followers && (
                      <li>
                        <I18n path="specs.followers" count={nice_number(user.followers)} />
                      </li>
                      )}
                      {this.isMe() && (
                        <li>
                          <span className="blue-text">
                            <I18n path="user.you" />
                          </span>
                        </li>
                      )}
                    </ul>
                  </h2>
                </div>
              </div>
            </div>
          </div>
        </div>

        <div className="content-wrapper">
          <section className="grid-wrapper">
            <h4>
              <I18n path="playlist.title_plural" />
            </h4>
            <Grid items={playlists} />
          </section>
        </div>
      </div>
    );
  }
}

const mapStateToProps = (state, ownProps) => {
  const uri = decodeUri(ownProps.match.params.uri);
  const loadingSelector = makeLoadingSelector([`(.*)${uri}(.*)`]);
  const userSelector = makeItemSelector(uri);
  const user = userSelector(state);
  let playlists = null;
  if (user && user.playlists_uris) {
    const playlistsSelector = makeItemSelector(user.playlists_uris);
    playlists = playlistsSelector(state);
  }
  return {
    uri,
    me: state.spotify.me,
    spotify_authorized: state.spotify.authorization,
    user,
    playlists,
    loading: loadingSelector(state),
  };
};

const mapDispatchToProps = (dispatch) => ({
  coreActions: bindActionCreators(coreActions, dispatch),
  uiActions: bindActionCreators(uiActions, dispatch),
  spotifyActions: bindActionCreators(spotifyActions, dispatch),
});

export default connect(mapStateToProps, mapDispatchToProps)(User);
