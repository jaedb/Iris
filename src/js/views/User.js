
import React from 'react';
import { connect } from 'react-redux';
import { bindActionCreators } from 'redux';
import ErrorMessage from '../components/ErrorMessage';
import Thumbnail from '../components/Thumbnail';
import PlaylistGrid from '../components/PlaylistGrid';
import FollowButton from '../components/Fields/FollowButton';
import LazyLoadListener from '../components/LazyLoadListener';
import Parallax from '../components/Parallax';
import NiceNumber from '../components/NiceNumber';
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

class User extends React.Component {
  componentDidMount() {
    this.setWindowTitle();
    this.props.coreActions.loadUser(this.props.uri);
    this.props.coreActions.loadUserPlaylists(this.props.uri);
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

  setWindowTitle(user = this.props.user) {
    if (user) {
      this.props.uiActions.setWindowTitle(`${user.name} (user)`);
    } else {
      this.props.uiActions.setWindowTitle('User');
    }
  }

  loadMore() {
    this.props.spotifyActions.getMore(
      this.props.user.playlists_more,
      {
        parent_type: 'user',
        parent_key: this.props.uri,
        records_type: 'playlist',
      },
    );
  }

  isMe() {
    const userid = getFromUri('userid', this.props.uri);
    return (this.props.me && this.props.me.id && this.props.me.id == userid);
  }

  render() {
    const user_id = getFromUri('userid', this.props.uri);

    if (!this.props.user) {
      if (isLoading(this.props.load_queue, [`spotify_users/${user_id}`, `spotify_users/${user_id}/playlists/?`])) {
        return <Loader body loading />
      }
      return (
        <ErrorMessage type="not-found" title="Not found">
          <p>
Could not find user with URI "
            {encodeURIComponent(this.props.uri)}
"
          </p>
        </ErrorMessage>
      );
    }

    const user = collate(this.props.user, { playlists: this.props.playlists });

    if (user && user.images) {
      var image = user.images.huge;
    } else {
      var image = null;
    }

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
                    <FollowButton className="primary" uri={user.uri} addText="Follow" removeText="Unfollow" />
                  </div>
                  <h2>
                    <ul className="details">
                  {!this.props.slim_mode ? <li className="source"><Icon type="fontawesome" name={sourceIcon(user.uri)} /></li> : null}
                  {user.playlists_total ? (
                  <li>
                  <NiceNumber value={user.playlists_total} />
                  {' '}
playlists
                </li>
                ) : null}
                  {user.followers ? (
                  <li>
                  <NiceNumber value={user.followers} />
                  {' '}
followers
                </li>
                ) : null}
                  {this.isMe() ? <li><span className="blue-text">You</span></li> : null}
                </ul>
                  </h2>
                </div>
              </div>
            </div>
          </div>
        </div>

        <div className="content-wrapper">
          <section className="grid-wrapper">
            <h4>Playlists</h4>
            <PlaylistGrid playlists={user.playlists} />
            <LazyLoadListener
              loadKey={user.playlists_more}
              showLoader={user.playlists_more}
              loadMore={() => this.loadMore()}
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
