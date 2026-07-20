import React from 'react';
import ReactGA from 'react-ga';

const trackEvent = ({
  category,
  action,
  label,
}) => {
  // While we call this regardless, if ReactGA hasn't been initialized (in App) then it won't
  // do anything. This is the case when the user has reporting disabled.
  ReactGA.event({ category, action, label });
};

const Trackable = ({
  category,
  action,
  label,
  ...rest
}) => {
  const onClick = () => {
    trackEvent({
      category,
      action,
      label,
    });
  };

  return (
    <span
      onClick={onClick}
      {...rest}
    />
  );
};

export {
  Trackable,
  trackEvent,
};

export default {
  Trackable,
  trackEvent,
};
