import React, { memo } from 'react';
import URILink from './URILink';

const LinksSentence = memo(({ items, className, nolinks }) => {
  if (!items) return <span className={`${className} links-sentence`}>-</span>;

  return (
    <span className={`${className} links-sentence`}>
      {
				items.map(({ name, uri, type }, index) => {
				  if (!name) return <span>-</span>;

				  let separator = null;
				  if (index == items.length - 2) {
				    separator = ' and ';
				  } else if (index < items.length - 2) {
				    separator = ', ';
				  }

				  if (!name) {
				    var content = <span>-</span>;
				  } else if (!uri || nolinks) {
				    var content = <span>{name}</span>;
				  } else {
				    var content = (
  <URILink
    className="links-sentence__item links-sentence__item--link"
    uri={uri}
    type={type}
  >
    {name}
  </URILink>
				    );
				  }

				  return (
  <span key={`index_${uri}`}>
    {content}
    {separator}
  </span>
				  );
				})
			}
    </span>
  );
});

export default LinksSentence;
