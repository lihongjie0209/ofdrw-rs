'use strict';

const assert = require('node:assert/strict');
const fs = require('node:fs');
const os = require('node:os');
const path = require('node:path');
const test = require('node:test');
const zlib = require('node:zlib');
const { Document, OfdError, createSesSeal } = require('..');

const fixture = (name) => path.resolve(__dirname, '../../../crates/ofdrw/tests/fixtures', name);
const javaPlainFixture = path.resolve(__dirname, '../../../../ofdrw-crypto/src/test/resources/hello.ofd');

test('set image interpolation and restore the default', () => {
  const document = Document.open(fixture('image-mask.ofd'));
  try {
    const baseline = document.pageSvg(0);
    for (const [value, hint] of [[false, 'optimizeSpeed'], [true, 'optimizeQuality']]) {
      document.setPageImageInterpolate(0, '5', value);
      assert.ok(document.pageSvg(0).includes(`image-rendering="${hint}"`));
    }
    document.setPageImageInterpolate(0, '5');
    assert.equal(document.pageSvg(0), baseline);
    const before = document.toBytes();
    assert.throws(() => document.setPageImageInterpolate(0, '5', 1), TypeError);
    assert.throws(() => document.setPageImageInterpolate(-1, '5', false), RangeError);
    assert.throws(() => document.setPageImageInterpolate(0, 'missing', false), OfdError);
    assert.deepEqual(document.toBytes(), before);
    document.setPageImageInterpolate(0, '5', false);
    const reopened = Document.fromBytes(document.toBytes());
    try {
      assert.ok(reopened.pageSvg(0).includes('image-rendering="optimizeSpeed"'));
      assert.deepEqual(reopened.validateReferences(), []);
    } finally { reopened.close(); }
  } finally { document.close(); }
  assert.throws(() => document.setPageImageInterpolate(0, '5', false), OfdError);
});

test('retain ActualText through native CFF import and PDF round trips', () => {
  let document = Document.fromPdfBytes(fs.readFileSync(fixture('actual-text-cff.pdf')));
  for (let index = 0; index < 3; index += 1) {
    let pdf;
    try {
      assert.equal(document.pageText(0).join(''), '中文');
      assert.deepEqual(document.validateReferences(), []);
      const reopened = Document.fromBytes(document.toBytes());
      try {
        assert.equal(reopened.pageText(0).join(''), '中文');
        pdf = reopened.toPdf();
      } finally {
        reopened.close();
      }
    } finally {
      document.close();
    }
    if (index < 2) document = Document.fromPdfBytes(pdf);
  }
});

test('import Unicode sequences with original embedded PDF glyphs', () => {
  for (const [name, text, glyphs] of [
    ['type0-ligature-glyph.pdf', ['fiA', 'fi'], [3315, 36]],
    ['type1-opentype-cff.pdf', ['fiA', 'fi'], [2, 1]],
    ['type1-affine-cff.pdf', ['fiA', 'fi'], [2, 1]],
    ['type1-reflected-cff.pdf', ['fiA', 'fi'], [2, 1]],
    ['type1-rotated-cff.pdf', ['fiA', 'fi'], [2, 1]],
    ['type1-opentype-affine-cff.pdf', ['fiA', 'fi'], [2, 1]],
    ['type0-fd-matrices-cff.pdf', ['fiA', 'fi'], [2, 1]],
    ['type0-fd-translated-top-cff.pdf', ['fiA', 'fi'], [2, 1]],
    ['type0-fd-ranges-cff.pdf', ['fiA', 'fi'], [2, 1]],
    ['type0-fd-only-cff.pdf', ['fiA', 'fi'], [2, 1]],
    ['type0-vertical-fd-cff.pdf', ['fiA', 'fi'], [2, 1]],
    ['type1-stroked-affine-cff.pdf', ['fiA', 'fi'], [2, 1]],
    ['type0-clipped-fd-cff.pdf', ['fiA', 'fi'], [2, 1]],
    ['type0-clipped-ligature.pdf', ['fiA', 'fi'], [3315, 36]],
    ['type1-raw-cff.pdf', ['fiA', 'fi'], [2, 1]],
    ['type0-opentype-cid-cff.pdf', ['fiA', 'fi'], [2, 1]],
    ['type0-raw-cid-cff.pdf', ['fiA', 'fi'], [2, 1]],
    ['type0-name-cff.pdf', ['fiA', 'fi'], [2, 1]],
    ['type0-vertical-cid-cff.pdf', ['fiA', 'fi'], [2, 1]],
    ['type1-internal-cff.pdf', ['ﬁA', 'ﬁ'], [2, 1]],
    ['type1-implicit-base-cff.pdf', ['ﬁﬁ', 'ﬁ'], [2, 2]],
    ['type1-dingbats-base-cff.pdf', ['✂✂', '✂'], [2, 2]],
    ['truetype-glyph-name-sequence.pdf', ['fiA', 'fi'], [3315, 36]],
    ['type0-vertical-default.pdf', ['fiA', 'fi'], [3315, 36]],
    ['type0-vertical-metrics.pdf', ['fiA', 'fi'], [3315, 36]],
    ['type0-vertical-transformed.pdf', ['fiA', 'fi'], [3315, 36]],
    ['truetype-ligature-glyph.pdf', ['fiA', 'fi'], [3315, 36]],
    ['type0-no-unicode-cmap-glyph.pdf', ['fiA', 'fi'], [3315, 36]],
    ['type0-variation-glyph.pdf', ['0\ufe00', '0\ufe00'], [19]],
  ]) {
    const document = Document.fromPdfBytes(fs.readFileSync(fixture(name)));
    try {
      assert.deepEqual(document.pageText(0), text);
      const objects = document.pageTextObjects(0);
      assert.deepEqual(objects[0].cg_transforms[0].glyphs, glyphs);
      assert.equal(document.fonts().filter((font) => font.embedded_path !== null).length, 1);
      const reopened = Document.fromBytes(document.toBytes());
      try {
        assert.deepEqual(reopened.pageTextObjects(0), objects);
        assert.deepEqual(reopened.pagePng(0, 72), document.pagePng(0, 72));
        assert.deepEqual(reopened.validateReferences(), []);
      } finally { reopened.close(); }
    } finally { document.close(); }
  }
});

test('decode mixed-length Type0 source codes independently of CID widths', () => {
  for (const [name, text] of [
    ['type0-mixed-cmap.pdf', ['中文']],
    ['type0-four-byte-cmap.pdf', ['中文AB', '中']],
    ['type0-gbk-cmap.pdf', ['中文']],
    ['type0-gbk-vertical-cmap.pdf', ['中文']],
    ['type0-gbk-vertical-collection.pdf', ['中文']],
    ['type0-cns-vertical-cmap.pdf', ['中文']],
    ['type0-cns-vertical-collection.pdf', ['中文']],
    ['type0-jis-vertical-cmap.pdf', ['日本']],
    ['type0-jis-vertical-collection.pdf', ['日本']],
    ['type0-ks-vertical-cmap.pdf', ['한국']],
    ['type0-ks-vertical-collection.pdf', ['한국']],
    ['type0-cns-cmap.pdf', ['中文']],
    ['type0-jis-cmap.pdf', ['日本']],
    ['type0-ks-cmap.pdf', ['한국']],
    ['type0-gbk-collection-unicode.pdf', ['中文']],
    ['type0-cns-collection-unicode.pdf', ['中文']],
    ['type0-jis-collection-unicode.pdf', ['日本']],
    ['type0-ks-collection-unicode.pdf', ['한국']],
    ['type0-identity-collection-unicode.pdf', ['中文']],
    ['type0-embedded-collection-unicode.pdf', ['中文']],
    ['type0-supplementary-collection-unicode.pdf', ['\u{31350}']],
  ]) {
    const document = Document.fromPdfBytes(fs.readFileSync(fixture(name)));
    try {
      assert.deepEqual(document.pageText(0), text);
      const reopened = Document.fromBytes(document.toBytes());
      try {
        assert.deepEqual(reopened.pageText(0), text);
        assert.deepEqual(reopened.pageTextObjects(0), document.pageTextObjects(0));
        assert.deepEqual(reopened.pagePng(0, 72), document.pagePng(0, 72));
        assert.deepEqual(reopened.validateReferences(), []);
      } finally { reopened.close(); }
    } finally { document.close(); }
  }
});

test('preserve named and descriptor PDF font styles through an OFD round trip', () => {
  for (const [name, weight] of [['font-style-standard.pdf', 700], ['font-style-descriptor.pdf', 600]]) {
    const document = Document.fromPdfBytes(fs.readFileSync(fixture(name)));
    try {
      assert.deepEqual(document.pageText(0), ['中文']);
      const objects = document.pageTextObjects(0);
      assert.equal(objects[0].weight, weight);
      assert.equal(objects[0].italic, true);
      const reopened = Document.fromBytes(document.toBytes());
      try {
        assert.deepEqual(reopened.pageTextObjects(0), objects);
        assert.deepEqual(reopened.pagePng(0, 72), document.pagePng(0, 72));
        assert.deepEqual(reopened.validateReferences(), []);
      } finally { reopened.close(); }
    } finally { document.close(); }
  }
});

test('preserve PDF positions with indirect simple widths and the default text origin', () => {
  const direct = Document.fromPdfBytes(fs.readFileSync(fixture('type1-tounicode.pdf')));
  try {
    for (const name of ['simple-indirect-widths.pdf', 'text-default-origin.pdf']) {
      const document = Document.fromPdfBytes(fs.readFileSync(fixture(name)));
      try {
        assert.deepEqual(document.pageText(0), ['中文']);
        assert.deepEqual(document.pageTextObjects(0), direct.pageTextObjects(0));
        assert.deepEqual(document.pagePng(0, 72), direct.pagePng(0, 72));
      } finally { document.close(); }
    }
  } finally { direct.close(); }
});

test('import standard font metrics and symbolic encodings without Widths', () => {
  for (const [name, text, advance] of [
    ['standard14-tounicode.pdf', '中文', 6.804],
    ['standard14-symbol.pdf', 'ΑΒ', 8.664],
    ['standard14-dingbats.pdf', '✁✂', 11.688],
  ]) {
    const document = Document.fromPdfBytes(fs.readFileSync(fixture(name)));
    try {
      assert.deepEqual(document.pageText(0), [text]);
      const delta = Number(document.pageTextObjects(0)[0].codes[0].delta_x);
      assert.ok(Math.abs(delta - advance * 25.4 / 72) < 1e-6);
      const reopened = Document.fromBytes(document.toBytes());
      try {
        assert.deepEqual(reopened.pageText(0), [text]);
        assert.deepEqual(reopened.validateReferences(), []);
      } finally { reopened.close(); }
    } finally { document.close(); }
  }
});

test('decode PDF Differences glyph names without a ToUnicode map', () => {
  for (const name of ['encoding-differences.pdf', 'custom-type1-differences.pdf']) {
    const document = Document.fromPdfBytes(fs.readFileSync(fixture(name)));
    try {
      assert.deepEqual(document.pageText(0), ['中文']);
      const reopened = Document.fromBytes(document.toBytes());
      try {
        assert.deepEqual(reopened.pageText(0), ['中文']);
        assert.deepEqual(reopened.validateReferences(), []);
        assert.deepEqual(reopened.pagePng(0, 72), document.pagePng(0, 72));
      } finally { reopened.close(); }
    } finally { document.close(); }
  }
});

test('import Chinese Type0 text with indirect encoding and CID width metadata', () => {
  const document = Document.fromPdfBytes(fs.readFileSync(fixture('type0-indirect-widths.pdf')));
  const direct = Document.fromPdfBytes(fs.readFileSync(fixture('type0-identity-tounicode.pdf')));
  try {
    assert.deepEqual(document.pageText(0), ['中文']);
    assert.deepEqual(document.pageTextObjects(0), direct.pageTextObjects(0));
    assert.deepEqual(document.pagePng(0, 72), direct.pagePng(0, 72));
    const reopened = Document.fromBytes(document.toBytes());
    try {
      assert.deepEqual(reopened.pageText(0), ['中文']);
      assert.deepEqual(reopened.validateReferences(), []);
    } finally { reopened.close(); }
  } finally { document.close(); direct.close(); }
});

test('save a Node.js document to disk and reopen it', () => {
  const directory = fs.mkdtempSync(path.join(os.tmpdir(), 'ofdrw-node-'));
  const target = path.join(directory, 'saved.ofd');
  const document = Document.create();
  try {
    document.addTextPage('saved from Node.js', 40, 30);
    assert.throws(() => document.save(path.join(directory, 'missing', 'bad.ofd')), OfdError);
    assert.throws(() => document.save('bad\0path'), TypeError);
    document.save(target);
    assert.equal(fs.existsSync(target), true);
    const reopened = Document.open(target);
    try {
      assert.deepEqual(reopened.pageText(0), ['saved from Node.js']);
      assert.deepEqual(reopened.validateReferences(), []);
    } finally {
      reopened.close();
    }
  } finally {
    document.close();
    fs.rmSync(directory, {recursive: true, force: true});
  }
});

function assertSameEntries(actual, expected) {
  assert.deepEqual(actual.entries(), expected.entries());
  for (const name of expected.entries()) assert.deepEqual(actual.entry(name), expected.entry(name), name);
}

test('package header round trip', () => {
  const document = Document.create();
  try {
    assert.deepEqual(document.packageInfo(), { version: '1.2', doc_type: 'OFD' });
    document.setPackageHeader('1.3 & beta', 'OFD-A');
    assert.throws(() => document.setPackageHeader('', 'OFD'), OfdError);
    const copy = Document.fromBytes(document.toBytes());
    try {
      assert.deepEqual(copy.packageInfo(), { version: '1.3 & beta', doc_type: 'OFD-A' });
      copy.setEntry('OFDEntries.xml', Buffer.from('<DocEntries/>'));
      const protectedBytes = copy.toBytes();
      assert.throws(() => copy.setPackageHeader('1.2', 'OFD'), OfdError);
      assert.throws(() => copy.setDocumentInfoField(0, 'Title', 'edit'), OfdError);
      assert.throws(() => copy.appendBlankPage(0, 20, 20), OfdError);
      assert.deepEqual(copy.toBytes(), protectedBytes);
    } finally { copy.close(); }
  } finally { document.close(); }
});

test('UTF-16 page XML round trips through the Node.js binding', () => {
  const document = Document.create();
  try {
    document.addTextPage('跨语言 UTF-16', 40, 30);
    const pagePath = document.pages()[0].path;
    const raw = Buffer.from(document.entry(pagePath).toString('utf8'), 'utf16le').swap16();
    document.setEntry(pagePath, raw);
    const reopened = Document.fromBytes(document.toBytes());
    try {
      assert.deepEqual(reopened.pageText(0), ['跨语言 UTF-16']);
      assert.deepEqual(reopened.entry(pagePath), raw);
    } finally { reopened.close(); }
  } finally { document.close(); }
});

test('page readers ignore same-named extension namespace content through the native ABI', () => {
  const document = Document.create();
  try {
    document.addTextPage('Node real', 80, 50);
    const entryPath = 'Doc_0/Pages/Page_0/Content.xml';
    const page = document.entry(entryPath).toString('utf8')
      .replace('<ofd:Page xmlns:ofd=', '<ofd:Page xmlns:ext="urn:example:extension" xmlns:ofd=')
      .replace('<ofd:Content>',
        '<ext:Content><ext:Layer ID="fake"><ext:TextCode>fake</ext:TextCode></ext:Layer></ext:Content>' +
        '<ofd:Content><ext:Layer ID="fake"><ext:TextCode>fake</ext:TextCode></ext:Layer>')
      .replace('<ofd:TextObject ',
        '<ext:TextObject ID="fake" Boundary="0 0 1 1"><ext:TextCode>fake</ext:TextCode></ext:TextObject>' +
        '<ofd:TextObject ')
      .replace('<ofd:TextCode ', '<ext:TextCode>fake</ext:TextCode><ofd:TextCode ');
    document.setEntry(entryPath, Buffer.from(page));
    const reopened = Document.fromBytes(document.toBytes());
    try {
      assert.deepEqual(reopened.pageText(0), ['Node real']);
      assert.equal(reopened.pageLayers(0).length, 1);
      assert.equal(reopened.pageLayers(0)[0].object_count, 1);
      assert.equal(reopened.pageObjects(0).length, 1);
      assert.equal(reopened.pageTextObjects(0)[0].codes.length, 1);
      assert.deepEqual(reopened.keywordPositions(['fake']), []);
      assert.match(reopened.pageSvg(0), /Node real/);
      assert.doesNotMatch(reopened.pageSvg(0), /fake/);
    } finally { reopened.close(); }
  } finally { document.close(); }
});

test('document and page multimedia resources round trip and remain isolated', () => {
  const document = Document.create();
  try {
    document.addBlankPage(20, 20);
    document.addBlankPage(20, 20);
    const documentId = document.addMultimediaResource(0, 'Audio', 'MP3', Buffer.from('document audio'));
    const pageId = document.addPageMultimediaResource(0, 'Audio', 'WAV', Buffer.from('page audio'));
    assert.deepEqual(document.documentMultimediaResources(0).map((item) => item.id), [documentId]);
    assert.deepEqual(new Set(document.pageMultimediaResources(0).map((item) => item.id)), new Set([documentId, pageId]));
    assert.deepEqual(document.pageMultimediaResources(1).map((item) => item.id), [documentId]);
    assert.equal(document.documentMultimediaData(0, documentId).toString(), 'document audio');
    assert.equal(document.pageMultimediaData(0, pageId).toString(), 'page audio');
    assert.throws(() => document.documentMultimediaData(0, pageId), OfdError);
    assert.throws(() => document.pageMultimediaData(1, pageId), OfdError);

    document.replaceDocumentMultimediaData(0, documentId, Buffer.from('updated document'));
    document.replacePageMultimediaData(0, pageId, Buffer.from('updated page'));
    const reopened = Document.fromBytes(document.toBytes());
    try {
      assert.equal(reopened.pageMultimediaData(1, documentId).toString(), 'updated document');
      assert.equal(reopened.pageMultimediaData(0, pageId).toString(), 'updated page');
      assert.deepEqual(reopened.validateReferences(), []);
      assert.throws(() => reopened.removePageMultimediaResource(1, pageId), OfdError);
      reopened.removePageMultimediaResource(0, pageId);
      reopened.removeDocumentMultimediaResource(0, documentId);
      assert.deepEqual(reopened.pageMultimediaResources(0), []);
      assert.deepEqual(reopened.pageMultimediaResources(1), []);
      assert.deepEqual(reopened.validateReferences(), []);
    } finally { reopened.close(); }
  } finally { document.close(); }
});

test('audio and video resources drive object actions and link annotations', () => {
  const document = Document.create();
  try {
    document.addTextPage('play', 40, 30);
    const objectId = document.pageObjects(0)[0].id;
    const audioId = document.addMultimediaResource(0, 'Audio', 'MP3', Buffer.from('audio'));
    const videoId = document.addMultimediaResource(0, 'Video', 'MP4', Buffer.from('video'));
    document.addPageObjectMediaAction(0, objectId, {kind: 'Sound', resource_id: audioId, volume: 75, repeat: true});
    document.addPageObjectMediaAction(0, objectId, {kind: 'Movie', resource_id: videoId, operator: 'Pause'});
    const soundLink = document.addMediaLinkAnnotation(0,
      {kind: 'Sound', resource_id: audioId, boundary: [1, 1, 5, 5]});
    const movieLink = document.addMediaLinkAnnotation(0,
      {kind: 'Movie', resource_id: videoId, boundary: [7, 1, 5, 5]});
    document.addPageObjectSoundAction(0, objectId, audioId, {volume: 25, synchronous: true});
    document.addPageObjectMovieAction(0, objectId, videoId, 'Resume');
    const soundShortcutLink = document.addSoundLinkAnnotation(0, audioId, [1, 8, 5, 5], {repeat: true});
    const movieShortcutLink = document.addMovieLinkAnnotation(0, videoId, [7, 8, 5, 5], 'Stop');
    const actions = document.pageObjectActions(0, objectId);
    assert.deepEqual(actions.map((action) => action.kind), ['Sound', 'Movie', 'Sound', 'Movie']);
    assert.equal(actions[2].attributes.Volume, '25');
    assert.equal(actions[2].attributes.Synchronous, 'true');
    assert.equal(actions[3].attributes.Operator, 'Resume');
    assert.ok(document.annotations().some((item) => item.id === soundLink));
    assert.ok(document.annotations().some((item) => item.id === movieLink));
    assert.ok(document.annotations().some((item) => item.id === soundShortcutLink));
    assert.ok(document.annotations().some((item) => item.id === movieShortcutLink));
    assert.throws(() => document.removeDocumentMultimediaResource(0, audioId), OfdError);
    assert.throws(() => document.addPageObjectMediaAction(0, objectId,
      {kind: 'Sound', resource_id: videoId}), OfdError);
    const reopened = Document.fromBytes(document.toBytes());
    try {
      assert.deepEqual(reopened.pageObjectActions(0, objectId), actions);
      assert.ok(reopened.annotations().some((item) => item.id === soundShortcutLink));
      assert.ok(reopened.annotations().some((item) => item.id === movieShortcutLink));
      assert.deepEqual(reopened.validateReferences(), []);
    } finally { reopened.close(); }
  } finally { document.close(); }
});

test('create, edit, serialize, render, and round-trip Unicode and NUL text', () => {
  const document = Document.create();
  try {
    document.addTextPage('你好\0 Node', 40, 30);
    document.addBlankPage(20, 20);
    assert.equal(document.pageCount, 2);
    assert.deepEqual(document.pageText(0), ['你好\0 Node']);
    assert.deepEqual(document.pageText(1), []);
    assert.equal(document.pages()[0].physical_box[2], 40);
    assert.match(document.pageSvg(0), /^<svg /);
    assert.deepEqual([...document.pagePng(0, 25.4).subarray(0, 8)], [137, 80, 78, 71, 13, 10, 26, 10]);
    assert.equal(document.toPdf().subarray(0, 5).toString(), '%PDF-');
    assert.deepEqual(document.validateReferences(), []);

    const copy = Document.fromBytes(document.toBytes());
    try {
      assert.deepEqual(copy.pageText(0), ['你好\0 Node']);
      assert.equal(copy.duplicatePage(0), 2);
      assert.deepEqual(copy.pageText(2), ['你好\0 Node']);
      assert.deepEqual(copy.validateReferences(), []);
    } finally {
      copy.close();
    }
  } finally {
    document.close();
    document.close();
  }
  assert.throws(() => document.pageCount, /closed/);
});

test('flow plain and styled text across pages', () => {
  const document = Document.create();
  try {
    const pages = document.addFlowedText(0, 'A\nB\nC', {
      page_size: [40, 22], margins: [5, 5, 5, 5], font_size: 5, line_height: 6,
    });
    assert.deepEqual(pages, [0, 1]);
    assert.deepEqual(document.pageText(0), ['A', 'B']);
    assert.deepEqual(document.pageText(1), ['C']);
    const styledPages = document.addFlowedSpans(0, [
      { text: 'red', fill_color: [255, 0, 0] },
      { text: ' blue', bold: true },
    ], { page_size: [80, 40], margins: [5, 5, 5, 5], font_size: 5, line_height: 6 });
    assert.deepEqual(styledPages, [2]);
    assert.equal(document.pageText(2).join(''), 'red blue');
    const count = document.pageCount;
    assert.throws(() => document.addFlowedText(0, 'invalid', { margins: [1, 2] }), OfdError);
    assert.throws(() => document.addFlowedSpans(0, [{ bold: true }]), OfdError);
    assert.throws(() => document.addFlowedSpans(0, 'not spans'), TypeError);
    assert.equal(document.pageCount, count);
    const reopened = Document.fromBytes(document.toBytes());
    try {
      assert.deepEqual(reopened.allText(), document.allText());
      assert.deepEqual(reopened.validateReferences(), []);
    } finally {
      reopened.close();
    }
  } finally {
    document.close();
  }
});

