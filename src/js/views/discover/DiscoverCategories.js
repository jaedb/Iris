import React from 'react';
import { connect } from 'react-redux';
import { bindActionCreators } from 'redux';
import Header from '../../components/Header';
import Icon from '../../components/Icon';
import { Grid } from '../../components/Grid';
import Loader from '../../components/Loader';
import * as uiActions from '../../services/ui/actions';
import * as spotifyActions from '../../services/spotify/actions';
import {
  encodeUri,
} from '../../util/format';
import { i18n, I18n } from '../../locale';
import { indexToArray } from '../../util/arrays';
import { makeLoadingSelector } from '../../util/selectors';

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
      loading,
      categories,
      uiActions,
    } = this.props;

    if (loading) {
      return <Loader body loading />;
    }

    return (
      <div className="view discover-categories-view">
        <Header uiActions={uiActions}>
          <Icon name="mood" type="material" />
          <I18n path="discover.categories.title" />
        </Header>
        <section className="content-wrapper grid-wrapper">
          <Grid
            className="grid--tiles"
            items={categories}
            getLink={(item) => `/discover/categories/${encodeUri(item.uri)}`}
            sourceIcon={false}
          />
        </section>
      </div>
    );
  }
}

const mapStateToProps = (state) => {
  const loadingSelector = makeLoadingSelector(['(.*)categories(.*)']);

  return {
    loading: loadingSelector(state),
    categories: indexToArray(state.spotify.categories),
  };
};

const mapDispatchToProps = (dispatch) => ({
  uiActions: bindActionCreators(uiActions, dispatch),
  spotifyActions: bindActionCreators(spotifyActions, dispatch),
});

export default connect(mapStateToProps, mapDispatchToProps)(DiscoverCategories);
