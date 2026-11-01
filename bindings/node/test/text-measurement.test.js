'use strict';
const test = require('node:test');
const assert = require('node:assert/strict');
const fs = require('node:fs');
const path = require('node:path');
const { Document } = require('..');
const fixtures = path.resolve(__dirname, '../../../crates/ofdrw/tests/fixtures');

test('read-only text measurement matches font tables and sizes a decorated box', () => {
  const fixture = JSON.parse(fs.readFileSync(path.join(fixtures, 'text-measurement.json')));
  const { spans, content_width: width, ...base } = fixture.request;
  for (const shape of [false, true]) {
    const doc = Document.create();
    try {
      const font = doc.addEmbeddedFont('DejaVu Serif', fs.readFileSync(path.join(fixtures, fixture.font_fixture)));
      const options = { ...base, font_id: String(font), shape };
      const before = doc.toBytes();
      const metrics = doc.measureFlowedSpans(0, spans, width, options);
      assert.deepEqual(metrics, fixture.expected);
      assert.deepEqual(doc.toBytes(), before);
      assert.equal(doc.pageCount, 0);
      assert.deepEqual(doc.measureFlowedSpans(0, spans, width, { ...options,
        page_size: [0, 0], margins: [-1, -1, -1, -1], start: { page_index: 999, y: -1 },
      }), metrics);
      const boxes = doc.addBoxLayout(0, [{ type: 'box', size: [width, metrics.height],
        padding: [1, 1, 1, 1], border: [0.2, 0.2, 0.2, 0.2], integrity: true,
      }], { page_size: [30, 80], margins: [2, 2, 2, 2] });
      const placed = boxes.placements[0];
      const [x, y, w, h] = placed.content_box;
      assert.equal(h, metrics.height);
      assert.deepEqual(doc.addFlowedSpans(0, spans, { ...options,
        page_size: [30, 80], margins: [y, 30-x-w, 80-y-h, x], start: { page_index: 0, y },
      }), [0]);
      const objects = doc.pageTextObjects(0);
      const lines = metrics.lines.filter(line => line.text);
      assert.equal(objects.length, lines.length);
      objects.forEach((object, index) => {
        assert.ok(Math.abs(object.boundary[0] - x - lines[index].x) < 1e-8);
        assert.ok(Math.abs(object.boundary[1] - y - lines[index].y) < 1e-8);
      });
      const reopened = Document.fromBytes(doc.toBytes());
      try {
        assert.deepEqual(reopened.pageText(0), ['A', 'B']);
        assert.deepEqual(reopened.pagePng(0, 144), doc.pagePng(0, 144));
      } finally { reopened.close(); }
    } finally { doc.close(); }
  }
});

test('measurement rejects invalid input without creating pages or changing resources', () => {
  const doc = Document.create();
  try {
    const before = doc.toBytes();
    assert.equal(doc.measureFlowedSpans(0, [{ text: 'A' }], 20).height, 6);
    assert.equal(doc.measureFlowedSpans(0, [{ text: 'A\n\n' }], 20).lines.length, 1);
    for (const width of [0, -1, NaN, Infinity]) {
      assert.throws(() => doc.measureFlowedSpans(0, [{ text: 'A' }], width), RangeError);
    }
    for (const spans of [[], [{ text: 'A', font_id: 'missing' }], [{ text: '\n' }]]) {
      assert.throws(() => doc.measureFlowedSpans(0, spans, 20));
    }
    assert.throws(() => doc.measureFlowedSpans(0, {}, 20), TypeError);
    assert.throws(() => doc.measureFlowedSpans(-1, [{ text: 'A' }], 20), RangeError);
    assert.deepEqual(doc.toBytes(), before);
  } finally { doc.close(); }
  assert.throws(() => doc.measureFlowedSpans(0, [{ text: 'A' }], 20));
});


