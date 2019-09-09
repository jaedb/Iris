
import React from 'react';
import { connect } from 'react-redux';
import { bindActionCreators } from 'redux';
import Link from '../../components/Link';

import PlaylistGrid from '../../components/PlaylistGrid';
import Header from '../../components/Header';
import Icon from '../../components/Icon';
import Parallax from '../../components/Parallax';
import Loader from '../../components/Loader';

import * as helpers from '../../helpers';
import * as uiActions from '../../services/ui/actions';
import * as mopidyActions from '../../services/mopidy/actions';
import * as spotifyActions from '../../services/spotify/actions';

class DiscoverFeatured extends React.Component {
  constructor(props) {
    super(props);
  }

  componentDidMount() {
    this.props.uiActions.setWindowTitle('Featured playlists');
    if (!this.props.featured_playlists) {
      this.props.spotifyActions.getFeaturedPlaylists();
    }
  }

  playPlaylist(e, playlist) {
    this.props.mopidyActions.playPlaylist(playlist.uri);
  }

  handleContextMenu(e, item) {
    e.preventDefault();
    const data = {
      e,
      context: 'playlist',
      uris: [item.uri],
      items: [item],
    };
    this.props.uiActions.showContextMenu(data);
  }

  renderIntro(playlist = null) {
    if (playlist) {
      return (
        <div className="intro preserve-3d">
          <Parallax image={playlist.images ? playlist.images.large : null} blur />
        </div>
      );
    }
    return (
      <div className="intro">
        <Parallax disabled={this.props.disable_parallax} />
      </div>
    );
  }

  render() {
    if (helpers.isLoading(this.props.load_queue, ['spotify_browse/featured-playlists'])) {
      return (
        <div className="view discover-featured-view preserve-3d">
          <Header className="overlay" uiActions={this.props.uiActions}>
            <Icon name="star" type="material" />
						Featured playlists
          </Header>
          <Loader body loading />
        </div>
      );
    }

    const playlists = [];
    if (this.props.featured_playlists) {
      for (let i = 0; i < this.props.featured_playlists.playlists.length; i++) {
        const uri = this.props.featured_playlists.playlists[i];
        if (this.props.playlists.hasOwnProperty(uri)) {
          playlists.push(this.props.playlists[uri]);
        }
      }
    }

    // Pull the first playlist out and we'll use this for the parallax artwork
    const first_playlist = playlists[0];

    const options = (
      <a className="button button--no-hover" onClick={(e) => { this.props.uiActions.hideContextMenu(); this.props.spotifyActions.getFeaturedPlaylists(); }}>
        <Icon name="refresh" />
Refresh
      </a>
    );

    return (
      <div className="view discover-featured-view preserve-3d">
        <Header className="overlay" options={options}>
          <Icon name="star" type="material" />
					Featured playlists
        </Header>
        {this.renderIntro(first_playlist)}
        <section className="content-wrapper grid-wrapper">
          {playlists ? <PlaylistGrid playlists={playlists} /> : null }
        </section>
      </div>
    );
  }
}

const mapStateToProps = (state, ownProps) => ({
  theme: state.ui.theme,
  load_queue: state.ui.load_queue,
  featured_playlists: state.spotify.featured_playlists,
  playlists: state.core.playlists,
});

const mapDispatchToProps = (dispatch) => ({
  uiActions: bindActionCreators(uiActions, dispatch),
  mopidyActions: bindActionCreators(mopidyActions, dispatch),
  spotifyActions: bindActionCreators(spotifyActions, dispatch),
});

export default connect(mapStateToProps, mapDispatchToProps)(DiscoverFeatured);
