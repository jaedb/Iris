
import React from 'react';
import { withRouter } from 'react-router';
import { Link } from 'react-router-dom';

import { scrollTo } from '../util/helpers';

/**
 * Extends react-router's Link but provides the ability to hook in to the navigation event
 * which lets us scroll to the top of our <main> for a more traditional navigation experience
 * */
class CustomLink extends React.Component {
  constructor(props) {
    super(props);
  }

  handleClick(e) {
    // Fetch the current scroll position of our #main element and
    // save to our history's state, so clicking 'back' etc will restore
    // the previous scroll position.
    // Note that this doesn't trigger lazy-load elements (unless scrolling
    // exposes the LazyLoader component).
    const main = document.getElementById('main');
    const state = (this.props.location && this.props.location.state ? this.props.location.state : {});
    state.scroll_position = main.scrollTop;

    this.props.history.replace({ state });

    // Allow a link to disable auto-scrolling to the top of the page
    // on navigation. Useful for tabs, etc.
    if (!this.props.retainScroll) {
      scrollTo(this.props.scrollTo, (this.props.scrollTo));
    }
  }

  handleContextMenu(e) {
    if (this.props.onContextMenu) {
      this.props.onContextMenu(e);
    }
  }

  isLinkActive(link) {
    // Decode both links
    // This handles issues where one link is encoded and the other isn't,
    // but they're otherwise identical
    link = decodeURIComponent(link);
    const current_link = decodeURIComponent(this.props.history.location.pathname);

    if (this.props.exact) {
      return current_link === link;
    }
    return current_link.startsWith(link);
  }

  render() {
    let className = '';
    if (this.props.className) {
      className += this.props.className;
    }

    if (!this.props.to) {
      return <span className={className}>{this.props.children}</span>;
    }

    // We have an active detector method
    // This is used almost solely by the Sidebar navigation
    if (this.props.history !== undefined) {
      if (this.isLinkActive(this.props.to)) {
        if (this.props.activeClassName) {
          className += ` ${this.props.activeClassName}`;
        } else {
          className += ' active';
        }
      }
    }

    return (
      <Link
        onClick={(e) => this.handleClick(e)}
        onContextMenu={(e) => this.handleContextMenu(e)}
        className={className}
        to={this.props.to}
      >
        {this.props.children}
      </Link>
    );
  }
}

export default withRouter(CustomLink);
