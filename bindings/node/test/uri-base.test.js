'use strict';

const assert = require('node:assert/strict');
const fs = require('node:fs');
const path = require('node:path');
const test = require('node:test');
const { Document } = require('..');

test('PDF catalog URI base survives OFD and PDF round trips', () => {
  const source = fs.readFileSync(path.resolve(__dirname, '../../../crates/ofdrw/tests/fixtures/uri-base-links.pdf'));
  const expected = [
    'https://example.org/资料/页.html?字=值#段',
    'https://example.org/docs/manual.pdf?view=full',
    'https://example.org/docs/manual.pdf?lang=en#section',
    'https://other.example/a',
    'mailto:user@example.org',
    'https://example.org/docs/manual.pdf?lang=en',
  ];
  const check = (document) => {
    assert.deepEqual(document.annotations().map((item) => item.actions[0].attributes.URI), expected);
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
