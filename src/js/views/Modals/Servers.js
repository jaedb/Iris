import React, { useEffect } from 'react';
import { useHistory } from 'react-router-dom';
import { useDispatch, useSelector } from 'react-redux';
import Thumbnail from '../../components/Thumbnail';
import LinksSentence from '../../components/LinksSentence';
import Modal from './Modal';
import * as uiActions from '../../services/ui/actions';
import * as mopidyActions from '../../services/mopidy/actions';
import { i18n, I18n } from '../../locale';
import { indexToArray } from '../../util/arrays';

const Servers = () => {
  const history = useHistory();
  const dispatch = useDispatch();
  const servers = indexToArray(useSelector((state) => state.mopidy.servers || {}));
  const current_server = useSelector((state) => state.mopidy.current_server);

  useEffect(() => dispatch(uiActions.setWindowTitle(i18n('modal.servers.title'))), []);
  useEffect(() => {
    servers.forEach(({ id }) => {
      dispatch(mopidyActions.getServerState(id));
    });
  }, []);

  const onClick = (server) => {
    dispatch(mopidyActions.setCurrentServer(server));
    history.push('/queue');
  }

  return (
    <Modal className="modal--servers">

      <h1>
        <I18n path="modal.servers.title" />
      </h1>

      <div className="list small">
        <>
          {servers.map((server) => {
            const {
              id,
              name,
              playback_state,
              current_track,
            } = server;

            return (
              <div key={id} onClick={() => onClick(server)} className="list__item">
                <Thumbnail images={current_track?.images} size="small" />
                <h4 className="list__item__name">
                  {name}
                  {playback_state && (
                    <span className="flag flag--default">{playback_state}</span>
                  )}
                  {id === current_server && (
                    <span className="flag flag--blue">
                      <I18n path="modal.servers.current" />
                    </span>
                  )}
                </h4>
                {current_track ? (
                  <ul className="list__item__details details">
                    <li>{current_track?.name}</li>
                    <li><LinksSentence items={current_track?.artists} type="artist" nolinks /></li>
                  </ul>
                ) : (
                  <div className="list__item__details details">
                    <I18n path="modal.servers.nothing_playing" />
                  </div>
                )}
              </div>
            );
          })}
        </>
      </div>
    </Modal>
  );
}

export default Servers;
