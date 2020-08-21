import React, { memo } from 'react';
import Link from './Link';
import { uriType as uriTypeHelper } from '../util/helpers';

export default memo((props) => {
  let to = null;
  let { uri } = props;
  const uriType = uriTypeHelper(uri);
  if (!props.unencoded) {
    uri = encodeURIComponent(uri);
  }

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
      var exploded = uri.split('%3A');
      to = `/search/${exploded[2]}/${exploded[3]}`;
      break;

    default:
      to = null;
  }

  if (uri) {
    return (
      <Link
        className={props.className ? props.className : null}
        to={to}
        onContextMenu={(e) => (props.handleContextMenu ? props.handleContextMenu(e) : null)}
      >
        {props.children}
      </Link>
    );
  }
  return (
    <span className={props.className ? props.className : null}>
      {props.children}
    </span>
  );
});
