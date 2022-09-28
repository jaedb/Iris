import React from 'react';
import URILink from './URILink';
import { I18n } from '../locale';
import {
  getFromUri,
  titleCase,
  uriType,
} from '../util/helpers';

export default ({
  by,
  from,
  className = '',
  inline,
}) => {
  if (!from) return null;
  const { uri, name } = from;
  const type = from?.type || uriType(uri);
  let link = null;
  switch (type) {
    case 'discover':
      link = (
        <URILink type="recommendations" uri={getFromUri('seeds', uri)}>
          <I18n path="discover.title" />
        </URILink>
      );
      break;

    case 'browse':
      let directory = '';
      if (uri.indexOf('file://') > -1) {
        directory = decodeURIComponent(uri.substr(uri.lastIndexOf('/'), uri.length));
      }
      link = (
        <URILink type={type} uri={uri} suffix={directory}>
          <I18n path="library.browse.title" />
        </URILink>
      );
      break;

    case 'search':
      link = (
        <URILink type={type} uri={uri}>
          <I18n path="search.title" />
        </URILink>
      );
      break;

    case 'radio':
      link = <I18n path="modal.edit_radio.title" />;
      break;

    case 'queue-history':
      link = <I18n path="queue_history.title" />;
      break;

    default:
      link = <URILink type={type} uri={uri}>{name || titleCase(type)}</URILink>;
  }

  if (inline) {
    return (
      <div className={className}>
        <I18n path="specs.added_from" />
        {link}
      </div>
    );
  }

  return (
    <div className={`${className} tooltip`}>
      {link}
      <span className="tooltip__content">
      <I18n path="specs.added_by" params={{ by }} />
      </span>
    </div>
  );
};
