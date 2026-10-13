function validateHandoff(message) {
  try {
    const url = new URL(message.url);
    return url.protocol === 'http:' || url.protocol === 'https:';
  } catch {
    return false;
  }
}

function redactUrl(value) {
  const url = new URL(value);
  for (const key of url.searchParams.keys()) {
    url.searchParams.set(key, '[REDACTED]');
  }
  return url.toString();
}

module.exports = { validateHandoff, redactUrl };
