import React, { useState, useEffect } from 'react';
import { useSelector, useDispatch } from 'react-redux';
import { useParams } from 'react-router-dom';
import Modal from './Modal';
import * as coreActions from '../../services/core/actions';
import * as uiActions from '../../services/ui/actions';
import { i18n, I18n } from '../../locale';
import { decodeUri } from '../../util/format';
import TextField from '../../components/Fields/TextField';
import Button from '../../components/Button';

const CreatePlaylist = () => {
  const { uris } = useParams();
  const tracks_uris = uris ? decodeUri(uris).split(',') : undefined;
  const dispatch = useDispatch();
  const [playlist, setPlaylist] = useState({
    scheme: 'm3u',
    name: '',
    description: '',
    collaborative: false,
    public: false,
  });
  const spotifyAvailable = useSelector((state) => state.spotify.access_token);
  useEffect(
    () => {
      dispatch(uiActions.setWindowTitle(i18n('modal.edit_playlist.title_create')));
    },
    [],
  );

  const onChange = (updates) => setPlaylist({ ...playlist, ...updates });
  const onSubmit = (e) => {
    e.preventDefault();

    if (!playlist.name || playlist.name === '') {
      return false;
    }
    dispatch(
      coreActions.createPlaylist({ ...playlist, tracks_uris }),
    );
    window.history.back();
    return false;
  };

  const fields = () => {
    switch (playlist.scheme) {
      case 'spotify':
        return (
          <div>
            <div className="field text">
              <div className="name">
                <I18n path="modal.edit_playlist.name" />
              </div>
              <div className="input">
                <TextField
                  onChange={(value) => onChange({ name: value })}
                  value={playlist.name}
                />
              </div>
            </div>

            <div className="field text">
              <div className="name">
                <I18n path="modal.edit_playlist.description" />
              </div>
              <div className="input">
                <TextField
                  onChange={(value) => onChange({ description: value })}
                  value={playlist.description}
                />
              </div>
            </div>

            <div className="field checkbox white">
              <div className="name">
                <I18n path="modal.edit_playlist.options.label" />
              </div>
              <div className="input">
                <label>
                  <input
                    type="checkbox"
                    name="public"
                    checked={playlist.public}
                    onChange={() => onChange({ public: !playlist.public })}
                  />
                  <span className="label">
                    <I18n path="modal.edit_playlist.options.public" />
                  </span>
                </label>
                <label>
                  <input
                    type="checkbox"
                    name="collaborative"
                    checked={playlist.collaborative}
                    onChange={() => onChange({ collaborative: !playlist.collaborative })}
                  />
                  <span className="label">
                    <I18n path="modal.edit_playlist.options.collaborative" />
                  </span>
                </label>
              </div>
            </div>
          </div>
        );

      default:
        return (
          <div>
            <div className="field text">
              <div className="name">
                <I18n path="modal.edit_playlist.name" />
              </div>
              <div className="input">
                <TextField
                  onChange={(value) => onChange({ name: value })}
                  value={playlist.name}
                />
              </div>
            </div>
          </div>
        );
    }
  }

  return (
    <Modal className="modal--create-playlist">
      <h1>
        <I18n path="modal.edit_playlist.title_create" />
      </h1>
      {tracks_uris && (
        <h2 className="mid_grey-text">
          <I18n
            path="modal.edit_playlist.subtitle"
            count={tracks_uris.length}
            plural={tracks_uris.length > 1 ? 's' : ''}
          />
        </h2>
      )}
      <form onSubmit={onSubmit}>
        <div className="field radio white">
          <div className="name">
            <I18n path="modal.edit_playlist.provider" />
          </div>
          <div className="input">
            <label>
              <input
                type="radio"
                name="scheme"
                value="m3u"
                checked={playlist.scheme === 'm3u'}
                onChange={(e) => onChange({ scheme: e.target.value })}
              />
              <span className="label">
                <I18n path="services.mopidy.title" />
              </span>
            </label>
            <label>
              <input
                type="radio"
                name="scheme"
                value="spotify"
                disabled={!spotifyAvailable}
                checked={playlist.scheme === 'spotify'}
                onChange={(e) => onChange({ scheme: e.target.value })}
              />
              <span className="label">
                <I18n path="services.spotify.title" />
              </span>
            </label>
          </div>
        </div>
        {fields()}
        <div className="actions centered-text">
          <Button
            type="primary"
            size="large"
            submit
            tracking={{ category: 'CreatePlaylist', action: 'Submit' }}
          >
            <I18n path="modal.edit_playlist.create_playlist" />
          </Button>
        </div>
      </form>
    </Modal>
  );
};

export default CreatePlaylist;
