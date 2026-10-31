'use strict';
const assert = require('node:assert/strict');
const test = require('node:test');
const { Document } = require('..');

const region = (x0, x1) => `<ofd:Region><ofd:Area Start="${x0} 3"><ofd:Line Point1="${x1} 3"/><ofd:Line Point1="${x1} 7"/><ofd:Line Point1="${x0} 7"/><ofd:Close/></ofd:Area></ofd:Region>`;

for (const overlap of [false, true]) {
  test(`native Link annotation exports all click actions (overlap=${overlap})`, () => {
    const expected = ['https://example.org/first', 'https://example.org/second'];
    const document = Document.create();
    let data;
    try {
      document.addBlankPage(30, 20);
      document.addUriLinkAnnotation(0, expected[0], [2, 3, 10, 5]);
      const source = document.annotations()[0].source_path;
      let xml = document.entry(source).toString('utf8');
      if (overlap) xml = xml.replace('<ofd:Action Event="CLICK">', '<ofd:Action Event="CLICK">' + region(2, 8));
      assert.ok(xml.includes('</ofd:Actions>'));
      xml = xml.replace('</ofd:Actions>', `<ofd:Action Event="CLICK">${overlap ? region(5, 11) : ''}<ofd:URI URI="${expected[1]}"/></ofd:Action></ofd:Actions>`);
      document.setEntry(source, Buffer.from(xml, 'utf8'));
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
