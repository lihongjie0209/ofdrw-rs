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
