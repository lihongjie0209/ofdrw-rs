'use strict';
const test = require('node:test');
const assert = require('node:assert/strict');
const fs = require('node:fs');
const path = require('node:path');
const { Document } = require('..');
const fixture = () => JSON.parse(fs.readFileSync(path.resolve(__dirname,
  '../../../crates/ofdrw/tests/fixtures/box-layout.json')));

test('box layout returns every fragment field and places text using its content rectangle', () => {
  const { request: { items, ...options }, expected } = fixture();
  const doc = Document.create();
  try {
    const result = doc.addBoxLayout(0, items, options);
    assert.deepEqual(result, expected);
    assert.equal(doc.pagePaths(0).length, 1);
    const continued = doc.addBoxLayout(0, [{ type: 'box', size: [5, 4],
      margin: [0.5, 0.5, 0.5, 0.5], border: [0.25, 0.25, 0.25, 0.25],
      padding: [0.5, 0.5, 0.5, 0.5], background_color: [230, 240, 250], offset: [1, 0],
    }], { ...options, start: result.next });
    const placed = continued.placements[0];
    assert.deepEqual(placed.allocation, [2, 8, 7.5, 6.5]);
    assert.deepEqual(placed.content_box, [4.25, 9.25, 5, 4]);
    doc.addTextObject(placed.page_index, 'Hi', placed.content_box, { size: 2 });
    const reopened = Document.fromBytes(doc.toBytes());
    try {
      assert.deepEqual(reopened.pageText(1), ['Hi']);
      assert.deepEqual(reopened.pagePng(1, 144), doc.pagePng(1, 144));
      assert.deepEqual(reopened.validateReferences(), []);
    } finally { reopened.close(); }
  } finally { doc.close(); }
});

test('box layout defaults, explicit breaks, invalid content and closed documents', () => {
  const doc = Document.create();
  try {
    const before = doc.toBytes();
    for (const invalid of [
      { type: 'box', size: [-1, 3] }, { type: 'box', size: [1, 1e308] },
      { type: 'box', size: [1, 1], float: 'justify' }, { type: 'box' },
      { type: 'line_break', size: [1, 1] }, { type: 'box', size: [1, 1], alpha: 256 },
    ]) {
      assert.throws(() => doc.addBoxLayout(0, [{ type: 'box', size: [4, 4], background_color: [255, 0, 0] }, invalid]));
      assert.deepEqual(doc.toBytes(), before);
    }
    assert.throws(() => doc.addBoxLayout(0, [], { font_size: 2 }));
    for (const alpha of [NaN, Infinity, -Infinity]) {
      assert.throws(() => doc.addBoxLayout(0, [{ type: 'box', size: [1, 1], alpha }]), RangeError);
      assert.deepEqual(doc.toBytes(), before);
    }
    assert.throws(() => doc.addBoxLayout(-1, []), RangeError);
    assert.throws(() => doc.addBoxLayout(0, {}), TypeError);
    assert.deepEqual(doc.addBoxLayout(0, []), { pages: [], placements: [], next: null });
    const result = doc.addBoxLayout(0, [{ type: 'page_break' }, { type: 'page_break' }]);
    assert.deepEqual(result.pages, [0, 1]);
    assert.deepEqual(result.next, { page_index: 1, y: 25.4 });
  } finally { doc.close(); }
  assert.throws(() => doc.addBoxLayout(0, []));
});


test('Java dashed border reference matches boxes, measured text and nested containers', () => {
  const reference = JSON.parse(fs.readFileSync(path.resolve(__dirname,
    '../../../crates/ofdrw/tests/fixtures/box-border-dashes-java.json')));
  for (const kind of ['box', 'text_box', 'container']) for (const entry of reference.cases) {
    const box = {size: [10, 5], border: entry.border, border_dash: entry.dash};
    const options = {page_size: [30, 20], margins: [2, 2, 2, 2]};
    const doc = Document.create();
    try {
      if (kind === 'box') doc.addBoxLayout(0, [{type: 'box', ...box}], options);
      else if (kind === 'text_box') doc.addTextBoxes(0, [{layout: box, spans: [{text: 'A'}],
        style: {font_size: 2, line_height: 5}}], options);
      else doc.addFlowContainers(0, [{layout: {size: [20, 0]}, children: [
        {type: 'container', container: {layout: box, children: [{type: 'paragraph',
          spans: [{text: 'A'}], style: {font_size: 2, line_height: 5}}]}}, {type: 'line_break'},
      ]}], options);
      assert.deepEqual(doc.pagePaths(0).map(p => p.boundary), entry.paths.map(p => p.boundary));
      const xml = doc.entry(doc.pages()[0].path).toString();
      const pattern = entry.paths[0].pattern.join(' ');
      assert.equal(xml.split(`DashPattern="${pattern}"`).length - 1, entry.paths.length);
      const reopened = Document.fromBytes(doc.toBytes());
      try { assert.deepEqual(reopened.pagePaths(0), doc.pagePaths(0)); assert.ok(reopened.pagePng(0, 30).length); }
      finally { reopened.close(); }
      const before = doc.toBytes();
      for (const border_dash of [[0], [-1], [1, 0], [Number.MAX_VALUE], Array(258).fill(1)]) {
        assert.throws(() => doc.addBoxLayout(0, [{type: 'box', ...box, border_dash}], options));
        assert.deepEqual(doc.toBytes(), before);
      }
    } finally { doc.close(); }
  }
});
