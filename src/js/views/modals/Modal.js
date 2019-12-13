
import React from 'react';
import { connect } from 'react-redux';
import { bindActionCreators } from 'redux';

import * as helpers from '../../helpers';
import Icon from '../../components/Icon';

class Modal extends React.Component {
  constructor(props) {
    super(props);
  }

  componentDidMount() {
    $('body').addClass('modal-open');
  }

  componentWillUnmount() {
    $('body').removeClass('modal-open');
  }

  render() {
    let className = 'modal';
    if (this.props.className) {
      className += ` ${this.props.className}`;
    }

    return (
      <div className={className}>

        <div className="controls">
          {this.props.noclose ? null : (
            <div className="control close" onClick={(e) => window.history.back()}>
              <Icon name="close" className="white" />
            </div>
          ) }
        </div>

        <div className="content">
          {this.props.children}
        </div>
      </div>
    );
  }
}

const mapStateToProps = (state, ownProps) => ({
  shortkeys_enabled: state.ui.shortkeys_enabled,
});

const mapDispatchToProps = (dispatch) => ({});

export default connect(mapStateToProps, mapDispatchToProps)(Modal);
