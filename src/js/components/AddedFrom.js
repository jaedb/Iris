import React, { useEffect } from 'react';
import { useSelector, useDispatch } from 'react-redux';
import URILink from './URILink';
import { I18n, i18n } from '../locale';
import {
  getFromUri,
  titleCase,
  uriType,
} from '../util/helpers';
import {
  makeItemSelector,
  makeLoadingSelector,
} from '../util/selectors';
import { loadUri } from '../services/core/actions';

export default ({
  by,
  uri,
  className = '',
  inline,
}) => {
  if (!uri) return null;

  const dispatch = useDispatch();
  const from = useSelector(makeItemSelector(uri));
  const loading = useSelector(makeLoadingSelector([`(.*)${uri}(.*)`]));

  useEffect(() => {
    if (uri && !from && !loading) {
      dispatch(loadUri(uri));
    }
  }, []);

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
      link = (
        <URILink type={from?.type} uri={uri}>
          <I18n path="library.browse.title" />
        </URILink>
      );
      break;

    case 'search':
      link = (
        <URILink type={from?.type} uri={uri}>
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
      link = <URILink type={type} uri={uri}>{from?.name || titleCase(type)}</URILink>;
  }

  if (inline) {
    return (
      <div className={className}>
        {i18n('specs.added_from')}
        {link}
      </div>
    );
  }

  return (
    <div className={`${className} tooltip`}>
      {link}
      <span className="tooltip__content">
        {i18n('specs.added_from_by', { by, name: type })}
      </span>
    </div>
  );
};
