// Friendlier explanations for HTTP status codes, keyed by status.
const HTTP_STATUS_EXPLANATIONS = {
  0: 'Could not reach the server. Check your network connection, or that the server is running.',
  400: 'The request was invalid or rejected by the server.',
  401: 'You are not authenticated, or your session has expired.',
  403: 'You do not have permission to do that.',
  404: 'The requested resource could not be found. It may have been removed or renamed.',
  408: 'The request timed out.',
  429: 'Too many requests were sent in a short period. Wait a moment and try again.',
  500: 'The server encountered an internal error.',
  502: 'The server is temporarily unavailable (bad gateway).',
  503: 'The server is temporarily unavailable.',
  504: 'The server took too long to respond (gateway timeout).',
};

// Common failure signatures that are technically accurate but meaningless to
// a non-technical reader, mapped to a plain-language explanation of what
// likely happened.
const PATTERN_EXPLANATIONS = [
  {
    pattern: /unexpected end of (json )?input|json\.parse|unexpected token/i,
    explanation: 'The server returned an empty or invalid response. This usually means the request failed partway through, your session expired, or the connection was interrupted.',
  },
  {
    pattern: /failed to fetch|networkerror|network request failed|load failed|err_internet_disconnected|err_connection|err_name_not_resolved/i,
    explanation: 'Could not reach the server. Check your network connection.',
  },
  {
    pattern: /invalid_grant|refresh token expired|grant has been revoked/i,
    explanation: 'Your authorization for this service has expired or been revoked. Try reconnecting it in Settings.',
  },
  {
    pattern: /timed? ?out/i,
    explanation: 'The request took too long and timed out.',
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
    return `${matchedPattern.explanation} (${description})`;
  }

  const status = extractStatus(data);
  if (status !== null && HTTP_STATUS_EXPLANATIONS[status]) {
    return `${HTTP_STATUS_EXPLANATIONS[status]} (${description})`;
  }

  return description;
}

export default humanizeError;
