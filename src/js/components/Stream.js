
import React from 'react';
import { connect } from 'react-redux';
import { bindActionCreators } from 'redux';
import * as coreActions from '../services/core/actions';
import { SnapStream } from './SnapStream';

class Stream extends React.Component {
  constructor(props) {
    super(props);

    if (props.enabled && props.streaming_enabled) {
      this.snapstream = new SnapStream('192.168.1.201', 1780);
    } else {
      this.snapstream = null;
    }
  }

  componentDidUpdate = () => {
    const { enabled, streaming_enabled } = this.props;

    if (enabled && streaming_enabled && !this.snapstream) {
      this.snapstream = new SnapStream('192.168.1.201', 1780);
    }
    if ((!enabled || !streaming_enabled) && this.snapstream) {
      this.snapstream = null;
    }
  }

  render = () => null;
}

const mapStateToProps = (state) => ({
  enabled: state.snapcast.enabled,
  streaming_enabled: state.snapcast.streaming_enabled,
});

const mapDispatchToProps = (dispatch) => ({
  coreActions: bindActionCreators(coreActions, dispatch),
});

export default connect(mapStateToProps, mapDispatchToProps)(Stream);
