
import React, { memo } from 'react';

export default memo((props) => {
	return (
		<div className={"error-message"+(props.type ? " error-message--"+props.type : "")}>
			
			<i className="error-message__icon icon icon--material">error</i>

			<h4 className="error-message__title">
				{props.title ? props.title : "Unknown error"}	
			</h4>

			<div className="error-message__content">
				{props.children}
			</div>

		</div>
	);
});

