'use strict';
const test = require('node:test');
const assert = require('node:assert/strict');
const fs = require('node:fs');
const path = require('node:path');
const {Document} = require('..');
const root = path.resolve(__dirname, '../../../crates/ofdrw/tests/fixtures/canvas-java');
const requests = JSON.parse(fs.readFileSync(path.join(root, 'requests.json')));
const references = JSON.parse(fs.readFileSync(path.join(root, 'reference.json'))).cases;

test('canvas batches match independent Java paths, styles and reference pixels', () => {
  for (const entry of requests.cases) {
    const reference = references.find(ref => ref.name === entry.name);
    const doc = Document.create();
    const java = Document.open(path.join(root, reference.render_reference || `${entry.name}.ofd`));
    try {
      doc.addBlankPage(40, 30);
      doc.setPageArea(0, {physical_box: [0, 0, 40, 30], application_box: [0, 0, 40, 30]});
      const commands = structuredClone(entry.commands);
      const ids = doc.drawCanvas(0, requests.boundary, commands);
      assert.deepEqual(commands, entry.commands);
      assert.equal(ids.length, reference.objects.length);
      assert.ok(ids.every(id => typeof id === 'string'));
      assert.deepEqual(doc.pagePaths(0).map(item => item.id), ids);
      assert.deepEqual(doc.pageObjectStrokeStyles(0).map(item => item.style.line_width), reference.objects.map(item => item.line_width));
      for (const dpi of [25.4, 101.6]) assert.deepEqual(doc.pagePng(0, dpi), java.pagePng(0, dpi), entry.name);
      const reopened = Document.fromBytes(doc.toBytes());
      try {
        assert.deepEqual(reopened.validateReferences(), []);
        assert.deepEqual(reopened.pagePng(0, 101.6), doc.pagePng(0, 101.6));
      } finally {reopened.close();}
    } finally {doc.close(); java.close();}
  }
});

test('canvas preserves zero width and opacity, clears opacity and returns no IDs for an empty batch', () => {
  const doc = Document.create();
  try {
    doc.addBlankPage(40, 30);
    const before = doc.toBytes();
    assert.deepEqual(doc.drawCanvas(0, [0, 0, 40, 30], []), []);
    assert.deepEqual(doc.toBytes(), before);
    const ids = doc.drawCanvas(0, [0, 0, 40, 30], [
      {type: 'set_draw_param', style: {line_width: 0, cap: 'round', dash_sequence: [1, 2, 3], stroke: {color: [0, 0, 0]}}},
      {type: 'rotate', angle: 0}, {type: 'rect', rect: [2, 2, 12, 8]},
      {type: 'set_global_alpha', alpha: 0}, {type: 'stroke'},
      {type: 'set_global_alpha', alpha: null}, {type: 'stroke'},
      {type: 'set_global_alpha'}, {type: 'fill', even_odd: true},
    ]);
    assert.equal(ids.length, 3);
    assert.deepEqual(doc.pageObjects(0).map(item => item.alpha), [0, 255, 255]);
    assert.ok(doc.pageObjectStrokeStyles(0).every(item => item.style.line_width === 0));
    assert.match(doc.pageSvg(0), /fill-rule="evenodd"/);
  } finally {doc.close();}
});

test('canvas rejects unknown fields, invalid geometry and nonfinite opacity atomically', () => {
  const doc = Document.create();
  try {
    doc.addBlankPage(40, 30);
    const before = doc.toBytes();
    for (const bad of [
      {type: 'unknown'}, {type: 'stroke', extra: 1}, {type: 'begin_path', point: [1, 2]},
      {type: 'move_to', point: [1]}, {type: 'move_to', point: [1, 2], extra: true},
      {type: 'set_line_width', width: -1}, {type: 'set_global_alpha', alpha: 1.1},
      {type: 'set_global_alpha', alpha: NaN}, {type: 'set_global_alpha', alpha: Infinity},
      {type: 'set_draw_param', style: {extra: true}}, {type: 'fill', even_odd: 0},
    ]) {
      assert.throws(() => doc.drawCanvas(0, [0, 0, 40, 30], [{type: 'rect', rect: [1, 1, 4, 4]}, {type: 'stroke'}, bad]));
      assert.deepEqual(doc.toBytes(), before);
    }
    assert.throws(() => doc.drawCanvas(0, [0, 0, 40, 30], {}), TypeError);
    assert.throws(() => doc.drawCanvas(-1, [0, 0, 40, 30], []));
    assert.throws(() => doc.drawCanvas(9, [0, 0, 40, 30], []));
    assert.throws(() => doc.drawCanvas(0, [0, 0, 0, 30], []));
  } finally {doc.close();}
  assert.throws(() => doc.drawCanvas(0, [0, 0, 40, 30], []));
});
