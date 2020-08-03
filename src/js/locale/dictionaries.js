import { merge } from 'lodash';
import { en } from './en.yaml';
import { _de } from './de.yaml';

// Merge languages with English. This provides English fallbacks to untranslated
// fields without breaking the UI (with blanks)

const de = {};
merge(de, en, _de);

const available = [
  { key: 'en', name: en.name },
  { key: 'de', name: de.name },
];

export default {
  en,
  de,
  available,
};

export {
  en,
  de,
  available,
};
