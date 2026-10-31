import { Document, createSesSeal, type ActionInfo, type ArchiveViolation, type AttachmentInfo, type AttachmentMetadata, type ColorSpaceInfo, type ColorSpaceSpec, type CustomTagInfo, type DocumentPermissions, type DrawParamInfo, type DrawParamStyle, type ExtensionInfo, type FlowSpan, type FlowTextOptions, type FontInfo, type KeywordPosition, type MediaActionRequest, type MultimediaResourceInfo, type OutlineInfo, type PageArea, type PageInfo, type PageLayerInfo, type PageObjectClipsInfo, type PageObjectInfo, type PageObjectPaintInfo, type PageObjectStrokeStyleInfo, type PathStrokeStyle, type SesSealSpec, type TableBlock, type TableCell, type TableOptions, type TextCodeLayout, type TextObjectOptions, type TextTypographyStyle, type VersionInfo, type ViewPreferences } from '../index';
import type { IntegrityTrustedVerificationInfo, SignatureChainVerificationInfo, SignatureDirectTrustVerificationInfo, SignatureTrustVerificationInfo } from '../index';
import type { GradientPaintStyle, GouraudPaintStyle, ImageBorderStyle, PatternPaintStyle, SolidPaintStyle } from '../index';

function exercise(document: Document, source: Document, certificate: Uint8Array, key: Buffer): void {
  const shapedTextOptions: TextObjectOptions = {
    shape: true, font_id: 12, language: 'en', fill_color: [0, 0, 255], size: 6,
  };
  document.addTextObject(0, 'fifi', [2, 2, 30, 8], shapedTextOptions);
  document.addTextObject(0, 'plain', [2, 12, 30, 8], {fill_color: [0, 0, 0]});
  // @ts-expect-error language requires shape=true
  document.addTextObject(0, 'bad', [2, 2, 30, 8], {language: 'en'});
  // @ts-expect-error shaping requires a font ID
  document.addTextObject(0, 'bad', [2, 2, 30, 8], {shape: true});
  document.setPageImageInterpolate(0, '5', false);
  document.setPageImageInterpolate(0, '5', true);
  document.setPageImageInterpolate(0, '5', null);
  document.setPageImageInterpolate(0, '5');
  // @ts-expect-error interpolation requires a boolean or null
  document.setPageImageInterpolate(0, '5', 1);
  document.save('output.ofd');
  document.setFontAttribute(0, 'FamilyName', 'Example');
  document.setFontAttribute(0, 'Bold');
  document.replaceFontData(0, Buffer.from('font'));
  document.replacePageFontData(0, 0, Buffer.from('font'));
  document.reorderDocumentPages(0, [1, 0]);
  document.setPageFontAttribute(0, 0, 'Serif', 'true');
  document.removeDocumentTemplate(0, '1');
  const bodyTemplateId: string = document.promotePageToTemplate(0, [], 'Body');
  document.applyPageTemplate(1, bodyTemplateId, 'Body');
  document.removeCustomTag(0);
  const updatedTag: CustomTagInfo = document.updateCustomTag(0, 'updated', {namespace: 'urn:updated'});
  void updatedTag;
  const replacedTag: CustomTagInfo = document.replaceCustomTagData(0, Buffer.from('<Tag/>'));
  void replacedTag;
  document.setPageObjectClipRects(0, '1', [[0, 0, 10, 10]], true);
  document.setPageObjectPathClips(0, '1', [[{boundary: [0, 0, 10, 10], commands: 'M 0 0 L 10 10'}]]);
  document.setPageObjectClips(0, '1', [[
    {kind: 'Path', boundary: [0, 0, 10, 10], commands: 'M 0 0 L 10 10'},
    {kind: 'Text', boundary: [0, 0, 10, 10], font_id: '2', size: 5, text: 'A', x: 0, y: 5},
  ]]);
  const annotationActions: ActionInfo[] = document.annotationObjectActions(0, '2', '3');
  void annotationActions;
  document.addAnnotationObjectAction(0, '2', '3', {kind: 'uri', uri: 'https://example.org'});
  document.addAnnotationObjectAction(0, '2', '3', {kind: 'page', event: 'CLICK', destination: {page_id: '1', kind: 'Fit'}});
  document.addAnnotationObjectAction(0, '2', '3', {kind: 'bookmark', name: 'chapter'});
  document.addAnnotationObjectAction(0, '2', '3', {kind: 'attachment', id: '4', new_window: false});
  document.setAnnotationObjectActionRegions(0, '2', '3', 0, [[0, 0, 5, 4]]);
  document.removeAnnotationObjectAction(0, '2', '3', 0);
  // @ts-expect-error annotation graphics require CLICK
  document.addAnnotationObjectAction(0, '2', '3', {kind: 'uri', event: 'PO', uri: 'x'});
  // @ts-expect-error URI actions require a URI
  document.addAnnotationObjectAction(0, '2', '3', {kind: 'uri'});
  document.setAnnotationCreator(0, '2', 'author');
  document.setAnnotationSubtype(0, '2', 'review');
  document.setAnnotationLastModDate(0, '2', '2024-02-29');
  document.setAnnotationAppearanceBoundary(0, '2', [0, 0, 10, 10]);
  document.setAnnotationRemark(0, '2', 'note');
  document.setAnnotationParameter(0, '2', 'status', 'draft');
  document.removeAnnotationParameter(0, '2', 'status');
  document.setAnnotationVisible(0, '2', true);
  document.setAnnotationPrint(0, '2', true);
  document.setAnnotationFlag(0, '2', 'ReadOnly', false);
  const gotoLinkId: string = document.addGotoLinkAnnotation(0,
    {page_id: '1', kind: 'Fit'}, [0, 0, 10, 10]);
  const attachmentLinkId: string = document.addAttachmentLinkAnnotation(0, '3', [0, 0, 10, 10], true);
  void gotoLinkId; void attachmentLinkId;
  const watermarkId: string = document.addTextWatermark(0, 'draft', [0, 0, 10, 10], 5,
    {opacity: 120});
  const stampId: string = document.addTextStampAnnotation(0, 'approved', [0, 0, 10, 10], 5);
  const watermarkIds: string[] = document.addTextWatermarkToAllPages('copy', 5);
  const imageWatermarkId: string = document.addImageWatermark(0, [0, 0, 10, 10],
    {data: key, format: 'png'});
  const imageStampId: string = document.addImageStampAnnotation(0, [0, 0, 10, 10],
    {resourceId: '1'});
  const imageWatermarkIds: string[] = document.addImageWatermarkToAllPages(key, 'png', [0, 0, 10, 10]);
  const compositeTextId: string = document.addCompositeTextObject(0, 'text', [0, 0, 10, 10], 5, '1');
  void watermarkId; void stampId; void watermarkIds; void imageWatermarkId; void imageStampId;
  void imageWatermarkIds;
  void compositeTextId;
  document.removeExtension(0);
  document.updateExtension(0, {document_index: 0, app_name: 'updated'});
  const rawSignatureIndex: number = document.addDigitalSignature(key,
    '2020-09-13 12:26:40', 'custom-signature-id');
  const rawSignatureValid: boolean = document.checkDigitalSignatureWithCertificate(
    rawSignatureIndex, certificate, 'custom-signature-id');
  const signerCertificateValid: boolean = document.checkSignatureCertificateValidity(
    rawSignatureIndex, certificate, 1_893_456_000);
  void signerCertificateValid;
  const directAnchorValid: boolean = document.checkSignatureDirectTrustAnchor(
    rawSignatureIndex, certificate, certificate, 1_893_456_000);
  const sealIssuerValid: boolean = document.checkSesSealIssuerOrderedSm2Chain(
    rawSignatureIndex, [], certificate, 1_893_456_000);
  void directAnchorValid; void sealIssuerValid;
  void rawSignatureValid;
  const sealSpec: SesSealSpec = {
    version: 4, vendorId: 'ofdrw-rs', sealId: 'typescript', name: 'Seal', sealType: 3,
    authorizedSignerCertificates: [certificate], createdAt: 1600000000,
    validFrom: 1600000000, validUntil: 1900000000,
    pictureType: 'PNG', pictureWidthMm: 40, pictureHeightMm: 40,
  };
  const seal = createSesSeal(key, certificate, key, sealSpec);
  const signatureIndex: number = document.addSesSignature(key, certificate, seal,
    '2020-09-13 12:26:40', {pageIndex: 0, boundary: [10, 20, 40, 40]});
  void signatureIndex;
  const media: MultimediaResourceInfo[] = document.pageMultimediaResources(0);
  const documentMedia: MultimediaResourceInfo[] = document.documentMultimediaResources(0);
  const mediaBytes: Buffer = document.pageMultimediaData(0, '1');
  const documentMediaBytes: Buffer = document.documentMultimediaData(0, '1');
  const pageMediaId: string = document.addPageMultimediaResource(0, 'Audio', 'WAV', mediaBytes);
  document.replacePageMultimediaData(0, pageMediaId, mediaBytes);
  document.replaceDocumentMultimediaData(0, '1', documentMediaBytes);
  document.removePageMultimediaResource(0, pageMediaId);
  document.removeDocumentMultimediaResource(0, '1');
  const mediaAction: MediaActionRequest = {kind: 'Sound', resource_id: '1', volume: 75};
  document.addPageObjectMediaAction(0, '2', mediaAction);
  document.addPageObjectSoundAction(0, '2', '1', {volume: 75, repeat: true});
  document.addPageObjectMovieAction(0, '2', '3', 'Pause');
  const mediaLinkId: string = document.addMediaLinkAnnotation(0,
    {kind: 'Movie', resource_id: '3', boundary: [1, 1, 5, 5], operator: 'Pause'});
  const soundLinkId: string = document.addSoundLinkAnnotation(0, '1', [1, 1, 5, 5], {repeat: true});
  const movieLinkId: string = document.addMovieLinkAnnotation(0, '3', [7, 1, 5, 5], 'Play');
  void mediaLinkId;
  void soundLinkId;
  void movieLinkId;
  void media;
  void documentMedia;
  const orderedIntegrity: IntegrityTrustedVerificationInfo | null =
    document.verifyIntegrityManifestSm2WithOrderedChain(certificate, [], certificate, 1_893_456_000);
  const pooledIntegrity: IntegrityTrustedVerificationInfo | null =
    document.verifyIntegrityManifestSm2WithCertificatePool(certificate, [certificate], certificate,
      1_893_456_000, {signatureDistid: '1234567812345678'});
  void orderedIntegrity;
  void pooledIntegrity;
  const directTrust: SignatureDirectTrustVerificationInfo =
    document.verifySignatureWithDirectTrustAnchor(0, certificate, certificate, 1_893_456_000);
  const signerChain: SignatureChainVerificationInfo =
    document.verifySignatureWithOrderedSm2Chain(0, certificate, [], certificate, 1_893_456_000);
  const signerPool: SignatureChainVerificationInfo =
    document.verifySignatureWithCertificatePool(0, certificate, [certificate], certificate,
      1_893_456_000, {signatureDistid: '1234567812345678'});
  void directTrust;
  void signerChain;
  void signerPool;
  const signatureTrust: SignatureTrustVerificationInfo =
    document.verifySignatureWithOrderedSm2Chains(0, {
      signerCertificate: certificate, signerAnchor: certificate, atUnixSeconds: 1_893_456_000,
    });
  const signaturePool: SignatureTrustVerificationInfo =
    document.verifySignatureWithCertificatePools(0, {
      signerCertificate: certificate, signerIntermediates: [certificate],
      signerAnchor: certificate, atUnixSeconds: 1_893_456_000,
    });
  void signatureTrust;
  void signaturePool;
  const descriptionHash: {path: string; valid: boolean; expected: string; actual: string} =
    document.checkSignatureDescriptionHash(0);
  const sealMatch: boolean | null = document.checkSignatureSealMatch(0);
  const sealAuthorized: boolean = document.checkSesSealSignerAuthorization(0);
  const sealValid: boolean = document.checkSesSealValidity(0, 1_600_000_000);
  const sealCertificateValid: boolean = document.checkSesSealCertificateValidity(0, 1_600_000_000);
  const sealValueValid: boolean = document.checkSesSignatureValueWithDistid(0, '1234567812345678');
  const sealSignatureValid: boolean = document.checkSesSealSignatureWithDistid(0, '1234567812345678');
  void descriptionHash; void sealMatch; void sealAuthorized; void sealValid; void sealCertificateValid;
  void sealValueValid; void sealSignatureValid;
  const page: PageInfo = document.pages()[0];
  const textNoteId: string = document.addTextNoteAnnotation(0, 'note', [1, 1, 5, 5], {icon: 'Comment', open: true});
  const shapeId: string = document.addShapeAnnotation(0, 'Circle', [1, 1, 5, 5], {fillColor: [255, 0, 0]});
  const lineId: string = document.addLineAnnotation(0, [1, 1, 5, 5], [0, 0, 5, 5], {strokeColor: [255, 0, 0]});
  const multiPathId: string = document.addMultiPathAnnotation(0, [1, 1, 10, 10], [{commands: 'M 0 0 L 10 10', boundary: [0, 0, 10, 10], stroke_color: [255, 0, 0]}]);
  const appendedIds: string[] = document.appendAnnotationObjects(0, multiPathId, [{kind: 'text', text: 'more', boundary: [0, 0, 5, 5], size: 3}]);
  document.setAnnotationObjectName(0, multiPathId, appendedIds[0], 'label');
  document.setAnnotationObjectBlendMode(0, multiPathId, appendedIds[0], 'Multiply');
  document.setAnnotationObjectDrawParam(0, multiPathId, appendedIds[0], 42);
  const compositeIds: string[] = document.appendAnnotationObjects(0, multiPathId, [{kind: 'composite', resource_id: '42', boundary: [0, 0, 5, 5], alpha: 180}]);
  document.setAnnotationPathData(0, multiPathId, appendedIds[0], 'M 0 0 L 5 5');
  document.setAnnotationPathStrokeStyle(0, multiPathId, appendedIds[0], {line_width: 1});
  document.setAnnotationTextCode(0, multiPathId, appendedIds[0], 0, 'text');
  document.removeAnnotationTextCode(0, multiPathId, appendedIds[0], 0);
  document.appendAnnotationTextCode(0, multiPathId, appendedIds[0], 'text', { delta_x: [1, 1, 1] });
  document.setAnnotationTextCodeLayout(0, multiPathId, appendedIds[0], 0, { x: 1, y: 2 });
  document.setAnnotationTextTypography(0, multiPathId, appendedIds[0], { size: 4, italic: true });
  document.setAnnotationGraphicResource(0, multiPathId, appendedIds[0], '1');
  const layers: PageLayerInfo[] = document.pageLayers(0);
  document.setPageLayerName(0, '1', layers[0]?.name ?? null);
  const objects: PageObjectInfo[] = document.pageObjects(0);
  document.setPageObjectBoundary(0, '1', [0, 0, 10, 10]);
  document.setPageObjectCtm(0, '1', [1, 0, 0, 1, 0, 0]);
  document.setPageObjectVisible(0, '1', true);
  document.setPageObjectAlpha(0, '1', 128);
  document.setPagePathData(0, '1', 'M 0 0 L 10 10');
  const pathStyle: PathStrokeStyle = { line_width: 1.25, cap: 'round', dash_sequence: [3, 2] };
  document.setPagePathStrokeStyle(0, '1', pathStyle);
  document.setPagePathDashSequence(0, '1', [3, 2], 1);
  const solid: SolidPaintStyle = { color: [255, 0, 0] };
  const gradient: GradientPaintStyle = { kind: 'axial', start_point: [0, 0], end_point: [10, 0],
    segments: [{ color: [255, 0, 0] }, { color: [0, 0, 255] }] };
  const gouraud: GouraudPaintStyle = { points: [
    { x: 0, y: 0, color: [255, 0, 0] }, { x: 10, y: 0, color: [0, 255, 0] },
    { x: 0, y: 10, color: [0, 0, 255] },
  ] };
  const pattern: PatternPaintStyle = { width: 4, height: 4, cells: [] };
  document.setPageObjectSolidFill(0, '1', solid);
  document.setPageObjectSolidStroke(0, '1', solid);
  document.setPageObjectGradientFill(0, '1', gradient);
  document.setPageObjectGradientStroke(0, '1', gradient);
  document.setPageObjectGouraudFill(0, '1', gouraud);
  document.setPageObjectGouraudStroke(0, '1', gouraud);
  document.setPageObjectPatternFill(0, '1', pattern);
  document.setPageObjectPatternStroke(0, '1', pattern);
  const border: ImageBorderStyle = { line_width: 0.5, color: [255, 0, 0] };
  document.setPageImageMask(0, '1', '2');
  document.setPageImageSubstitution(0, '1', '2');
  document.setPageImageBorder(0, '1', border);
  const typography: TextTypographyStyle = { size: 4, h_scale: 0.8, italic: true };
  const codeLayout: TextCodeLayout = { x: 12, y: 5, delta_x: [2] };
  document.setPageTextTypography(0, '1', typography);
  document.setPageTextCode(0, '1', 0, 'A\0B');
  document.appendPageTextCode(0, '1', 'C', codeLayout);
  document.setPageTextCodeLayout(0, '1', 1, codeLayout);
  document.removePageTextCode(0, '1', 1);
  document.setPageTextFont(0, '1', '1');
  document.setPageTextPaintFlags(0, '1', { fill: true, stroke: false });
  document.setPageObjectName(0, '1', 'shape');
  document.setPageObjectBlendMode(0, '1', 'Multiply');
  document.setPageObjectDrawParam(0, '1');
  const layerId: string = document.addPageLayer(0, 'Foreground');
  document.setPageLayerType(0, layerId, 'Background');
  document.setPageLayerDrawParam(0, layerId);
  document.movePageObjectToLayer(0, '1', layerId);
  document.movePageObject(0, '1', null);
  document.groupPageObjects(0, '1', '2');
  document.ungroupPageObjects(0, '1');
  document.removePageLayer(0, layerId);
  const pageBox: number[] | null = page.physical_box;
  const flowOptions: FlowTextOptions = { page_size: [40, 30], margins: [5, 5, 5, 5], align: 'center' };
  const flowedPages: number[] = document.addFlowedText(0, 'hello', flowOptions);
  const spans: FlowSpan[] = [{ text: 'hello', bold: true }, { text: 'world', fill_color: [255, 0, 0] }];
  document.addFlowedSpans(0, spans, flowOptions);
  const fontId: string = document.addEmbeddedFont('Example', Buffer.from('font'));
  document.addTextPageWithFont('hello\0world', fontId, 40, 30);
  document.appendTextPageWithFont(0, 'hello', fontId, 40, 30);
  document.addImagePage(Buffer.from('image'), 'png', 40, 30);
  document.appendImagePage(0, Buffer.from('image'), 'png', 40, 30);
  document.addPathPage('M 0 0 L 10 10', [0, 0, 10, 10], 40, 30);
  document.appendPathPage(0, 'M 0 0 L 10 10', [0, 0, 10, 10], 40, 30);
  document.addEmbeddedFontToDocument(0, 'Example', Buffer.from('font'));
  document.addEmbeddedFontToPage(0, 'Example', Buffer.from('font'));
  const fonts: FontInfo[] = document.fonts();
  const fontData: Buffer = document.fontData(0);
  const pageFonts: FontInfo[] = document.pageFonts(0);
  const pageFontData: Buffer = document.pageFontData(0, 0);
  const colorSpec: ColorSpaceSpec = { color_type: 'RGB', palette: [[255, 0, 0]] };
  const colorId: string = document.addDocumentColorSpace(0, colorSpec);
  document.addPageColorSpace(0, colorSpec);
  document.setDocumentColorSpace(0, colorId, colorSpec);
  document.setPageColorSpace(0, colorId, colorSpec);
  document.removeDocumentColorSpace(0, colorId);
  document.removePageColorSpace(0, colorId);
  const spaces: ColorSpaceInfo[] = document.documentColorSpaces(0);
  const pageSpaces: ColorSpaceInfo[] = document.pageColorSpaces(0);
  document.setDocumentDefaultColorSpace(0, colorId);
  const defaultColor: string | null = document.documentDefaultColorSpace(0);
  const drawStyle: DrawParamStyle = { fill: { color: [255, 0, 0] }, line_width: 1 };
  const drawId: string = document.addDocumentDrawParam(0, drawStyle);
  const pageDrawId: string = document.addPageDrawParam(0, { relative: drawId });
  document.setDocumentDrawParam(0, drawId, drawStyle);
  document.setPageDrawParam(0, pageDrawId, { relative: drawId });
  document.removePageDrawParam(0, pageDrawId);
  document.removeDocumentDrawParam(0, drawId);
  const documentDrawParams: DrawParamInfo[] = document.documentDrawParams(0);
  const pageDrawParams: DrawParamInfo[] = document.pageDrawParams(0);
  void documentDrawParams; void pageDrawParams; void pageDrawId;
  const tableOptions: TableOptions = { column_widths: [30, 30], repeat_header: true };
  const tablePages: number[] = document.addTable(0, [['A', 'B']], tableOptions);
  const tableCells: TableCell[] = [{ row: 0, column: 0, column_span: 2, text: 'Merged', bold: true }];
  document.addTableCells(0, 1, tableCells, tableOptions);
  const blocks: TableBlock[] = [
    { type: 'text', text: 'nested' },
    { type: 'table', table: { row_count: 1, column_widths: [20], cells: [{ row: 0, column: 0, text: 'inside' }] } },
  ];
  document.addTableCells(0, 1, [{ row: 0, column: 0, blocks }], tableOptions);
  const words: string[][] = document.allText();
  const positions: KeywordPosition[] = document.keywordPositions(['hello'], [0]);
  const violations: ArchiveViolation[] = document.archiveViolations();
  const attachmentMetadata: AttachmentMetadata = { format: 'text/plain', visible: true };
  document.addAttachmentToDocument(0, 'example.txt', Buffer.from('example'));
  const attachments: AttachmentInfo[] = document.attachments();
  document.renameAttachment(0, 'renamed.txt');
  document.replaceAttachmentData(0, Buffer.from('replacement'));
  document.setAttachmentMetadata(0, attachmentMetadata);
  document.removeAttachment(0);
  const removedEntry: boolean = document.removeEntry('scratch.bin');
  document.setDocumentInfoField(0, 'Title', 'OFD');
  document.addDocumentKeyword(0, 'invoice');
  document.removeDocumentKeyword(0, 0);
  document.setDocumentCustomData(0, 'kind', 'invoice');
  document.removeDocumentCustomData(0, 'kind');
  document.removeDocumentInfoField(0, 'Title');
  const permissions: DocumentPermissions = document.documentPermissions(0);
  document.setDocumentPermissions(0, permissions);
  const preferences: ViewPreferences = document.viewPreferences(0);
  document.setViewPreferences(0, preferences);
  const area: PageArea | null = document.documentPageArea(0);
  document.setDocumentPageArea(0, { physical_box: [0, 0, 210, 297] });
  document.setPageArea(0, { physical_box: [0, 0, 100, 80], content_box: [5, 5, 90, 70] });
  const actions: ActionInfo[] = document.documentActions(0);
  document.addDocumentAction(0, { kind: 'uri', event: 'DO', uri: 'https://example.org' });
  document.removeDocumentAction(0, 0);
  document.addPageAction(0, { kind: 'uri', event: 'PO', uri: 'https://example.org' });
  document.removePageAction(0, 0);
  const tag: CustomTagInfo = document.addCustomTag(0, 'invoice', Buffer.from('<Invoice/>'));
  const tagBytes: Buffer = document.customTagData(0);
  document.addExtension({ document_index: 0, app_name: 'Example' });
  const extensions: ExtensionInfo[] = document.extensions();
  const outlineIndex: number = document.addOutline(0, 'Chapter', { destination: { page_id: '1', kind: 'Fit' } });
  const outlines: OutlineInfo[] = document.outlines();
  document.setOutlineTitle(0, outlines[0]?.title ?? 'Section');
  document.setOutlineExpanded(0, true);
  document.setOutlineDestination(0, { page_id: page.id, kind: 'Fit' });
  document.setOutlineBookmark(0, 'chapter');
  document.removeOutline(outlineIndex);
  const snapshot: VersionInfo = document.addVersionSnapshot(0, 'v1', 'Initial');
  const versions: VersionInfo[] = document.versions();
  const extracted: Document = document.extractVersionSnapshot(0);
  extracted.close();
  const clips: PageObjectClipsInfo[] = document.pageObjectClips(0);
  const paints: PageObjectPaintInfo[] = document.pageObjectPaints(0);
  const strokes: PageObjectStrokeStyleInfo[] = document.pageObjectStrokeStyles(0);
  const objectActions: ActionInfo[] = document.pageObjectActions(0, '1');
  document.pageSourceObjectActions(0, page.path, '1');
  document.addPageObjectUriAction(0, '1', 'https://example.org');
  document.addPageObjectGotoAction(0, '1', { page_id: page.id, kind: 'Fit' });
  document.addPageObjectBookmarkAction(0, '1', 'chapter');
  document.addPageObjectAttachmentAction(0, '1', '2', true);
  document.setPageObjectActionRegions(0, '1', 0, [[0, 0, 10, 10]]);
  document.removePageObjectAction(0, '1', 0);
  const bodyIndex: number = document.appendDocumentBodyFrom(source, 0);
  const insertedIndex: number = document.insertTextPage(bodyIndex, 0, 'a\0b');
  const compositeId: string = document.addCompositePathObject(insertedIndex, 'M 0 0 L 1 1', [0, 0, 1, 1]);
  document.setPageCompositeUnitImages(insertedIndex, compositeId, { substitutionId: '12' });
  const svg: string = document.pageSvg(insertedIndex, { preferImageSubstitution: true });
  const png: Buffer = document.pagePng(insertedIndex, 96, { preferImageSubstitution: true });
  const pdf: Buffer = document.toPdf({ preferImageSubstitution: true });
  const bookmarkId: string = document.addBookmarkLinkAnnotation(insertedIndex, 'chapter', [0, 0, 1, 1]);
  const encrypted: Document = document.encryptWithRecipients([
    { type: 'password', username: 'alice', password: 'secret' },
    { type: 'certificate', username: 'bob', certificate },
  ]);
  const decrypted: Document = encrypted.decryptWithSm2PrivateKey(key, 'bob');
  const archival: Document = decrypted.prepareArchive();
  archival.close();
  decrypted.close();
  encrypted.close();
  void [pageBox, area, actions, tag, tagBytes, extensions, outlines, snapshot, versions, clips, paints, strokes, objectActions, words, positions, violations, attachments, compositeId, bookmarkId, svg, png, pdf, flowedPages, tablePages, fontId, fonts, fontData, pageFonts, pageFontData, layers, objects, spaces, pageSpaces, defaultColor, drawId];

  // @ts-expect-error password recipients need a password
  document.encryptWithPasswords([{ username: 'alice' }]);
  // @ts-expect-error certificate recipients need DER bytes
  document.encryptWithCertificates([{ username: 'bob', certificate: 'der' }]);
  // @ts-expect-error mixed recipients require a recognized discriminator
  document.encryptWithRecipients([{ type: 'other', username: 'alice' }]);
  // @ts-expect-error render options require a boolean
  document.toPdf({ preferImageSubstitution: 1 });
  // @ts-expect-error document indexes must be numbers
  document.setDocumentInfoField('0', 'Title', 'OFD');
  // @ts-expect-error permission fields must be complete
  document.setDocumentPermissions(0, { edit: true });
  // @ts-expect-error the physical page box needs four numbers
  document.setPageArea(0, { physical_box: [0, 0, 100] });
  // @ts-expect-error direct page actions require a PO event
  document.addPageAction(0, { kind: 'uri', event: 'DO', uri: 'https://example.org' });
  // @ts-expect-error extension application name is required
  document.addExtension({ document_index: 0 });
  // @ts-expect-error parent outline index must be numeric
  document.addOutline(0, 'Chapter', { parentIndex: '0' });
  // @ts-expect-error version names must be strings
  document.addVersionSnapshot(0, 'v2', 42);
  // @ts-expect-error action target must have a destination kind
  document.addPageObjectGotoAction(0, '1', { page_id: page.id });
  // @ts-expect-error newWindow must be a boolean
  document.addPageObjectAttachmentAction(0, '1', '2', 1);
  // @ts-expect-error action rectangles need four numbers
  document.setPageObjectActionRegions(0, '1', 0, [[0, 0, 10]]);
  // @ts-expect-error flow alignment must use a supported value
  document.addFlowedText(0, 'hello', { align: 'justify' });
  // @ts-expect-error span color needs three channels
  document.addFlowedSpans(0, [{ text: 'hello', fill_color: [255, 0] }]);
  // @ts-expect-error table column widths are required
  document.addTable(0, [['A']], { repeat_header: true });
  // @ts-expect-error table cell row is required
  document.addTableCells(0, 1, [{ column: 0, text: 'A' }], tableOptions);
  // @ts-expect-error image blocks require encoded data and dimensions
  document.addTableCells(0, 1, [{ row: 0, column: 0, blocks: [{ type: 'image', format: 'png' }] }], tableOptions);
  // @ts-expect-error font bytes cannot be a string
  document.addEmbeddedFont('Example', 'font');
  // @ts-expect-error attachment visibility must be a boolean
  document.setAttachmentMetadata(0, { visible: 'no' });
  // @ts-expect-error font ID must be numeric or a numeric string
  document.addTextPageWithFont('hello', false);
  // @ts-expect-error layer type must be one of the supported values
  document.addPageLayer(0, 'Overlay');
  // @ts-expect-error a CTM needs six numbers
  document.setPageObjectCtm(0, '1', [1, 0, 0]);
  // @ts-expect-error color type must be supported
  document.addDocumentColorSpace(0, { color_type: 'LAB' });
  // @ts-expect-error solid draw color needs three channels
  document.addDocumentDrawParam(0, { fill: { color: [255, 0] } });
}

void exercise;
