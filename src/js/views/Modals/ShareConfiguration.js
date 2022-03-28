import React, { useState, useEffect } from 'react';
import { pick } from 'lodash';
import { useDispatch, useSelector } from 'react-redux';
import Modal from './Modal';
import { setWindowTitle, closeModal } from '../../services/ui/actions';
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
                <I18n path="modal.share_configuration.server.title" />
              </div>
              <div className="description mid_grey-text">
                <I18n path="modal.share_configuration.server.description" />
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

const ShareConfiguration = () => {
  const dispatch = useDispatch();
  const [recipients, setRecipients] = useState([]);
  const [selectedConfigs, setSelectedConfigs] = useState([]);
  const spotify = useSelector((state) => state.spotify);
  const genius = useSelector((state) => state.genius);
  const lastfm = useSelector((state) => state.lastfm);
  const ui = useSelector((state) => state.ui);
  const snapcast = useSelector((state) => state.snapcast);

  useEffect(() => {
    setWindowTitle(i18n('modal.share_configuration.title'));
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

  const collateConfig = () => {
    const obj = {};
    selectedConfigs.forEach((name) => {
      switch (name) {
        case 'spotify':
          obj.spotify = pick(spotify, ['authorization', 'me']);
          break;
        case 'genius':
          obj.genius = pick(genius, ['authorization', 'me']);
          break;
        case 'lastfm':
          obj.lastfm = pick(lastfm, ['authorization', 'me']);
          break;
        case 'snapcast':
          obj.snapcast = pick(snapcast, ['enabled', 'host', 'port', 'ssl']);
          break;
        case 'ui':
          obj.ui = pick(
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
          );
          break;
        default:
          break;
      }
    });
    return obj;
  };

  const onSubmit = (e) => {
    e.preventDefault();
    const configuration = collateConfig();

    for (const recipient of recipients) {
      if (recipient === 'SERVER') {
        dispatch(setSharedConfig(configuration));
      } else {
        dispatch(
          deliverMessage(
            recipient,
            'share_configuration_received',
            configuration,
          ),
        );
      }
    }

    dispatch(closeModal());
  }

  return (
    <Modal className="modal--share-configuration">

      <h1>
        <I18n path="modal.share_configuration.title" />
      </h1>
      <h2>
        <I18n path="modal.share_configuration.subtitle" />
      </h2>

      <form onSubmit={onSubmit}>
        <div className="field checkbox white">
          <div className="name">
            <I18n path="modal.share_configuration.recipients" />
          </div>
          <RecipientsList
            selected={recipients}
            onChange={onRecipientChanged}
          />
        </div>

        <div className="field checkbox checkbox--block">
          <div className="name">
            <I18n path="modal.share_configuration.configurations" />
          </div>
          <div className="input">

            {spotify?.authorization && (
              <div className="checkbox-group__item">
                <label>
                  <input
                    type="checkbox"
                    name="spotify"
                    checked={selectedConfigs.indexOf('spotify') > -1}
                    onChange={() => onSelectedConfigChanged('spotify')}
                  />
                  <div className="label">
                    <div>
                      <div className="title">
                        <I18n
                          path="modal.share_configuration.authorization"
                          service={i18n('services.spotify.title')}
                        />
                      </div>
                      <div className="description mid_grey-text">
                        <I18n
                          path="modal.share_configuration.logged_in_as"
                          name={spotify.me?.name}
                        />
                      </div>
                    </div>
                  </div>
                </label>
              </div>
            )}

            {lastfm?.authorization && (
              <div className="checkbox-group__item">
                <label>
                  <input
                    type="checkbox"
                    name="lastfm_authorization"
                    checked={selectedConfigs.indexOf('lastfm') > -1}
                    onChange={() => onSelectedConfigChanged('lastfm')}
                  />
                  <div className="label">
                    <div>
                      <div className="title">
                        <I18n
                          path="modal.share_configuration.authorization"
                          service={i18n('services.lastfm.title')}
                        />
                      </div>
                      <div className="description mid_grey-text">
                        <I18n
                          path="modal.share_configuration.logged_in_as"
                          name={lastfm.me?.name}
                        />
                      </div>
                    </div>
                  </div>
                </label>
              </div>
            )}

            {genius?.authorization && (
              <div className="checkbox-group__item">
                <label>
                  <input
                    type="checkbox"
                    name="genius_authorization"
                    checked={selectedConfigs.indexOf('genius') > -1}
                    onChange={() => onSelectedConfigChanged('genius')}
                  />
                  <div className="label">
                    <div>
                      <div className="title">
                        <I18n
                          path="modal.share_configuration.authorization"
                          service={i18n('services.genius.title')}
                        />
                      </div>
                      <div className="description mid_grey-text">
                        <I18n
                          path="modal.share_configuration.logged_in_as"
                          name={genius?.me?.name || 'Unknown'}
                        />
                      </div>
                    </div>
                  </div>
                </label>
              </div>
            )}

            <div className="checkbox-group__item">
              <label>
                <input
                  type="checkbox"
                  name="snapcast"
                  checked={selectedConfigs.indexOf('snapcast') > -1}
                  onChange={() => onSelectedConfigChanged('snapcast')}
                />
                <div className="label">
                  <div>
                    <div className="title">
                      <I18n path="services.snapcast.title" />
                    </div>
                    <div className="description mid_grey-text">
                      <I18n path="modal.share_configuration.snapcast_description" />
                    </div>
                  </div>
                </div>
              </label>
            </div>

            <div className="checkbox-group__item">
              <label>
                <input
                  type="checkbox"
                  name="interface"
                  checked={selectedConfigs.indexOf('ui') > -1}
                  onChange={() => onSelectedConfigChanged('ui')}
                />
                <div className="label">
                  <div>
                    <div className="title">
                      <I18n path="modal.share_configuration.interface" />
                    </div>
                    <div className="description mid_grey-text">
                      <I18n path="modal.share_configuration.interface_description" />
                    </div>
                  </div>
                </div>
              </label>
            </div>
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

export default ShareConfiguration;
