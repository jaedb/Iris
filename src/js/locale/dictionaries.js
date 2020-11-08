import { merge } from 'lodash';
import en from './en.yaml';
import _de from './de.yaml';
import _fr from './fr.yaml';
import _sv from './sv.yaml';
import _nl from './nl.yaml';
import _ja from './ja.yaml';
import _pl from './pl.yaml';

// Merge languages with English. This provides English fallbacks to untranslated
// fields without breaking the UI (with blanks)
const de = {};
merge(de, en, _de);
const fr = {};
merge(fr, en, _fr);
const sv = {};
merge(sv, en, _sv);
const nl = {};
merge(nl, en, _nl);
const ja = {};
merge(ja, en, _ja);
const pl = {};
merge(pl, en, _pl);

const available = [
  { key: 'en', name: en.name },
  { key: 'de', name: de.name },
  { key: 'fr', name: fr.name },
  { key: 'ja', name: ja.name },
  { key: 'nl', name: nl.name },
  { key: 'pl', name: pl.name },
  { key: 'sv', name: sv.name },
];

export default {
  en,
  de,
  fr,
  sv,
  nl,
  ja,
  pl,
  available,
};

export {
  en,
  de,
  fr,
  sv,
  nl,
  ja,
  pl,
  available,
};
