
import React from 'react';
import { connect } from 'react-redux';
import { bindActionCreators } from 'redux';
import LinksSentence from '../LinksSentence';
import TextField from './TextField';
import * as coreActions from '../../services/core/actions';
import * as uiActions from '../../services/ui/actions';
import * as spotifyActions from '../../services/spotify/actions';
import { generateGuid } from '../../util/helpers';
import { i18n } from '../../locale';

class AddSeedField extends React.Component {
  constructor(props) {
    super(props);

    this.state = {
      value: '',
    };
    this.id = generateGuid();
    this.timer = null;
    this.handleClick = this.handleClick.bind(this);
  }

  componentDidMount() {
    const {
      genres,
      spotifyActions: {
        getGenres,
      },
    } = this.props;

    window.addEventListener('click', this.handleClick, false);

    if (!genres) {
      getGenres();
    }
  }

  componentWillUnmount() {
    window.removeEventListener('click', this.handleClick, false);
  }

  handleClick = (e) => {
    const {
      spotifyActions: {
        clearAutocompleteResults,
      },
    } = this.props;

    if ($(e.target).closest('.add-seed-field').length <= 0) {
      clearAutocompleteResults(this.id);
    }
  }

  handleChange = (value) => {
    const {
      spotifyActions: {
        getAutocompleteResults,
      },
    } = this.props;
    const self = this;

    // update our local state
    this.setState({ value });

    // start a timer to perform the actual search
    // this provides a wee delay between key presses to avoid request spamming
    clearTimeout(this.timer);
    this.timer = setTimeout(
      () => {
        self.setState({ searching: true });
        getAutocompleteResults(self.id, value, ['artist', 'track', 'genre']);
      },
      500,
    );
  }

  handleSelect = (e, item) => {
    const {
      onSelect,
      spotifyActions: {
        clearAutocompleteResults,
      },
    } = this.props;

    this.setState({ value: '' });
    onSelect(e, item.uri);
    clearAutocompleteResults(this.id);
  }

  results = (type) => {
    const {
      results: resultsProp,
    } = this.props;

    const results = resultsProp[this.id];
    if (type) {
      if (results) {
        return results[type];
      }
      return null;
    }
    return results;
  }

  renderResults = (type) => {
    const results = this.results(type);
    if (!results) return null;

    return (
      <div className="type">
        <h4 className="mid_grey-text">{type}</h4>
        {
          results.slice(0, 3).map((item) => (
            <div className="result" key={item.uri} onClick={(e) => this.handleSelect(e, item)}>
              {item.name}
              {type === 'tracks' && (
                <span className="mid_grey-text">
                  {' '}
                  <LinksSentence items={item.artists} type="artist" nolinks />
                </span>
              )}
            </div>
          ))
        }
      </div>
    );
  }

  render = () => {
    const {
      placeholder,
      className: classNameProp = '',
    } = this.props;
    const { value } = this.state;
    const results = this.results();

    let className = 'field autocomplete-field add-seed-field';
    if (results && results.loading) {
      className += ' loading';
    }
    return (
      <div className={`${className} ${classNameProp}`}>
        <div className="input">
          <TextField
            value={value}
            onChange={this.handleChange}
            placeholder={placeholder || i18n('fields.start_typing')}
            everyChange
          />
        </div>
        <div className="results">
          {this.renderResults('artists')}
          {this.renderResults('tracks')}
          {this.renderResults('genres')}
        </div>
      </div>
    );
  }
}

const mapStateToProps = (state) => ({
  genres: (state.spotify.genres || null),
  results: (state.spotify.autocomplete_results || {}),
});

const mapDispatchToProps = (dispatch) => ({
  coreActions: bindActionCreators(coreActions, dispatch),
  uiActions: bindActionCreators(uiActions, dispatch),
  spotifyActions: bindActionCreators(spotifyActions, dispatch),
});

export default connect(mapStateToProps, mapDispatchToProps)(AddSeedField);
