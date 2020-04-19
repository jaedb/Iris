
import React from 'react';
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
    this.props.uiActions.createNotification({ level: 'warning', content: 'Import successful' });
  }

  renderNotifications() {
    if (!this.props.notifications || this.props.notifications.length <= 0) return null;

    const notifications = indexToArray(this.props.notifications);

    return (
      <span>
        {
          notifications.map((notification) => {
            switch (notification.type) {
              case 'shortcut':
                return (
                  <div className={`notification notification--shortcut${notification.closing ? ' closing' : ''}`} key={notification.key} data-duration={notification.duration}>
                    <Icon name={notification.content} />
                  </div>
                );

              case 'share-configuration-received':
                return (
                  <div className="notification notification--info" key={notification.key} data-key={notification.key} data-duration={notification.duration}>
                    <Icon name="close" className="notification__close-button" onClick={(e) => this.props.uiActions.removeNotification(notification.key, true)} />

                    <h4 className="notification__title">Configuration shared</h4>
                    <div className="notification__content">
                      <p>Another user has shared their configuration with you. This includes:</p>
                      <ul>
                        {notification.configuration.ui ? <li>User interface</li> : null}
                        {notification.configuration.spotify ? <li>Spotify</li> : null}
                        {notification.configuration.lastfm ? <li>LastFM</li> : null}
                        {notification.configuration.genius ? <li>Genius</li> : null}
                        {notification.configuration.snapcast ? <li>Snapcast</li> : null}
                      </ul>
                      <p>Do you want to import this?</p>
                    </div>
                    <div className="notification__actions">
                      <a className="notification__actions__item button button--default" onClick={(e) => this.importConfiguration(notification.key, notification.configuration)}>Import now</a>
                    </div>
                  </div>
                );

              default:
                return (
                  <div className={`notification notification--${notification.level}${notification.closing ? ' closing' : ''}`} key={notification.key} data-key={notification.key} data-duration={notification.duration}>
                    <Icon name="close" className="notification__close-button" onClick={(e) => this.props.uiActions.removeNotification(notification.key, true)} />
                    {notification.title ? <h4 className="notification__title">{notification.title}</h4> : null}
                    {notification.content ? <div className="notification__content">{notification.content}</div> : null}
                    {notification.description ? <div className="notification__description">{notification.description}</div> : null}
                    {notification.links ? (
                      <div className="notification__actions">
                        {notification.links.map((link, i) => <a className="notification__actions__item button button--secondary" href={link.url} target={link.new_window ? '_blank' : 'self'} key={i}>{link.text}</a>)}
                      </div>
                    ) : null}
                  </div>
                );
            }
          })
        }
      </span>
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
          <div
            className={
              `notification notification--${level} notification--process${closing ? ' closing' : ''}`
            }
            key={key}
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
        );

      case 'finished':
        return (
          <div
            className={
              `notification notification--${level} notification--process${closing ? ' closing' : ''}`
            }
            key={key}
          >
            <Icon className="notification__icon" name={level === 'error' ? 'close' : 'check'} />
            {content && content !== '' && <div className="notification__content">{content}</div>}
            {description && description !== '' && <div className="notification__description">{description}</div>}
            <Icon name="close" className="notification__close-button" onClick={() => { uiActions.closeProcess(key); }} />
          </div>
        );

      case 'cancelling':
        return (
          <div
            className={
              `notification notification--${level} notification--process cancelling${closing ? ' closing' : ''}`
            }
            key={key}
          >
            <Loader />
            {content && content !== '' && <div className="notification__content">{content}</div>}
            {description && description !== '' && <div className="notification__description">{description}</div>}
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
      <span>
        {processes.map((process) => this.renderProcess(process))}
      </span>
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
