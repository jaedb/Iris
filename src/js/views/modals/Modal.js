import React from 'react';
import { connect } from 'react-redux';
import Icon from '../../components/Icon';

class Modal extends React.Component {
  componentDidMount() {
    $('body').addClass('modal-open');
  }

  componentWillUnmount() {
    $('body').removeClass('modal-open');
  }

  render() {
    const {
      extraControls = null,
      noclose = false,
      children,
      className = '',
    } = this.props;

    return (
      <div className={`modal ${className}`}>

        <div className="controls">
          {extraControls}
          {!noclose && (
            <div className="control close" onClick={(e) => window.history.back()}>
              <Icon name="close" className="white" />
            </div>
          )}
        </div>

        <div className="content">
          {children}
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