test('natural zero glyph advances remain zero through measurement JSON and flow', () => {
  const doc = Document.create();
  try {
    const font = doc.addEmbeddedFont('DejaVu Sans', fs.readFileSync(path.join(fixtures, 'dejavu-sans.ttf')));
    const options = {font_id: String(font), font_size: 2, line_height: 3};
    const spans = [{text: '\u0301'}];
    const before = doc.toBytes();
    const measured = doc.measureFlowedSpans(0, spans, 20, options);
    assert.equal(measured.width, 0);
    assert.equal(measured.height, 3);
    assert.equal(measured.lines[0].text, '\u0301');
    assert.deepEqual(doc.toBytes(), before);
    assert.throws(() => doc.measureFlowedSpans(0, [{text: '\u0301', letter_spacing: -0.1}], 20, options));
    assert.deepEqual(doc.toBytes(), before);
    assert.deepEqual(doc.addFlowedSpans(0, spans, options), [0]);
    assert.deepEqual(doc.pageText(0), ['\u0301']);
    assert.ok(doc.pagePng(0, 30).length);
  } finally { doc.close(); }
});


test('inline placeholder metrics match Java without embedded fonts or painted objects', () => {
  const fixture = JSON.parse(fs.readFileSync(path.join(fixtures, 'inline-placeholders-java.json')));
  for (const shape of [false, true]) for (const entry of fixture.cases) {
    const doc = Document.create();
    try {
      const before = doc.toBytes();
      const options = {...entry.style, shape};
      const metrics = doc.measureFlowedSpans(0, entry.spans, entry.content_width, options);
      assert.equal(metrics.lines.length, entry.lines.length, entry.name);
      metrics.lines.forEach((line, index) => {
        const expected = entry.lines[index];
        assert.deepEqual(Object.fromEntries(Object.keys(expected).map(key => [key, line[key]])), expected, entry.name);
      });
      assert.equal(metrics.width, Math.max(...entry.lines.map(line => line.width)));
      assert.equal(metrics.height, entry.lines.reduce((sum, line) => sum+line.height, 0));
      assert.deepEqual(doc.toBytes(), before);
      doc.addFlowedSpans(0, entry.spans, {...options, page_size: [entry.content_width+4, 50], margins: [2,2,2,2]});
      assert.equal(doc.pageCount, 1);
      assert.deepEqual(doc.pageText(0), []);
      assert.deepEqual(doc.pageObjects(0), []);
      const reopened = Document.fromBytes(doc.toBytes());
      try {assert.deepEqual(reopened.pageText(0), []); assert.ok(reopened.pagePng(0, 30).length);}
      finally {reopened.close();}
    } finally {doc.close();}
  }
});

test('inline placeholders reach mixed writers and reject invalid geometry atomically', () => {
  for (const shape of [false, true]) {
    const doc = Document.create();
    try {
      const spans = [{text: '', placeholder_width: 3, font_size: 5}, {text: '', placeholder_width: 2, font_size: 2}];
      const style = {font_size: 2, line_height: 3, shape};
      const options = {page_size: [30,20], margins: [2,2,2,2]};
      const root = {layout: {size: [10,0]}, children: [{type: 'paragraph', spans, style}]};
      doc.addBlankPage(30,20);
      const fixed = doc.addFixedContainers(0, [{position: [2,2], container: root}]);
      const boxes = doc.addTextBoxes(0, [{layout: root.layout, spans, style}], options);
      const flowed = doc.addFlowContainers(0, [root], options);
      for (const placed of [fixed[0], boxes.placements[0], flowed.placements[0]]) {
        assert.deepEqual(placed.content_box, [2,2,10,6]);
      }
      doc.addFlowBlocks(0, [{type: 'paragraph', spans}], {...style, ...options});
      assert.equal(doc.pageCount, 4);
      for (let page=0; page<4; page++) assert.deepEqual(doc.pageObjects(page), []);
      const before = doc.toBytes();
      for (const width of [-1, 27, null, true, 'wide', NaN, Infinity]) {
        const bad = {text: '', placeholder_width: width, font_size: 2};
        assert.throws(() => doc.measureFlowedSpans(0, [bad], 26, style));
        assert.throws(() => doc.addFlowedSpans(0, [...spans, bad], {...style, ...options}));
        assert.deepEqual(doc.toBytes(), before);
      }
      assert.throws(() => doc.addFlowedSpans(0, [{text: 'hidden', placeholder_width: 1}], {...style, ...options}));
      assert.deepEqual(doc.toBytes(), before);
    } finally {doc.close();}
  }
});
