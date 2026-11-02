'use strict';
const test = require('node:test');
const assert = require('node:assert/strict');
const fs = require('node:fs');
const path = require('node:path');
const zlib = require('node:zlib');
const {Document} = require('..');
const root = path.resolve(__dirname, '../../../crates/ofdrw/tests/fixtures/canvas-affine-clips-java');
const cases = JSON.parse(fs.readFileSync(path.join(root, 'reference.json'), 'utf8')).cases;

// Controlled reference/output PNGs use non-interlaced 8-bit RGB or opaque RGBA.
function rgbPng(data) {
  assert.deepEqual(data.subarray(0, 8), Buffer.from([137, 80, 78, 71, 13, 10, 26, 10]));
  const width = data.readUInt32BE(16), height = data.readUInt32BE(20);
  assert.equal(data[24], 8);
  assert.ok([2, 6].includes(data[25]));
  assert.equal(data[28], 0);
  const channels = data[25] === 2 ? 3 : 4;
  const compressed = [];
  for (let offset = 8; offset < data.length;) {
    const length = data.readUInt32BE(offset);
    if (data.toString('ascii', offset + 4, offset + 8) === 'IDAT') {
      compressed.push(data.subarray(offset + 8, offset + 8 + length));
    }
    offset += length + 12;
  }
  const raw = zlib.inflateSync(Buffer.concat(compressed));
  const stride = width * channels;
  assert.equal(raw.length, height * (stride + 1));
  const pixels = Buffer.alloc(width * height * 3);
  let previous = Buffer.alloc(stride);
  for (let y = 0; y < height; y++) {
    const start = y * (stride + 1), kind = raw[start];
    assert.ok(kind >= 0 && kind <= 4);
    const row = Buffer.from(raw.subarray(start + 1, start + 1 + stride));
    for (let i = 0; i < stride; i++) {
      const left = i >= channels ? row[i - channels] : 0;
      const above = previous[i], diagonal = i >= channels ? previous[i - channels] : 0;
      let prediction = 0;
      if (kind === 1) prediction = left;
      if (kind === 2) prediction = above;
      if (kind === 3) prediction = Math.floor((left + above) / 2);
      if (kind === 4) {
        const p = left + above - diagonal;
        const a = Math.abs(p - left), b = Math.abs(p - above), c = Math.abs(p - diagonal);
        prediction = a <= b && a <= c ? left : b <= c ? above : diagonal;
      }
      row[i] = (row[i] + prediction) & 255;
    }
    for (let x = 0; x < width; x++) {
      const source = x * channels, destination = (y * width + x) * 3;
      if (channels === 4) assert.equal(row[source + 3], 255);
      row.copy(pixels, destination, source, source + 3);
    }
    previous = row;
  }
  return {width, height, pixels};
}

function assertRasterMatches(actualPng, expectedPng) {
  const actual = rgbPng(actualPng), expected = rgbPng(expectedPng);
  const {width, height} = actual;
  assert.equal(width, expected.width); assert.equal(height, expected.height);
  const offsets = [-width - 1, -width, -width + 1, -1, 0, 1, width - 1, width, width + 1];
  const white = (data, index) => data[index * 3] === 255 && data[index * 3 + 1] === 255 && data[index * 3 + 2] === 255;
  let error = 0, samples = 0;
  for (const [first, second] of [[actual.pixels, expected.pixels], [expected.pixels, actual.pixels]]) {
    for (let y = 1; y < height - 1; y++) for (let x = 1; x < width - 1; x++) {
      const index = y * width + x;
      if (offsets.some(offset => white(first, index + offset))) continue;
      let best = Infinity;
      for (const offset of offsets) {
        let difference = 0;
        for (let c = 0; c < 3; c++) difference += Math.abs(first[index * 3 + c] - second[(index + offset) * 3 + c]);
        best = Math.min(best, difference);
        if (best === 0) break;
      }
      error += best; samples += 3;
    }
  }
  assert.ok(samples > 100 && error / samples < 4, `mean error ${error / samples}`);
  let extra = 0;
  for (let y = 1; y < height - 1; y++) for (let x = 1; x < width - 1; x++) {
    const index = y * width + x;
    if (offsets.every(offset => white(expected.pixels, index + offset)) &&
        actual.pixels.subarray(index * 3, index * 3 + 3).some(value => value < 248)) extra++;
  }
  assert.ok(extra / (width * height) < 0.005, `extra paint ${extra}`);
}

function commandsFor(entry) {
  return structuredClone(entry.commands).flatMap(command => {
    if (command.type === 'circle_arc') Object.assign(command, {start_angle: 0, end_angle: 360, sweep: true});
    if (command.type !== 'fill_rect') return [command];
    const {color, ...rectangle} = command;
    return [{type: 'set_line_width', width: 0.353}, {type: 'set_fill_color', color},
      {type: 'set_stroke_color', color}, rectangle];
  });
}

for (const entry of cases) {
  test(`affine clipping matches independent Java2D geometry: ${entry.name}`, () => {
    const doc = Document.create();
    try {
      doc.addBlankPage(40, 30);
      const commands = commandsFor(entry), saved = structuredClone(commands);
      assert.ok(doc.drawCanvas(0, [4, 3, 28, 22], commands).length > 0);
      assert.deepEqual(commands, saved);
      assert.deepEqual(doc.validateReferences(), []);
      const reopened = Document.fromBytes(doc.toBytes());
      try {
        for (const ppm of [4, 12]) {
          const png = doc.pagePng(0, ppm * 25.4);
          assertRasterMatches(png, fs.readFileSync(path.join(root, `${entry.name}-geometry-${ppm}.png`)));
          assert.deepEqual(reopened.pagePng(0, ppm * 25.4), png);
        }
        assert.deepEqual(reopened.validateReferences(), []);
      } finally {reopened.close();}
    } finally {doc.close();}
  });
}
