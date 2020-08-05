
import React from 'react';
import { connect } from 'react-redux';
import { bindActionCreators } from 'redux';
import Header from '../../components/Header';
import Icon from '../../components/Icon';
import PlaylistGrid from '../../components/PlaylistGrid';
import LazyLoadListener from '../../components/LazyLoadListener';
import Loader from '../../components/Loader';
import * as uiActions from '../../services/ui/actions';
import * as spotifyActions from '../../services/spotify/actions';
import { isLoading } from '../../util/helpers';
import { collate } from '../../util/format';
import { i18n } from '../../locale';

class DiscoverCategory extends React.Component {
  componentDidMount() {
    this.loadCategory();
    this.setWindowTitle();
  }

  componentDidUpdate = ({
    match: {
      params: {
        id: prevId,
      },
    },
    category: prevCategory,
  }) => {
    const {
      match: {
        params: {
          id,
        },
      },
      category,
    } = this.props;

    if (prevId !== id) this.loadCategory();
    if (!prevCategory && category) this.setWindowTitle(category);
  }

  setWindowTitle = (category = this.props.category) => {
    const {
      uiActions: { setWindowTitle },
    } = this.props;

    if (category) {
      setWindowTitle(category.name);
    } else {
      setWindowTitle(i18n('discover.category.title'));
    }
  }

  loadCategory = () => {
    const {
      category,
      match: {
        params: { id },
      },
      spotifyActions: {
        getCategory,
        getCategoryPlaylists,
      },
    } = this.props;

    if (!category) {
      getCategory(id);
    }

    if (!category.playlists_uris) {
      getCategoryPlaylists(id);
    }
  }

  loadMore = () => {
    const {
      spotifyActions: {
        getMore,
      },
      category: {
        playlists_more,
      },
      match: {
        params: {
          id,
        },
      },
    } = this.props;

    getMore(
      playlists_more,
      null,
      {
        type: 'SPOTIFY_CATEGORY_PLAYLISTS_LOADED_MORE',
        uri: `category:${id}`,
      },
    );
  }

  render = () => {
    const {
      category: categoryProp,
      playlists,
      load_queue,
      uiActions,
    } = this.props;

    if (isLoading(load_queue, ['spotify_browse/categories/'])) {
      return (
        <div className="view discover-categories-view">
          <Header>
            <Icon name="mood" type="material" />
            {(categoryProp ? categoryProp.name : i18n('discover.category.category'))}
          </Header>
          <Loader body loading />
        </div>
      );
    }

    if (!categoryProp) {
      return null;
    }

    const category = collate(categoryProp, { playlists });

    return (
      <div className="view discover-categories-view">
        <Header uiActions={uiActions}>
          <Icon name="mood" type="material" />
          {category.name}
        </Header>
        <div className="content-wrapper">
          <section className="grid-wrapper">
            <PlaylistGrid playlists={category.playlists} />
          </section>
          <LazyLoadListener
            loadKey={category.playlists_more}
            showLoader={category.playlists_more}
            loadMore={this.loadMore}
          />
        </div>
      </div>
    );
  }
}

const mapStateToProps = (state, ownProps) => ({
  load_queue: state.ui.load_queue,
  playlists: state.core.playlists,
  category: (state.spotify.categories && state.spotify.categories[`category:${ownProps.match.params.id}`] !== undefined ? state.spotify.categories[`category:${ownProps.match.params.id}`] : false),
});

const mapDispatchToProps = (dispatch) => ({
  uiActions: bindActionCreators(uiActions, dispatch),
  spotifyActions: bindActionCreators(spotifyActions, dispatch),
});

export default connect(mapStateToProps, mapDispatchToProps)(DiscoverCategory);
