
import React from 'react';
import { connect } from 'react-redux';
import { bindActionCreators } from 'redux';
import PlaylistGrid from '../../components/PlaylistGrid';
import Header from '../../components/Header';
import Icon from '../../components/Icon';
import Loader from '../../components/Loader';
import * as uiActions from '../../services/ui/actions';
import * as mopidyActions from '../../services/mopidy/actions';
import * as spotifyActions from '../../services/spotify/actions';
import { isLoading } from '../../util/helpers';
import { i18n, I18n } from '../../locale';
import Button from '../../components/Button';
import { indexToArray } from '../../util/arrays';

class DiscoverFeatured extends React.Component {
  componentDidMount() {
    const {
      uris,
      uiActions: {
        setWindowTitle,
      },
      spotifyActions: {
        getFeaturedPlaylists,
      },
    } = this.props;

    setWindowTitle(i18n('discover.featured.title'));
    if (!uris) getFeaturedPlaylists();
  }

  onRefresh = () => {
    const {
      uiActions: {
        hideContextMenu,
      },
      spotifyActions: {
        getFeaturedPlaylists,
      },
    } = this.props;
    hideContextMenu();
    getFeaturedPlaylists(true);
  }

  handleContextMenu = (e, item) => {
    const {
      uiActions: {
        showContextMenu,
      },
    } = this.props;

    e.preventDefault();
    const data = {
      e,
      context: 'playlist',
      uris: [item.uri],
      items: [item],
    };
    showContextMenu(data);
  }

  render = () => {
    const {
      load_queue,
      uiActions,
      uris,
      items,
    } = this.props;

    if (isLoading(load_queue, ['(.*)spotify_browse/featured-playlists(.*)'])) {
      return (
        <div className="view discover-featured-view preserve-3d">
          <Header className="overlay" uiActions={uiActions}>
            <Icon name="star" type="material" />
            <I18n path="discover.featured.title" />
          </Header>
          <Loader body loading />
        </div>
      );
    }

    const playlists = indexToArray(items, uris || []);

    const options = (
      <Button
        noHover
        onClick={this.onRefresh}
        tracking={{ category: 'DiscoverFeatured', action: 'Refresh' }}
      >
        <Icon name="refresh" />
        <I18n path="actions.refresh" />
      </Button>
    );

    return (
      <div className="view discover-featured-view preserve-3d">
        <Header uiActions={uiActions} options={options}>
          <Icon name="star" type="material" />
          <I18n path="discover.featured.title" />
        </Header>
        <section className="content-wrapper grid-wrapper">
          {<PlaylistGrid playlists={playlists} />}
        </section>
      </div>
    );
  }
}

const mapStateToProps = (state) => ({
  theme: state.ui.theme,
  load_queue: state.ui.load_queue,
  uris: state.spotify && state.spotify.featured_playlists ? state.spotify.featured_playlists.uris : null,
  items: state.core.items,
});

const mapDispatchToProps = (dispatch) => ({
  uiActions: bindActionCreators(uiActions, dispatch),
  mopidyActions: bindActionCreators(mopidyActions, dispatch),
  spotifyActions: bindActionCreators(spotifyActions, dispatch),
});

export default connect(mapStateToProps, mapDispatchToProps)(DiscoverFeatured);
