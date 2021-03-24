import React from 'react';
import { i18n } from '../../locale';
import Button from '../Button';

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
      onConfirm,
      ...rest
    } = this.props;
    let { content } = this.props;
    const {
      confirming,
      timing_out,
    } = this.state;

    if (confirming) {
      content = confirmingContent || i18n('actions.confirm');
    }

    if (working && workingContent) {
      content = workingContent;
    }

    return (
      <Button
        type="destructive"
        confirming={confirming}
        timingOut={timing_out}
        working={working}
        onClick={this.handleClick}
        onMouseLeave={this.handleMouseLeave}
        onMouseEnter={this.handleMouseEnter}
        {...rest}
      >
        {content}
      </Button>
    );
  }
}
