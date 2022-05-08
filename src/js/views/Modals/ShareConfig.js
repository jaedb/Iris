import React, { useState, useEffect } from 'react';
import { pick, map } from 'lodash';
import { useDispatch, useSelector } from 'react-redux';
import Modal from './Modal';
import {
  setWindowTitle,
  closeModal,
  createNotification,
} from '../../services/ui/actions';
import { deliverMessage, setSharedConfig } from '../../services/pusher/actions';
import { i18n, I18n } from '../../locale';
import Button from '../../components/Button';
import { indexToArray } from '../../util/arrays';

const RecipientsList = ({
  selected,
  onChange,
}) => {
  const {
    connection_id: currentConnectionId,
    connections,
  } = useSelector((state) => state.pusher);
  const connectionsArray = indexToArray(connections).filter(
    ({ connection_id }) => connection_id !== currentConnectionId,
  );

  return (
    <div className="input checkbox-group">
      <div className="checkbox-group__item">
        <label>
          <input
            type="checkbox"
            name="connection_server"
            checked={selected.includes('SERVER')}
            onChange={() => onChange('SERVER')}
          />
          <div className="label">
            <div>
              <div className="title">
                <I18n path="modal.shared_config.server.label" />
              </div>
              <div className="description mid_grey-text">
                <I18n path="modal.shared_config.server.description" />
              </div>
            </div>
          </div>
        </label>
      </div>
      {
        connectionsArray.map(({ connection_id, username, ip }) => (
          <div key={connection_id} className="checkbox-group__item">
            <label>
              <input
                type="checkbox"
                name={`connection_${connection_id}`}
                checked={selected.includes(connection_id)}
                onChange={() => onChange(connection_id)}
              />
              <div className="label">
                <div>
                  <div className="title">{username}</div>
                  <div className="description mid_grey-text">{ip}</div>
                </div>
              </div>
            </label>
          </div>
        ))
      }
    </div>
  );
}

const ShareConfig = () => {
  const dispatch = useDispatch();
  const [recipients, setRecipients] = useState([]);
  const [selectedConfigs, setSelectedConfigs] = useState([]);
  const spotify = useSelector((state) => state.spotify);
  const genius = useSelector((state) => state.genius);
  const lastfm = useSelector((state) => state.lastfm);
  const ui = useSelector((state) => state.ui);
  const snapcast = useSelector((state) => state.snapcast);

  useEffect(() => {
    setWindowTitle(i18n('modal.shared_config.title'));
  }, []);

  const onRecipientChanged = (id) => {
    setRecipients((prev) => {
      const next = [...prev];
      if (next.includes(id)) {
        const index = next.indexOf(id);
        next.splice(index, 1);
      } else {
        next.push(id);
      }
      return next;
    });
  }

  const onSelectedConfigChanged = (id) => {
    setSelectedConfigs((prev) => {
      const next = [...prev];
      if (next.includes(id)) {
        const index = next.indexOf(id);
        next.splice(index, 1);
      } else {
        next.push(id);
      }
      return next;
    });
  }

  const allConfigs = {
    ui: pick(
      ui,
      [
        'language',
        'theme',
        'smooth_scrolling_enabled',
        'hotkeys_enabled',
        'allow_reporting',
        'wide_scrollbars_enabled',
        'hide_scrollbars',
        'grid_glow_enabled',
        'sort',
        'initial_setup_complete',
        'uri_schemes_search_enabled',
      ],
    ),
    snapcast: pick(snapcast, ['enabled', 'host', 'port', 'ssl']),
    spotify: pick(spotify, ['authorization', 'me']),
    genius: pick(genius, ['authorization', 'me']),
    lastfm: pick(lastfm, ['authorization', 'me']),
  }

  const onSubmit = (e) => {
    e.preventDefault();
    const config = pick(allConfigs, selectedConfigs)

    for (const recipient of recipients) {
      if (recipient === 'SERVER') {
        dispatch(setSharedConfig(config));
      } else {
        dispatch(
          deliverMessage(
            recipient,
            'share_config_received',
            config,
          ),
        );
      }
    }
    dispatch(createNotification({
      content: i18n('modal.shared_config.shared'),
    }));

    dispatch(closeModal());
  }

  return (
    <Modal className="modal--share-configuration">

      <h1>
        <I18n path="modal.shared_config.push.title" />
      </h1>
      <h2>
        <I18n path="modal.shared_config.push.subtitle" />
      </h2>

      <form onSubmit={onSubmit}>
        <div className="field checkbox white">
          <div className="name">
            <I18n path="modal.shared_config.recipients" />
          </div>
          <RecipientsList
            selected={recipients}
            onChange={onRecipientChanged}
          />
        </div>

        <div className="field checkbox checkbox--block">
          <div className="name">
            <I18n path="modal.shared_config.config.label" />
          </div>
          <div className="input">
            {map(allConfigs, (data, name) => (
              <div className="checkbox-group__item" key={name}>
                <label>
                  <input
                    type="checkbox"
                    name={name}
                    checked={selectedConfigs.indexOf(name) > -1}
                    onChange={() => onSelectedConfigChanged(name)}
                  />
                  <div className="label">
                    <div>
                      <div className="title">
                        <I18n path={`modal.shared_config.config.${name}.label`} />
                      </div>
                      <div className="description mid_grey-text">
                        <I18n
                          path={`modal.shared_config.config.${name}.description`}
                          name={data.me?.name || 'Unknown'}
                        />
                      </div>
                    </div>
                  </div>
                </label>
              </div>
            ))}
          </div>
        </div>

        <div className="actions centered-text">
          <Button
            type="primary"
            size="large"
            disabled={recipients.length <= 0}
            onClick={onSubmit}
            tracking={{ category: 'ShareConfiguration', action: 'Send' }}
          >
            <I18n path="actions.send" />
          </Button>
        </div>
      </form>
    </Modal>
  );
}

export default ShareConfig;
