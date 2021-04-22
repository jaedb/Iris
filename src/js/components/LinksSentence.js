import React, { memo } from 'react';
import URILink from './URILink';

const LinksSentence = memo(({ items, itemType, className, nolinks }) => {
  if (!items) return <span className={`${className} links-sentence`}>-</span>;

  return (
    <span className={`${className} links-sentence`}>
      {
        items.map(({ name, uri, type }, index) => {
          if (!name) return <span>-</span>;

          let separator = null;
          if (index === items.length - 2) {
            separator = ' and ';
          } else if (index < items.length - 2) {
            separator = ', ';
          }

          let content = null;
          if (!name) {
            content = <span>-</span>;
          } else if (!uri || nolinks) {
            content = <span>{name}</span>;
          } else {
            content = (
              <URILink
                className="links-sentence__item links-sentence__item--link"
                uri={uri}
                type={itemType || type}
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
