'use strict';
const test = require('node:test');
const assert = require('node:assert/strict');
const fs = require('node:fs');
const path = require('node:path');
const {Document} = require('..');
const root = path.resolve(__dirname, '../../../crates/ofdrw/tests/fixtures');
const cases = JSON.parse(fs.readFileSync(path.join(root, 'canvas-font-style-java.json'), 'utf8')).cases;

test('font shorthand matches 986 Java cases without changing resources or caller settings', () => {
  assert.equal(cases.length, 986);
  const doc = Document.create();
  try {
    doc.addBlankPage(40, 30);
    const ids = Object.fromEntries(['CanvasTest', '宋体'].map(name =>
      [name, String(doc.addEmbeddedFont(name, fs.readFileSync(path.join(root, 'dejavu-sans.ttf'))))]));
    const before = doc.toBytes();
    for (const entry of cases) {
      const base = {weight: entry.base_weight, italic: entry.base_italic,
        letter_spacing: 0.25, read_direction: 90, char_direction: 270, align: 'end'};
      const saved = structuredClone(base);
      const font = doc.resolveCanvasFontStyle(0, entry.input, base, entry.pixels_per_mm);
      assert.ok(Math.abs(font.size - entry.expected.size) < 1e-12, entry.input);
      assert.deepEqual(font, {...base, size: font.size, font_id: ids[entry.expected.family],
        weight: entry.expected.weight, italic: entry.expected.italic});
      assert.deepEqual(base, saved);
    }
    assert.deepEqual(doc.toBytes(), before);
    const font = doc.resolveCanvasFontStyle(0, 'bold 4mm CanvasTest');
    doc.drawCanvas(0, [0, 0, 40, 30], [{type: 'set_font', font},
      {type: 'fill_text', text: 'Canvas', point: [2, 10]}]);
    const reopened = Document.fromBytes(doc.toBytes());
    try {
      assert.deepEqual(reopened.validateReferences(), []);
      assert.deepEqual(reopened.pagePng(0, 101.6), doc.pagePng(0, 101.6));
    } finally {reopened.close();}
  } finally {doc.close();}
});

test('font shorthand rejects invalid requests atomically', () => {
  const doc = Document.create();
  try {
    doc.addBlankPage(40, 30);
    const before = doc.toBytes();
    for (const args of [[0, '0mm New'], [0, '4mm New', {}, 0],
      [0, '4mm New', {align: 'bad'}], [0, '4mm New', {size: -1}],
      [0, '4mm New\0'], [0, 12], [0, '4mm New', []], [-1, '4mm New'], [1, '4mm New']]) {
      assert.throws(() => doc.resolveCanvasFontStyle(...args));
    }
    assert.deepEqual(doc.toBytes(), before);
    assert.deepEqual(doc.resolveCanvasFontStyle(0, '4mm New'), doc.resolveCanvasFontStyle(0, '4mm New'));
  } finally {doc.close();}
  assert.throws(() => doc.resolveCanvasFontStyle(0, '4mm New'));
});
