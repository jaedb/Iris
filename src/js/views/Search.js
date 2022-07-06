import React from 'react';
import { connect } from 'react-redux';
import { bindActionCreators } from 'redux';
import Header from '../components/Header';
import Icon from '../components/Icon';
import DropdownField from '../components/Fields/DropdownField';
import SearchForm from '../components/Fields/SearchForm';
import SearchResults from '../components/SearchResults';
import * as coreActions from '../services/core/actions';
import * as uiActions from '../services/ui/actions';
import * as mopidyActions from '../services/mopidy/actions';
import * as spotifyActions from '../services/spotify/actions';
import { titleCase } from '../util/helpers';
import { withRouter } from '../util';
import { i18n } from '../locale';

class Search extends React.Component {
  constructor(props) {
    super(props);

    this.state = {
      type: props.type || 'all',
      term: props.term || '',
    };
  }

  componentDidMount = () => {
    const {
      uiActions: {
        setWindowTitle,
      },
    } = this.props;

    setWindowTitle('Search');

    // Auto-focus on the input field
    $(document).find('.search-form input').focus();
    this.digestUri();
  }

  componentDidUpdate = ({
    type: prevType,
    term: prevTerm,
  }) => {
    const {
      type: typeProp,
      term: termProp,
    } = this.props;
    const { type, term } = this.state;

    if (prevType !== typeProp || prevTerm !== termProp) {
      this.search(type, term);
    }
  }

  onSubmit = (term) => {
    const { type } = this.state;
    const { navigate } = this.props;
    const encodedTerm = encodeURIComponent(term);

    this.setState(
      { term },
      () => {
        navigate(`/search/${type}/${encodedTerm}`);
      },
    );
  }

  onReset = () => {
    const { navigate } = this.props;
    navigate('/search');
  }

  onSortChange = (value) => {
    const { uiActions: { hideContextMenu } } = this.props;
    this.setSort(value);
    hideContextMenu();
  }

  onSourceChange = (value) => {
    const {
      uiActions: {
        set,
        hideContextMenu,
      },
    } = this.props;
    set({ uri_schemes_search_enabled: value });
    hideContextMenu();
  }

  onSourceClose = () => {
    this.search(true);
  };

  digestUri = () => {
    const {
      type,
      term,
    } = this.props;

    if (type && term) {
      this.setState({ type, term }, () => {
        this.search();
      });
    } else if (!term || term === '') {
      this.clearSearch();
    }
  }

  clearSearch = () => {
    const {
      uiActions: {
        setWindowTitle,
      },
    } = this.props;

    setWindowTitle(i18n('search.title'));
    this.setState({ term: '' });
  }

  search = (force = false) => {
    const {
      coreActions: {
        startSearch,
      },
      uiActions: {
        setWindowTitle,
      },
      search_results_query: {
        type: existingType,
        term: existingTerm,
      },
    } = this.props;
    const {
      type,
      term,
    } = this.state;

    setWindowTitle(i18n('search.title_window', { term: decodeURIComponent(term) }));

    if ((type && term && (force || existingType !== type || existingTerm !== term))) {
      startSearch({ type, term });
    }
  }

  setSort = (value) => {
    const {
      sort,
      sort_reverse,
      uiActions: {
        set,
      },
    } = this.props;

    let reverse = false;
    if (sort === value) reverse = !sort_reverse;

    const data = {
      search_results_sort_reverse: reverse,
      search_results_sort: value,
    };
    set(data);
  }

  render = () => {
    const {
      term,
      type,
    } = this.state;
    const {
      uri_schemes,
      sort,
      sort_reverse,
      navigate,
      uri_schemes_search_enabled,
      uiActions,
    } = this.props;

    const sort_options = [
      { value: 'followers', label: i18n('common.popularity') },
      { value: 'name', label: i18n('common.name') },
      { value: 'artist', label: i18n('common.artist') },
      { value: 'duration', label: i18n('common.duration') },
      { value: 'uri', label: i18n('common.source') },
    ];

    const provider_options = uri_schemes.map((item) => ({
      value: item,
      label: titleCase(item.replace(':', '').replace('+', ' ')),
    }));

    const options = (
      <>
        <DropdownField
          icon="swap_vert"
          name={i18n('common.sort')}
          value={sort}
          valueAsLabel
          options={sort_options}
          selected_icon={sort_reverse ? 'keyboard_arrow_up' : 'keyboard_arrow_down'}
          handleChange={this.onSortChange}
        />
        <DropdownField
          icon="cloud"
          name={i18n('common.sources')}
          value={uri_schemes_search_enabled}
          options={provider_options}
          handleChange={this.onSourceChange}
          onClose={this.onSourceClose}
        />
      </>
    );

    let searchResults;

    switch (type) {
      case 'artists':
        searchResults = <SearchResults type="artists" query={{ term, type: 'artists' }} />;
        break;
      case 'albums':
        searchResults = <SearchResults type="albums" query={{ term, type: 'albums' }} />;
        break;
      case 'playlists':
        searchResults = <SearchResults type="playlists" query={{ term, type: 'playlists' }} />;
        break;
      case 'tracks':
        searchResults = <SearchResults type="tracks" query={{ term, type: 'tracks' }} />
        break;
      default:
        searchResults = (
          <>
            <div className="search-result-sections cf">
              <section className="search-result-sections__item">
                <div className="inner">
                  <SearchResults type="artists" query={{ term, type: 'artists' }} all />
                </div>
              </section>
              <section className="search-result-sections__item">
                <div className="inner">
                  <SearchResults type="albums" query={{ term, type: 'albums' }} all />
                </div>
              </section>
              <section className="search-result-sections__item">
                <div className="inner">
                  <SearchResults type="playlists" query={{ term, type: 'playlists' }} all />
                </div>
              </section>
            </div>
            <SearchResults type="tracks" query={{ term, type: 'tracks' }} all />
          </>
        );
    }

    return (
      <div className="view search-view">
        <Header options={options} uiActions={uiActions}>
          <Icon name="search" type="material" />
        </Header>

        <SearchForm
          key={`search_form_${type}_${term}`}
          term={term}
          onSubmit={this.onSubmit}
          onReset={this.onReset}
        />

        <div className="content-wrapper">
          {searchResults}
        </div>
      </div>
    );
  }
}

const mapStateToProps = (state, ownProps) => {
  const {
    params: {
      type,
      term,
    },
    navigation,
  } = ownProps;
  console.log(ownProps);
  const {
    mopidy: {
      uri_schemes = [],
    },
    ui: {
      uri_schemes_search_enabled = [],
      search_results_sort: sort = 'followers.total',
      search_results_sort_reverse,
    },
    core: {
      search_results: {
        query: search_results_query = {},
      } = {},
    },
  } = state;

  return {
    type,
    term,
    navigation,
    uri_schemes,
    uri_schemes_search_enabled,
    sort,
    sort_reverse: !!search_results_sort_reverse,
    search_results_query,
  };
};

const mapDispatchToProps = (dispatch) => ({
  coreActions: bindActionCreators(coreActions, dispatch),
  uiActions: bindActionCreators(uiActions, dispatch),
  mopidyActions: bindActionCreators(mopidyActions, dispatch),
  spotifyActions: bindActionCreators(spotifyActions, dispatch),
});

export default withRouter(connect(mapStateToProps, mapDispatchToProps)(Search));
