import React from 'react';
import { connect } from 'react-redux';
import { bindActionCreators } from 'redux';

import * as helpers from '../../helpers';
import * as uiActions from '../../services/ui/actions';

class SearchForm extends React.Component {
  constructor(props) {
    super(props);

    this.state = {
      term: this.props.term,
      pristine: true,
    };
  }

  componentWillReceiveProps(nextProps) {
    if (this.state.pristine && this.state.term == '' && this.state.term !== nextProps.term) {
      this.setState({ term: nextProps.term, pristine: false });
    }
  }

  handleBlur(e) {
    this.setState({ pristine: false });
    if (this.props.onBlur) {
      this.props.onBlur(this.state.term);
    }
  }

  handleFocus(e) {
    this.setState({ pristine: false });
  }

  handleSubmit(e) {
    e.preventDefault();

    // check for uri type matching
    switch (helpers.uriType(this.state.term)) {
      case 'album':
        this.props.history.push(`/album/${encodeURIComponent(this.state.term)}`);
        break;

      case 'artist':
        this.props.history.push(`/artist/${encodeURIComponent(this.state.term)}`);
        break;

      case 'playlist':
        this.props.history.push(`/playlist/${encodeURIComponent(this.state.term)}`);
        break;

      case 'track':
        this.props.history.push(`/track/${encodeURIComponent(this.state.term)}`);
        break;

      default:
        this.props.onSubmit(this.state.term);
        break;
    }

    return false;
  }

  render() {
    return (
      <form className="search-form" onSubmit={(e) => this.handleSubmit(e)}>
        <label>
          <input
            type="text"
            placeholder="Search..."
            onChange={(e) => this.setState({ term: e.target.value, pristine: false })}
            onBlur={(e) => this.handleBlur}
            onFocus={(e) => this.handleFocus}
            value={this.state.term}
          />
        </label>
      </form>
    );
  }
}

const mapDispatchToProps = (dispatch) => ({
  uiActions: bindActionCreators(uiActions, dispatch),
});

export default connect(mapDispatchToProps)(SearchForm);
