import React, { memo } from 'react';
import Link from './Link';
import { uriType as uriTypeHelper } from '../util/helpers';
import { encodeUri } from '../util/format';

export default memo(({
  type,
  uri: rawUri,
  className,
  handleContextMenu,
  children,
  unencoded,
}) => {
  let to = null;
  const uriType = type || uriTypeHelper(rawUri);
  const uri = (!unencoded) ? encodeUri(rawUri) : rawUri;

  switch (uriType) {
    case 'playlist':
      to = `/playlist/${uri}`;
      break;

    case 'artist':
      to = `/artist/${uri}`;
      break;

    case 'album':
      to = `/album/${uri}`;
      break;

    case 'track':
      to = `/track/${uri}`;
      break;

    case 'user':
      to = `/user/${uri}`;
      break;

    case 'browse':
      to = `/library/browse/${uri}`;
      break;

    case 'recommendations':
      to = `/discover/recommendations/${uri}`;
      break;

    case 'search':
      var exploded = uri.split(':');
      to = `/search/${exploded[2]}/${exploded[3]}`;
      break;

    default:
      to = `/uri/${uri}`;
  }

  if (uri) {
    return (
      <Link
        className={className}
        to={to}
        onContextMenu={handleContextMenu}
      >
        {children}
      </Link>
    );
  }
  return (
    <span className={className}>
      {children}
    </span>
  );
});
