import React from 'react';
import { connect } from 'react-redux';
import { bindActionCreators } from 'redux';
import localForage from 'localforage';
import Modal from './Modal';
import * as uiActions from '../../services/ui/actions';
import * as coreActions from '../../services/core/actions';
import { i18n, I18n } from '../../locale';
import Button from '../../components/Button';

class Reset extends React.Component {
  constructor(props) {
    super(props);

    this.state = {
      test_mode: false,
      preferences: false,
      database: true,
      cache: ('serviceWorker' in navigator),
      service_worker: ('serviceWorker' in navigator),
      working: false,
    };
  }

  componentDidMount() {
    this.props.uiActions.setWindowTitle(i18n('modal.reset.title'));
  }

  handleSubmit = (e) => {
    const {
      coreActions: {
        resetState,
      },
    } = this.props;
    const {
      preferences,
      database,
      cache,
      service_worker,
      test_mode,
      working,
    } = this.state;

    if (working) return null;

    this.setState({ working: true });

    e.preventDefault();
    const tasks = [];
    const stateKeysToReset = [];
    const stateKeys = ['mopidy', 'pusher', 'ui', 'spotify', 'snapcast', 'lastfm', 'genius'];

    if (preferences) {
      tasks.push(
        new Promise((resolve) => {
          const keysToRemove = [];
          stateKeys.forEach((key) => {
            keysToRemove.push(`persist:${key}`);
            stateKeysToReset.push(key);
          });
          keysToRemove.forEach((key, index) => {
            localForage.removeItem(key).then(() => {
              console.debug(`Removed ${key}`);
              if (index === keysToRemove.length - 1) {
                resolve();
              }
            });
          });
        }),
      );
    }
    if (database) {
      tasks.push(
        new Promise((resolve) => {
          localForage.keys().then((keys) => {
            const keysToRemove = keys.filter(
              (key) => !stateKeys.includes(key.replace('persist:', '')),
            );
            keysToRemove.forEach((key, index) => {
              localForage.removeItem(key).then(() => {
                console.debug(`Removed ${key}`);
                if (index === keysToRemove.length - 1) {
                  resolve();
                }
              });
            });
          });
        }),
      );
    }
    if (cache) {
      tasks.push(
        new Promise((resolve, reject) => {
          if ('serviceWorker' in navigator) {
            caches.keys().then((cacheNames) => {
              cacheNames.forEach((cacheName) => {
                caches.delete(cacheName);
              });
              resolve();
            });
          } else {
            reject();
          }
        }),
      );
    }
    if (service_worker) {
      tasks.push(
        new Promise((resolve, reject) => {
          if ('serviceWorker' in navigator) {
            // Unregister all service workers
            // This forces our SW to bugger off and a new one is registered on refresh
            navigator.serviceWorker.getRegistrations().then(
              (registrations) => {
                for (const registration of registrations) {
                  registration.unregister();
                }
                resolve();
              },
            );
          } else {
            reject();
          }
        }),
      );
    }

    Promise.all(tasks).then(() => {
      console.log('Reset complete, refreshing...');
      resetState(stateKeysToReset);
      setTimeout(
        () => window.location = `/iris/settings${test_mode ? '?test_mode=0' : ''}`,
        1000,
      );
    });
  }

  render = () => {
    const {
      preferences,
      database,
      service_worker,
      cache,
      test_mode,
      working,
    } = this.state;

    return (
      <Modal className="modal--reset">

        <h1>
          <I18n path="modal.reset.title" />
        </h1>
        <h2>
          <I18n path="modal.reset.subtitle" />
        </h2>

        <form onSubmit={this.handleSubmit}>
          <div className="field checkbox checkbox--block">
            <div className="name">
              <I18n path="modal.reset.items" />
            </div>
            <div className="input">
              {['preferences', 'database', 'cache', 'service_worker', 'test_mode'].map((name) => {
                const { [name]: value } = this.state;
                const disabled = (name === 'cache' || name === 'service_worker')
                  && !('serviceWorker' in navigator);

                return (
                  <div className="checkbox-group__item" key={name}>
                    <label>
                      <input
                        type="checkbox"
                        name="spotify"
                        checked={value}
                        disabled={disabled}
                        onChange={() => this.setState({ [name]: !value })}
                      />
                      <div className="label">
                        <div>
                          <div className="title">
                            <I18n path={`modal.reset.${name}.label`} />
                          </div>
                          <div className="description mid_grey-text">
                            <I18n path={`modal.reset.${name}.description`} />
                          </div>
                        </div>
                      </div>
                    </label>
                  </div>
                );
              })}
            </div>
          </div>

          <div className="actions centered-text">
            <Button
              type="primary"
              size="large"
              onClick={this.handleSubmit}
              working={working}
              disabled={!database && !cache && !service_worker && !preferences && !test_mode}
              tracking={{ category: 'Reset', action: 'Submit' }}
            >
              <I18n path="actions.reset" />
            </Button>
          </div>
        </form>
      </Modal>
    );
  }
}

const mapStateToProps = () => ({});
const mapDispatchToProps = (dispatch) => ({
  coreActions: bindActionCreators(coreActions, dispatch),
  uiActions: bindActionCreators(uiActions, dispatch),
});

export default connect(mapStateToProps, mapDispatchToProps)(Reset);
