
import React from 'react'

export default class ErrorBoundary extends React.Component {

	constructor(props) {
		super(props);
		this.state = { hasError: false };
	}

	componentDidCatch(error, info) {
		// Display fallback UI
		this.setState({ hasError: true });
		// You can also log the error to an error reporting service
		//logErrorToMyService(error, info);
		console.error(error,info);
	}

	render() {
		if (this.state.hasError){
			// You can render any custom fallback UI
			return <p className="grey-text">Failed to render</p>;
		}
		return this.props.children;
	}
}

