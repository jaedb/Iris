import React from 'react';
import { connect } from 'react-redux';
import { bindActionCreators } from 'redux';
import Modal from './Modal';
import * as uiActions from '../../services/ui/actions';

class ImageZoom extends React.Component {
  componentDidMount() {
    this.props.uiActions.setWindowTitle('Zoomed image');
  }

  render() {
    return (
      <Modal className="modal--image-zoom">
        <img src={this.props.location.search.replace('?url=', '')} />
      </Modal>
    );
  }
}

const mapStateToProps = (state) => ({});

const mapDispatchToProps = (dispatch) => ({
  uiActions: bindActionCreators(uiActions, dispatch),
});

export default connect(mapStateToProps, mapDispatchToProps)(ImageZoom);
