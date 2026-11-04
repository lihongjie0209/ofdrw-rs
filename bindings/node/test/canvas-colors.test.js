'use strict';
const test = require('node:test');
const assert = require('node:assert/strict');
const fs = require('node:fs');
const path = require('node:path');
const {parseCanvasColor} = require('..');

test('Canvas color helper matches all 461 Java reference cases without a document', () => {
  const source = path.resolve(__dirname, '../../../crates/ofdrw/tests/fixtures/canvas-color-java.json');
  const cases = JSON.parse(fs.readFileSync(source, 'utf8')).cases;
  assert.equal(cases.length, 461);
  for (const entry of cases) {
    const values = entry.expected;
    const expected = values === null ? null : {color: values.slice(0, 3), alpha: values.length === 4 ? values[3] : 255};
    assert.deepEqual(parseCanvasColor(entry.input), expected, entry.input);
  }
});

test('Canvas color helper validates types and preserves invalid-color null results', () => {
  for (const value of [null, 1, Buffer.from('red'), [], 'red\0blue']) {
    assert.throws(() => parseCanvasColor(value), TypeError);
  }
  for (const value of ['rgb(999,0,0)', 'rgba(1,2,3,NaN)', 'x'.repeat(4097)]) {
    assert.equal(parseCanvasColor(value), null);
  }
});
