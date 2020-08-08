
import React from 'react';
import { connect } from 'react-redux';
import { bindActionCreators } from 'redux';
import * as coreActions from '../../services/core/actions';
import Icon from '../Icon';

class PinButton extends React.Component {
  remove = () => {
    const {
      coreActions: {
        removePinned,
      },
      uri,
    } = this.props;
    removePinned(uri);
  }

  add = () => {
    const {
      coreActions: {
        addPinned,
      },
      uri,
    } = this.props;
    addPinned(uri);
  }

  render = () => {
    const {
      uri,
      isPinned,
    } = this.props;
    let { className = '' } = this.props;
    if (!uri) return null;

    className += ' button button--discrete';

    if (isPinned) {
      return (
        <button
          type="button"
          className={`${className} button--destructive`}
          onClick={this.remove}
        >
          <Icon name="bookmark" />
        </button>
      );
    }
    return (
      <button
        type="button"
        className={`${className} button--default`}
        onClick={this.add}
      >
        <Icon name="bookmark_border" />
      </button>
    );
  }
}

const mapStateToProps = (state, { uri }) => ({
  isPinned: state.core.pinned ? state.core.pinned.find((item) => item === uri) : false,
});

const mapDispatchToProps = (dispatch) => ({
  coreActions: bindActionCreators(coreActions, dispatch),
});

export default connect(mapStateToProps, mapDispatchToProps)(PinButton);
