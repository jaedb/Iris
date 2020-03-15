
import React from 'react';
import { connect } from 'react-redux';
import { bindActionCreators } from 'redux';
import * as pusherActions from '../services/pusher/actions';
import { indexToArray } from '../util/arrays';

class PusherConnectionList extends React.Component {
  componentDidMount() {
    if (this.props.connected) {
      this.props.pusherActions.getConnections();
    }
  }

  componentDidUpdate = ({ connected: prevConnected }) => {
    const { connected, pusherActions: { getConnections } } = this.props;
    if (!prevConnected && connected) getConnections();
  }

  render() {
    if (!this.props.connected) {
      return <div className="pusher-connection-list mid_grey-text">Not connected</div>;
    }

    const connections = indexToArray(this.props.connections);
    if (connections.length <= 0) {
      return <div className="pusher-connection-list mid_grey-text">No connections</div>;
    }

    return (
      <div className="pusher-connection-list">
        {
					connections.map((connection) => {
					  let is_me = false;
					  if (connection.connection_id == this.props.connection_id) {
					    is_me = true;
					  }

					  return (
  <div className={is_me ? 'connection cf me' : 'connection cf'} key={connection.connection_id}>
    <div className="col col--w30">
      { connection.username }
      {' '}
      {is_me ? <span>(you)</span> : null }
    </div>
    <div className="col col--w70">
      {connection.ip}
      <span className="mid_grey-text">
        {' '}
(
        {connection.connection_id}
)
      </span>
    </div>
  </div>
					  );
					})
				}
      </div>
    );
  }
}

const mapStateToProps = (state, ownProps) => ({
  connected: state.pusher.connected,
  connection_id: state.pusher.connection_id,
  connections: state.pusher.connections,
});

const mapDispatchToProps = (dispatch) => ({
  pusherActions: bindActionCreators(pusherActions, dispatch),
});

export default connect(mapStateToProps, mapDispatchToProps)(PusherConnectionList);
