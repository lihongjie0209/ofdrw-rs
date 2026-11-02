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

const imageRoot = path.join(root, '..', 'canvas-images-java');
const imageRequests = JSON.parse(fs.readFileSync(path.join(imageRoot, 'requests.json')));

test('canvas image Buffers, offset Uint8Arrays and base64 match six independent Java references', () => {
  for (const entry of imageRequests.cases) {
    for (const kind of ['buffer', 'uint8', 'base64']) {
      const commands = structuredClone(entry.commands);
      const originals = [];
      for (const command of commands) {
        if (command.data_base64 && kind !== 'base64') {
          const data = Buffer.from(command.data_base64, 'base64');
          const padded = new Uint8Array(data.length + 4);
          padded.set(data, 2);
          command.data = kind === 'buffer' ? data : padded.subarray(2, 2 + data.length);
          delete command.data_base64;
          originals.push([command, command.data, Buffer.from(command.data)]);
        }
        Object.freeze(command);
      }
      const doc = Document.create();
      const java = Document.open(path.join(imageRoot, `${entry.name}.ofd`));
      try {
        doc.addBlankPage(40, 30);
        doc.setPageArea(0, {physical_box: [0, 0, 40, 30], application_box: [0, 0, 40, 30]});
        const ids = doc.drawCanvas(0, imageRequests.boundary, Object.freeze(commands));
        assert.equal(ids.length, 1);
        assert.deepEqual(doc.pageImages(0).map(image => image.id), ids);
        for (const [command, data, before] of originals) {
          assert.equal(command.data, data);
          assert.deepEqual(Buffer.from(data), before);
          assert.equal(Object.hasOwn(command, 'data_base64'), false);
        }
        for (const dpi of [25.4, 101.6]) assert.deepEqual(doc.pagePng(0, dpi), java.pagePng(0, dpi), entry.name);
        const reopened = Document.fromBytes(doc.toBytes());
        try {
          assert.deepEqual(reopened.validateReferences(), []);
          assert.deepEqual(reopened.pagePng(0, 101.6), doc.pagePng(0, 101.6));
        } finally {reopened.close();}
      } finally {doc.close(); java.close();}
    }
  }
});

test('canvas rejects invalid image bytes and conflicting encodings before changing the document', () => {
  const data = fs.readFileSync(path.join(imageRoot, 'source.png'));
  const valid = {type: 'draw_image', data, format: 'png', rect: [1, 1, 6, 4]};
  const doc = Document.create();
  try {
    doc.addBlankPage(40, 30);
    const before = doc.toBytes();
    for (const bad of [
      {...valid, data: 'not bytes'}, {...valid, data: null}, {...valid, data_base64: ''},
      {...valid, data: Buffer.from('bad')},
      {type: 'draw_image_at', data, format: 'png', point: [0, 0], pixels_per_mm: 0},
    ]) {
      assert.throws(() => doc.drawCanvas(0, [0, 0, 40, 30], [valid, bad]));
      assert.deepEqual(doc.toBytes(), before);
    }
  } finally {doc.close();}
});

const textRoot = path.join(root, '..', 'canvas-text-java');
const textRequests = JSON.parse(fs.readFileSync(path.join(textRoot, 'requests.json')));

test('canvas text measurement and painting match all sixteen Java direction combinations', () => {
  const doc = Document.create();
  const java = Document.open(path.join(textRoot, 'directions.ofd'));
  try {
    doc.addBlankPage(90, 90);
    doc.setPageArea(0, {physical_box: [0, 0, 90, 90], application_box: [0, 0, 90, 90]});
    const fontId = doc.addEmbeddedFont('CanvasTest', fs.readFileSync(path.join(textRoot, '..', 'dejavu-sans.ttf')));
    const before = doc.toBytes();
    const commands = [{type: 'translate', offset: [1, 1]}, {type: 'set_global_alpha', alpha: 0.5}];
    for (const entry of textRequests.cases) {
      const font = Object.freeze({...entry.font, font_id: fontId});
      assert.deepEqual(doc.measureCanvasText(0, textRequests.text, font), entry.metrics);
      commands.push({type: 'set_font', font}, {type: 'fill_text', text: textRequests.text, point: entry.point});
    }
    const controls = JSON.parse(fs.readFileSync(path.join(textRoot, 'controls.json')));
    for (const entry of controls.cases) {
      const text = `A${String.fromCodePoint(entry.code)}B`;
      assert.deepEqual(doc.measureCanvasText(0, text, {font_id: fontId, size: 4}), entry.metrics);
    }
    assert.deepEqual(doc.toBytes(), before);
    const snapshot = structuredClone(commands);
    const ids = doc.drawCanvas(0, textRequests.boundary, commands);
    assert.deepEqual(commands, snapshot);
    assert.equal(ids.length, 16);
    assert.deepEqual(doc.pageTextObjects(0).map(item => item.id), ids);
    for (const dpi of [25.4, 101.6]) assert.deepEqual(doc.pagePng(0, dpi), java.pagePng(0, dpi));
    const reopened = Document.fromBytes(doc.toBytes());
    try {
      assert.deepEqual(reopened.validateReferences(), []);
      assert.deepEqual(reopened.pagePng(0, 101.6), doc.pagePng(0, 101.6));
    } finally {reopened.close();}
  } finally {doc.close(); java.close();}
});

