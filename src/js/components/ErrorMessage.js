
import React from 'react';

export default class ErrorMessage extends React.Component {

	constructor(props) {
		super(props);
	}

	render() {
		return (
			<div className={"error-message"+(this.props.type ? " error-message--"+this.props.type : "")}>
				
				<i className="error-message__icon icon icon--material">error</i>

				<h4 className="error-message__title">
					{this.props.title ? this.props.title : "Unknown error"}	
				</h4>

				<div className="error-message__content">
					{this.props.children}
				</div>

			</div>
		);
	}
}

