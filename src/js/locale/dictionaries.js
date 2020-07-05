import { merge } from 'lodash';
import en from './en.yaml';
import _de from './de.yaml';

const de = {};
merge(de, en, _de);

export default {
  en,
  de,
};

export {
  en,
  de,
};
