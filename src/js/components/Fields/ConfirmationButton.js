
import React from 'react';
import { i18n } from '../../locale';

export default class ConfirmationButton extends React.Component {
  constructor(props) {
    super(props);
    this.state = {
      timing_out: false,
      confirming: false,
    };
    this.confirming = false;
    this.unconfirmTimer = false;
  }

  componentWillUnmount() {
    clearTimeout(this.unconfirmTimer);
  }

  handleClick = () => {
    const { confirming } = this.state;
    const { onConfirm } = this.props;

    if (confirming) {
      this.setState({ confirming: false });
      onConfirm();
    } else {
      this.setState({ confirming: true });
    }
  }

  handleMouseEnter = () => {
    this.setState({ timing_out: false });
    clearTimeout(this.unconfirmTimer);
  }

  handleMouseLeave = () => {
    const { confirming } = this.state;

    if (confirming) {
      this.setState({ timing_out: true });
      this.unconfirmTimer = setTimeout(
        () => {
          this.setState({ confirming: false });
        },
        2000,
      );
    }
  }

  render = () => {
    const {
      confirmingContent,
      working,
      workingContent,
      className: classNameProp,
    } = this.props;
    let { content } = this.props;
    const {
      confirming,
      timing_out,
    } = this.state;
    let className = 'button';

    if (confirming) {
      className += ' button--confirming';
      content = confirmingContent || i18n('actions.confirm');
      if (timing_out) {
        className += ' button--timing-out';
      }
    }

    if (working) {
      className += ' button--working';

      if (workingContent) {
        content = workingContent;
      }
    }

    return (
      <button
        className={`${className} ${classNameProp}`}
        onClick={this.handleClick}
        onMouseLeave={this.handleMouseLeave}
        onMouseEnter={this.handleMouseEnter}
        type="button"
      >
        {content}
      </button>
    );
  }
}
