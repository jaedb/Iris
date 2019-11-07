
import React from 'react';
import { connect } from 'react-redux';
import { bindActionCreators } from 'redux';

import VolumeControl from './Fields/VolumeControl';
import MuteControl from './Fields/MuteControl';
import LatencyControl from './Fields/LatencyControl';
import TextField from './Fields/TextField';
import SelectField from './Fields/SelectField';

import * as helpers from '../helpers';

const SnapcastClients = ({ actions, group, groups, show_disconnected_clients }) => {
  if (!show_disconnected_clients && group.clients) {
    var clients = helpers.applyFilter('connected', true, group.clients);
  } else {
    var { clients } = group;
  }

  if (!clients || clients.length <= 0) {
    return (
      <div className="text mid_grey-text">
        No clients
      </div>
    );
  }

  return (
    <div className="list snapcast__clients">
      {
        clients.map((client) => {
          let class_name = 'list__item list__item--no-interaction snapcast__client';
          if (client.connected) {
            class_name += ' snapcast__client--connected';
          } else {
            class_name += ' snapcast__client--disconnected';
          }

          return (
            <div className={class_name} key={client.id}>
              <div className="snapcast__client__details">
                <label className="field">
                  <div className="name">
                    Name
                  </div>
                  <div className="input">
                    <TextField
                      onChange={(value) => actions.setClientName(client.id, value)}
                      value={client.name}
                    />
                  </div>
                </label>
                <label className="field dropdown">
                  <div className="name">
                    Group
                  </div>
                  <div className="input">
                    <SelectField
                      onChange={(value) => actions.setClientGroup(client.id, value)}
                      value={group.id}
                      options={[
                        ...groups.map((group) => ({ value: group.id, label: group.name })),
                        { value: group.id, label: 'New group' },
                      ]}
                    />
                  </div>
                </label>
                <div className="snapcast__client__volume field">
                  <div className="name">
                    Volume
                  </div>
                  <div className="input">
                    <MuteControl
                      className="snapcast__mute-control snapcast__client__mute-control"
                      mute={client.mute}
                      onMuteChange={(mute) => actions.setClientMute(client.id, mute)}
                    />
                    <VolumeControl
                      className="snapcast__volume-control snapcast__client__volume-control"
                      volume={client.volume}
                      mute={client.mute}
                      onVolumeChange={(percent) => actions.setClientVolume(client.id, percent)}
                    />
                  </div>
                </div>
                <div className="snapcast__client__latency field">
                  <div className="name">
                    Latency
                  </div>
                  <div className="input">
                    <LatencyControl
                      max="150"
                      value={client.latency}
                      onChange={(value) => actions.setClientLatency(client.id, parseInt(value))}
                    />
                    <TextField
                      className="tiny"
                      type="number"
                      onChange={(value) => actions.setClientLatency(client.id, parseInt(value))}
                      value={String(client.latency)}
                    />
                  </div>
                </div>
              </div>
            </div>
          );
        })
      }
    </div>
  );
}

export default SnapcastClients;
