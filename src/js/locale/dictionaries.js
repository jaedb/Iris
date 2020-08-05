import { merge } from 'lodash';
import en from './en.yaml';
import _de from './de.yaml';
import _sv from './sv.yaml';

// Merge languages with English. This provides English fallbacks to untranslated
// fields without breaking the UI (with blanks)
const de = {};
merge(de, en, _de);
const sv = {};
merge(sv, en, _sv);

const available = [
  { key: 'en', name: en.name },
  { key: 'de', name: de.name },
  { key: 'sv', name: sv.name },
];

export default {
  en,
  de,
  sv,
  available,
};

export {
  en,
  de,
  sv,
  available,
};
