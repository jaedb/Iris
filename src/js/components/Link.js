import React from 'react';
import { Link as RouterLink, useHistory, useLocation } from 'react-router-dom';
import { scrollTo } from '../util/helpers';
import { decodeUri } from '../util/format';

const updateScrollPosition = ({
  retainScroll = false,
  scrollTarget = 0,
  history,
  location: {
    state,
    pathname,
  },
}) => {
  // Fetch the current scroll position of our #main element and save to our history's state, so
  // clicking 'back' etc will restore the previous scroll position.
  // This doesn't trigger lazy-load elements (unless scrolling exposes the LazyLoader component).
  const main = document.getElementById('main');

  history.replace(
    pathname,
    {
      ...state,
      scroll_position: main.scrollTop,
      previous: {
        pathname,
      },
    },
  );

  // And now scroll to the top of the page. This can be disabled to allow in-page navigation of
  // tabs, etc
  if (!retainScroll) scrollTo(scrollTarget, (scrollTarget));
};

/**
 * Extends react-router's Link but provides the ability to hook in to the navigation event
 * which lets us scroll to the top of our <main> for a more traditional navigation experience
 * */
const Link = ({
  retainScroll,
  scrollTo: scrollTarget,
  onContextMenu,
  className = '',
  activeClassName,
  to,
  exact,
  children,
}) => {
  if (!to) return <span className={className}>{children}</span>;
  const history = useHistory();
  const location = useLocation();
  const onClick = () => updateScrollPosition({
    history, location, retainScroll, scrollTarget,
  });

  // Decode both links. This handles issues where one link is encoded and the other isn't, but
  // they're otherwise identical
  const link = decodeUri(to);
  const currentLink = decodeUri(history.location.pathname);
  const isLinkActive = exact ? currentLink === link : currentLink.startsWith(link);

  // We have an active detector method
  // This is used almost solely by the Sidebar navigation
  const active = history && isLinkActive ? (activeClassName || 'active') : '';
  return (
    <RouterLink
      onClick={onClick}
      onContextMenu={onContextMenu}
      className={`${className} ${active}`}
      to={to}
    >
      {children}
    </RouterLink>
  );
};

export default Link;

export {
  Link,
  updateScrollPosition,
};
