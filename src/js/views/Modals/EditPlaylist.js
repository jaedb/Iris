import React, { useEffect, useState } from 'react';
import { useSelector, useDispatch } from 'react-redux';
import { useParams } from 'react-router-dom';
import Modal from './Modal';
import { loadPlaylist, savePlaylist } from '../../services/core/actions';
import { closeModal, setWindowTitle } from '../../services/ui/actions';
import { uriSource } from '../../util/helpers';
import { i18n, I18n } from '../../locale';
import Button from '../../components/Button';
import { decodeUri } from '../../util/format';
import { makeItemSelector } from '../../util/selectors';

const EMPTY_FORM = {
  loaded: false,
  error: null,
  name: '',
  description: '',
  image: null,
  public: false,
  collaborative: false,
};

const Fields = ({
  uri,
  form,
  onChange,
  onChangeImage,
}) => (
  <div>
    <div>
      <div className="field text">
        <div className="name">
          <I18n path="modal.edit_playlist.name" />
        </div>
        <div className="input">
          <input
            type="text"
            name="name"
            onChange={onChange}
            value={form.name}
          />
        </div>
      </div>
      {uriSource(uri) === 'spotify' && (
        <div>
          <div className="field text">
            <div className="name">
              <I18n path="modal.edit_playlist.description" />
            </div>
            <div className="input">
              <input
                type="text"
                name="description"
                onChange={onChange}
                value={form.description}
              />
            </div>
          </div>
          <div className="field file">
            <div className="name">
              <I18n path="modal.edit_playlist.image.label" />
            </div>
            <div className="input">
              <input
                type="file"
                placeholder="Leave empty to keep existing image"
                onChange={onChangeImage}
              />
              <div className="description">
                <I18n path="modal.edit_playlist.image.description" />
              </div>
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
                  checked={form.isPublic}
                  onChange={onChange}
                />
                <span className="label">
                  <I18n path="modal.edit_playlist.options.public" />
                </span>
              </label>
              <label>
                <input
                  type="checkbox"
                  name="collaborative"
                  checked={form.collaborative}
                  onChange={onChange}
                />
                <span className="label">
                  <I18n path="modal.edit_playlist.options.collaborative" />
                </span>
              </label>
            </div>
          </div>
        </div>
      )}
    </div>
  </div>
);

const EditPlaylist = () => {
  const { uri: encodedUri } = useParams();
  const uri = decodeUri(encodedUri);
  const itemSelector = makeItemSelector(uri);
  const playlist = useSelector(itemSelector);
  const dispatch = useDispatch();
  const [form, setForm] = useState(EMPTY_FORM);
  const [error, setError] = useState();

  useEffect(() => {
    dispatch(setWindowTitle(i18n('modal.edit_playlist.title')));

    if (uri && !playlist) dispatch(loadPlaylist(uri));
  }, []);

  useEffect(() => {    
    setForm({
      loaded: true,
      name: playlist?.name || '',
      description: playlist?.description || '',
      public: (playlist?.public === true),
      collaborative: (playlist?.collaborative === true),
    });
  }, [playlist]);

  const onChange = ({ target: { name, type, value, checked }}) =>
    setForm((prev) => ({
      ...prev,
      [name]: type === 'checkbox' ? checked : value,
    }));
  
  const onChangeImage = (e) => {
    const file_reader = new FileReader();
    file_reader.addEventListener('load', (e) => {
      const image_base64 = e.target.result.replace('form:image/jpeg;base64,', '');
      setForm({ image: image_base64 });
    });
    file_reader.readAsDataURL(e.target.files[0]);
  }
  
  const onSubmit = (e) => {
    e.preventDefault();

    if (form.name == '') {
      setError(i18n('modal.edit_playlist.name_required'));
      return;
    }
    const data = uriSource(uri) === 'spotify'
      ? form
      : { name: form.name };
    dispatch(savePlaylist(uri, data));
    dispatch(closeModal());
  }

  return (
    <Modal className="modal--edit-playlist">
      <h1>
        <I18n path="modal.edit_playlist.title" />
      </h1>
      {error && <h3 className="red-text">{error}</h3>}
      <form onSubmit={onSubmit}>
        <Fields
          uri={uri}
          form={form}
          onChange={onChange}
          onChangeImage={onChangeImage}
        />

        <div className="actions centered-text">
          <Button
            type="primary"
            size="large"
            tracking={{ category: 'EditPlaylist', action: 'Submit' }}
            submit
          >
            <I18n path="actions.save" />
          </Button>
        </div>
      </form>
    </Modal>
  );
}

export default EditPlaylist;
