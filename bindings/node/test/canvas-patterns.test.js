'use strict';
const test = require('node:test');
const assert = require('node:assert/strict');
const fs = require('node:fs');
const path = require('node:path');
const {Document} = require('..');
const root = path.resolve(__dirname, '../../../crates/ofdrw/tests/fixtures');
const cases = JSON.parse(fs.readFileSync(path.join(root, 'canvas-pattern-java/reference.json'), 'utf8')).cases;
const source = fs.readFileSync(path.join(root, 'canvas-images-java/source.png'));
for (const entry of cases) {
  test(`Canvas image pattern matches Java writer: ${entry.name}`, () => {
    const doc = Document.create(), java = Document.open(path.join(root, `canvas-pattern-java/${entry.name}.ofd`));
    try {
      doc.addBlankPage(40, 30);
      doc.setPageArea(0, {physical_box: [0, 0, 40, 30], application_box: [0, 0, 40, 30]});
      const options = {repetition: entry.repetition, pixels_per_mm: entry.ppm};
      if (entry.size) options.image_size = entry.size;
      if (entry.transform) options.ctm = entry.transform;
      const savedOptions = structuredClone(options);
      const pattern = doc.createCanvasPattern(0, source, 'png', options);
      assert.deepEqual(options, savedOptions);
      assert.equal(pattern.width, entry.expected.width);
      assert.equal(pattern.height, entry.expected.height);
      assert.deepEqual(pattern.cells[0].ctm, entry.expected.image_ctm);
      assert.deepEqual(pattern.cells[0].boundary, entry.expected.image_boundary);
      assert.deepEqual(doc.pagePaths(0), []); assert.deepEqual(doc.pageImages(0), []);
      const style = entry.stroke ? {stroke_pattern: pattern, line_width: 1.2} : {fill_pattern: pattern};
      const commands = [{type: 'set_draw_param', style}];
      if (entry.canvas_ctm) commands.push({type: 'set_transform', matrix: entry.canvas_ctm});
      if ('alpha' in entry) commands.push({type: 'set_global_alpha', alpha: entry.alpha});
      commands.push(...(entry.stroke ? [{type: 'rect', rect: [2, 2, 18, 12]}, {type: 'stroke'}] :
        [{type: 'fill_rect', rect: [2, 2, 18, 12]}]));
      const saved = structuredClone(pattern);
      doc.drawCanvas(0, [4, 3, 24, 20], commands);
      assert.deepEqual(pattern, saved);
      const reopened = Document.fromBytes(doc.toBytes());
      try {
        for (const dpi of [25.4, 101.6]) {
          const png = doc.pagePng(0, dpi);
          assert.deepEqual(png, java.pagePng(0, dpi));
          assert.deepEqual(reopened.pagePng(0, dpi), png);
        }
        assert.deepEqual(reopened.validateReferences(), []);
      } finally {reopened.close();}
    } finally {doc.close(); java.close();}
  });
}
test('Canvas pattern errors preserve resource bytes', () => {
  const doc = Document.create();
  try {
    doc.addBlankPage(40, 30);
    const before = doc.toBytes();
    for (const options of [{repetition: 'repeat-x'}, {pixels_per_mm: 0}, {image_size: [0, 4]},
      {image_size: [4]}, {ctm: [1, 2]}, {ctm: [1, 0, 0, 1, NaN, 0]}, {extra: true}]) {
      assert.throws(() => doc.createCanvasPattern(0, source, 'png', options));
      assert.deepEqual(doc.toBytes(), before);
    }
    for (const [data, format] of [[Buffer.from('bad'), 'png'], [Buffer.alloc(0), 'png'], [source, 'jpeg']]) {
      assert.throws(() => doc.createCanvasPattern(0, data, format));
      assert.deepEqual(doc.toBytes(), before);
    }
    assert.throws(() => doc.createCanvasPattern(1, source, 'png'));
    assert.deepEqual(doc.toBytes(), before);
  } finally {doc.close();}
  assert.throws(() => doc.createCanvasPattern(0, source, 'png'));
});
