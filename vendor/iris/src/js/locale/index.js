import React from 'react';
import { useSelector } from 'react-redux';
import get from 'lodash/get';
import dictionaries from './dictionaries';
import { titleCase } from '../util/helpers';

const PARAMS_REG_EXP = '%{(.*?)}';
const paramsRegExp = new RegExp(PARAMS_REG_EXP, 'g');
const languagesAvailable = dictionaries.available;

const translate = (path, params = {}, transform, language) => {
  const dictionary = dictionaries[language || window.language || 'en'] || dictionaries.en;

  let value = get((dictionary), path, '');
  value = value.replace(
    paramsRegExp,
    (replaceText, key) => (params.hasOwnProperty(key) ? params[key] : ''),
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
  const language = useSelector((state) => state.ui.language);
  if (window.language !== language) window.language = language;

  if (!children) {
    return translate(path, params, transform, language);
  }
  return (
    <>
      {!contentAfter && children}
      {translate(path, params, transform, language)}
      {contentAfter && children}
    </>
  );
};

// Prefer usage of I18n *component*, but sometimes you need a raw string, rather than an Object.
// CAUTION: Directly calling i18n relies on window.language already being populated
// TODO: Upgrade as much usage of i18n() to <I18n /> as possible.
const i18n = translate;

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
