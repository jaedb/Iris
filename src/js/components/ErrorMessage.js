
import React, { memo } from 'react';

export default memo(({ type, title, children }) => (
  <div className={`error-message${type ? ` error-message--${type}` : ''}`}>
    <i className="error-message__icon icon icon--material">error</i>
    <h4 className="error-message__title">
      {title ? title : 'Unknown error'}
    </h4>
    <div className="error-message__content">
      {children}
    </div>
  </div>
));
