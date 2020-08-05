
import React, { Fragment } from 'react';
import { connect } from 'react-redux';
import { bindActionCreators } from 'redux';

import * as uiActions from '../services/ui/actions';
import * as spotifyActions from '../services/spotify/actions';
import * as lastfmActions from '../services/lastfm/actions';
import * as geniusActions from '../services/genius/actions';
import * as snapcastActions from '../services/snapcast/actions';

import Icon from './Icon';
import Loader from './Loader';
import { indexToArray } from '../util/arrays';
import { i18n, I18n } from '../locale';
import ErrorBoundary from './ErrorBoundary';

class Notifications extends React.Component {
  importConfiguration(notification_key, configuration) {
    if (configuration.ui) {
      this.props.uiActions.set(configuration.ui);
    }

    if (configuration.spotify) {
      this.props.spotifyActions.importAuthorization(configuration.spotify.authorization, configuration.spotify.me);
    }

    if (configuration.snapcast) {
      this.props.snapcastActions.set(configuration.snapcast);
      setTimeout(() => this.props.snapcastActions.connect(), 100);
    }

    if (configuration.lastfm) {
      this.props.lastfmActions.importAuthorization(configuration.lastfm.authorization, configuration.lastfm.me);
    }

    if (configuration.genius) {
      this.props.geniusActions.importAuthorization(configuration.genius.authorization, configuration.genius.me);
    }

    this.props.uiActions.removeNotification(notification_key, true);
    this.props.uiActions.createNotification({
      level: 'warning',
      content: i18n('modal.share_configuration.import.successful'),
    });
  }

  renderNotifications() {
    if (!this.props.notifications || this.props.notifications.length <= 0) return null;

    const notifications = indexToArray(this.props.notifications);

    return (
      <ErrorBoundary>
        {
          notifications.map((notification) => {
            switch (notification.type) {
              case 'shortcut':
                return (
                  <div className="notification__wrapper" key={notification.key}>
                    <div className={`notification notification--shortcut${notification.closing ? ' closing' : ''}`} data-duration={notification.duration}>
                      <Icon name={notification.content} />
                    </div>
                  </div>
                );

              case 'share-configuration-received':
                return (
                  <div className="notification__wrapper" key={notification.key}>
                    <div className="notification notification--info" key={notification.key} data-duration={notification.duration}>
                      <Icon
                        name="close"
                        className="notification__close-button"
                        onClick={(e) => this.props.uiActions.removeNotification(notification.key, true)}
                      />
                      <h4 className="notification__title">
                        <I18n path="modal.share_configuration.import.title" />
                      </h4>
                      <div className="notification__content">
                        <p>
                          <I18n path="modal.share_configuration.import.subtitle" />
                        </p>
                        <ul>
                          {notification.configuration.ui && (
                            <li><I18n path="modal.share_configuration.interface" /></li>
                          )}
                          {notification.configuration.spotify && (
                            <li><I18n path="services.spotify.title" /></li>
                          )}
                          {notification.configuration.lastfm && (
                            <li><I18n path="services.lastfm.title" /></li>
                          )}
                          {notification.configuration.genius && (
                            <li><I18n path="services.genius.title" /></li>
                          )}
                          {notification.configuration.snapcast && (
                            <li><I18n path="services.snapcast.title" /></li>
                          )}
                        </ul>
                        <p>
                          <I18n path="modal.share_configuration.import.do_you_want_to_import" />
                        </p>
                      </div>
                      <div className="notification__actions">
                        <a className="notification__actions__item button button--default" onClick={(e) => this.importConfiguration(notification.key, notification.configuration)}>
                          <I18n path="modal.share_configuration.import.import_now" />
                        </a>
                      </div>
                    </div>
                  </div>
                );

              default:
                return (
                  <div className="notification__wrapper" key={notification.key}>
                    <div
                      className={`notification notification--${notification.level}${notification.closing ? ' closing' : ''}`}
                      data-key={notification.key}
                      data-duration={notification.duration}
                    >
                      <Icon
                        name="close"
                        className="notification__close-button"
                        onClick={() => this.props.uiActions.removeNotification(notification.key, true)}
                      />
                      {notification.title && (
                        <h4 className="notification__title">
                          {notification.title}
                        </h4>
                      )}
                      {notification.content && (
                        <div className="notification__content">
                          {notification.content}
                        </div>
                      )}
                      {notification.description && (
                        <div className="notification__description">
                          {notification.description}
                        </div>
                      )}
                      {notification.links && (
                        <div className="notification__actions">
                          {
                            notification.links.map((link, i) => (
                              <a
                                className="notification__actions__item button button--secondary"
                                href={link.url}
                                target={link.new_window ? '_blank' : 'self'}
                                key={i}
                              >
                                {link.text}
                              </a>
                            ))
                          }
                        </div>
                      )}
                    </div>
                  </div>
                );
            }
          })
        }
      </ErrorBoundary>
    );
  }

