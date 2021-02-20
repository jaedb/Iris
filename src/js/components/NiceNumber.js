import { memo } from 'react';

const nice_number = (value) => {
  let formatted = parseInt(value, 10);

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
};

const NiceNumber = memo(({ value }) => nice_number(value));

export {
  NiceNumber,
  nice_number,
};

export default NiceNumber;
