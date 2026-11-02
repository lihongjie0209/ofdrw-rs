'use strict';
const test = require('node:test');
const assert = require('node:assert/strict');
const fs = require('node:fs');
const path = require('node:path');
const {canvasPixelsToMm, canvasMmToPixels} = require('..');
const cases = JSON.parse(fs.readFileSync(path.resolve(__dirname,
  '../../../crates/ofdrw/tests/fixtures/canvas-units-java.json'), 'utf8')).cases;
const fromBits = value => Buffer.from(value, 'hex').readDoubleBE();

test('Canvas unit conversions match all 329 Java integer and float-bit references', () => {
  assert.equal(cases.length, 329);
  for (const entry of cases) {
    const ppm = fromBits(entry.ppm_bits);
    if (entry.op === 'mm') {
      const encoded = Buffer.alloc(8);
      encoded.writeDoubleBE(canvasPixelsToMm(entry.input, ppm));
      assert.equal(encoded.toString('hex'), entry.expected_bits);
    } else {
      assert.equal(canvasMmToPixels(fromBits(entry.input_bits), ppm), entry.expected);
    }
  }
  assert.equal(canvasPixelsToMm(378), 100);
  assert.equal(canvasMmToPixels(100), 378);
});

test('Canvas unit conversions reject invalid densities, types and integer range', () => {
  for (const ppm of [0, -0, -1, NaN, Infinity, -Infinity, '4', null]) {
    assert.throws(() => canvasPixelsToMm(4, ppm));
    assert.throws(() => canvasMmToPixels(4, ppm));
  }
  for (const pixels of [-2147483649, 2147483648, 1.5, NaN, Infinity, '1', true, null]) {
    assert.throws(() => canvasPixelsToMm(pixels));
  }
  for (const mm of ['1', null, true]) assert.throws(() => canvasMmToPixels(mm));
});
