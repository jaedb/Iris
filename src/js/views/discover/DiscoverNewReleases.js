
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
import { i18n, I18n } from '../../locale';

class DiscoverNewReleases extends React.Component {
  componentDidMount() {
    const {
      new_releases,
      uiActions: {
        setWindowTitle,
      },
      spotifyActions: {
        getNewReleases,
      },
    } = this.props;

    setWindowTitle(i18n('discover.new_releases.title'));

    if (!new_releases) {
      getNewReleases();
    }
  }

  loadMore = () => {
    const {
      new_releases_more,
      spotifyActions: {
        getMore,
      },
    } = this.props;

    getMore(
      new_releases_more,
      null,
      {
        type: 'SPOTIFY_NEW_RELEASES_LOADED',
      },
    );
  }

  playAlbum = (album) => {
    const { mopidyActions: { playURIs } } = this.props;

    playURIs([album.uri], album.uri);
  }

  refresh = () => {
    const {
      uiActions: {
        hideContextMenu,
      },
      spotifyActions: {
        getNewReleases,
      },
    } = this.props;

    hideContextMenu();
    getNewReleases();
  }

  handleContextMenu(e, item) {
    const { uriActions: { showContextMenu } } = this.props;

    e.preventDefault();
    showContextMenu({
      e,
      context: 'album',
      uris: [item.uri],
      items: [item],
    });
  }

  renderIntro = ({ images: { large } = {} } = {}) => (
    <div className="intro preserve-3d">
      <Parallax image={large} blur />
    </div>
  );

  render = () => {
    const {
      load_queue,
      new_releases,
      albums: albumsProp,
      new_releases_more,
      uiActions,
    } = this.props;

    if (isLoading(load_queue, ['spotify_browse/new-releases'])) {
      return (
        <div className="view discover-new-releases-view">
          <Header>
            <Icon name="new_releases" type="material" />
            <I18n path="discover.new_releases.title" />
          </Header>
          <Loader body loading />
        </div>
      );
    }

    const albums = [];
    if (new_releases) {
      for (const uri of new_releases) {
        if (albumsProp.hasOwnProperty(uri)) {
          albums.push(albumsProp[uri]);
        }
      }
    }

    const options = (
      <a className="button button--no-hover" onClick={this.refresh}>
        <Icon name="refresh" />
        <I18n path="actions.refresh" />
      </a>
    );

    return (
      <div className="view discover-new-releases-view preserve-3d">
        <Header options={options} uiActions={uiActions}>
          <Icon name="new_releases" type="material" />
          <I18n path="discover.new_releases.title" />
        </Header>
        <section className="content-wrapper grid-wrapper">
          <AlbumGrid albums={albums} />
        </section>
        <LazyLoadListener
          loadKey={new_releases_more}
          showLoader={new_releases_more}
          loadMore={this.loadMore}
        />
      </div>
    );
  }
}

const mapStateToProps = (state) => ({
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
