import { i18n } from '../locale';

// Common failure signatures that are technically accurate but meaningless to
// a non-technical reader, mapped to the translation key holding a
// plain-language explanation of what likely happened.
const PATTERN_EXPLANATIONS = [
  {
    pattern: /unexpected end of (json )?input|json\.parse|unexpected token/i,
    key: 'errors.invalid_response',
  },
  {
    pattern: /failed to fetch|networkerror|network request failed|load failed|err_internet_disconnected|err_connection|err_name_not_resolved/i,
    key: 'errors.network_unreachable',
  },
  {
    pattern: /invalid_grant|refresh token expired|grant has been revoked/i,
    key: 'errors.authorization_expired',
  },
  {
    pattern: /timed? ?out/i,
    key: 'errors.request_timeout',
  },
];

const extractStatus = (data = {}) => {
  if (data.xhr && typeof data.xhr.status === 'number') return data.xhr.status;
  if (typeof data.status === 'number') return data.status;
  return null;
};

/**
 * Takes a raw error description (often a bare exception message or HTTP
 * status line) and, where we recognise the shape of the problem, prefixes it
 * with a plain-language explanation of the likely cause. Falls back to the
 * original description untouched when nothing is recognised, so this is
 * always safe to apply.
 */
export function humanizeError(description, data = {}) {
  if (!description || typeof description !== 'string') return description;

  const matchedPattern = PATTERN_EXPLANATIONS.find(({ pattern }) => pattern.test(description));
  if (matchedPattern) {
    const explanation = i18n(matchedPattern.key);
    if (explanation) return `${explanation} (${description})`;
  }

  const status = extractStatus(data);
  if (status !== null) {
    const explanation = i18n(`errors.http_status.${status}`);
    if (explanation) return `${explanation} (${description})`;
  }

  return description;
}

export default humanizeError;
