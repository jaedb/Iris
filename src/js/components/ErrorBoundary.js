
import React from 'react';
import ErrorMessage from './ErrorMessage';

export default class ErrorBoundary extends React.Component {
  constructor(props) {
    super(props);
    this.state = {
      hasError: false,
      error: null,
      info: null,
    };
  }

  componentDidCatch(error, info) {
    this.setState({
      hasError: true,
      error,
      info,
    });
    console.error(error, info);
  }

  render() {
    if (this.state.hasError) {
      return (
        <ErrorMessage type="error-boundary">
          {this.state.info ? <pre className="error-message__trace">{this.state.info.componentStack}</pre> : null}
        </ErrorMessage>
      );
    }
    return this.props.children;
  }
}
