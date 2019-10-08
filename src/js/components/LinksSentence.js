
import React, { memo } from 'react';
import URILink from './URILink';

export default memo((props) => {
  if (!props.items) {
    return <span className={props.className ? `${props.className} links-sentence` : 'links-sentence'}>-</span>;
  }

  return (
    <span className={props.className ? `${props.className} links-sentence` : 'links-sentence'}>
      {
				props.items.map((item, index) => {
				  if (!item) {
				    return <span>-</span>;
				  }

				  let separator = null;
				  if (index == props.items.length - 2) {
				    separator = ' and ';
				  } else if (index < props.items.length - 2) {
				    separator = ', ';
				  }

				  if (!item.name) {
				    var content = <span>-</span>;
				  } else if (!item.uri || props.nolinks) {
				    var content = <span>{ item.name }</span>;
				  } else {
				    var content = <URILink className="links-sentence__item links-sentence__item--link" uri={item.uri}>{ item.name }</URILink>;
				  }

				  return (
						<span key={`index_${item.uri}`}>
							{ content }
							{ separator }
						</span>
				  );
				})
			}
    </span>
  );
});
