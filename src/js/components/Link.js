import React from 'react';
import { Link, useHistory, useLocation } from 'react-router-dom';
import { scrollTo } from '../util/helpers';
import { decodeUri } from '../util/format';

/**
 * Extends react-router's Link but provides the ability to hook in to the navigation event
 * which lets us scroll to the top of our <main> for a more traditional navigation experience
 * */
export default ({
  retainScroll,
  scrollTo: scrollToProp,
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

  const onClick = () => {
    // Fetch the current scroll position of our #main element and save to our history's state, so
    // clicking 'back' etc will restore the previous scroll position.
    // This doesn't trigger lazy-load elements (unless scrolling exposes the LazyLoader component).
    const main = document.getElementById('main');

    history.replace(
      location.pathname,
      {
        ...location.state,
        scroll_position: main.scrollTop,
        previous: {
          pathname: location.pathname,
        },
      },
    );

    // Allow a link to disable auto-scrolling to the top of the page
    // on navigation. Useful for tabs, etc.
    if (!retainScroll) scrollTo(scrollToProp, (scrollToProp));
  };

  // Decode both links. This handles issues where one link is encoded and the other isn't, but
  // they're otherwise identical
  const link = decodeUri(to);
  const currentLink = decodeUri(history.location.pathname);
  const isLinkActive = exact ? currentLink === link : currentLink.startsWith(link);

  // We have an active detector method
  // This is used almost solely by the Sidebar navigation
  const active = history && isLinkActive ? (activeClassName || 'active') : '';
  return (
    <Link
      onClick={onClick}
      onContextMenu={onContextMenu}
      className={`${className} ${active}`}
      to={to}
    >
      {children}
    </Link>
  );
};
