
import React from 'react';
import { connect } from 'react-redux';
import { bindActionCreators } from 'redux';
import Header from '../../components/Header';
import Icon from '../../components/Icon';
import AlbumGrid from '../../components/AlbumGrid';
import Parallax from '../../components/Parallax';
import LazyLoadListener from '../../components/LazyLoadListener';
import Loader from '../../components/Loader';
import * as uiActions from '../../services/ui/actions';
import * as mopidyActions from '../../services/mopidy/actions';
import * as spotifyActions from '../../services/spotify/actions';
import { isLoading } from '../../util/helpers';
import { collate } from '../../util/format';

class DiscoverNewReleases extends React.Component {
  componentDidMount() {
    this.props.uiActions.setWindowTitle('New releases');

    if (!this.props.new_releases) {
      this.props.spotifyActions.getNewReleases();
    }
  }

  loadMore() {
    this.props.spotifyActions.getMore(
      this.props.new_releases_more,
      null,
      {
        type: 'SPOTIFY_NEW_RELEASES_LOADED',
      },
    );
  }

  playAlbum(e, album) {
    this.props.mopidyActions.playURIs([album.uri], album.uri);
  }

  handleContextMenu(e, item) {
    e.preventDefault();
    const data = {
      e,
      context: 'album',
      uris: [item.uri],
      items: [item],
    };
    this.props.uiActions.showContextMenu(data);
  }

  renderIntro = ({ images: { large } = {} } = {}) => (
    <div className="intro preserve-3d">
      <Parallax image={large} blur />
    </div>
  );

  render() {
    if (isLoading(this.props.load_queue, ['spotify_browse/new-releases'])) {
      return (
        <div className="view discover-new-releases-view">
          <Header>
            <Icon name="new_releases" type="material" />
						New releases
          </Header>
          <Loader body loading />
        </div>
      );
    }

    const albums = [];
    if (this.props.new_releases) {
      for (const uri of this.props.new_releases) {
        if (this.props.albums.hasOwnProperty(uri)) {
          albums.push(this.props.albums[uri]);
        }
      }
    }

    const options = (
      <a className="button button--no-hover" onClick={(e) => { this.props.uiActions.hideContextMenu(); this.props.spotifyActions.getNewReleases(); }}>
        <Icon name="refresh" />
Refresh
      </a>
    );

    return (
      <div className="view discover-new-releases-view preserve-3d">
        <Header options={options} uiActions={this.props.uiActions}>
          <Icon name="new_releases" type="material" />
					New releases
        </Header>
        <section className="content-wrapper grid-wrapper">
          <AlbumGrid albums={albums} />
        </section>
        <LazyLoadListener
          loadKey={this.props.new_releases_more}
          showLoader={this.props.new_releases_more}
          loadMore={() => this.loadMore()}
        />
      </div>
    );
  }
}

const mapStateToProps = (state, ownProps) => ({
  theme: state.ui.theme,
  load_queue: state.ui.load_queue,
  artists: state.core.artists,
  albums: state.core.albums,
  new_releases: (state.spotify.new_releases ? state.spotify.new_releases : null),
  new_releases_more: (state.spotify.new_releases_more ? state.spotify.new_releases_more : null),
  new_releases_total: (state.spotify.new_releases_total ? state.spotify.new_releases_total : null),
});

const mapDispatchToProps = (dispatch) => ({
  uiActions: bindActionCreators(uiActions, dispatch),
  mopidyActions: bindActionCreators(mopidyActions, dispatch),
  spotifyActions: bindActionCreators(spotifyActions, dispatch),
});

export default connect(mapStateToProps, mapDispatchToProps)(DiscoverNewReleases);
