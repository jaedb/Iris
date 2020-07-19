
import React from 'react';
import { connect } from 'react-redux';
import { bindActionCreators } from 'redux';
import Header from '../../components/Header';
import Icon from '../../components/Icon';
import CategoryGrid from '../../components/CategoryGrid';
import Loader from '../../components/Loader';
import * as uiActions from '../../services/ui/actions';
import * as spotifyActions from '../../services/spotify/actions';
import {
  isLoading,
} from '../../util/helpers';
import { i18n, I18n } from '../../locale';
import { indexToArray } from '../../util/arrays';

class DiscoverCategories extends React.Component {
  componentDidMount() {
    const {
      categories,
      spotifyActions: {
        getCategories,
      },
      uiActions: {
        setWindowTitle,
      },
    } = this.props;

    // Check for an empty category index, or where we've only got one loaded
    // This would be the case if you've refreshed from within a category and only loaded
    // the single record.
    if (!categories || Object.keys(categories).length <= 1) {
      getCategories();
    }
    setWindowTitle(i18n('discover.categories.title'));
  }

  render = () => {
    const {
      load_queue,
      categories: categoriesProp,
      uiActions,
    } = this.props;

    if (isLoading(load_queue, ['spotify_browse/categories'])) {
      return (
        <div className="view discover-categories-view">
          <Header icon="grid" title={i18n('discover.categories.title')} />
          <Loader body loading />
        </div>
      );
    }

    const categories = indexToArray(categoriesProp);

    return (
      <div className="view discover-categories-view">
        <Header uiActions={uiActions}>
          <Icon name="mood" type="material" />
          <I18n path="discover.categories.title" />
        </Header>
        <section className="content-wrapper grid-wrapper">
          <CategoryGrid categories={categories} />
        </section>
      </div>
    );
  }
}

const mapStateToProps = (state) => ({
  categories: state.spotify.categories,
  load_queue: state.ui.load_queue,
});

const mapDispatchToProps = (dispatch) => ({
  uiActions: bindActionCreators(uiActions, dispatch),
  spotifyActions: bindActionCreators(spotifyActions, dispatch),
});

export default connect(mapStateToProps, mapDispatchToProps)(DiscoverCategories);
