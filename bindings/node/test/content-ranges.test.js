'use strict';
const test = require('node:test');
const assert = require('node:assert/strict');
const fs = require('node:fs');
const path = require('node:path');
const {Document} = require('..');
const fixtures = path.resolve(__dirname, '../../../crates/ofdrw/tests/fixtures');

test('protected ranges preserve fragment geometry and independent column offsets', () => {
  const fixture = JSON.parse(fs.readFileSync(path.join(fixtures, 'box-content-ranges.json')));
  for (const entry of fixture.cases) {
    const {items, content_ranges: policies, ...options} = entry.request;
    const original = structuredClone({items, policies});
    const doc = Document.create();
    try {
      assert.deepEqual(doc.addBoxLayoutWithContentRanges(0, items, policies, options), entry.expected);
      assert.deepEqual({items, policies}, original);
      const reopened = Document.fromBytes(doc.toBytes());
      try {assert.equal(reopened.pageCount, 2); assert.ok(reopened.pagePng(1, 30).length);}
      finally {reopened.close();}
      const before = doc.toBytes();
      for (const bad of [null, {}, [{source_index: -1, ranges: []}],
        [{source_index: 0, ranges: [[0, 11]]}], [{source_index: 0, ranges: [[3, 2]]}],
        [{source_index: 0, ranges: [[0, 4], [3, 5]]}], [{source_index: 0, ranges: [[0, NaN]]}],
        [{source_index: 0, ranges: [], extra: true}], [{source_index: 0, ranges: []}, {source_index: 0, ranges: []}]]) {
        assert.throws(() => doc.addBoxLayoutWithContentRanges(0, items, bad, options));
        assert.deepEqual(doc.toBytes(), before);
      }
      assert.throws(() => doc.addBoxLayoutWithContentRanges(0, {}, policies));
      assert.throws(() => doc.addBoxLayoutWithContentRanges(-1, items, policies));
      assert.throws(() => doc.addBoxLayoutWithContentRanges(99, items, policies));
    } finally {doc.close();}
    assert.throws(() => doc.addBoxLayoutWithContentRanges(0, items, policies));
  }
});

test('measured lines become protected intervals and empty policies preserve geometric cuts', () => {
  const doc = Document.create();
  const ordinary = Document.create();
  const empty = Document.create();
  try {
    const metrics = doc.measureFlowedSpans(0, [{text: 'A\nB\nC\nD'}], 8, {font_size: 2, line_height: 3});
    const policies = [{source_index: 0, ranges: metrics.lines.map(line => [line.y, line.y+line.height])}];
    const items = [{type: 'box', size: [8, metrics.height]}];
    const options = {page_size: [20,14], margins: [2,2,2,2]};
    const result = doc.addBoxLayoutWithContentRanges(0, items, policies, options);
    assert.deepEqual(result.placements.map(p => p.content_box[3]), [9,3]);
    assert.deepEqual(empty.addBoxLayoutWithContentRanges(0, items, [], options), ordinary.addBoxLayout(0, items, options));
  } finally {doc.close(); ordinary.close(); empty.close();}
});
