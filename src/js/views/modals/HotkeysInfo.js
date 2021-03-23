import React from 'react';
import { connect } from 'react-redux';
import { bindActionCreators } from 'redux';
import Modal from './Modal';
import * as uiActions from '../../services/ui/actions';
import { I18n, i18n } from '../../locale';

const hotkeys = [
  { label: 'info', keys: ['i'] },
  { label: 'play_pause', keys: ['p', 'spacebar'] },
  { label: 'stop', keys: ['s'] },
  { label: 'rewind', keys: ['r'] },
  { label: 'fastforward', keys: ['f'] },
  { label: 'next', keys: ['.', '>'] },
  { label: 'previous', keys: [',', '<'] },
  { label: 'volume_up', keys: ['=', '+'] },
  { label: 'volume_down', keys: ['-', '_'] },
  { label: 'mute', keys: ['0'] },
  { label: 'exit', keys: ['esc'] },
  { label: 'now_playing', keys: ['1'] },
  { label: 'search', keys: ['2'] },
  { label: 'kiosk', keys: ['3'] },
];

class HotkeysInfo extends React.Component {
  componentDidMount() {
    const { uiActions: { setWindowTitle } } = this.props;
    setWindowTitle('Hotkeys');
  }

  render = () => (
    <Modal className="modal--hotkeys-info">
      <h1><I18n path="modal.hotkeys_info.title" /></h1>
      <div className="list small playlists">
        {hotkeys.map((hotkey) => (
          <div className="list__item" key={hotkey.label}>
            {hotkey.keys.map((key, index) => (
              <>
                <pre>
                  {key}
                </pre>
                {index === 0 && hotkey.keys.length > 1 && ' or '}
              </>
            ))}
            <span className="description">
              {i18n(`modal.hotkeys_info.keys.${hotkey.label}`)}
            </span>
          </div>
        ))}
      </div>
    </Modal>
  )
}

const mapStateToProps = () => ({});

const mapDispatchToProps = (dispatch) => ({
  uiActions: bindActionCreators(uiActions, dispatch),
});

export default connect(mapStateToProps, mapDispatchToProps)(HotkeysInfo);
