# 功能与 API 参考

[返回项目首页](../README.md) · [各语言安装和使用](../README.md#python)

本文按功能说明当前实现、语言接口和具体限制。正文保留英文 API 说明，便于对照代码；
当前实现范围仍是 Java OFDRW 的子集。

## 目录

- [包结构与基础读写](#包结构与基础读写)
- [多文档结构与导入](#多文档结构与导入)
- [附件](#附件)
- [批注与图章](#批注与图章)
- [多媒体资源与动作](#多媒体资源与动作)
- [页面对象与文字编辑](#页面对象与文字编辑)
- [文本流式布局](#文本流式布局)
- [表格与嵌套布局](#表格与嵌套布局)
- [页面顺序与对象分组](#页面顺序与对象分组)
- [SVG 预览](#svg-预览)
- [SVG 字体与绘制效果](#svg-字体与绘制效果)
- [文本提取与检索](#文本提取与检索)
- [PNG 渲染](#png-渲染)
- [PDF 导出](#pdf-导出)
- [PDF 导入与兼容范围](#pdf-导入与兼容范围)
- [模板与图层](#模板与图层)
- [文档版本快照](#文档版本快照)
- [书签与大纲](#书签与大纲)
- [签名与电子印章](#签名与电子印章)
- [引用和结构验证](#引用和结构验证)
- [OFD-A 检查与转换](#ofd-a-检查与转换)
- [文档加密](#文档加密)
- [Build and test](#build-and-test)
- [Python 绑定](#python-绑定)
- [C ABI 与句柄生命周期](#c-abi-与句柄生命周期)
- [Go 绑定](#go-绑定)
- [Node.js 绑定](#nodejs-绑定)
- [持续集成](#持续集成)
- [Current API conventions](#current-api-conventions)

## 包结构与基础读写

Rust implementation of the first OFD package and document-reading layer, with
one C ABI used by Python, Go, and Node.js. This is an **early subset**, not yet a full
replacement for the Java modules. Package XML supports UTF-8 and
UTF-16LE/UTF-16BE with or without a byte-order mark. UTF-16 XML is decoded for
structured reads and normalized to UTF-8 if edited; `entry()` and payload
data methods return original bytes. Signature reference checks hash original
package bytes, including UTF-16 XML. Current operations include package
open/save, entry access, page count, positioned text/path/image object extraction,
raw package entry insertion, replacement, and deletion,
document default page area read/write (`CommonData/PageArea`, inherited by
pages without an explicit or template area),
with malformed or duplicated page boxes rejected during page reading,
optional root `OFDEntries.xml` manifest reading and structural comparison of
listed files, signed-value path (including XML comments or CDATA in
`SignedValueLoc`), and extra package files, plus verification
of Java ofdrw's SM2/SM3 `OFDEntries.xml` signature using its embedded
certificate. The integrity API reports the file-list and signature results
separately; certificate trust and validity are left to the caller,
and `protect_integrity_sm2()` creates a signed integrity manifest from a
matching SM2 private key and X.509 certificate. Python exposes the same method;
Go uses `ProtectIntegritySM2`, while C uses `ofd_protect_integrity_sm2`.

`verify_integrity_manifest_sm2_with_ordered_chain()` also checks the embedded
signer certificate against a caller-supplied, ordered SM2 chain and trust
anchor at a caller-supplied time. Python exposes the same name, Go uses
`VerifyIntegrityManifestSM2WithOrderedChain`, and C accepts JSON options.
The report keeps certificate-chain, signature, and package-file results
separate. This bounded check does not perform path building, revocation,
certificate policy, name constraints, or trusted time validation.

`verify_integrity_manifest_sm2_with_certificate_pool()` adds bounded path
building from up to 16 unordered intermediate candidates, with at most eight
intermediates per path; Python uses the same name, Go uses
`VerifyIntegrityManifestSM2WithCertificatePool`, and C accepts the same JSON
fields as the ordered-chain call. It still leaves revocation, policy, name
constraints, and trusted time to the caller.
structured document permission read/write (`Document/Permissions`, including
print limits and validity dates); boolean values split by XML comments or
CDATA are parsed as one value,
initial viewer preference read/write (`Document/VPreferences`, including page
mode, layout, window controls, and zoom),
direct document action read/append/remove (`Document/Actions`, including URI,
page destination, bookmark, and attachment targets),
direct page action read/append/remove (`Page/Actions`, with `PO` page-open events
and the same target types),
custom tag index read/write (`Document/CustomTags`) with XML payload access,
optional namespaces and schema references,
in-place custom tag metadata updates that retain the XML payload and FileLoc,
custom tag XML replacement that updates FileLoc and payload together while
retaining the former package entry,
document extension index read/write (`Document/Extensions`) with application
metadata, simple properties, external data references, and complete `Data` XML,
in-place extension record updates with the same validation as insertion,
global-index removal of custom tag and extension records while retaining their
referenced package files for other document bodies and archived versions,
with unique case-insensitive matching for existing index, payload, schema, and
`ExtendData` package references. Ambiguous matches fail before an edit even
when one entry matches the reference exactly. New index and custom-tag payload
paths skip case-insensitive collisions. These index links and referenced file
paths resolve text split by XML comments or CDATA; replacing custom-tag data
rewrites the complete `FileLoc` value,
structured writes reject new ZIP entry names that collide with existing entries
or with another new entry after ASCII case folding; raw `set_entry()` remains
available for inspecting and repairing malformed packages,
image resource byte access, embedded attachment read/write,
creation of fixed-layout text, PNG/JPEG/GIF/BMP/TIFF/JBIG2/JPEG2000 image, and vector path pages, and
appending text pages to existing documents while preserving unrelated XML.

Rust `from_bytes()` / `to_bytes()`, Python `Document.from_bytes()` /
`to_bytes()`, Go `FromBytes()` / `ToBytes()`, and C `ofd_from_bytes()` /
`ofd_to_bytes()` open and serialize complete OFD ZIP packages in memory.

## 多文档结构与导入

`append_document_body()` (Rust/Python), `AppendDocumentBody()` (Go), and
`ofd_append_document_body()` (C) add an empty `DocBody` with an independent
document root, public resources, and ID space. The returned zero-based index
can be passed to indexed page, resource, and metadata writers. Existing entries
are preserved; signed packages reject the addition. When adding a body to a
newly created package, use the indexed `append_*` page methods afterward.

Document roots are read from direct `DocBody/DocRoot` children. OFD-looking
sections nested under foreign-namespace extension elements do not replace the
document's real root, common data, or page list.
Multiple direct `DocRoot`, `CommonData`, or `Pages` elements are rejected by
the corresponding document and page readers instead of choosing one silently.
Reference validation skips extension-wrapped page, template, and annotation
graphics, color references, clip geometry, and navigation targets. Such targets
also do not block page deletion.

`move_document_body()` (Rust/Python), `MoveDocumentBody()` (Go), and
`ofd_move_document_body()` (C) reorder `DocBody` elements in `OFD.xml` while
preserving their paths and contents. Page and document indexes follow the new
order; signed packages reject the change.
`remove_document_body()` (Rust/Python), `RemoveDocumentBody()` (Go), and
`ofd_remove_document_body()` (C) detach a body from `OFD.xml` while retaining
its package entries. At least one body must remain, and signed packages reject
the removal.

`extract_document_body()` (Rust/Python), `ExtractDocumentBody()` (Go), and
`ofd_extract_document_body()` (C) return a separate package with one active
body. The source remains unchanged and all package entries are retained to
preserve shared references. A single-body signed package can be copied;
extracting from a multi-body signed package is rejected.
`append_document_bodies_from()` (Rust/Python),
`AppendDocumentBodiesFrom()` (Go), and
`ofd_append_document_bodies_from_json()` (C) import every document body from an
unsigned OFD into separate bodies. Source files move under a unique
`Imported_N/` directory; relative links continue to work, recognized
package-root absolute paths in XML and `DocInfo/Cover` are rebased, and each
imported `DocID` is regenerated. The root path, cover path, and ID may contain
XML text split by comments or CDATA; importing replaces each entire value.

XML package entries with uppercase or mixed-case `.xml` suffixes receive the
same path rebasing and version-snapshot rewriting as lowercase XML entries.
Signed packages and archived versions are
rejected, as are additional `DocBody` children that need package-root path
rewriting. Unknown XML values remain unchanged; an unclassified absolute path
that points to a source entry is rejected to avoid a broken imported link.

Foreign-namespace elements and attributes with OFD-like path names follow
this rule too; they are never silently rewritten as OFD references.
This check joins text split by XML comments or CDATA before classifying paths.
The operation leaves both packages unchanged on error. This is a
document-body import; pages are not combined into one body.
`append_document_body_from()` (Rust/Python), `AppendDocumentBodyFrom()` (Go),
and `ofd_append_document_body_from()` (C) import one selected source body using
the same relocation and signature checks. To retain possible cross-body
resource references, the import copies the source package entries into the
new `Imported_N/` directory, including entries from bodies not selected.

Document-root and cover references resolve a unique archive entry regardless
of filename case before they are rebased. Cover metadata edits use the same
lookup rule and reject ambiguous matches.
Vector path pages can also be appended to existing document bodies without
requiring a font resource. The existing page list and `MaxUnitID` are updated
while unrelated package entries stay intact.

PNG, JPEG, GIF, BMP, TIFF, JBIG2, and JPEG2000 image pages can likewise be appended to existing
document bodies. Their page, image bytes, multimedia resource, and ID updates
are committed together.
Page duplication and these append operations read split `MaxUnitID` text as
one value before allocating IDs, and replace the whole XML value when updating
it. Existing `DocumentRes`, `Attachments`, and `Annotations` links may also be
split by XML comments or CDATA; attachment edits and annotation reads resolve
the complete path.

The same ID allocation rule applies to new multimedia resources, annotations,
composite graphics, promoted page templates, color spaces, draw parameters,
embedded fonts, page layers, graphic objects, and pattern cells.
New documents also recognize a `DocumentRes` link split by XML text boundaries
when deciding whether the resource link already exists.

## 附件

Attachments can be added to an existing document body with
`add_attachment_to_document()` (Rust/Python), `AddAttachmentToDocument()` (Go),
or `ofd_add_attachment_to_document()` (C), preserving its existing list.
Attachment metadata includes optional format, creation and modification times,
visibility, and usage. `set_attachment_metadata()` (Rust/Python),
`SetAttachmentMetadata()` (Go), and `ofd_set_attachment_metadata_json()` (C)
replace these fields atomically; written timestamps use `YYYY-MM-DDTHH:mm:ss`.
`replace_attachment_data()` (Rust/Python), `ReplaceAttachmentData()` (Go), and
`ofd_replace_attachment_data()` (C) update an attachment's `FileLoc` and declared
size together while retaining its ID, metadata, and former payload entry.

`rename_attachment()` (Rust/Python), `RenameAttachment()` (Go), and
`ofd_rename_attachment()` (C) change an attachment's display name while
retaining its ID, content, and references.
`remove_attachment()` (Rust/Python), `RemoveAttachment()` (Go), and
`ofd_remove_attachment()` (C) remove a list entry by global index while
retaining its payload file and allocated ID. Signed document bodies reject
attachment removal.

## 批注与图章

`add_path_annotation()` (Rust/Python), `AddPathAnnotation()` (Go), and
`ofd_add_path_annotation_json()` (C) add styled vector `Path` annotations
without requiring a font resource. The path uses coordinates relative to its
appearance boundary and accepts the same style fields as page path objects.
`add_multi_path_annotation()` (Rust/Python), `AddMultiPathAnnotation()` (Go),
`addMultiPathAnnotation()` (Node.js), and
`ofd_add_multi_path_annotation_json()` (C) place 1–4096 independently styled
PathObjects in one `Path` annotation appearance. Each child boundary is local
to the annotation box. Invalid path geometry or commands leave the document
unchanged. Java ofdrw reads the child objects from the generated appearance.

`add_mixed_annotation()` (Rust/Python), `AddMixedAnnotation()` (Go),
`addMixedAnnotation()` (Node.js), and `ofd_add_mixed_annotation_json()` (C)
place ordered path, text, image, and composite objects in one `Path` appearance. Text
uses a page-visible font; images refer to page-visible image resources. Child
composite objects refer to page-visible composite graphic resources. Child
boundaries are local to the appearance. Missing resources and invalid objects
reject the entire edit. `add_mixed_stamp_annotation()` (Rust/Python),
`AddMixedStampAnnotation()` (Go), and `addMixedStampAnnotation()` (Node.js)
write the same appearance in a `Stamp` annotation; C JSON accepts
`"annotation_type":"Stamp"`.

`append_annotation_objects()` (Rust/Python), `AppendAnnotationObjects()` (Go),
`appendAnnotationObjects()` (Node.js), and
`ofd_append_annotation_objects_json()` (C) append ordered path, text, image,
or composite graphics to an existing appearance and return their new object IDs.
The edit is atomic; missing resources and invalid geometry leave the document
unchanged.
`annotation_objects(page_index, annotation_id)` (Rust/Python),
`AnnotationObjects()` (Go), `annotationObjects()` (Node.js), and
`ofd_annotation_objects_json()` (C) read direct PathObject, TextObject,
ImageObject, and CompositeObject entries from an annotation appearance in
painting order, including nested PageBlocks. The result includes geometry,
resource and font IDs, text runs, and abbreviated path data.

They also expose optional `name` and `blend_mode` values.
`update_annotation_object()` / `remove_annotation_object()` (Rust/Python),
`UpdateAnnotationObject()` / `RemoveAnnotationObject()` (Go), matching Node.js
methods, and matching C ABI calls edit or remove one direct appearance object
by ID. Updates can change boundary, CTM, visibility, and alpha together;
`ctm: null` clears the transform. Invalid updates leave the package unchanged.
`set_annotation_object_name()` and `set_annotation_object_blend_mode()`
(Rust/Python), their Go and Node.js counterparts, and the matching C ABI
calls edit those direct attributes. Blend modes use the same validation and
canonical names as page objects. Null clears either attribute.

`set_annotation_object_draw_param()` and its Python, Go, Node.js, and C ABI
counterparts set a DrawParam visible to the annotation's page; an absent
reference clears it. Invalid or unresolved references leave the package intact.
`set_annotation_path_data()` (Rust/Python), `SetAnnotationPathData()` (Go),
`setAnnotationPathData()` (Node.js), and `ofd_set_annotation_path_data()` (C)
replace the abbreviated drawing commands of one annotation PathObject. Other
appearance objects and attributes are preserved; invalid commands, a different
object type, and signed pages leave the document unchanged.

`set_annotation_path_stroke_style()` (Rust/Python),
`SetAnnotationPathStrokeStyle()` (Go), `setAnnotationPathStrokeStyle()` (Node.js),
and `ofd_set_annotation_path_stroke_style_json()` (C) replace an annotation
PathObject's direct line width, cap, join, miter limit, and dash settings.
They use the same style schema and validation as page path stroke edits.
`set_annotation_text_code()` (Rust/Python), `SetAnnotationTextCode()` (Go),
`setAnnotationTextCode()` (Node.js), and `ofd_set_annotation_text_code()` (C)
replace one TextCode run in an unshaped annotation TextObject. A character-count
change remeasures DeltaX when present. Shaped objects and incompatible vertical
advances are rejected without changing the document.

`append_annotation_text_code()` (Rust/Python), `AppendAnnotationTextCode()` (Go),
`appendAnnotationTextCode()` (Node.js), and `ofd_append_annotation_text_code_json()` (C)
append positioned runs with the same coordinate and delta validation as page text.
`set_annotation_text_code_layout()` (Rust/Python),
`SetAnnotationTextCodeLayout()` (Go), `setAnnotationTextCodeLayout()` (Node.js),
and `ofd_set_annotation_text_code_layout_json()` (C) replace one run's direct
X/Y and DeltaX/DeltaY attributes. Omitted fields clear direct values.
`set_annotation_text_typography()` (Rust/Python),
`SetAnnotationTextTypography()` (Go), `setAnnotationTextTypography()` (Node.js),
and `ofd_set_annotation_text_typography_json()` (C) edit annotation text size,
horizontal scale, directions, weight, and italic styling with page-text validation.

`set_annotation_text_paint_flags()` (Rust/Python),
`SetAnnotationTextPaintFlags()` (Go), `setAnnotationTextPaintFlags()` (Node.js),
and `ofd_set_annotation_text_paint_flags_json()` (C) set direct fill and stroke
switches; omitted values restore OFD defaults.
`annotation_objects()` and its Python/Go/Node.js counterparts return the
effective typography values for text objects; non-text objects use null values.
They also return effective `fill` and `stroke` switches for text and path
objects, with null values for image and composite objects.

They also return structured `codes` with direct X/Y and DeltaX/DeltaY values,
plus validated `cg_transforms` glyph mappings. The `text` list remains available.
The Python, Go, and Node.js set/append methods use
`ofd_edit_annotation_text_code_json()` so Unicode text including embedded NUL
survives C ABI transfer and OFD serialization.
Annotation summary and appearance-object readers decode OFD Unicode escapes
back to the original text.

`remove_annotation_text_code()` (Rust/Python), `RemoveAnnotationTextCode()` (Go),
`removeAnnotationTextCode()` (Node.js), and `ofd_remove_annotation_text_code()` (C)
remove one unshaped run while preserving the next run's inherited position;
at least one TextCode must remain.
`set_annotation_text_font()` (Rust/Python), `SetAnnotationTextFont()` (Go),
`setAnnotationTextFont()` (Node.js), and `ofd_set_annotation_text_font()` (C)
select a font resource visible on the annotation's page. Changing the font of
shaped text is rejected because its glyph IDs depend on the original font.

`set_annotation_graphic_resource()` (Rust/Python),
`SetAnnotationGraphicResource()` (Go), `setAnnotationGraphicResource()` (Node.js),
and `ofd_set_annotation_graphic_resource()` (C) replace an annotation image or
composite object's `ResourceID`. The replacement must be visible to the page
and have the matching resource type; invalid edits leave the package unchanged.
`add_highlight_annotation()` (Rust/Python), `AddHighlightAnnotation()` (Go),
and `ofd_add_highlight_annotation_json()` (C) add a rectangular `Highlight`
annotation with RGB color and an OFD alpha byte. Its appearance is a filled
path, so SVG, PNG, and PDF output show the highlight.

`add_text_note_annotation()` (Rust/Python), `AddTextNoteAnnotation()` (Go),
`addTextNoteAnnotation()` (Node.js), and
`ofd_add_text_note_annotation_json()` (C) create editable `Text` notes with
remark content, icon name, initial open state, color, and opacity. SVG and PNG
previews use a simple marker; PDF export writes a PDF text annotation.
`add_shape_annotation()` (Rust/Python), `AddShapeAnnotation()` (Go),
`addShapeAnnotation()` (Node.js), and `ofd_add_shape_annotation_json()` (C)
create editable PDF `Square` and `Circle` annotations. Boundaries, stroke
widths, and dash lengths use millimeters; insets use fractions of the box.

The OFD appearance is a vector path and the shape survives OFD and PDF
round trips.
`add_line_annotation()` (Rust/Python), `AddLineAnnotation()` (Go),
`addLineAnnotation()` (Node.js), and `ofd_add_line_annotation_json()` (C)
create editable PDF `Line` annotations with a vector OFD preview. Endpoints
are measured in millimeters from the annotation boundary's upper-left corner;
stroke width and dash lengths also use millimeters.
Changing a Line annotation's appearance boundary scales its preview path
and exported PDF endpoints with the new boundary.

Annotation list and page annotation references resolve a unique archive entry
regardless of filename case. Missing or ambiguous references are reported as
`annotation_reference`; edits to an existing annotation page use its resolved
entry path.
PDF `Text`, `Square`, `Circle`, `Underline`, `StrikeOut`, and `Squiggly`
annotations use OFD `Type="Path"` and retain their PDF kind in OFD `Subtype`.
This keeps the package readable by Java ofdrw's annotation type parser.

Older packages that used the PDF kind as OFD `Type` remain readable for PDF
export.
`add_image_stamp_annotation()` and `add_image_stamp_annotation_from_bytes()`
(Rust), `add_image_stamp_annotation()` (Python),
`AddImageStampAnnotation()` / `AddImageStampAnnotationFromBytes()` (Go), and
`ofd_add_image_stamp_annotation_json()` (C) add image-backed `Stamp`
annotations, using an existing image resource or adding one atomically.
Text-backed `Stamp` annotations use `add_text_stamp_annotation()` or
`add_styled_text_stamp_annotation()` (Rust), `add_text_stamp_annotation()`
(Python), `AddTextStampAnnotation()` or `AddStyledTextStampAnnotation()` (Go),
and `ofd_add_text_stamp_annotation_json()` (C). The style fields match text
watermarks and the appearance remains editable through annotation metadata APIs.

`add_uri_link_annotation()` (Rust/Python), `AddURILinkAnnotation()` (Go), and
`ofd_add_uri_link_annotation_json()` (C) add an invisible clickable `Link`
annotation with a `CLICK`/`URI` action. Annotation metadata includes parsed
actions, including destinations from existing OFD files. PDF export preserves
clickable URI links and direct `Goto` links to pages in the same document body.
Rust's `add_uri_link_annotation_with_quads()` and
`add_goto_link_annotation_with_quads()` accept convex quadrilateral click areas.
Python accepts `quad_points=` on both link writers, Go provides
`AddURILinkAnnotationWithQuads()` and `AddGotoLinkAnnotationWithQuads()`, and the
existing C JSON writers accept an optional `quad_points` array of four
`[x,y]` points per area. Coordinates use page millimeters.

`add_goto_link_annotation()` and `add_bookmark_link_annotation()` (Rust/Python),
`AddGotoLinkAnnotation()` and `AddBookmarkLinkAnnotation()` (Go), and matching C
JSON exports create page and named bookmark links. PDF export resolves bookmark
targets within the same document body; links to missing pages are skipped.
`set_annotation_visible()` and `set_annotation_print()` (Rust/Python), their
Go `SetAnnotationVisible()` / `SetAnnotationPrint()` counterparts, and the C
`ofd_set_annotation_visible()` / `ofd_set_annotation_print()` exports update
annotation display flags. `Visible=false` hides an annotation in SVG and PNG
previews; `Print=false` omits its artwork from printed PDF pages while preserving
screen previews. PDF export retains link annotations with `Hidden` or an unset
`Print` flag so their targets and screen/print behavior survive conversion.

Link `NoZoom`, `NoRotate`, and editing restrictions map to PDF flags; the raw
`PDFFlags` parameter retains flags without a direct OFD equivalent.
Link and attachment-link remarks, creators, and last modification days export
as PDF `Contents`, `T`, and `M`. Imported PDF `NM` names are retained in the
`PDFName` annotation parameter and reused on export; annotations created in
OFD use their OFD IDs as PDF `NM` values. Multiple URI or page-link click areas
export as one PDF annotation with multiple `QuadPoints`. When an attachment
link needs multiple PDF annotations, later names receive numeric suffixes to
keep names unique on the page.

The annotation `NoZoom`, `NoRotate`, and `ReadOnly` metadata flags can also be
set through Rust's named setters, Python `set_annotation_flag()`, Go
`SetAnnotationFlag()`, and C `ofd_set_annotation_flag()`; these flags are
preserved for OFD viewers and do not alter current preview geometry.
`add_page_object_attachment_action()` and `add_attachment_link_annotation()`
write OFD `GotoA` actions targeting an attachment in the same document body.
Python exposes the same names; Go uses `AddPageObjectAttachmentAction()` and
`AddAttachmentLinkAnnotation()`. The C ABI provides matching exports, with
`-1`/`0`/`1` for omitted/false/true `NewWindow` on the direct object action.

`validate_references()` reports missing `GotoA` targets in document, page,
template, and annotation XML.

## 多媒体资源与动作

`add_multimedia_resource()` adds Image, Audio or Video bytes and a `MultiMedia`
resource to a new or existing unsigned document body, returning its ID.
Image bytes use the same PNG, JPEG, GIF, BMP, TIFF, JBIG2, and JPEG2000 validation
as image objects; their format names are normalized in the resource entry.
PNG, JPEG, and GIF inputs are decoded within pixel and allocation limits before
writing, so a matching file header alone does not admit a truncated image.

`add_page_multimedia_resource()` creates a new `PageRes` file and payload scoped
to one page, updating its document's `MaxUnitID` atomically. Python uses the
same name, Go provides `AddPageMultimediaResource()`, and C exposes
`ofd_add_page_multimedia_resource()`.
`add_page_object_sound_action()` and `add_page_object_movie_action()` attach
`Sound` and `Movie` click actions to direct page objects; matching
`add_sound_link_annotation()` and `add_movie_link_annotation()` create invisible
clickable annotations. The Sound writer accepts optional volume (0–100), repeat,
and synchronous flags; Movie accepts Play, Stop, Pause, or Resume. Python exposes
the same method names, Go uses exported CamelCase names, and the C ABI exposes
multimedia resource creation plus JSON action and annotation writers.

`page_multimedia_resources()` lists effective image, audio, and video resources
for a page, with page-local IDs taking precedence over document resources.
`page_multimedia_data()` returns the original bytes by resource ID. Python uses
the same names, Go provides `PageMultimediaResources()` and
`PageMultimediaData()`, and C provides `ofd_page_multimedia_resources_json()`
and `ofd_page_multimedia_data()`.
`document_multimedia_resources()` and `document_multimedia_data()` expose
document-level resources even when no page references them; Python uses the
same names, Go uses `DocumentMultimediaResources()` and
`DocumentMultimediaData()`, and C has matching `ofd_document_*` exports.

`replace_document_multimedia_data()` updates a document resource without
changing its ID. It allocates a new media entry and keeps the old bytes for
other resource definitions that may refer to them. Signed document bodies
reject the edit. Python uses the same name; Go provides
`ReplaceDocumentMultimediaData()`, and C exposes
`ofd_replace_document_multimedia_data()`.
`replace_page_multimedia_data()` replaces the effective resource by page and
ID, following the same page-local precedence as the reader. Python uses the
same name, Go provides `ReplacePageMultimediaData()`, and C exposes
`ofd_replace_page_multimedia_data()`. When a page-local resource file is shared
with another page, a template, or a document body, replacement copies that resource file
and updates only the selected page's `PageRes` link.

Multimedia readers and replacement resolve `MediaFile` and `PageRes` paths
split by XML comments or CDATA and replace the complete path value.
`remove_document_multimedia_resource()` removes a document resource definition
only when reference validation shows no new dangling references. It keeps the
payload entry and allocated ID, and signed document bodies reject the edit.
Python uses the same name, Go provides `RemoveDocumentMultimediaResource()`,
and C exposes `ofd_remove_document_multimedia_resource()`.

`remove_page_multimedia_resource()` removes a page-local definition under the
same reference rule. It rejects resource files shared with another page or a
document body, keeps the payload and ID, and leaves document-level resources
untouched. Go provides `RemovePageMultimediaResource()` and C exposes
`ofd_remove_page_multimedia_resource()`.
Reference validation checks audio/video resource type, payload existence, and
playback attributes in pages, templates, and annotations. Media playback itself
is delegated to OFD viewers; SVG, PNG, and PDF previews do not play media.

## 页面对象与文字编辑

Positioned text and vector path objects can be added to any existing page.
Text insertion accepts a selected font resource or the document's first font;
an optional opaque RGB `fill_color` is accepted by Python `add_text_object()`
and the C JSON writer for plain or shaped text. Go exposes
`AddColoredTextObject()` and `AddColoredShapedTextObject()`; Rust exposes
`add_colored_text_object()` and `add_colored_shaped_text_object()`.

Go's `AddColoredShapedTextObjectWithLanguage()` and the Python/Rust shaped
variants also accept a language tag.
path insertion accepts fill, stroke, width, RGB colors, whole-object alpha,
and the even-odd fill rule. Rust uses `PathStyle.alpha`/`even_odd`, Python and
C JSON use `alpha`/`even_odd`, and Go uses `PathStyle.Alpha`/`EvenOdd`.
Independent `fill_alpha` and `stroke_alpha` byte values apply opacity to each
path color, including paths inside composite graphics.

Stroke style also accepts cap (`butt`, `round`, `square`), join (`miter`,
`round`, `bevel`), positive miter limit, a positive dash sequence of 1..256
values, and dash offset. The two-value dash/gap field remains available.
These options also apply to composite paths.
Positioned text can be shaped with an embedded OpenType font using
`add_shaped_text_object()` (Rust), `add_text_object(..., shape=True)` (Python),
`AddShapedTextObject()` (Go), or `shape: true` in the C JSON input. The writer
uses `CGTransform` and positioned glyph advances for one line, including
Arabic right-to-left text and mixed left-to-right/right-to-left runs. Unicode
bidirectional levels determine visual run order while the original Unicode
text stays in `TextCode` for extraction. Each visual run is also split at
Unicode script boundaries, preserving combining marks and common punctuation
with adjacent text. The Java layout engine's complete paragraph rules remain
pending.

Positioned shaped text accepts an optional BCP 47 language tag through Rust's
`add_shaped_text_object_with_language()`, Python's `add_text_object(...,
shape=True, language="sr")`, Go's `AddShapedTextObjectWithLanguage()`, or the C
JSON writer's `language` field. The tag selects localized OpenType glyph forms
when the embedded font provides them. Shaped flowed text accepts the same
language tag in `FlowTextStyle` (Rust and Go), as `language=` in Python, or in
the C JSON input. Styled spans can override that language with their own
`language` field. Measurement, wrapping, and output shaping use the selected
language consistently. Language settings require shaping to be enabled.

`page_text_objects()` (Rust/Python), `PageTextObjects()` (Go), and
`ofd_page_text_objects_json()` (C) expose each `CGTransform` with its UTF-16
code position/count, glyph count, and glyph IDs in drawing order. Missing
`CodeCount` or `GlyphCount` defaults to one as in the Java model; malformed
mappings return an error.
These readers also expose a text object's optional `HScale` value, plus its
effective `ReadDirection`, `CharDirection`, `Weight`, and `Italic` values with
OFD defaults (0, 0, 400, and false). Invalid declared values return an error.

`set_page_text_typography()` (Rust/Python), `SetPageTextTypography()` (Go),
and `ofd_set_page_text_typography_json()` (C) atomically update an existing
page text object's positive size, optional horizontal scale, directions,
weight, and italic flag. Omitted optional fields restore OFD defaults;
invalid values and edits to signed pages leave the page unchanged.
`set_page_text_code()` (Rust/Python), `SetPageTextCode()` (Go), and
`ofd_set_page_text_code()` (C) replace one zero-based `TextCode` run on a
direct, unshaped page text object. The run's origin and the object's
appearance are preserved. When the character count changes, `DeltaX` is
remeasured using the selected font. Runs with `DeltaY` must keep their
character count; objects with `CGTransform` require reshaping and are rejected.

Plain multi-character text writers emit measured `DeltaX` advances. For an
input OFD `TextCode` without `DeltaX` or `DeltaY`, the preview keeps successive
characters at the same origin, as specified by OFD.
`set_page_text_code_layout()` (Rust/Python), `SetPageTextCodeLayout()` (Go),
and `ofd_set_page_text_code_layout_json()` (C) update a direct, unshaped
`TextCode` run's X/Y coordinates and DeltaX/DeltaY advances. Each supplied
delta array has one finite value per character after the first. The first run
must declare X and Y; later runs may omit them and inherit the previous run's
coordinate. Invalid layouts and edits to signed pages leave the page intact.

`append_page_text_code()` (Rust/Python), `AppendPageTextCode()` (Go), and
`ofd_append_page_text_code_json()` (C) append another positioned run to a
direct, unshaped text object. Existing runs remain intact; omitted X/Y values
inherit the preceding run. Text and layout are validated together before the
page is changed.
`remove_page_text_code()` (Rust/Python), `RemovePageTextCode()` (Go), and
`ofd_remove_page_text_code()` (C) remove a direct, unshaped run while keeping
at least one `TextCode`. If the following run inherited X or Y from the
removed run, its effective coordinates are written explicitly so it stays in
place. Signed pages and invalid indexes remain unchanged.

`set_page_text_font()` (Rust/Python), `SetPageTextFont()` (Go), and
`ofd_set_page_text_font()` (C) select a font resource visible to the direct
page. Invalid font references and edits to signed pages are rejected. Shaped
text with `CGTransform` can only retain its current font, because its glyph
IDs belong to that font.
`set_page_text_paint_flags()` (Rust/Python), `SetPageTextPaintFlags()` (Go),
and `ofd_set_page_text_paint_flags_json()` (C) update a direct text object's
fill and stroke switches together. Clearing a switch's direct value restores
the OFD defaults: fill on and stroke off. Invalid targets and signed pages
leave the document unchanged.

`append_text_page_with_font()` (Rust/Python), `AppendTextPageWithFont()` (Go),
and `ofd_append_text_page_with_font()` (C) append a text page using a chosen
font from the selected document body.
PNG, JPEG, GIF, BMP, TIFF, JBIG2, and JPEG2000 objects can be inserted into new or existing pages;
the image bytes and multimedia resource are saved with the page object.
GIF, BMP, TIFF, and JPEG2000 bytes remain in the OFD package and are converted to PNG for
SVG, PNG, and PDF preview. JPEG2000 accepts JP2 containers and raw codestreams.

Animated GIF previews use the first frame, matching fixed-layout page rendering.
Decoding these formats is limited to 16 million pixels. Multi-page TIFF previews
use the first image directory while retaining the complete original TIFF bytes.
Image writers emit a `CTM` that fills the declared OFD boundary even when
pixel dimensions have a different aspect ratio. When an existing image omits
`CTM`, rendering uses the OFD unit square; image output is clipped to its
declared boundary.

An `ImageObject` without `ResourceID` or an available media file is skipped
during preview, matching the Java PDF renderer; `validate_references()` reports
the missing reference or file, including images in annotations and templates.
MediaFile references use a unique ASCII case-insensitive ZIP entry when the
exact path is absent. This reads Java's `containsJPEG.ofd` fixture, whose
`Doc_0` reference points to a `DOC_0` archive directory. Ambiguous matches
return an error.

Document `PublicRes` and `DocumentRes`, page-local `PageRes`, and embedded
`FontFile`, color `Profile`, and attachment list/payload references use the
same rule. Resource and attachment editors update the matched archive entry
instead of creating a second path with different casing.
Font metadata returns the matched archive path, so Rust, Python, and Go font
byte readers can open it directly. Public `entry()` access remains exact.

`ImageMask` resources are exposed by `page_images()` and rendered as transparent
pixels where the mask is not white. Missing masks appear in reference validation;
preview retains the unmasked image. A size mismatch also leaves the image
unchanged; masked images are limited to 16 million pixels before decoding.
`set_page_image_mask()` (Rust/Python), `SetPageImageMask()` (Go), and
`ofd_set_page_image_mask()` (C) set or clear an existing page image's mask after
checking the resource, decoding both images, and comparing pixel dimensions.

`Substitution` is exposed on direct and composite image listings as an
alternative multimedia resource ID. Reference validation checks it in pages,
composites, templates, and annotations. `set_page_image_substitution()`
(Rust/Python), `SetPageImageSubstitution()` (Go), and
`ofd_set_page_image_substitution()` (C) set or clear it after checking that the
resource exists and contains a supported image. Preview uses the primary
`ResourceID` by default. Rust `RenderOptions`, Python's
`prefer_image_substitution=True`, Go's `RenderOptions.PreferImageSubstitution`,
and the C `*_with_options` exports select a resolvable `Substitution` for SVG,
PNG, and PDF output. If the alternative is missing or has an unrecognized
format, rendering falls back to `ResourceID`. An `ImageMask` is applied to the
selected image only when its pixel dimensions match.

Composite graphic units may also declare `Thumbnail` and `Substitution`
multimedia resource IDs. Reference validation checks both; the same render
option selects a resolvable unit `Substitution` in place of vector content,
falling back to that content when the alternative is unavailable. Unit
`Substitution` IDs split by XML comments or CDATA resolve during validation
and rendering.
Image `Border` elements render in SVG, PNG, and PDF previews with their OFD
line width, horizontal and vertical corner radii, dash sequence and offset,
and border color and opacity. `set_page_image_border()` (Rust/Python),
`SetPageImageBorder()` (Go), and `ofd_set_page_image_border_json()` (C) replace
or remove a direct page image border. The writer accepts an RGB color;
the reader also renders color spaces resolved from OFD resources. Direct and
composite image listings include a parsed `border` object with OFD defaults,
the declared color attributes, and the original `BorderColor` XML for complex
paints.

Text, path, image, and composite graphic objects can be removed by OFD ID
without changing other objects on the page. Composite graphic objects can be
listed with `page_composite_objects()` (Rust/Python),
`PageCompositeObjects()` (Go), or `ofd_page_composite_objects_json()` (C),
including their resource ID, boundary, transform, alpha, and draw parameter.
`page_composite_units()` (Rust/Python), `PageCompositeUnits()` (Go), and
`ofd_page_composite_units_json()` (C) list accessible unit IDs, dimensions,
and optional thumbnail and substitution image resource IDs.

`set_page_composite_unit_images()` (Rust/Python),
`SetPageCompositeUnitImages()` (Go), and
`ofd_set_page_composite_unit_images()` (C) set or clear those two image
references after validating their media payloads. Editing a shared unit
updates every placement that uses its resource file. A unit declared in
document resources requires image references from document or public
resources so every page can resolve them.
When a composite unit has no drawable content, SVG/PNG/PDF previews use its
thumbnail image. If image substitution is requested and available, it takes
precedence over both the vector content and thumbnail.

`add_composite_object()` (Rust/Python), `AddCompositeObject()` (Go), and
`ofd_add_composite_object_json()` (C) place an existing composite graphic unit
from page or document resources, updating the page and maximum object ID together.
`add_composite_path_object()` (Rust/Python), `AddCompositePathObject()` (Go),
and `ofd_add_composite_path_object_json()` (C) create a composite graphic
resource containing a styled vector path in `DocumentRes`, then place it
atomically.
`add_composite_text_object()` (Rust/Python), `AddCompositeTextObject()` (Go),
and `ofd_add_composite_text_object_json()` (C) similarly create a composite
resource with positioned text and a selected document font.

`add_composite_image_object()` (Rust/Python), `AddCompositeImageObject()` (Go),
and `ofd_add_composite_image_object()` (C) create a multimedia resource and a
composite graphic unit containing a PNG, JPEG, or JBIG2 image in one operation.
`page_composite_images()` / `page_composite_image_data()` (Rust/Python),
`PageCompositeImages()` / `PageCompositeImageData()` (Go), and matching C
functions list nested composite images in page placement order and return
their original bytes. Image boundaries and transforms remain local to the
containing composite unit. Composite inspection, reference validation, and
previews ignore same-named objects in extension namespaces or under extension
wrappers.

`add_blank_page()` and `append_blank_page()` (Rust/Python),
`AddBlankPage()` and `AppendBlankPage()` (Go), and matching C functions create
empty pages without a placeholder text object. Existing document bodies can
receive blank pages without requiring a font resource.

## 文本流式布局

`add_flowed_text()` (Rust/Python), `AddFlowedText()` (Go), and
`ofd_add_flowed_text_json()` (C) wrap plain text within page margins and append
as many pages as needed. Embedded or available system font advances guide line
breaks; spaces are preserved as text and wrapping follows glyph advances.
Plain and styled wrapping keep Unicode grapheme clusters intact, including
combining marks and joined emoji. A cluster wider than the content area is
rejected before the document is changed.

Plain, styled, and shaped wrapping prefer Unicode line-break opportunities
such as spaces; an overlong word falls back to grapheme-sized breaks. Existing
leading spaces remain in the text instead of forming a blank line by themselves.
Explicit LF, CRLF, CR, vertical tab, form feed, next-line, line separator,
and paragraph separator characters create lines in all flow modes. CRLF is
treated as one break even when its characters are in different styled spans;
trailing breaks do not append empty pages.

For a two-character pair that retains the original glyphs, OpenType pair
kerning adjusts both the wrap width and the written OFD `DeltaX` position.
Explicit newlines within the text are retained, while trailing newlines do
not create empty pages. Lines support left, center, or right alignment and
an indent on the first laid-out line of the input block. The writer rejects a
glyph wider than the full content area without changing the
document. If a glyph fits the full width but not the indented first line, it
moves to the next line. The plain-text method does not apply per-span styles,
or the Java layout engine's full pagination rules.

Set `indent_each_paragraph=true` to reapply the same first-line indent after
each explicit line or paragraph break. This works in plain, styled, and
shaped flow; the default keeps indentation on the input block's first line.
`rest_line_indent` sets the left indent in millimeters for lines that do not
receive the first-line indent, including wrapped lines across pages. With
`indent_each_paragraph=false`, this also applies to lines after explicit
breaks. It defaults to zero and must leave positive content width. Plain,
styled, and shaped flow measure the reduced width before breaking lines. C
JSON, Python, Go, and Rust accept the same field.

`paragraph_gap` adds millimeters between explicitly separated paragraphs in
all three modes. It defaults to zero, must be nonnegative, and is omitted at
the top of a new page when a paragraph moves there.
Set `keep_paragraphs_together=true` to move a wrapped paragraph as a unit to
the next page when it cannot fit below preceding content. A paragraph taller
than the usable page area returns an error before the document changes. This
applies to plain, styled, and shaped flow.

Set `shape=true` (Python/C), `Shape=true` (Go), or `FlowTextStyle.shape=true`
(Rust) to wrap with an embedded OpenType font using shaped line widths and
Unicode grapheme boundaries. Each written line retains its text and
`CGTransform` glyph mapping. This mode supports mixed text directions within
a line and is also available for styled spans.
Python accepts `align="left"|"center"|"right"`, `first_line_indent` in
millimeters, and `first_line_indent_chars` as a placeholder character count.

Go exposes the same options through `DefaultFlowTextStyle()`. A character
indent uses the default font size for plain text, or the first nonempty span's
font size plus letter spacing for styled text. A positive millimeter indent
takes precedence.
`add_flowed_spans()` (Rust/Python), `AddFlowedSpans()` (Go), and
`ofd_add_flowed_spans_json()` (C) flow a sequence of styled spans across lines
and pages. Each span may override the font, size, RGB fill color, bold, italic,
and OFD weight (100–900) settings. A span may also disable fill or enable text
stroke with its own RGB color and line width. Explicit weight overrides `bold`.

Spans on one line share a baseline. Each line's height is its
largest font size plus the difference between `line_height` and the default
`font_size`; pages break when the next complete line cannot fit. Pair kerning
applies within each span and line, while separate spans retain independent
positions. Letter spacing
is written as `DeltaX`; optional underlines are separate OFD paths with adjustable offset
and width. With `shape=true`, each span uses its embedded OpenType font for
shaping and wraps at Unicode grapheme boundaries. The Java layout engine's
other paragraph features remain pending.

`line_break_after` forces the next span onto a new line. `keep_together` keeps
each newline-delimited part of a span on one line; an overwide part returns an
error without modifying the document. In shaped flow, a grapheme that crosses
span boundaries inherits the first span's appearance, while any overlapping
`keep_together` span still keeps the whole touched grapheme with its following
content.

## 表格与嵌套布局

`add_table()` (Rust/Python), `AddTable()` (Go), and `ofd_add_table_json()` (C)
lay out fixed-width columns on newly appended pages. A positive `row_height`
uses fixed-height rows; `row_height=0` sizes each row from its wrapped content.
Cell text wraps within padding, rows stay together, and the first row can
repeat as a header after a page break. `add_table_cells()` (Rust/Python) and
`AddTableCells()` (Go) accept cells with row and column spans through the same
`ofd_add_table_json()` C entry point. Vertically merged row groups stay on one
page; auto height grows to fit their content. Borders and text are OFD path and
text objects, so they remain selectable and render in SVG, PNG, and PDF.

Each cell supplies `row` and `column`; `text` can be omitted for an image-only
cell. `row_span` and `column_span` default to one in C/Python JSON. Cells must
cover the table grid once, and a
repeated header cannot span later rows.
Cells may also set `background_color` and `text_color` as RGB byte triples,
and `align` as `left`, `center`, or `right`. These options apply to each
line of a merged or ordinary cell; the background fills the whole cell.

Cells may override `font_size` and `line_height` in millimeters. If only
`font_size` is set, the table's extra leading is preserved. Both values
contribute to wrapping and automatic row height, including nested grids.
`padding` overrides the cell's top, right, bottom, and left inner spacing as
four millimeter values. It controls text wrapping, content placement, and
automatic row height, including nested cells.

`border_width` and `border_color` override the table border for one cell;
zero width omits that cell's border. These options also apply to nested cells.
Cells may also select an existing document font by `font_id`; nested cells
inherit their containing cell's font unless they choose another. Wrapping
uses the selected font's measured glyph advances, and unknown IDs fail before
the document changes.

`bold` and `italic` apply to every text line in a cell, including ordered text
blocks and nested grid cells. Bold writes OFD text weight 700; italic writes
the OFD `Italic` flag.
`underline` and `strikethrough` draw selectable text with matching-color
vector decoration paths on each line, including wrapped and nested cell text.
`letter_spacing` adds millimeter spacing between adjacent Unicode grapheme clusters. It
participates in wrapping and is written as `TextCode DeltaX` advances; negative
spacing is accepted while glyph advances remain positive.

`vertical_align` places the cell's combined content at `top` (default),
`center`, or `bottom` within its padded height. It applies to outer and
nested grids, including text and images; invalid values are rejected.
Each cell can contain an image after its text. Set `image` to PNG, JPEG, GIF, BMP,
TIFF, JBIG2, or JPEG2000 bytes plus `format`, `width`, and `height` in millimeters. Rust takes
`TableCellImage { data, format, width, height }`; Python accepts `data` as
bytes; Go uses `TableCellImage.Data`. The C JSON entry point accepts
`data_base64`. Image height contributes to automatic row sizing, the image
uses the cell alignment, and a mixed text/image cell has one padding-sized
gap. Invalid image data or geometry leaves the document unchanged. For
ordered cell content, use `blocks`: each block is a `text` paragraph, an
`image`, a `spacer` with a height, a `rule` with a thickness and optional
RGB color. Text blocks may override cell alignment and RGB text color; image
blocks use the cell alignment. A rule spans the cell's inner width and uses
the table border color when its own color is omitted. Blocks are separated by
one padding-sized gap and contribute to automatic row height. `blocks` cannot
be combined with the legacy `text` or `image` fields. Rust uses
`TableCellBlock`, Python accepts image `data` as bytes, and Go uses
`TableCellBlock.Data`. The C JSON entry point uses `data_base64` for image
blocks. A `table` block embeds a complete grid with `row_count`,
`column_widths`, and `cells`; it also accepts `row_height`, `padding`,
`border_width`, and `border_color`. Nested grids may contain further table
blocks up to eight levels deep. They use the outer table's font and text
metrics, stay inside one outer cell, and contribute their full height to
automatic row sizing. Nested columns must fit the containing cell's inner
width; the same grid coverage and span checks apply recursively.

Tables with no nonempty text blocks can be added to document bodies without
font resources. A text-bearing table still requires its selected font.

## 页面顺序与对象分组

Pages can be moved within a document body while preserving their content paths
and unrelated package entries. Later page additions retain the new order.
`insert_text_page()` / `insert_blank_page()` (Rust/Python),
`InsertTextPage()` / `InsertBlankPage()` (Go), and matching C functions insert
at a zero-based position within one document body and return the new global
page index. Positions equal to the body's page count append. The operation is
atomic and rejects signed bodies.

`move_page_object()` (Rust/Python), `MovePageObject()` (Go), and
`ofd_move_page_object()` (C) place a page object before another object or at
the end of its current layer or `PageBlock`. A target in another layer or
`PageBlock` also moves the object into that container; unrelated page XML and
allocated IDs are preserved.
`group_page_objects()` (Rust/Python), `GroupPageObjects()` (Go), and
`ofd_group_page_objects()` (C) wrap an inclusive range of ordered sibling
graphic objects in a new `PageBlock`. The range must share one layer or
existing `PageBlock`; grouping preserves drawing order and object IDs and can
be repeated to create nested blocks.

`ungroup_page_objects()` (Rust/Python), `UngroupPageObjects()` (Go), and
`ofd_ungroup_page_objects()` (C) remove the immediate `PageBlock` containing a
named object, lifting all its children into the parent in order. Blocks with
attributes, local namespace declarations, or unsupported children are
rejected so their metadata is not discarded.
`page_objects()` exposes the optional graphic `Name` attribute. Rust and Python
`set_page_object_name()` and Go `SetPageObjectName()` set or clear it on an
unsigned page; XML-special characters round-trip through the C ABI.

It also exposes `BlendMode`; Rust/Python `set_page_object_blend_mode()` and Go
`SetPageObjectBlendMode()` set or clear supported compositing modes on direct
page objects. The C ABI uses `ofd_set_page_object_blend_mode()`.
Rust/Python `set_page_object_clip_rects()` and Go `SetPageObjectClipRects()`
replace a direct object's clips with rectangles in object coordinates. Each
rectangle becomes a separate `Clip`; the visible area is their intersection.
An empty list removes the clips. `follows_ctm` writes `TransFlag`, and the C ABI
uses `ofd_set_page_object_clip_rects_json()`.

For arbitrary OFD path areas, Rust/Python `set_page_object_path_clips()` and Go
`SetPageObjectPathClips()` accept clips containing one or more path areas. Each
area has a boundary and abbreviated path commands, with optional area/path CTM
and even-odd fill rule. Optional `fill`, `stroke`, and `draw_param` fields select
the clip shape and stroke geometry; omitted flags write fill enabled and stroke
disabled. DrawParam IDs must resolve from the page's resources. The C ABI uses
`ofd_set_page_object_path_clips_json()`.

Writers mark generated path areas as filled and not stroked, matching the
rectangle/path area intended by these APIs. Existing OFD path areas without
flags use the OFD defaults (stroke enabled, fill disabled); SVG/PNG/PDF previews
render their stroked outline as the clip shape. Clip readers expose the effective
`fill` and `stroke` flags.
Rust/Python `set_page_object_clips()` and Go `SetPageObjectClips()` also accept
`Text` areas in the same clip as `Path` areas. Text areas specify a resolvable
font ID, size, boundary, text, and baseline position; optional CTMs, horizontal
scale, and character direction are supported. The C ABI uses
`ofd_set_page_object_clips_json()`.

Text clip writers can also take positioned `text_codes` and `cg_transforms` for
multi-run text and explicit glyph IDs. Their combined text must equal `text`;
invalid glyph ranges and displacements reject the edit atomically.
Text areas can specify `fill`, `stroke`, and an Area `draw_param`; omitted flags
use the OFD text defaults (fill enabled, stroke disabled). Stroked text clip
outlines use the resolved line width in SVG, PNG, and PDF output.

Rust/Python `page_object_clips()` and Go `PageObjectClips()` read clips from page
and template objects in painting order. Each area includes its original `Area`
XML so glyph mappings and extensions remain available alongside parsed fields.
Text areas also expose their positioned `TextCode` runs, raw glyph displacement
sequences, horizontal scale, character direction, and `CGTransform` glyph maps.
The C ABI uses `ofd_page_object_clips_json()`.

The reader accepts empty `Clips` nodes emitted by Java ofdrw as no clipping,
but rejects empty `Clip` nodes, an `Area` without exactly one `Path` or `Text`,
invalid clip CTMs, and nonpositive declared clip boundaries. Reference validation
reports malformed clip areas in pages, templates, composite units, and
annotation appearances.
Direct page layers can be reclassified as `Background`, `Body`, or `Foreground`
with Rust/Python `set_page_layer_type()` or Go `SetPageLayerType()`. Layer names
can be set or cleared with `set_page_layer_name()` / `SetPageLayerName()` /
`setPageLayerName()` (Node.js). Their reusable
drawing parameters can be set or cleared with `set_page_layer_draw_param()` /
`SetPageLayerDrawParam()`. The C ABI exposes `ofd_set_page_layer_type()`,
`ofd_set_page_layer_name()`, and
`ofd_set_page_layer_draw_param()`. References are checked before editing, and
signed pages reject edits.

`set_page_object_visible()` / `SetPageObjectVisible()` and
`set_page_object_alpha()` / `SetPageObjectAlpha()` update an existing page
graphic object's visibility and whole-object opacity. Hidden or fully
transparent text is omitted from text extraction as well as previews; the
C ABI uses `ofd_set_page_object_visible()` and `ofd_set_page_object_alpha()`.
`set_page_object_boundary()` / `SetPageObjectBoundary()` move or resize an
existing graphic object's OFD boundary; `set_page_object_ctm()` /
`SetPageObjectCTM()` set or clear its local transform. The C ABI uses
`ofd_set_page_object_boundary()` and `ofd_set_page_object_ctm_json()`.

Coordinates are millimeters, boundary dimensions must be positive, and all
matrix entries must be finite.
Object and layer additions, removals, moves, and visibility, alpha, boundary,
and CTM changes reject pages in signed document bodies before changing XML.
Page area edits, page reordering, new pages, bookmarks, outlines, embedded fonts, and attachments
also reject signed document bodies before changing their protected entries.

`add_page_object_uri_action()` / `AddPageObjectURIAction()` attaches an OFD
`CLICK` URI action to a direct graphic object; repeated calls append actions
in order. `page_object_actions()` / `PageObjectActions()` reads its action
events, kinds, and attributes. `page_source_object_actions()` /
`PageSourceObjectActions()` also reads actions on template objects, using the
`source_path` returned by `page_objects()`. The C functions are
`ofd_add_page_object_uri_action()`, `ofd_page_object_actions_json()`, and
`ofd_page_source_object_actions_json()`.
`add_page_object_goto_action()` / `AddPageObjectGotoAction()` appends an OFD
`CLICK` `Goto` action to a page within the same document body. Destinations
use the same page ID and optional coordinates as bookmarks and outlines; the
C entry point is `ofd_add_page_object_goto_action_json()`.

`add_page_object_bookmark_action()` / `AddPageObjectBookmarkAction()` targets
a named bookmark in the same document body. `remove_page_object_action()` /
`RemovePageObjectAction()` removes an action by its position on the object;
the C functions are `ofd_add_page_object_bookmark_action()` and
`ofd_remove_page_object_action()`.
`set_page_object_action_regions()` / `SetPageObjectActionRegions()` sets one or
more rectangular trigger areas for an existing action; an empty list removes
its explicit `Region`. The C entry point is
`ofd_set_page_object_action_regions_json()`, taking an array of `[x,y,width,height]`
rectangles in page millimeters. Action readers return each Area path and its
commands. PDF export turns the first supported `CLICK` action on a visible
page or template graphic object into Link annotations. Each explicit rectangular
Area creates its own link; convex four-sided Areas use PDF `QuadPoints` for
their precise hit region. Without `Region`, the object's `Boundary` is used.

URI, direct page destinations, and named bookmarks are supported. PDF export
reports an error for curves and other Region paths that cannot be represented
accurately by a PDF link quadrilateral. Link annotation actions also use
explicit Regions when present. Template actions produce links on each page
that references the template, in the same layer order as rendering.

Pages without navigation or annotation dependencies can be removed from document
order. The dependency check covers document, page, template, and annotation
destinations, including numeric PageID aliases. Removing a page retains its
package entries to avoid discarding shared data; page removal is rejected for
signed document bodies.
When two pages in one document body have the same numeric page ID, reference
validation reports `duplicate_page_id`. Annotation lookup and creation, SVG
preview, and PDF export reject the ambiguous page ID instead of assigning one
page's annotations to both pages.

Page navigation writers accept numeric ID aliases such as `4` and `04` when
they identify one page; duplicate targets are rejected before editing.
Adding an annotation reuses the page's existing record when its `PageID` is
written with a numeric alias; duplicate matching records are rejected.
Reference validation reports duplicate and invalid annotation page records.
Signature stamps with an aliased `PageRef` still render on their page.

`reorder_document_pages()` (Rust/Python), `ReorderDocumentPages()` (Go),
`reorderDocumentPages()` (Node.js), and
`ofd_reorder_document_pages_json()` (C) apply a complete permutation of one
document body's pages in one edit. Indexes are zero-based within that body;
invalid permutations leave the package unchanged. Page files, IDs, resources,
annotations, and unrelated `Document.xml` markup retain their paths and data.
`duplicate_page()` (Rust/Python), `DuplicatePage()` (Go), and
`ofd_duplicate_page()` (C) append a copy of a page to the same document body and
return its new zero-based index. The copied page gets a new package entry, page
ID, and graphic object IDs while retaining its relative page resources,
document resources, and template links. Annotation records and appearances are
copied with new IDs. Annotation `FileLoc` values split by XML comments or CDATA
are resolved and replaced as complete paths. Signed source pages are rejected.

`duplicate_page_at()` (Rust/Python), `DuplicatePageAt()` (Go), and
`ofd_duplicate_page_at()` (C) place the copy at a zero-based position within
the same document body and return its global index. The page and its
annotations are copied and reordered atomically.
`duplicate_page_object()` (Rust/Python), `DuplicatePageObject()` (Go), and
`ofd_duplicate_page_object()` (C) copy a direct text, path, image, or composite
object immediately after itself in the same layer or PageBlock. The copy keeps
its style and resource references, receives a fresh ID, and updates MaxUnitID
in the same edit. Nested clipping `Path` and `Text` IDs also receive fresh
values. Unknown nested IDs, signed pages, and candidate IDs already present
on the page are rejected. A stale `MaxUnitID` reported for
the document body's pages or resources also rejects the copy.

Subsequent edits to a newly created document preserve unrelated `Document.xml`
elements, including extension nodes, while updating pages, resources, and bookmarks.

## SVG 预览

Pages can be exported as SVG previews with text, basic vector paths, embedded
PNG/JPEG/GIF/BMP/TIFF/JBIG2 images, template content, path/text clipping areas, and common
axial/radial and free-form/grid Gouraud gradients, and nested composite graphic units containing
text, paths, and images. Repeating gradients honor `MapUnit`; radial gradients
support nonzero start radius, eccentricity, and angle. Advanced composite
content and full font shaping remain unsupported. Text clipping areas use
embedded font glyph outlines when `CGTransform` supplies glyph IDs.

## SVG 字体与绘制效果

Standalone SVG previews carry visible embedded TrueType/OpenType sfnt fonts as
data URLs, up to eight MiB of source font bytes per page. Fonts with the same
declared name receive distinct SVG family names. All embedded faces use internal
family names so CSS generic names and comma-separated names remain unambiguous.
Fonts referenced by page, template, annotation, and composite text are embedded
before unused resources when the size limit is reached. Larger or unsupported font
programs retain the named-font fallback. PNG and PDF rendering continue to
load embedded fonts directly without the SVG data URLs.

Gouraud meshes interpolate vertex color and alpha, honor `Extend` and
`BackColor`, and rasterize up to four million pixels per painted object.
For path objects without `Boundary`, the mesh uses the parsed path bounds.
`set_page_object_gouraud_fill()` (Rust/Python), `SetPageObjectGouraudFill()`
(Go), and `ofd_set_page_object_gouraud_fill_json()` (C) replace a direct path
or text object's fill. Points use object-local millimeters and RGB color;
`vertices_per_row` selects a regular grid, while omitting it selects free-form
triangles with optional `edge_flag` 1 or 2 to reuse the previous edge.

Invalid meshes and edits to signed pages leave the page unchanged.
The corresponding `set_page_object_gouraud_stroke()` / `SetPageObjectGouraudStroke()`
and C `ofd_set_page_object_gouraud_stroke_json()` write mesh shading to a
path or text outline while enabling its stroke.
`set_page_object_gradient_fill()` (Rust/Python),
`SetPageObjectGradientFill()` (Go), and
`ofd_set_page_object_gradient_fill_json()` (C) write axial or radial
gradients to direct path and text objects. Styles specify start/end points,
at least two RGB segments with optional positions and alpha, `Extend`,
and direct, repeat, or reflect mapping. Radial styles also specify an end
radius and may set a start radius, eccentricity, and angle. Invalid styles
leave the page unchanged.

The corresponding `set_page_object_gradient_stroke()` /
`SetPageObjectGradientStroke()` and C
`ofd_set_page_object_gradient_stroke_json()` write axial or radial shading
to a path or text outline. The style's `fill_alpha` field controls the
`StrokeColor` container opacity for this call.
`set_page_object_pattern_fill()` / `set_page_object_pattern_stroke()`
(Rust/Python), `SetPageObjectPatternFill()` /
`SetPageObjectPatternStroke()` (Go), and the matching C JSON functions write
repeated patterns for direct path or text objects. A pattern has a positive
tile size, optional steps, reflection, origin, transform, and opacity. Its
independent cell coordinate space accepts path, text, image, and composite
objects; image, font, and composite resources must already be visible to the
page. An optional `thumbnail` names an existing image resource and can be used
without cells to write an image-backed tile. The writer allocates distinct cell IDs, updates `MaxUnitID`, and commits
the page and document changes together.

`set_page_object_solid_fill()` / `set_page_object_solid_stroke()`
(Rust/Python), `SetPageObjectSolidFill()` / `SetPageObjectSolidStroke()`
(Go), and the matching C JSON functions replace an existing path or text
paint with an RGB color and byte opacity. They can replace gradients or
patterns without altering the object's other paint.
`page_object_paints()` (Rust/Python), `PageObjectPaints()` (Go), and
`ofd_page_object_paints_json()` (C) read direct path and text paint declarations
from page and template layers in painting order. Each entry reports whether
fill and stroke are enabled, the paint kind, alpha, color attributes, and the
original paint XML. `effective_fill` and `effective_stroke` also resolve object,
layer, and relative `DrawParam` references using the same precedence as SVG
rendering; their `effective_*_draw_param` fields identify inherited sources.

Missing direct paint nodes return null. A null effective paint leaves the
renderer’s implicit black fallback unspecified. Pattern cell objects are excluded.
`page_object_stroke_styles()` (Rust/Python), `PageObjectStrokeStyles()` (Go),
and `ofd_page_object_stroke_styles_json()` (C) expose effective path and text
line width, cap, join, miter limit, dash pattern, and dash offset after the same
DrawParam inheritance used by SVG rendering. Text objects report null for
path-only geometry. `dash_sequence` returns the full OFD array, including
one-value and longer patterns; the existing `dash_pattern` pair is populated
only for two-value patterns. Records include template objects and painting order.

`set_page_path_stroke_style()` (Rust/Python), `SetPagePathStrokeStyle()` (Go),
and `ofd_set_page_path_stroke_style_json()` (C) replace a direct page path's
stroke geometry in one edit. Line width must be positive; optional fields
clear direct attributes when omitted, allowing DrawParam inheritance. Invalid
styles, non-path IDs, and edits to signed pages leave the page unchanged.
The atomic stroke edit accepts either the two-value `dash_pattern` or a full
`dash_sequence`; supplying both is an error.

`set_page_path_dash_sequence()` (Rust/Python), `SetPagePathDashSequence()` (Go),
and `ofd_set_page_path_dash_sequence_json()` (C) write 1..256 positive finite
dash lengths with an optional offset. A null sequence clears both direct dash
attributes. Invalid sequences leave the page unchanged.
New `PathStyle` and reusable `DrawParamStyle` values also accept
`dash_sequence` in Rust, Python, Go, and C JSON. The two-value `dash_pattern`
field remains available; supplying both fields is an error.

`set_page_path_data()` (Rust/Python), `SetPagePathData()` (Go), and
`ofd_set_page_path_data()` (C) replace a direct page path's OFD abbreviated
drawing commands while preserving its boundary and paints. Commands are
validated against the supported path syntax before the page is changed.
Page path, text, image, composite, and layer-count readers likewise treat
pattern cell content as paint data rather than independent page objects.

Text extraction and keyword search omit text inside pattern cells; SVG/PNG
rendering paints those cells only through their parent pattern.
An imported or newly written `CellContent` with no graphic objects uses its `Thumbnail` image
as the SVG/PNG preview tile; reference validation reports missing or unreadable
thumbnail images.
`add_document_draw_param()` (Rust/Python), `AddDocumentDrawParam()` (Go), and
`ofd_add_document_draw_param_json()` (C) create reusable document drawing
parameters with optional `Relative` inheritance, line and dash settings, and
solid RGB, axial/radial gradient, Gouraud, or repeated pattern fill/stroke paints.

`add_page_draw_param()` (Rust/Python), `AddPageDrawParam()` (Go),
`addPageDrawParam()` (Node.js), and `ofd_add_page_draw_param_json()` (C) create
page-scoped drawing parameters. They may inherit visible document or page
styles and use resources visible on that page.
`document_draw_params()` / `page_draw_params()` in Rust and Python,
`DocumentDrawParams()` / `PageDrawParams()` in Go, and
`documentDrawParams()` / `pageDrawParams()` in Node.js list the definitions
with their IDs, optional `Relative` references, defining resource paths, and
self-contained XML. Page results include inherited document resources in
precedence order; a page-local definition wins when IDs overlap.

`set_document_draw_param()` and `set_page_draw_param()` in Rust and Python,
`SetDocumentDrawParam()` and `SetPageDrawParam()` in Go, and their Node.js/C
counterparts replace a definition without changing its ID. They validate
inheritance and pattern resources in the target scope, reserve new pattern
cell IDs, and commit resource XML plus `MaxUnitID` atomically.
Updates to a resource XML linked by another page or document body are rejected
to avoid changing that other scope. Document DrawParam insertion likewise
rejects a shared public resource file.

`remove_document_draw_param()` / `remove_page_draw_param()` and their Python,
Go, Node.js, and C counterparts delete unused definitions in their own scope.
They reject active object, layer, annotation, clip, and `Relative` references,
including cases where a page definition would otherwise fall back to a
same-ID document definition. Allocated IDs and pattern payload files remain.
Reference validation also reports broken inheritance in unused DrawParams.

Pattern cell resources must be visible from document resources so the DrawParam
can be reused across pages; its cell object IDs are reserved in `MaxUnitID`. Each fill and
stroke accepts only one paint kind. `set_page_object_draw_param()` /
`SetPageObjectDrawParam()` / `ofd_set_page_object_draw_param()` assign or clear
the reference on a direct page object. Resource XML and `MaxUnitID` change
atomically. The writer creates a public resource file when an existing
document has none; subsequent font insertion in a new document preserves
the drawing parameters already in its public resources.

Page metadata includes the effective `PhysicalBox`, `ApplicationBox`,
`ContentBox`, and `BleedBox`, using page, template, and document area
precedence. `BleedBox` defaults to the physical box. SVG, PNG, and PDF output
clip page content to `ApplicationBox` when it is specified.
When `PhysicalBox` is omitted, the selected area's `ApplicationBox`, then
`ContentBox`, supplies the page canvas, matching the Java reader.

`set_page_area()` (Rust/Python), `SetPageArea()` (Go), and
`ofd_set_page_area_json()` (C) write explicit page boxes in millimeters while
preserving unrelated page XML. Optional boxes can be removed.
Graphic objects marked `Visible="false"` are omitted from previews.
Graphic `BlendMode` values, including the `Darken` mode used by invoice stamps,
are applied to SVG, PNG, and PDF output.
Pattern paints support tiled cell content, X/Y spacing, row/column reflection,
transforms, and page- or object-relative placement.

Solid colors and gradient stops resolve document color spaces, including
GRAY, RGB, CMYK, indexed palettes, `DefaultCS`, and 1/2/4/8/16 bit channels.
Palette channel values split by XML comments or CDATA are joined for listing
and rendering.
When a color space declares an ICC `Profile`, supported GRAY, RGB, and CMYK
profiles are converted to sRGB for SVG, PNG, and PDF output. Without a profile,
CMYK uses a device independent arithmetic approximation. A 16-bit
`BitsPerComponent` color is transformed at 16-bit precision before its final
8-bit output conversion.

`add_document_color_space()` (Rust/Python), `AddDocumentColorSpace()` (Go),
and `ofd_add_document_color_space_json()` (C) add GRAY, RGB, or CMYK color
spaces to public resources and allocate an OFD ID. The spec accepts
`color_type`, optional `bits_per_component` (default 8 in the JSON binding),
optional `profile` relative to the resource file, and optional `palette`
channel arrays. The writer validates channel counts, component ranges, and
ICC profile type before changing the package. `document_color_spaces()` and
`page_color_spaces()` (or their Go and C equivalents) list definitions and
their source resource path; page definitions win on duplicate IDs.

Resource reading also rejects palette entries whose channel count or values
exceed the declared type and bit depth; `validate_references()` reports these
as `color_space_resources` issues.
Font, color-space, and multimedia readers ignore OFD-looking nodes inside
foreign-namespace extension wrappers. Resource edits select the real OFD
definition even when an extension reuses its ID. Newly created documents
preserve unrelated resource XML when multimedia is added, replaced, or removed.

`set_document_color_space()` / `SetDocumentColorSpace()` /
`setDocumentColorSpace()` and their page counterparts replace a definition in
its original scope while preserving its ID. The update validates palettes and
ICC profiles and rejects edits that introduce unresolved references.
`remove_document_color_space()` / `RemoveDocumentColorSpace()` /
`removeDocumentColorSpace()` and the matching page methods remove unreferenced
definitions while retaining allocated IDs. DefaultCS and linked page or
resource paint references block deletion; resource files shared outside the
selected scope are protected.

`add_page_color_space()` / `AddPageColorSpace()` /
`ofd_add_page_color_space_json()` create a page-local resource file without
editing files shared by other pages. `document_default_color_space()` and
`set_document_default_color_space()` (with matching Go and C APIs) read, set,
or clear `CommonData/DefaultCS`. The setter requires a document-visible
ColorSpace ID so all pages can resolve the default.
Page and template `PageRes` files can supply multimedia, fonts, composite graphics,
`DrawParam`, and color space resources. Page resources take precedence over
document resources with the same ID.

## 文本提取与检索

Text extraction expands visible composite graphic units in page order. Text
objects and composites with `Visible="false"` or `Alpha="0"` are omitted from
page text, region text, and keyword queries as they are from previews.
Template-aware text and positioned text follow page-layer painting order, so
foreground template text comes after the page's own text.
`all_text()` (Rust/Python), `AllText()` (Go), and `ofd_all_text_json()` (C)
return one array of TextCode strings per page in package order, including empty
arrays for blank pages.

`page_text_in_region()` (Rust/Python), `PageTextInRegion()` (Go), and
`ofd_page_text_in_region_json()` (C) extract characters whose TextCode origins
fall inside a rectangle in page millimeters. The region includes page templates
and nested composite text, and applies object boundaries, CTM, and DeltaX/Y.
`keyword_positions()` (Rust/Python), `KeywordPositions()` (Go), and
`ofd_keyword_positions_json()` (C) locate repeated keywords across TextCode
runs within one TextObject, including templates and composite text. Separate
TextObjects do not form a single keyword. Matches return a zero-based page
index and a box in millimeters. Boxes use the declared text size as an
approximate glyph width and height, then apply `HScale`, `CharDirection`,
object CTM, and parent transforms before returning page coordinates.

Path preview applies OFD fill rules, line joins and caps, dash patterns,
and fill/stroke color alpha values.
Path objects without a `Boundary` use their path coordinates and CTM directly,
matching the Java converter; an explicitly malformed boundary remains invalid.
Path and text objects with a `Boundary` are clipped to that rectangle in SVG,
PNG, and PDF output, including after object transforms.

SVG, PNG, and PDF output resolve object and layer `DrawParam` references,
including `Relative` inheritance and color/line style overrides.
Rendering skips an unresolved top-level `DrawParam` as the Java converter does;
`validate_references()` still reports it. `CGTransform` defaults an omitted
`CodeCount` to one, matching the Java model. Its `Glyphs` text can span XML
comments or CDATA without changing the glyph mapping.

OFD `TextCode` DeltaX and DeltaY advances, including compressed `g` repeats,
are applied to individual glyph positions in SVG, PNG, and PDF output.
Four-digit hexadecimal `TextCode` escapes are decoded for extraction, search,
positioning, and previews, including UTF-16 surrogate pairs. TextCode content
split across XML text nodes by comments or CDATA is read as one run. Text writers
escape literal backslashes, carriage returns, and XML-disallowed characters so their output
round-trips through the decoder. SVG previews replace XML-disallowed decoded
characters with U+FFFD while extraction retains the original character. Plain
spaces remain unescaped in Rust output for compatibility with Java ofdrw,
which reads `TextCode` content without decoding the standard's hex escapes.

Rust, Python, and Go text-page writers and direct `TextCode` editors accept
embedded U+0000 through the JSON C ABI (`ofd_write_text_page_json` and
`ofd_edit_text_code_json`). The earlier NUL-terminated C functions remain
available for callers with ordinary text.
Text weight, italic style, fill and stroke, horizontal glyph scale, and
character direction are represented in the same previews.
For embedded TrueType/OpenType sfnt fonts, `CGTransform` glyph IDs are drawn
from the font outlines, regardless of the resource filename extension. Glyph
mappings use UTF-16 positions across adjacent `TextCode` runs, including a
mapping that consumes characters in more than one run; overlapping mappings
and mappings that split a UTF-16 character are rejected. Text remains
available for extraction. Raw CFF data without an sfnt wrapper still uses the
text fallback.

## PNG 渲染

Pages can also be rendered to PNG at a requested DPI through Rust, C, Python,
Go, and Node.js. The rasterizer loads system and embedded fonts, uses a white page
background, and limits each output to 64 million pixels. PNG rendering shares
the current SVG preview's OFD drawing coverage. Repeated previews reuse up to
four parsed render trees for pages with at most eight million pixels and eight
MiB of SVG plus embedded font data. Changes to page content, embedded fonts,
or DPI create a different cache key.

## PDF 导出

Documents can be exported to a multi-page vector PDF with the same drawing
coverage. Resolved fonts are embedded with ToUnicode maps for selectable text.
OFD attachments export as PDF `EmbeddedFiles` with Unicode filenames and their
original bytes. Duplicate attachment names receive distinct name-tree keys
while retaining their filenames. Creation and modification timestamps are
written to embedded-file parameters. ASCII, non-space `Usage` values other than
`none` become PDF `AFRelationship` names, and those files are listed in the
catalog `AF` array of a PDF 2.0 output. Associated files without a usable MIME
format use `application/octet-stream` as their PDF stream `Subtype`. OFD
`GotoA` attachment links become PDF `FileAttachment` annotations referencing
those embedded files. The export does
not transfer OFD signatures.

Complete PDF conversion is still pending.

## PDF 导入与兼容范围

PDF `ri` rendering-intent operators and ExtGState `RI` entries with a name
operand select the corresponding ICC conversion for vector colors when the
profile supports it. Image XObject `Intent` names select ICC conversion for
decoded raster images, including JPEG; an omitted image intent uses
`RelativeColorimetric`. Unknown intent names also use `RelativeColorimetric`;
an unavailable ICC conversion falls back to a usable profile transform.

Indexed ICC palettes are converted separately for each rendering intent, so
vector and raw indexed image colors follow the selected intent. Device colors
retain their existing color paths; complete color management remains pending.
PDF axial and radial shadings with ICC color spaces also use the active `ri`
or ExtGState `RI` intent when converting their color stops.

For a single OFD document body, PDF export writes Title, Author, Subject,
Creator, Keywords, CreationDate, and ModDate from `DocInfo` into the PDF Info
dictionary. Multiple OFD keywords become one comma-separated PDF string;
dates are written at day precision, matching OFD `DocInfo`.
OFD outline trees export as PDF bookmarks with hierarchy, expanded state,
Unicode titles, and page destinations. Named OFD bookmark targets are resolved
against the same document body before export.

`from_pdf_bytes()` (Rust/Python), `FromPDF()` (Go), and
`ofd_from_pdf_bytes()` (C) import PDF pages containing basic vector paths,
solid RGB, gray, or CMYK fills and strokes, and RGB, gray, or CMYK images with affine
placement (including rotation, mirroring, and skew)
with raw, Flate, JPEG, or supported CCITT fax encoding. They retain page size and painting order.
PDF `Names/EmbeddedFiles` name trees import embedded file specifications as OFD
attachments, preferring the Unicode `UF` filename over `F`. Unfiltered and
Flate-compressed file streams are supported with a 64 MiB decoded size limit;
the stream `Subtype`, `Params/CreationDate`, and `Params/ModDate` become OFD
attachment metadata. Declared `Params/Size` must match the decoded byte count.

File specification `AFRelationship` names become OFD attachment `Usage`.
PDF timezone offsets are validated while the local clock fields are retained,
because OFD attachment dates do not store an offset. Unsupported filters and
malformed name trees return an error. Document, page, page-tree ancestor,
structure-tree root and nested structure-element,
supported page annotation, page resource dictionaries and their Font, Type0
descendant font, FontDescriptor, Image/Form XObject, ExtGState, ColorSpace,
Pattern, Shading, and Properties `AF` arrays
also import associated files. ColorSpace arrays are searched for associated
files on their indirect members. Structure-tree traversal is bounded to 10,000
nodes and 64 levels; repeated file-specification references are imported once.

File specifications present in the name tree or another `AF` array are
deduplicated. Page
`FileAttachment` annotations import their file specifications and become
clickable OFD attachment links at the original
page location; the PDF annotation icon artwork is not imported. A bounded scan
of objects reachable from the PDF catalog also finds `AF` arrays on form fields
and other PDF dictionaries or streams. This scan stops after 100,000 objects;
unreferenced PDF objects are not imported.

PDF link rectangles, quadrilateral coordinates, destination type names and
coordinates, annotation subtype and flags, and URI action type and target may
be indirect objects. This also applies to file-attachment annotation subtype
and flags; missing references fail import. URI targets accept UTF-8 bytes with
an optional byte-order mark, or UTF-16BE/UTF-16LE strings with a byte-order
mark, including non-ASCII paths. Malformed encodings and characters invalid
in XML fail import.

PDF `CCITTFaxDecode` imports one-bit Group 3 one-dimensional and mixed
one/two-dimensional data, and Group 4 data, including ASCIIHex filter chains.
`K`, `Columns`, `Rows`, `EndOfLine`, `EncodedByteAlign`, `EndOfBlock`, and
`BlackIs1` parameters are checked. The decoded columns and rows must match
the image dimensions; malformed or incomplete rows fail import. CCITT-compressed
explicit image masks use the same decoder. Nonzero `DamagedRowsBeforeError`
remains unsupported because damaged-row recovery is not implemented.

PDF `JBIG2Decode` imports one-bit image XObjects with optional `JBIG2Globals`
symbol dictionaries. Page and globals streams support the existing bounded
byte-filter chains, including ASCIIHex and Flate. The importer checks page
dimensions, sample depth, and global segment page associations before decoding.
The same JBIG2 decoder renders OFD resources, including embedded symbol
dictionaries that previously could produce a blank preview. The reproducible
fixtures include an MMR pattern and Adobe PDF Reference 1.6 Example 3.4.

PDF `JPXDecode` images with JP2 containers or raw JPEG2000 codestreams also
import, including the standard case where `ColorSpace` and
`BitsPerComponent` are absent. The PDF image dimensions are checked against
the codestream. Explicit device and supported calibrated or ICC color spaces
must have the same component count as the encoded image; calibrated and ICC
samples are converted to sRGB. Color-space arrays may be indirect PDF objects.

`SMaskInData=0` ignores an embedded alpha channel, `1` keeps it,
and `2` unblends preblended RGB or gray samples using an optional `Matte` array.
JPEG2000 color-key `/Mask` arrays compare the source samples at their encoded
bit depth (up to 16 bits) and produce an RGBA PNG. Preblended CMYK samples
remain unsupported.
Image dimensions, bit depth, `/Mask` and `/Decode` arrays, and their numeric
array entries can be indirect PDF objects. This also applies to soft-mask
dimensions and explicit image-mask dimensions, bit depth, and decoding polarity.

JPEG2000 `SMaskInData` and `Matte`, including the latter's components, and
the `ImageMask` flag can also be indirect.
Missing references fail import.
DeviceCMYK colors use the PDF reference's device conversion to RGB for OFD
display. CMYK and YCCK JPEGs are decoded to RGB PNG; their color-key masks
compare decoded CMYK samples and produce RGBA PNG. DeviceGray, DeviceRGB,
and DeviceCMYK JPEGs also apply nondefault `Decode` arrays after color-key
comparison. Their transformed samples are stored as PNG.

`CalGray` color spaces import for vector paint, raw/Flate images, and grayscale
JPEGs. The required white point and optional gamma are validated; gray samples
are converted to sRGB. A nonzero `BlackPoint` is rejected because that
calibration is not yet applied.
`CalRGB` also imports for vector paint, raw/Flate images, and RGB JPEGs. Its
per-channel gamma and matrix map samples to XYZ; the white point is adapted to
sRGB D65 before encoding. Nonzero `BlackPoint` is rejected.

PDF `Lab` imports for vector paint, raw/Flate images, and JPEGs, including
Indexed palettes with a Lab base. The Lab `Range` controls a/b clipping and
image default decoding; nonzero `BlackPoint` is rejected. DCT `DecodeParms`
accept `ColorTransform` 0 or 1 for three-component JPEGs; an Adobe APP14
marker in the JPEG takes precedence. Four-component JPEGs with
`ColorTransform` 1 and no Adobe marker decode YUVK into CMYK before applying
the PDF color space, `Decode`, and color-key mask.

CalGray, CalRGB and Lab parameter dictionaries, their WhitePoint, BlackPoint,
Gamma, Matrix and Range arrays, and individual numeric entries may be indirect
objects. Scalar CalGray Gamma may also be indirect. The same validation and
color conversion apply to vector paint, image samples and shading; missing or
cyclic references return errors.

Color-space family names, ICC profile `N` channel counts and Indexed `hival`
may also be indirect objects. ICC counts must still be integers matching the
embedded profile, and Indexed `hival` must be an integer from 0 through 255;
lookup length is checked against the resolved count and base space.

The PDF `cs`/`CS` and `sc`/`SC`/`scn`/`SCN` operators support DeviceGray,
DeviceRGB, DeviceCMYK, CalGray, CalRGB, and Lab, including names mapped by
the current page or Form resource dictionary. Selecting a space resets its
current color, and `q`/`Q` restore the selected spaces and colors. Vector
painting also accepts named `Indexed` color spaces backed by a byte string or
lookup stream with a supported base color space. Real indices
round to the nearest entry and clamp to the palette range. Other color space
families still return an error.

PDF `ICCBased` color spaces with one, three, or four channels use their embedded
ICC profiles to convert vector colors, indexed palettes, and raw image samples
to sRGB. Grayscale, RGB, CMYK, and YCCK JPEG images with a separate ICC color
space are converted to sRGB, including component `Decode` arrays and color-key
masks.
Page resource `DefaultGray`, `DefaultRGB`, and `DefaultCMYK` entries remap
matching device color spaces for vector painting and supported raw, Flate,
JPEG, and JPEG2000 image paths. The replacement must have the same component
count; invalid replacements fail import. JPEG2000 color conversion uses the
codestream samples before applying color-key masks and retains embedded opacity.

Form XObjects import recursively with their matrix, BBox clipping, inherited
graphics state, and local font, image, and ExtGState resources. Nested forms
restore the caller's state and cyclic references return an error. Form
transparency groups with DeviceRGB or an inherited blending space and opaque content
are imported, including isolated and knockout groups; nested Forms and image
XObjects, inline images, and supported axial or radial shadings are also
imported when their content can be checked recursively as opaque. Groups with
transparency state, image masks, or image alpha still return an error because
group compositing is not represented.

Reference XObjects import the embedded Form proxy appearance, including its
matrix and BBox. The referenced external PDF page is not fetched. A page-level
transparency group with a
DeviceRGB or inherited blending space is accepted. Isolation and knockout
flags require the page's content and referenced Forms and images to pass the
recursive opaque-content check; other page group configurations return an
error. Group attributes can be direct values or indirect PDF objects.

PDF `Text` notes without appearance streams import with their remark, icon name,
open state, RGB color, opacity, flags, author, and PDF name. OFD previews draw
a simple filled note marker; PDF export recreates an editable text note and
omits the preview marker from PDF page content. PDF viewers choose the actual
note icon and popup appearance.

PDF `Popup` annotations linked to a supported Text, text markup, Square,
Circle, or Line parent import with their rectangle and open state stored on
the OFD parent annotation. PDF export recreates the popup and its reciprocal
parent reference. A standalone popup imports as an OFD text note with its own
content and metadata; PDF export restores the `Popup` subtype without a parent.

A popup with an invalid or unsupported parent returns an error.
PDF `Square` and `Circle` annotations without appearance streams import as OFD
vector appearances and export again as editable PDF annotations. Solid or dashed
borders, opacity, rectangle
differences, flags, remark, author, and PDF name are retained. Border effects
and custom appearance streams still return an error. Gray and CMYK outline or
interior colors are converted to RGB during import. Changing the OFD annotation
appearance boundary also resizes the generated Square or Circle path and scales
its PDF rectangle differences for export.

PDF `Line` annotations without appearance streams also import as OFD vector
appearances and export as editable PDF annotations. Endpoints, solid or dashed
borders, stroke color, opacity, flags, annotation metadata, `/LE` line ending
styles, `/IC` interior color, and `/LL`, `/LLE`, `/LLO` leader-line geometry are
retained. Standard line endings and leader lines have vector OFD previews that
are redrawn when the annotation boundary changes. Captions and custom
appearances remain unsupported.

PDF `Ink` annotations without appearance streams import as one OFD annotation
with one path per stroke. The `InkList` coordinates remain relative to the
annotation rectangle, so changing its appearance boundary redraws the OFD
paths and rescales the editable PDF strokes. Solid or dashed borders, stroke
color, opacity, flags, remark, author, and PDF name survive OFD and PDF
round trips. Invalid stroke arrays and points outside the rectangle return
an error.

PDF `Polygon` and `PolyLine` annotations without appearance streams import
as editable OFD vector paths. Their `Vertices`, border, color, opacity,
flags, metadata, and supported dimension intent survive PDF export.
Polygons use `IC` as the shape fill; polylines use it for their `/LE` line
endings. OFD previews draw standard line endings. Changing the annotation
appearance boundary rescales the path and exported vertices. Border effects,
measurement dictionaries, and PDF 2.0 `Path` geometry still return an error.

PDF `Highlight`, `Underline`, `StrikeOut`, and `Squiggly` annotations without appearance
streams import as filled OFD paths derived from their `QuadPoints`. Underlines
occupy a narrow strip above the text edge; strikeouts occupy the middle of
each quadrilateral. Squiggly marks are bounded zigzag strips beside the text
edge. RGB `C` color, `CA` opacity, print, and visibility flags
are retained; missing color defaults to yellow for highlights and black for
lines. The generated OFD appearance uses normal alpha compositing, so a PDF
viewer's highlight blend behavior can differ. Malformed quadrilaterals, color,
and opacity return errors.

The original quadrilaterals, color, and opacity are also kept in annotation
parameters. PDF export uses them to recreate editable text-markup annotations
with their subtype, geometry, flags, remark, creator, and modification day.
Their OFD appearance remains available to SVG and PNG previews; PDF page
content excludes that appearance so the PDF annotation is drawn only once.

Quadrilaterals are stored relative to the OFD appearance boundary, so moving or
resizing that boundary also moves or resizes the exported PDF markup. Rust's
`add_highlight_annotation()` creates the same PDF markup metadata for native
highlights, including highlights added through the language bindings.
Supported PDF annotations also map `Contents`, `T`, and `M` to OFD remark,
creator, and last modification day. A partial `M` date without a day leaves
the OFD annotation's generated date unchanged.

PDF `Link` annotations with URI or direct page `GoTo` actions import as OFD
links, including page rotation, `UserUnit`, print, and visibility flags.
Supported PDF annotation types map the `NoZoom`, `NoRotate`, and `Locked`
flags to OFD's matching display and editing metadata. PDF `ReadOnly`
restricts interaction rather than editing, so its raw flags integer is kept
in an OFD annotation parameter named `PDFFlags`; the current renderer does
not enforce that interaction restriction. Other PDF flags without an exact
OFD equivalent are retained in the same parameter.

Named PDF destinations in the catalog's legacy `/Dests` dictionary or
`/Names` destination name tree resolve to OFD page targets. Missing and
malformed names return an error. Link `QuadPoints` import as OFD action Regions,
including multiple quadrilaterals and page rotation. Points outside the link
`Rect` are ignored as required by PDF; malformed arrays return an error.

Appearance streams and other PDF annotation subtypes still return an error.
Link border artwork is not represented in OFD.
Raw and Flate images accept 1, 2, 4, 8, or 16 bits per RGB/gray component,
including per-row byte padding and component `Decode` arrays. Sixteen-bit
samples are read in big-endian order and converted to 8-bit PNG output after
decoding; color-key masks compare the original sample values. The shared
`gray-16-bit.pdf` fixture exercises this import through Rust, C, Python, Go,
and Node.js. JPEG passthrough
requires 8-bit components and the default decode range.

Raw and packed image samples also accept `RunLengthDecode`, `ASCIIHexDecode`,
and `ASCII85Decode`, including abbreviated names and filter chains with Flate.
One-bit image masks use the same filter decoder. Decoded intermediate data and
final samples are size-limited.
`LZWDecode` accepts `EarlyChange=0` or `1`; LZW and Flate accept TIFF
horizontal-difference and PNG row predictors through `DecodeParms`. Parameter
arrays follow the filter order. Invalid codes, row lengths, and parameters
return an error rather than silently changing pixels.

Raw and Flate `Indexed` images accept the same packed bit depths, with direct or
named color spaces and string or stream lookup tables. The palette may use
DeviceGray, DeviceRGB, or DeviceCMYK. Explicit `Decode` arrays and color-key
`Mask` ranges apply to source indices before palette expansion.
Single-channel `DCTDecode` images with an `Indexed` color space also expand
through the palette; explicit `Decode` arrays and color-key masks use the
decoded source samples.

RGB/gray images with a grayscale `/SMask` image import as RGBA PNG; the mask
may have different dimensions and is resampled over the image. A `/Matte`
array on the soft mask restores preblended DeviceRGB or DeviceGray colors
before applying alpha. RGB/gray images with a `/Mask` color-key array
import as RGBA PNG for raw, Flate, and JPEG samples; ranges are compared to
the original samples before the image `Decode` mapping. A `/Mask` reference to
a one-bit image-mask XObject also imports as RGBA PNG; its `Decode` polarity,
row padding, and different mask resolution are respected. Directly painted
image-mask XObjects use the current nonstroking RGB/gray color and also import
as RGBA PNG. Graphics-state soft masks remain unsupported.

Positive PDF `/UserUnit` page scaling (up to 75,000) applies to page boxes,
paths, strokes, Type1 text, images, and clipping geometry. PDF `/Rotate`
values of 90, 180, and 270 degrees rotate page geometry, imported objects,
and clips, including objects with their own CTM; other multiples of 90 are normalized. Angles outside 90-degree
steps return an error.

Vector painting includes PDF `f*`, `B*`, and `b*` even-odd fill operators.
PDF stroke cap (`J`), join (`j`), miter limit (`M`), and positive dash arrays
(`d`) of up to 256 values are imported into OFD path styles with graphics-state
save/restore. The SVG preview preserves the full dash sequence.
The same stroke settings also import from `gs` resources through `LW`, `LC`,
`LJ`, `ML`, and `D`; `FL` flatness is validated. These settings follow PDF
graphics-state save/restore and apply to later strokes and stroked text.

`gs` `Font` arrays also select a font by indirect PDF object reference and
set its size, including when that font has no page resource name. `q/Q`
restores the earlier text font and size.
PDF flatness tolerance (`i`) is validated; imported paths keep their original
Bezier curves and the renderer determines rasterization quality.
PDF `gs` resources containing `CA` and `ca` alpha constants are imported for
paths, text, and images. Standard `/BM` blend modes are preserved for these
objects; arrays select the first supported mode and fall back to `Normal`.

Explicit default `OP=false`, `op=false`, `OPM=0`, `SA=false`, and `TK=true`
entries are accepted. `TR=Identity` and `TR2=Identity/Default` are also
accepted; nonidentity transfer functions remain unsupported. Nondefault values of these graphics-state controls
remain unsupported because they can change compositing or stroke behavior.
Soft masks and other unsupported extended graphics-state entries return an error.
Basic horizontal Type1 and simple TrueType text is
imported as OFD text, using the document's fallback font. Consecutive text
strings, `TJ` adjustments, and `Tc`/`Tw` spacing use explicit PDF font widths
when available, or the built-in advances of the fourteen standard Type1 faces
when `Widths` is omitted. One-byte `ToUnicode` CMaps override text decoding while
the original character codes still determine spacing. With a usable embedded
TrueType/OpenType program, a code may map to a Unicode sequence while retaining
one source glyph and one PDF advance. Custom and subset Type1 fonts may use an explicit named
encoding or an Encoding dictionary with a named BaseEncoding and Differences;
a complete ToUnicode CMap retains priority when supplied. Type0 CID fonts
with `Identity-H`/`Identity-V` or an embedded Encoding CMap and a
`ToUnicode` CMap also import as OFD text; descendant `DW` and both forms of
`W` determine glyph advance. Embedded TrueType/OpenType programs from
`CIDFontType2` are copied using valid PDF `CIDToGIDMap` selectors. Simple
TrueType source encodings and Differences names select their actual glyphs.
When Unicode and source glyphs differ, OFD `CGTransform` retains both the
original Unicode text and the PDF glyph sequence; DeltaX remains per glyph,
and CodeCount measures the unescaped text in UTF-16 units. This includes
ligatures, variation selectors and CID fonts without a Unicode sfnt `cmap`.
Object boundaries include transformed source glyph bounds, retaining negative
bearings and other outline overhangs instead of clipping them at the baseline.
Embedded font bytes remain unchanged and repeated runs reuse their resource.
Invalid glyph indices and odd CIDToGIDMap byte lengths return errors.
Mapped runs require known PDF advances for multiple glyphs and are limited
to 10,000 glyphs per object; decoded Unicode expansion is bounded to 200,000
UTF-16 units per text-showing run. Without a usable embedded program,
multi-character mappings remain unsupported rather than losing text or
guessing glyphs. The mapping follows
[ISO 32000-1, section 9.10.3](https://developer.adobe.com/document-services/docs/assets/35e4369068f86065372c18787171a17e/PDF_ISO_32000-1.pdf).

Simple-font `Encoding` dictionaries may provide a named `BaseEncoding`
(`StandardEncoding`, `WinAnsiEncoding`, `MacRomanEncoding` or `MacExpertEncoding`) and a
`Differences` array. Standard Latin Type1 faces use StandardEncoding when the
dictionary omits its base; custom Type1 and TrueType fonts require an explicit
encoding/base or a ToUnicode map. Unavailable built-in font encodings are not
inferred from the font name. Adobe glyph-list names, variant suffixes, `uniXXXX` and
`uXXXX`–`uXXXXXX` names recover Unicode text, including supplementary-plane
characters. Underscore-separated names, multi-group `uni` names and Adobe
glyph-list sequences retain multiple characters per source glyph when an
embedded font supplies the exact glyph and source advances. Source codes still select widths
and word spacing. ToUnicode retains precedence when present.

Encoding dictionaries, base names, difference arrays and their members may
be indirect. Unmapped names fail only if used by the text; sequence mappings
without a usable embedded font fail rather than changing the glyph layout. Parsing accepts at most 4,096 Differences entries and 1,024 bytes per
glyph name. Embedded TrueType reuse resolves overridden glyph names in the
actual program; an unavailable source glyph name retains the font fallback.
ToUnicode aliases change text semantics without replacing source outlines.
The mapping follows the [Adobe Glyph List specification](https://github.com/adobe-type-tools/agl-specification).
Named and difference encodings follow
[ISO 32000-1, section 9.6.6](https://developer.adobe.com/document-services/docs/assets/35e4369068f86065372c18787171a17e/PDF_ISO_32000-1.pdf).
MacExpert mappings retain their Unicode private-use characters for small caps
and similar glyph variants; displaying them still requires a suitable font.

Standard-font advances come from Adobe AFM data and do not depend on installed
substitute-font metrics. Explicit `Widths` retain priority, and no kerning is
implicitly added. Known standard-font names with a six-uppercase-letter subset
prefix also resolve. `Symbol` and `ZapfDingbats` use their native encodings when
no encoding is specified, including when a Differences dictionary omits its
base. ToUnicode still changes extracted text without changing source glyph
advances. These tables provide metrics, not font outlines; preview font fallback
still applies. Data provenance and permissions are retained in the
[font metric notice](../crates/ofdrw/assets/core14/NOTICE) and distributed packages.

Imported text retains the bold/italic classification of standard font names,
including valid subset prefixes. FontDescriptor `FontWeight` (100–900 in steps
of 100), `ItalicAngle` and the italic `Flags` bit also apply to substitute
fonts; descriptors and their fields may be indirect, including on CID descendants.
Malformed declared fields and broken references return errors. `ForceBold`
is a rasterization hint and does not change font weight. When the embedded
TrueType/OpenType program can be reused safely, its intrinsic style takes
precedence over contradictory descriptor metadata. Intrinsic weights map to
the nearest OFD weight; italic classification does not reproduce an arbitrary
declared slant angle in a substitute font. Styles survive OFD save/reopen.

Simple-font `FirstChar`, `LastChar`, `Widths` arrays and numeric width entries
may be indirect objects. A declared table must contain exactly one finite
width per code in its 0–255 range, with at most 256 entries. Invalid numeric
entries and unresolved or cyclic references return errors.

`BT` initializes the text and line matrices to identity, so text can start at
the default text-space origin without `Td` or `Tm`; the current graphics
transform still applies. Each new text object resets these matrices while
retaining font and spacing parameters. Continued strings require known font
advances or another explicit position. This follows
[ISO 32000-1, section 9.4.1](https://developer.adobe.com/document-services/docs/assets/35e4369068f86065372c18787171a17e/PDF_ISO_32000-1.pdf).

Type0 `Identity-H`/`Identity-V` encoding names, default CID widths (`DW`), `W` arrays,
range endpoints, per-CID width arrays and individual widths may be indirect
objects. Width validation and `TJ` spacing apply after resolution; broken or
cyclic references return errors. Width-table expansion is limited to 1,000,000
assignments per parse, counting overlapping and overwritten declarations.

Embedded Encoding CMaps accept mixed one-, two-, three- and four-byte source
codes. `begincidchar`/`begincidrange` map source codes to CIDs, while ToUnicode
uses the original source code and its byte length. CID widths and embedded
CIDToGIDMap checks use the resulting CID. `beginnotdefchar`/`beginnotdefrange`
provide fallback CIDs for otherwise unmapped codes; the final default is CID 0.
`UseCMap` inherits another embedded stream or Identity-H/Identity-V with local mapping
overrides; a `usecmap` declaration must agree with its dictionary base name.
Only descendant font number zero is accepted. Codespace matching compares
each byte's bounds and rejects overlapping or prefix-ambiguous ranges.
Parsing limits are 128 codespace ranges, 100,000 assignments (including
overwrites), one MiB of decoded CMap data across inheritance, and eight
inheritance links. Word spacing applies to a one-byte source code 32, even
when ToUnicode gives it a different character; two-byte codes do not gain
word spacing merely because they map to a space. These rules follow
[ISO 32000-1, sections 9.3.3 and 9.7.5](https://developer.adobe.com/document-services/docs/assets/35e4369068f86065372c18787171a17e/PDF_ISO_32000-1.pdf).

Vertical Identity-V and embedded CMaps with WMode 1 preserve horizontal glyph
outlines at their vertical origins. CIDFont DW2 defaults to [880 -1000]; W2
supports both arrays of metric triples and CID ranges, including indirect
values. Default origin x uses half the horizontal CID width. Character/word
spacing and TJ adjustments advance along the vertical text axis; horizontal
scaling applies to glyph outlines and origin x, while text rise and arbitrary
nondegenerate text matrices retain their original geometry. OFD DeltaX/DeltaY
store both position components, with CGTransform retaining embedded glyphs.
WMode inherits through UseCMap and local declarations override the base;
conflicting stream/program declarations or modes other than 0/1 are rejected.
Vertical metric values must be finite and within ±10,000 glyph units, and
W2 expansion is limited to 1,000,000 assignments including overwrites.
Independent Ghostscript rasters cover default/custom metrics, spacing,
mirroring and shear; Rust and Python/Go/Node tests cover text and round trips.
These metrics follow
[ISO 32000-1, sections 9.4.4 and 9.7.4.3](https://developer.adobe.com/document-services/docs/assets/35e4369068f86065372c18787171a17e/PDF_ISO_32000-1.pdf).

The native library also bundles 189 Adobe CMap resources (116 horizontal and
73 vertical),
including GBK-EUC-H, UniGB-UTF16-H, UniCNS-UTF16-H, UniJIS-UTF16-H and
UniKS-UTF16-H and their vertical counterparts. They cover the current Adobe CNS1, GB1, Japan1, Korea1, KR,
Identity and Manga1 collections and require no runtime downloads. Named
maps and embedded UseCMap bases use the same bounded parser. A named map's
Adobe Registry and Ordering must match the descendant CIDFont's
CIDSystemInfo; indirect collection metadata is accepted. Identity-H and
Identity-V remain collection-neutral. Vertical resources inherit their horizontal
base mappings and retain source-defined punctuation CID substitutions.
Up to sixteen parsed named maps share immutable tables; local overrides
copy only the table being changed, and cache hits still charge parsing budgets.
Comments may appear between mapping operands without changing percent signs
inside literal strings. Literal/array/dictionary nesting is limited to 64.

Sources are pinned to [Adobe cmap-resources](https://github.com/adobe-type-tools/cmap-resources/tree/f5cf3bca7fdfeaceb77aa82847e974f2306c20b4).
The [manifest](../crates/ofdrw/assets/cmaps/manifest.json) lists all names,
Git blob hashes and SHA-256 digests; original source headers and the
[BSD notice](../crates/ofdrw/assets/cmaps/NOTICE) are retained in distributions.

When a Type0 font omits ToUnicode, an Adobe CNS1, GB1, Japan1, Korea1 or KR
descendant collection can supply Unicode through the bundled official
CID-to-Unicode maps. This also applies to Identity-H/Identity-V and embedded Encoding
CMaps when the descendant declares a supported Adobe collection. Registry,
Ordering and a nonnegative integer Supplement are required and may be
indirect. Source codes first map to CIDs, then to Unicode; their numeric
values are not assumed to be Unicode. Five immutable Unicode maps load on
demand without network or system resources. Supplementary-plane scalars
remain intact through UTF-16 surrogate pairs. A present ToUnicode retains
priority, including errors for malformed or incomplete explicit maps.
Unknown collections and undefined mappings still return errors. Multi-scalar
mappings retain their source glyph through CGTransform when a usable embedded
program is available; Unicode variation sequences are never silently shortened.
This fallback follows
[ISO 32000-1, section 9.10.2](https://opensource.adobe.com/dc-acrobat-sdk-docs/standards/pdfstandards/pdf/PDF32000_2008.pdf).
Source mappings and original notices are pinned to
[Adobe mapping-resources-pdf](https://github.com/adobe-type-tools/mapping-resources-pdf/tree/2dd5e53fb74a01718b9dfd448a0d1cce6fff2aa5),
with [per-file hashes](../crates/ofdrw/assets/cid_unicode/manifest.json) checked
offline in CI. ToUnicode CMaps also accept single-line XUID metadata.

Other source fonts use the document's fallback font for previews. Vertical
writing and unbundled named CID CMaps remain unsupported. The `'` and `"` line operators
are supported.
Text-state operators such as `Tr` and `Tf` are accepted before `BT`; text
positioning and drawing still require an active text object. ToUnicode CMaps
with `CMapVersion` and `WMode` metadata also import.

Rotated, skewed, and nonuniform PDF text and graphics matrices are imported
into OFD text `CTM`; degenerate text matrices are rejected.
PDF Info Title, Author, Subject, Creator, Keywords, CreationDate, and ModDate
import into OFD `DocInfo`. PDF keyword text is kept as one OFD keyword; PDF
dates with a full calendar day are normalized to `YYYY-MM-DD`, while valid
partial dates have no OFD day value and are omitted. Invalid full dates and
metadata text that cannot be represented in XML return an error. Time fields
and timezone offsets in full dates are validated before their day is retained.

PDF outline trees import into OFD `Outlines`, preserving Unicode titles,
parent-child order, initial expanded state, and supported `GoTo` destinations.
Malformed parent links and cycles return an error.
PDF `W`/`W*` clipping paths are preserved for subsequent paths, images, and
text; successive clips intersect, and `q`/`Q` restore the clipping state.
PDF inline images (`BI`/`ID`/`EI`) import through the same image pipeline as
image XObjects, including RGB, grayscale, and stencil samples. Filtered inline
images support the image filter chain used for XObjects, including Flate, LZW,
RunLength, ASCIIHex, and ASCII85 decoding; filter abbreviations are expanded.

Flate and LZW predictor parameters are applied to filtered inline images.
The importer also handles filtered inline images in Form XObjects.
PDF axial (type 2) and radial (type 3) `sh` shading import as OFD gradients
for type 2 exponential functions, one-input type 0 sampled functions,
and type 4 calculator functions,
including nested type 3 stitching functions composed of these subfunctions.
The shading Function may also be an array containing one scalar function per
color component, mixing types 0, 2, 3 and 4. Direct or indirect containers and
component parameters are accepted. Each component retains real values and
its own Domain and Range until the combined color-space conversion, including
Lab and ICC rendering intents. Component count and single-output dimensions
are checked before conversion. Nested function arrays are rejected.
Single exponential functions with a linear component mapping also sample
calibrated, Lab and ICC color conversions before writing their color stops.

Function arrays retain the union of sample-table knots, cubic extrema,
function-domain boundaries and stitching bounds before adaptive sampling.
Calculator and stitching region changes preserve separate colors at jumps.
The same finite-sampling limitations described below apply. Array-wide parse
limits are 4,096 function nodes, 100,000 samples and 1 MiB of calculator code;
each component retains its existing individual limits. Evaluation is bounded
at 65,536 composite samples per array interval and 10,000 resulting color stops.

Stitching supports up to 16 nesting levels, preserves discontinuities under
reversed encoding, and applies every enclosing output `Range` in component
space before color conversion. Constant encodings select the right-hand
subfunction at an internal bound. Cyclic function references and results
exceeding the shared 10,000-color-stop limit return errors.
Function parameters and numeric array members may be indirect objects,
including exponential `N/C0/C1/Domain`, sampled parameters, and stitching
`Functions/Bounds/Encode/Domain`. Unresolved or cyclic references and invalid
array dimensions return errors; explicit malformed endpoints do not fall back
to default colors. Axial and radial indirect-reference fixtures produce the
same OFD SVG and PNG output as their direct-value counterparts.

Calculator functions implement the 42 PDF arithmetic, relational, boolean,
bitwise, conditional and stack operators with signed 32-bit integers and
finite real arithmetic. Integer arithmetic overflow promotes to real where
appropriate; invalid conversions, type errors, stack errors and undefined
arithmetic return errors. Conditional blocks short-circuit. Input Domain,
output Range, enclosing stitching ranges, and color-space component bounds
apply before color conversion. The same bounded byte-filter chain decodes
calculator streams, including Flate compression.

Calculator gradients use 64 initial intervals followed by bounded adaptive
color-stop sampling. Changes in conditional branches, rounding, integer
conversion and the atan angle region guide subdivision; located discontinuities
retain separate left/right stops. This finite sampling does not exhaustively resolve oscillations
between samples. Limits are 64 KiB of code, 4,096 instructions, 32 nested
conditional levels, 256 stack operands, 65,536 evaluations per calculator
interval, and 10,000 color stops. Complex expressions exceeding these limits return an
error. Axial and radial fixtures exercise the feature through all language
bindings; the axial pixel regression uses an independent Ghostscript render.

Sampled functions support linear and cubic (Order 3) interpolation, 1/2/4/8/12/16/24/32-bit
samples, bounded byte-filter chains, reversed `Encode` and `Decode` mappings,
and input/output clipping. Every sample-table knot is retained before adaptive
color conversion so narrow color transitions are not skipped. Cubic functions
retain interior turning points and clipping thresholds before adaptive sampling;
table endpoints repeat their nearest sample. Tables with fewer than four samples
use linear interpolation as required by PDF. An independent Ghostscript render
checks cubic axial shading. Multi-input sampled functions remain unsupported.
Nonlinear color conversions use bounded adaptive color stops.

Stitching bounds, reversed `Encode` intervals, and the stitching function's
output `Range` become explicit OFD color stops. Nondefault shading domains are normalized to
the OFD gradient axis, including domains that clip stitching intervals.
Inputs outside a type 2 or stitching function's `Domain` use the nearest
boundary value, including when the entire visible interval is outside it.

Linear subfunctions with an output `Range` add color stops at clipping
thresholds, preserving flat color intervals and transitions.
Lab shading functions retain L* in 0–100 and a*/b* within the color space's
declared `Range` when sampling and inserting clipping stops.
Both shading `Extend` flags map to OFD's four extension modes, including the
PDF default of no extension. Radial circles must be nested, and their transform must preserve circles;
increasing and decreasing radius order are supported. The current clipping
path, graphics transform, fill alpha, and blend mode are retained. Other
shading types return an error.

An optional PDF shading `BBox` is transformed and intersected with the
current clipping path. For direct `sh` painting, `Background` is ignored as
required by PDF.
Nonzero PDF `Tz` horizontal text scaling maps to OFD `HScale`, including
scaled glyph advances, character spacing, and `TJ` adjustments. Negative
scaling mirrors the text through its OFD `CTM`; zero scaling is rejected.

PDF `Ts` text rise and `Tr` modes 0–3 map to OFD text position, fill, stroke,
and no-paint text; RGB/gray text colors and separate fill/stroke alpha are
preserved. Modes 4–7 also preserve glyph clipping after `ET`, including the
union of multiple text runs and `q`/`Q` clip restoration. Zero-width text
strokes use a 0.1 mm display width, matching imported zero-width paths.

Transparency, graphics-state soft masks, and other
unsupported painting operators currently return an error; this importer is
not yet a complete PDF converter.
PDF `BX`/`EX` compatibility sections may be nested. Unknown extension
operators within them are ignored; known PDF operators that the importer
cannot render still return an error.
PDF `MP`, `DP`, `BMC`, `BDC`, and `EMC` marked-content operators are accepted
for nonoptional content, including named property lists and nesting across
complete text objects. Their structural metadata is not copied to OFD.

Optional-content (`/OC`) sequences, image/Form XObjects, and Link annotations use the default
configuration's OCG `BaseState`, `ON`, and `OFF` lists. Hidden text still
advances the text position, and hidden drawing retains graphics-state changes.
The OCG list, intents, and `BaseState` may be indirect PDF objects.
OCMD membership policies (`AnyOn`, `AllOn`, `AnyOff`, `AllOff`) and `VE`
visibility expressions are evaluated from those group states, including OCMD
dictionaries embedded directly in named properties or XObject `OC` entries. Automatic
usage states (`AS`) are ignored for this fixed-output import. OCG `View` and
`Design` intents, plus `All`, are matched against the default configuration;
groups without a matching intent do not affect visibility. Unknown named
intents are ignored, allowing future extensions alongside known intents.

When the catalog omits `OCProperties`, optional-content markers are ignored
and their drawing is imported as visible.
The current SVG-to-PDF text path can reorder Arabic letters in text extracted
from the PDF; the OFD `TextCode` and its extracted text retain the original
order. PDF extraction of right-to-left text still needs a dedicated fix.

## 模板与图层

Template page sources can be listed, template text can be included in text
extraction, and page geometry falls back through template and document defaults.

Template references also resolve a unique ASCII case-insensitive ZIP entry
when the exact path is absent, as in Java's `v4signed.ofd` fixture; ambiguous
matches return an error, and source paths report the actual ZIP entry name.
Duplicate `TemplatePage` IDs in one document body are rejected during page
geometry and source reading, and reported by reference validation.

`page_layers()` (Rust/Python), `PageLayers()` (Go), and
`ofd_page_layers_json()` (C) expose page and template layers in OFD painting
order, including optional name, layer type, draw parameter, source path, and object count.
`promote_page_to_template()` / `PromotePageToTemplate()` moves an existing
page's content into a template beside its page XML, registers it in
`CommonData`, and applies it to the source and selected pages. This keeps
relative page-resource paths valid and avoids duplicate graphic IDs.

`apply_page_template()` / `ApplyPageTemplate()` attaches that template to
later pages in the same document body. The C functions are
`ofd_promote_page_to_template()` and `ofd_apply_page_template()`.
`remove_page_template()` / `RemovePageTemplate()` removes a page reference.
`remove_document_template()` / `RemoveDocumentTemplate()` removes a definition
only after every page in that document body stops using it. The template XML
and allocated ID remain in the package. The C equivalents are
`ofd_remove_page_template()` and `ofd_remove_document_template()`.

Promotion rejects signed source documents and source pages that already use
templates.
`page_objects()` (Rust/Python), `PageObjects()` (Go), and
`ofd_page_objects_json()` (C) list direct text, path, image, and composite
placements across page and template layers in painting order. Each entry
includes its source and layer, boundary, CTM, visibility, alpha, draw parameter,
and resource ID. Composite contents remain accessible through the dedicated
composite readers.

Page object and layer edits accept numeric ID aliases such as `4` and `04`;
duplicate numeric IDs cause ambiguous edits to fail without changing the package.
Graphic objects nested in one or more `PageBlock` containers are included in
page object, text, path, image, paint, stroke-style, layer-count, and action
readers. SVG, PNG, and PDF previews render those objects in their page order;
`PageBlock` itself has no graphic transform or paint.

Object attribute, text, path, image, paint, and action edits can target a
graphic object inside a `PageBlock` by ID. Such objects can also be moved to
another page layer, lifted to their current layer, reordered within their
container, or removed.
SVG, PNG, and PDF rendering follows the same background, body, and foreground
order even when the XML lists layers in another order.

`add_page_layer()` and `move_page_object_to_layer()` (Rust/Python),
`AddPageLayer()` and `MovePageObjectToLayer()` (Go), and the corresponding C
functions create background, body, or foreground layers and move existing
graphic objects between page layers. New objects enter the body layer by
default. Optional layer `DrawParam` IDs must resolve in page resources.
`remove_page_layer()` / `RemovePageLayer()` and the C equivalent remove only
empty page layers; assigned IDs remain reserved to prevent reuse.

Document metadata and custom data can be read; standard plain-text metadata
fields can be updated without rebuilding unrelated `OFD.xml` nodes.
Metadata readers, editors, and reference validation ignore same-named elements
in extension namespaces while accepting the standard and legacy OFD namespaces.
Namespace-free OFD files remain readable. In namespaced files, selected legacy
namespace-free annotation, bookmark, and page-box fields are accepted when no
same-named OFD namespace sibling exists.

Version descriptions and signature indexes/descriptions apply the same rule to
document roots, file lists, signed values, and protected-file references.
Page tree lookup, page geometry, and page edits also ignore same-named extension
elements, including extension pages inside a real `Pages` list.
Bookmark and outline readers, editors, page deletion guards, and navigation
reference checks likewise ignore extension and stray namespace-free lookalikes.

Document and page action readers and editors apply the same namespace rule to
action lists, targets, and regions. Page-object actions and PDF link export also
ignore extension lookalikes, as do attachment action reference checks.
Page text, keyword positioning, object and layer lists, and SVG rendering skip
graphic content in extension namespaces.
Clip reading, editing, validation, and rendering ignore extension `Clips`,
`Clip`, `Area`, `Path`, and `Text` lookalikes.

Resource readers and reference validation select OFD font, color-space,
palette, draw-parameter, multimedia, pattern, and composite definitions when
extension nodes reuse their names or IDs.
Page resource links and template references, definitions, and edits ignore
same-named extension nodes, including ones with matching IDs.
Pattern cell rendering and page, template, annotation, and composite object
validation ignore extension graphic objects and their IDs.

Annotation indexes, appearances, and metadata edits likewise select OFD
elements when extension nodes reuse the same names or IDs.
`CreationDate` and `ModDate` setters require valid `YYYY-MM-DD` calendar dates;
`DocUsage` accepts `Normal`, `EBook`, `ENewsPaper`, or `EMagzine`.
New packages use `OFD.xml` Version `1.2` and DocType `OFD`. Root attributes are
available through `package_info()` (Rust/Python), `PackageInfo()` (Go), and
`packageInfo()` (Node.js). The matching `set_package_header()` /
`SetPackageHeader()` / `setPackageHeader()` methods update both attributes in
unsigned packages; signed, encrypted, and integrity-protected packages reject
the structured edit.

Structured Rust editors use a guarded package-entry writer and reject changes
while `OFDEntries.xml` integrity protection is present. Raw `set_entry()` and
`remove_entry()` remain explicit escape hatches for package repair.
`validate_references()` reports `invalid_package_version` for a missing or
malformed numeric major.minor version and `invalid_document_type` unless the
type is `OFD` or `OFD-A`. Legacy numeric versions remain readable.
The required `DocID` may be changed through the same setter using a unique
32-digit hexadecimal UUID; removing it or reusing another body's ID is rejected.

The reader accepts packages with malformed IDs for inspection;
`validate_references()` reports `invalid_doc_id` and `duplicate_doc_id`.
Empty or omitted IDs in older Java OFD samples remain accepted for reading.
`document_info()` and its Python, Go, and Node.js wrappers return an empty
`doc_id` for those legacy packages while preserving the other metadata.
Reading also accepts standard hyphenated UUIDs and compares them with compact
UUIDs when checking duplicates.

The optional `DocInfo/Cover` location is exposed alongside those fields.
Setting it through `set_document_info_field()` requires an existing package
entry, and `validate_references()` reports missing or invalid cover locations.
`remove_document_info_field()` / `RemoveDocumentInfoField()` removes an optional
standard field. `add_document_keyword()` / `AddDocumentKeyword()` appends a
keyword, and `remove_document_keyword()` / `RemoveDocumentKeyword()` removes
one by zero-based index. C equivalents use the `ofd_` prefix.

## 文档版本快照

`versions()` (Rust/Python), `Versions()` (Go), and `ofd_versions_json()` (C)
read each `DocBody/Versions` entry and its referenced `DocVersion`, including
the version ID, index, current flag, document root, and file list. Reference
validation reports malformed version descriptions and missing listed files.
File paths split by XML comments or CDATA remain readable and extractable.
`add_version_snapshot()` (Rust/Python), `AddVersionSnapshot()` (Go), and
`ofd_add_version_snapshot_json()` (C) copy current package files into an
independent version tree, write its `DocVersion` file list, and mark it as the
default version. Previous snapshots keep their file bytes when the live
document changes. The file list includes a standalone `OFD.xml`, so the
snapshot files can be extracted as a separate OFD package. The writer converts
known absolute package references, including `DocInfo/Cover`, to paths relative
to the copied files, including references split by XML comments or CDATA,
rejects unclassified absolute references, and rejects signed packages because
copying their paths would invalidate signatures.

`extract_version_snapshot()` (Rust/Python), `ExtractVersionSnapshot()` (Go),
and `ofd_extract_version_snapshot()` (C) reconstruct a standalone OFD from one
`versions()` entry. When the version lists a standalone `OFD.xml`, every listed
file must be present under its snapshot directory. For a version without
`OFD.xml`, listed package paths are retained and a single-body `OFD.xml` is
generated from its `DocRoot` and the live body's `DocID`. Missing listed files
still cause an error. The live package is unchanged.

## 书签与大纲

Named bookmarks and hierarchical outlines can be read, including GoTo page
destinations and bookmark targets. Bookmarks and nested outlines can be added
to new or existing documents while retaining unrelated XML. Named bookmarks
can be removed from one document body; outline removal deletes the selected
node and its descendants by the current flattened index. Both edits reject
signed document bodies. Bookmark removal also rejects a name still used by a
GoTo action in the document, a live page or template, or an annotation.

`set_outline_title()` and `set_outline_expanded()` (Rust/Python),
`SetOutlineTitle()` and `SetOutlineExpanded()` (Go), and their Node.js camelCase
equivalents edit an existing node while retaining its actions and children.
`set_outline_destination()` / `SetOutlineDestination()` /
`setOutlineDestination()` retargets a direct GoTo action to a page in the same
document body. Other actions and child nodes remain in place; the C ABI uses
`ofd_set_outline_destination_json()`.
`set_outline_bookmark()` / `SetOutlineBookmark()` /
`setOutlineBookmark()` points the same action at a named bookmark in the
document body; the C ABI uses `ofd_set_outline_bookmark()`.

Written destination coordinates use OFD `Dest` child elements, and the Java
`CT_Dest` reader recognizes their values.
`set_document_custom_data()` / `SetDocumentCustomData()` creates or updates a
named `CustomData` value in `DocInfo`; `remove_document_custom_data()` /
`RemoveDocumentCustomData()` removes it. The C functions are
`ofd_set_document_custom_data()` and `ofd_remove_document_custom_data()`.
Edits to `OFD.xml` metadata reject signed packages.
Page annotation metadata, appearance boundaries, attributes, and appearance
text can be read through the same Rust/C/Python/Go interface. Text watermark
annotations can be added to new or existing pages; annotation indexes and
page annotation files are updated while preserving unrelated XML.

Annotation `Creator` can be updated and optional `Remark` content can be set
or removed through Rust, Python, Go, and the C ABI. Remark is returned with
annotation metadata; these edits preserve the appearance and unrelated XML.
Named annotation `Parameters/Parameter` values can also be read, added,
replaced, and removed through the same four layers. Remarks and parameter
values split by XML comments or CDATA are read and replaced as complete text.

Optional annotation `Subtype` can be set or removed, and `LastModDate` can be
read or set to a validated `YYYY-MM-DD` date through Rust, C, Python, and Go.
New annotations include the required `LastModDate` using the current UTC date.
`set_annotation_appearance_boundary()` (Rust/Python),
`SetAnnotationAppearanceBoundary()` (Go), and the matching C export move or
resize an existing annotation appearance in page millimeters.

Rust `add_styled_text_watermark()`, Python's optional watermark arguments,
Go `AddStyledTextWatermark()`, and the C JSON writer support font selection,
RGB color, OFD alpha, rotation, bold, and italic. Optional `tile_spacing`
gives horizontal and vertical grid steps in millimeters and writes one text
object per grid position under a single `Watermark` annotation; the default
still writes one text object. A page is limited to 4,096 tiles.

`add_text_watermark_to_all_pages()` (Rust/Python),
`AddTextWatermarkToAllPages()` (Go), and
`ofd_add_text_watermark_all_pages_json()` (C) cover each existing page using
its effective page box. The operation is atomic across document bodies and
returns annotation IDs in page order.
Image watermarks use a page-visible image resource or add one from image bytes.
Rust `add_image_watermark()` / `add_image_watermark_from_bytes()`, Python
`add_image_watermark()`, Go `AddImageWatermark()` /
`AddImageWatermarkFromBytes()`, and C `ofd_add_image_watermark_json()` accept
an appearance boundary and OFD alpha (0–255). Adding from bytes commits the
resource and annotation together; invalid image data leaves the document
unchanged.

`add_image_watermark_to_all_pages()` (Rust/Python),
`AddImageWatermarkToAllPages()` (Go), and
`ofd_add_image_watermark_all_pages_json()` (C) apply the same image and boundary
to each existing page atomically, reusing one image resource per document body.
Basic annotation appearances are included in SVG previews.
`remove_annotation()` / `RemoveAnnotation()` / `ofd_remove_annotation()`
removes a page annotation by ID. The page annotation file and allocated IDs
stay in the package. Annotation edits reject signed document bodies.

Annotation reads and edits resolve numeric ID aliases; duplicate numeric IDs
are reported as ambiguous.
Font resources and embedded font bytes can be read from PublicRes and
DocumentRes files. Newly created documents can embed a TrueType or OpenType
font and reference it from a text page. Existing document bodies can also embed
fonts with `add_embedded_font_to_document()` (Rust/Python),
`AddEmbeddedFontToDocument()` (Go), or `ofd_add_embedded_font_to_document()` (C).

`add_embedded_font_to_page()` (Rust/Python), `AddEmbeddedFontToPage()` (Go),
`addEmbeddedFontToPage()` (Node.js), and `ofd_add_embedded_font_to_page()` (C)
create a page-specific `PageRes` and font file. The page link, resource,
payload, and document `MaxUnitID` update together; other pages keep their
resource lists. The returned ID can be used by positioned text on that page.
Font references compare numeric IDs, so aliases such as `2` and `02` resolve
to the same resource in shaping, text edits, flow, tables, clips, and patterns.

A document whose `MaxUnitID` is below an existing ID rejects this allocation.
`set_font_attribute()` (Rust/Python), `SetFontAttribute()` (Go),
`setFontAttribute()` (Node.js), and `ofd_set_font_attribute()` (C) edit a
document font by index. They accept `FontName`, `FamilyName`, `Charset`,
`Italic`, `Bold`, `Serif`, and `FixedWidth`; passing no value clears an
optional attribute. Invalid values leave the resource unchanged. Font bytes,
unknown XML, and unrelated resources are preserved.

`set_page_font_attribute()` (Rust/Python), `SetPageFontAttribute()` (Go),
`setPageFontAttribute()` (Node.js), and `ofd_set_page_font_attribute()` (C)
select by index in `page_fonts()` and also edit page-local or template
fonts. Editing a shared resource file affects every page that uses it.
`replace_font_data()` and `replace_page_font_data()` (Rust/Python),
`ReplaceFontData()` and `ReplacePageFontData()` (Go), the matching Node.js
methods, and `ofd_replace_font_data()` / `ofd_replace_page_font_data()` (C)
replace an embedded TrueType or OpenType font at its existing package path.

The file extension must match the replacement format when it is `.ttf` or
`.otf`. Both font insertion and replacement parse the first font face to reject
truncated or malformed files. Invalid data and edits to signed or integrity-protected entries are
rejected without changing the package. A shared font file changes for every
page that references it.

Resource lookup reads every declared PublicRes and DocumentRes file in order.
`page_fonts()` and `page_font_data()` (Rust/Python), `PageFonts()` and
`PageFontData()` (Go), and matching C functions expose the effective fonts
from a page and its templates, including page-local `PageRes` fonts.
Positioned, shaped, and composite text writers resolve the target page's
`PageRes` fonts before document fonts, matching the reading and rendering
resource order.

## 签名与电子印章

Signature descriptions, protected file lists, stamp placement, signature
values, and electronic seal bytes can be read. Protected file digests can be
compared using SM3 or SHA-256. Protected `CheckValue` values are decoded as
XML Base64, allowing spaces, tabs, and line breaks without changing the digest;
malformed values fail their individual reference check. Adding another
signature reads a split `MaxSignId` as one reserved ID value. Scalar values read
through the shared XML helper, including signature paths, reference digests,
DocRoot paths, DocInfo fields and lists, page boxes, page path commands,
attachment locations, and resource paths, concatenate direct text nodes separated by comments or CDATA
boundaries. For SES v4/v5
seals, the SM3 digest of
`Signature.xml` can be compared with the digest embedded in `SignedValue.dat`,
and the SM2 signature over `TBS_Sign` can be checked against the embedded
certificate public key using the default SM2 distinguishing identifier.

If a protected package entry is missing, its reference check has `valid=false`
and an empty `actual` digest; other protected entries are still checked.
Reference validation reports the missing path. A declared but missing detached
seal makes `check_signature_seal_match()` return false.
When an SES signature references a detached seal file, `check_signature_seal_match()`
(Rust/Python), `CheckSignatureSealMatch()` (Go), and the matching C function
compare it with the seal embedded in `SignedValue.dat`. The combined trust
reports include `seal_match` and require it to pass when present. This check
does not authenticate the seal issuer's signature. `check_ses_seal_signature()`
(Rust/Python), `CheckSESSealSignature()` (Go), and the matching C function
verify the embedded seal's SM2 signature against its embedded issuer
certificate, with an optional separate distinguishing ID. Combined reports
include `seal_signature_valid` using the standard SM2 ID and require it to pass
for SES v4/v5 seals. A nonstandard seal ID can be checked with the standalone
method.

Certificate possession does not establish trust in the seal issuer; its
certificate chain is still unchecked. `check_ses_seal_certificate_validity()`
(Rust/Python), `CheckSESSealCertificateValidity()` (Go), and the matching C
function check the embedded seal issuer certificate's X.509 validity interval
at a caller-supplied UTC Unix timestamp. Combined reports include
`seal_certificate_validity_valid` and require it to pass for SES seals. This
timestamp is not independently trusted, and validity alone does not establish
trust in the seal issuer.

`check_ses_seal_issuer_ordered_sm2_chain()` (Rust/Python),
`CheckSESSealIssuerOrderedSM2Chain()` (Go), and the matching C function check
the embedded seal issuer certificate against up to eight explicitly ordered
SM2 intermediates and a caller-trusted anchor. The check enforces issuance
signatures, validity, CA constraints, and key usage. It is independent of the
seal signature check; callers should require both results. The existing
combined signer reports do not include this separate seal issuer chain because
they accept only the signer trust anchor. This bounded chain check does not
perform path building, revocation, certificate policy, name constraint, or
trusted time validation.

`verify_signature_with_ordered_sm2_chains()` (Rust/Python),
`VerifySignatureWithOrderedSM2Chains()` (Go), and the C JSON report accept
separate signer and seal issuer anchors and ordered intermediate lists. Their
`all_valid` requires the protected file checks, OFD signature, seal signature,
certificate validity, and both applicable chains. Raw signatures have no seal
issuer chain and must supply empty seal inputs. Separate SM2 distinguishing
IDs can be supplied for the signer certificate, OFD signature, seal issuer
certificate, and seal signature.

`check_ses_seal_signer_authorization()` (Rust/Python),
`CheckSESSealSignerAuthorization()` (Go), and the C equivalent compare the
embedded signer certificate with the seal's certificate list or its SM3 or
SHA-256 digest list. `check_ses_seal_validity()` / `CheckSESSealValidity()`
check the seal's own creation and validity dates at a caller-supplied time.
Both results appear in combined reports as `seal_signer_authorized` and
`seal_validity_valid`, and `all_valid` requires them for SES signatures.

Raw SM2 digital signatures can be checked against a caller-supplied DER X.509
certificate. The library verifies the signature over the original
`Signature.xml` bytes and exposes the result through Rust, C, Python, and Go.
`check_signature_certificate_validity()` (Rust/Python),
`CheckSignatureCertificateValidity()` (Go), and
`ofd_check_signature_certificate_validity()` (C) check the signer's X.509
validity interval at a caller-supplied Unix timestamp. SES signatures use the
embedded certificate; raw signatures require a supplied DER certificate.

The timestamp is caller-supplied and is not independently trusted.
`check_signature_direct_trust_anchor()` (Rust/Python),
`CheckSignatureDirectTrustAnchor()` (Go), and
`ofd_check_signature_direct_trust_anchor()` (C) verify one direct SM2 X.509
issuance step against a caller-trusted certificate, including issuer name,
certificate signature, validity periods, CA constraints, key usage, and the
OFD signature value. The certificate-signing SM2 distinguishing ID is supplied
separately; OpenSSL 3 certificates in the tests use an empty ID. This check
does not validate intermediate chains, revocation, policies, name constraints,
trusted time, or protected file digests. The Java repository's root test
certificate lacks `keyCertSign` usage, so this strict check rejects it even
though the raw digital signature can be verified.

`verify_signature_with_direct_trust_anchor()` (Rust/Python),
`VerifySignatureWithDirectTrustAnchor()` (Go),
`verifySignatureWithDirectTrustAnchor()` (Node.js), and
`ofd_verify_signature_with_direct_trust_anchor_json()` (C) combine protected
file digests, the SES description digest when applicable, the signature value,
signer validity, and the direct trusted issuer into a report with `all_valid`.
An empty protected file list makes `all_valid` false. The caller still chooses
the trust anchor and evaluation time; the one-step and revocation limits above
also apply to this report.

`check_signature_ordered_sm2_chain()` and
`verify_signature_with_ordered_sm2_chain()` (Rust),
`verify_signature_with_ordered_sm2_chain()` (Python),
`VerifySignatureWithOrderedSM2Chain()` (Go),
`verifySignatureWithOrderedSm2Chain()` (Node.js), and
`ofd_verify_signature_with_ordered_sm2_chain_json()` (C) accept an explicitly
ordered list of up to eight intermediate SM2 X.509 certificates. They check
each issuance signature, validity interval, CA usage, and path length before
reporting the protected file and OFD signature checks. The caller supplies
the trust anchor and time. The C function accepts a JSON array of base64 DER
intermediates. This bounded check does not perform path building, revocation,
certificate policy, name constraint, or trusted time validation.

The default OFD signature SM2 distinguishing ID is `1234567812345678`.
Rust exposes `*_with_distid` signing and verification methods plus report
methods ending in `_and_distids`; Python accepts an optional `signature_distid`
argument, and Go exposes corresponding `WithDistID` / `AndDistIDs` methods.
The X.509 certificate signature ID remains separate from the OFD signature ID.
Callers must retain any non-default OFD ID for subsequent verification because
the generated raw `Sign` entry does not encode it.

Complete certificate path trust, revocation, and other signature formats are
pending.
`verify_signature_with_certificate_pool()` also searches up to 16 unordered
intermediate certificates for a path to a caller-trusted anchor, then runs the
existing protected-file, signature-value, and signer-chain checks. It accepts
a supplied signer certificate for raw signatures and uses the embedded signer
certificate for SES signatures. Python exposes the same method; Go uses
`VerifySignatureWithCertificatePool`, and C uses
`ofd_verify_signature_with_certificate_pool_json`. Node.js uses
`verifySignatureWithCertificatePool()`. The bounded search does not
perform revocation, policy, name-constraint, or trusted-time validation.

For SES signatures, `verify_signature_with_certificate_pools()` builds the
signer and seal issuer paths from separate candidate lists and returns both
trust results. Python exposes the same method; Go uses
`VerifySignatureWithCertificatePools`, and C uses
`ofd_verify_signature_with_certificate_pools_json`.
For single-document packages,
`add_digital_signature()` (Rust/Python), `AddDigitalSignature()` (Go), and
`ofd_add_digital_signature()` (C) create a raw SM2 signature. The caller passes
a 32-byte private key scalar and a claimed `YYYY-MM-DD HH:MM:SS` signing time.

The writer hashes existing package files with SM3 after adding the
`Signatures` link to `OFD.xml`, then signs the generated `Signature.xml`.
Additional signatures can be appended if previous protected file digests
still match and no prior signature protects `Signatures.xml`. Appending retains
the old `OFD.xml`, descriptions, and signature values while updating the
signature list; new signatures protect prior signature files. Existing
`Signatures` index paths split by XML comments or CDATA are resolved in full.

New signature file paths must not collide with existing package entries,
including names that differ only by ASCII case. Appending skips occupied
signature directories; a collision on the first signature leaves the package unchanged.
The matching certificate is kept outside the OFD and must be supplied for
verification. `add_ses_signature()` (Rust/Python), `AddSESSignature()` (Go),
and `ofd_add_ses_signature()` (C) create SES v4/v5 signatures from an existing
signed seal, the authorized signer certificate, and its matching SM2 private
key. The writer embeds the SES signed value, stores the seal as `Seal.esl`, and
includes the seal in the protected file hashes. Separate signer and seal SM2
IDs are supported. Rust provides `add_ses_signature_with_stamp()`, Python
accepts `page_index=` and `boundary=` on `add_ses_signature()`, and Go provides
`AddSESSignatureAtPage()`, and Node.js accepts `{pageIndex, boundary}` on
`addSesSignature()`. A page index and millimeter boundary place a visible
`StampAnnot`; Rust SVG, PNG and PDF output draw raster, SVG and embedded OFD
seal pictures there. Existing `StampAnnot.Clip` rectangles, including the
strips used by multi-page riding stamps, are exposed in signature metadata and
applied during rendering. Embedded OFD seal pages are rasterized with a
transparent background before placement; ordinary page PNG export keeps its
white background.

The writer checks seal signature and signer authorization before changing the
document. `create_ses_seal()` (Rust/Python), `CreateSESSeal()` (Go),
`createSesSeal()` (Node.js), and `ofd_create_ses_seal_json()` (C) generate SES v4/v5 seals from an issuer key,
matching issuer certificate, authorized signer certificates, validity window,
and picture. They do not establish certificate trust or validity at the
claimed time. SVG seal pictures are rasterized with a 16 million
pixel limit. Malformed signature appearances are skipped during page rendering;
call the signature verification APIs to inspect validity.

The Java interop script checks two Rust-created raw signatures and SES v4/v5
signatures using both Java-provided and Rust-generated seals with the original
Java `OFDValidator`.

## 引用和结构验证

`validate_references()` (Rust/Python), `ValidateReferences()` (Go), and
`ofd_validate_references_json()` (C) report page object IDs and boundaries,
font and media references, template links, and attachment, annotation, and
signature entry errors, including composite and `DrawParam` references.
It also reports unknown `DefaultCS` IDs and explicit `ColorSpace` IDs in page,
template, resource, and annotation paint as `default_color_space_reference`
or `color_space_reference`. Validation continues when a linked resource file
is missing, so that file and independent reference issues are both reported.

Invalid palette indexes and color channel counts or values are reported as
`invalid_color_index` and `invalid_color_value` according to the resolved
ColorSpace type and `BitsPerComponent`.
Clip `Area` references to `DrawParam` and clip `Text` font references are
checked in pages, templates, composite units, and annotation appearances.
Referenced public, document, page, and template resource files are checked for
missing, invalid, duplicate, and out-of-range IDs on Font, MultiMedia,
DrawParam, ColorSpace, CompositeGraphicUnit, and Pattern definitions. Numeric
aliases count as the same ID within a resource file or across files in the
same `DocumentRes` or `PublicRes` list of a document body. Page resources may
reuse IDs to override document resources; a shared file is checked once.

Attachment IDs must be positive integers within `MaxUnitID`; numeric aliases
such as `2` and `02` are treated as the same ID for duplicate detection and
`GotoA` references. Missing attachment files are reported individually while
validation continues through the remaining attachments. Invalid attachment IDs
and empty names are reported as `invalid_attachment_id` and
`invalid_attachment_name`.
PageBlock and graphic object IDs are checked as positive integers. Layer ID 0
is accepted for compatibility with Java-generated OFD fixtures. Numeric aliases
are treated as duplicates within and across pages of the same document body.

Abbreviated path data must begin with an S or M move command; a path starting
with a draw or close command is reported as `invalid_path_data`.
Optional IDs on clipping `Path` and `Text` elements also participate in these
checks and in the `MaxUnitID` bound for pages, templates, composite graphic
resources, and annotation appearances. Layer and graphic object IDs inside
composite graphic resources are checked once per resource file for duplicates,
document-wide collisions, and `MaxUnitID` bounds even when several pages use
the same resource.

Referenced template object IDs are checked for required values and `MaxUnitID`
bounds, and collisions with page objects are reported. Repeated IDs within one
template are accepted for compatibility with Java-generated invoice files; a
shared template is checked once.
Each document body has an independent ID space.
Page IDs are checked as nonzero integers unique within each document body;
numeric aliases such as `2` and `02` are considered duplicates, and page
references are compared by their numeric value.

Optional `PageBlock` IDs are checked for duplicates and `MaxUnitID` bounds;
blocks without an ID remain valid.
Template page objects are also checked for font and composite references,
boundaries, and required path data, including objects inside `PageBlock`.
Annotation appearances receive the same font, composite, `DrawParam`, object
boundary, and path-data checks; their own appearance boundary is validated.

Text objects on pages, templates, annotation appearances, and inside composite
units are also checked for valid size and finite `TextCode` coordinates and
advances. A text object must have a `TextCode`, and its first run must declare
both X and Y. They report malformed positioning as `invalid_text_position`,
unsupported typography values as `invalid_text_typography`, and inconsistent
`CGTransform` mappings as `invalid_cg_transform`.

Annotation and appearance object IDs are checked for positive numeric values,
`MaxUnitID` bounds, and collisions with page, template, and other annotation
objects in the same document body.
Composite graphic resources are also checked for nested image, font, path,
composite, boundary, and drawing-parameter references, unit dimensions,
required content, and cyclic composite references.
They also report missing page targets in document, page, template, and
annotation `Dest` elements, annotation page records, and signature stamps,
plus missing named bookmarks in GoTo actions.

Results contain `path`, `code`, and `message`.
Each document body must have one `DocInfo`; validation reports
`missing_doc_info` or `duplicate_doc_info` otherwise. Metadata reads and edits
reject ambiguous `DocInfo` elements, while an absent legacy `DocID` is read as
an empty string. Repeated standard metadata elements report
`duplicate_doc_info_field`, and unrecognized `DocUsage` values report
`invalid_doc_usage`. Date validation is limited to newly written metadata;
existing Java samples use several date formats.

Metadata validation joins direct XML text nodes for `DocID`, `DocUsage`, and
`Cover`; replacing a `CustomData` value removes its entire prior XML content,
including comments and CDATA sections.
`DocInfo/Cover` resolves a unique package entry with the same case matching
rules as other OFD references; missing, invalid, and ambiguous locations
report `cover_reference`.
This checks references in the supported subset; it is not a complete OFD
conformance or cryptographic signature check.

## OFD-A 检查与转换

`archive_violations()` (Rust/Python), `ArchiveViolations()` (Go), and
`ofd_archive_violations_json()` (C) report the currently supported OFD-A
checks: `DOC_TYPE`, `SINGLE_DOC`, `PERMISSION`, `V_PREFERENCES`, `EXTENSION`, `AUDIO_VIDEO`,
`PAGEBLOCK_DEPTH`, `IMAGE_INTERPOLATE`, `TEXT_SIZE`, `TEXT_HSCALE`, and
`EXTERNAL_RESOURCE`, `NON_GOTO_ACTION`, `ATTACHMENT`, `COLOR_PROFILE`,
`COLOR_SPACE`, `IMAGE_FORMAT`, `SIGNATURE`, `ANNOTATION`, `CLIP_AREA`,
`FONT_SUBSET`, `IMAGE_EXTENSION`, `IMAGE_RESOURCE_REG`, `OUTLINE_ACTION`, `RESOURCE_PLACEMENT`,
`LAYER_NAME`, `REDUNDANT_PAINT`, and `ENCRYPTION`. Each result has a rule, severity,
message, package path, actual value, and expected value when applicable.

The check is read-only and sorted by rule. An empty result is not an OFD-A
certificate: font subsetting is checked only for embedded files, some Java rules
are partial, and OFD-A conversion is incomplete.
`prepare_archive()` (Rust/Python), `PrepareArchive()` (Go), and
`ofd_prepare_archive()` (C) return a separate package after setting `DocType`
to `OFD-A`, removing document permission, viewer preference, and extension declarations, disabling
image interpolation in pages, referenced templates, and annotation appearances,
converting GIF image resources referenced through pages or templates to PNG, moving compatible
misplaced public resources into an existing or newly linked `DocumentRes` and page or template fonts/color spaces into
`PublicRes` (creating a linked public resource file when needed), flattening
pure horizontal `CTM` scaling into `HScale` for unrotated text without clips or
complex paint, scaling explicit `TextCode` X positions and `DeltaX` advances
while retaining `CGTransform` glyph mappings,
attribute-free page and template blocks deeper than three levels when they have no local
namespace declarations, flattening the same kind of page blocks inside
annotation appearances, removing custom image resource
children and non-Goto OFD actions (recognizing current and legacy OFD namespaces
while preserving identically named elements in custom XML or extension
namespaces), removing zero-area clips while hiding their
referencing objects in pages and templates, setting annotation `ReadOnly`, `NoZoom`, and `NoRotate`
flags, removing redundant paint attributes from page, template, and annotation objects,
and assigning unique names to duplicate direct page layers. Non-TXT/XML attachments are replaced with XML summaries containing their
original name, format, byte count, and SHA-256 digest. Obsolete attachment and
GIF payload files are removed when no XML value still references their paths;
ambiguous references retain the old files.

Archival checks and preparation join XML text split by comments or CDATA when
reading resource paths, text codes, external URLs, and `MaxUnitID`; replacing a
converted GIF path or stale `MaxUnitID` rewrites the complete value. Obsolete
payload pruning also checks joined XML text before deleting a file.
Resource moves require compatible
resolved bases, namespace bindings, and nonconflicting IDs; otherwise the
resources remain in place for inspection. A stale `MaxUnitID` is raised to the
highest numeric OFD element ID found in the package; unrelated XML namespaces
do not affect it. Removing actions can
discard interactive behavior. The source package is unchanged. Encrypted, signed,
integrity-protected, and multi-body packages are rejected. Call
`archive_violations()` on the result: the conversion pipeline does not yet
resolve every OFD-A rule.

Full layout, complete OFD rendering, complete PDF conversion, complete signature verification and creation,
and full OFD-A coverage are pending.

## 文档加密

OFD 2.0 password encryption uses SM3 password key derivation and SM4-CBC with
PKCS#7 padding, matching Java `ofdrw-crypto`. `encrypt_with_passwords()`
creates a separate encrypted package for one or more users; each has a name,
password, and optional `User`/`Owner` role. `is_encrypted()`,
`encryption_info()`, and `decrypt_with_password()` inspect and decrypt an
encrypted package. Python uses the same method names, Go uses
`EncryptWithPasswords`, `IsEncrypted`, `EncryptionInfo`, and
`DecryptWithPassword`; the C ABI exposes corresponding functions. The Java
password-encrypted fixture is decrypted by Rust tests, and the Java
`RustPasswordInteropTest` decrypts a Rust-generated fixture byte for byte.

Node.js exposes `encryptWithPasswords()`, `decryptWithPassword()`,
`isEncrypted`, and `encryptionInfo()` through the same C ABI.
Encryption and decryption return new documents, leaving the input unchanged.

SM2 certificate encryption uses the X.509 DER public key to wrap the SM4 file
key in the Java-compatible ASN.1 `EncryptedWK` format. Rust
`encrypt_with_certificates()` accepts named certificate recipients and
`decrypt_with_sm2_private_key()` accepts a raw 32-byte SM2 private key. Python
uses the same method names with certificate DER as `bytes`; Go uses
`EncryptWithCertificates` and `DecryptWithSM2PrivateKey` with `[]byte`; the C ABI
has matching exports. Java-generated certificate packages decrypt in all three
languages, and `RustCertificateInteropTest` checks Java decryption of a
Rust-generated fixture.

Node.js exposes `encryptWithCertificates()` and
`decryptWithSm2PrivateKey()` for DER certificate recipients and raw SM2 keys.
`encrypt_with_recipients()` accepts password and certificate users together.
The resulting seed omits a single protection-case ID, as Java ofdrw does for
mixed users. Python uses `encrypt_with_recipients()` with a `type` field,
Go uses `EncryptWithRecipients`, and C exposes
`ofd_encrypt_with_recipients_json()`. Node.js uses `encryptWithRecipients()`.
Both credentials decrypt the same package;
Java and Rust fixtures verify interoperability in both directions.

## Build and test

```sh
cargo test --workspace
cargo clippy --workspace --all-targets -- -D warnings
cargo build -p ofdrw-ffi
bash scripts/java-interop.sh
bash scripts/check-rust-coverage.sh
OFDRW_FULL_JAVA_PNG=1 cargo test -p ofdrw --test fixtures renders_available_java_fixture_corpus
cargo test --release -p ofdrw imports_java_xbrl_pdf_with_rgb_page_group_in_release_mode
cargo run -p ofdrw --example export_pdf -- input.ofd output.pdf
cargo run -p ofdrw --example import_pdf -- input.pdf output.ofd
```

The coverage script clears prior coverage artifacts before measuring each crate,
then enforces 90% line coverage for the core and 93% for the C ABI. This avoids
counting stale objects from other crates or earlier source versions. Pass `ofdrw`
or `ofdrw-ffi` to run one crate's gate; the default runs both.

The Rust integration suite also opens 38 reference OFD files from seven Java
modules, checks every package for resolvable references, and renders every
page to SVG. A separate test renders all 66 available Java OFD fixtures and
170 pages from the reader, layout, converter, sign, archive, crypto, and tool
modules. It extracts text from every page and rasterizes one sample from each
module in the standard test run. Set `OFDRW_FULL_JAVA_PNG=1` to rasterize the
first page of all 66 fixtures; that full scan has also passed. The curated validation
set excludes repository fixtures that themselves contain broken links, stale
IDs, or nonpositive object boundaries.
`java_text_sha256.tsv` records Java `ContentExtractor.getPageContent()` results
for 167 of these pages. The Rust integration test compares each page's ordered
text runs against the Java result using SHA-256 of a sequence of eight-byte
big-endian UTF-8 lengths followed by each run's UTF-8 bytes. The remaining
three pages are `ofdrw-converter/.../y.ofd`: Java's extractor returns no page
text there, while Rust includes text from composite graphic units; the
test checks that Rust still extracts text from all three.
The release-only PDF corpus test imports Java converter's `xbrl-001.pdf`,
checks references and rendering, and round-trips the resulting OFD. It runs
in the Linux CI matrix job; debug builds skip it because this sample takes
several minutes without optimization.
The Java interoperability script also checks that Java ofdrw reads a
Rust-generated four-value dash pattern on an original and duplicated path,
with distinct object IDs. It reads mirrored text imported from negative PDF
`Tz` and checks the resulting positive `HScale` and negative CTM. It parses a
Rust-generated custom tag with Java's
`CustomTags` model and checks both its replacement XML payload and retained
former payload.

From the repository root, verify Rust encryption against Java:

```sh
mvn -q -pl ofdrw-crypto -am -DskipTests=false -Dtest=RustPasswordInteropTest,RustCertificateInteropTest,RustMixedInteropTest,RustIntegrityInteropTest -Dsurefire.failIfNoSpecifiedTests=false test
```

## Python 绑定

Python uses CFFI. A wheel build runs `cargo build --release` and includes the
native library. Wheels use a `py3-none-<platform>` tag because the Rust C ABI
does not link against a specific CPython version; package metadata requires
Python 3.10 or newer:

```sh
cd bindings/python
python -m build --wheel
```

For development, set `PYTHONPATH=bindings/python` from this directory and
build the debug native library first. The loader also accepts `OFDRW_LIB_PATH`.
When set, this path is required: loading fails instead of searching for another
library if the specified file is unavailable.

## C ABI 与句柄生命周期

The C ABI treats document handles as opaque tokens. Concurrent calls sharing a
handle and `ofd_close()` are serialized; a call after close returns an error,
and closing the same token twice is safe. This also applies to Python, Go, and
Node.js bindings that use the shared library.

`ofd_buffer_free_parts(data, len)` releases a returned buffer through scalar
arguments. Go uses this entry point because purego does not support passing
structs by value on Windows. The original `ofd_buffer_free(buffer)` remains
available; release each returned buffer through exactly one of these methods.

## Go 绑定

Go loads the same C ABI dynamically without cgo. Build the Rust library before
running Go tests:

```sh
cargo build -p ofdrw-ffi
cd bindings/go
CGO_ENABLED=0 go test -count=1 -tags=integration ./...
```

Set `OFDRW_LIB_PATH` to an absolute library path when using the Go package
outside this repository. The Go wrapper is compiled for Linux, macOS, and
Windows on x86_64 and ARM64; CI executes native tests on all six targets.
An explicit path never falls back to a different installed or build library.

## Node.js 绑定

Node.js 20+ uses the same C ABI through Koffi. For source development, build
the Rust library, then install and test the Node.js package:

```sh
cargo build -p ofdrw-ffi
cd bindings/node
npm ci
OFDRW_LIB_PATH="$(realpath ../../target/debug/libofdrw_ffi.so)" npm test
```

On macOS use `libofdrw_ffi.dylib`; on Windows set `OFDRW_LIB_PATH` to the
absolute path of `target/debug/ofdrw_ffi.dll`. The explicit path selects the
fresh source build when an older packaged native library is also present.
If the explicit path cannot be loaded, Node.js reports that error without
falling back to the packaged library.
Node.js package staging loads each native library against the wrapper and
records interface and file digests; `npm pack` checks all six staged targets.

Node.js `Document.save(path)` writes an OFD file through the same atomic
native save operation used by the Rust core; `Document.open(path)` reopens it.

The Node.js `Document` API currently covers opening OFD/PDF bytes or a file,
page and image inspection, keyword and region text search, template and
composite graphic inspection, SVG/PNG/PDF/OFD export, text and blank pages,
paginated plain and styled text and fixed-grid tables through the Rust layout engine,
embedded font creation and font byte access for shaped text,
positioned text/path/image and composite text/path/image objects, template creation
and application, document-body import and ordering, page insertion and ordering,
image and vector path page creation and per-document appending,
with OFD drawing commands checked before path pages, path objects, or path
annotations are written,
while `validate_references()` reports `invalid_path_data` for malformed
paths in pages, templates, annotations, and composite graphics read from a package,
including nested clip paths (with `missing_path_data` for empty clip paths),
page duplication, direct page-object duplication/removal,
direct page layer creation, edits, removal, object reordering, and PageBlock grouping,
direct object geometry, visibility, opacity, and blend mode edits,
document and page color-space resource creation and inspection,
reusable document and page DrawParam creation, inspection, and scoped updates
for page objects and layers,
package entries, and attachments. Call `close()`
when finished. The CI-built npm tarball bundles libraries for Linux, macOS,
and Windows on x64 and ARM64. `OFDRW_LIB_PATH` overrides the bundled library
when a custom build is needed. Node.js also exposes raw SM2 signing,
certificate-based signature value checks, protected-file digest checks, and
caller-supplied SM2 distinguishing IDs for raw signature creation and verification.
It can create SES v4/v5 seals and document signatures, including positioned
stamps and separate issuer, seal, and signer distinguishing IDs.
Node.js exposes trusted integrity-manifest verification with ordered chains or an unordered
certificate pool. It also exposes combined signer and SES seal issuer trust
reports with ordered SM2 chains or candidate certificate pools.
Password and SM2 certificate package encryption and decryption are supported.
Node.js also exposes bookmarks, common annotations, and the supported OFD-A
violation report and preparation operation.
Node.js exposes separate SES description-hash, seal-match, seal authorization,
seal validity, and issuer-certificate validity checks, plus caller-supplied
SM2 distinguishing IDs for signer and seal signature checks.
Node.js attachment editing supports adding to an existing document body,
renaming, replacing bytes, replacing standard metadata, and removing an entry
by global attachment index.
Node.js also lists, reads, adds, replaces, and removes document-level and
page-local multimedia resources through the shared C ABI.
Sound and Movie page-object actions and media link annotations are also
available in Node.js.
`documents()` reads DocInfo for every body, while `setDocumentInfoField()`,
`removeDocumentInfoField()`, `addDocumentKeyword()`, `removeDocumentKeyword()`,
`setDocumentCustomData()`, and `removeDocumentCustomData()` edit its metadata.
Node.js also reads and writes document permissions, viewer preferences, and
document default or explicit page areas through the existing C ABI.
It reads, appends, and removes document and page actions with URI, page,
bookmark, or attachment targets; page actions use the `PO` event.
`customTags()`, `customTagData()`, `addCustomTag()`,
`replaceCustomTagData()`, `extensions()`, and
`addExtension()` expose supported custom tag payloads and extension records.
`outlines()`, `addOutline()`, and `removeOutline()` manage hierarchical document
navigation nodes, including destinations and expanded state.
`versions()`, `addVersionSnapshot()`, and `extractVersionSnapshot()` expose
independent OFD version trees; extracted versions return a separate handle.
Node.js reads object clips, direct and effective paints, resolved stroke styles,
and actions on page or template source objects. It edits direct page-object
rectangle, path, and mixed path/text clips through the matching `setPageObject*Clips()` methods.
Direct page objects also accept
solid RGB, axial and radial gradient, Gouraud mesh, and repeating pattern fill
and stroke edits through the corresponding `setPageObject*()` methods.
Node.js also sets or clears page-image masks, substitutions, and borders.
Node.js edits annotation creator, subtype, modification date, appearance
boundary, remark, named parameters, visibility, print, and standard flags.
It creates page-destination and attachment link annotations through the shared
C ABI, including optional page quad points and attachment new-window behavior.
It also creates text and image Watermark and Stamp annotations and applies
text or image watermarks to all current pages in one atomic operation.
Direct page objects also accept
path command and stroke geometry edits through `setPagePathData()`,
`setPagePathStrokeStyle()`, and `setPagePathDashSequence()`. Text run, typography,
font, and paint flag edits use the corresponding `setPageText*()`,
`appendPageTextCode()`, and `removePageTextCode()` methods. Direct page objects
also accept URI, page-destination, bookmark, and attachment actions. Their clickable
rectangle regions can be edited, and actions can be removed by index.
SVG, PNG, and PDF exports accept an optional image-substitution preference;
composite resource thumbnail and substitution IDs can be edited through Node.js.
Root integrity manifest reading, SM2 signing, and verification are available.
Python, Go, and Node.js declare the current C ABI operations. Go uses scalar buffer release, JSON positioned SES stamping, and a newer
SES signing entry point in place of older aliases; Node.js uses newer JSON or
configurable entry points for 10 older aliases. CI checks these bindings with
`scripts/check_binding_ffi_parity.py`.

## 持续集成

The GitHub Actions workflow runs Rust, Python, Go, and Node.js tests on all six target
platforms, checks Rust and wrapper coverage plus Node.js TypeScript declarations,
builds a Python wheel on each platform, assembles and smoke-tests a Node.js
tarball with all six native libraries, and verifies that Java can read a Rust-generated document.
The Java job also generates fresh Rust password, certificate, mixed-recipient,
and integrity-protected packages before testing Java decryption and verification,
including wrong-password and extra-file rejection. The Python
integration suite also opens all 66 OFD samples in the Java modules, extracting
text and parsing SVG previews for every page.

## Current API conventions

- Page indexes are zero-based in Rust, Python, Go, Node.js, and the C ABI.
- Package paths use forward slashes and must remain inside the ZIP root.
- Document roots and page `BaseLoc` references resolve a unique archive entry
  even when the reference differs in letter case; ambiguous matches are rejected.
- The Rust parser retains unchanged package-entry bytes, including unknown XML.
- Raw `set_entry()` / `SetEntry()` replaces package bytes even when an entry is
  protected by a signature. Use `check_signature_references()` /
  `CheckSignatureReferences()` afterward to detect invalidated digests;
  structured editors reject changes to signed content.
- Raw `remove_entry()` / `RemoveEntry()` / `removeEntry()` removes a ZIP entry
  and returns whether it existed. It can invalidate references or signatures;
  run the corresponding checks after deletion.
- Missing annotation files are skipped during reading and preview; `validate_references()` reports their paths.
- Missing resource description files are skipped during reading and preview;
  `validate_references()` reports their paths. A resource `BaseLoc="/"` denotes
  the package root.
- C callers own document handles and returned buffers and must release them with
  `ofd_close` and `ofd_buffer_free`.
- All C ABI failures return a nonzero status; `ofd_last_error` provides details
  for the calling OS thread.
