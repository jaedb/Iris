import React from 'react';
import ErrorMessage from './ErrorMessage';

export default class ErrorBoundary extends React.Component {
  constructor(props) {
    super(props);
    this.state = {
      hasError: false,
      error: null,
      info: {},
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

  render = () => {
    const { hasError, info: { componentStack } = {} } = this.state;
    const { children, silent } = this.props;

    if (hasError) {
      if (silent) return null;
      return (
        <ErrorMessage type="error-boundary">
          {componentStack && <pre className="error-message__trace">{componentStack}</pre>}
        </ErrorMessage>
      );
    }
    return children;
  }
}
