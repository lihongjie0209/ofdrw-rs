'use strict';
const assert = require('node:assert/strict');
const test = require('node:test');
const { Document } = require('..');

for (const overlap of [false, true]) {
  test(`native Link annotation exports all click actions (overlap=${overlap})`, () => {
    const expected = ['https://example.org/first', 'https://example.org/second'];
    const document = Document.create();
    let data;
    try {
      document.addBlankPage(30, 20);
      document.addUriLinkAnnotation(0, expected[0], [2, 3, 10, 5]);
      const annotationId = document.annotations()[0].id;
      const objectId = document.annotationObjects(0, annotationId)[0].id;
      document.addAnnotationObjectAction(0, annotationId, objectId, {kind: 'uri', uri: 'https://example.org/discard'});
      document.removeAnnotationObjectAction(0, annotationId, objectId, 1);
      document.addAnnotationObjectAction(0, annotationId, objectId, {kind: 'uri', uri: expected[1]});
      document.setAnnotationObjectActionRegions(0, annotationId, objectId, 0, [[1, 1, 2, 2]]);
      document.setAnnotationObjectActionRegions(0, annotationId, objectId, 0, []);
      assert.equal(document.annotationObjectActions(0, annotationId, objectId)[0].region, null);
      if (overlap) {
        document.setAnnotationObjectActionRegions(0, annotationId, objectId, 0, [[2, 3, 6, 4]]);
        document.setAnnotationObjectActionRegions(0, annotationId, objectId, 1, [[5, 3, 6, 4]]);
      }
      assert.deepEqual(document.annotationObjectActions(0, annotationId, objectId).map(a => a.attributes.URI), expected);
      const before = document.toBytes();
      assert.throws(() => document.addAnnotationObjectAction(0, annotationId, objectId, {kind: 'uri', uri: 'x', event: 'PO'}));
      assert.throws(() => document.removeAnnotationObjectAction(0, annotationId, objectId, 99));
      assert.throws(() => document.setAnnotationObjectActionRegions(0, annotationId, objectId, 0, [[0, 0, -1, 2]]));
      assert.deepEqual(document.toBytes(), before);
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
