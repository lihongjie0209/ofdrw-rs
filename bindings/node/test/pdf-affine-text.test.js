"use strict";
const test = require("node:test");
const assert = require("node:assert/strict");
const fs = require("node:fs");
const path = require("node:path");
const {Document} = require("..");
const root = path.resolve(__dirname, "../../../crates/ofdrw/tests/fixtures/pdf-text-affine");
const cases = JSON.parse(fs.readFileSync(path.join(root, "reference.json"), "utf8")).cases;

for (const entry of cases) {
  test(`affine PDF text preserves vectors and semantics: ${entry.name}`, () => {
    const doc = Document.fromPdfBytes(fs.readFileSync(path.join(root, entry.file)));
    try {
      assert.equal(doc.pageText(0).join(""), entry.text);
      assert.deepEqual(doc.pageImages(0), []);
      assert.ok(doc.pagePaths(0).length > 0);
      const png = doc.pagePng(0, 144);
      const reopened = Document.fromBytes(doc.toBytes());
      try {
        assert.deepEqual(reopened.pageText(0), doc.pageText(0));
        assert.deepEqual(reopened.pagePng(0, 144), png);
        assert.deepEqual(reopened.validateReferences(), []);
        const exported = Document.fromPdfBytes(reopened.toPdf());
        try {
          assert.equal(exported.pageText(0).join(""), entry.text);
          assert.deepEqual(exported.validateReferences(), []);
        } finally { exported.close(); }
      } finally { reopened.close(); }
    } finally { doc.close(); }
  });
}
