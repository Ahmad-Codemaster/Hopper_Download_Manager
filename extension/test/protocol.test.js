const test = require('node:test');
const assert = require('node:assert/strict');
const { validateHandoff, redactUrl } = require('../src/protocol');

test('accepts authorized HTTP handoff and redacts query values', () => {
  assert.equal(validateHandoff({ url: 'https://example.test/file?token=secret' }), true);
  assert.equal(redactUrl('https://example.test/file?token=secret'), 'https://example.test/file?token=%5BREDACTED%5D');
});

test('rejects unsupported handoff schemes', () => {
  assert.equal(validateHandoff({ url: 'javascript:alert(1)' }), false);
});
