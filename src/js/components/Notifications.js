
import React from 'react';
import { connect } from 'react-redux';
import { bindActionCreators } from 'redux';

import * as uiActions from '../services/ui/actions';
import * as spotifyActions from '../services/spotify/actions';
import * as lastfmActions from '../services/lastfm/actions';
import * as geniusActions from '../services/genius/actions';
import * as snapcastActions from '../services/snapcast/actions';

import Icon from './Icon';

class Notifications extends React.Component {
  constructor(props) {
    super(props);
  }

  importConfiguration(notification_key, configuration) {
    console.log('Importing configuration', configuration);
    const configurations = '';

    if (configuration.interface) {
      this.props.uiActions.set(configuration.interface);
    }

    if (configuration.spotify) {
      this.props.spotifyActions.importAuthorization(configuration.spotify.authorization, configuration.spotify.me);
    }

    if (configuration.lastfm) {
      this.props.lastfmActions.importAuthorization(configuration.lastfm.authorization, configuration.lastfm.me);
    }

    if (configuration.genius) {
      this.props.geniusActions.importAuthorization(configuration.genius.authorization, configuration.genius.me);
    }

    this.props.uiActions.removeNotification(notification_key, true);
    this.props.uiActions.createNotification({ type: 'info', content: 'Import successful' });
  }

  renderNotifications() {
    if (!this.props.notifications || this.props.notifications.length <= 0) return null;

    const notifications = [];
    for (const key in this.props.notifications) {
      if (this.props.notifications.hasOwnProperty(key)) {
        notifications.push(this.props.notifications[key]);
      }
    }

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
      </ul>
      <p>Do you want to import this?</p>
    </div>
    <div className="notification__actions">
      <a className="notification__actions__item button button--default" onClick={(e) => this.importConfiguration(notification.key, notification.configuration)}>Import</a>
    </div>
  </div>
					      );

					    default:
					      return (
  <div className={`notification notification--${notification.type}${notification.closing ? ' closing' : ''}`} key={notification.key} data-key={notification.key} data-duration={notification.duration}>
    <Icon name="close" className="notification__close-button" onClick={(e) => this.props.uiActions.removeNotification(notification.key, true)} />
    {notification.title ? <h4 className="notification__title">{notification.title}</h4> : null}
    {notification.content ? <div className="notification__content">{notification.content}</div> : null}
    {notification.description ? <div className="notification__description">{notification.description}</div> : null }
    {notification.links ? (
      <div className="notification__actions">
        {notification.links.map((link, i) => <a className="notification__actions__item button button--secondary" href={link.url} target={link.new_window ? '_blank' : 'self'} key={i}>{link.text}</a>)}
      </div>
    ) : null }
  </div>
					      );
					  }
					})
				}
      </span>
    );
  }

  renderProcess(process) {
    let progress = 0;
    if (process.data.total && process.data.remaining) {
      progress = ((process.data.total - process.data.remaining) / process.data.total * 100).toFixed();
    }

    switch (process.status) {
      case 'running':
        return (
          <div className={`notification notification--process${process.closing ? ' closing' : ''}`} key={process.key}>
            <div className="loader">
              <div className="progress">
                <div className="fill" style={{ width: `${progress}%` }} />
              </div>
            </div>
            {process.message}
            <Icon name="close" className="notification__close-button" onClick={(e) => { this.props.uiActions.cancelProcess(process.key); }} />
          </div>
        );

      case 'cancelling':
        return (
          <div className={`notification notification--process cancelling${process.closing ? ' closing' : ''}`} key={process.key}>
            <div className="loader" />
						Cancelling
          </div>
        );

      case 'cancelled':
      case 'finished':
        return null;
    }
  }

  renderProcesses() {
    if (!this.props.processes || this.props.processes.length <= 0) return null;

    const processes = [];
    for (const key in this.props.processes) {
      if (this.props.processes.hasOwnProperty(key)) {
        processes.push(this.props.processes[key]);
      }
    }

    return (
      <span>
        {processes.map((process) => this.renderProcess(process))}
      </span>
    );
  }

  // do we want the loading of everything to be displayed?
  // not likely...
  renderLoader() {
    if (!this.props.load_queue) {
      return null;
    }

    const { load_queue } = this.props;
    let load_count = 0;
    for (const key in load_queue) {
      if (load_queue.hasOwnProperty(key)) {
        load_count++;
      }
    }

    if (load_count > 0) {
      let className = 'loading ';
      if (load_count > 20) {
        className += 'high';
      } else if (load_count > 5) {
        className += 'medium';
      } else {
        className += 'low';
      }
      return (
        <div className={className} />
      );
    }
    return null;
  }

  render() {
    return (
      <div className="notifications">
        {this.renderLoader()}
        {this.renderNotifications()}
        {this.renderProcesses()}
      </div>
    );
  }
}

const mapStateToProps = (state, ownProps) => ({
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
