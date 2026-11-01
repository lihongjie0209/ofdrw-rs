'use strict';
const test = require('node:test');
const assert = require('node:assert/strict');
const {Document} = require('..');

test('path writers retain zero line width through native rendering and round trips', () => {
  for (const writer of ['addPathObject', 'addCompositePathObject', 'addPathAnnotation']) {
    const doc = Document.create();
    try {
      doc.addBlankPage(20, 20);
      const id = doc[writer](0, 'M 2 5 L 18 5', [0, 0, 20, 20], {line_width: 0});
      if (writer === 'addPathObject') {
        doc.setPageObjectCtm(0, id, [1, 0, 0, 3, 0, 0]);
        assert.equal(doc.pageObjectStrokeStyles(0)[0].style.line_width, 0);
      }
      assert.match(doc.pageSvg(0), /vector-effect="non-scaling-stroke"/);
      assert.match(doc.pageSvg(0), /ofd-device-hairline-/);
      const reopened = Document.fromBytes(doc.toBytes());
      try {
        assert.deepEqual(reopened.validateReferences(), []);
        for (const dpi of [25.4, 101.6, 300]) {
          assert.deepEqual(reopened.pagePng(0, dpi), doc.pagePng(0, dpi));
        }
      } finally {reopened.close();}
    } finally {doc.close();}
  }
});

test('zero width stroke updates preserve dashes and reject negative widths atomically', () => {
  const doc = Document.create();
  try {
    doc.addBlankPage(20, 20);
    const id = doc.addPathObject(0, 'M 2 5 L 18 5', [0, 0, 20, 20]);
    doc.setPagePathStrokeStyle(0, id, {line_width: 0, dash_pattern: [2, 2]});
    assert.equal(doc.pageObjectStrokeStyles(0)[0].style.line_width, 0);
    assert.match(doc.pageSvg(0), /ofd-device-hairline-/);
    const before = doc.toBytes();
    assert.throws(() => doc.setPagePathStrokeStyle(0, id, {line_width: -1}));
    assert.throws(() => doc.addPathObject(0, 'M 2 5 L 18 5', [0, 0, 20, 20], {line_width: -1}));
    assert.deepEqual(doc.toBytes(), before);
  } finally {doc.close();}
});
