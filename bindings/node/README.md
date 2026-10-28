# ofdrw-rs for Node.js

Node.js wrapper for the Rust OFD core. It uses the versioned C ABI through
[Koffi](https://koffi.dev/).

Install the release package directly from GitHub:

```sh
npm install https://github.com/lihongjie0209/ofdrw-rs/releases/download/v0.1.0/ofdrw-rs-0.1.0.tgz
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
`npm install /path/to/ofdrw-rs-0.1.0.tgz`;
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
