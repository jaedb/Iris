import React from 'react';
import { useDispatch, useSelector } from 'react-redux';

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
import Button from './Button';

const Notification = ({
  item: {
    key,
    type,
    level,
    title,
    content,
    description,
    links,
    configuration,
    duration,
    closing,
  },
  importConfiguration,
}) => {
  const dispatch = useDispatch();
  const { removeNotification } = uiActions;

  switch (type) {
    case 'shortcut':
      return (
        <div className="notification__wrapper">
          <div className={`notification notification--shortcut${closing ? ' closing' : ''}`} data-duration={duration}>
            <Icon name={content} />
            {title && (
              <h4 className="notification__title">{title}</h4>
            )}
          </div>
        </div>
      );

    default:
      return (
        <div className="notification__wrapper" key={key}>
          <div
            className={`notification notification--${level}${closing ? ' closing' : ''}`}
            data-key={key}
            data-duration={duration}
          >
            <Icon
              name="close"
              className="notification__close-button"
              onClick={() => dispatch(removeNotification(key, true))}
            />
            {title && (
              <h4 className="notification__title">
                {title}
              </h4>
            )}
            {content && (
              <div className="notification__content">
                {content}
              </div>
            )}
            {description && (
              <div className="notification__description">
                {description}
              </div>
            )}
            {links && (
              <div className="notification__actions">
                {
                  links.map((link, i) => (
                    <Button
                      type="secondary"
                      className="notification__actions__item"
                      href={link.url}
                      target={link.new_window ? '_blank' : 'self'}
                      key={i}
                      tracking={{ category: 'NotificationLink', action: 'Click', label: link.text }}
                    >
                      {link.text}
                    </Button>
                  ))
                }
              </div>
            )}
          </div>
        </div>
      );
  }
};

const NotificationItems = () => {
  const index = useSelector((state) => state.ui.notifications);
  const items = indexToArray(index);
  const dispatch = useDispatch();

  const importConfiguration = (notification_key, configuration) => {
    if (configuration.ui) {
      dispatch(uiActions.set(configuration.ui));
    }

    if (configuration.spotify) {
      dispatch(spotifyActions.importAuthorization(
        configuration.spotify.authorization,
        configuration.spotify.me,
      ));
    }

    if (configuration.snapcast) {
      dispatch(snapcastActions.set(configuration.snapcast));
      setTimeout(() => dispatch(snapcastActions.connect()), 100);
    }

    if (configuration.lastfm) {
      dispatch(lastfmActions.importAuthorization(
        configuration.lastfm.authorization,
        configuration.lastfm.me,
      ));
    }

    if (configuration.genius) {
      dispatch(geniusActions.importAuthorization(
        configuration.genius.authorization,
        configuration.genius.me,
      ));
    }

    dispatch(uiActions.removeNotification(notification_key, true));
    dispatch(uiActions.createNotification({
      content: i18n('modal.shared_config.imported'),
    }));
  }

  if (!items || !items.length) return null;

  return (
    <ErrorBoundary>
      {items.map((item) => (
        <Notification
          key={`${item.key}_${item.status}`}
          item={item}
          importConfiguration={importConfiguration}
        />
      ))}
    </ErrorBoundary>
  );
};

const Process = ({
  item: {
    total,
    remaining,
    level = 'info',
    content,
    description,
    status,
    closing,
  },
  cancelProcess,
  closeProcess,
}) => {
  let progress = 0;
  if (total && remaining) {
    progress = ((total - remaining) / total).toFixed(4);
  }

  switch (status) {
    case 'running':
      return (
        <div className="notification__wrapper">
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
            <Icon name="close" className="notification__close-button" onClick={cancelProcess} />
          </div>
        </div>
      );
    case 'finished':
      return (
        <div className="notification__wrapper">
          <div
            className={
              `notification notification--${level} notification--process${closing ? ' closing' : ''}`
            }
          >
            <Icon className="notification__icon" name={level === 'error' ? 'close' : 'check'} />
            {content && content !== '' && <div className="notification__content">{content}</div>}
            {description && description !== '' && <div className="notification__description">{description}</div>}
            <Icon name="close" className="notification__close-button" onClick={closeProcess} />
          </div>
        </div>
      );
    case 'cancelling':
      return (
        <div className="notification__wrapper">
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
    default:
      return null;
  }
};

const ProcessItems = () => {
  const dispatch = useDispatch();
  const index = useSelector((state) => state.ui.processes);
  const items = indexToArray(index).filter(
    (p) => p.notification && p.status !== 'cancelled' && p.status !== 'completed',
  );
  if (!items || !items.length) return null;

  const {
    cancelProcess,
    closeProcess,
  } = uiActions;

  return (
    <>
      {items.map((item) => (
        <Process
          key={`${item.key}_${item.status}`}
          item={item}
          closeProcess={() => dispatch(closeProcess(item.key))}
          cancelProcess={() => dispatch(cancelProcess(item.key))}
        />
      ))}
    </>
  );
};

const Notifications = () => {
  return (
    <div className="notifications">
      <NotificationItems />
      <ProcessItems />
    </div>
  );
};

export default Notifications;
