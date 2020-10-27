
import React from 'react';
import { connect } from 'react-redux';
import { bindActionCreators } from 'redux';
import LinksSentence from '../LinksSentence';
import * as coreActions from '../../services/core/actions';
import * as uiActions from '../../services/ui/actions';
import * as spotifyActions from '../../services/spotify/actions';
import { generateGuid, uriType } from '../../util/helpers';
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
    window.addEventListener('click', this.handleClick, false);

    if (!this.props.genres) {
      this.props.spotifyActions.getGenres();
    }
  }

  componentWillUnmount() {
    window.removeEventListener('click', this.handleClick, false);
  }

  handleClick(e) {
    if ($(e.target).closest('.add-seed-field').length <= 0) {
      this.props.spotifyActions.clearAutocompleteResults(this.id);
    }
  }

  handleChange(e, value) {
    const self = this;

    // update our local state
    this.setState({ value });

    // start a timer to perform the actual search
    // this provides a wee delay between key presses to avoid request spamming
    clearTimeout(this.timer);
    this.timer = setTimeout(
      () => {
            	self.setState({ searching: true });
        self.props.spotifyActions.getAutocompleteResults(self.id, value, ['artist', 'track', 'genre']);
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
      coreActions: {
        itemLoaded,
      },
    } = this.props;
    this.setState({ value: '' });
    onSelect(e, item.uri);
    clearAutocompleteResults(this.id);
    itemLoaded(item);
  }

  results() {
    if (typeof (this.props.results) === 'undefined') {
      return null;
    } if (typeof (this.props.results[this.id]) === 'undefined') {
      return null;
    }
    return this.props.results[this.id];
  }

  renderResults(type) {
    const results = this.results();
    if (!results || typeof (results[type]) === 'undefined' || results[type].length <= 0) return null;

    // only show the first 3
    const items = results[type].slice(0, 3);

    return (
      <div className="type">
        <h4 className="mid_grey-text">{type}</h4>
        {
					items.map((item) => (
  <div className="result" key={item.uri} onClick={(e) => this.handleSelect(e, item)}>
    {item.name}
    {type == 'tracks' ? (
      <span className="mid_grey-text">
        {' '}
        <LinksSentence items={item.artists} nolinks />
      </span>
    ) : null}
  </div>
					))
				}
      </div>
    );
  }

  render() {
    let className = 'field autocomplete-field add-seed-field';
    if (this.results() && this.results().loading) {
      className += ' loading';
    }
    return (
      <div className={className}>
        <div className="input">
          <input
            type="text"
            value={this.state.value}
            onChange={(e) => this.handleChange(e, e.target.value)}
            placeholder={this.props.placeholder || i18n('fields.start_typing')}
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

const mapStateToProps = (state, ownProps) => ({
  genres: (state.ui.genres ? state.ui.genres : null),
  results: (state.spotify.autocomplete_results ? state.spotify.autocomplete_results : {}),
});

const mapDispatchToProps = (dispatch) => ({
  coreActions: bindActionCreators(coreActions, dispatch),
  uiActions: bindActionCreators(uiActions, dispatch),
  spotifyActions: bindActionCreators(spotifyActions, dispatch),
});

export default connect(mapStateToProps, mapDispatchToProps)(AddSeedField);
