
import React from 'react';
import { connect } from 'react-redux';
import { bindActionCreators } from 'redux';
import VolumeControl from './VolumeControl';
import MuteControl from './MuteControl';
import Icon from '../Icon';
import DropdownField from './DropdownField';
import * as coreActions from '../../services/core/actions';
import * as pusherActions from '../../services/pusher/actions';
import * as snapcastActions from '../../services/snapcast/actions';
import { sortItems, indexToArray } from '../../util/arrays';
import { collate } from '../../util/format';

class OutputControl extends React.Component {
  constructor(props) {
    super(props);

    this.state = {
      expanded: false,
    };

    this.handleClick = this.handleClick.bind(this);
  }

  handleClick(e) {
    if (!this.props.force_expanded && $(e.target).closest('.output-control').length <= 0) {
      this.setExpanded(false);
    }
  }

  componentDidUpdate = ({
    force_expanded: prev_force_expanded,
  }) => {
    const { force_expanded } = this.props;
    if (!prev_force_expanded && force_expanded) this.setExpanded(true);
  }

  setExpanded(expanded = !this.state.expanded, props = this.props) {
    if (expanded) {
      this.setState({ expanded });
      window.addEventListener('click', this.handleClick, false);

      // Re-check our snapcast clients
      // TODO: Once we have push events, remove this as it'll (marginally)
      // slow down the reveal/render
      if (props.pusher_connected && props.snapcast_enabled) {
        this.props.snapcastActions.getServer();
      }
    } else {
      this.setState({ expanded });
      window.removeEventListener('click', this.handleClick, false);
    }
  }

  snapcastGroups() {
    const {
      snapcast_streams,
      snapcastActions,
      snapcast_groups,
      snapcast_clients: clients,
    } = this.props;

    const groups = indexToArray(snapcast_groups);
    if (groups.length <= 0) return null;

    const streams = Object.keys(snapcast_streams).map(
      (id) => ({ value: id, label: id }),
    );

    return (
      <div>
        {
          groups.map((simpleGroup) => {
            const group = collate(simpleGroup, { clients });
            if (
              !group.clients ||
              !group.clients.length ||
              !group.clients.filter((client) => client.connected).length
            ) {
              return null;
            }
            return (
              <div className="output-control__item outputs__item--snapcast" key={group.id}>
                <div className="output-control__item__name">
                  {group.name}
                </div>
                <div className="output-control__item__controls">
                  <DropdownField
                    name="Source"
                    value={group.stream_id}
                    icon="settings_input_component"
                    options={streams}
                    noLabel
                    handleChange={(value) => snapcastActions.setGroupStream(group.id, value)}
                  />
                  <MuteControl
                    className="output-control__item__mute"
                    noTooltip
                    mute={group.mute}
                    onMuteChange={(mute) => snapcastActions.setGroupMute(group.id, mute)}
                  />
                  <VolumeControl
                    className="output-control__item__volume"
                    volume={group.volume}
                    mute={group.mute}
                    onVolumeChange={(percent, previousPercent) => snapcastActions.setGroupVolume(group.id, percent, previousPercent)}
                  />
                </div>
              </div>
            )
          })
        }
      </div>
    );
  }

  commands() {
    const {
      pusher_commands,
      pusherActions,
    } = this.props;

    if (!pusher_commands) return null;

    let items = indexToArray(pusher_commands);
    if (items.length <= 0) return null;

    items = sortItems(items, 'sort_order');

    return (
      <div className="output-control__item output-control__item--commands commands">
        {
          items.map((command) => (
            <div
              key={command.id}
              className="commands__item commands__item--interactive"
              onClick={(e) => pusherActions.runCommand(command.id)}
            >
              <Icon className="commands__item__icon" name={command.icon} />
              <span className={`${command.colour}-background commands__item__background`} />
            </div>
          ))
        }
      </div>
    );
  }

  localStreaming() {
    const {
      http_streaming_enabled,
      http_streaming_volume,
      http_streaming_mute,
      coreActions,
    } = this.props;

    if (!http_streaming_enabled) return null;

    return (
      <div className="output-control__item outputs__item--icecast">
        <div className="output-control__item__name">
          Local browser
        </div>
        <div className="output-control__item__controls">
          <MuteControl
            className="output-control__item__mute"
            noTooltip
            mute={http_streaming_mute}
            onMuteChange={(mute) => coreActions.set({ http_streaming_mute: mute })}
          />
          <VolumeControl
            className="output-control__item__volume"
            volume={http_streaming_volume}
            mute={http_streaming_mute}
            onVolumeChange={(percent) => coreActions.set({ http_streaming_volume: percent })}
          />
        </div>
      </div>
    );
  }

  renderOutputs() {
    const snapcastGroups = this.snapcastGroups();
    const localStreaming = this.localStreaming();
    const commands = this.commands();

    if (!snapcastGroups && !localStreaming && !commands) {
      return (
        <div className="output-control__items output-control__items--no-results">
          <p className="no-results">No outputs</p>
        </div>
      );
    }
    return (
      <div className="output-control__items">
        {commands}
        {localStreaming}
        {snapcastGroups}
      </div>
    );
  }

  render() {
    if (this.state.expanded) {
      return (
        <span className="output-control">
          <button className="control speakers active" onClick={(e) => this.setExpanded()}><Icon name="speaker" /></button>
          {this.renderOutputs()}
        </span>
      );
    }

    // No customisable outputs
    if (!this.props.http_streaming_enabled && !this.props.snapcast_enabled && !this.props.pusher_commands) {
      return (
        <span className="output-control disabled">
          <button className="control speakers"><Icon name="speaker" /></button>
        </span>
      );
    }
    return (
      <span className="output-control">
        <button className="control speakers" onClick={(e) => this.setExpanded()}><Icon name="speaker" /></button>
      </span>
    );
  }
}

const mapStateToProps = (state, ownProps) => ({
  http_streaming_enabled: state.core.http_streaming_enabled,
  http_streaming_volume: parseInt(state.core.http_streaming_volume) || 50,
  http_streaming_mute: state.core.http_streaming_mute,
  pusher_connected: state.pusher.connected,
  snapcast_enabled: (state.pusher.config ? state.pusher.config.snapcast_enabled : null),
  show_disconnected_clients: (state.ui.snapcast_show_disconnected_clients !== undefined ? state.ui.snapcast_show_disconnected_clients : false),
  snapcast_clients: state.snapcast.clients,
  snapcast_groups: state.snapcast.groups,
  snapcast_streams: state.snapcast.streams,
  pusher_commands: (state.pusher.commands ? state.pusher.commands : {}),
});

const mapDispatchToProps = (dispatch) => ({
  coreActions: bindActionCreators(coreActions, dispatch),
  snapcastActions: bindActionCreators(snapcastActions, dispatch),
  pusherActions: bindActionCreators(pusherActions, dispatch),
});

export default connect(mapStateToProps, mapDispatchToProps)(OutputControl);
