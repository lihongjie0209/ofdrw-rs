'use strict';

const test = require('node:test');
const assert = require('node:assert/strict');
const fs = require('node:fs');
const path = require('node:path');
const { Document } = require('../index.js');
const fixtures = path.resolve(__dirname, '../../../crates/ofdrw/tests/fixtures');

test('mixed flow composes paragraphs, images, tables, and a reusable cursor', () => {
  const { request, expected } = JSON.parse(fs.readFileSync(path.join(fixtures, 'flow-blocks.json')));
  const { blocks, ...options } = request;
  for (const shape of [false, true]) {
    const doc = Document.create();
    try {
      const font = doc.addEmbeddedFont('DejaVu Serif', fs.readFileSync(path.join(fixtures, 'dejavu-serif.ttf')));
      const result = doc.addFlowBlocks(0, blocks, { ...options, font_id: String(font), shape });
      assert.deepEqual(result, expected);
      assert.deepEqual(doc.pageText(0), ['A', 'R1']);
      assert.deepEqual(doc.pageText(1), ['R2', 'B']);
      assert.deepEqual(doc.pageImages(0)[0].boundary, [7, 6, 6, 4]);
      assert.deepEqual(doc.validateReferences(), []);
      const continued = doc.addFlowBlocks(0, [{ type: 'paragraph', text: 'C' }], {
        ...options, font_id: String(font), shape, start: result.next,
      });
      assert.deepEqual(continued, { pages: [1], next: { page_index: 1, y: 13 } });
      const reopened = Document.fromBytes(doc.toBytes());
      try {
        assert.deepEqual(reopened.pageText(1), ['R2', 'B', 'C']);
        assert.deepEqual(reopened.pagePng(0, 72), doc.pagePng(0, 72));
      } finally { reopened.close(); }
    } finally { doc.close(); }
  }
});

test('mixed flow rejects invalid content atomically and preserves explicit blank pages', () => {
  const { request } = JSON.parse(fs.readFileSync(path.join(fixtures, 'flow-blocks.json')));
  const { blocks, ...options } = request;
  const doc = Document.create();
  try {
    const before = doc.toBytes();
    for (const invalid of [
      { type: 'paragraph', text: 'x', style: { font_size: 0 } },
      { type: 'paragraph', text: 'x', style: { page_size: [20, 20] } },
      { type: 'image', data_base64: '', format: 'PNG', width: 2, height: 2 },
      { type: 'spacer', height: 1e308 },
      { type: 'paragraph', text: 'x', spans: [] },
    ]) {
      assert.throws(() => doc.addFlowBlocks(0, [...blocks, invalid], options));
      assert.deepEqual(doc.toBytes(), before);
    }
    assert.throws(() => doc.addFlowBlocks(-1, blocks));
    assert.throws(() => doc.addFlowBlocks(0, {}), TypeError);
    assert.deepEqual(doc.addFlowBlocks(0, []), { pages: [], next: null });
    assert.deepEqual(doc.addFlowBlocks(0, [
      { type: 'paragraph', text: 'A', style: { font_size: 4 } },
      { type: 'spacer', height: 2 }, { type: 'paragraph', text: 'B' },
      { type: 'page_break' }, { type: 'page_break' },
    ], options), { pages: [0, 1, 2], next: { page_index: 2, y: 2 } });
    assert.deepEqual(doc.pageTextObjects(0)[1].boundary.slice(0, 2), [2, 10]);
    assert.deepEqual(doc.pageText(1), []);
  } finally { doc.close(); }
  assert.throws(() => doc.addFlowBlocks(0, blocks));
});
