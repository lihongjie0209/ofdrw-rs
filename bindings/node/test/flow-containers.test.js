'use strict';
const test = require('node:test');
const assert = require('node:assert/strict');
const fs = require('node:fs');
const path = require('node:path');
const { Document } = require('../index.js');
const fixtures = path.resolve(__dirname, '../../../crates/ofdrw/tests/fixtures');

function typography(children, font, shape) {
  for (const child of children) {
    if (child.type === 'paragraph') Object.assign(child.style, {font_id: String(font), shape});
    if (child.type === 'table') child.table_style.font_id = String(font);
    if (child.type === 'container') typography(child.container.children, font, shape);
  }
}

test('nested mixed containers return complete fragments and draw all child kinds', () => {
  for (const mode of [{shape: false, dynamic: false}, {shape: true, dynamic: false}, {shape: false, dynamic: true}, {shape: true, dynamic: true}, {shape: false, dynamic: false, repeat: true}]) {
    const {shape, dynamic, repeat} = mode;
    const {request, expected: normalExpected, text: normalText, image_boundary, header_expected, header_text} = JSON.parse(fs.readFileSync(path.join(fixtures, 'flow-containers.json')));
    const {containers, ...options} = request;
    const doc = Document.create();
    try {
      const font = doc.addEmbeddedFont('DejaVu Sans', fs.readFileSync(path.join(fixtures, 'dejavu-sans.ttf')));
      typography(containers[0].children, font, shape);
      if (dynamic) containers[0].children[1].container.layout.clear = "none";
      if (repeat) containers[0].children[3].table_style.repeat_header = true;
      const expected = repeat ? header_expected : normalExpected;
      const text = repeat ? header_text : normalText;
      const last = expected.pages.at(-1);
      const before = structuredClone(containers);
      const result = doc.addFlowContainers(0, containers, options);
      assert.deepEqual(result, expected);
      assert.deepEqual(containers, before);
      text.forEach((expected, page) => assert.deepEqual(doc.pageText(page), expected));
      assert.deepEqual(doc.pageImages(1)[0].boundary, image_boundary);
      const reopened = Document.fromBytes(doc.toBytes());
      try { assert.deepEqual(reopened.pageText(2), text[2]); assert.ok(reopened.pagePng(1, 30).length); }
      finally { reopened.close(); }
      const extra = {layout: {size: [4, 0]}, children: [{type: 'paragraph', spans: [{text: 'Q'}], style: containers[0].children[0].style}]};
      assert.deepEqual(doc.addFlowContainers(0, [extra], {...options, start: result.next}).pages, [last]);
      assert.deepEqual(doc.pageText(last), [...text[last], 'Q']);
    } finally { doc.close(); }
  }
});

test('invalid container requests leave the document unchanged', () => {
  const root = {layout: {size: [12, 0]}, children: []};
  const doc = Document.create();
  try {
    const before = doc.toBytes();
    for (const child of [{}, {type: 'unknown'}, {type: 'paragraph', spans: []},
      {type: 'spacer', height: -1}, {type: 'spacer', extra: true},
      {type: 'container', container: {layout: {size: [4, 0]}, children: false}}]) {
      assert.throws(() => doc.addFlowContainers(0, [{...root, children: [child]}]));
      assert.deepEqual(doc.toBytes(), before);
    }
    for (const value of [NaN, Infinity, -Infinity]) assert.throws(() => doc.addFlowContainers(0, [{...root, gap: value}]));
    assert.throws(() => doc.addFlowContainers(0, {}));
    assert.throws(() => doc.addFlowContainers(-1, [root]));
  } finally { doc.close(); }
  assert.throws(() => doc.addFlowContainers(0, [root]));
});


test('container controls preserve explicit blank pages and roundtrip in plain and shaped text', () => {
  for (const shape of [false, true]) {
    const fixture = JSON.parse(fs.readFileSync(path.join(fixtures, 'container-controls.json')));
    const {containers, ...options} = fixture.request;
    const doc = Document.create();
    try {
      const font = doc.addEmbeddedFont('DejaVu Sans', fs.readFileSync(path.join(fixtures, 'dejavu-sans.ttf')));
      typography(containers[0].children, font, shape);
      const before = structuredClone(containers);
      assert.deepEqual(doc.addFlowContainers(0, containers, options), fixture.expected);
      assert.deepEqual(containers, before);
      const reopened = Document.fromBytes(doc.toBytes());
      try {
        fixture.text.forEach((expected, page) => {
          assert.deepEqual(doc.pageText(page), expected);
          assert.deepEqual(reopened.pageText(page), expected);
        });
        assert.ok(reopened.pagePng(1, 30).length);
      } finally { reopened.close(); }
      const encoded = doc.toBytes();
      for (const type of ['line_break', 'page_area_fill']) {
        const invalid = structuredClone(containers);
        invalid[0].children.push({type, height: 1});
        assert.throws(() => doc.addFlowContainers(0, invalid, options));
        assert.deepEqual(doc.toBytes(), encoded);
      }
    } finally { doc.close(); }
  }
});
