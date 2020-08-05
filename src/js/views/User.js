
import React from 'react';
import { connect } from 'react-redux';
import { bindActionCreators } from 'redux';
import ErrorMessage from '../components/ErrorMessage';
import Thumbnail from '../components/Thumbnail';
import PlaylistGrid from '../components/PlaylistGrid';
import FollowButton from '../components/Fields/FollowButton';
import LazyLoadListener from '../components/LazyLoadListener';
import Parallax from '../components/Parallax';
import NiceNumber, { nice_number } from '../components/NiceNumber';
import Loader from '../components/Loader';
import Icon from '../components/Icon';
import * as coreActions from '../services/core/actions';
import * as uiActions from '../services/ui/actions';
import * as spotifyActions from '../services/spotify/actions';
import {
  isLoading,
  getFromUri,
  sourceIcon,
} from '../util/helpers';
import { collate } from '../util/format';
import { i18n, I18n } from '../locale';

class User extends React.Component {
  componentDidMount() {
    const {
      uri,
      coreActions: {
        loadUser,
        loadUserPlaylists,
      },
    } = this.props;

    this.setWindowTitle();
    loadUser(uri);
    loadUserPlaylists(uri);
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
      load_queue,
      user: userProp,
      playlists,
      slim_mode,
    } = this.props;

    const user_id = getFromUri('userid', uri);

    if (!userProp) {
      if (isLoading(load_queue, [`spotify_users/${user_id}`, `spotify_users/${user_id}/playlists/?`])) {
        return <Loader body loading />
      }
      return (
        <ErrorMessage type="not-found" title="Not found">
          <p>
            <I18n path="errors.uri_not_found" uri={uri} />
          </p>
        </ErrorMessage>
      );
    }

    const user = collate(userProp, { playlists });
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
                  </div>
                  <h2>
                    <ul className="details">
                      {!slim_mode && (
                        <li className="source">
                          <Icon type="fontawesome" name={sourceIcon(user.uri)} />
                        </li>
                      )}
                      {user.playlists_total && (
                        <li>
                          <I18n path="specs.playlists" count={nice_number(user.playlists_total)} />
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
            <PlaylistGrid playlists={user.playlists} />
            <LazyLoadListener
              loadKey={user.playlists_more}
              showLoader={user.playlists_more}
              loadMore={this.loadMore}
            />
          </section>
        </div>
      </div>
    );
  }
}

const mapStateToProps = (state, ownProps) => {
  const uri = decodeURIComponent(ownProps.match.params.uri);
  return {
    uri,
    me: state.spotify.me,
    load_queue: state.ui.load_queue,
    spotify_authorized: state.spotify.authorization,
    playlists: state.core.playlists,
    user: (state.core.users[uri] !== undefined ? state.core.users[uri] : false),
  };
};

const mapDispatchToProps = (dispatch) => ({
  coreActions: bindActionCreators(coreActions, dispatch),
  uiActions: bindActionCreators(uiActions, dispatch),
  spotifyActions: bindActionCreators(spotifyActions, dispatch),
});

export default connect(mapStateToProps, mapDispatchToProps)(User);
