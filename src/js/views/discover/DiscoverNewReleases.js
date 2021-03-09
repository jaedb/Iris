import React from 'react';
import { connect } from 'react-redux';
import { bindActionCreators } from 'redux';
import Header from '../../components/Header';
import Icon from '../../components/Icon';
import { Grid } from '../../components/Grid';
import Parallax from '../../components/Parallax';
import LazyLoadListener from '../../components/LazyLoadListener';
import Loader from '../../components/Loader';
import * as uiActions from '../../services/ui/actions';
import * as mopidyActions from '../../services/mopidy/actions';
import * as spotifyActions from '../../services/spotify/actions';
import { i18n, I18n } from '../../locale';
import Button from '../../components/Button';
import {
  makeItemSelector,
  makeLoadingSelector,
} from '../../util/selectors';

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
      more,
      spotifyActions: {
        getMore,
      },
    } = this.props;

    getMore(
      more,
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
    getNewReleases(true);
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
      loading,
      albums,
      more,
      uiActions,
    } = this.props;

    if (loading) {
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

    const options = (
      <Button
        noHover
        onClick={this.refresh}
        tracking={{ category: 'DiscoverFeatured', action: 'Refresh' }}
      >
        <Icon name="refresh" />
        <I18n path="actions.refresh" />
      </Button>
    );

    return (
      <div className="view discover-new-releases-view preserve-3d">
        <Header options={options} uiActions={uiActions}>
          <Icon name="new_releases" type="material" />
          <I18n path="discover.new_releases.title" />
        </Header>
        <section className="content-wrapper grid-wrapper">
          <Grid items={albums} />
        </section>
        <LazyLoadListener
          loadKey={more}
          showLoader={more}
          loadMore={this.loadMore}
        />
      </div>
    );
  }
}

const loadingSelector = makeLoadingSelector(['(.*)new-releases(.*)offset=0(.*)']);
const mapStateToProps = (state) => {
  const {
    ui: {
      theme,
    },
    spotify: {
      new_releases: uris,
      new_releases_more: more,
      new_releases_total: total,
    },
  } = state;
  const itemSelector = makeItemSelector(uris);

  return {
    uris,
    loading: loadingSelector(state),
    albums: itemSelector(state),
    more,
    total,
    theme,
  };
};

const mapDispatchToProps = (dispatch) => ({
  uiActions: bindActionCreators(uiActions, dispatch),
  mopidyActions: bindActionCreators(mopidyActions, dispatch),
  spotifyActions: bindActionCreators(spotifyActions, dispatch),
});

export default connect(mapStateToProps, mapDispatchToProps)(DiscoverNewReleases);
