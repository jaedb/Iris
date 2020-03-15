
import React from 'react';
import { withRouter } from 'react-router';
import { Link } from 'react-router-dom';
import { scrollTo } from '../util/helpers';

/**
 * Extends react-router's Link but provides the ability to hook in to the navigation event
 * which lets us scroll to the top of our <main> for a more traditional navigation experience
 * */
class CustomLink extends React.Component {
  handleClick = () => {
    const {
      location: {
        state,
      },
      history,
      retainScroll,
      scrollTo: scrollToProp,
    } = this.props;

    // Fetch the current scroll position of our #main element and save to our history's state, so
    // clicking 'back' etc will restore the previous scroll position.
    // This doesn't trigger lazy-load elements (unless scrolling exposes the LazyLoader component).
    const main = document.getElementById('main');

    history.replace({
      ...state,
      scroll_position: main.scrollTop,
    });

    // Allow a link to disable auto-scrolling to the top of the page
    // on navigation. Useful for tabs, etc.
    if (!retainScroll) scrollTo(scrollToProp, (scrollToProp));
  }

  handleContextMenu = (e) => {
    const { onContextMenu } = this.props;

    if (onContextMenu) onContextMenu(e);
  }

  isLinkActive = (link) => {
    const {
      exact,
      history: {
        location: {
          pathname,
        } = {},
      } = {},
    } = this.props;

    // Decode both links. This handles issues where one link is encoded and the other isn't, but
    // they're otherwise identical
    link = decodeURIComponent(link);
    const current_link = decodeURIComponent(pathname);

    if (exact) return current_link === link;
    return current_link.startsWith(link);
  }

  render = () => {
    const {
      className,
      activeClassName,
      to,
      children,
      history,
    } = this.props;

    if (!to) return <span className={className}>{children}</span>;

    // We have an active detector method
    // This is used almost solely by the Sidebar navigation
    const active = history && this.isLinkActive(to) ? (activeClassName || 'active') : '';

    return (
      <Link
        onClick={this.handleClick}
        onContextMenu={this.handleContextMenu}
        className={`${className} ${active}`}
        to={to}
      >
        {children}
      </Link>
    );
  }
}

export default withRouter(CustomLink);
