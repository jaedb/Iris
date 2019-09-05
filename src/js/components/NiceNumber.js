
import React, { memo } from 'react';

export default memo((props) => {
  let formatted = parseInt(props.value);

  // > 1 million
  if (formatted > 1000000) {
    formatted /= 1000000;
    formatted = Math.round(formatted * 10) / 10;
    formatted = `${formatted}m`;

    // > 1 thousand
  } else if (formatted > 1000) {
    formatted /= 1000;
    formatted = Math.round(formatted * 10) / 10;
    formatted = `${formatted}k`;
  } else {
    formatted = formatted.toLocaleString();
  }

  return formatted;
});