test('embed a font and shape paginated text', () => {
  const fontBytes = fs.readFileSync(fixture('dejavu-serif.ttf'));
  const document = Document.create();
  try {
    const fontId = document.addEmbeddedFont('DejaVu Serif', fontBytes);
    const fontIndex = document.fonts().findIndex((font) => font.id === fontId);
    assert.ok(fontIndex >= 0);
    assert.deepEqual(document.fontData(fontIndex), fontBytes);
    const pages = document.addFlowedText(0, 'fifi', {
      page_size: [14.5, 22], margins: [5, 5, 5, 5], font_id: fontId, shape: true,
    });
    assert.deepEqual(pages, [0]);
    assert.deepEqual(document.pageText(0), ['fi', 'fi']);
    assert.ok(document.pageTextObjects(0).every((item) => item.cg_transforms[0].glyph_count === 1));
    document.addTextPageWithFont('字体\0A', fontId, 40, 30);
    assert.deepEqual(document.pageText(1), ['字体\0A']);
    assert.equal(document.pageTextObjects(1)[0].font_id, fontId);
    assert.match(document.pageSvg(1), /@font-face\{font-family:&quot;ofd-embedded-font-0&quot;/);
    assert.match(document.pageSvg(1), /font-family="ofd-embedded-font-0"/);
    assert.match(document.pageSvg(1), /data:font\/ttf;base64,/);
    document.addTextPageWithFont('Numeric ID', Number(fontId), 40, 30);
    assert.deepEqual(document.pageText(2), ['Numeric ID']);
    const pageFontIndex = document.pageFonts(0).findIndex((font) => font.id === fontId);
    assert.ok(pageFontIndex >= 0);
    assert.deepEqual(document.pageFontData(0, pageFontIndex), fontBytes);
    assert.throws(() => document.addEmbeddedFont('bad', Buffer.from('not a font')), OfdError);
    assert.throws(() => document.addTextPageWithFont('bad', 'not-an-id'), TypeError);
    assert.throws(() => document.addTextPageWithFont('bad', null), TypeError);
    const reopened = Document.fromBytes(document.toBytes());
    try {
      const secondId = reopened.addEmbeddedFontToDocument(0, 'DejaVu Serif copy', fontBytes);
      assert.ok(reopened.fonts().some((font) => font.id === secondId));
      reopened.appendTextPageWithFont(0, 'Appended', secondId, 40, 30);
      assert.deepEqual(reopened.pageText(3), ['Appended']);
      assert.equal(reopened.pageTextObjects(3)[0].font_id, secondId);
      assert.deepEqual(reopened.pageText(0), ['fi', 'fi']);
      assert.deepEqual(reopened.validateReferences(), []);
    } finally {
      reopened.close();
    }
  } finally {
    document.close();
  }
});

test('shaped flow resolves a numeric font ID alias through Node.js', () => {
  const document = Document.create();
  try {
    const fontId = document.addEmbeddedFont('DejaVu Serif',
      fs.readFileSync(fixture('dejavu-serif.ttf')));
    document.addFlowedText(0, 'fifi', {
      page_size: [40, 30], margins: [5, 5, 5, 5], font_id: `0${fontId}`, shape: true,
    });
    assert.deepEqual(document.pageText(0), ['fifi']);
    assert.ok(document.pageTextObjects(0)[0].cg_transforms.length > 0);
    assert.deepEqual(document.validateReferences(), []);
  } finally { document.close(); }
});

test('positioned shaped text accepts color, language, and a numeric font ID alias', () => {
  const document = Document.create();
  try {
    const fontId = document.addEmbeddedFont('DejaVu Serif',
      fs.readFileSync(fixture('dejavu-serif.ttf')));
    document.addBlankPage(80, 50);
    const firstId = document.addTextObject(0, 'office', [5, 5, 50, 10], {
      size: 6, font_id: `0${fontId}`, shape: true, language: 'en', fill_color: [0, 0, 255],
    });
    const secondId = document.addTextObject(0, 'fifi', [5, 20, 50, 10], {
      size: 6, font_id: Number(fontId), shape: true,
    });
    const objects = document.pageTextObjects(0);
    assert.deepEqual(objects.map((object) => object.id), [firstId, secondId]);
    assert.ok(objects.every((object) => object.cg_transforms.length > 0));
    assert.deepEqual(document.pageText(0), ['office', 'fifi']);
    assert.match(document.pageSvg(0), /#0000ff/);
    const reopened = Document.fromBytes(document.toBytes());
    try {
      assert.deepEqual(reopened.pageText(0), ['office', 'fifi']);
      assert.deepEqual(reopened.validateReferences(), []);
    } finally { reopened.close(); }
  } finally { document.close(); }
});

test('page object edits resolve numeric ID aliases through Node.js', () => {
  const document = Document.create();
  try {
    document.addTextPage('aliased object', 80, 50);
    const id = document.pageTextObjects(0)[0].id;
    const alias = `0${id}`;
    document.setPageObjectBoundary(0, alias, [12, 10, 30, 8]);
    assert.deepEqual(document.pageTextObjects(0)[0].boundary, [12, 10, 30, 8]);
    document.removePageObject(0, alias);
    assert.deepEqual(document.pageTextObjects(0), []);
    assert.deepEqual(document.validateReferences(), []);
  } finally { document.close(); }
});

test('annotation reads and edits resolve numeric ID aliases through Node.js', () => {
  const document = Document.create();
  try {
    document.addBlankPage(40, 30);
    const id = document.addShapeAnnotation(0, 'Square', [2, 3, 8, 5]);
    const alias = `0${id}`;
    assert.ok(document.annotationObjects(0, alias).length > 0);
    document.setAnnotationCreator(0, alias, 'alias creator');
    assert.equal(document.annotations()[0].attributes.Creator, 'alias creator');
    document.removeAnnotation(0, alias);
    assert.deepEqual(document.annotations(), []);
    assert.deepEqual(document.validateReferences(), []);
  } finally { document.close(); }
});

test('adding an annotation reuses an existing aliased page record', () => {
  const document = Document.create();
  try {
    document.addBlankPage(40, 30);
    document.addTextNoteAnnotation(0, 'first', [2, 2, 8, 8]);
    const pageId = document.pages()[0].id;
    const listPath = 'Doc_0/Annots/Annotations.xml';
    const list = document.entry(listPath).toString('utf8');
    document.setEntry(listPath, Buffer.from(list.replace(`PageID="${pageId}"`, `PageID="0${pageId}"`)));
    document.addTextNoteAnnotation(0, 'second', [12, 2, 8, 8]);
    assert.equal(document.annotations().length, 2);
    assert.equal(document.entry(listPath).toString('utf8').match(/<ofd:Page PageID=/g).length, 1);
    assert.deepEqual(document.validateReferences(), []);
    const current = document.entry(listPath).toString('utf8');
    const duplicate = `<ofd:Page PageID="${pageId}"><ofd:FileLoc>Page_0/Annotation.xml</ofd:FileLoc></ofd:Page>`;
    document.setEntry(listPath, Buffer.from(current.replace('</ofd:Annotations>', `${duplicate}</ofd:Annotations>`)));
    assert.ok(document.validateReferences().some((issue) => issue.code === 'duplicate_annotation_page_id'));
    document.setEntry(listPath, Buffer.from(document.entry(listPath).toString('utf8')
      .replace(`PageID="${pageId}"`, 'PageID="bad"')));
    assert.ok(document.validateReferences().some((issue) => issue.code === 'invalid_annotation_page_id'));
  } finally { document.close(); }
});

test('edit OFD font substitution attributes through Node.js', () => {
  const data = fs.readFileSync(fixture('dejavu-serif.ttf'));
  const document = Document.create();
  try {
    const id = document.addEmbeddedFont('DejaVu Serif', data);
    const index = document.fonts().findIndex((font) => font.id === id);
    assert.ok(index >= 0);
    document.setFontAttribute(index, 'FamilyName', 'A & B');
    document.setFontAttribute(index, 'Charset', 'SHIFT_JIS');
    document.setFontAttribute(index, 'Bold', 'true');
    assert.equal(document.fonts()[index].family_name, 'A & B');
    assert.equal(document.fonts()[index].attributes.Charset, 'shift-jis');
    const before = document.toBytes();
    assert.throws(() => document.setFontAttribute(index, 'Charset', 'invalid'), OfdError);
    assert.deepEqual(document.toBytes(), before);
    document.setFontAttribute(index, 'Bold');
    const replacement = fs.readFileSync(fixture('dejavu-sans.ttf'));
    document.replaceFontData(index, replacement);
    assert.deepEqual(document.fontData(index), replacement);
    assert.throws(() => document.replaceFontData(index, Buffer.from('invalid')), OfdError);
    assert.throws(() => document.replaceFontData(index, Buffer.from([0, 1, 0, 0])), OfdError);
    const reopened = Document.fromBytes(document.toBytes());
    try {
      assert.equal(reopened.fonts()[index].family_name, 'A & B');
      assert.equal(reopened.fonts()[index].attributes.Bold, undefined);
      assert.deepEqual(reopened.fontData(index), replacement);
    } finally { reopened.close(); }
  } finally { document.close(); }
});

test('edit a page-local font without changing the public font through Node.js', () => {
  const document = Document.create();
  try {
    document.addBlankPage(30, 20);
    const pagePath = document.pages()[0].path;
    const pageDir = pagePath.slice(0, pagePath.lastIndexOf('/'));
    const original = fs.readFileSync(fixture('dejavu-sans.ttf'));
    document.setEntry(`${pageDir}/local.ttf`, original);
    document.setEntry(`${pageDir}/PageRes.xml`, Buffer.from(
      '<ofd:Res xmlns:ofd="http://www.ofdspec.org/2016"><ofd:Fonts>' +
      '<ofd:Font ID="1" FontName="Local"><ofd:FontFile>local.ttf</ofd:FontFile></ofd:Font></ofd:Fonts></ofd:Res>'));
    const page = document.entry(pagePath).toString('utf8');
    document.setEntry(pagePath, Buffer.from(page.replace('<ofd:Content>',
      '<ofd:PageRes>PageRes.xml</ofd:PageRes><ofd:Content>')));
    assert.equal(document.pageFonts(0)[0].font_name, 'Local');
    document.setPageFontAttribute(0, 0, 'FamilyName', 'Page & Local');
    assert.equal(document.pageFonts(0)[0].family_name, 'Page & Local');
    assert.notEqual(document.fonts()[0].family_name, 'Page & Local');
    const replacement = fs.readFileSync(fixture('dejavu-serif.ttf'));
    document.replacePageFontData(0, 0, replacement);
    assert.deepEqual(document.pageFontData(0, 0), replacement);
    assert.throws(() => document.replacePageFontData(0, 0, Buffer.from('invalid')), OfdError);
    assert.throws(() => document.replacePageFontData(0, 0, Buffer.from([0, 1, 0, 0])), OfdError);
    const before = document.toBytes();
    assert.throws(() => document.setPageFontAttribute(0, 0, 'Charset', 'invalid'), OfdError);
    assert.deepEqual(document.toBytes(), before);
    const reopened = Document.fromBytes(before);
    try {
      assert.equal(reopened.pageFonts(0)[0].family_name, 'Page & Local');
      assert.deepEqual(reopened.pageFontData(0, 0), replacement);
    }
    finally { reopened.close(); }
  } finally { document.close(); }
});

test('embed a font only in the selected page through Node.js', () => {
  const data = fs.readFileSync(fixture('dejavu-sans.ttf'));
  const document = Document.create();
  try {
    document.addBlankPage(40, 30);
    document.addBlankPage(40, 30);
    const id = document.addEmbeddedFontToPage(0, 'Page & Font', data);
    assert.equal(document.pageFonts(0)[0].id, id);
    assert.deepEqual(document.pageFontData(0, 0), data);
    assert.ok(document.pageFonts(1).every((font) => font.id !== id));
    const before = document.toBytes();
    assert.throws(() => document.addEmbeddedFontToPage(0, 'bad', Buffer.from('invalid')), OfdError);
    assert.deepEqual(document.toBytes(), before);
    const reopened = Document.fromBytes(before);
    try {
      assert.deepEqual(reopened.pageFontData(0, 0), data);
      assert.deepEqual(reopened.validateReferences(), []);
    } finally { reopened.close(); }
  } finally { document.close(); }
});

test('create document and page color spaces and render their colors', () => {
  const document = Document.create();
  try {
    document.addBlankPage(30, 20);
    const grayId = document.addDocumentColorSpace(0, { color_type: 'GRAY', bits_per_component: 4 });
    const paletteId = document.addPageColorSpace(0, {
      color_type: 'RGB', palette: [[255, 0, 0], [0, 0, 255]],
    });
    document.setDocumentDefaultColorSpace(0, grayId);
    assert.equal(document.documentDefaultColorSpace(0), grayId);
    assert.equal(document.documentColorSpaces(0)[0].id, grayId);
    assert.equal(document.pageColorSpaces(0).find((space) => space.id === paletteId).palette.length, 2);
    document.setDocumentColorSpace(0, grayId, { color_type: 'GRAY', bits_per_component: 4 });
    document.setPageColorSpace(0, paletteId, {
      color_type: 'RGB', palette: [[0, 255, 0], [0, 0, 255]],
    });
    assert.equal(document.documentColorSpaces(0)[0].bits_per_component, 4);
    assert.deepEqual(document.pageColorSpaces(0).find((space) => space.id === paletteId).palette[0], [0, 255, 0]);
    assert.equal(document.documentDefaultColorSpace(0), grayId);
    const beforeInvalid = document.pageColorSpaces(0);
    assert.throws(() => document.setDocumentColorSpace(0, paletteId, { color_type: 'RGB' }), OfdError);
    assert.throws(() => document.setPageColorSpace(0, paletteId, {
      color_type: 'RGB', palette: [[256, 0, 0]],
    }), OfdError);
    assert.deepEqual(document.pageColorSpaces(0), beforeInvalid);
    document.addPathObject(0, 'M 0 0 L 10 0 L 10 10 C', [0, 0, 10, 10],
      { fill: true, stroke: false, fill_color: [0, 0, 0] });
    document.addPathObject(0, 'M 0 0 L 10 0 L 10 10 C', [12, 0, 10, 10],
      { fill: true, stroke: false, fill_color: [0, 0, 0] });
    const pagePath = document.pages()[0].path;
    let xml = document.entry(pagePath).toString('utf8');
    xml = xml.replace('<ofd:FillColor Value="0 0 0"/>', '<ofd:FillColor Value="8"/>');
    xml = xml.replace('<ofd:FillColor Value="0 0 0"/>',
      `<ofd:FillColor ColorSpace="${paletteId}" Index="1"/>`);
    document.setEntry(pagePath, Buffer.from(xml));
    const svg = document.pageSvg(0);
    assert.ok(svg.includes('#888888'));
    assert.ok(svg.includes('#0000ff'));
    assert.throws(() => document.addDocumentColorSpace(0, {
      color_type: 'RGB', palette: [[256, 0, 0]],
    }), OfdError);
    assert.throws(() => document.setDocumentDefaultColorSpace(0, 'missing'), OfdError);
    const copy = Document.fromBytes(document.toBytes());
    try {
      assert.equal(copy.documentDefaultColorSpace(0), grayId);
      assert.deepEqual(copy.pageColorSpaces(0), document.pageColorSpaces(0));
      assert.deepEqual(copy.validateReferences(), []);
      copy.setDocumentDefaultColorSpace(0, null);
      assert.equal(copy.documentDefaultColorSpace(0), null);
    } finally {
      copy.close();
    }
  } finally {
    document.close();
  }
});

test('edit solid, gradient, Gouraud, and pattern paints through Node.js', () => {
  const document = Document.create();
  try {
    document.addBlankPage(20, 20);
    const id = document.addPathObject(0, 'M 0 0 L 20 0 L 20 20 L 0 20 C', [0, 0, 20, 20],
      { fill: true, stroke: true });
    document.setPageObjectSolidFill(0, id, { color: [255, 0, 0] });
    document.setPageObjectSolidStroke(0, id, { color: [0, 0, 255], alpha: 200 });
    let svg = document.pageSvg(0);
    assert.match(svg, /#ff0000/);
    assert.match(svg, /#0000ff/);

    const gradient = { kind: 'axial', start_point: [0, 0], end_point: [20, 0],
      segments: [{ color: [255, 0, 0] }, { color: [0, 0, 255] }] };
    document.setPageObjectGradientFill(0, id, gradient);
    document.setPageObjectGradientStroke(0, id, gradient);
    svg = document.pageSvg(0);
    assert.match(svg, /<linearGradient/);
    assert.match(svg, /stroke="url\(#ofd-shading-/);

    const gouraud = { points: [
      { x: 0, y: 0, color: [255, 0, 0] }, { x: 20, y: 0, color: [0, 255, 0] },
      { x: 0, y: 20, color: [0, 0, 255] }, { x: 20, y: 20, color: [255, 255, 0] },
    ], vertices_per_row: 2 };
    document.setPageObjectGouraudFill(0, id, gouraud);
    document.setPageObjectGouraudStroke(0, id, gouraud);
    svg = document.pageSvg(0);
    assert.match(svg, /ofd-gouraud-/);
    assert.match(svg, /stroke="url\(#ofd-gouraud-/);

    const pattern = { width: 4, height: 4, cells: [{ kind: 'path',
      commands: 'M 0 0 L 2 0 L 2 4 L 0 4 C', boundary: [0, 0, 2, 4],
      fill: true, stroke: false, fill_color: [255, 0, 0] }] };
    document.setPageObjectPatternFill(0, id, pattern);
    document.setPageObjectPatternStroke(0, id, pattern);
    svg = document.pageSvg(0);
    assert.match(svg, /fill="url\(#ofd-pattern-/);
    assert.match(svg, /stroke="url\(#ofd-pattern-/);
    assert.deepEqual(document.validateReferences(), []);
    const reopened = Document.fromBytes(document.toBytes());
    try { assert.match(reopened.pageSvg(0), /ofd-pattern-/); } finally { reopened.close(); }

    const before = document.toBytes();
    assert.throws(() => document.setPageObjectPatternFill(0, id, { ...pattern, width: 0 }), OfdError);
    assert.throws(() => document.setPageObjectGradientStroke(0, id,
      { ...gradient, map_unit: 0 }), OfdError);
    assert.throws(() => document.setPageObjectGouraudFill(0, id,
      { ...gouraud, vertices_per_row: 3 }), OfdError);
    assert.throws(() => document.setPageObjectSolidFill(0, id, null), TypeError);
    assert.deepEqual(document.toBytes(), before);
  } finally {
    document.close();
  }
});

test('remove unreferenced document and page color spaces', () => {
  const document = Document.create();
  try {
    document.addBlankPage(20, 20);
    const publicId = document.addDocumentColorSpace(0, { color_type: 'RGB' });
    const localId = document.addPageColorSpace(0, { color_type: 'GRAY' });
    document.setDocumentDefaultColorSpace(0, publicId);
    assert.throws(() => document.removeDocumentColorSpace(0, publicId), OfdError);
    document.removePageColorSpace(0, localId);
    assert.throws(() => document.removePageColorSpace(0, publicId), OfdError);
    document.setDocumentDefaultColorSpace(0, null);
    document.removeDocumentColorSpace(0, publicId);
    assert.deepEqual(document.pageColorSpaces(0), []);
    const reopened = Document.fromBytes(document.toBytes());
    try { assert.deepEqual(reopened.pageColorSpaces(0), []); } finally { reopened.close(); }
  } finally {
    document.close();
  }
});

test('report unresolved ColorSpace references through Node.js', () => {
  const document = Document.create();
  try {
    document.addBlankPage(20, 20);
    const page = document.pages()[0].path;
    const xml = document.entry(page).toString('utf8');
    document.setEntry(page, Buffer.from(xml.replace('</ofd:Content>',
      '<ofd:Layer ID="500"><ofd:PathObject ID="501" Boundary="0 0 1 1" Fill="true">'
      + '<ofd:FillColor ColorSpace="999" Index="0"/>'
      + '<ofd:AbbreviatedData>M 0 0 L 1 1</ofd:AbbreviatedData>'
      + '</ofd:PathObject></ofd:Layer></ofd:Content>')));
    assert.ok(document.validateReferences().some((issue) => issue.code === 'color_space_reference'));
    const colorId = document.addDocumentColorSpace(0, {
      color_type: 'RGB', palette: [[255, 0, 0]],
    });
    document.setEntry(page, Buffer.from(document.entry(page).toString('utf8').replace(
      'ColorSpace="999" Index="0"', `ColorSpace="${colorId}" Index="1"`)));
    assert.ok(document.validateReferences().some((issue) => issue.code === 'invalid_color_index'));
  } finally {
    document.close();
  }
});

test('share a reusable draw parameter across page objects', () => {
  const document = Document.create();
  try {
    document.addBlankPage(40, 20);
    const drawParam = document.addDocumentDrawParam(0, {
      line_width: 1.5, fill: { color: [255, 0, 0] },
    });
    const first = document.addPathObject(0, 'M 0 0 L 10 0 L 10 10 C', [0, 0, 10, 10],
      { fill: true, stroke: false });
    const second = document.addPathObject(0, 'M 0 0 L 10 0 L 10 10 C', [15, 0, 10, 10],
      { fill: true, stroke: false });
    document.setPageObjectDrawParam(0, first, drawParam);
    document.setPageObjectDrawParam(0, second, drawParam);
    assert.equal(document.pageObjects(0).filter((object) => object.draw_param === drawParam).length, 2);
    assert.ok(document.pageSvg(0).includes('#ff0000'));
    assert.throws(() => document.addDocumentDrawParam(0, { line_width: -1 }), OfdError);
    const copy = Document.fromBytes(document.toBytes());
    try {
      assert.equal(copy.pageObjects(0).filter((object) => object.draw_param === drawParam).length, 2);
      assert.ok(copy.pageSvg(0).includes('#ff0000'));
      assert.deepEqual(copy.validateReferences(), []);
    } finally {
      copy.close();
    }
  } finally {
    document.close();
  }
});

test('page draw parameters inherit public styles and stay on their page', () => {
  const document = Document.create();
  try {
    document.addBlankPage(20, 20);
    document.appendBlankPage(0, 20, 20);
    const first = document.addPathObject(0, 'M 0 0 L 10 0 L 10 10 C', [0, 0, 10, 10],
      { fill: true, stroke: false });
    const second = document.addPathObject(1, 'M 0 0 L 10 0 L 10 10 C', [0, 0, 10, 10],
      { fill: true, stroke: false });
    const shared = document.addDocumentDrawParam(0, { fill: { color: [255, 0, 0] } });
    const local = document.addPageDrawParam(0, {
      relative: shared, fill: { color: [0, 255, 0] },
    });
    assert.deepEqual(document.documentDrawParams(0).map((item) => item.id), [shared]);
    const firstPageParams = document.pageDrawParams(0);
    assert.deepEqual(firstPageParams.map((item) => item.id).sort(), [shared, local].sort());
    const localInfo = firstPageParams.find((item) => item.id === local);
    assert.equal(localInfo.relative, shared);
    assert.match(localInfo.xml, /<ofd:DrawParam/);
    assert.ok(localInfo.resource_path.endsWith(`RustPageDrawParams_${local}.xml`));
    assert.deepEqual(document.pageDrawParams(1).map((item) => item.id), [shared]);
    assert.throws(() => document.pageDrawParams(-1), RangeError);
    document.setPageObjectDrawParam(0, first, local);
    document.setPageObjectDrawParam(1, second, shared);
    document.setDocumentDrawParam(0, shared, { fill: { color: [0, 0, 255] } });
    document.setPageDrawParam(0, local, {
      relative: shared, fill: { color: [0, 255, 0] },
    });
    assert.match(document.pageSvg(1), /fill="#0000ff"/);
    assert.match(document.pageSvg(0), /fill="#00ff00"/);
    const before = document.toBytes();
    assert.throws(() => document.setPageDrawParam(0, local, { relative: local }), OfdError);
    assert.throws(() => document.setPageDrawParam(1, local, {}), OfdError);
    assert.throws(() => document.setDocumentDrawParam(0, local, {}), OfdError);
    assert.throws(() => document.setPageObjectDrawParam(1, second, local), OfdError);
    assert.throws(() => document.addPageDrawParam(1, { relative: local }), OfdError);
    assert.throws(() => document.addPageDrawParam(-1, {}), RangeError);
    assert.deepEqual(document.toBytes(), before);
    assert.deepEqual(document.validateReferences(), []);
    assert.throws(() => document.removePageDrawParam(0, local), OfdError);
    assert.throws(() => document.removeDocumentDrawParam(0, shared), OfdError);
    assert.deepEqual(document.toBytes(), before);
    document.setPageObjectDrawParam(0, first, null);
    document.setPageObjectDrawParam(1, second, null);
    document.removePageDrawParam(0, local);
    document.removeDocumentDrawParam(0, shared);
    assert.deepEqual(document.pageDrawParams(0), []);
  } finally {
    document.close();
  }
});

test('paginate a table and repeat its header', () => {
  const document = Document.create();
  try {
    const options = {
      column_widths: [12, 12], page_size: [30, 26], margins: [2, 2, 2, 2],
      row_height: 8, font_size: 3, line_height: 3, padding: 1, repeat_header: true,
    };
    assert.deepEqual(document.addTable(0, [['H1', 'H2'], ['A', 'B'], ['C', 'D']], options), [0, 1]);
    assert.deepEqual(document.pageText(1), ['H1', 'H2', 'C', 'D']);
    const merged = document.addTableCells(0, 2, [
      { row: 0, column: 0, text: 'Left', bold: true },
      { row: 0, column: 1, text: 'Right' },
      { row: 1, column: 0, column_span: 2, text: 'Merged', background_color: [220, 230, 240] },
    ], { ...options, page_size: [40, 40] });
    assert.deepEqual(merged, [2]);
    assert.deepEqual(document.pageText(2), ['Left', 'Right', 'Merged']);
    const nested = document.addTableCells(0, 1, [{
      row: 0, column: 0,
      blocks: [
        { type: 'text', text: 'Outer' },
        { type: 'table', table: {
          row_count: 1, column_widths: [10], cells: [{ row: 0, column: 0, text: 'Inner' }],
        } },
      ],
    }], { column_widths: [24], page_size: [30, 30], margins: [2, 2, 2, 2], row_height: 0, font_size: 3, line_height: 3 });
    assert.deepEqual(nested, [3]);
    assert.ok(document.pageText(3).includes('Inner'));
    const before = document.toBytes();
    assert.throws(() => document.addTable(0, [['bad']], { column_widths: [0] }), OfdError);
    assert.throws(() => document.addTable(0, 'bad', options), TypeError);
    assert.throws(() => document.addTableCells(0, 1, [
      { row: 0, column: 0, text: 'A' }, { row: 0, column: 0, text: 'B' },
    ], options), OfdError);
    assert.throws(() => document.addTableCells(0, -1, [], options), RangeError);
    assert.deepEqual(document.toBytes(), before);
    const reopened = Document.fromBytes(before);
    try {
      assert.deepEqual(reopened.pageText(1), ['H1', 'H2', 'C', 'D']);
      assert.deepEqual(reopened.pageText(2), ['Left', 'Right', 'Merged']);
      assert.ok(reopened.pageText(3).includes('Inner'));
      assert.deepEqual(reopened.validateReferences(), []);
    } finally {
      reopened.close();
    }
  } finally {
    document.close();
  }
});

test('edit document metadata, keywords, and custom data across OFD serialization', () => {
  const document = Document.create();
  try {
    assert.equal(document.documents()[0].title, null);
    const docId = '0123456789abcdef0123456789abcdef';
    assert.throws(() => document.setDocumentInfoField(0, 'DocID', 'invalid'), OfdError);
    assert.throws(() => document.setDocumentInfoField(0, 'CreationDate', '2024-02-30'), OfdError);
    assert.throws(() => document.setDocumentInfoField(0, 'DocUsage', 'Magazine'), OfdError);
    assert.throws(() => document.removeDocumentInfoField(0, 'DocID'), OfdError);
    document.setDocumentInfoField(0, 'DocID', docId);
    document.setDocumentInfoField(0, 'Title', '发票 & <OFD>');
    document.setDocumentInfoField(0, 'CreationDate', '2024-02-29');
    document.setDocumentInfoField(0, 'DocUsage', 'EBook');
    document.setEntry('Cover.png', Buffer.from('cover'));
    document.setDocumentInfoField(0, 'Cover', 'COVER.PNG');
    assert.deepEqual(document.validateReferences(), []);
    document.addDocumentKeyword(0, '票据 & 发票');
    document.addDocumentKeyword(0, 'remove me');
    document.removeDocumentKeyword(0, 1);
    document.setDocumentCustomData(0, '客户&类型', '<VIP>');
    document.setDocumentCustomData(0, '客户&类型', '更新 & 完成');
    assert.throws(() => document.setDocumentInfoField(0, 'DocRoot', 'bad'), OfdError);
    assert.throws(() => document.removeDocumentKeyword(0, 5), OfdError);
    assert.throws(() => document.setDocumentCustomData(-1, 'key', 'value'), RangeError);
    assert.throws(() => document.addDocumentKeyword(0, 'bad\0word'), TypeError);
    const copy = Document.fromBytes(document.toBytes());
    try {
      const info = copy.documents()[0];
      assert.equal(info.title, '发票 & <OFD>');
      assert.equal(info.doc_id, docId);
      assert.equal(info.creation_date, '2024-02-29');
      assert.equal(info.doc_usage, 'EBook');
      assert.equal(info.cover, 'COVER.PNG');
      assert.deepEqual(info.keywords, ['票据 & 发票']);
      assert.deepEqual(info.custom_data, [{ name: '客户&类型', value: '更新 & 完成' }]);
      copy.removeDocumentInfoField(0, 'Title');
      copy.removeDocumentKeyword(0, 0);
      copy.removeDocumentCustomData(0, '客户&类型');
      assert.equal(copy.documents()[0].title, null);
      assert.deepEqual(copy.documents()[0].keywords, []);
      assert.deepEqual(copy.documents()[0].custom_data, []);
      assert.deepEqual(copy.validateReferences(), []);
    } finally {
      copy.close();
    }
  } finally {
    document.close();
  }
});

test('ignore extension namespace elements while editing OFD metadata', () => {
  const document = Document.create();
  try {
    document.addBlankPage(40, 30);
    const xml = document.entry('OFD.xml').toString()
      .replace('<ofd:OFD ', '<ofd:OFD xmlns:ext="urn:example:metadata" ')
      .replace('</ofd:DocInfo>',
        '<ext:Title>extension</ext:Title><ext:Cover>Missing.png</ext:Cover>'
        + '<ofd:Title>original</ofd:Title></ofd:DocInfo>')
      .replace('</ofd:DocBody>',
        '<ext:Versions><ext:Version ID="foreign"/></ext:Versions>'
        + '<ext:Signatures>Missing.xml</ext:Signatures></ofd:DocBody>')
      .replace('</ofd:OFD>',
        '<ext:DocBody><ext:DocRoot>Missing.xml</ext:DocRoot></ext:DocBody></ofd:OFD>');
    document.setEntry('OFD.xml', Buffer.from(xml));
    assert.equal(document.documents().length, 1);
    assert.equal(document.documents()[0].title, 'original');
    assert.deepEqual(document.versions(), []);
    assert.deepEqual(document.signatures(), []);
    assert.deepEqual(document.validateReferences(), []);
    document.setDocumentInfoField(0, 'Title', 'changed');
    const updated = document.entry('OFD.xml').toString();
    assert.ok(updated.includes('<ext:Title>extension</ext:Title>'));
    assert.ok(updated.includes('<ofd:Title>changed</ofd:Title>'));
    assert.deepEqual(document.validateReferences(), []);
  } finally {
    document.close();
  }
});

test('report malformed and duplicate document IDs from package XML', () => {
  const document = Document.create();
  try {
    document.addBlankPage(40, 30);
    document.appendDocumentBody();
    const [first, second] = document.documents();
    const original = document.entry('OFD.xml');
    const xml = original.toString('utf8');
    assert.deepEqual(document.validateReferences(), []);
    document.setEntry('OFD.xml', Buffer.from(xml.replace(second.doc_id, 'invalid')));
    assert.ok(document.validateReferences().some((issue) => issue.code === 'invalid_doc_id'));
    document.setEntry('OFD.xml', Buffer.from(xml.replace(second.doc_id, first.doc_id.toUpperCase())));
    assert.ok(document.validateReferences().some((issue) => issue.code === 'duplicate_doc_id'));
    document.setEntry('OFD.xml', Buffer.from(xml.replace(`<ofd:DocID>${second.doc_id}</ofd:DocID>`, '')));
    assert.equal(document.documents()[1].doc_id, '');
    assert.deepEqual(document.validateReferences(), []);
    const missingId = Document.fromBytes(document.toBytes());
    try { assert.equal(missingId.documents()[1].doc_id, ''); } finally { missingId.close(); }
    const docInfo = xml.match(/<ofd:DocInfo>[\s\S]*?<\/ofd:DocInfo>/)[0];
    document.setEntry('OFD.xml', Buffer.from(xml.replace(docInfo, '')));
    assert.ok(document.validateReferences().some((issue) => issue.code === 'missing_doc_info'));
    assert.throws(() => document.documents());
    document.setEntry('OFD.xml', Buffer.from(xml.replace(docInfo, `${docInfo}<ofd:DocInfo/>`)));
    assert.ok(document.validateReferences().some((issue) => issue.code === 'duplicate_doc_info'));
    assert.throws(() => document.documents());
    document.setEntry('OFD.xml', Buffer.from(xml.replace('</ofd:DocInfo>', '<ofd:Title>first</ofd:Title><ofd:Title>second</ofd:Title></ofd:DocInfo>')));
    assert.ok(document.validateReferences().some((issue) => issue.code === 'duplicate_doc_info_field'));
    document.setEntry('OFD.xml', Buffer.from(xml.replace('</ofd:DocInfo>', '<ofd:DocUsage>Other</ofd:DocUsage></ofd:DocInfo>')));
    assert.ok(document.validateReferences().some((issue) => issue.code === 'invalid_doc_usage'));
    document.setEntry('OFD.xml', original);
    assert.deepEqual(document.validateReferences(), []);
  } finally {
    document.close();
  }
});

test('round-trip document permissions and viewer preferences', () => {
  const document = Document.create();
  try {
    const permissions = document.documentPermissions(0);
    assert.equal(permissions.edit, true);
    assert.deepEqual(permissions.print, { printable: true, copies: -1 });
    const changedPermissions = {
      ...permissions,
      edit: false,
      print: { printable: true, copies: 2 },
      valid_period: { start_date: '2024-02-29T12:00:00', end_date: null },
    };
    document.setDocumentPermissions(0, changedPermissions);
    assert.deepEqual(document.documentPermissions(0), changedPermissions);

    const preferences = document.viewPreferences(0);
    assert.equal(preferences.page_layout, 'OneColumn');
    const changedPreferences = {
      ...preferences,
      page_mode: 'UseOutlines',
      hide_toolbar: true,
      zoom_mode: 'FitWidth',
    };
    document.setViewPreferences(0, changedPreferences);
    assert.deepEqual(document.viewPreferences(0), changedPreferences);
    assert.throws(() => document.setDocumentPermissions(0, { edit: false }), OfdError);
    assert.throws(() => document.setViewPreferences(0, { ...changedPreferences, zoom: 1.25 }), OfdError);
    assert.throws(() => document.setViewPreferences(0, null), TypeError);
    assert.throws(() => document.documentPermissions(-1), RangeError);

    const copy = Document.fromBytes(document.toBytes());
    try {
      assert.deepEqual(copy.documentPermissions(0), changedPermissions);
      assert.deepEqual(copy.viewPreferences(0), changedPreferences);
      assert.deepEqual(copy.validateReferences(), []);
    } finally {
      copy.close();
    }
  } finally {
    document.close();
  }
});

test('set document and page areas independently', () => {
  const document = Document.create();
  try {
    document.addBlankPage(100, 80);
    assert.deepEqual(document.documentPageArea(0).physical_box, [0, 0, 210, 297]);
    document.setDocumentPageArea(0, {
      physical_box: [0, 0, 150, 60],
      application_box: [10, 5, 130, 50],
    });
    assert.deepEqual(document.documentPageArea(0).application_box, [10, 5, 130, 50]);
    assert.deepEqual(document.pages()[0].physical_box, [0, 0, 100, 80]);
    document.setPageArea(0, {
      physical_box: [0, 0, 80, 40],
      content_box: [5, 5, 70, 30],
    });
    assert.deepEqual(document.pages()[0].physical_box, [0, 0, 80, 40]);
    assert.deepEqual(document.pages()[0].content_box, [5, 5, 70, 30]);
    assert.throws(() => document.setPageArea(0, { physical_box: [0, 0, 0, 20] }), RangeError);
    assert.throws(() => document.setDocumentPageArea(0, {}), RangeError);
    assert.throws(() => document.setPageArea(1, { physical_box: [0, 0, 20, 20] }), OfdError);
    const copy = Document.fromBytes(document.toBytes());
    try {
      assert.deepEqual(copy.documentPageArea(0), document.documentPageArea(0));
      assert.deepEqual(copy.pages()[0], document.pages()[0]);
      assert.deepEqual(copy.validateReferences(), []);
    } finally {
      copy.close();
    }
  } finally {
    document.close();
  }
});

test('read, append, and remove document and page actions', () => {
  const document = Document.create();
  try {
    document.addBlankPage(40, 30);
    const destination = { page_id: document.pages()[0].id, kind: 'Fit' };
    document.addBookmark(0, 'start', destination);
    document.addAttachment('note.txt', Buffer.from('hello'));
    const attachmentId = document.attachments()[0].id;
    assert.deepEqual(document.documentActions(0), []);
    assert.deepEqual(document.pageActions(0), []);
    document.addDocumentAction(0, { kind: 'uri', event: 'DO', uri: 'https://example.org/?x=1&y=2' });
    document.addDocumentAction(0, { kind: 'page', event: 'PO', destination });
    document.addDocumentAction(0, { kind: 'bookmark', event: 'CLICK', name: 'start' });
    document.addDocumentAction(0, { kind: 'attachment', event: 'DO', id: attachmentId, new_window: true });
    document.addPageAction(0, { kind: 'uri', event: 'PO', uri: 'https://example.org/page' });
    document.addPageAction(0, { kind: 'page', event: 'PO', destination });
    assert.deepEqual(document.documentActions(0).map((action) => action.event), ['DO', 'PO', 'CLICK', 'DO']);
    assert.equal(document.documentActions(0)[0].attributes.URI, 'https://example.org/?x=1&y=2');
    assert.equal(document.documentActions(0)[1].destination.page_id, destination.page_id);
    assert.equal(document.documentActions(0)[2].bookmark, 'start');
    assert.equal(document.pageActions(0)[0].event, 'PO');
    assert.throws(() => document.addPageAction(0, { kind: 'uri', event: 'DO', uri: 'https://bad' }), OfdError);
    assert.throws(() => document.addDocumentAction(0, { kind: 'unknown', event: 'DO' }), TypeError);
    assert.throws(() => document.addDocumentAction(0, { kind: 'bookmark', event: 'DO', name: 'missing' }), OfdError);
    assert.throws(() => document.removePageAction(0, 9), OfdError);
    document.removeDocumentAction(0, 3);
    document.removePageAction(0, 1);
    const copy = Document.fromBytes(document.toBytes());
    try {
      assert.deepEqual(copy.documentActions(0), document.documentActions(0));
      assert.deepEqual(copy.pageActions(0), document.pageActions(0));
      assert.deepEqual(copy.validateReferences(), []);
    } finally {
      copy.close();
    }
  } finally {
    document.close();
  }
});

test('preserve custom tag payloads and document extensions', () => {
  const document = Document.create();
  try {
    const payload = Buffer.from('<Invoice Ref="票据 &amp; foo"/>');
    const tag = document.addCustomTag(0, 'invoice', payload, { namespace: 'urn:example:invoice' });
    assert.equal(tag.type_id, 'invoice');
    assert.equal(tag.namespace, 'urn:example:invoice');
    assert.deepEqual(document.customTags(), [tag]);
    assert.deepEqual(document.customTagData(0), payload);

    document.addExtension({
      document_index: 0,
      app_name: '应用 & Co',
      company: 'Acme',
      app_version: '1.2',
      date: '2024-02-29',
      ref_id: 1,
      properties: [{ name: 'status', type_name: 'string', value: 'ready & checked' }],
      data_xml: ['<ofd:Data xmlns:ofd="http://www.ofdspec.org/2016"><Payload/></ofd:Data>'],
    });
    assert.equal(document.extensions()[0].app_name, '应用 & Co');
    assert.equal(document.extensions()[0].properties[0].value, 'ready & checked');
    assert.throws(() => document.addExtension({ document_index: 0, app_name: 'bad', date: '2025-02-29' }), OfdError);
    assert.throws(() => document.addCustomTag(0, 'bad', 'text'), TypeError);
    assert.throws(() => document.customTagData(9), OfdError);
    const copy = Document.fromBytes(document.toBytes());
    try {
      assert.deepEqual(copy.customTags(), document.customTags());
      assert.deepEqual(copy.customTagData(0), payload);
      assert.deepEqual(copy.extensions(), document.extensions());
      assert.deepEqual(copy.validateReferences(), []);
      assert.throws(() => copy.updateCustomTag(-1, 'bad'), RangeError);
      assert.throws(() => copy.updateCustomTag(1, 'bad'), OfdError);
      assert.throws(() => copy.updateCustomTag(0, 'bad', {schemaPath: 'missing.xsd'}), OfdError);
      const updatedTag = copy.updateCustomTag(0, 'updated & invoice', {namespace: 'urn:updated'});
      assert.equal(updatedTag.path, tag.path);
      assert.equal(updatedTag.namespace, 'urn:updated');
      assert.deepEqual(copy.customTagData(0), payload);
      assert.throws(() => copy.replaceCustomTagData(-1, Buffer.from('<New/>')), RangeError);
      assert.throws(() => copy.replaceCustomTagData(0, Buffer.from('<broken>')), OfdError);
      assert.deepEqual(copy.customTagData(0), payload);
      const replacedTag = copy.replaceCustomTagData(0, Buffer.from('<New/>'));
      assert.notEqual(replacedTag.path, tag.path);
      assert.equal(replacedTag.type_id, updatedTag.type_id);
      assert.deepEqual(copy.customTagData(0), Buffer.from('<New/>'));
      const updatedExtension = { ...copy.extensions()[0], app_name: 'Updated & Co' };
      assert.throws(() => copy.updateExtension(-1, updatedExtension), RangeError);
      assert.throws(() => copy.updateExtension(1, updatedExtension), OfdError);
      copy.updateExtension(0, updatedExtension);
      assert.equal(copy.extensions()[0].app_name, 'Updated & Co');
      assert.throws(() => copy.removeCustomTag(-1), RangeError);
      assert.throws(() => copy.removeExtension(1), OfdError);
      copy.removeCustomTag(0);
      copy.removeExtension(0);
      assert.deepEqual(copy.customTags(), []);
      assert.deepEqual(copy.extensions(), []);
      assert.deepEqual(copy.entry(tag.path), payload);
      assert.deepEqual(copy.validateReferences(), []);
    } finally {
      copy.close();
    }
  } finally {
    document.close();
  }
});

test('create and remove a nested outline tree', () => {
  const document = Document.create();
  try {
    document.addBlankPage(40, 30);
    document.addBlankPage(40, 30);
    const destination = { page_id: document.pages()[0].id, kind: 'Fit' };
    const root = document.addOutline(0, '第一章 & Node', { destination });
    assert.equal(root, 0);
    assert.equal(document.addOutline(0, '第二章'), 1);
    const child = document.addOutline(0, '1.1 节', { parentIndex: root, expanded: false });
    assert.equal(child, 1);
    document.setOutlineTitle(root, '更新章节 & Node');
    document.setOutlineExpanded(root, false);
    document.setOutlineDestination(root, { page_id: document.pages()[1].id, kind: 'Fit' });
    document.addBookmark(0, '索引 & Node', { page_id: document.pages()[1].id, kind: 'Fit' });
    document.setOutlineBookmark(root, '索引 & Node');
    assert.equal(document.outlines()[0].bookmark_name, '索引 & Node');
    assert.throws(() => document.setOutlineBookmark(root, 'missing'), OfdError);
    document.setOutlineDestination(root, { page_id: document.pages()[1].id, kind: 'Fit' });
    assert.equal(document.outlines()[0].title, '更新章节 & Node');
    assert.equal(document.outlines()[0].expanded, false);
    assert.throws(() => document.setOutlineExpanded(root, 1), TypeError);
    assert.deepEqual(document.outlines().map((item) => item.title), ['更新章节 & Node', '1.1 节', '第二章']);
    assert.equal(document.outlines()[1].parent_index, 0);
    assert.equal(document.outlines()[1].expanded, false);
    assert.equal(document.outlines()[0].destination.page_id, document.pages()[1].id);
    assert.throws(() => document.setOutlineDestination(root, { page_id: 'missing', kind: 'Fit' }), OfdError);
    assert.throws(() => document.addOutline(0, 'bad', { parentIndex: 9 }), OfdError);
    assert.throws(() => document.addOutline(0, 'bad', { expanded: 1 }), TypeError);
    assert.throws(() => document.addOutline(0, 'bad', { parentIndex: -1 }), RangeError);
    assert.throws(() => document.removeOutline(9), OfdError);
    const copy = Document.fromBytes(document.toBytes());
    try {
      assert.deepEqual(copy.outlines(), document.outlines());
      copy.removeOutline(0);
      assert.deepEqual(copy.outlines().map((item) => item.title), ['第二章']);
      assert.deepEqual(copy.validateReferences(), []);
    } finally {
      copy.close();
    }
  } finally {
    document.close();
  }
});

test('snapshot and extract independent document versions', () => {
  const document = Document.create();
  try {
    document.addTextPage('第一版', 40, 30);
    assert.deepEqual(document.versions(), []);
    const first = document.addVersionSnapshot(0, 'v1', '初稿');
    assert.equal(first.id, 'v1');
    assert.ok(first.files.length > 0);
    document.addTextPage('第二版', 40, 30);
    const second = document.addVersionSnapshot(0, 'v2');
    assert.equal(second.id, 'v2');
    assert.deepEqual(document.versions().map((version) => version.id), ['v1', 'v2']);
    assert.equal(document.versions()[1].current, true);
    assert.throws(() => document.addVersionSnapshot(0, 'v1'), OfdError);
    assert.throws(() => document.addVersionSnapshot(0, 'bad/id'), OfdError);
    assert.throws(() => document.extractVersionSnapshot(2), OfdError);
    const original = document.extractVersionSnapshot(0);
    const latest = document.extractVersionSnapshot(1);
    try {
      assert.equal(original.pageCount, 1);
      assert.deepEqual(original.pageText(0), ['第一版']);
      assert.equal(latest.pageCount, 2);
      assert.deepEqual(latest.pageText(1), ['第二版']);
      assert.deepEqual(original.versions(), []);
      assert.deepEqual(original.validateReferences(), []);
      assert.deepEqual(latest.validateReferences(), []);
    } finally {
      original.close();
      latest.close();
    }
  } finally {
    document.close();
  }
});

test('edit page object rectangle, path, and mixed text clips', () => {
  const document = Document.create();
  try {
    document.addTextPage('font', 30, 30);
    const fontId = document.fonts()[0].id;
    const objectId = document.addPathObject(0, 'M 0 0 L 20 20', [0, 0, 20, 20]);
    const clips = () => document.pageObjectClips(0).find((item) => item.id === objectId);
    document.setPageObjectClipRects(0, objectId, [[0, 0, 10, 20]], true);
    assert.equal(clips().follows_ctm, true);
    assert.equal(clips().clips[0][0].kind, 'Path');
    assert.throws(() => document.setPageObjectClipRects(0, objectId, [], 1), TypeError);
    assert.throws(() => document.setPageObjectClipRects(0, objectId, [[0, 0, -1, 20]]), OfdError);
    assert.equal(clips().clips[0][0].boundary[2], 10);
    document.setPageObjectPathClips(0, objectId, [[{
      boundary: [0, 0, 10, 20], commands: 'M 0 0 L 10 0 L 10 20 L 0 20 C', even_odd: true,
    }]]);
    assert.equal(clips().clips[0][0].rule, 'Even-Odd');
    document.setPageObjectClips(0, objectId, [[
      {kind: 'Path', boundary: [0, 0, 5, 20], commands: 'M 0 0 L 5 0 L 5 20 L 0 20 C'},
      {kind: 'Text', boundary: [0, 0, 20, 20], font_id: fontId, size: 14, text: 'O', x: 10, y: 15},
    ]]);
    assert.deepEqual(clips().clips[0].map((area) => area.kind), ['Path', 'Text']);
    assert.equal(clips().clips[0][1].text, 'O');
    assert.throws(() => document.setPageObjectClips(0, objectId, [[]]), OfdError);
    assert.equal(clips().clips[0].length, 2);
    const reopened = Document.fromBytes(document.toBytes());
    try {
      assert.deepEqual(reopened.pageObjectClips(0), document.pageObjectClips(0));
      assert.deepEqual(reopened.validateReferences(), []);
    } finally {
      reopened.close();
    }
    document.setPageObjectClips(0, objectId, []);
    assert.deepEqual(clips().clips, []);
  } finally {
    document.close();
  }
});

test('inspect visual object attributes and edit direct object actions', () => {
  const document = Document.create();
  try {
    document.addBlankPage(40, 30);
    const objectId = document.addPathObject(0, 'M 0 0 L 20 0 L 20 20 C', [0, 0, 20, 20]);
    const pagePath = document.pages()[0].path;
    assert.equal(document.pageObjectClips(0).find((item) => item.id === objectId).clips.length, 0);
    assert.equal(document.pageObjectPaints(0).find((item) => item.id === objectId).kind, 'PathObject');
    assert.ok(document.pageObjectStrokeStyles(0).find((item) => item.id === objectId).style.line_width > 0);
    assert.deepEqual(document.pageObjectActions(0, objectId), []);
    const destination = { page_id: document.pages()[0].id, kind: 'Fit' };
    document.addBookmark(0, 'chapter', destination);
    document.addAttachment('note.txt', Buffer.from('hello'));
    const attachmentId = document.attachments()[0].id;
    document.addPageObjectUriAction(0, objectId, 'https://example.org/?a=1&b=2');
    document.addPageObjectGotoAction(0, objectId, destination);
    document.addPageObjectBookmarkAction(0, objectId, 'chapter');
    document.addPageObjectAttachmentAction(0, objectId, attachmentId, true);
    document.setPageObjectActionRegions(0, objectId, 0, [[2, 2, 5, 5], [10, 2, 5, 5]]);
    const actions = document.pageObjectActions(0, objectId);
    assert.deepEqual(actions.map((action) => action.kind), ['URI', 'Goto', 'Goto', 'GotoA']);
    assert.equal(actions[0].attributes.URI, 'https://example.org/?a=1&b=2');
    assert.equal(actions[0].region.areas.length, 2);
    assert.equal(actions[1].destination.page_id, destination.page_id);
    assert.equal(actions[2].bookmark, 'chapter');
    assert.equal(actions[3].attributes.AttachID, attachmentId);
    assert.deepEqual(document.pageSourceObjectActions(0, pagePath, objectId), actions);
    document.setPageObjectActionRegions(0, objectId, 0, []);
    assert.equal(document.pageObjectActions(0, objectId)[0].region, null);
    document.setPageObjectActionRegions(0, objectId, 0, [[2, 2, 5, 5], [10, 2, 5, 5]]);
    assert.throws(() => document.addPageObjectUriAction(0, 'missing', 'https://example.org'), OfdError);
    assert.throws(() => document.addPageObjectBookmarkAction(0, objectId, 'missing'), OfdError);
    assert.throws(() => document.addPageObjectAttachmentAction(0, objectId, 'missing'), OfdError);
    assert.throws(() => document.addPageObjectAttachmentAction(0, objectId, attachmentId, 1), TypeError);
    assert.throws(() => document.setPageObjectActionRegions(0, objectId, 0, [[0, 0, 0, 1]]), RangeError);
    assert.throws(() => document.removePageObjectAction(0, objectId, 9), OfdError);
    document.removePageObjectAction(0, objectId, 1);
    const copy = Document.fromBytes(document.toBytes());
    try {
      assert.deepEqual(copy.pageObjectActions(0, objectId), document.pageObjectActions(0, objectId));
      assert.deepEqual(copy.pageObjectPaints(0), document.pageObjectPaints(0));
      assert.deepEqual(copy.validateReferences(), []);
    } finally {
      copy.close();
    }
  } finally {
    document.close();
  }
});

test('edit page layers, object order, and page blocks', () => {
  const document = Document.create();
  try {
    document.addBlankPage(40, 30);
    const a = document.addPathObject(0, 'M 0 0 L 5 5', [0, 0, 5, 5]);
    const b = document.addPathObject(0, 'M 0 0 L 6 6', [5, 0, 6, 6]);
    const c = document.addPathObject(0, 'M 0 0 L 7 7', [10, 0, 7, 7]);
    const body = document.pageLayers(0)[0].id;
    const layer = document.addPageLayer(0, 'Foreground');
    document.movePageObjectToLayer(0, c, layer);
    assert.equal(document.pageObjects(0).find((object) => object.id === c).layer_id, layer);
    assert.equal(document.pageLayers(0).find((item) => item.id === layer).object_count, 1);
    document.setPageLayerType(0, layer, 'Background');
    document.setPageLayerName(0, layer, '图层 & A');
    assert.equal(document.pageLayers(0).find((item) => item.id === layer).layer_type, 'Background');
    assert.equal(document.pageLayers(0).find((item) => item.id === layer).name, '图层 & A');
    assert.throws(() => document.setPageLayerName(0, layer, 'bad\0name'), TypeError);
    document.setPageLayerName(0, layer, null);
    assert.equal(document.pageLayers(0).find((item) => item.id === layer).name, null);
    document.setPageLayerDrawParam(0, layer, null);
    assert.throws(() => document.removePageLayer(0, layer), OfdError);
    document.movePageObjectToLayer(0, c, body);
    document.removePageLayer(0, layer);
    document.movePageObject(0, c, a);
    assert.deepEqual(document.pageObjects(0).map((object) => object.id), [c, a, b]);
    document.groupPageObjects(0, a, b);
    assert.deepEqual(document.pageObjects(0).map((object) => object.id), [c, a, b]);
    document.ungroupPageObjects(0, a);
    assert.throws(() => document.movePageObjectToLayer(0, c, 'missing'), OfdError);
    const copy = Document.fromBytes(document.toBytes());
    try {
      assert.deepEqual(copy.pageObjects(0).map((object) => object.id), [c, a, b]);
      assert.deepEqual(copy.validateReferences(), []);
    } finally {
      copy.close();
    }
  } finally {
    document.close();
  }
});

test('edit page object geometry and display attributes', () => {
  const document = Document.create();
  try {
    document.addBlankPage(40, 30);
    const id = document.addPathObject(0, 'M 0 0 L 10 0 L 10 10 C', [0, 0, 10, 10]);
    document.setPageObjectBoundary(0, id, [12, 2, 10, 10]);
    document.setPageObjectCtm(0, id, [1, 0, 0, 1, 3, 4]);
    document.setPageObjectVisible(0, id, false);
    document.setPageObjectAlpha(0, id, 128);
    document.setPageObjectName(0, id, 'highlight');
    document.setPageObjectBlendMode(0, id, 'Multiply');
    document.setPageObjectDrawParam(0, id, null);
    const object = document.pageObjects(0).find((item) => item.id === id);
    assert.deepEqual(object.boundary, [12, 2, 10, 10]);
    assert.deepEqual(object.ctm, [1, 0, 0, 1, 3, 4]);
    assert.equal(object.visible, false);
    assert.equal(object.alpha, 128);
    assert.equal(object.name, 'highlight');
    assert.equal(object.blend_mode, 'Multiply');
    assert.throws(() => document.setPageObjectBoundary(0, id, [0, 0, 0, 1]), RangeError);
    assert.throws(() => document.setPageObjectCtm(0, id, [1, 0, 0]), RangeError);
    assert.throws(() => document.setPageObjectVisible(0, id, 1), TypeError);
    assert.throws(() => document.setPageObjectAlpha(0, id, 256), RangeError);
    document.setPageObjectVisible(0, id, true);
    document.setPageObjectCtm(0, id);
    document.setPageObjectName(0, id);
    document.setPageObjectBlendMode(0, id);
    const copy = Document.fromBytes(document.toBytes());
    try {
      assert.deepEqual(copy.pageObjects(0), document.pageObjects(0));
      assert.deepEqual(copy.validateReferences(), []);
    } finally {
      copy.close();
    }
  } finally {
    document.close();
  }
});

test('edit path commands and stroke geometry without corrupting failed edits', () => {
  const document = Document.create();
  try {
    document.addBlankPage(40, 30);
    const id = document.addPathObject(0, 'M 0 0 L 10 0', [1, 2, 10, 10],
      { stroke: true, fill: false });
    document.setPagePathData(0, id, 'M 0 0 L 10 0 L 10 10');
    document.setPagePathStrokeStyle(0, id, {
      line_width: 1.25, cap: 'round', join: 'bevel', dash_sequence: [2, 1], dash_offset: 0.5,
    });
    let style = document.pageObjectStrokeStyles(0).find((item) => item.id === id).style;
    assert.equal(style.line_width, 1.25);
    assert.equal(style.cap, 'round');
    assert.deepEqual(style.dash_sequence, [2, 1]);
    document.setPagePathDashSequence(0, id, [3, 2], 1);
    style = document.pageObjectStrokeStyles(0).find((item) => item.id === id).style;
    assert.deepEqual(style.dash_sequence, [3, 2]);
    assert.equal(style.dash_offset, 1);
    assert.ok(document.pageSvg(0).includes('stroke-dasharray="3 2"'));
    const before = document.toBytes();
    assert.throws(() => document.setPagePathData(0, id, 'invalid command'), OfdError);
    assert.throws(() => document.setPagePathStrokeStyle(0, id, { line_width: -1 }), OfdError);
    assert.throws(() => document.setPagePathDashSequence(0, id, [0, 2]), OfdError);
    assert.throws(() => document.setPagePathDashSequence(0, id, '3 2'), TypeError);
    assert.deepEqual(document.toBytes(), before);
    const copy = Document.fromBytes(before);
    try {
      assert.deepEqual(copy.pagePaths(0), document.pagePaths(0));
      assert.deepEqual(copy.pageObjectStrokeStyles(0), document.pageObjectStrokeStyles(0));
      assert.deepEqual(copy.validateReferences(), []);
    } finally {
      copy.close();
    }
    document.setPagePathDashSequence(0, id);
    assert.equal(document.pageObjectStrokeStyles(0).find((item) => item.id === id).style.dash_sequence, null);
  } finally {
    document.close();
  }
});

test('edit positioned text runs and typography across an OFD round trip', () => {
  const document = Document.create();
  try {
    document.addTextPage('AB', 40, 30);
    const objectId = document.pageTextObjects(0)[0].id;
    const fontId = document.fonts()[0].id;
    document.setPageTextCode(0, objectId, 0, 'A\0B');
    document.appendPageTextCode(0, objectId, 'C', { x: 12, y: 5 });
    document.setPageTextCodeLayout(0, objectId, 1, { x: 14, y: 6 });
    document.setPageTextTypography(0, objectId, {
      size: 4, h_scale: 0.8, weight: 700, italic: true,
    });
    document.setPageTextFont(0, objectId, fontId);
    document.setPageTextPaintFlags(0, objectId, { fill: true, stroke: true });
    assert.deepEqual(document.pageText(0), ['A\0B', 'C']);
    let text = document.pageTextObjects(0)[0];
    assert.equal(text.codes[1].x, 14);
    assert.equal(text.size, 4);
    assert.equal(text.weight, 700);
    assert.equal(document.pageObjectPaints(0)[0].stroke_enabled, true);
    const before = document.toBytes();
    assert.throws(() => document.setPageTextTypography(0, objectId, { size: 4, weight: 750 }), OfdError);
    assert.throws(() => document.setPageTextCodeLayout(0, objectId, 0, {}), OfdError);
    assert.throws(() => document.setPageTextFont(0, objectId, 'missing'), OfdError);
    assert.throws(() => document.setPageTextPaintFlags(0, objectId, { fill: 'yes' }), OfdError);
    assert.deepEqual(document.toBytes(), before);
    const copy = Document.fromBytes(before);
    try {
      assert.deepEqual(copy.pageText(0), ['A\0B', 'C']);
      assert.deepEqual(copy.pageTextObjects(0), document.pageTextObjects(0));
      assert.deepEqual(copy.validateReferences(), []);
    } finally {
      copy.close();
    }
    document.removePageTextCode(0, objectId, 0);
    assert.deepEqual(document.pageText(0), ['C']);
    document.setPageTextPaintFlags(0, objectId);
    assert.equal(document.pageObjectPaints(0)[0].stroke_enabled, false);
  } finally {
    document.close();
  }
});

test('open a Java OFD sample and preserve image bytes', () => {
  const file = path.resolve(__dirname, '../../../../ofdrw-reader/src/test/resources/发票示例.ofd');
  const document = Document.open(file);
  try {
    assert.ok(document.pageCount > 0);
    assert.ok(document.pages()[0].path.endsWith('.xml'));
    assert.ok(document.pageImageData(0, 0).length > 0);
    assert.deepEqual(document.validateReferences(), []);
  } finally {
    document.close();
  }
});

test('decrypt Java password package and round-trip Node password recipients', () => {
  const java = Document.fromBytes(fs.readFileSync(fixture('java-password-encrypted.ofd')));
  try {
    assert.equal(java.isEncrypted, true);
    assert.deepEqual(java.encryptionInfo()[0].usernames, ['777']);
    assert.throws(() => java.decryptWithPassword('wrong', '777'), OfdError);
    const restored = java.decryptWithPassword('12345678', '777');
    try {
      assert.equal(restored.isEncrypted, false);
      assert.ok(restored.pageCount > 0);
      const expected = Document.open(javaPlainFixture);
      try {
        assertSameEntries(restored, expected);
      } finally {
        expected.close();
      }
    } finally {
      restored.close();
    }
  } finally {
    java.close();
  }

  const plain = Document.create();
  try {
    plain.addTextPage('Node 密码', 40, 30);
    const encrypted = plain.encryptWithPasswords([
      { username: 'alice', password: 'first-pass' },
      { username: '李四', password: 'second-pass', userType: 'Owner' },
    ]);
    try {
      assert.equal(plain.isEncrypted, false);
      assert.equal(encrypted.isEncrypted, true);
      assert.deepEqual(encrypted.encryptionInfo()[0].usernames, ['alice', '李四']);
      const reopened = Document.fromBytes(encrypted.toBytes());
      try {
        assert.throws(() => reopened.decryptWithPassword('first-pass', '李四'), OfdError);
        for (const [password, username] of [['first-pass', 'alice'], ['second-pass', '李四']]) {
          const restored = reopened.decryptWithPassword(password, username);
          try {
            assert.deepEqual(restored.pageText(0), ['Node 密码']);
            assertSameEntries(restored, plain);
          } finally {
            restored.close();
          }
        }
      } finally {
        reopened.close();
      }
    } finally {
      encrypted.close();
    }
    assert.throws(() => plain.encryptWithPasswords(null), TypeError);
    assert.throws(() => plain.encryptWithPasswords([{ username: 'a\0b', password: 'x' }]), TypeError);
    assert.throws(() => plain.encryptWithPasswords([{ username: 'a', password: 'x', userType: 'Admin' }]), OfdError);
  } finally {
    plain.close();
  }
});

test('decrypt Java SM2 package and round-trip Node certificate recipient', () => {
  const key = fs.readFileSync(fixture('user-private-key.bin'));
  const certificate = fs.readFileSync(fixture('user-cert.der'));
  const java = Document.fromBytes(fs.readFileSync(fixture('java-cert-encrypted.ofd')));
  try {
    assert.equal(java.isEncrypted, true);
    const javaKey = fs.readFileSync(fixture('java-sm2-private-key.bin'));
    assert.throws(() => java.decryptWithSm2PrivateKey(key, '777'), OfdError);
    const restored = java.decryptWithSm2PrivateKey(javaKey, '777');
    try {
      assert.ok(restored.pageCount > 0);
      assert.equal(restored.isEncrypted, false);
      const expected = Document.open(javaPlainFixture);
      try {
        assertSameEntries(restored, expected);
      } finally {
        expected.close();
      }
    } finally {
      restored.close();
    }
  } finally {
    java.close();
  }

  const plain = Document.create();
  try {
    plain.addTextPage('Node SM2', 40, 30);
    const encrypted = plain.encryptWithCertificates([
      { username: '证书用户', certificate, userType: 'Owner' },
    ]);
    try {
      assert.equal(encrypted.isEncrypted, true);
      const reopened = Document.fromBytes(encrypted.toBytes());
      try {
        assert.throws(() => reopened.decryptWithSm2PrivateKey(Buffer.alloc(32), '证书用户'), OfdError);
        const restored = reopened.decryptWithSm2PrivateKey(key, '证书用户');
        try {
          assert.deepEqual(restored.pageText(0), ['Node SM2']);
          assertSameEntries(restored, plain);
        } finally {
          restored.close();
        }
      } finally {
        reopened.close();
      }
    } finally {
      encrypted.close();
    }
    assert.throws(() => plain.encryptWithCertificates([{ username: 'bad', certificate: 'der' }]), TypeError);
  } finally {
    plain.close();
  }
});

test('encrypt one package for password and certificate users', () => {
  const certificate = fs.readFileSync(fixture('user-cert.der'));
  const key = fs.readFileSync(fixture('user-private-key.bin'));
  const plain = Document.create();
  try {
    plain.addTextPage('mixed users', 40, 30);
    const encrypted = plain.encryptWithRecipients([
      { type: 'password', username: 'password-user', password: 'secret' },
      { type: 'certificate', username: 'certificate-user', certificate },
    ]);
    try {
      assert.equal(encrypted.isEncrypted, true);
      assert.deepEqual(encrypted.encryptionInfo()[0].usernames, ['password-user', 'certificate-user']);
      const reopened = Document.fromBytes(encrypted.toBytes());
      try {
        const byPassword = reopened.decryptWithPassword('secret', 'password-user');
        const byCertificate = reopened.decryptWithSm2PrivateKey(key, 'certificate-user');
        try {
          assertSameEntries(byPassword, plain);
          assertSameEntries(byCertificate, plain);
        } finally {
          byPassword.close();
          byCertificate.close();
        }
      } finally {
        reopened.close();
      }
    } finally {
      encrypted.close();
    }
    assert.throws(() => plain.encryptWithRecipients([{ type: 'unknown', username: 'a' }]), TypeError);
  } finally {
    plain.close();
  }
});

test('import mirrored PDF text with a negative horizontal scale', () => {
  const document = Document.fromPdfBytes(fs.readFileSync(fixture('negative-horizontal-scale.pdf')));
  try {
    assert.deepEqual(document.pageText(0), ['AB', 'C', 'D']);
    assert.deepEqual(document.pageTextObjects(0).map((item) => item.h_scale), [0.5, 0.5, null]);
    const objects = document.pageObjects(0);
    assert.equal(objects[0].ctm[0], -1);
    assert.ok(Math.abs(objects[2].boundary[0] - 11 * 25.4 / 72) < 1e-6);
    assert.deepEqual(document.validateReferences(), []);
    const reopened = Document.fromBytes(document.toBytes());
    try {
      assert.deepEqual(reopened.pageText(0), ['AB', 'C', 'D']);
    } finally {
      reopened.close();
    }
  } finally {
    document.close();
  }
});

test('import PDF and duplicate a path object with a fresh ID', () => {
  const document = Document.fromPdfBytes(fs.readFileSync(fixture('vector.pdf')));
  try {
    const original = document.pagePaths(0);
    assert.equal(original.length, 2);
    const id = document.duplicatePageObject(0, original[0].id);
    const paths = document.pagePaths(0);
    assert.deepEqual(paths.map((item) => item.id), [original[0].id, id, original[1].id]);
    assert.notEqual(id, original[0].id);
    assert.deepEqual(document.validateReferences(), []);
    document.removePageObject(0, id);
    assert.deepEqual(document.pagePaths(0), original);
  } finally {
    document.close();
  }
});

test('import and export a standalone PDF popup through Node.js', () => {
  const document = Document.fromPdfBytes(fs.readFileSync(fixture('standalone-popup.pdf')));
  try {
    const annotations = document.annotations();
    assert.equal(annotations.length, 1);
    assert.equal(annotations[0].remark, 'standalone review');
    assert.equal(annotations[0].attributes.Creator, 'Reviewer');
    assert.equal(annotations[0].parameters.PDFPopupStandalone, 'true');
    assert.equal(annotations[0].parameters.PDFTextOpen, 'true');
    assert.deepEqual(document.validateReferences(), []);
    const reopened = Document.fromPdfBytes(document.toPdf());
    try {
      assert.equal(reopened.annotations()[0].remark, 'standalone review');
      assert.equal(reopened.annotations()[0].parameters.PDFPopupStandalone, 'true');
    } finally { reopened.close(); }
  } finally { document.close(); }
});

test('import and export PDF Ink strokes through Node.js', () => {
  const document = Document.fromPdfBytes(fs.readFileSync(fixture('ink.pdf')));
  try {
    const ink = document.annotations()[0];
    assert.equal(ink.subtype, 'Ink');
    assert.equal(ink.remark, 'two pen strokes');
    assert.equal(ink.attributes.Creator, 'Reviewer');
    assert.equal(document.annotationObjects(0, ink.id).length, 2);
    assert.deepEqual(document.validateReferences(), []);
    const reopened = Document.fromPdfBytes(document.toPdf());
    try {
      assert.equal(reopened.annotations()[0].subtype, 'Ink');
      assert.equal(reopened.annotationObjects(0, reopened.annotations()[0].id).length, 2);
    } finally { reopened.close(); }
  } finally { document.close(); }
});

test('import and export PDF Polygon and PolyLine through Node.js', () => {
  for (const [name, subtype, pathCount] of [
    ['polygon.pdf', 'Polygon', 1],
    ['polyline.pdf', 'PolyLine', 3],
  ]) {
    const document = Document.fromPdfBytes(fs.readFileSync(fixture(name)));
    try {
      const shape = document.annotations()[0];
      assert.equal(shape.subtype, subtype);
      assert.equal(shape.attributes.Creator, 'Reviewer');
      assert.equal(document.annotationObjects(0, shape.id).length, pathCount);
      assert.deepEqual(document.validateReferences(), []);
      const reopened = Document.fromPdfBytes(document.toPdf());
      try {
        assert.equal(reopened.annotations()[0].subtype, subtype);
        assert.equal(reopened.annotationObjects(0, reopened.annotations()[0].id).length, pathCount);
      } finally { reopened.close(); }
    } finally { document.close(); }
  }
});

test('import and export a linked PDF popup through Node.js', () => {
  const document = Document.fromPdfBytes(fs.readFileSync(fixture('popup.pdf')));
  try {
    const annotations = document.annotations();
    assert.equal(annotations.length, 1);
    assert.equal(annotations[0].subtype, 'Text');
    assert.equal(annotations[0].remark, 'review');
    assert.equal(annotations[0].parameters.PDFPopupOpen, 'true');
    assert.deepEqual(document.validateReferences(), []);
    const reopened = Document.fromPdfBytes(document.toPdf());
    try {
      assert.equal(reopened.annotations()[0].parameters.PDFPopupOpen, 'true');
      assert.equal(reopened.annotations()[0].parameters.PDFPopupBoundary,
        annotations[0].parameters.PDFPopupBoundary);
    } finally { reopened.close(); }
  } finally { document.close(); }
});

test('import indirect optional PDF content through Node.js', () => {
  const document = Document.fromPdfBytes(fs.readFileSync(fixture('optional-content-indirect.pdf')));
  try {
    assert.deepEqual(document.pageText(0), ['B']);
    assert.deepEqual(document.validateReferences(), []);
    const reopened = Document.fromBytes(document.toBytes());
    try {
      assert.deepEqual(reopened.pageText(0), ['B']);
    } finally {
      reopened.close();
    }
  } finally {
    document.close();
  }
});

test('import PDF normal annotation appearances through Node.js', () => {
  for (const name of ['matrix', 'widget', 'link', 'image', 'nonprint', 'no-view']) {
    const document = Document.fromPdfBytes(fs.readFileSync(fixture(`annotation-ap-${name}.pdf`)));
    try {
      assert.equal(document.pageCount, 1);
      const annotations = document.annotations();
      assert.equal(annotations.length, 1);
      assert.equal(annotations[0].remark, 'custom appearance');
      if (name === 'link') assert.equal(annotations[0].actions[0].attributes.URI, 'https://example.com/appearance');
      const png = document.pagePng(0, 144);
      assert.deepEqual(document.validateReferences(), []);
      const reopened = Document.fromBytes(document.toBytes());
      try {
        assert.deepEqual(reopened.pagePng(0, 144), png);
        assert.equal(reopened.toPdf().subarray(0, 5).toString(), '%PDF-');
        const converted = Document.fromPdfBytes(reopened.toPdf());
        try {
          const annotations = converted.annotations();
          assert.equal(annotations.length, 1);
          assert.equal(annotations[0].remark, 'custom appearance');
          if (name === 'nonprint') {
            assert.equal(annotations[0].attributes.Print, 'false');
            assert.ok(converted.pageSvg(0).includes('#ff0000'));
          } else if (name === 'no-view') {
            assert.equal(annotations[0].attributes.Visible, 'false');
            assert.equal(annotations[0].attributes.Print ?? 'true', 'true');
            assert.ok(!converted.pageSvg(0).includes('#ff0000'));
          }
        } finally { converted.close(); }
      } finally { reopened.close(); }
    } finally { document.close(); }
  }
});

test('import a sixteen-bit PDF image through the C ABI', () => {
  const document = Document.fromPdfBytes(fs.readFileSync(fixture('gray-16-bit.pdf')));
  try {
    assert.equal(document.pageImages(0).length, 1);
    assert.deepEqual(document.pageImageData(0, 0).subarray(0, 8), Buffer.from([137, 80, 78, 71, 13, 10, 26, 10]));
    assert.deepEqual(document.validateReferences(), []);
    const copy = Document.fromBytes(document.toBytes());
    try {
      assert.deepEqual(copy.pageImageData(0, 0), document.pageImageData(0, 0));
    } finally {
      copy.close();
    }
  } finally {
    document.close();
  }
});

test('import JBIG2 PDF images with and without global dictionaries through the C ABI', () => {
  for (const name of ['jbig2-mmr.pdf', 'jbig2-globals.pdf']) {
    const document = Document.fromPdfBytes(fs.readFileSync(fixture(name)));
    try {
      assert.equal(document.pageImages(0).length, 1);
      assert.deepEqual(document.pageImageData(0, 0).subarray(0, 8), Buffer.from([137, 80, 78, 71, 13, 10, 26, 10]));
      assert.deepEqual(document.validateReferences(), []);
      const copy = Document.fromBytes(document.toBytes());
      try {
        assert.deepEqual(copy.pageImageData(0, 0), document.pageImageData(0, 0));
      } finally {
        copy.close();
      }
    } finally {
      document.close();
    }
  }
});

test('import PDF axial and radial sampled shading functions through the C ABI', () => {
  for (const filename of [
    'shading-sampled-axial.pdf', 'shading-sampled-radial.pdf',
    'shading-sampled-cubic-axial.pdf', 'shading-sampled-cubic-radial.pdf',
    'shading-nested-stitching-axial.pdf', 'shading-nested-stitching-radial.pdf',
    'shading-indirect-functions-axial.pdf', 'shading-indirect-functions-radial.pdf',
    'shading-calculator-axial.pdf', 'shading-calculator-radial.pdf',
    'shading-function-array-axial.pdf', 'shading-function-array-radial.pdf',
  ]) {
    const document = Document.fromPdfBytes(fs.readFileSync(fixture(filename)));
    try {
      const svg = document.pageSvg(0);
      for (const color of ['#ff0000', '#00ff00', '#0000ff']) assert.ok(svg.includes(color));
      assert.deepEqual(document.validateReferences(), []);
    } finally {
      document.close();
    }
  }
});

test('import PDF Gouraud meshes and preserve their paints and rasters', () => {
  for (const [name, kind] of [
    ['mesh-free-rgb', 'Gouraud'],
    ['mesh-lattice-rgb', 'LaGouraud'],
    ['mesh-packed-gray', 'Gouraud'],
    ['mesh-wide-rgb', 'Gouraud'],
    ['mesh-lattice-clipped', 'LaGouraud'],
    ['mesh-cmyk', 'Gouraud'],
    ['mesh-calgray', 'Gouraud'],
    ['mesh-function-array', 'Gouraud'],
    ['mesh-sampled-function', 'Gouraud'],
    ['mesh-stitching-function', 'Gouraud'],
    ['mesh-calculator-function', 'Gouraud'],
    ['patch-coons-flat', 'Gouraud'],
    ['patch-coons-curved', 'Gouraud'],
    ['patch-tensor-curved', 'Gouraud'],
    ['patch-tensor-folded', 'Gouraud'],
    ['patch-coons-reuse-1', 'Gouraud'],
    ['patch-coons-reuse-2', 'Gouraud'],
    ['patch-coons-reuse-3', 'Gouraud'],
    ['patch-tensor-reuse', 'Gouraud'],
    ['patch-coons-function', 'Gouraud'],
    ['patch-tensor-clipped', 'Gouraud'],
    ['patch-coons-wide', 'Gouraud'],
    ['patch-coons-packed', 'Gouraud'],
    ['patch-coons-calgray', 'Gouraud'],
    ['patch-tensor-cmyk', 'Gouraud'],
    ['mesh-function', 'Gouraud'],
  ]) {
    const document = Document.fromPdfBytes(fs.readFileSync(fixture(name + '.pdf')));
    try {
      const paints = document.pageObjectPaints(0);
      assert.equal(paints.length, 1);
      assert.equal(paints[0].fill.kind, kind);
      const png = document.pagePng(0, 72);
      const reopened = Document.fromBytes(document.toBytes());
      try {
        assert.deepEqual(reopened.pagePng(0, 72), png);
        assert.deepEqual(reopened.pageObjectPaints(0), paints);
        assert.deepEqual(reopened.validateReferences(), []);
      } finally {
        reopened.close();
      }
    } finally {
      document.close();
    }
  }
});

test('import two-input PDF function shadings through the C ABI', () => {
  for (const name of [
    'plane-calculator-rgb',
    'plane-calculator-step',
    'plane-component-array',
    'plane-sampled-rgb',
    'plane-sampled-packed',
    'plane-sampled-cubic',
    'plane-sampled-cubic-small',
    'plane-sampled-reversed',
    'plane-cmyk',
    'plane-calgray',
    'plane-transformed',
    'plane-bbox-background',
  ]) {
    const document = Document.fromPdfBytes(fs.readFileSync(fixture(name + '.pdf')));
    try {
      const images = document.pageImages(0);
      assert.equal(images.length, 1);
      const png = document.pagePng(0, 72);
      const reopened = Document.fromBytes(document.toBytes());
      try {
        assert.deepEqual(reopened.pagePng(0, 72), png);
        assert.deepEqual(reopened.pageImages(0), images);
        assert.deepEqual(reopened.validateReferences(), []);
      } finally { reopened.close(); }
    } finally { document.close(); }
  }
});

test('import a Group 4 fax PDF image through the C ABI', () => {
  const document = Document.fromPdfBytes(fs.readFileSync(fixture('ccitt-g4.pdf')));
  try {
    assert.equal(document.pageImages(0).length, 1);
    assert.deepEqual(document.pageImageData(0, 0).subarray(0, 8), Buffer.from([137, 80, 78, 71, 13, 10, 26, 10]));
    assert.deepEqual(document.pagePng(0, 72).subarray(0, 8), Buffer.from([137, 80, 78, 71, 13, 10, 26, 10]));
    assert.deepEqual(document.validateReferences(), []);
  } finally {
    document.close();
  }
});

test('duplicate an object with nested clip IDs through the C ABI', () => {
  const document = Document.create();
  try {
    document.addBlankPage(40, 30);
    const objectId = document.addPathObject(0, 'M 0 0 L 20 0 L 20 20 C', [0, 0, 20, 20]);
    const pagePath = document.pages()[0].path;
    const rootPath = 'Doc_0/Document.xml';
    const root = document.entry(rootPath).toString('utf8');
    const max = Number(root.match(/<ofd:MaxUnitID>(\d+)<\/ofd:MaxUnitID>/)[1]);
    const clip = `<ofd:Clips><ofd:Clip><ofd:Area><ofd:Path ID="${max + 1}" Boundary="0 0 10 10"><ofd:AbbreviatedData>M 0 0 L 10 0 L 10 10 C</ofd:AbbreviatedData></ofd:Path></ofd:Area></ofd:Clip></ofd:Clips>`;
    const page = document.entry(pagePath).toString('utf8');
    document.setEntry(pagePath, Buffer.from(page.replace('</ofd:PathObject>', `${clip}</ofd:PathObject>`)));
    assert.ok(document.validateReferences().some((issue) => issue.code === 'id_exceeds_max'));
    assert.throws(() => document.duplicatePageObject(0, objectId), OfdError);
    document.setEntry(rootPath, Buffer.from(root.replace(`<ofd:MaxUnitID>${max}</ofd:MaxUnitID>`, `<ofd:MaxUnitID>${max + 1}</ofd:MaxUnitID>`)));
    const copyId = document.duplicatePageObject(0, objectId);
    assert.equal(copyId, String(max + 2));
    const updated = document.entry(pagePath).toString('utf8');
    assert.ok(updated.includes(`<ofd:Path ID="${max + 3}"`));
    assert.ok(document.entry(rootPath).toString('utf8').includes(`<ofd:MaxUnitID>${max + 3}</ofd:MaxUnitID>`));
    assert.equal(document.pagePaths(0).length, 2);
    assert.deepEqual(document.validateReferences(), []);
  } finally {
    document.close();
  }
});

test('create positioned text, dashed path, and image objects', () => {
  const document = Document.create();
  try {
    document.addBlankPage(40, 30);
    const png = document.pagePng(0, 25.4);
    const textId = document.addTextObject(0, 'A\0B', [2, 2, 15, 5], { fill_color: [0, 0, 255] });
    const pathId = document.addPathObject(0, 'M 0 0 L 10 0', [5, 10, 10, 1], {
      stroke_color: [255, 0, 0], dash_sequence: [1, 2, 3, 4], dash_offset: 2,
    });
    const imageId = document.addImageObject(0, png, 'png', [20, 2, 10, 10]);
    assert.deepEqual(document.pageText(0), ['A\0B']);
    assert.equal(document.pageTextObjects(0)[0].id, textId);
    assert.equal(document.pagePaths(0)[0].id, pathId);
    assert.equal(document.pageImages(0)[0].id, imageId);
    assert.deepEqual(document.pageImageData(0, 0), png);
    const svg = document.pageSvg(0);
    assert.match(svg, /A/);
    assert.match(svg, /#ff0000/);
    assert.match(svg, /data:image\/png;base64,/);
    assert.deepEqual(document.validateReferences(), []);
    const copy = Document.fromBytes(document.toBytes());
    try {
      assert.deepEqual(copy.pageText(0), ['A\0B']);
      assert.equal(copy.pagePaths(0)[0].id, pathId);
      assert.deepEqual(copy.pageImageData(0, 0), png);
      assert.deepEqual(copy.validateReferences(), []);
    } finally {
      copy.close();
    }
  } finally {
    document.close();
  }
});

test('edit image mask, substitution, and border through Node.js', () => {
  const document = Document.create();
  try {
    document.addBlankPage(20, 20);
    const png = document.pagePng(0, 25.4);
    const id = document.addImageObject(0, png, 'png', [2, 2, 10, 10]);
    const alternate = document.addPageMultimediaResource(0, 'Image', 'png', png);
    document.setPageImageMask(0, id, alternate);
    document.setPageImageSubstitution(0, id, alternate);
    document.setPageImageBorder(0, id, { line_width: 0.5, color: [255, 0, 0],
      dash_pattern: [1, 1] });
    let xml = document.entry('Doc_0/Pages/Page_0/Content.xml').toString('utf8');
    assert.match(xml, new RegExp(`ImageMask="${alternate}"`));
    assert.match(xml, new RegExp(`Substitution="${alternate}"`));
    assert.match(xml, /<ofd:Border/);
    assert.match(document.pageSvg(0), /#ff0000/);
    assert.deepEqual(document.validateReferences(), []);
    const copy = Document.fromBytes(document.toBytes());
    try { assert.match(copy.pageSvg(0), /#ff0000/); } finally { copy.close(); }

    const before = document.toBytes();
    assert.throws(() => document.setPageImageMask(0, id, '999'), OfdError);
    assert.throws(() => document.setPageImageSubstitution(0, id, '999'), OfdError);
    assert.throws(() => document.setPageImageBorder(0, id, { dash_pattern: [1, 0] }), OfdError);
    assert.deepEqual(document.toBytes(), before);
    document.setPageImageMask(0, id);
    document.setPageImageSubstitution(0, id);
    document.setPageImageBorder(0, id);
    xml = document.entry('Doc_0/Pages/Page_0/Content.xml').toString('utf8');
    assert.doesNotMatch(xml, /ImageMask=|Substitution=|<ofd:Border/);
    assert.deepEqual(document.validateReferences(), []);
  } finally {
    document.close();
  }
});

test('create and append image pages in selected document bodies', () => {
  const source = Document.create();
  let png;
  try {
    source.addBlankPage(8, 6);
    png = source.pagePng(0, 25.4);
  } finally {
    source.close();
  }

  const document = Document.create();
  try {
    assert.throws(() => document.addImagePage(Buffer.from('bad'), 'png', 40, 30), OfdError);
    assert.equal(document.pageCount, 0);
    document.addImagePage(png, 'png', 40, 30);
    assert.deepEqual(document.pageImageData(0, 0), png);
    assert.equal(document.pages()[0].physical_box[2], 40);
    const reopened = Document.fromBytes(document.toBytes());
    try {
      assert.equal(reopened.appendDocumentBody(), 1);
      reopened.appendImagePage(1, png, 'png', 50, 35);
      assert.deepEqual(reopened.pages().map((page) => page.document_index), [0, 1]);
      assert.deepEqual(reopened.pageImageData(1, 0), png);
      assert.equal(reopened.pages()[1].physical_box[2], 50);
      const before = reopened.toBytes();
      assert.throws(() => reopened.appendImagePage(2, png, 'png'), OfdError);
      assert.throws(() => reopened.appendImagePage(1, Buffer.from('bad'), 'png'), OfdError);
      assert.deepEqual(reopened.toBytes(), before);
      assert.throws(() => reopened.appendImagePage(-1, png, 'png'), RangeError);
      assert.throws(() => reopened.addImagePage('bad', 'png'), TypeError);
      assert.deepEqual(reopened.validateReferences(), []);
      const copy = Document.fromBytes(reopened.toBytes());
      try {
        assert.deepEqual(copy.pageImageData(1, 0), png);
        assert.deepEqual(copy.validateReferences(), []);
      } finally {
        copy.close();
      }
    } finally {
      reopened.close();
    }
  } finally {
    document.close();
  }
});

test('create and append vector path pages in selected document bodies', () => {
  const document = Document.create();
  try {
    const commands = 'M 0 0 L 10 0 L 10 10 C';
    document.addPathPage(commands, [2, 3, 10, 10], 40, 30, true, false);
    assert.equal(document.pagePaths(0).length, 1);
    assert.equal(document.pagePaths(0)[0].abbreviated_data, commands);
    assert.deepEqual(document.pagePaths(0)[0].boundary, [2, 3, 10, 10]);
    assert.equal(document.pagePaths(0)[0].fill, true);
    assert.equal(document.pagePaths(0)[0].stroke, false);
    assert.equal(document.pages()[0].physical_box[2], 40);
    assert.match(document.pageSvg(0), /<path /);
    const reopened = Document.fromBytes(document.toBytes());
    try {
      assert.equal(reopened.appendDocumentBody(), 1);
      reopened.appendPathPage(1, commands, [4, 5, 10, 10], 50, 35);
      assert.deepEqual(reopened.pages().map((page) => page.document_index), [0, 1]);
      assert.equal(reopened.pagePaths(1).length, 1);
      assert.equal(reopened.pagePaths(1)[0].fill, false);
      assert.equal(reopened.pagePaths(1)[0].stroke, true);
      assert.equal(reopened.pages()[1].physical_box[2], 50);
      const before = reopened.toBytes();
      assert.throws(() => reopened.appendPathPage(2, commands, [0, 0, 10, 10]), OfdError);
      assert.throws(() => reopened.appendPathPage(1, '', [0, 0, 10, 10]), OfdError);
      assert.throws(() => reopened.appendPathPage(1, 'M 0', [0, 0, 10, 10]), OfdError);
      assert.throws(() => reopened.appendPathPage(1, 'L 10 10', [0, 0, 10, 10]), OfdError);
      assert.deepEqual(reopened.toBytes(), before);
      assert.throws(() => reopened.appendPathPage(-1, commands, [0, 0, 10, 10]), RangeError);
      assert.throws(() => reopened.addPathPage(commands, [0, 0, 10, 10], 40, 30, 'false'), TypeError);
      assert.deepEqual(reopened.validateReferences(), []);
      const copy = Document.fromBytes(reopened.toBytes());
      try {
        assert.equal(copy.pagePaths(1).length, 1);
        assert.deepEqual(copy.validateReferences(), []);
        const pagePath = copy.pages()[1].path;
        const original = copy.entry(pagePath);
        copy.setEntry(pagePath, Buffer.from(original.toString('utf8').replace(commands, 'X 0 0')));
        assert.ok(copy.validateReferences().some((issue) =>
          issue.code === 'invalid_path_data' && issue.path === pagePath));
        copy.setEntry(pagePath, original);
        const clip = '<ofd:Clips><ofd:Clip><ofd:Area><ofd:Path Boundary="0 0 10 10"><ofd:AbbreviatedData>X 0 0</ofd:AbbreviatedData></ofd:Path></ofd:Area></ofd:Clip></ofd:Clips>';
        copy.setEntry(pagePath, Buffer.from(original.toString('utf8').replace(
          '</ofd:PathObject>', `${clip}</ofd:PathObject>`)));
        assert.ok(copy.validateReferences().some((issue) =>
          issue.code === 'invalid_path_data' && issue.message.includes('clip Path')));
        copy.setEntry(pagePath, original);
        assert.deepEqual(copy.validateReferences(), []);
      } finally {
        copy.close();
      }
    } finally {
      reopened.close();
    }
  } finally {
    document.close();
  }
});

test('search positioned text and inspect template sources and page objects', () => {
  const document = Document.create();
  try {
    document.addBlankPage(80, 50);
    document.addBlankPage(80, 50);
    const id = document.addTextObject(0, 'Searchable', [10, 20, 40, 8]);
    assert.deepEqual(document.allText(), [['Searchable'], []]);
    assert.deepEqual(document.pageTextInRegion(0, [0, 0, 80, 50]), ['Searchable']);
    assert.deepEqual(document.pageTextInRegion(0, [60, 40, 5, 5]), []);
    assert.equal(document.keywordPositions(['Search'], [0])[0].page_index, 0);
    assert.deepEqual(document.keywordPositions(['Search'], [1]), []);
    assert.equal(document.pageSources(0).length, 1);
    assert.equal(document.pageObjects(0)[0].id, id);
    assert.equal(document.pageLayers(0)[0].object_count, 1);
    const templateId = document.promotePageToTemplate(0, [1]);
    assert.deepEqual(document.pageText(0), []);
    assert.deepEqual(document.pageTextWithTemplates(0), ['Searchable']);
    assert.deepEqual(document.pageTextWithTemplates(1), ['Searchable']);
    assert.equal(document.pageSources(1)[0].template_id, templateId);
    assert.equal(document.pageObjects(1)[0].template_id, templateId);
    document.removePageTemplate(1, templateId);
    assert.deepEqual(document.pageTextWithTemplates(1), []);
    document.applyPageTemplate(1, templateId);
    assert.deepEqual(document.pageTextWithTemplates(1), ['Searchable']);
    assert.deepEqual(document.validateReferences(), []);
    const copy = Document.fromBytes(document.toBytes());
    try {
      assert.deepEqual(copy.pageTextWithTemplates(1), ['Searchable']);
      assert.deepEqual(copy.validateReferences(), []);
    } finally {
      copy.close();
    }
    const templatePath = document.pageSources(0)[0].path;
    assert.throws(() => document.removeDocumentTemplate(0, templateId), OfdError);
    document.removePageTemplate(0, templateId);
    document.removePageTemplate(1, templateId);
    document.removeDocumentTemplate(0, templateId);
    assert.deepEqual(document.pageTextWithTemplates(0), []);
    assert.deepEqual(document.pageTextWithTemplates(1), []);
    assert.ok(document.entry(templatePath).length > 0);
    assert.throws(() => document.applyPageTemplate(1, templateId), OfdError);
    assert.deepEqual(document.validateReferences(), []);
    assert.throws(() => document.keywordPositions('Search'), TypeError);
    assert.throws(() => document.keywordPositions(['Search'], [-1]), RangeError);
    assert.throws(() => document.pageTextInRegion(0, [0, 0, 0, 5]), RangeError);
  } finally {
    document.close();
  }
});

test('duplicate template IDs fail page geometry and report reference ambiguity', () => {
  const document = Document.create();
  try {
    document.addTextPage('template', 40, 30);
    document.promotePageToTemplate(0, []);
    const rootPath = 'Doc_0/Document.xml';
    const original = document.entry(rootPath).toString('utf8');
    const definition = original.match(/<ofd:TemplatePage\b[^>]*\/>/)?.[0];
    assert.ok(definition);
    document.setEntry(rootPath, Buffer.from(original.replace(definition,
      `${definition}${definition}`), 'utf8'));
    assert.throws(() => document.pages(), OfdError);
    assert.throws(() => document.pageSources(0), OfdError);
    assert.ok(document.validateReferences().some((issue) =>
      issue.code === 'template_reference' && issue.message.includes('ambiguous')));
    const reopened = Document.fromBytes(document.toBytes());
    try { assert.throws(() => reopened.pages(), OfdError); }
    finally { reopened.close(); }
  } finally { document.close(); }
});

test('duplicate page IDs cannot assign one annotation to two pages', () => {
  const document = Document.create();
  try {
    document.addBlankPage(40, 30);
    document.addBlankPage(40, 30);
    const id = document.addTextNoteAnnotation(0, 'first', [2, 2, 8, 8]);
    const pages = document.pages();
    const rootPath = 'Doc_0/Document.xml';
    const source = document.entry(rootPath).toString('utf8');
    const originalId = `<ofd:Page ID="${pages[1].id}"`;
    const changed = source.replace(originalId, `<ofd:Page ID="0${pages[0].id}"`);
    assert.notEqual(changed, source);
    document.setEntry(rootPath, Buffer.from(changed));
    const before = document.toBytes();
    assert.ok(document.validateReferences().some((issue) => issue.code === 'duplicate_page_id'));
    for (const page of [0, 1]) {
      assert.throws(() => document.annotationObjects(page, id), OfdError);
      assert.throws(() => document.pageSvg(page), OfdError);
      assert.throws(() => document.addUriLinkAnnotation(page, 'https://example.com', [2, 2, 8, 8]), OfdError);
    }
    assert.deepEqual(document.toBytes(), before);
  } finally { document.close(); }
});

test('navigation resolves numeric page ID aliases through Node.js and PDF export', () => {
  const document = Document.create();
  try {
    document.addTextPage('source', 40, 30);
    document.addBlankPage(40, 30);
    const destination = {page_id: `0${document.pages()[1].id}`, kind: 'Fit'};
    const objectId = document.pageTextObjects(0)[0].id;
    document.addBookmark(0, 'alias', destination);
    document.addOutline(0, 'alias', {destination});
    document.addPageObjectGotoAction(0, objectId, destination);
    document.addGotoLinkAnnotation(0, destination, [2, 2, 8, 8]);
    assert.deepEqual(document.validateReferences(), []);
    assert.equal(document.toPdf().subarray(0, 5).toString(), '%PDF-');
  } finally { document.close(); }
});

test('Body templates resolve numeric ID aliases across Node.js serialization and edits', () => {
  const document = Document.create();
  try {
    document.addTextPage('aliased template', 30, 20);
    document.addBlankPage(30, 20);
    const templateId = document.promotePageToTemplate(0, [], 'Body');
    const pagePath = document.pages()[0].path;
    const pageXml = document.entry(pagePath).toString('utf8');
    assert.match(pageXml, /<ofd:Area>.*<\/ofd:Area>/);
    const aliased = pageXml.replace(/<ofd:Area>.*?<\/ofd:Area>/, '')
      .replace(`TemplateID="${templateId}"`, `TemplateID="0${templateId}"`);
    document.setEntry(pagePath, Buffer.from(aliased));

    assert.deepEqual(document.pages()[0].physical_box, [0, 0, 30, 20]);
    assert.equal(document.pageSources(0)[0].template_id, `0${templateId}`);
    assert.deepEqual(document.pageTextWithTemplates(0), ['aliased template']);
    document.applyPageTemplate(1, `0${templateId}`, 'Body');
    assert.deepEqual(document.pageTextWithTemplates(1), ['aliased template']);
    assert.deepEqual(document.validateReferences(), []);

    const reopened = Document.fromBytes(document.toBytes());
    try {
      assert.deepEqual(reopened.pages()[0].physical_box, [0, 0, 30, 20]);
      assert.deepEqual(reopened.pageTextWithTemplates(1), ['aliased template']);
      reopened.removePageTemplate(0, templateId);
      reopened.removePageTemplate(1, templateId);
      reopened.removeDocumentTemplate(0, templateId);
      assert.deepEqual(reopened.validateReferences(), []);
    } finally { reopened.close(); }
  } finally { document.close(); }
});

test('foreground template text follows page text through Node.js', () => {
  const document = Document.create();
  try {
    document.addTextPage('foreground', 80, 50);
    document.addTextPage('page', 80, 50);
    document.promotePageToTemplate(0, [1], 'Foreground');
    assert.deepEqual(document.pageTextWithTemplates(1), ['page', 'foreground']);
    assert.deepEqual(document.pageTextInRegion(1, [-100, -100, 300, 300]),
      ['page', 'foreground']);
    const reopened = Document.fromBytes(document.toBytes());
    try {
      assert.deepEqual(reopened.pageTextWithTemplates(1), ['page', 'foreground']);
    } finally { reopened.close(); }
  } finally { document.close(); }
});

test('keyword search does not join separate text objects', () => {
  const document = Document.create();
  try {
    document.addBlankPage(80, 50);
    document.addTextObject(0, 'A', [10, 20, 10, 8]);
    document.addTextObject(0, 'B', [20, 20, 10, 8]);
    assert.deepEqual(document.pageText(0), ['A', 'B']);
    assert.deepEqual(document.keywordPositions(['AB']), []);
    assert.equal(document.keywordPositions(['A', 'B']).length, 2);
    const reopened = Document.fromBytes(document.toBytes());
    try {
      assert.deepEqual(reopened.keywordPositions(['AB']), []);
    } finally {
      reopened.close();
    }
  } finally {
    document.close();
  }
});

test('manage multiple document bodies and import selected bodies', () => {
  const document = Document.create();
  try {
    document.addTextPage('first body', 30, 20);
    assert.equal(document.appendDocumentBody(), 1);
    document.appendTextPage(1, 'second body', 30, 20);
    document.appendBlankPage(1, 30, 20);
    assert.deepEqual(document.allText(), [['first body'], ['second body'], []]);
    assert.deepEqual(document.pages().map((page) => page.document_index), [0, 1, 1]);
    assert.equal(document.documents().length, 2);
    document.moveDocumentBody(1, 0);
    assert.deepEqual(document.allText(), [['second body'], [], ['first body']]);
    assert.deepEqual(document.pages().map((page) => page.document_index), [0, 0, 1]);

    const extracted = document.extractDocumentBody(0);
    try {
      assert.deepEqual(extracted.allText(), [['second body'], []]);
      assert.deepEqual(extracted.validateReferences(), []);
      const target = Document.create();
      try {
        target.addTextPage('target', 30, 20);
        assert.equal(target.appendDocumentBodyFrom(extracted, 0), 1);
        assert.deepEqual(target.allText(), [['target'], ['second body'], []]);
        assert.deepEqual(target.validateReferences(), []);
        const extra = target.appendDocumentBodiesFrom(extracted);
        assert.deepEqual(extra, [2]);
        assert.deepEqual(target.allText(), [['target'], ['second body'], [], ['second body'], []]);
        assert.deepEqual(target.validateReferences(), []);
        const reopened = Document.fromBytes(target.toBytes());
        try {
          assert.deepEqual(reopened.allText(), target.allText());
          assert.deepEqual(reopened.validateReferences(), []);
        } finally {
          reopened.close();
        }
        assert.throws(() => target.appendDocumentBodyFrom(target, 99), OfdError);
      } finally {
        target.close();
      }
    } finally {
      extracted.close();
    }
    document.removeDocumentBody(0);
    assert.deepEqual(document.allText(), [['first body']]);
    assert.throws(() => document.removeDocumentBody(0), OfdError);
    assert.deepEqual(document.validateReferences(), []);
    assert.throws(() => document.appendDocumentBodiesFrom({}), TypeError);
  } finally {
    document.close();
  }
});

test('insert, reorder, duplicate, and remove pages within document bodies', () => {
  const document = Document.create();
  try {
    document.addTextPage('A', 30, 20);
    document.addTextPage('B', 30, 20);
    assert.equal(document.appendDocumentBody(), 1);
    document.appendTextPage(1, 'C', 30, 20);
    assert.equal(document.insertTextPage(0, 1, 'X\0Y', 30, 20), 1);
    assert.equal(document.insertBlankPage(1, 0, 30, 20), 3);
    assert.deepEqual(document.allText(), [['A'], ['X\0Y'], ['B'], [], ['C']]);
    document.movePage(0, 2);
    assert.deepEqual(document.allText(), [['X\0Y'], ['B'], ['A'], [], ['C']]);
    assert.equal(document.duplicatePageAt(1, 0), 0);
    assert.deepEqual(document.allText(), [['B'], ['X\0Y'], ['B'], ['A'], [], ['C']]);
    document.removePage(2);
    assert.deepEqual(document.allText(), [['B'], ['X\0Y'], ['A'], [], ['C']]);
    const beforeReorder = document.toBytes();
    assert.throws(() => document.reorderDocumentPages(0, [0, 0, 2]), OfdError);
    assert.deepEqual(document.toBytes(), beforeReorder);
    document.reorderDocumentPages(0, [2, 0, 1]);
    assert.deepEqual(document.allText(), [['A'], ['B'], ['X\0Y'], [], ['C']]);
    assert.deepEqual(document.pages().map((page) => page.document_index), [0, 0, 0, 1, 1]);
    assert.deepEqual(document.validateReferences(), []);
    const copy = Document.fromBytes(document.toBytes());
    try {
      assert.deepEqual(copy.allText(), document.allText());
      assert.deepEqual(copy.validateReferences(), []);
    } finally {
      copy.close();
    }
    assert.throws(() => document.movePage(0, 4), OfdError);
    assert.throws(() => document.removePage(99), OfdError);
  } finally {
    document.close();
  }
});

test('create and reuse a composite text resource', () => {
  const document = Document.create();
  try {
    document.addTextPage('base', 40, 30);
    document.addBlankPage(40, 30);
    const fontId = document.fonts()[0].id;
    const before = document.toBytes();
    assert.throws(() => document.addCompositeTextObject(0, '', [2, 2, 20, 8], 5, fontId), OfdError);
    assert.throws(() => document.addCompositeTextObject(0, 'bad', [2, 2, 20, 8], 5, 'missing'), OfdError);
    assert.deepEqual(document.toBytes(), before);
    const id = document.addCompositeTextObject(0, '组合 A', [2, 2, 20, 8], 5, fontId);
    const resourceId = document.pageCompositeObjects(0)[0].resource_id;
    assert.equal(document.pageCompositeObjects(0)[0].id, id);
    assert.deepEqual(document.pageText(0), ['base', '组合 A']);
    const reused = document.addCompositeObject(1, resourceId, [2, 2, 20, 8]);
    assert.equal(document.pageCompositeObjects(1)[0].id, reused);
    assert.deepEqual(document.pageText(1), ['组合 A']);
    assert.match(document.pageSvg(1), /组合 A/);
    const copy = Document.fromBytes(document.toBytes());
    try {
      assert.deepEqual(copy.pageText(1), ['组合 A']);
      assert.deepEqual(copy.validateReferences(), []);
    } finally {
      copy.close();
    }
  } finally {
    document.close();
  }
});

test('create and reuse composite path and image resources', () => {
  const document = Document.create();
  try {
    document.addBlankPage(40, 30);
    document.addBlankPage(40, 30);
    const png = document.pagePng(0, 25.4);
    const pathId = document.addCompositePathObject(0, 'M 0 0 L 10 0 L 10 10 C', [2, 2, 10, 10]);
    const imageId = document.addCompositeImageObject(0, png, 'png', [14, 2, 10, 10]);
    const objects = document.pageCompositeObjects(0);
    assert.deepEqual(objects.map((object) => object.id), [pathId, imageId]);
    assert.equal(document.pageCompositeUnits(0).length, 2);
    assert.equal(document.pageCompositeImages(0).length, 1);
    assert.deepEqual(document.pageCompositeImageData(0, 0), png);
    const reused = document.addCompositeObject(1, objects[0].resource_id, [1, 1, 10, 10]);
    assert.equal(document.pageCompositeObjects(1)[0].id, reused);
    assert.match(document.pageSvg(1), /<path /);
    assert.deepEqual(document.validateReferences(), []);
    const copy = Document.fromBytes(document.toBytes());
    try {
      assert.deepEqual(copy.pageCompositeImageData(0, 0), png);
      assert.equal(copy.pageCompositeObjects(1)[0].resource_id, objects[0].resource_id);
      assert.deepEqual(copy.validateReferences(), []);
    } finally {
      copy.close();
    }
  } finally {
    document.close();
  }
});

test('render and export with a composite image substitution', () => {
  const document = Document.create();
  try {
    document.addBlankPage(40, 30);
    const commands = 'M 0 0 L 20 0 L 20 20 L 0 20 C';
    const redPath = document.addPathObject(0, commands, [0, 0, 20, 20],
      { fill: true, stroke: false, fill_color: [255, 0, 0] });
    const redPng = document.pagePng(0, 25.4);
    document.removePageObject(0, redPath);
    const greenPath = document.addPathObject(0, commands, [0, 0, 20, 20],
      { fill: true, stroke: false, fill_color: [0, 255, 0] });
    const greenPng = document.pagePng(0, 25.4);
    document.removePageObject(0, greenPath);
    assert.notDeepEqual(redPng, greenPng);

    document.addCompositeImageObject(0, redPng, 'png', [1, 1, 20, 20]);
    const unitId = document.pageCompositeObjects(0)[0].resource_id;
    const greenImageId = document.addImageObject(0, greenPng, 'png', [1, 1, 20, 20]);
    const mediaId = document.pageImages(0)[0].resource_id;
    document.removePageObject(0, greenImageId);
    document.setPageCompositeUnitImages(0, unitId,
      { thumbnailId: mediaId, substitutionId: mediaId });
    assert.equal(document.pageCompositeUnits(0).find((unit) => unit.id === unitId).substitution, mediaId);
    const defaultSvg = document.pageSvg(0);
    const substitutedSvg = document.pageSvg(0, { preferImageSubstitution: true });
    assert.ok(defaultSvg.includes(redPng.toString('base64')));
    assert.ok(substitutedSvg.includes(greenPng.toString('base64')));
    assert.notDeepEqual(document.pagePng(0, 25.4),
      document.pagePng(0, 25.4, { preferImageSubstitution: true }));
    assert.notDeepEqual(document.toPdf(), document.toPdf({ preferImageSubstitution: true }));
    assert.deepEqual(document.validateReferences(), []);
    const copy = Document.fromBytes(document.toBytes());
    try {
      assert.ok(copy.pageSvg(0, { preferImageSubstitution: true }).includes(greenPng.toString('base64')));
      assert.deepEqual(copy.validateReferences(), []);
    } finally {
      copy.close();
    }
    const unitPath = `Doc_0/CompositeRes_${unitId}.xml`;
    const resourceXml = document.entry(unitPath).toString();
    assert.match(resourceXml, /<ofd:Content>/);
    document.setEntry(unitPath, Buffer.from(resourceXml.replace(
      /<ofd:Content>[\s\S]*?<\/ofd:Content>/, '<ofd:Content/>')));
    const thumbnailSvg = document.pageSvg(0);
    assert.ok(thumbnailSvg.includes(greenPng.toString('base64')));
    assert.ok(!thumbnailSvg.includes(redPng.toString('base64')));
    assert.ok(document.pagePng(0, 25.4).subarray(0, 8).equals(Buffer.from([137, 80, 78, 71, 13, 10, 26, 10])));
    const before = document.toBytes();
    assert.throws(() => document.setPageCompositeUnitImages(0, unitId, { substitutionId: '999999' }), OfdError);
    assert.deepEqual(document.toBytes(), before);
    assert.throws(() => document.pageSvg(0, { preferImageSubstitution: 1 }), TypeError);
    assert.throws(() => document.pagePng(0, 96, null), TypeError);
  } finally {
    document.close();
  }
});

test('report duplicate resource IDs across separate resource files', () => {
  const document = Document.create();
  try {
    document.addBlankPage(40, 30);
    document.addCompositePathObject(0, 'M 0 0 L 10 0 L 10 10 C', [1, 1, 10, 10]);
    document.addCompositePathObject(0, 'M 0 0 L 10 0 L 10 10 C', [12, 1, 10, 10]);
    assert.deepEqual(document.validateReferences(), []);
    const objects = document.pageCompositeObjects(0);
    const first = objects[0].resource_id;
    const second = objects[1].resource_id;
    const path = `Doc_0/CompositeRes_${second}.xml`;
    const xml = document.entry(path).toString('utf8');
    document.setEntry(path, Buffer.from(xml.replace(
      `<ofd:CompositeGraphicUnit ID="${second}"`,
      `<ofd:CompositeGraphicUnit ID="0${first}"`,
    )));
    assert.ok(document.validateReferences().some((issue) =>
      issue.path === path && issue.code === 'duplicate_resource_id'));
  } finally {
    document.close();
  }
});

test('create an editable text note annotation', () => {
  const document = Document.create();
  try {
    document.addBlankPage(30, 20);
    const id = document.addTextNoteAnnotation(0, '审核 & <通过>', [2, 3, 5, 5], {
      color: [0, 100, 200], opacity: 128, icon: 'Comment', open: true,
    });
    const note = document.annotations()[0];
    assert.equal(note.id, id);
    assert.equal(note.type, 'Path');
    assert.equal(note.subtype, 'Text');
    assert.equal(note.remark, '审核 & <通过>');
    assert.equal(note.parameters.PDFTextIcon, 'Comment');
    assert.equal(note.parameters.PDFTextOpen, 'true');
    assert.equal(document.toPdf().subarray(0, 5).toString(), '%PDF-');
    const before = document.toBytes();
    assert.throws(() => document.addTextNoteAnnotation(0, 'bad', [2, 3, 5, 5], { icon: 'Unknown' }), OfdError);
    assert.deepEqual(document.toBytes(), before);
  } finally {
    document.close();
  }
});

test('create editable Square and Circle annotations', () => {
  const document = Document.create();
  try {
    document.addBlankPage(30, 20);
    const square = document.addShapeAnnotation(0, 'Square', [2, 3, 10, 5], {
      strokeColor: [0, 0, 255], fillColor: [255, 0, 0], lineWidth: 0.5,
      dashSequence: [1, 2], opacity: 128, insets: [0.1, 0.1, 0.1, 0.1],
    });
    const circle = document.addShapeAnnotation(0, 'Circle', [15, 3, 8, 8], { strokeColor: null, fillColor: [0, 255, 0] });
    const shapes = document.annotations();
    assert.equal(shapes[0].id, square);
    assert.equal(shapes[0].type, 'Path');
    assert.equal(shapes[0].subtype, 'Square');
    assert.equal(shapes[0].parameters.PDFShapeDash, '1 2');
    assert.equal(shapes[1].id, circle);
    assert.equal(shapes[1].type, 'Path');
    assert.equal(shapes[1].subtype, 'Circle');
    assert.equal(shapes[1].parameters.PDFShapeStroke, undefined);
    const reopened = Document.fromBytes(document.toBytes());
    try { assert.equal(reopened.annotations().length, 2); } finally { reopened.close(); }
    assert.equal(document.toPdf().subarray(0, 5).toString(), '%PDF-');
    const before = document.toBytes();
    assert.throws(() => document.addShapeAnnotation(0, 'Triangle', [2, 3, 10, 5]), OfdError);
    assert.deepEqual(document.toBytes(), before);
  } finally { document.close(); }
});

test('create editable Line annotation', () => {
  const document = Document.create();
  try {
    document.addBlankPage(30, 20);
    const id = document.addLineAnnotation(0, [2, 3, 10, 5], [0, 5, 10, 0], {
      strokeColor: [255, 0, 0], lineWidth: 0.5, dashSequence: [1, 2], opacity: 128,
    });
    const line = document.annotations()[0];
    assert.equal(line.id, id);
    assert.equal(line.subtype, 'Line');
    assert.equal(line.parameters.PDFLine, '0 1 1 0');
    assert.equal(document.toPdf().subarray(0, 5).toString(), '%PDF-');
    const before = document.toBytes();
    assert.throws(() => document.addLineAnnotation(0, [2, 3, 10, 5], [0, 5, 11, 0]), OfdError);
    assert.deepEqual(document.toBytes(), before);
  } finally { document.close(); }
});

test('create one annotation with multiple styled paths', () => {
  const document = Document.create();
  try {
    document.addBlankPage(40, 30);
    const paths = [
      {commands: 'M 0 0 L 10 10', boundary: [0, 0, 10, 10], stroke_color: [255, 0, 0]},
      {commands: 'M 0 10 L 10 0', boundary: [10, 0, 10, 10], stroke_color: [0, 0, 255], dash_sequence: [2, 1]},
    ];
    const id = document.addMultiPathAnnotation(0, [3, 4, 25, 15], paths);
    assert.equal(document.annotations().length, 1);
    assert.equal(document.annotations()[0].id, id);
    assert.match(document.pageSvg(0), /#ff0000/);
    assert.match(document.pageSvg(0), /#0000ff/);
    const reopened = Document.fromBytes(document.toBytes());
    try { assert.equal(reopened.annotations().length, 1); } finally { reopened.close(); }
    const before = document.toBytes();
    assert.throws(() => document.addMultiPathAnnotation(0, [3, 4, 25, 15],
      [...paths, {commands: 'bad', boundary: [0, 0, 10, 10]}]), OfdError);
    assert.deepEqual(document.toBytes(), before);
  } finally { document.close(); }
});

test('create one annotation with mixed path, text, and image graphics', () => {
  const document = Document.create();
  try {
    document.addBlankPage(40, 30);
    const imageId = document.addMultimediaResource(0, 'Image', 'PNG', document.pagePng(0, 25.4));
    const objects = [
      {kind: 'path', commands: 'M 0 0 L 9 9', boundary: [0, 0, 10, 10], stroke_color: [255, 0, 0]},
      {kind: 'text', text: 'Mixed', boundary: [10, 0, 12, 10], size: 4, color: [0, 0, 255]},
      {kind: 'image', resource_id: imageId, boundary: [22, 0, 8, 8], opacity: 200},
    ];
    const id = document.addMixedAnnotation(0, [2, 3, 30, 12], objects);
    assert.equal(document.annotations()[0].id, id);
    const appearance = document.annotationObjects(0, id);
    assert.deepEqual(appearance.map(item => item.kind), ['PathObject', 'TextObject', 'ImageObject']);
    assert.equal(appearance[1].text[0], 'Mixed');
    assert.equal(appearance[2].resource_id, imageId);
    const svg = document.pageSvg(0);
    assert.match(svg, /#ff0000/);
    assert.match(svg, /Mixed/);
    const stampId = document.addMixedStampAnnotation(0, [2, 15, 30, 12], objects);
    assert.equal(document.annotations()[1].id, stampId);
    assert.equal(document.annotations()[1].type, 'Stamp');
    assert.equal(document.annotationObjects(0, stampId).length, 3);
    const appended = document.appendAnnotationObjects(0, id, [
      {kind: 'path', commands: 'M 0 0 L 4 4', boundary: [1, 1, 5, 5], stroke_color: [0, 128, 0]},
      {kind: 'text', text: 'More', boundary: [6, 1, 10, 5], size: 3},
    ]);
    assert.equal(appended.length, 2);
    assert.deepEqual(document.annotationObjects(0, id).slice(-2).map(item => item.id), appended);
    assert.equal(document.annotations().length, 2);
    const reopened = Document.fromBytes(document.toBytes());
    try {
      assert.deepEqual(reopened.annotationObjects(0, id).slice(-2).map(item => item.id), appended);
    } finally { reopened.close(); }
    const beforeAppendError = document.toBytes();
    assert.throws(() => document.appendAnnotationObjects(0, id, [
      {kind: 'image', resource_id: '999999', boundary: [0, 0, 3, 3]},
    ]), OfdError);
    assert.deepEqual(document.toBytes(), beforeAppendError);
    assert.throws(() => document.appendAnnotationObjects(0, id, {}), TypeError);
    assert.throws(() => document.annotationObjects(0, 'missing'), OfdError);
    const before = document.toBytes();
    assert.throws(() => document.addMixedAnnotation(0, [2, 3, 30, 12],
      [...objects, {kind: 'image', resource_id: '999999', boundary: [22, 0, 8, 8]}]), OfdError);
    assert.deepEqual(document.toBytes(), before);
    document.updateAnnotationObject(0, id, appearance[0].id, {
      boundary: [1, 1, 9, 9], ctm: [1, 0, 0, 1, 2, 3], visible: false, alpha: 128,
    });
    let edited = document.annotationObjects(0, id);
    assert.deepEqual(edited[0].boundary, [1, 1, 9, 9]);
    assert.deepEqual(edited[0].ctm, [1, 0, 0, 1, 2, 3]);
    assert.equal(edited[0].visible, false);
    assert.equal(edited[0].alpha, 128);
    document.updateAnnotationObject(0, id, appearance[0].id, {ctm: null});
    assert.equal(document.annotationObjects(0, id)[0].ctm, null);
    const beforeInvalid = document.toBytes();
    assert.throws(() => document.updateAnnotationObject(0, id, appearance[0].id,
      {boundary: [0, 0, 0, 9]}), OfdError);
    assert.deepEqual(document.toBytes(), beforeInvalid);
    document.removeAnnotationObject(0, id, appearance[2].id);
    edited = document.annotationObjects(0, id);
    assert.equal(edited.length, 4);
    assert.equal(document.annotationObjects(0, stampId).length, 3);
  } finally { document.close(); }
});

test('create and append composite graphics in an annotation', () => {
  const document = Document.create();
  try {
    document.addBlankPage(40, 30);
    document.addCompositePathObject(0, 'M 0 0 L 8 0 L 8 8 C', [0, 0, 8, 8],
      {fill: true, stroke: false, fill_color: [255, 0, 0]});
    const resourceId = document.pageCompositeObjects(0)[0].resource_id;
    const graphic = {kind: 'composite', resource_id: resourceId,
      boundary: [0, 0, 8, 8], ctm: [1, 0, 0, 1, 1, 2], alpha: 180};
    const annotationId = document.addMixedStampAnnotation(0, [2, 2, 20, 12], [graphic]);
    const appended = document.appendAnnotationObjects(0, annotationId, [graphic]);
    const appearance = document.annotationObjects(0, annotationId);
    assert.equal(appearance.length, 2);
    assert.deepEqual(appearance.map(item => item.kind), ['CompositeObject', 'CompositeObject']);
    assert.equal(appearance[0].resource_id, resourceId);
    assert.equal(appearance[0].alpha, 180);
    assert.equal(appearance[1].id, appended[0]);
    assert.match(document.pageSvg(0), /#ff0000/);
    assert.deepEqual(document.validateReferences(), []);
    const before = document.toBytes();
    assert.throws(() => document.appendAnnotationObjects(0, annotationId,
      [{kind: 'composite', resource_id: '999999', boundary: [0, 0, 8, 8]}]), OfdError);
    assert.deepEqual(document.toBytes(), before);
  } finally { document.close(); }
});

test('edit annotation path commands and preserve failed edits', () => {
  const document = Document.create();
  try {
    document.addBlankPage(40, 30);
    const id = document.addMixedAnnotation(0, [2, 2, 20, 12], [
      {kind: 'path', commands: 'M 0 0 L 8 8', boundary: [0, 0, 8, 8]},
      {kind: 'text', text: 'keep', boundary: [8, 0, 8, 8], size: 3},
    ]);
    const objects = document.annotationObjects(0, id);
    document.setAnnotationPathData(0, id, objects[0].id, 'M 0 8 L 8 0');
    document.setAnnotationPathStrokeStyle(0, id, objects[0].id,
      {line_width: 1.25, cap: 'round', dash_sequence: [1, 2, 3], dash_offset: 0.5});
    const before = document.toBytes();
    const reopened = Document.fromBytes(before);
    try {
      const edited = reopened.annotationObjects(0, id);
      assert.equal(edited[0].commands, 'M 0 8 L 8 0');
      assert.deepEqual(edited[1], objects[1]);
      assert.match(reopened.pageSvg(0), /<path /);
    } finally { reopened.close(); }
    assert.throws(() => document.setAnnotationPathData(0, id, objects[0].id, 'invalid'), OfdError);
    assert.throws(() => document.setAnnotationPathData(0, id, objects[1].id, 'M 0 0 L 8 8'), OfdError);
    assert.throws(() => document.setAnnotationPathStrokeStyle(0, id, objects[0].id, {line_width: -1}), OfdError);
    assert.throws(() => document.setAnnotationPathStrokeStyle(0, id, objects[1].id, {line_width: 1}), OfdError);
    assert.deepEqual(document.toBytes(), before);
  } finally { document.close(); }
});

test('edit annotation text runs and remeasure advances', () => {
  const document = Document.create();
  try {
    document.addBlankPage(40, 30);
    const id = document.addMixedAnnotation(0, [2, 2, 25, 12], [
      {kind: 'text', text: 'AB', boundary: [0, 0, 15, 8], size: 3},
      {kind: 'path', commands: 'M 0 0 L 8 8', boundary: [15, 0, 8, 8]},
    ]);
    const objects = document.annotationObjects(0, id);
    document.setAnnotationTextCode(0, id, objects[0].id, 0, 'A & <B>');
    const before = document.toBytes();
    const reopened = Document.fromBytes(before);
    try {
      const edited = reopened.annotationObjects(0, id);
      assert.deepEqual(edited[0].text, ['A & <B>']);
      assert.deepEqual(edited[1], objects[1]);
      assert.match(reopened.pageSvg(0), /A &amp; &lt;B&gt;/);
    } finally { reopened.close(); }
    assert.throws(() => document.setAnnotationTextCode(0, id, objects[1].id, 0, 'wrong'), OfdError);
    assert.throws(() => document.setAnnotationTextCode(0, id, objects[0].id, 1, 'wrong'), OfdError);
    assert.throws(() => document.setAnnotationTextCode(0, id, objects[0].id, -1, 'wrong'), RangeError);
    assert.deepEqual(document.toBytes(), before);
  } finally { document.close(); }
});

test('edit annotation text font through native binding', () => {
  const document = Document.create();
  try {
    document.addBlankPage(40, 30);
    const fontId = document.addEmbeddedFont('DejaVu Serif', fs.readFileSync(fixture('dejavu-serif.ttf')));
    const annotationId = document.addMixedAnnotation(0, [2, 2, 20, 10], [
      {kind: 'text', text: 'AB', boundary: [0, 0, 15, 8], size: 3},
    ]);
    const objectId = document.annotationObjects(0, annotationId)[0].id;
    document.setAnnotationTextFont(0, annotationId, objectId, fontId);
    assert.equal(document.annotationObjects(0, annotationId)[0].font_id, fontId);
    const before = document.toBytes();
    assert.throws(() => document.setAnnotationTextFont(0, annotationId, objectId, '999999'), OfdError);
    assert.deepEqual(document.toBytes(), before);
  } finally { document.close(); }
});

test('remove annotation text run while preserving inherited position', () => {
  const document = Document.create();
  try {
    document.addBlankPage(40, 30);
    const id = document.addMixedAnnotation(0, [2, 2, 20, 10], [
      {kind: 'text', text: 'AB', boundary: [0, 0, 15, 8], size: 3},
    ]);
    const objectId = document.annotationObjects(0, id)[0].id;
    const path = document.annotations()[0].source_path;
    const source = document.entry(path).toString('utf8');
    const modified = source.replace('</ofd:TextObject>',
      '<ofd:TextCode>CD</ofd:TextCode></ofd:TextObject>');
    assert.notEqual(modified, source);
    document.setEntry(path, Buffer.from(modified));
    document.removeAnnotationTextCode(0, id, objectId, 0);
    assert.deepEqual(document.annotationObjects(0, id)[0].text, ['CD']);
    assert.match(document.entry(path).toString('utf8'), /<ofd:TextCode X="0" Y="3">CD<\/ofd:TextCode>/);
    const before = document.toBytes();
    assert.throws(() => document.removeAnnotationTextCode(0, id, objectId, 0), OfdError);
    assert.deepEqual(document.toBytes(), before);
  } finally { document.close(); }
});

test('append positioned annotation text run with atomic validation', () => {
  const document = Document.create();
  try {
    document.addBlankPage(40, 30);
    const id = document.addMixedAnnotation(0, [2, 2, 20, 10], [
      {kind: 'text', text: 'AB', boundary: [0, 0, 15, 8], size: 3},
    ]);
    const objectId = document.annotationObjects(0, id)[0].id;
    document.appendAnnotationTextCode(0, id, objectId, 'C&D', {delta_x: [2, 2.5]});
    assert.deepEqual(document.annotationObjects(0, id)[0].text, ['AB', 'C&D']);
    const path = document.annotations()[0].source_path;
    assert.match(document.entry(path).toString('utf8'),
      /<ofd:TextCode DeltaX="2 2.5">C&amp;D<\/ofd:TextCode>/);
    const before = document.toBytes();
    assert.throws(() => document.appendAnnotationTextCode(0, id, objectId, 'XY',
      {delta_x: [1, 2]}), OfdError);
    assert.deepEqual(document.toBytes(), before);
  } finally { document.close(); }
});

test('annotation text edits preserve embedded NUL', () => {
  const document = Document.create();
  try {
    document.addBlankPage(40, 30);
    const id = document.addMixedAnnotation(0, [2, 2, 20, 10], [
      {kind: 'text', text: 'AB', boundary: [0, 0, 15, 8], size: 3},
    ]);
    const objectId = document.annotationObjects(0, id)[0].id;
    document.setAnnotationTextCode(0, id, objectId, 0, 'A\0B');
    document.appendAnnotationTextCode(0, id, objectId, 'C\0D');
    assert.deepEqual(document.annotationObjects(0, id)[0].text, ['A\0B', 'C\0D']);
    const path = document.annotations()[0].source_path;
    assert.match(document.entry(path).toString('utf8'), /A\\0000B/);
    const reopened = Document.fromBytes(document.toBytes());
    try {
      assert.deepEqual(reopened.annotationObjects(0, id)[0].text, ['A\0B', 'C\0D']);
    } finally { reopened.close(); }
  } finally { document.close(); }
});

test('edit annotation text layout and restore inherited coordinates', () => {
  const document = Document.create();
  try {
    document.addBlankPage(40, 30);
    const id = document.addMixedAnnotation(0, [2, 2, 20, 10], [
      {kind: 'text', text: 'AB', boundary: [0, 0, 15, 8], size: 3},
    ]);
    const objectId = document.annotationObjects(0, id)[0].id;
    document.appendAnnotationTextCode(0, id, objectId, 'CD', {x: 12, y: 6});
    document.setAnnotationTextCodeLayout(0, id, objectId, 1, {delta_x: [2.5]});
    const path = document.annotations()[0].source_path;
    assert.match(document.entry(path).toString('utf8'),
      /<ofd:TextCode DeltaX="2.5">CD<\/ofd:TextCode>/);
    assert.deepEqual(document.annotationObjects(0, id)[0].codes[1],
      {x: null, y: null, delta_x: '2.5', delta_y: null, text: 'CD'});
    assert.deepEqual(document.annotationObjects(0, id)[0].cg_transforms, []);
    const before = document.toBytes();
    assert.throws(() => document.setAnnotationTextCodeLayout(0, id, objectId, 0, {}), OfdError);
    assert.deepEqual(document.toBytes(), before);
  } finally { document.close(); }
});

test('edit annotation typography and reject invalid weight atomically', () => {
  const document = Document.create();
  try {
    document.addBlankPage(40, 30);
    const id = document.addMixedAnnotation(0, [2, 2, 20, 10], [
      {kind: 'text', text: 'AB', boundary: [0, 0, 15, 8], size: 3},
    ]);
    const objectId = document.annotationObjects(0, id)[0].id;
    document.setAnnotationTextTypography(0, id, objectId,
      {size: 4, h_scale: 0.8, read_direction: 180, weight: 700, italic: true});
    const info = document.annotationObjects(0, id)[0];
    assert.equal(info.size, 4);
    assert.equal(info.h_scale, 0.8);
    assert.equal(info.read_direction, 180);
    assert.equal(info.weight, 700);
    assert.equal(info.italic, true);
    const path = document.annotations()[0].source_path;
    const xml = document.entry(path).toString('utf8');
    assert.match(xml, /HScale="0.8"/);
    assert.match(xml, /Weight="700"/);
    const before = document.toBytes();
    assert.throws(() => document.setAnnotationTextTypography(0, id, objectId,
      {size: 4, weight: 750}), OfdError);
    assert.deepEqual(document.toBytes(), before);
  } finally { document.close(); }
});

test('edit annotation text paint flags and restore defaults', () => {
  const document = Document.create();
  try {
    document.addBlankPage(40, 30);
    const id = document.addMixedAnnotation(0, [2, 2, 20, 10], [
      {kind: 'text', text: 'AB', boundary: [0, 0, 15, 8], size: 3},
      {kind: 'path', commands: 'M 0 0 L 8 8', boundary: [15, 0, 5, 8]},
    ]);
    const objects = document.annotationObjects(0, id);
    assert.equal(objects[0].fill, true);
    assert.equal(objects[0].stroke, false);
    assert.equal(objects[1].fill, false);
    assert.equal(objects[1].stroke, true);
    const path = document.annotations()[0].source_path;
    document.setAnnotationTextPaintFlags(0, id, objects[0].id, {fill: false, stroke: true});
    assert.match(document.entry(path).toString('utf8'), /Fill="false" Stroke="true"/);
    assert.equal(document.annotationObjects(0, id)[0].fill, false);
    assert.equal(document.annotationObjects(0, id)[0].stroke, true);
    const before = document.toBytes();
    const reopened = Document.fromBytes(before);
    try {
      assert.equal(reopened.annotationObjects(0, id)[0].fill, false);
      assert.equal(reopened.annotationObjects(0, id)[0].stroke, true);
    } finally { reopened.close(); }
    assert.throws(() => document.setAnnotationTextPaintFlags(0, id, objects[1].id), OfdError);
    assert.throws(() => document.setAnnotationTextPaintFlags(0, id, objects[0].id,
      {fill: 'no'}), OfdError);
    assert.deepEqual(document.toBytes(), before);
    document.setAnnotationTextPaintFlags(0, id, objects[0].id);
    const textTag = document.entry(path).toString('utf8').match(/<ofd:TextObject\b[^>]*>/)[0];
    assert.doesNotMatch(textTag, /Fill="false"|Stroke="true"/);
    assert.equal(document.annotationObjects(0, id)[0].fill, true);
    assert.equal(document.annotationObjects(0, id)[0].stroke, false);
  } finally { document.close(); }
});

test('edit annotation object name and blend mode', () => {
  const document = Document.create();
  try {
    document.addBlankPage(40, 30);
    const id = document.addMixedAnnotation(0, [2, 2, 20, 10], [
      {kind: 'text', text: 'AB', boundary: [0, 0, 15, 8], size: 3},
    ]);
    const objectId = document.annotationObjects(0, id)[0].id;
    document.setAnnotationObjectName(0, id, objectId, '标题 & "注释"');
    document.setAnnotationObjectBlendMode(0, id, objectId, 'multiply');
    const drawParam = document.addDocumentDrawParam(0, {line_width: 1.25});
    document.setAnnotationObjectDrawParam(0, id, objectId, drawParam);
    const info = document.annotationObjects(0, id)[0];
    assert.equal(info.name, '标题 & "注释"');
    assert.equal(info.blend_mode, 'Multiply');
    assert.equal(info.draw_param, drawParam);
    assert.match(document.pageSvg(0), /mix-blend-mode:multiply/);
    const before = document.toBytes();
    assert.throws(() => document.setAnnotationObjectBlendMode(0, id, objectId, 'unknown'), OfdError);
    assert.throws(() => document.setAnnotationObjectDrawParam(0, id, objectId, '999999'), OfdError);
    assert.deepEqual(document.toBytes(), before);
    const reopened = Document.fromBytes(before);
    try {
      assert.equal(reopened.annotationObjects(0, id)[0].name, '标题 & "注释"');
    } finally { reopened.close(); }
    document.setAnnotationObjectName(0, id, objectId);
    document.setAnnotationObjectBlendMode(0, id, objectId);
    document.setAnnotationObjectDrawParam(0, id, objectId);
    assert.equal(document.annotationObjects(0, id)[0].name, null);
    assert.equal(document.annotationObjects(0, id)[0].blend_mode, null);
    assert.equal(document.annotationObjects(0, id)[0].draw_param, null);
  } finally { document.close(); }
});

test('replace annotation image resource atomically', () => {
  const document = Document.create();
  try {
    document.addBlankPage(40, 30);
    const png = document.pagePng(0, 25.4);
    const first = document.addMultimediaResource(0, 'Image', 'PNG', png);
    const second = document.addMultimediaResource(0, 'Image', 'PNG', png);
    const id = document.addMixedAnnotation(0, [2, 2, 20, 12], [
      {kind: 'image', resource_id: first, boundary: [0, 0, 8, 8]},
      {kind: 'path', commands: 'M 0 0 L 8 8', boundary: [8, 0, 8, 8]},
    ]);
    const objects = document.annotationObjects(0, id);
    document.setAnnotationGraphicResource(0, id, objects[0].id, second);
    const before = document.toBytes();
    const reopened = Document.fromBytes(before);
    try {
      const edited = reopened.annotationObjects(0, id);
      assert.equal(edited[0].resource_id, second);
      assert.deepEqual(edited[0].boundary, objects[0].boundary);
      assert.deepEqual(edited[1], objects[1]);
      assert.deepEqual(reopened.validateReferences(), []);
    } finally { reopened.close(); }
    assert.throws(() => document.setAnnotationGraphicResource(0, id, objects[0].id, '999999'), OfdError);
    assert.throws(() => document.setAnnotationGraphicResource(0, id, objects[1].id, second), OfdError);
    assert.deepEqual(document.toBytes(), before);
  } finally { document.close(); }
});

test('create text and image watermarks and stamps through Node.js', () => {
  const document = Document.create();
  try {
    document.addBlankPage(40, 30);
    document.addBlankPage(40, 30);
    const textId = document.addTextWatermark(0, '机密', [2, 2, 20, 8], 5,
      {color: [180, 20, 20], opacity: 160, italic: true});
    const textStampId = document.addTextStampAnnotation(0, '批准', [2, 12, 20, 8], 5);
    const textIds = document.addTextWatermarkToAllPages('副本', 4, {bold: true});
    assert.equal(textIds.length, 2);
    const png = document.pagePng(0, 25.4);
    const imageId = document.addImageWatermark(0, [24, 2, 10, 10], {data: png, format: 'png'}, 180);
    const resourceId = document.addPageMultimediaResource(0, 'Image', 'PNG', png);
    const imageStampId = document.addImageStampAnnotation(0, [24, 14, 10, 10], {resourceId});
    const imageIds = document.addImageWatermarkToAllPages(png, 'png', [1, 22, 6, 6], 120);
    assert.equal(imageIds.length, 2);
    const annotations = document.annotations();
    assert.deepEqual(annotations.filter((item) => item.type === 'Stamp').map((item) => item.id),
      [textStampId, imageStampId]);
    assert.ok(annotations.some((item) => item.id === textId));
    assert.ok(annotations.some((item) => item.id === imageId));
    assert.throws(() => document.addImageWatermark(0, [1, 1, 5, 5],
      {data: png, format: 'png', resourceId}), TypeError);
    assert.throws(() => document.addImageWatermark(0, [1, 1, 5, 5],
      {data: Buffer.from('bad'), format: 'png'}), OfdError);
    assert.throws(() => document.addImageWatermarkToAllPages(png, 'png', [1, 1, 5, 5], 256), RangeError);
    const copy = Document.fromBytes(document.toBytes());
    try {
      assert.deepEqual(copy.annotations(), annotations);
      assert.deepEqual(copy.validateReferences(), []);
    } finally {
      copy.close();
    }
  } finally {
    document.close();
  }
});

test('create page and attachment link annotations through Node.js', () => {
  const document = Document.create();
  try {
    document.addBlankPage(40, 30);
    document.addBlankPage(40, 30);
    document.addAttachment('note.txt', Buffer.from('linked'));
    const targetPage = document.pages()[1].id;
    const attachmentId = document.attachments()[0].id;
    const gotoId = document.addGotoLinkAnnotation(0,
      {page_id: targetPage, kind: 'XYZ', left: 4}, [2, 3, 10, 5]);
    const attachmentLinkId = document.addAttachmentLinkAnnotation(0, attachmentId, [14, 3, 10, 5], true);
    const annotations = document.annotations();
    assert.deepEqual(annotations.map((item) => item.id), [gotoId, attachmentLinkId]);
    assert.equal(annotations[0].actions[0].destination.page_id, targetPage);
    assert.equal(annotations[1].actions[0].attributes.AttachID, attachmentId);
    assert.equal(annotations[1].actions[0].attributes.NewWindow, 'true');
    assert.throws(() => document.addGotoLinkAnnotation(0,
      {page_id: 'missing', kind: 'Fit'}, [2, 3, 10, 5]), OfdError);
    assert.throws(() => document.addAttachmentLinkAnnotation(0, 'missing', [2, 3, 10, 5]), OfdError);
    assert.throws(() => document.addAttachmentLinkAnnotation(0, attachmentId, [2, 3, 10, 5], 'yes'), TypeError);
    assert.equal(document.annotations().length, 2);
    const copy = Document.fromBytes(document.toBytes());
    try {
      assert.deepEqual(copy.annotations(), annotations);
      assert.deepEqual(copy.validateReferences(), []);
    } finally {
      copy.close();
    }
  } finally {
    document.close();
  }
});

test('edit annotation metadata and appearance through Node.js', () => {
  const document = Document.create();
  try {
    document.addBlankPage(40, 30);
    const id = document.addPathAnnotation(0, 'M 0 0 L 10 10', [2, 3, 10, 10]);
    const annotation = () => document.annotations()[0];
    document.setAnnotationCreator(0, id, '张三 & <团队>');
    document.setAnnotationSubtype(0, id, 'review');
    document.setAnnotationLastModDate(0, id, '2024-02-29');
    document.setAnnotationAppearanceBoundary(0, id, [4, 5, 10, 10]);
    document.setAnnotationRemark(0, id, '审核 & 通过');
    document.setAnnotationParameter(0, id, '状态', '草稿');
    document.setAnnotationVisible(0, id, false);
    document.setAnnotationPrint(0, id, false);
    document.setAnnotationFlag(0, id, 'NoZoom', true);
    document.setAnnotationFlag(0, id, 'NoRotate', true);
    document.setAnnotationFlag(0, id, 'ReadOnly', false);
    assert.equal(annotation().attributes.Creator, '张三 & <团队>');
    assert.equal(annotation().subtype, 'review');
    assert.equal(annotation().last_mod_date, '2024-02-29');
    assert.deepEqual(annotation().appearance_box, [4, 5, 10, 10]);
    assert.equal(annotation().remark, '审核 & 通过');
    assert.equal(annotation().parameters['状态'], '草稿');
    assert.equal(annotation().attributes.Visible, 'false');
    assert.equal(annotation().attributes.Print, 'false');
    assert.equal(annotation().attributes.NoZoom, 'true');
    assert.equal(annotation().attributes.NoRotate, 'true');
    assert.equal(annotation().attributes.ReadOnly, 'false');
    const before = document.toBytes();
    assert.throws(() => document.setAnnotationCreator(0, id, ' '), OfdError);
    assert.throws(() => document.setAnnotationLastModDate(0, id, '2023-02-29'), OfdError);
    assert.throws(() => document.setAnnotationFlag(0, id, 'Invalid', true), RangeError);
    assert.throws(() => document.setAnnotationVisible(0, id, 1), TypeError);
    assert.deepEqual(document.toBytes(), before);
    const reopened = Document.fromBytes(before);
    try {
      assert.deepEqual(reopened.annotations(), document.annotations());
      assert.deepEqual(reopened.validateReferences(), []);
    } finally {
      reopened.close();
    }
    document.removeAnnotationParameter(0, id, '状态');
    document.setAnnotationSubtype(0, id, null);
    document.setAnnotationRemark(0, id, null);
    assert.deepEqual(annotation().parameters, {});
    assert.equal(annotation().subtype, null);
    assert.equal(annotation().remark, null);
  } finally {
    document.close();
  }
});

test('create bookmarks and annotations, then validate their references', () => {
  const document = Document.create();
  try {
    document.addBlankPage(40, 30);
    const pageId = document.pages()[0].id;
    document.addBookmark(0, '第一章', { page_id: pageId, kind: 'Fit' });
    assert.equal(document.bookmarks()[0].name, '第一章');
    const pathId = document.addPathAnnotation(0, 'M 0 0 L 10 0 L 10 10 C', [1, 1, 10, 10]);
    const highlightId = document.addHighlightAnnotation(0, [3, 3, 12, 4], [255, 255, 0], 80);
    const linkId = document.addUriLinkAnnotation(0, 'https://example.com/', [2, 18, 10, 5]);
    const bookmarkLinkId = document.addBookmarkLinkAnnotation(0, '第一章', [15, 18, 10, 5]);
    assert.deepEqual(document.annotations().map((item) => item.id),
      [pathId, highlightId, linkId, bookmarkLinkId]);
    assert.deepEqual(document.validateReferences(), []);
    const copy = Document.fromBytes(document.toBytes());
    try {
      assert.equal(copy.bookmarks()[0].name, '第一章');
      assert.equal(copy.annotations().length, 4);
      assert.deepEqual(copy.validateReferences(), []);
    } finally {
      copy.close();
    }
    document.removeAnnotation(0, bookmarkLinkId);
    document.removeBookmark(0, '第一章');
    assert.deepEqual(document.bookmarks(), []);
    assert.equal(document.annotations().length, 3);
    assert.deepEqual(document.validateReferences(), []);
    assert.throws(() => document.addBookmark(0, '', { page_id: pageId, kind: 'Fit' }), OfdError);
  } finally {
    document.close();
  }
});

test('prepare an archival copy while retaining the source package', () => {
  const document = Document.create();
  try {
    document.addTextPage('archive', 40, 30);
    assert.ok(document.archiveViolations().some((issue) => issue.rule === 'DOC_TYPE'));
    const before = document.toBytes();
    const prepared = document.prepareArchive();
    try {
      assert.ok(!prepared.archiveViolations().some((issue) => issue.rule === 'DOC_TYPE'));
      assert.deepEqual(prepared.pageText(0), ['archive']);
      assert.deepEqual(prepared.validateReferences(), []);
      assert.deepEqual(document.toBytes(), before);
      const reopened = Document.fromBytes(prepared.toBytes());
      try {
        assert.ok(!reopened.archiveViolations().some((issue) => issue.rule === 'DOC_TYPE'));
      } finally {
        reopened.close();
      }
    } finally {
      prepared.close();
    }
  } finally {
    document.close();
  }
});

test('preserve custom actions and uppercase XML references through the native binding', () => {
  const source = Document.create();
  const target = Document.create();
  try {
    source.addTextPage('archive', 40, 30);
    const oldPage = 'Doc_0/Pages/Page_0/Content.xml';
    const upperPage = 'Doc_0/Pages/Page_0/CONTENT.XML';
    const page = source.entry(oldPage).toString().replace(
      '</ofd:Page>',
      '<ofd:Actions><ofd:Action Event="PO"><ofd:URI URI="relative"/></ofd:Action></ofd:Actions>'
      + '<ext:Actions xmlns:ext="urn:example:business"><ext:Action><ext:URI URI="local"/>'
      + '</ext:Action></ext:Actions></ofd:Page>',
    );
    source.setEntry(upperPage, Buffer.from(page));
    source.removeEntry(oldPage);
    const documentPath = 'Doc_0/Document.xml';
    source.setEntry(documentPath, Buffer.from(source.entry(documentPath).toString()
      .replace('Pages/Page_0/Content.xml', 'Pages/Page_0/CONTENT.XML')));
    source.setEntry('Doc_0/Payload.bin', Buffer.from('payload'));
    source.setEntry('Doc_0/Refs.XML', Buffer.from(
      '<Refs><FileLoc>/Doc_0/Payload.bin</FileLoc></Refs>',
    ));
    source.setEntry('Doc_0/Business.xml', Buffer.from(
      '<Business><Actions><Action><URI URI="local"/></Action></Actions></Business>',
    ));

    assert.ok(source.archiveViolations().some((issue) => issue.rule === 'NON_GOTO_ACTION'));
    const prepared = source.prepareArchive();
    try {
      const preparedPage = prepared.entry(upperPage).toString();
      assert.ok(!preparedPage.includes('<ofd:URI'));
      assert.ok(preparedPage.includes('<ext:Action><ext:URI URI="local"/></ext:Action>'));
      assert.deepEqual(prepared.entry('Doc_0/Business.xml'), source.entry('Doc_0/Business.xml'));
      assert.ok(!prepared.archiveViolations().some((issue) => issue.rule === 'NON_GOTO_ACTION'));
      assert.deepEqual(prepared.pageText(0), ['archive']);
    } finally {
      prepared.close();
    }

    assert.equal(target.appendDocumentBodyFrom(source, 0), 1);
    assert.ok(target.entry('Imported_0/Doc_0/Refs.XML').toString()
      .includes('/Imported_0/Doc_0/Payload.bin'));
    source.addVersionSnapshot(0, 'uppercase-xml');
    assert.ok(source.entry('Versions/uppercase-xml/Snapshot/Doc_0/Refs.XML').toString()
      .includes('<FileLoc>Payload.bin</FileLoc>'));
  } finally {
    source.close();
    target.close();
  }
});

test('foreign namespace path lookalikes cannot break imported or snapshotted documents', () => {
  const source = Document.create();
  const target = Document.create();
  try {
    source.setEntry('Doc_0/Payload.bin', Buffer.from('payload'));
    source.setEntry('Doc_0/Foreign.xml', Buffer.from(
      '<x:Refs xmlns:x="urn:other"><x:FileLoc>/Doc_0/Payload.bin</x:FileLoc></x:Refs>',
    ));
    const sourceBefore = source.toBytes();
    const targetBefore = target.toBytes();
    assert.throws(() => target.appendDocumentBodyFrom(source, 0), OfdError);
    assert.throws(() => source.addVersionSnapshot(0, 'foreign'), OfdError);
    assert.deepEqual(source.toBytes(), sourceBefore);
    assert.deepEqual(target.toBytes(), targetBefore);

    const literal = Buffer.from(
      '<x:Refs xmlns:x="urn:other"><x:FileLoc>/outside/value.xml</x:FileLoc></x:Refs>',
    );
    source.setEntry('Doc_0/Foreign.xml', literal);
    assert.equal(target.appendDocumentBodyFrom(source, 0), 1);
    assert.deepEqual(target.entry('Imported_0/Doc_0/Foreign.xml'), literal);
    source.addVersionSnapshot(0, 'foreign');
    assert.deepEqual(source.entry('Versions/foreign/Snapshot/Doc_0/Foreign.xml'), literal);
  } finally {
    source.close();
    target.close();
  }
});

test('remove viewer preferences from an archival copy through the Node.js binding', () => {
  const document = Document.create();
  try {
    document.addBlankPage(40, 30);
    const entry = 'Doc_0/Document.xml';
    const updated = document.entry(entry).toString().replace(
      '</ofd:Document>',
      '<ofd:VPreferences><ofd:PageMode>UseOutlines</ofd:PageMode></ofd:VPreferences></ofd:Document>',
    );
    document.setEntry(entry, Buffer.from(updated));
    const before = document.toBytes();
    assert.ok(document.archiveViolations().some((issue) => issue.rule === 'V_PREFERENCES'));
    const prepared = document.prepareArchive();
    try {
      assert.deepEqual(document.toBytes(), before);
      assert.ok(!prepared.entry(entry).includes(Buffer.from('VPreferences')));
      assert.ok(!prepared.archiveViolations().some((issue) => issue.rule === 'V_PREFERENCES'));
    } finally {
      prepared.close();
    }
  } finally {
    document.close();
  }
});

test('remove redundant stroke attributes from an archival copy through the Node.js binding', () => {
  const document = Document.create();
  try {
    document.addTextPage('archive', 40, 30);
    const entry = 'Doc_0/Pages/Page_0/Content.xml';
    const updated = document.entry(entry).toString().replace(
      '<ofd:TextObject ',
      '<ofd:TextObject Stroke="false" LineWidth="2" Cap="Round" ',
    );
    document.setEntry(entry, Buffer.from(updated));
    const before = document.toBytes();
    assert.equal(document.archiveViolations().filter((issue) => issue.rule === 'REDUNDANT_PAINT').length, 2);
    const prepared = document.prepareArchive();
    try {
      assert.deepEqual(document.toBytes(), before);
      assert.ok(!prepared.entry(entry).includes(Buffer.from('LineWidth="2"')));
      assert.ok(!prepared.archiveViolations().some((issue) => issue.rule === 'REDUNDANT_PAINT'));
      assert.deepEqual(prepared.pageText(0), document.pageText(0));
    } finally {
      prepared.close();
    }
  } finally {
    document.close();
  }
});

test('preserve raw package entries and embedded attachment bytes', () => {
  const document = Document.create();
  const payload = Buffer.from([0, 255, 42, 0, 128]);
  try {
    document.addTextPage('attachment', 40, 30);
    document.setEntry('Extra/binary.dat', payload);
    document.addAttachment('数据.bin', payload);
    assert.ok(document.entries().includes('Extra/binary.dat'));
    assert.deepEqual(document.entry('Extra/binary.dat'), payload);
    assert.equal(document.attachments()[0].name, '数据.bin');
    assert.deepEqual(document.attachmentData(0), payload);
    const copy = Document.fromBytes(document.toBytes());
    try {
      assert.deepEqual(copy.entry('Extra/binary.dat'), payload);
      assert.deepEqual(copy.attachmentData(0), payload);
      assert.deepEqual(copy.validateReferences(), []);
    } finally {
      copy.close();
    }
    assert.equal(document.removeEntry('Extra/binary.dat'), true);
    assert.equal(document.removeEntry('Extra/binary.dat'), false);
    const withoutExtra = Document.fromBytes(document.toBytes());
    try {
      assert.ok(!withoutExtra.entries().includes('Extra/binary.dat'));
    } finally {
      withoutExtra.close();
    }
    assert.throws(() => document.removeEntry('../outside'), OfdError);
    assert.throws(() => document.entry('missing.bin'), OfdError);
    assert.throws(() => document.setEntry('../outside', payload), OfdError);
  } finally {
    document.close();
  }
});

test('edit attachments in a selected document body and preserve metadata', () => {
  const document = Document.create();
  try {
    document.addTextPage('first', 40, 30);
    const secondBody = document.appendDocumentBody();
    document.addAttachmentToDocument(0, 'first.txt', Buffer.from('old'));
    document.addAttachmentToDocument(secondBody, 'second.txt', Buffer.from('before'));
    const id = document.attachments()[1].id;
    document.renameAttachment(1, 'renamed.txt');
    document.replaceAttachmentData(1, Buffer.from([0, 255, 42]));
    document.setAttachmentMetadata(1, {
      format: 'application/octet-stream',
      creation_date: '2024-02-29T12:30:00',
      mod_date: '2024-03-01T09:00:00',
      visible: false,
      usage: 'Data',
    });
    const edited = document.attachments()[1];
    assert.equal(edited.document_index, secondBody);
    assert.equal(edited.id, id);
    assert.equal(edited.name, 'renamed.txt');
    assert.equal(edited.format, 'application/octet-stream');
    assert.equal(edited.creation_date, '2024-02-29T12:30:00');
    assert.equal(edited.mod_date, '2024-03-01T09:00:00');
    assert.equal(edited.visible, false);
    assert.equal(edited.usage, 'Data');
    assert.deepEqual(document.attachmentData(1), Buffer.from([0, 255, 42]));

    const beforeError = document.toBytes();
    assert.throws(() => document.renameAttachment(99, 'missing.txt'), OfdError);
    assert.throws(() => document.setAttachmentMetadata(1, { creation_date: 'invalid' }), OfdError);
    assert.throws(() => document.setAttachmentMetadata(1, { visible: 'no' }), OfdError);
    assert.throws(() => document.removeAttachment(-1), RangeError);
    assert.deepEqual(document.toBytes(), beforeError);

    const reopened = Document.fromBytes(document.toBytes());
    try {
      assert.deepEqual(reopened.attachments(), document.attachments());
      assert.deepEqual(reopened.attachmentData(1), Buffer.from([0, 255, 42]));
      reopened.removeAttachment(0);
      assert.equal(reopened.attachments().length, 1);
      assert.equal(reopened.attachments()[0].id, id);
      assert.deepEqual(reopened.attachmentData(0), Buffer.from([0, 255, 42]));
      assert.deepEqual(reopened.validateReferences(), []);
    } finally {
      reopened.close();
    }
  } finally {
    document.close();
  }
});

test('sign a document, verify its value and references, then detect tampering', () => {
  const key = fs.readFileSync(fixture('user-private-key.bin'));
  const certificate = fs.readFileSync(fixture('user-cert.der'));
  const document = Document.create();
  try {
    document.addTextPage('signed payload', 40, 30);
    assert.equal(document.addDigitalSignature(key, '2020-04-22 16:22:44'), 0);
    assert.equal(document.signatures().length, 1);
    assert.ok(document.signedValueData(0).length > 0);
    assert.equal(document.checkDigitalSignatureWithCertificate(0, certificate), true);
    assert.ok(document.checkSignatureReferences(0).every((check) => check.valid));
    const copy = Document.fromBytes(document.toBytes());
    try {
      assert.equal(copy.checkDigitalSignatureWithCertificate(0, certificate), true);
      assert.ok(copy.checkSignatureReferences(0).every((check) => check.valid));
      const signedPath = copy.signatures()[0].references.find((reference) => reference.path.endsWith('Content.xml')).path;
      const original = copy.entry(signedPath).toString('utf8');
      assert.ok(original.includes('signed payload'));
      copy.setEntry(signedPath, Buffer.from(original.replace('signed payload', 'changed payload')));
      assert.equal(copy.checkDigitalSignatureWithCertificate(0, certificate), true);
      assert.ok(copy.checkSignatureReferences(0).some((check) => !check.valid));
    } finally {
      copy.close();
    }
  } finally {
    document.close();
  }
});

test('accept XML Base64 whitespace in signature reference digests', () => {
  const sample = path.resolve(__dirname, '../../../../ofdrw-reader/src/test/resources/SESV4SignDoc.ofd');
  const document = Document.open(sample);
  try {
    const signature = document.signatures()[0];
    const value = signature.references[0].check_value;
    const wrapped = `${value.slice(0, 20)}\n\t${value.slice(20)}`;
    const source = document.entry(signature.path).toString('utf8');
    assert.ok(source.includes(value));
    document.setEntry(signature.path, Buffer.from(source.replace(value, wrapped)));
    const checks = document.checkSignatureReferences(0);
    assert.ok(checks.every((check) => check.valid));
    assert.equal(checks[0].expected, wrapped);
  } finally {
    document.close();
  }
});

test('sign and verify a raw SM2 signature with a custom distinguishing ID', () => {
  const key = fs.readFileSync(fixture('user-private-key.bin'));
  const certificate = fs.readFileSync(fixture('user-cert.der'));
  const document = Document.create();
  try {
    document.addTextPage('custom signature ID', 40, 30);
    assert.throws(() => document.addDigitalSignature(key, '2020-04-22 16:22:44', 'bad\0id'), TypeError);
    assert.equal(document.addDigitalSignature(key, '2020-04-22 16:22:44', 'node-signature-id'), 0);
    assert.equal(document.checkDigitalSignatureWithCertificate(0, certificate), false);
    assert.equal(document.checkDigitalSignatureWithCertificate(0, certificate, 'wrong-id'), false);
    assert.equal(document.checkDigitalSignatureWithCertificate(0, certificate, 'node-signature-id'), true);
    assert.ok(document.checkSignatureReferences(0).every((check) => check.valid));
    const reopened = Document.fromBytes(document.toBytes());
    try {
      assert.equal(reopened.checkDigitalSignatureWithCertificate(0, certificate, 'node-signature-id'), true);
    } finally { reopened.close(); }
  } finally { document.close(); }
});

test('check a direct SM2 trust anchor and a Java seal issuer chain', () => {
  const signer = fs.readFileSync(fixture('trust-leaf-cert.der'));
  const anchor = fs.readFileSync(fixture('trust-root-cert.der'));
  const unrelated = fs.readFileSync(fixture('root-cert.der'));
  const document = Document.create();
  try {
    document.addTextPage('direct trust', 40, 30);
    document.addDigitalSignature(fs.readFileSync(fixture('trust-leaf-private-key.bin')),
      '2030-01-01 00:00:00');
    assert.equal(document.checkSignatureDirectTrustAnchor(0, signer, anchor, 1_893_456_000), true);
    assert.equal(document.checkSignatureDirectTrustAnchor(0, signer, unrelated, 1_893_456_000), false);
    const trusted = document.verifySignatureWithDirectTrustAnchor(0, signer, anchor, 1_893_456_000);
    assert.equal(trusted.all_valid, true);
    assert.equal(trusted.direct_trust_anchor_valid, true);
    assert.equal(trusted.references_valid, true);
    const untrusted = document.verifySignatureWithDirectTrustAnchor(0, signer, unrelated, 1_893_456_000);
    assert.equal(untrusted.direct_trust_anchor_valid, false);
    assert.equal(untrusted.all_valid, false);
    assert.equal(document.verifySignatureWithDirectTrustAnchor(0, signer, anchor,
      1_893_456_000, {signatureDistid: 'wrong-id'}).signature_value_valid, false);
    assert.throws(() => document.verifySignatureWithDirectTrustAnchor(0, signer, anchor, -1), RangeError);
    assert.throws(() => document.verifySignatureWithDirectTrustAnchor(0, signer, anchor,
      1_893_456_000, {certificateDistid: 'bad\0id'}), TypeError);
    assert.throws(() => document.checkSignatureDirectTrustAnchor(0, signer, anchor, -1), RangeError);
    assert.throws(() => document.checkSignatureDirectTrustAnchor(0, signer, anchor,
      1_893_456_000, 'bad\0id'), TypeError);
  } finally { document.close(); }

  const signed = Document.open(path.resolve(__dirname,
    '../../../../ofdrw-reader/src/test/resources/SESV4SignDoc.ofd'));
  try {
    assert.equal(signed.checkSesSealIssuerOrderedSm2Chain(0, [], unrelated, 1_600_000_000), false);
    assert.throws(() => signed.checkSesSealIssuerOrderedSm2Chain(0, 'bad', unrelated,
      1_600_000_000), TypeError);
    assert.throws(() => signed.checkSesSealIssuerOrderedSm2Chain(0, [], unrelated, -1), RangeError);
  } finally { signed.close(); }
});

test('verify a signed document with trusted SM2 chains and candidate pools', () => {
  const signer = fs.readFileSync(fixture('chain-leaf-cert.der'));
  const intermediate = fs.readFileSync(fixture('chain-intermediate-cert.der'));
  const unrelated = fs.readFileSync(fixture('root-cert.der'));
  const anchor = fs.readFileSync(fixture('chain-root-cert.der'));
  const document = Document.create();
  try {
    document.addTextPage('trusted payload', 40, 30);
    document.addDigitalSignature(
      fs.readFileSync(fixture('chain-leaf-private-key.bin')), '2030-01-01 00:00:00');
    assert.equal(document.checkSignatureCertificateValidity(0, signer, 1_893_456_000), true);
    assert.equal(document.checkSignatureCertificateValidity(0, signer, 0), false);
    assert.throws(() => document.checkSignatureCertificateValidity(0, signer, -1), RangeError);
    const options = {
      signerCertificate: signer, signerIntermediates: [intermediate],
      signerAnchor: anchor, atUnixSeconds: 1_893_456_000,
    };
    const signerOrdered = document.verifySignatureWithOrderedSm2Chain(0, signer,
      [intermediate], anchor, 1_893_456_000);
    assert.equal(signerOrdered.all_valid, true);
    assert.equal(signerOrdered.certificate_chain_valid, true);
    const signerPooled = document.verifySignatureWithCertificatePool(0, signer,
      [unrelated, intermediate], anchor, 1_893_456_000);
    assert.equal(signerPooled.all_valid, true);
    const signerMissing = document.verifySignatureWithCertificatePool(0, signer,
      [unrelated], anchor, 1_893_456_000);
    assert.equal(signerMissing.signature_value_valid, true);
    assert.equal(signerMissing.certificate_chain_valid, false);
    assert.equal(signerMissing.all_valid, false);
    assert.throws(() => document.verifySignatureWithOrderedSm2Chain(0, signer,
      'bad', anchor, 1_893_456_000), TypeError);
    assert.throws(() => document.verifySignatureWithCertificatePool(0, signer,
      [intermediate], anchor, -1), RangeError);
    const ordered = document.verifySignatureWithOrderedSm2Chains(0, options);
    assert.equal(ordered.all_valid, true);
    assert.equal(ordered.signature.certificate_chain_valid, true);
    assert.equal(ordered.signature.references_valid, true);
    assert.equal(ordered.seal_issuer_chain_valid, null);
    const pooled = document.verifySignatureWithCertificatePools(0,
      {...options, signerIntermediates: [unrelated, intermediate]});
    assert.equal(pooled.all_valid, true);
    const missing = document.verifySignatureWithCertificatePools(0,
      {...options, signerIntermediates: [unrelated]});
    assert.equal(missing.signature.signature_value_valid, true);
    assert.equal(missing.signature.certificate_chain_valid, false);
    assert.equal(missing.all_valid, false);
    assert.throws(() => document.verifySignatureWithOrderedSm2Chains(0,
      {...options, signerIntermediates: 'invalid'}), TypeError);
    assert.throws(() => document.verifySignatureWithOrderedSm2Chains(0,
      {...options, atUnixSeconds: -1}), RangeError);
    const signedPath = document.signatures()[0].references
      .find((reference) => reference.path.endsWith('Content.xml')).path;
    document.setEntry(signedPath, Buffer.from(document.entry(signedPath).toString('utf8')
      .replace('trusted payload', 'changed payload')));
    const tampered = document.verifySignatureWithOrderedSm2Chains(0, options);
    assert.equal(tampered.signature.signature_value_valid, true);
    assert.equal(tampered.signature.references_valid, false);
    assert.equal(tampered.all_valid, false);
  } finally { document.close(); }
});

test('inspect a Java SES-signed OFD and its embedded seal', () => {
  const file = path.resolve(__dirname, '../../../../ofdrw-reader/src/test/resources/SESV4SignDoc.ofd');
  const document = Document.open(file);
  try {
    assert.equal(document.signatures()[0].type, 'Seal');
    assert.ok(document.sealData(0).length > 1000);
    assert.ok(document.signedValueData(0).length > 0);
    assert.equal(document.checkSesSignatureValue(0), true);
    assert.equal(document.checkSesSealSignature(0), true);
    assert.equal(document.checkSesSignatureValueWithDistid(0, '1234567812345678'), true);
    assert.equal(document.checkSesSignatureValueWithDistid(0, 'wrong-id'), false);
    assert.equal(document.checkSesSealSignatureWithDistid(0, '1234567812345678'), true);
    assert.equal(document.checkSesSealSignatureWithDistid(0, 'wrong-id'), false);
    const description = document.checkSignatureDescriptionHash(0);
    assert.equal(description.valid, true);
    assert.equal(description.expected, description.actual);
    assert.equal(document.checkSignatureSealMatch(0), true);
    assert.equal(document.checkSesSealSignerAuthorization(0), true);
    assert.equal(document.checkSesSealValidity(0, 0), false);
    assert.equal(document.checkSesSealValidity(0, 1600000000), true);
    assert.equal(document.checkSesSealCertificateValidity(0, 0), false);
    assert.equal(document.checkSesSealCertificateValidity(0, 1600000000), true);
    assert.throws(() => document.checkSesSealValidity(0, -1), RangeError);
    assert.ok(document.checkSignatureReferences(0).every((check) => check.valid));
  } finally {
    document.close();
  }
});

test('nested OFD seal keeps a transparent background in Node.js previews', () => {
  function firstPngPixel(png) {
    assert.deepEqual(png.subarray(0, 8), Buffer.from([137, 80, 78, 71, 13, 10, 26, 10]));
    let offset = 8;
    const imageData = [];
    while (offset < png.length) {
      const length = png.readUInt32BE(offset);
      const type = png.toString('ascii', offset + 4, offset + 8);
      if (type === 'IHDR') {
        assert.equal(png[offset + 16], 8, 'expected 8-bit PNG');
        assert.equal(png[offset + 17], 6, 'expected RGBA PNG');
      }
      if (type === 'IDAT') imageData.push(png.subarray(offset + 8, offset + 8 + length));
      offset += length + 12;
      if (type === 'IEND') break;
    }
    const row = zlib.inflateSync(Buffer.concat(imageData));
    // Every PNG filter predicts zero for the top-left pixel.
    return [...row.subarray(1, 5)];
  }

  const file = path.resolve(__dirname, '../../../../ofdrw-layout/src/test/resources/keyword2.ofd');
  const document = Document.open(file);
  try {
    const svg = document.pageSvg(0);
    const stamp = svg.indexOf('data-ofd-stamp=');
    assert.notEqual(stamp, -1);
    const match = svg.slice(stamp).match(/href="data:image\/png;base64,([^"]+)"/);
    assert.ok(match, 'expected a rasterized OFD seal');
    assert.equal(firstPngPixel(Buffer.from(match[1], 'base64'))[3], 0);
  } finally { document.close(); }

  const blank = Document.create();
  try {
    blank.addBlankPage(20, 20);
    assert.deepEqual(firstPngPixel(blank.pagePng(0, 25.4)), [255, 255, 255, 255]);
  } finally { blank.close(); }
});

test('create SES seal, add positioned signature, and verify after reopening', () => {
  const key = fs.readFileSync(fixture('user-private-key.bin'));
  const certificate = fs.readFileSync(fixture('user-cert.der'));
  const picture = fs.readFileSync(path.resolve(__dirname,
    '../../../../ofdrw-layout/src/test/resources/testimg.png'));
  const spec = {
    version: 4, vendorId: 'ofdrw-rs', sealId: 'node-seal', name: 'Node 印章',
    sealType: 3, authorizedSignerCertificates: [certificate],
    createdAt: 1600000000, validFrom: 1600000000, validUntil: 1900000000,
    pictureType: 'PNG', pictureWidthMm: 40, pictureHeightMm: 40,
  };
  assert.throws(() => createSesSeal(key, certificate, picture,
    {...spec, authorizedSignerCertificates: 'invalid'}), TypeError);
  assert.throws(() => createSesSeal(Buffer.alloc(32, 1), certificate, picture, spec), OfdError);
  const seal = createSesSeal(key, certificate, picture, spec);
  assert.equal(seal[0], 0x30);
  const document = Document.create();
  try {
    document.addTextPage('signed', 210, 297);
    assert.throws(() => document.addSesSignature(key, certificate, seal,
      '2020-09-13 12:26:40', {pageIndex: 0}), TypeError);
    assert.throws(() => document.addSesSignature(key, certificate, seal,
      '2020-09-13 12:26:40', {pageIndex: -1, boundary: [10, 20, 40, 40]}), RangeError);
    assert.equal(document.addSesSignature(key, certificate, seal,
      '2020-09-13 12:26:40', {pageIndex: 0, boundary: [10, 20, 40, 40]}), 0);
    assert.equal(document.signatures()[0].stamps.length, 1);
    assert.match(document.pageSvg(0).toString(), /data-ofd-stamp="s002"/);
    assert.equal(document.checkSesSealSignature(0), true);
    assert.equal(document.checkSesSealSignerAuthorization(0), true);
    assert.equal(document.checkSesSignatureValue(0), true);
    const reopened = Document.fromBytes(document.toBytes());
    try {
      assert.equal(reopened.checkSesSealSignature(0), true);
      assert.equal(reopened.checkSesSignatureValue(0), true);
      assert.ok(reopened.checkSignatureReferences(0).every((item) => item.valid));
    } finally { reopened.close(); }
  } finally { document.close(); }
});

test('create and verify SES seal and signature with independent custom SM2 IDs', () => {
  const key = fs.readFileSync(fixture('user-private-key.bin'));
  const certificate = fs.readFileSync(fixture('user-cert.der'));
  const picture = fs.readFileSync(path.resolve(__dirname,
    '../../../../ofdrw-layout/src/test/resources/testimg.png'));
  const seal = createSesSeal(key, certificate, picture, {
    version: 5, vendorId: 'ofdrw-rs', sealId: 'custom-id', name: 'Custom ID',
    sealType: 3, authorizedSignerCertificates: [certificate],
    createdAt: 1600000000, validFrom: 1600000000, validUntil: 1900000000,
    pictureType: 'PNG', pictureWidthMm: 40, pictureHeightMm: 40,
  }, 'custom-seal-id');
  const document = Document.create();
  try {
    document.addTextPage('signed', 210, 297);
    assert.equal(document.addSesSignature(key, certificate, seal,
      '2020-09-13 12:26:40', {signatureDistid: 'custom-signature-id',
        sealDistid: 'custom-seal-id'}), 0);
    assert.equal(document.checkSesSignatureValue(0), false);
    assert.equal(document.checkSesSignatureValueWithDistid(0, 'custom-signature-id'), true);
    assert.equal(document.checkSesSealSignature(0), false);
    assert.equal(document.checkSesSealSignatureWithDistid(0, 'custom-seal-id'), true);
    const reopened = Document.fromBytes(document.toBytes());
    try {
      assert.equal(reopened.checkSesSignatureValueWithDistid(0, 'custom-signature-id'), true);
      assert.equal(reopened.checkSesSealSignatureWithDistid(0, 'custom-seal-id'), true);
    } finally { reopened.close(); }
  } finally { document.close(); }
});

test('verify a Java integrity manifest and distinguish extra files from signature failure', () => {
  const file = path.resolve(__dirname, '../../../../ofdrw-crypto/src/test/resources/hello-integrity.ofd');
  const document = Document.open(file);
  try {
    assert.ok(document.integrityManifest().files.length > 0);
    assert.equal(document.checkIntegrityManifest().complete, true);
    assert.deepEqual(document.verifyIntegrityManifestSm2(), {
      manifest_complete: true, signature_valid: true, all_valid: true,
    });
    assert.equal(document.verifyIntegrityManifestSm2('wrong-id').signature_valid, false);
    document.setEntry('extra.bin', Buffer.from('extra'));
    assert.deepEqual(document.checkIntegrityManifest().extra_files, ['extra.bin']);
    assert.deepEqual(document.verifyIntegrityManifestSm2(), {
      manifest_complete: false, signature_valid: true, all_valid: false,
    });
  } finally {
    document.close();
  }
});

test('create and reopen SM2 integrity protection', () => {
  const key = fs.readFileSync(fixture('user-private-key.bin'));
  const certificate = fs.readFileSync(fixture('user-cert.der'));
  const document = Document.create();
  try {
    document.addTextPage('protected', 40, 30);
    assert.equal(document.integrityManifest(), null);
    assert.equal(document.checkIntegrityManifest(), null);
    assert.equal(document.verifyIntegrityManifestSm2(), null);
    document.protectIntegritySm2(key, certificate);
    assert.equal(document.verifyIntegrityManifestSm2().all_valid, true);
    assert.throws(() => document.protectIntegritySm2(key, certificate), OfdError);
    const copy = Document.fromBytes(document.toBytes());
    try {
      assert.equal(copy.verifyIntegrityManifestSm2().all_valid, true);
    } finally {
      copy.close();
    }
  } finally {
    document.close();
  }
});

test('verify SM2 integrity with an ordered chain and unordered certificate pool', () => {
  const atUnixSeconds = 1_893_456_000;
  const signer = fs.readFileSync(fixture('trust-leaf-cert.der'));
  const anchor = fs.readFileSync(fixture('trust-root-cert.der'));
  const ordered = Document.create();
  try {
    assert.equal(ordered.verifyIntegrityManifestSm2WithOrderedChain(
      signer, [], anchor, atUnixSeconds), null);
    ordered.protectIntegritySm2(fs.readFileSync(fixture('trust-leaf-private-key.bin')), signer);
    const valid = ordered.verifyIntegrityManifestSm2WithOrderedChain(
      signer, [], anchor, atUnixSeconds);
    assert.deepEqual(valid, {
      manifest_complete: true, signature_valid: true,
      certificate_chain_valid: true, all_valid: true,
    });
    const wrongId = ordered.verifyIntegrityManifestSm2WithOrderedChain(
      signer, [], anchor, atUnixSeconds, {signatureDistid: 'wrong-id'});
    assert.equal(wrongId.signature_valid, false);
    assert.equal(wrongId.all_valid, false);
    const expired = ordered.verifyIntegrityManifestSm2WithOrderedChain(
      signer, [], anchor, 1_600_000_000);
    assert.equal(expired.signature_valid, true);
    assert.equal(expired.certificate_chain_valid, false);
    assert.equal(expired.all_valid, false);
    assert.throws(() => ordered.verifyIntegrityManifestSm2WithOrderedChain(
      signer, 'invalid', anchor, atUnixSeconds), TypeError);
    assert.throws(() => ordered.verifyIntegrityManifestSm2WithOrderedChain(
      signer, [], anchor, -1), RangeError);
  } finally { ordered.close(); }

  const chainSigner = fs.readFileSync(fixture('chain-leaf-cert.der'));
  const chainAnchor = fs.readFileSync(fixture('chain-root-cert.der'));
  const intermediate = fs.readFileSync(fixture('chain-intermediate-cert.der'));
  const unrelated = fs.readFileSync(fixture('root-cert.der'));
  const pooled = Document.create();
  try {
    pooled.protectIntegritySm2(fs.readFileSync(fixture('chain-leaf-private-key.bin')), chainSigner);
    const valid = pooled.verifyIntegrityManifestSm2WithCertificatePool(
      chainSigner, [unrelated, intermediate], chainAnchor, atUnixSeconds);
    assert.equal(valid.signature_valid, true);
    assert.equal(valid.certificate_chain_valid, true);
    assert.equal(valid.all_valid, true);
    const missing = pooled.verifyIntegrityManifestSm2WithCertificatePool(
      chainSigner, [unrelated], chainAnchor, atUnixSeconds);
    assert.equal(missing.signature_valid, true);
    assert.equal(missing.certificate_chain_valid, false);
    assert.equal(missing.all_valid, false);
  } finally { pooled.close(); }
});

test('reject malformed data and invalid calls without corrupting a document', () => {
  assert.throws(() => Document.fromBytes(Buffer.from('bad')), OfdError);
  const document = Document.create();
  try {
    assert.throws(() => document.pageText(0), OfdError);
    assert.throws(() => document.addTextPage('x', 0, 10), RangeError);
    assert.throws(() => document.addTextPage(3), TypeError);
    assert.throws(() => document.pageText(-1), RangeError);
    assert.throws(() => document.pagePng(0, Infinity), RangeError);
    assert.throws(() => document.duplicatePageObject(0, 'x\0y'), TypeError);
    assert.throws(() => document.addPathObject(0, 'M 0 0', [0, 0, 0, 1]), RangeError);
    assert.throws(() => document.addTextObject(0, 'x', [0, 0, 1, 1], null), TypeError);
    assert.throws(() => document.setEntry('x', 'text'), TypeError);
    assert.equal(document.pageCount, 0);
  } finally {
    document.close();
  }
});


test('import PDF Separation colors through the C ABI', () => {
  for (const name of [
    'separation-paths',
    'separation-cmyk',
    'separation-sampled',
    'separation-stitching',
    'separation-calculator',
    'separation-image',
    'separation-image16-mask',
    'separation-jpeg',
    'separation-indexed',
    'separation-all',
    'separation-none',
    'separation-none-clipping',
    'separation-none-independent',
    'separation-none-text-clip',
    'separation-shading',
    'separation-mesh',
    'separation-patch',
    'separation-plane',
  ]) {
    const document = Document.fromPdfBytes(fs.readFileSync(fixture(name + '.pdf')));
    try {
      if (name === 'separation-none') {
        assert.deepEqual(document.pageText(0), ['none']);
        assert.deepEqual(document.pageImages(0), []);
      }
      const png = document.pagePng(0, 72);
      const reopened = Document.fromBytes(document.toBytes());
      try {
        assert.deepEqual(reopened.pagePng(0, 72), png);
        assert.deepEqual(reopened.validateReferences(), []);
      } finally { reopened.close(); }
    } finally { document.close(); }
  }
});


test('import PDF DeviceN colors through the C ABI', () => {
  for (const name of [
    'devicen-jpx-two',
    'devicen-jpx-five',
    'devicen-jpx-thirty-two',
    'devicen-jpx-five-mask',
    'devicen-jpx-five-sixteen-mask',
    'devicen-jpx-five-soft-mask',
    'devicen-jpx-five-no-bpc',
    'devicen-jpx-four',
    'devicen-jpx-three-alpha',
    'devicen-jpx-five-container',
    'devicen-paths',
    'devicen-sampled',
    'devicen-cubic',
    'devicen-cmyk',
    'devicen-three-input',
    'devicen-image',
    'devicen-image-five',
    'devicen-image-thirty-two',
    'devicen-image16-mask',
    'devicen-indexed',
    'devicen-none',
    'devicen-mixed-none',
    'devicen-none-clipping',
    'devicen-nchannel',
    'devicen-shading',
    'devicen-mesh',
    'devicen-patch',
    'devicen-plane',
  ]) {
    const document = Document.fromPdfBytes(fs.readFileSync(fixture(name + '.pdf')));
    try {
      if (name === 'devicen-none') {
        assert.deepEqual(document.pageText(0), ['none']);
        assert.deepEqual(document.pageImages(0), []);
      }
      const png = document.pagePng(0, 72);
      const reopened = Document.fromBytes(document.toBytes());
      try {
        assert.deepEqual(reopened.pagePng(0, 72), png);
        assert.deepEqual(reopened.validateReferences(), []);
      } finally { reopened.close(); }
    } finally { document.close(); }
  }
});


test('import image rendering intents through the C ABI', () => {
  for (const kind of ['xobject', 'form', 'inline', 'inline-embedded-ei', 'inline-trailing-cr', 'inline-flate', 'form-inline', 'masked']) {
    const document = Document.fromPdfBytes(fs.readFileSync(fixture('image-intent-' + kind + '.pdf')));
    try {
      assert.equal(document.pageImages(0).length, 5);
      const colors = Array.from({length: 5}, (_, index) => document.pageImageData(0, index));
      assert.deepEqual(colors[0], colors[2]);
      assert.notDeepEqual(colors[0], colors[1]);
      assert.notDeepEqual(colors[0], colors[3]);
      if (kind !== 'masked') assert.deepEqual(colors[1], colors[4]);
      const png = document.pagePng(0, 72);
      const reopened = Document.fromBytes(document.toBytes());
      try {
        assert.deepEqual(reopened.pagePng(0, 72), png);
        assert.deepEqual(reopened.validateReferences(), []);
      } finally { reopened.close(); }
    } finally { document.close(); }
  }
});


test('import tiling patterns through the C ABI', () => {
  for (const kind of ['colored', 'uncolored-rgb', 'nested', 'mask', 'cell-text', 'state-reuse']) {
    const document = Document.fromPdfBytes(fs.readFileSync(fixture('tiling-' + kind + '.pdf')));
    try {
      assert.equal(document.pageCount, 1);
      assert.ok(document.pageSvg(0).includes('<pattern'));
      assert.ok(document.pageCompositeUnits(0).length > 0);
      if (kind === 'state-reuse') assert.equal(document.pageCompositeUnits(0).length, 1);
      const png = document.pagePng(0, 72);
      const reopened = Document.fromBytes(document.toBytes());
      try {
        assert.deepEqual(reopened.pagePng(0, 72), png);
        assert.deepEqual(reopened.validateReferences(), []);
      } finally { reopened.close(); }
    } finally { document.close(); }
  }
});


test('import shading patterns through the C ABI', () => {
  for (const kind of ['axial', 'affine-radial', 'bbox-background', 'combined-alpha', 'nested', 'tensor', 'state-reuse', 'indirect-params']) {
    const document = Document.fromPdfBytes(fs.readFileSync(fixture('shading-pattern-' + kind + '.pdf')));
    try {
      assert.equal(document.pageCount, 1);
      assert.ok(document.pageSvg(0).includes('<pattern'));
      assert.ok(document.pageCompositeUnits(0).length > 0);
      if (kind === 'state-reuse') assert.equal(document.pageCompositeUnits(0).length, 1);
      const png = document.pagePng(0, 72);
      const reopened = Document.fromBytes(document.toBytes());
      try {
        assert.deepEqual(reopened.pagePng(0, 72), png);
        assert.deepEqual(reopened.validateReferences(), []);
      } finally { reopened.close(); }
    } finally { document.close(); }
  }
});


test('import direct affine shadings through the C ABI', () => {
  for (const kind of ['axial-shear', 'radial-shear', 'axial-alpha', 'radial-clip', 'axial-form', 'radial-multiply', 'radial-rotate']) {
    const document = Document.fromPdfBytes(fs.readFileSync(fixture('affine-shading-' + kind + '.pdf')));
    try {
      assert.equal(document.pageCount, 1);
      assert.ok(document.pageSvg(0).includes('<pattern'));
      assert.ok(document.pageCompositeUnits(0).length > 0);
      const png = document.pagePng(0, 72);
      const reopened = Document.fromBytes(document.toBytes());
      try {
        assert.deepEqual(reopened.pagePng(0, 72), png);
        assert.deepEqual(reopened.validateReferences(), []);
      } finally { reopened.close(); }
    } finally { document.close(); }
  }
});

test('import general radial shadings through the C ABI', () => {
  for (const kind of ['grow', 'equal', 'shrink-both', 'affine', 'pattern-background', 'nested-pattern', 'calculator-step']) {
    const document = Document.fromPdfBytes(fs.readFileSync(fixture('general-radial-' + kind + '.pdf')));
    try {
      assert.equal(document.pageCount, 1);
      assert.ok(document.pageSvg(0).includes('<image'));
      const png = document.pagePng(0, 72);
      const reopened = Document.fromBytes(document.toBytes());
      try {
        assert.deepEqual(reopened.pagePng(0, 72), png);
        assert.deepEqual(reopened.validateReferences(), []);
      } finally { reopened.close(); }
    } finally { document.close(); }
  }
});

test('import isolated transparency groups through the C ABI', () => {
  for (const kind of ['group-alpha', 'child-screen', 'clip', 'text', 'image-alpha', 'nested', 'pattern-cell']) {
    const document = Document.fromPdfBytes(fs.readFileSync(fixture('isolated-group-' + kind + '.pdf')));
    try {
      assert.equal(document.pageCount, 1);
      assert.ok(document.pageSvg(0).includes('isolation:isolate'));
      const png = document.pagePng(0, 72);
      const reopened = Document.fromBytes(document.toBytes());
      try {
        assert.deepEqual(reopened.pagePng(0, 72), png);
        assert.deepEqual(reopened.validateReferences(), []);
      } finally { reopened.close(); }
    } finally { document.close(); }
  }
});

test('import PDF graphics-state soft masks through the C ABI', () => {
  for (const kind of ['alpha', 'luminosity-bc', 'captured-ctm', 'group', 'nested-masks', 'pattern-cell-mask', 'fill-stroke-alpha']) {
    const document = Document.fromPdfBytes(fs.readFileSync(fixture('soft-mask-' + kind + '.pdf')));
    try {
      assert.equal(document.pageCount, 1);
      assert.ok(document.pageSvg(0).includes('<image'));
      const png = document.pagePng(0, 72);
      const reopened = Document.fromBytes(document.toBytes());
      try {
        assert.deepEqual(reopened.pagePng(0, 72), png);
        assert.deepEqual(reopened.validateReferences(), []);
      } finally { reopened.close(); }
    } finally { document.close(); }
  }
});


test('masked PDF group and direct text retains semantics through the C ABI', () => {
  for (const [kind, expected] of [['text', 'A'], ['text-fill-stroke-alpha', 'A'], ['group-text', 'A'], ['nested-group-text', 'AB'], ['group-text-reuse', 'AA'], ['group-text-inner-mask', 'A'], ['group-text-transform', 'A']]) {
    const document = Document.fromPdfBytes(fs.readFileSync(fixture('soft-mask-' + kind + '.pdf')));
    try {
      assert.equal(document.pageText(0).join(''), expected);
      const objects = document.pageTextObjects(0);
      assert.ok(objects.length > 0);
      const reopened = Document.fromBytes(document.toBytes());
      try {
        assert.equal(reopened.pageText(0).join(''), expected);
        assert.deepEqual(reopened.pageTextObjects(0), objects);
        assert.deepEqual(reopened.validateReferences(), []);
      } finally { reopened.close(); }
    } finally { document.close(); }
  }
});

test('shaped RTL PDF export keeps OFD and public SVG logical text', () => {
  for (const text of ['سلام', 'AسلامB']) {
    const document = Document.create();
    try {
      const fontId = document.addEmbeddedFont('DejaVu Sans', fs.readFileSync(fixture('dejavu-sans.ttf')));
      document.addBlankPage(120, 30);
      document.addTextObject(0, text, [5, 5, 110, 20], { size: 6, font_id: fontId, shape: true });
      assert.deepEqual(document.pageText(0), [text]);
      assert.ok(document.pageSvg(0).includes(text));
      assert.ok(!document.pageSvg(0).includes('data-ofd-rtl-semantic'));
      assert.equal(document.toPdf().subarray(0, 5).toString(), '%PDF-');
      const reopened = Document.fromBytes(document.toBytes());
      try {
        assert.deepEqual(reopened.pageText(0), [text]);
        assert.equal(reopened.toPdf().subarray(0, 5).toString(), '%PDF-');
        assert.deepEqual(reopened.validateReferences(), []);
      } finally { reopened.close(); }
    } finally { document.close(); }
  }
});

test('archival preparation removes audio and video from document and page resources', () => {
  const source = Document.create();
  try {
    source.addBlankPage(30, 20);
    source.addMultimediaResource(0, 'Audio', 'MP3', Buffer.from('audio'));
    source.addPageMultimediaResource(0, 'Video', 'MP4', Buffer.from('video'));
    const resources = source.pageMultimediaResources(0);
    assert.equal(resources.length, 2);
    assert.ok(source.archiveViolations().some(item => item.rule === 'AUDIO_VIDEO'));
    const before = source.toBytes();
    const prepared = source.prepareArchive();
    try {
      assert.deepEqual(prepared.pageMultimediaResources(0), []);
      assert.ok(!prepared.archiveViolations().some(item => item.rule === 'AUDIO_VIDEO'));
      for (const item of resources) assert.ok(!prepared.entries().includes(item.path));
      assert.deepEqual(prepared.validateReferences(), []);
      const reopened = Document.fromBytes(prepared.toBytes());
      try {
        assert.deepEqual(reopened.pageMultimediaResources(0), []);
        assert.deepEqual(reopened.validateReferences(), []);
      } finally { reopened.close(); }
    } finally { prepared.close(); }
    assert.deepEqual(source.toBytes(), before);
    assert.equal(source.pageMultimediaResources(0).length, 2);
  } finally { source.close(); }
});
