import React from 'react';
import { connect } from 'react-redux';
import { bindActionCreators } from 'redux';
import Header from '../../components/Header';
import Icon from '../../components/Icon';
import PlaylistGrid from '../../components/PlaylistGrid';
import Loader from '../../components/Loader';
import ErrorMessage from '../../components/ErrorMessage';
import Button from '../../components/Button';
import * as uiActions from '../../services/ui/actions';
import * as coreActions from '../../services/core/actions';
import * as spotifyActions from '../../services/spotify/actions';
import { I18n, i18n } from '../../locale';
import {
  makeItemSelector,
  makeLoadingSelector,
} from '../../util/selectors';

class DiscoverCategory extends React.Component {
  componentDidMount() {
    this.loadCategory();
    this.setWindowTitle();
  }

  componentDidUpdate = ({
    uri: prevUri,
    category: prevCategory,
  }) => {
    const {
      uri,
      category,
    } = this.props;

    if (prevUri !== uri) this.loadCategory();
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
      uri,
      category,
      spotifyActions: {
        getCategory,
      },
    } = this.props;

    if (!category) {
      getCategory(uri);
    }
  }

  refresh = () => {
    const {
      uri,
      uiActions: {
        hideContextMenu,
      },
      spotifyActions: {
        getCategory,
      },
    } = this.props;

    hideContextMenu();
    getCategory(uri, { forceRefetch: true });
  }

  render = () => {
    const {
      category,
      playlists,
      loading,
      uiActions,
      uri,
    } = this.props;

    if (loading) {
      return <Loader body loading />;
    }
    if (!category) {
      return (
        <ErrorMessage type="not-found" title="Not found">
          <p>
            <I18n path="errors.uri_not_found" uri={uri} />
          </p>
        </ErrorMessage>
      );
    }

    const options = (
      <Button
        noHover
        onClick={this.refresh}
        tracking={{ category: 'DiscoverCategory', action: 'Refresh' }}
      >
        <Icon name="refresh" />
        <I18n path="actions.refresh" />
      </Button>
    );

    return (
      <div className="view discover-categories-view">
        <Header uiActions={uiActions} options={options}>
          <Icon name="mood" type="material" />
          {category.name}
        </Header>
        <div className="content-wrapper">
          <section className="grid-wrapper">
            <PlaylistGrid playlists={playlists} />
          </section>
        </div>
      </div>
    );
  }
}

const mapStateToProps = (state, ownProps) => {
  const uri = decodeURIComponent(ownProps.match.params.uri);
  const loadingSelector = makeLoadingSelector([`spotify_category_${uri}`]);
  const categorySelector = makeItemSelector(uri);
  const category = categorySelector(state);
  let playlists = null;
  if (category && category.playlists_uris) {
    const playlistsSelector = makeItemSelector(category.playlists_uris);
    playlists = playlistsSelector(state);
  }

  return {
    uri,
    loading: loadingSelector(state),
    playlists,
    category,
  };
};

const mapDispatchToProps = (dispatch) => ({
  uiActions: bindActionCreators(uiActions, dispatch),
  coreActions: bindActionCreators(coreActions, dispatch),
  spotifyActions: bindActionCreators(spotifyActions, dispatch),
});

export default connect(mapStateToProps, mapDispatchToProps)(DiscoverCategory);
