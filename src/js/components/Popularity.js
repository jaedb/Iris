
import React, { memo } from 'react';

export default memo((props) => {
  if (props.popularity === undefined || props.popularity === null) {
    return null;
  }

  return (
    <span className="popularity">
      <span className="popularity-bars">
        <span className={`bar${props.popularity > 10 ? ' filled' : ''}`} />
        <span className={`bar${props.popularity > 30 ? ' filled' : ''}`} />
        <span className={`bar${props.popularity > 50 ? ' filled' : ''}`} />
        <span className={`bar${props.popularity > 70 ? ' filled' : ''}`} />
        <span className={`bar${props.popularity > 90 ? ' filled' : ''}`} />
      </span>
      <span className="popularity-value">
        {props.popularity}
% popularity
      </span>
    </span>
  );
});
