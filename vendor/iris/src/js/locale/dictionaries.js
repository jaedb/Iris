import { merge } from 'lodash';
import en from './en.yaml';
import _de from './de.yaml';
import _fr from './fr.yaml';
import _sv from './sv.yaml';
import _nl from './nl.yaml';
import _ja from './ja.yaml';
import _pl from './pl.yaml';
import _it from './it.yaml';
import _es from './es.yaml';
import _ru from './ru.yaml';
import _br from './br.yaml';
import _ko from './ko.yaml';

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
const it = {};
merge(it, en, _it);
const es = {};
merge(es, en, _es);
const ru = {};
merge(ru, en, _ru);
const br = {};
merge(br, en, _br);
const ko = {};
merge(ko, en, _ko);

const available = [
  { key: 'en', name: en.name },
  { key: 'es', name: es.name },
  { key: 'de', name: de.name },
  { key: 'fr', name: fr.name },
  { key: 'it', name: it.name },
  { key: 'ja', name: ja.name },
  { key: 'ko', name: ko.name },
  { key: 'nl', name: nl.name },
  { key: 'br', name: br.name },
  { key: 'pl', name: pl.name },
  { key: 'ru', name: ru.name },
  { key: 'sv', name: sv.name },
];

export default {
  en,
  br,
  es,
  de,
  fr,
  sv,
  nl,
  ja,
  pl,
  it,
  ru,
  ko,
  available,
};

export {
  en,
  es,
  de,
  fr,
  sv,
  nl,
  ja,
  pl,
  it,
  ru,
  ko,
  available,
};
