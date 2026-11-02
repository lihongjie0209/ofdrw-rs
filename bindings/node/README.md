# ofdrw-rs for Node.js

Node.js wrapper for the Rust OFD core. It uses the versioned C ABI through
[Koffi](https://koffi.dev/).

Check npm and GitHub Releases for version availability. Install from npm:

```sh
npm install ofdrw-rs@0.4.0
```

For development from this source tree, build the native library and install
dependencies:

```sh
cargo build -p ofdrw-ffi
cd bindings/node
npm ci
OFDRW_LIB_PATH="$(realpath ../../target/debug/libofdrw_ffi.so)" npm test
```

The Node.js test command type-checks the public TypeScript declarations and
requires at least 90% wrapper line coverage.
On macOS, use `libofdrw_ffi.dylib` in the command above. On Windows, set
`OFDRW_LIB_PATH` to the absolute path of `target/debug/ofdrw_ffi.dll` before
running `npm test`. This selects the freshly built library when a previously
staged package binary is also present under `native/`.

The CI-built npm tarball includes the native library for Linux, macOS, and
Windows on x64 and ARM64. Linux binaries are built on Ubuntu 24.04, so older
glibc distributions may need a native build. Install the tarball with
`npm install /path/to/ofdrw-rs-0.4.0.tgz`;
the library is loaded automatically. To use a custom native build, set
`OFDRW_LIB_PATH` to its absolute path. Packing from a source checkout requires
all six release libraries under `native/<platform>-<arch>/`. Stage each one
with `node scripts/native-package.js stage PLATFORM SOURCE`; staging loads the
library against the current wrapper and writes its interface and file digests.
`npm pack` checks every library against those digests.

```js
const { Document } = require('ofdrw-rs');

const document = Document.create();
try {
  console.log(document.packageInfo()); // { version: '1.2', doc_type: 'OFD' }
  document.addTextPage('你好，OFD', 210, 297);
  console.log(document.pageText(0));
  require('node:fs').writeFileSync('output.ofd', document.toBytes());
} finally {
  document.close();
}
```

`addImagePage(data, format, width?, height?)` creates an image page in a new
document. For an existing package or a selected document body, use
`appendImagePage(documentIndex, data, format, width?, height?)`. Dimensions are
in millimeters; both methods accept PNG, JPEG, GIF, BMP, single-page TIFF,
JBIG2, and JPEG2000 images.
`addPathPage(commands, boundary, pageWidth?, pageHeight?, fill?, stroke?)`
creates a page with one OFD vector path. `appendPathPage(documentIndex, …)`
adds it to a selected body. Bounds and page dimensions are in millimeters;
fill defaults to false and stroke to true.
`documentMultimediaResources()` and `pageMultimediaResources()` list image,
audio, and video resources; the matching `*MultimediaData()` methods return
their original bytes. `addPageMultimediaResource()` adds a page-local resource.
Document-level and page-level resources can be replaced or removed with
`replace*MultimediaData()` and `remove*MultimediaResource()`; removal rejects
resources still referenced by page content or actions.
`addPageObjectMediaAction()` adds a Sound or Movie click action to a page
object. `addMediaLinkAnnotation()` creates an invisible clickable region for
an Audio or Video resource. `addPageObjectSoundAction()` and `addPageObjectMovieAction()`
provide dedicated object methods; `addSoundLinkAnnotation()` and
`addMovieLinkAnnotation()` provide dedicated clickable regions.
Direct page paths and text accept `setPageObjectSolidFill/Stroke()`,
`setPageObjectGradientFill/Stroke()`, `setPageObjectGouraudFill/Stroke()`,
and `setPageObjectPatternFill/Stroke()`. Each takes a page index, object ID,
and style object matching the exported TypeScript interface. Invalid styles
leave the OFD unchanged; `pageObjectPaints()` and `pageSvg()` show the result.
`setPageImageMask()` and `setPageImageSubstitution()` set or clear a page image
object's alternative resource IDs. `setPageImageBorder()` sets a border style
or removes it when the style is omitted.

Password and SM2 certificate encryption return a new `Document` and leave the
source document open. Close each returned document after use:

```js
const { Document } = require('ofdrw-rs');
const document = Document.create();
try {
  document.addTextPage('secret text');
  const encrypted = document.encryptWithPasswords([
    { username: 'alice', password: 'secret', userType: 'Owner' },
  ]);
  try {
    const restored = encrypted.decryptWithPassword('secret', 'alice');
    try {
      console.log(restored.pageText(0));
    } finally {
      restored.close();
    }
  } finally {
    encrypted.close();
  }
} finally {
  document.close();
}
```

`encryptWithCertificates()` accepts `{ username, certificate, userType? }`
recipients with DER certificate bytes. `decryptWithSm2PrivateKey()` accepts a
raw 32-byte SM2 private key. `isEncrypted` and `encryptionInfo()` inspect a
package before decryption.
`encryptWithRecipients()` accepts a mixed array of password and certificate
recipients; add `type: 'password'` or `type: 'certificate'` to each entry.

`addVersionSnapshot(documentIndex, id, name?)` returns the created version
metadata. `extractVersionSnapshot(versionIndex)` returns an independent
`Document`; close that handle when finished.

For SES signatures, `checkSignatureDescriptionHash()` checks the description
digest, and `checkSignatureSealMatch()` compares the detached and embedded
seals. `checkSesSealSignerAuthorization()`, `checkSesSealValidity()` and
`checkSesSealCertificateValidity()` report separate authorization and time
checks. `checkSesSignatureValueWithDistid()` and
`checkSesSealSignatureWithDistid()` accept an explicit SM2 distinguishing ID.
These checks do not establish certificate trust; the chain and certificate
pool verification methods return trust results separately.

`createSesSeal(issuerPrivateKey, issuerCertificate, picture, spec, issuerDistid?)`
creates a DER SES v4/v5 seal. The spec uses camelCase fields such as `version`,
`sealId`, `authorizedSignerCertificates` (DER byte arrays), `createdAt`,
`validFrom`, `validUntil`, `pictureType`, `pictureWidthMm`, and `pictureHeightMm`.
`document.addSesSignature(privateKey, certificate, seal, dateTime, options?)`
signs a document with that seal and returns the signature index. Pass
`{pageIndex, boundary: [x, y, width, height]}` to place its stamp on a page.
Both calls accept optional SM2 distinguishing IDs (`issuerDistid` for the seal;
`signatureDistid` and `sealDistid` in signature options). The default is
`1234567812345678`.
For a raw SM2 signature, `addDigitalSignature(privateKey, dateTime,
signatureDistid?)` accepts the same optional ID. Pass that ID to
`checkDigitalSignatureWithCertificate(signature, certificate,
signatureDistid?)` when verifying.

Annotations can contain path, text, image, and composite graphics. To extend an existing
annotation, call `appendAnnotationObjects(page, annotationId, objects)`. It
returns the new object IDs in painting order; the objects use the same JSON
shape as `addMixedAnnotation` and `addMixedStampAnnotation`.
`setAnnotationPathData(page, annotationId, objectId, commands)` changes one
path object's drawing commands after creation.
`setAnnotationObjectName(page, annotationId, objectId, name?)` and
`setAnnotationObjectBlendMode(page, annotationId, objectId, blendMode?)`
set or clear direct appearance attributes. `annotationObjects()` reports both.
`setAnnotationObjectDrawParam(page, annotationId, objectId, drawParam?)`
uses a DrawParam visible to the page, or clears the reference when omitted.
`setAnnotationPathStrokeStyle(page, annotationId, objectId, style)` changes its
direct line width, cap, join, and dash settings using the page path style shape.
`setAnnotationTextCode(page, annotationId, objectId, codeIndex, text)` changes
one unshaped text run and remeasures horizontal advances when needed.
`appendAnnotationTextCode(page, annotationId, objectId, text, layout?)`
adds a positioned run with optional X/Y coordinates and per-character deltas.
`setAnnotationTextCodeLayout(page, annotationId, objectId, codeIndex, layout)`
replaces direct positioning; omitted fields restore inherited positions.
`setAnnotationTextTypography(page, annotationId, objectId, style)` replaces
size and optional direct typography attributes.
`setAnnotationTextPaintFlags(page, annotationId, objectId, options?)` sets
direct fill and stroke switches; omitted values restore OFD defaults.
`annotationObjects()` reports effective directions, weight, and italic styling,
plus the optional direct horizontal scale, for text objects.
It reports effective `fill` and `stroke` switches for text and path objects.
It also reports positioned `codes` and validated `cg_transforms` for text objects.
Both annotation text methods preserve Unicode text containing embedded NUL.
`removeAnnotationTextCode(page, annotationId, objectId, codeIndex)` removes
one run and preserves the next run's inherited position.
`setAnnotationTextFont(page, annotationId, objectId, fontId)` selects a font
resource visible on that page. Shaped text retains its original font.
`setAnnotationGraphicResource(page, annotationId, objectId, resourceId)` changes
an annotation image or composite object's resource to one visible on the page.

`addFlowedText(documentIndex, text, options?)` and
`addFlowedSpans(documentIndex, spans, options?)` append paginated text and return
the new page indexes. Options use the same snake_case field names as the Rust
flow API, including `page_size`, `margins`, `font_size`, `line_height`, `align`,
`paragraph_gap`, and `keep_paragraphs_together`. Styled spans can set font,
color, weight, underline, and line break behavior. For example:

```js
document.addFlowedText(0, 'First line\nSecond line', {
  page_size: [100, 80], margins: [10, 10, 10, 10], font_size: 5,
});
```

Use `addEmbeddedFont(name, bytes)` for a new document or
`addEmbeddedFontToDocument(documentIndex, name, bytes)` for an existing body.
Both return a font ID for `font_id` in flow options or text objects. With an
embedded OpenType font, `shape: true` writes positioned glyph mappings.
For a positioned run, `addTextObject(page, text, boundary, options)` accepts
`size`, `fill_color`, `font_id`, `shape`, and `language`. Shaping needs an
embedded font ID; `language` is available when `shape` is `true`:

```js
const fontId = document.addEmbeddedFont('DejaVu Serif', fontBytes);
document.addBlankPage(80, 50);
document.addTextObject(0, 'office', [5, 5, 50, 10], {
  size: 6, font_id: fontId, shape: true, language: 'en', fill_color: [0, 0, 255],
});
```

`addTextPageWithFont(text, fontId, width?, height?)` and
`appendTextPageWithFont(documentIndex, text, fontId, width?, height?)` write a
whole page with that font; these methods accept embedded NUL characters in
the text through the JSON ABI.
`fonts()` and `pageFonts(page)` list available fonts; `fontData(index)` and
`pageFontData(page, index)` return their original embedded bytes.

`addTable(documentIndex, rows, options)` paginates a rectangular array of
string cells and returns the new page indexes. `column_widths` is required;
`repeat_header` repeats the first row after a page break.
`addTableCells(documentIndex, rowCount, cells, options)` accepts positioned
cells with row or column spans, per-cell colors and text styles, and optional
image or nested blocks through the same Rust layout engine. Set `row_height: 0`
when a row should grow to fit nested content. Table images use
`{ data_base64: imageBytes.toString('base64'), format: 'png', width, height }`.

`addPageLayer()`, `setPageLayerType()`, and `removePageLayer()` manage direct
background, body, and foreground layers. `movePageObjectToLayer()` moves an
object between layers; `movePageObject()` places it before another object or
at the end of its container. `groupPageObjects()` and `ungroupPageObjects()`
manage `PageBlock` containers.
`setPageObjectBoundary()`, `setPageObjectCtm()`, `setPageObjectVisible()`,
`setPageObjectAlpha()`, `setPageObjectName()`, `setPageObjectBlendMode()`, and
`setPageObjectDrawParam()` edit direct object geometry and display attributes.
Pass `null` to clear an optional transform, name, blend mode, or draw parameter.
`setPagePathData()` replaces abbreviated path commands. `setPagePathStrokeStyle()`
sets direct line width, cap, join, and dash geometry; `setPagePathDashSequence()`
updates or clears just the dash sequence and offset.
`setPageTextCode()` and `appendPageTextCode()` accept Unicode text including NUL.
`setPageTextCodeLayout()` and `removePageTextCode()` edit individual unshaped
runs. `setPageTextTypography()`, `setPageTextFont()`, and
`setPageTextPaintFlags()` edit direct text styles.

`addDocumentColorSpace()` and `addPageColorSpace()` create GRAY, RGB, or CMYK
resources with optional bit depth, ICC profile path, or palette.
`setDocumentColorSpace()` and `setPageColorSpace()` update an existing resource
in its original scope while keeping its ID.
`removeDocumentColorSpace()` and `removePageColorSpace()` remove unreferenced
definitions; linked defaults and paint references must be cleared first.
`documentColorSpaces()` and `pageColorSpaces()` list their definitions;
`setDocumentDefaultColorSpace()` sets or clears a document-visible default.
`addDocumentDrawParam(documentIndex, style)` creates a reusable paint and stroke
style and returns an ID for `setPageObjectDrawParam()` or a layer draw parameter.
`addPageDrawParam(page, style)` creates a style scoped to one page; it may inherit
from a document style or another style visible on that page.
`documentDrawParams(documentIndex)` and `pageDrawParams(page)` list visible
definitions with their source resource paths and original XML.
`setDocumentDrawParam(documentIndex, id, style)` and
`setPageDrawParam(page, id, style)` replace a definition in its own scope while
preserving the ID used by page objects and layers.
`removeDocumentDrawParam(documentIndex, id)` and `removePageDrawParam(page, id)`
delete unreferenced definitions while keeping allocated IDs reserved.
Solid RGB paints, dash geometry, and the Rust core's gradient, Gouraud, and
pattern style JSON forms are accepted.

`pageSvg(page, { preferImageSubstitution: true })`,
`pagePng(page, dpi, { preferImageSubstitution: true })`, and
`toPdf({ preferImageSubstitution: true })` use replacement images declared by
composite graphic units. `setPageCompositeUnitImages(page, unitId,
{ thumbnailId, substitutionId })` sets or clears the two optional references.
The call validates both image IDs before changing the shared resource.

The current API covers document creation/opening, OFD/PDF byte import,
document metadata fields, keywords, and custom data,
document permissions, viewer preferences, and document/page areas,
document and page actions for URI, page, bookmark, and attachment targets,
custom tag XML payloads and document extensions, including removal of their
index records with `removeCustomTag()` and `removeExtension()` (referenced
package files remain available). `updateExtension()` replaces an existing
extension record in place,
while `updateCustomTag()` edits TypeID, namespace, and schema without changing
the tag's XML payload. `replaceCustomTagData()` writes a new XML payload and
updates its index path together; the former package entry remains available.
The API also covers hierarchical document outlines,
version snapshot listing, creation, and extraction,
page/text/path/image inspection, positioned keyword search, template and
composite graphic inspection, object clip/paint/stroke inspection, rectangle,
path, and mixed path/text clip edits, and direct
page-object URI, page, bookmark, and attachment actions with clickable rectangle regions,
and URI, page, bookmark, and attachment link annotations,
annotation creator, subtype, date, remark, parameters, appearance boundary,
visibility, print, and standard flag edits,
single-page text/image watermarks and Stamp annotations, and atomic
all-page text/image watermarks,
SVG/PNG/PDF/OFD export, adding text and blank
pages, `save()` to write an OFD file, creating positioned text/path/image and
composite text/path/image objects,
paginating plain and styled text and fixed-grid tables,
reusing composite units, promoting and applying page templates, removing
unused document template definitions, duplicating pages and direct objects,
removing direct objects, package-entry byte access,
page layer, object ordering, geometry, display, color-space, and reusable draw-parameter edits,
embedded attachments, and
reference validation. Template placement accepts `Background`, `Body`, or
`Foreground` as `zOrder`. It also reads signature metadata and value bytes, adds
raw SM2 signatures, verifies a signature with a supplied certificate, and
checks signer certificate validity and direct SM2 trust anchors at a
caller-supplied time, plus protected file digests.
`checkSignatureDirectTrustAnchor()` checks a supplied signer certificate
against a supplied trust anchor. For SES signatures,
`checkSesSealIssuerOrderedSm2Chain()` checks the embedded seal issuer against
an ordered list of intermediate certificates and a supplied trust anchor.
The signature API checks both the SES document
signature value and the embedded seal signature. A matching signature value does not prove that
the protected files remain unchanged; call `checkSignatureReferences()` too.
The basic checks do not establish certificate trust or validity.
`verifySignatureWithDirectTrustAnchor(signature, signer, anchor, time, options)`
returns the combined digest, signature, certificate validity, and direct trust
report. `verifySignatureWithOrderedSm2Chain(signature, signer, intermediates,
anchor, time, options)` checks an ordered signer chain, while
`verifySignatureWithCertificatePool(signature, signer, candidates, anchor,
time, options)` searches unordered candidates. Certificate arrays contain DER
byte buffers. `options` accepts separate `certificateDistid` and
`signatureDistid` values. Each report retains the individual check results.
`verifySignatureWithOrderedSm2Chains(signature, options)` checks an ordered
signer chain and optional SES seal issuer chain against caller supplied trust
anchors. `verifySignatureWithCertificatePools(signature, options)` searches
unordered intermediate candidates. `options` takes `signerCertificate`,
`signerIntermediates`, `signerAnchor`, `atUnixSeconds`, and optionally
`sealIntermediates`, `sealAnchor`, and separate signer/seal SM2 IDs. The report
keeps signature value, protected file digests, certificate chain, and seal
issuer results separate. These bounded checks do not perform revocation or
certificate policy validation. Calls are synchronous. `Document.close()` releases the
native handle and may be called more than once. Returned byte buffers are
copied into Node.js memory before the Rust allocation is released.
`addAttachmentToDocument(documentIndex, name, data)` adds a file to an existing
document body. `renameAttachment()`, `replaceAttachmentData()`,
`setAttachmentMetadata()`, and `removeAttachment()` use the zero-based global
attachment index returned by `attachments()`. Metadata can set `format`,
`creation_date`, `mod_date`, `visible`, and `usage`; omitted optional fields are
cleared, and omitted `visible` defaults to true. `addAttachment()` is a
convenience method for a newly created single-body package.
`integrityManifest()`, `checkIntegrityManifest()`, and
`verifyIntegrityManifestSm2()` expose the optional root `OFDEntries.xml`
manifest, file-list comparison, and its SM2 signature result separately.
`protectIntegritySm2()` creates a signed manifest from a matching SM2 private
key and DER certificate. `verifyIntegrityManifestSm2WithOrderedChain()`
checks an ordered SM2 chain against a caller-supplied trust anchor;
`verifyIntegrityManifestSm2WithCertificatePool()` finds a path through
unordered intermediate candidates. Both take DER certificate buffers, a
caller-supplied Unix timestamp, and optional `certificateDistid` and
`signatureDistid` strings. The basic signature check does not establish
certificate trust. Chain checks do not perform revocation or certificate
policy validation.
Direct `setEntry()` changes to a signed package can invalidate signature
digests; check signatures again after editing such entries.

For multi-document packages, `documents()` lists document bodies.
`appendDocumentBody()`, `moveDocumentBody()`, `removeDocumentBody()`,
`extractDocumentBody()`, `appendDocumentBodyFrom()`, and
`appendDocumentBodiesFrom()` manage them. `appendTextPage()` and
`appendBlankPage()` target a body by zero-based index. `insertTextPage()` and
`insertBlankPage()` take a body-local insertion position and return the new
global page index. `movePage()` and `removePage()` use global page indexes;
`duplicatePageAt()` takes a global source index and a body-local destination
position.

`bookmarks()` and `annotations()` inspect navigation and annotation metadata.
`annotationObjects(page, annotationId)` reads direct appearance objects in
painting order, including geometry, text runs, path commands, and resource IDs.
`updateAnnotationObject()` edits boundary, CTM, visibility, and alpha together;
pass `ctm: null` to clear a transform. `removeAnnotationObject()` removes one
appearance object by its ID.
`addBookmark()` accepts a destination such as `{ page_id, kind: 'Fit' }`.
`addPathAnnotation()`, `addMultiPathAnnotation()`, `addMixedAnnotation()`,
`addMixedStampAnnotation()`,
`addHighlightAnnotation()`, `addTextNoteAnnotation()`,
`addShapeAnnotation()`, `addLineAnnotation()`, `addUriLinkAnnotation()`,
and `addBookmarkLinkAnnotation()` create common annotation types;
`addMultimediaResource()` adds a page-visible image resource for a mixed
annotation's image object.
`removeAnnotation()` and `removeBookmark()` delete them. Call
`validateReferences()` after editing links. `archiveViolations()` reports the
currently supported OFD-A rules, and `prepareArchive()` returns a separate
package with supported archival normalizations applied. Check the returned
package again because complete OFD-A conversion is not yet implemented.

## 混合流布局（0.4.0 起）

```js
const document = Document.create();
try {
  const result = document.addFlowBlocks(0, [
    {type: 'paragraph', text: 'First paragraph'},
    {type: 'spacer', height: 4},
    {type: 'paragraph', spans: [{text: 'Second paragraph', bold: true}]},
  ], {block_gap: 2});
  document.addFlowBlocks(0, [{type: 'paragraph', text: 'Continued'}],
    {start: result.next});
  document.save('mixed.ofd');
} finally { document.close(); }
```

支持图片、跨页表格和 `page_break`，并提供 `FlowBlock`、`FlowDocumentOptions`
和 `FlowLayoutResult` TypeScript 类型。图片使用 `data_base64`。
详见[混合流布局](../../docs/FEATURES.md#混合流布局)。

## 盒模型与浮动行（0.4.0 起）

```js
const document = Document.create();
try {
  const result = document.addBoxLayout(0, [
    {type: 'box', size: [40, 10], padding: [2, 2, 2, 2],
      border: [0.2, 0.2, 0.2, 0.2], float: 'left', clear: 'none'},
    {type: 'box', size: [40, 10], float: 'right', clear: 'none'},
  ]);
  const placed = result.placements[0];
  document.addTextObject(placed.page_index, 'Hello', placed.content_box);
  document.save('boxes.ofd');
} finally { document.close(); }
```

提供 `LayoutBox`、`BoxLayoutItem`、`BoxLayoutOptions`、`BoxPlacement` 和
`BoxLayoutResult` TypeScript 类型。`next` 可传给下次调用的 `start`。
自动测量文本盒与嵌套混合容器见下文的布局接口。
详见[盒模型说明](../../docs/FEATURES.md#盒模型与浮动分段)。

## 文本自动测量（0.4.0 起）

```js
const document = Document.create();
try {
  const spans = [{text: 'First line\nSecond line'}];
  const metrics = document.measureFlowedSpans(0, spans, 40, {font_size: 5, line_height: 6});
  console.log(metrics.width, metrics.height, metrics.lines);
  document.addBoxLayout(0, [{type: 'box', size: [40, metrics.height],
    padding: [2, 2, 2, 2], border: [0.2, 0.2, 0.2, 0.2], integrity: true}]);
} finally { document.close(); }
```

提供 `FlowTextMetrics` 和 `FlowLineMetrics` 类型。测量不增加页面或资源，
页面几何和 `start` 被忽略；逐行位置和逻辑基线相对段落原点。
测得高度可用于盒模型；按行拆分的文本盒与一般嵌套容器见下文的布局接口。

### Intrinsic image sizes (since 0.4.0)

Flow images, table-cell images and image blocks accept `intrinsic_size: true`
(`IntrinsicSize: true` in Go). Omit width/height or leave them zero: pixel
sizes are converted at 5 pixels/mm, matching Java OFDRW, regardless of embedded
DPI metadata. `fit_within: [width, height]` (`FitWithin: &[2]float64{width, height}`
in Go) optionally shrinks the image to millimeter bounds while preserving
aspect ratio. It never enlarges smaller images. Nonzero explicit dimensions
cannot be combined with intrinsic sizing. Without intrinsic sizing, both
explicit dimensions remain required. These options share the same native
validation in C and all bindings, available since 0.4.0.

```javascript
const fs = require('node:fs');
const { Document } = require('ofdrw-rs');
const doc = Document.create();
try {
  doc.addFlowBlocks(0, [{
    type: 'image', data_base64: fs.readFileSync('picture.png').toString('base64'),
    format: 'PNG', intrinsic_size: true, fit_within: [40, 30],
  }]);
  fs.writeFileSync('picture.ofd', doc.toBytes());
} finally { doc.close(); }
```

### Automatic text boxes (since 0.4.0)

```javascript
const fs = require('node:fs');
const { Document } = require('ofdrw-rs');
const doc = Document.create();
try {
  const result = doc.addTextBoxes(0, [{
    layout: {size: [40, 0], padding: [2, 2, 2, 2], border: [0.2, 0.2, 0.2, 0.2]},
    spans: [{text: 'Text wraps and grows this decorated box.'}],
    style: {font_size: 4, line_height: 5},
  }]);
  fs.writeFileSync('text-box.ofd', doc.toBytes());
} finally { doc.close(); }
```

Content height replaces `layout.size[1]` / `Layout.Size[1]` automatically.
Width is clamped before wrapping, and complete lines move to the next page.
Typography can enable shaped text or paragraph keep rules; page geometry and
continuation come from the shared box options. The result contains all
fragment rectangles and `next` (`Next` in Go), reusable as `start` (`Start`).
General nested child containers use the mixed container API below. This API is available
since 0.4.0.

### Nested mixed containers (since 0.4.0)

```javascript
const fs = require('node:fs');
const { Document } = require('ofdrw-rs');
const doc = Document.create();
try {
  const result = doc.addFlowContainers(0, [{
    layout: {size: [40, 0], padding: [1, 1, 1, 1]}, gap: 1,
    children: [
      {type: 'paragraph', spans: [{text: 'Heading'}], style: {font_size: 4, line_height: 5}},
      {type: 'container', container: {
        layout: {size: [30, 0], border: [0.2, 0.2, 0.2, 0.2]},
        children: [{type: 'paragraph', spans: [{text: 'Nested text'}],
                    style: {font_size: 2, line_height: 3}}],
      }},
    ],
  }]);
  fs.writeFileSync('containers.ofd', doc.toBytes());
} finally { doc.close(); }
```

Image children use `data_base64`; font IDs in nested styles are strings.
TypeScript exports `FlowContainer` and the `FlowContainerChild` union.

Each container has `layout`, optional `gap`, and `children`. Supported child
types are paragraph, image, container, spacer, table, line_break and page_area_fill. A table child has a
`table` grid and optional `table_style` (`font_size`, `line_height`, `font_id`,
`repeat_header`). Images accept explicit dimensions or intrinsic sizing and
fit bounds. Spacer height defaults to zero. Returned root fragments and
`next` / `Next` support continuation with shared `start` / `Start` options.

Containers support floating rows at every nesting level. Adjacent nested
boxes share rows according to float, clear and width rules, with independent
page continuation per column. Paragraphs, images, tables and spacers occupy
full rows; `gap` separates rows. Repeated table headers stay with complete
body-row groups, and header/body pairs that cannot fit a full page fail.
Fragment heights include repeated headers and the space needed to align
continuing floating columns.
The whole call rolls back on invalid children. Limits are eight container
levels and 10,000 container/child entries. This API is available since 0.4.0.


`line_break` ends the current nonempty floating row; leading, repeated and
trailing empty breaks are ignored. `page_area_fill` consumes the current page's
remaining content area. If that area is already full, it starts and fills the
next page, so consecutive fills preserve explicit blank pages. A trailing fill
does not append another page. Fills reset row gaps and affect only their own
floating column. Both controls accept only `type`; a complete (`integrity`)
container that would span pages fails atomically.

```json
{"children": [{"type": "line_break"}, {"type": "page_area_fill"}]}
```


### Dashed box borders (since 0.4.0)

`border_dash` (`BorderDash` in Go) uses Java `Div.setBorderDash` arguments:
`[3]` means dash/gap `[3, 3]`; `[3, 2]` means dash/gap `[3, 2]`;
`[1, 3, 2]` sets offset 1 followed by dash/gap `[3, 2]`. Longer even
sequences contain dash/gap pairs, and longer odd sequences start with the
offset. Null or an empty sequence selects a solid border. Lengths must be
positive and finite, with at most 256 lengths; offsets below 0.000001 are
omitted, matching Java. Invalid patterns fail atomically, including placeholders.

This field works in measured boxes, automatic text boxes and nested mixed
containers. Equal-width borders use one closed stroked path; different edge
widths use separate paths with horizontal edges owning the corners. Page cuts
preserve the existing top/bottom border fragmentation rules. Dash phase starts
at each emitted path; allocation, text wrapping and continuation are unchanged.

```json
{"size": [40, 10], "border": [0.5, 0.5, 0.5, 0.5], "border_dash": [1, 3, 2]}
```


### Box painting layers (since 0.4.0)

Set `layout.layer` (`Layout.Layer` in Go) to `background`, `body` or
`foreground`. Measured box items accept the same `layer` field directly.
Decoration, ordinary/shaped text, underline paths, images and table children
use the selected layer across page fragments. A nested container without a
layer inherits its parent; an explicit layer overrides it for that subtree.
Roots default to Body. Painting follows OFD Background → Body → Foreground
order independently of the order in which boxes were submitted.

Existing direct page layers of the chosen type are reused; another layer is
created when needed. Existing empty Body layers on blank pages are retained.
Layer selection does not change measured dimensions, pagination or returned
placements. Later ordinary drawing calls continue to use Body. Invalid layer
values fail atomically. The same rules apply to static and advanced container
pagination and to existing pages with nonzero canvas origins.

```js
document.addBoxLayout(0, [{type: 'box', size: [40, 10],
  background_color: [240, 240, 240], layer: 'background'}]);
```

### Fixed page containers (since 0.4.0)

Available since 0.4.0; use bindings with their matching native library.

```js
const doc = Document.create();
try {
  doc.addBlankPage(80, 60);
  const placements = doc.addFixedContainers(0, [{
    position: [10, 20],
    container: {layout: {size: [40, 0]}, children: [
      {type: 'paragraph', spans: [{text: 'Hello'}]},
    ]},
  }]);
  fs.writeFileSync('fixed.ofd', doc.toBytes());
} finally { doc.close(); }
```

TypeScript exports `FixedContainer`; the method returns `BoxPlacement[]`. Positions
are absolute OFD millimeters on an existing global page. Zero height measures content;
positive heights control allocation and decoration while children may overflow.
The call never adds pages. Nested text/images/tables, offsets, layers and placeholders
follow flow-container rules. Page-area fillers are ignored; invalid later content
rolls back the entire call. Images use `data_base64` strings.

### Inline placeholders (since 0.4.0)

`FlowSpan` accepts `placeholder_width` in millimeters with empty `text` and
`font_size` as height. This reserves an indivisible inline rectangle without
painting objects or font resources; width zero is allowed.

```js
const metrics = doc.measureFlowedSpans(0, [
  {text: 'Label:'},
  {text: '', placeholder_width: 8, font_size: 4},
  {text: 'value'},
], 50);
```

The same spans work in styled flow, text boxes, flow blocks and fixed/flow
containers. Geometry participates in wrapping and line height while text omits
the placeholders. A leading placeholder absorbs a configured first-line indent
once. Character-count reservations use `count * (font_size + letter_spacing)`.
Use matching bindings and native library (0.4.0 or later).

### Protected content ranges (unreleased source API)

Use matching source bindings and native library; this addition is outside 0.4.0.
`BoxContentRanges` protects indivisible intervals while placing/decorating boxes:

```js
const metrics = doc.measureFlowedSpans(0, [{text: 'A\nB\nC\nD'}], 8,
  {font_size: 2, line_height: 3});
const policies = [{source_index: 0, ranges: metrics.lines.map(line => [line.y, line.y+line.height])}];
const result = doc.addBoxLayoutWithContentRanges(0,
  [{type: 'box', size: [8, metrics.height]}], policies,
  {page_size: [20,14], margins: [2,2,2,2]});
```

Intervals use original content coordinates and must be ordered, positive,
disjoint and inside the box. Empty policies retain geometric cuts. The result
uses ordinary box-layout geometry and continuation; child drawing remains with
the caller. Invalid policies roll back the entire call.

## Device hairlines (unreleased; after 0.4.0)

Pass `{line_width: 0}` to `addPathObject`, `addCompositePathObject`, or
`addPathAnnotation`. Existing paths accept
`doc.setPagePathStrokeStyle(0, objectId, {line_width: 0})`.
PNG retains one device pixel across DPI and transforms; PDF preserves a
native zero-width stroke. Use the matching unreleased native library.

## Vector canvas batches (unreleased; after 0.4.0)

Use a matching native library built from the same unreleased source.

```js
const {Document} = require('ofdrw-rs');
const fs = require('node:fs');
const document = Document.create();
try {
  document.addBlankPage(40, 30);
  const ids = document.drawCanvas(0, [0, 0, 40, 30], [
    {type: 'set_stroke_color', color: [0, 80, 200]},
    {type: 'rect', rect: [2, 3, 12, 8]},
    {type: 'stroke'},
  ]);
  fs.writeFileSync('canvas.ofd', document.toBytes());
} finally {document.close();}
```

`CanvasCommand` is a TypeScript discriminated union. The batch commits
atomically and returns string object IDs. Coordinates are millimeters and
angles are degrees. Unknown fields and nonfinite numbers are rejected.
A null or omitted `set_global_alpha.alpha` clears global opacity.
See the [command reference](../../docs/FEATURES.md#有状态向量画布) for vector operations and remaining text limitations.

### Canvas images (unreleased)

Use Buffer or Uint8Array in `data`, or use `data_base64`, never both. Offset
Uint8Array views are supported without changing the input. `draw_image_at`
uses natural pixel dimensions divided by `pixels_per_mm` (default 3.78).
`draw_image_region` converts source millimeters to pixels by truncation and
pads portions outside the source with transparency, matching Java. Negative
target dimensions mirror images; zero dimensions collapse them.

```js
const {readFileSync} = require('node:fs');
const {Document} = require('ofdrw-rs');
const image = readFileSync('picture.png');
const document = Document.create();
try {
  document.addBlankPage(40, 30);
  document.drawCanvas(0, [0, 0, 40, 30], [
    {type: 'draw_image', data: image, format: 'png', rect: [2, 2, 12, 8]},
    {type: 'draw_image_region', data: image, format: 'png',
      source: [0, 0, 1, 1], rect: [20, 2, 8, 8], pixels_per_mm: 2},
  ]);
  document.save('canvas-images.ofd');
} finally {document.close();}
```

### Canvas text (unreleased)

`set_font` accepts CanvasFont settings for size, tracking, weight, italic,
alignment and quarter-turn reading/character directions. Defaults are SimSun,
1 mm, weight 400 and start alignment. `measureCanvasText` returns logical width,
first glyph offset and signed advances without changing the document. Metrics
exclude kerning and shaping; embed a font for reproducible measurements.

```js
const {Document} = require('ofdrw-rs');
const document = Document.create();
const font = {size: 4, align: 'center'};
try {
  document.addBlankPage(40, 30);
  const metrics = document.measureCanvasText(0, 'Canvas', font);
  document.drawCanvas(0, [0, 0, 40, 30], [
    {type: 'set_font', font},
    {type: 'fill_text', text: 'Canvas', point: [20, 10]},
  ]);
  document.save('canvas-text.ofd');
  console.log(metrics.width);
} finally {document.close();}
```
