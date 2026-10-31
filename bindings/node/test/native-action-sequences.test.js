'use strict';
const assert = require('node:assert/strict');
const test = require('node:test');
const { Document } = require('..');

for (const overlap of [false, true]) {
  test(`native click actions survive PDF export (overlap=${overlap})`, () => {
    const expected = ['https://example.org/first', 'https://example.org/second'];
    const document = Document.create();
    let data;
    try {
      document.addBlankPage(30, 20);
      const id = String(document.addPathObject(0, 'M 0 0 L 10 0 L 10 5 C', [2, 3, 10, 5]));
      expected.forEach((uri) => document.addPageObjectUriAction(0, id, uri));
      if (overlap) {
        document.setPageObjectActionRegions(0, id, 0, [[2, 3, 6, 4]]);
        document.setPageObjectActionRegions(0, id, 1, [[5, 3, 6, 4]]);
      }
      data = document.toBytes();
    } finally { document.close(); }
    const reopened = Document.fromBytes(data);
    try {
      const imported = Document.fromPdfBytes(reopened.toPdf());
      try {
        const annotations = imported.annotations();
        assert.deepEqual(annotations.map((item) => item.actions.length).sort(), overlap ? [1, 1, 2] : [2]);
        for (const annotation of annotations) {
          const targets = annotation.actions.map((action) => action.attributes.URI);
          if (targets.length === 2) assert.deepEqual(targets, expected);
          else assert.ok(expected.includes(targets[0]));
        }
        assert.deepEqual(imported.validateReferences(), []);
      } finally { imported.close(); }
    } finally { reopened.close(); }
  });
}
