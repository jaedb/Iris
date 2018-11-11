
import React from 'react';

export default class ErrorBoundary extends React.Component {

	constructor(props) {
		super(props);
		this.state = { 
			hasError: false,
			error: null,
			info: null
		};
	}

	componentDidCatch(error, info){
		this.setState({
			hasError: true,
			error: error,
			info: info
		});
		console.error(error, info);
	}

	render() {
		if (this.state.hasError){
			return (
				<div className="error-boundary">

					<h4 className="error-boundary__title">
						<i className="icon icon--material">error</i>
						{this.state.error ? this.state.error.toString() : "Unknown error"}	
					</h4>

					{this.state.info ? <pre className="error-boundary__trace">{this.state.info.componentStack}</pre> : null}

				</div>
			);
		}
		return this.props.children;
	}
}

