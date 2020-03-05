
import React from 'react';
import ContextMenuTrigger from './ContextMenuTrigger';

export default class Header extends React.Component {
  constructor(props) {
    super(props);

    this.state = {
      expanded: false,
    };
  }

  handleContextMenuTrigger = (e, options) => {
    const {
      title,
      handleContextMenuTrigger,
      uiActions: {
        showContextMenu,
      },
    } = this.props;

    if (handleContextMenuTrigger) return handleContextMenuTrigger(e);

    e.preventDefault();
    const data = {
      e,
      context: 'custom',
      title,
      options,
    };
    showContextMenu(data);
  }

  renderContextMenuTrigger = () => {
    const {
      handleContextMenuTrigger,
      options,
    } = this.props;

    if (!handleContextMenuTrigger && !options) return null;

    return <ContextMenuTrigger onTrigger={(e) => this.handleContextMenuTrigger(e, options)} />;
  }

  renderOptions = () => {
    const {
      handleContextMenuTrigger,
      options,
    } = this.props;

    if (!options && !handleContextMenuTrigger) return null;

    return (
      <div className="header__options">
        {this.renderContextMenuTrigger()}
        <div className="header__options__wrapper">
          {options || null}
        </div>
      </div>
    );
  }

  render = () => {
    const { className, children } = this.props;

    return (
      <header className={className}>
        <h1>
          {children}
        </h1>
        {this.renderOptions()}
      </header>
    );
  }
}