test('canvas text defaults, empty text and invalid settings preserve document state', () => {
  const doc = Document.create();
  try {
    doc.addBlankPage(40, 30);
    const before = doc.toBytes();
    assert.deepEqual(doc.measureCanvasText(0, ''), {width: 0, first_offset: [0, 0], offsets: []});
    assert.ok(doc.measureCanvasText(0, 'x').width > 0);
    assert.deepEqual(doc.drawCanvas(0, [0, 0, 40, 30], [{type: 'fill_text', text: '', point: [0, 0]}]), []);
    assert.deepEqual(doc.toBytes(), before);
    for (const font of [{size: 0}, {size: NaN}, {weight: 450}, {align: 'bad'}, {read_direction: 1}, {extra: 1}, {font_id: '99999'}]) {
      assert.throws(() => doc.measureCanvasText(0, 'text', font));
      assert.throws(() => doc.drawCanvas(0, [0, 0, 40, 30], [
        {type: 'fill_text', text: 'first', point: [1, 5]},
        {type: 'set_font', font}, {type: 'fill_text', text: 'last', point: [1, 10]},
      ]));
      assert.deepEqual(doc.toBytes(), before);
    }
    assert.throws(() => doc.measureCanvasText(-1, 'text'), RangeError);
    assert.throws(() => doc.measureCanvasText(0, 1), TypeError);
    assert.throws(() => doc.measureCanvasText(0, 'text', []), TypeError);
  } finally {doc.close();}
  assert.throws(() => doc.measureCanvasText(0, 'text'));
});

test('canvas text areas match all 960 independent Java cases without mutation', () => {
  const textRoot = path.join(root, '../canvas-text-java');
  const cases = JSON.parse(fs.readFileSync(path.join(textRoot, 'areas.json'))).cases;
  assert.equal(cases.length, 960);
  const doc = Document.create();
  try {
    doc.addBlankPage(40, 30);
    const fonts = Object.fromEntries([...new Set(cases.map(entry => entry.font))].map(name =>
      [name, String(doc.addEmbeddedFont(name, fs.readFileSync(path.join(textRoot, '..', name))))]));
    const before = doc.toBytes();
    for (const entry of cases) {
      const font = {font_id: fonts[entry.font], size: 4, letter_spacing: entry.letter_spacing,
        read_direction: entry.read_direction, char_direction: entry.char_direction};
      const snapshot = structuredClone(font);
      const area = doc.measureCanvasTextArea(0, entry.text, font);
      assert.deepEqual(font, snapshot);
      const tolerance = entry.font === 'test-name-cff.otf' ? 0.0001 : 0;
      const count = Math.max(area.char_areas.length, 1);
      assert.equal(area.letter_spacing, entry.effective_letter_spacing);
      for (const key of ['width', 'height']) assert.ok(Math.abs(area[key] - entry[key]) <= tolerance * count);
      assert.equal(area.char_areas.length, entry.char_areas.length);
      area.char_areas.forEach((box, i) => box.forEach((value, j) =>
        assert.ok(Math.abs(value - entry.char_areas[i][j]) <= tolerance)));
    }
    assert.deepEqual(doc.toBytes(), before);
    const reopened = Document.fromBytes(before);
    try {assert.deepEqual(reopened.measureCanvasTextArea(0, 'A😀'), doc.measureCanvasTextArea(0, 'A😀'));}
    finally {reopened.close();}
  } finally {doc.close();}
});

test('canvas text area validates input and document lifetime', () => {
  const doc = Document.create();
  try {
    doc.addBlankPage(40, 30);
    const before = doc.toBytes();
    assert.deepEqual(doc.measureCanvasTextArea(0, ''), {width: 0, height: 0, letter_spacing: 0, char_areas: []});
    assert.equal(doc.measureCanvasTextArea(0, 'A😀').char_areas.length, 2);
    for (const font of [{size: 0}, {extra: 1}, {letter_spacing: NaN}, {letter_spacing: 1e308}, {font_id: '999999'}]) {
      assert.throws(() => doc.measureCanvasTextArea(0, 'ABC', font));
    }
    for (const page of [-1, 1]) assert.throws(() => doc.measureCanvasTextArea(page, 'x'));
    assert.throws(() => doc.measureCanvasTextArea(0, 1), TypeError);
    assert.throws(() => doc.measureCanvasTextArea(0, 'x', []), TypeError);
    assert.deepEqual(doc.toBytes(), before);
  } finally {doc.close();}
  assert.throws(() => doc.measureCanvasTextArea(0, ''));
});
