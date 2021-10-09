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

      <form>
        <>
          {servers.map((server) => {
            const {
              id,
              name,
              playback_state,
              current_track,
            } = server;

            return (
              <div key={id} onClick={() => onClick(server)}>
                <h3>
                  {name}
                  {playback_state ? ` (${playback_state})` : ''}
                </h3>
                <div style={{ display: 'flex', alignItems: 'center' }}>
                  <Thumbnail images={current_track?.images} size="small" />
                  <div style={{ paddingLeft: '1rem' }}>
                    <div>{current_track?.name}</div>
                    <em>
                      <LinksSentence items={current_track?.artists} type="artist" nolinks />
                    </em>
                  </div>
                </div>
              </div>
            );
          })}
        </>
      </form>
    </Modal>
  );
}

export default Servers;
