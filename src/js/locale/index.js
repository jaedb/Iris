import React, { Fragment } from 'react';
import get from 'lodash/get';
import dictionaries from './dictionaries';
import { titleCase } from '../util/helpers';

const PARAMS_REG_EXP = '%{(.*?)}';
const paramsRegExp = new RegExp(PARAMS_REG_EXP, 'g');
const languagesAvailable = dictionaries.available;

const i18n = (path, params = {}, transform) => {
  const dictionary = dictionaries[window.language || 'en'] || dictionaries.en;

  let value = get((dictionary), path, '');
  value = value.replace(
    paramsRegExp,
    (replaceText, key) => params.hasOwnProperty(key) ? params[key] : '',
  );

  switch (transform) {
    case 'upper':
      value = value.toUpperCase();
      break;
    case 'lower':
      value = value.toLowerCase();
      break;
    case 'title':
      value = titleCase(value);
      break;
    default:
      break;
  }

  return value;
};

const I18n = ({
  path,
  transform,
  children,
  contentAfter,
  ...params
}) => {
  if (!children) {
    return i18n(path, params, transform);
  }
  return (
    <Fragment>
      {!contentAfter && children}
      {i18n(path, params, transform)}
      {contentAfter && children}
    </Fragment>
  );
};

export default {
  I18n,
  i18n,
  languagesAvailable,
};

export {
  I18n,
  i18n,
  languagesAvailable,
};
