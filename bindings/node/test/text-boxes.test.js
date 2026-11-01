'use strict';
const test = require('node:test');
const assert = require('node:assert/strict');
const fs = require('node:fs');
const path = require('node:path');
const { Document } = require('../index.js');
const fixtures = path.resolve(__dirname, '../../../crates/ofdrw/tests/fixtures');

test('text boxes match complete fragments, draw shaped and plain text and continue', () => {
  for (const shape of [false, true]) {
    const { request, expected } = JSON.parse(fs.readFileSync(path.join(fixtures, 'text-boxes.json')));
    const { boxes, ...options } = request;
    const doc = Document.create();
    try {
      const font = doc.addEmbeddedFont('DejaVu Sans', fs.readFileSync(path.join(fixtures, 'dejavu-sans.ttf')));
      Object.assign(boxes[0].style, {font_id: String(font), shape});
      const original = structuredClone(boxes);
      const result = doc.addTextBoxes(0, boxes, options);
      assert.deepEqual(result, expected);
      assert.deepEqual(boxes, original);
      assert.deepEqual(doc.pageText(0), ['A', 'B', 'C']);
      assert.deepEqual(doc.pageText(1), ['D']);
      const reopened = Document.fromBytes(doc.toBytes());
      try { assert.deepEqual(reopened.pageText(1), ['D']); assert.ok(reopened.pagePng(0, 30).length); }
      finally { reopened.close(); }
      const continued = doc.addTextBoxes(0, [{...boxes[0], spans: [{text: 'E'}]}], {...options, start: result.next});
      assert.deepEqual(continued.pages, [1]);
      assert.deepEqual(doc.pageText(1), ['D', 'E']);
    } finally { doc.close(); }
  }
});

test('invalid text boxes leave the document unchanged', () => {
  const doc = Document.create();
  const box = {layout: {size: [8, 0]}, spans: [{text: 'A'}]};
  try {
    const before = doc.toBytes();
    for (const bad of [{}, {...box, spans: []}, {...box, style: false}, {...box, unknown: true},
      {...box, layout: {size: [0, 0]}}, {...box, layout: {size: [NaN, 0]}},
      {...box, layout: {size: [8, 0], alpha: Infinity}}]) {
      assert.throws(() => doc.addTextBoxes(0, [box, bad]));
      assert.deepEqual(doc.toBytes(), before);
    }
    assert.throws(() => doc.addTextBoxes(0, {}));
    assert.throws(() => doc.addTextBoxes(-1, [box]));
  } finally { doc.close(); }
  assert.throws(() => doc.addTextBoxes(0, [box]));
});