  renderProcess(process) {
    const {
      data: { total, remaining },
      level = 'info',
      content,
      description = null,
      status,
      closing,
      key,
    } = process;
    const { uiActions } = this.props;
    let progress = 0;
    if (total && remaining) {
      progress = ((total - remaining) / total).toFixed(4);
    }

    switch (status) {
      case 'running':
        return (
          <div className="notification__wrapper" key={key}>
            <div
              className={
                `notification notification--${level} notification--process${closing ? ' closing' : ''}`
              }
            >
              <Loader
                progress={progress}
                loading
                mini
                white
              />
              {content && content !== '' && <div className="notification__content">{content}</div>}
              {description && description !== '' && <div className="notification__description">{description}</div>}
              <Icon name="close" className="notification__close-button" onClick={() => { uiActions.cancelProcess(key); }} />
            </div>
          </div>
        );

      case 'finished':
        return (
          <div className="notification__wrapper" key={key}>
            <div
              className={
                `notification notification--${level} notification--process${closing ? ' closing' : ''}`
              }
            >
              <Icon className="notification__icon" name={level === 'error' ? 'close' : 'check'} />
              {content && content !== '' && <div className="notification__content">{content}</div>}
              {description && description !== '' && <div className="notification__description">{description}</div>}
              <Icon name="close" className="notification__close-button" onClick={() => { uiActions.closeProcess(key); }} />
            </div>
          </div>
        );

      case 'cancelling':
        return (
          <div className="notification__wrapper" key={key}>
            <div
              className={
                `notification notification--${level} notification--process cancelling${closing ? ' closing' : ''}`
              }
            >
              <Loader
                progress={progress}
                loading
                mini
                white
              />
              {content && content !== '' && <div className="notification__content">{content}</div>}
              {description && description !== '' && <div className="notification__description">{description}</div>}
              <Icon name="close" className="notification__close-button" />
            </div>
          </div>
        );

      case 'cancelled':
      case 'completed':
      default:
        return null;
    }
  }

  renderProcesses() {
    const { processes: processesObj = {} } = this.props;
    const processes = Object.keys(processesObj).map((key) => processesObj[key]);
    if (!processes.length) return null;

    return (
      <Fragment>
        {processes.map((process) => this.renderProcess(process))}
      </Fragment>
    );
  }

  render() {
    return (
      <div className="notifications">
        {this.renderNotifications()}
        {this.renderProcesses()}
      </div>
    );
  }
}

const mapStateToProps = (state) => ({
  broadcasts: (state.ui.broadcasts ? state.ui.broadcasts : []),
  notifications: (state.ui.notifications ? state.ui.notifications : []),
  processes: (state.ui.processes ? state.ui.processes : {}),
});

const mapDispatchToProps = (dispatch) => ({
  uiActions: bindActionCreators(uiActions, dispatch),
  spotifyActions: bindActionCreators(spotifyActions, dispatch),
  geniusActions: bindActionCreators(geniusActions, dispatch),
  lastfmActions: bindActionCreators(lastfmActions, dispatch),
  snapcastActions: bindActionCreators(snapcastActions, dispatch),
});

export default connect(mapStateToProps, mapDispatchToProps)(Notifications);
