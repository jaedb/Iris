import React from 'react';
import { connect } from 'react-redux';
import { bindActionCreators } from 'redux';
import * as uiActions from '../../services/ui/actions';
import { uriType } from '../../util/helpers';

class SearchForm extends React.Component {
  constructor(props) {
    super(props);

    this.state = {
      term: props.term,
      pristine: true,
    };
  }

  static getDerivedStateFromProps(props, state) {
    const { pristine, term } = state;
    if (pristine && term === '' && term !== props.term) {
      return {
        term,
        pristine: false,
      };
    }
    return null;
  }

  shouldComponentUpdate = (nextProps, nextState) => {
    const { term: termProp } = this.props;
    const { term: termState } = this.props;
    if (nextProps.term !== termProp) return true;
    if (nextState.term !== termState) return true;

    return false;
  }

  handleBlur() {
    const { onBlur } = this.props;
    const { term } = this.state;
    this.setState({ pristine: false });
    if (onBlur) {
      onBlur(term);
    }
  }

  handleFocus(e) {
    this.setState({ pristine: false });
  }

  handleSubmit(e) {
    e.preventDefault();

    // check for uri type matching
    switch (uriType(this.state.term)) {
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
