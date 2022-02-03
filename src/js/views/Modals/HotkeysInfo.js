import React from 'react';
import { connect } from 'react-redux';
import { bindActionCreators } from 'redux';
import Modal from './Modal';
import * as uiActions from '../../services/ui/actions';
import { I18n, i18n } from '../../locale';

const hotkeys = [
  { label: 'info', keysets: [['i']] },
  { label: 'play_pause', keysets: [['p'], ['spacebar']] },
  { label: 'stop', keysets: [['s']] },
  { label: 'rewind', keysets: [['r']] },
  { label: 'fastforward', keysets: [['f']] },
  { label: 'next', keysets: [['.'], ['>']] },
  { label: 'previous', keysets: [[','], ['<']] },
  { label: 'volume_up', keysets: [['=']] },
  { label: 'volume_down', keysets: [['-']] },
  { label: 'mute', keysets: [['0']] },
  { label: 'snapcast_volume_up', keysets: [['n', '=']] },
  { label: 'snapcast_volume_down', keysets: [['n', '=']] },
  { label: 'snapcast_mute', keysets: [['n', '0']] },
  { label: 'exit', keysets: [['esc']] },
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
          <div className="list__item list__item--no-interaction" key={hotkey.label}>
            {hotkey.keysets.map((keyset, keysetIndex) => (
              <>
                {keyset.map((key, keyIndex) => (
                  <>
                    <pre>
                      {key}
                    </pre>
                    {keyIndex === 0 && keyset.length > 1 && ' + '}
                  </>
                ))}
                {keysetIndex === 0 && hotkey.keysets.length > 1 && ' or '}
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
