import React from 'react';
import { connect } from 'react-redux';
import { bindActionCreators } from 'redux';
import * as uiActions from '../../services/ui/actions';
import { uriType } from '../../util/helpers';
import Icon from '../Icon';
import { i18n } from '../../locale';
import { encodeUri } from '../../util/format';

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
    const { term: termState } = this.state;
    if (nextProps.term !== termProp) return true;
    if (nextState.term !== termState) return true;

    return false;
  }

  onChange = (e) => {
    this.setState({
      term: e.target.value,
      pristine: false,
    });
  }

  onBlur = () => {
    const { onBlur } = this.props;
    const { term } = this.state;
    this.setState({ pristine: false });
    if (onBlur) {
      onBlur(term);
    }
  }

  onFocus = () => {
    this.setState({ pristine: false });
  }

  onSubmit = (e) => {
    const { term } = this.state;
    const { history } = this.props;
    e.preventDefault();

    // check for uri type matching
    switch (uriType(term)) {
      case 'album':
        history.push(`/album/${encodeUri(term)}`);
        break;

      case 'artist':
        history.push(`/artist/${encodeUri(term)}`);
        break;

      case 'playlist':
        history.push(`/playlist/${encodeUri(term)}`);
        break;

      case 'track':
        history.push(`/track/${encodeUri(term)}`);
        break;

      default:
        this.props.onSubmit(term);
        break;
    }

    return false;
  }

  render = () => {
    const { term } = this.state;
    const { onReset } = this.props;

    return (
      <form className="search-form" onSubmit={this.onSubmit}>
        <label>
          <input
            type="text"
            placeholder={i18n('fields.search')}
            onChange={this.onChange}
            onBlur={this.onBlur}
            onFocus={this.onFocus}
            value={term}
          />
        </label>
        {term && (
          <Icon name="close" className="search-form__reset" onClick={onReset} />
        )}
      </form>
    );
  }
}

const mapDispatchToProps = (dispatch) => ({
  uiActions: bindActionCreators(uiActions, dispatch),
});

export default connect(mapDispatchToProps)(SearchForm);
