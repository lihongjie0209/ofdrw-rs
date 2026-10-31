'use strict';
const assert = require('node:assert/strict');
const fs = require('node:fs');
const path = require('node:path');
const test = require('node:test');
const { Document } = require('..');

test('PDF action tree keeps execution order and regions', () => {
  const source = fs.readFileSync(path.resolve(__dirname, '../../../crates/ofdrw/tests/fixtures/link-action-tree.pdf'));
  const check = (document) => {
    const annotations = document.annotations();
    assert.equal(annotations.length, 1);
    const actions = annotations[0].actions;
    assert.deepEqual(actions.map((action) => action.kind), ['URI', 'Goto', 'URI', 'URI', 'URI', 'URI']);
    assert.deepEqual(actions.filter((action) => action.kind === 'URI').map((action) => action.attributes.URI), [
      'https://example.org/docs/start', 'https://example.org/docs/shared',
      'https://example.org/last', 'https://example.org/docs/tail', 'https://example.org/docs/shared',
    ]);
    assert.equal(actions[1].destination.page_id, document.pages()[1].id);
    assert.ok(actions[0].region);
    actions.forEach((action) => assert.deepEqual(action.region, actions[0].region));
    assert.deepEqual(document.validateReferences(), []);
  };
  const document = Document.fromPdfBytes(source);
  try {
    check(document);
    const reopened = Document.fromBytes(document.toBytes());
    try {
      check(reopened);
      const exported = Document.fromPdfBytes(reopened.toPdf());
      try { check(exported); } finally { exported.close(); }
    } finally { reopened.close(); }
  } finally { document.close(); }
});
