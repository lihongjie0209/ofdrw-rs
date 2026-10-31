export class OfdError extends Error {}

export interface SesSealSpec {
  version: 4 | 5;
  vendorId: string;
  sealId: string;
  name: string;
  sealType: number;
  authorizedSignerCertificates: Array<Buffer | Uint8Array>;
  createdAt: number;
  validFrom: number;
  validUntil: number;
  pictureType: string;
  pictureWidthMm: number;
  pictureHeightMm: number;
}

export interface SesSignatureOptions {
  signatureDistid?: string;
  sealDistid?: string;
  pageIndex?: number;
  boundary?: [number, number, number, number];
}

export function createSesSeal(issuerPrivateKey: Buffer | Uint8Array,
  issuerCertificate: Buffer | Uint8Array, picture: Buffer | Uint8Array,
  spec: SesSealSpec, issuerDistid?: string): Buffer;

export interface IntegrityTrustedVerificationInfo {
  manifest_complete: boolean;
  signature_valid: boolean;
  certificate_chain_valid: boolean;
  all_valid: boolean;
}

export interface Sm2ChainVerificationOptions {
  certificateDistid?: string;
  signatureDistid?: string;
}

export interface SignatureVerificationBase {
  references: Array<{path: string; valid: boolean; expected: string; actual: string}>;
  references_valid: boolean;
  description_hash: {path: string; valid: boolean; expected: string; actual: string} | null;
  seal_match: boolean | null;
  seal_signature_valid: boolean | null;
  seal_certificate_validity_valid: boolean | null;
  seal_signer_authorized: boolean | null;
  seal_validity_valid: boolean | null;
  signature_value_valid: boolean;
  certificate_validity_valid: boolean;
  all_valid: boolean;
}

export interface SignatureDirectTrustVerificationInfo extends SignatureVerificationBase {
  direct_trust_anchor_valid: boolean;
}

export interface SignatureChainVerificationInfo extends SignatureVerificationBase {
  certificate_chain_valid: boolean;
}

export interface SignatureTrustOptions {
  signerCertificate: Buffer | Uint8Array;
  signerIntermediates?: Array<Buffer | Uint8Array>;
  signerAnchor: Buffer | Uint8Array;
  sealIntermediates?: Array<Buffer | Uint8Array>;
  sealAnchor?: Buffer | Uint8Array;
  atUnixSeconds: number;
  signerCertificateDistid?: string;
  signerSignatureDistid?: string;
  sealCertificateDistid?: string;
  sealSignatureDistid?: string;
}

export interface SignatureTrustVerificationInfo {
  signature: {
    references: Array<{path: string; valid: boolean; expected: string; actual: string}>;
    references_valid: boolean;
    description_hash: {path: string; valid: boolean; expected: string; actual: string} | null;
    seal_match: boolean | null;
    seal_signature_valid: boolean | null;
    seal_certificate_validity_valid: boolean | null;
    seal_signer_authorized: boolean | null;
    seal_validity_valid: boolean | null;
    signature_value_valid: boolean;
    certificate_validity_valid: boolean;
    certificate_chain_valid: boolean;
    all_valid: boolean;
  };
  seal_issuer_chain_valid: boolean | null;
  all_valid: boolean;
}

export interface AnnotationPath {
  commands: string;
  boundary: [number, number, number, number];
  fill?: boolean;
  even_odd?: boolean;
  alpha?: number;
  stroke?: boolean;
  line_width?: number;
  cap?: 'butt' | 'round' | 'square';
  join?: 'miter' | 'round' | 'bevel';
  miter_limit?: number;
  dash_pattern?: [number, number];
  dash_sequence?: number[];
  dash_offset?: number;
  fill_color?: [number, number, number];
  fill_alpha?: number;
  stroke_color?: [number, number, number];
  stroke_alpha?: number;
}

export interface TextWatermarkStyle {
  fontId?: string | number | null;
  color?: [number, number, number];
  opacity?: number;
  angleDegrees?: number;
  tileSpacing?: [number, number] | null;
  bold?: boolean;
  italic?: boolean;
}

export type ImageAnnotationSource =
  | {resourceId: string | number; data?: never; format?: never}
  | {data: Buffer | Uint8Array; format: string; resourceId?: never};

export interface MultimediaResourceInfo {
  id: string;
  media_type: 'Image' | 'Audio' | 'Video' | null;
  format: string | null;
  path: string;
  size: number;
}

export type MediaActionRequest =
  | {kind: 'Sound'; resource_id: string; volume?: number; repeat?: boolean; synchronous?: boolean}
  | {kind: 'Movie'; resource_id: string; operator?: 'Play' | 'Stop' | 'Pause' | 'Resume'};

export type AnnotationGraphic = ({kind: 'path'} & AnnotationPath)
  | {kind: 'text'; text: string; boundary: [number, number, number, number]; size: number; font_id?: string; color?: [number, number, number]; opacity?: number}
  | {kind: 'image'; resource_id: string; boundary: [number, number, number, number]; opacity?: number}
  | {kind: 'composite'; resource_id: string; boundary: [number, number, number, number]; ctm?: [number, number, number, number, number, number]; alpha?: number; draw_param?: string};

export interface AnnotationObject {
  id: string | null;
  name: string | null;
  blend_mode: string | null;
  kind: string;
  boundary: [number, number, number, number] | null;
  ctm: [number, number, number, number, number, number] | null;
  visible: boolean;
  alpha: number;
  draw_param: string | null;
  resource_id: string | null;
  font_id: string | null;
  size: number | null;
  h_scale: number | null;
  fill: boolean | null;
  stroke: boolean | null;
  read_direction: number | null;
  char_direction: number | null;
  weight: number | null;
  italic: boolean | null;
  text: string[];
  codes: Array<{x: number | null; y: number | null; delta_x: string | null; delta_y: string | null; text: string}>;
  cg_transforms: Array<{code_position: number; code_count: number; glyph_count: number; glyphs: number[]}>;
  commands: string | null;
}

export interface PageInfo {
  index: number;
  document_index: number;
  id: string;
  path: string;
  physical_box: number[] | null;
  application_box: number[] | null;
  content_box: number[] | null;
  bleed_box: number[] | null;
}

export interface PageLayerInfo {
  id: string;
  name: string | null;
  layer_type: 'Background' | 'Body' | 'Foreground';
  draw_param: string | null;
  source_path: string;
  template_id: string | null;
  source_layer_index: number;
  object_count: number;
}

export interface PageObjectInfo {
  id: string;
  name: string | null;
  blend_mode: string | null;
  kind: string;
  layer_id: string;
  layer_type: string;
  source_layer_index: number;
  source_path: string;
  template_id: string | null;
  boundary: [number, number, number, number] | null;
  ctm: [number, number, number, number, number, number] | null;
  visible: boolean;
  alpha: number;
  draw_param: string | null;
  resource_id: string | null;
}

export interface FontInfo {
  document_index: number;
  id: string;
  font_name: string;
  family_name: string | null;
  resource_path: string;
  embedded_path: string | null;
  attributes: Record<string, string>;
}

export interface AttachmentInfo {
  document_index: number;
  id: string;
  name: string;
  path: string;
  size: number;
  format: string | null;
  creation_date: string | null;
  mod_date: string | null;
  visible: boolean;
  usage: string;
}

export interface AttachmentMetadata {
  format?: string | null;
  creation_date?: string | null;
  mod_date?: string | null;
  visible?: boolean;
  usage?: string | null;
}

export interface ColorSpaceSpec {
  color_type: 'GRAY' | 'RGB' | 'CMYK';
  bits_per_component?: 1 | 2 | 4 | 8 | 16;
  profile?: string | null;
  palette?: number[][];
}

export interface ColorSpaceInfo {
  id: string;
  color_type: string;
  bits_per_component: number;
  profile: string | null;
  palette: number[][];
  resource_path: string;
}

export interface DrawParamInfo {
  id: string;
  relative: string | null;
  xml: string;
  resource_path: string;
}

export interface SolidPaintStyle {
  color: [number, number, number];
  alpha?: number;
}

export interface GradientSegment {
  position?: number;
  color: [number, number, number];
  alpha?: number;
}

export interface GradientPaintStyle {
  kind: 'axial' | 'radial';
  start_point: [number, number];
  end_point: [number, number];
  segments: GradientSegment[];
  map_type?: 'direct' | 'repeat' | 'reflect';
  map_unit?: number;
  extend?: number;
  fill_alpha?: number;
  start_radius?: number;
  end_radius?: number;
  eccentricity?: number;
  angle?: number;
}

export interface GouraudControlPoint {
  x: number;
  y: number;
  color: [number, number, number];
  alpha?: number;
  edge_flag?: number;
}

export interface GouraudPaintStyle {
  points: GouraudControlPoint[];
  vertices_per_row?: number;
  extend?: boolean;
  back_color?: [number, number, number];
  back_alpha?: number;
}

export interface PatternPaintStyle {
  width: number;
  height: number;
  x_step?: number;
  y_step?: number;
  reflect?: 'normal' | 'row' | 'column' | 'row_and_column';
  relative_to?: 'object' | 'page';
  ctm?: [number, number, number, number, number, number];
  alpha?: number;
  thumbnail?: string;
  cells?: Record<string, unknown>[];
}

export interface ImageBorderStyle {
  line_width?: number;
  horizontal_corner_radius?: number;
  vertical_corner_radius?: number;
  dash_pattern?: number[];
  dash_offset?: number;
  color?: [number, number, number];
  color_alpha?: number;
}

export interface DrawParamStyle {
  relative?: string;
  line_width?: number;
  cap?: 'butt' | 'round' | 'square';
  join?: 'miter' | 'round' | 'bevel';
  miter_limit?: number;
  dash_pattern?: [number, number];
  dash_sequence?: number[];
  dash_offset?: number;
  fill?: SolidPaintStyle;
  stroke?: SolidPaintStyle;
  fill_gradient?: Record<string, unknown>;
  stroke_gradient?: Record<string, unknown>;
  fill_gouraud?: Record<string, unknown>;
  stroke_gouraud?: Record<string, unknown>;
  fill_pattern?: Record<string, unknown>;
  stroke_pattern?: Record<string, unknown>;
}

export interface PageArea {
  physical_box: [number, number, number, number];
  application_box?: [number, number, number, number] | null;
  content_box?: [number, number, number, number] | null;
  bleed_box?: [number, number, number, number] | null;
}

export interface FlowTextOptions {
  page_size?: [number, number];
  margins?: [number, number, number, number];
  font_size?: number;
  line_height?: number;
  paragraph_gap?: number;
  keep_paragraphs_together?: boolean;
  font_id?: string;
  align?: 'left' | 'center' | 'right';
  first_line_indent?: number;
  rest_line_indent?: number;
  first_line_indent_chars?: number;
  indent_each_paragraph?: boolean;
  shape?: boolean;
  language?: string;
}

export type TextObjectOptions = {
  size?: number;
  fill_color?: [number, number, number];
} & ({
  shape?: false;
  font_id?: string | number | null;
  language?: never;
} | {
  shape: true;
  font_id: string | number;
  language?: string;
});

export interface FlowSpan {
  text: string;
  font_id?: string;
  font_size?: number;
  language?: string;
  fill_color?: [number, number, number];
  fill?: boolean;
  stroke?: boolean;
  stroke_color?: [number, number, number];
  stroke_width?: number;
  bold?: boolean;
  weight?: number;
  italic?: boolean;
  letter_spacing?: number;
  underline?: boolean;
  underline_offset?: number;
  underline_width?: number;
  line_break_after?: boolean;
  keep_together?: boolean;
}

export interface TableOptions {
  column_widths: number[];
  row_height?: number;
  font_size?: number;
  line_height?: number;
  padding?: number;
  border_width?: number;
  page_size?: [number, number];
  margins?: [number, number, number, number];
  border_color?: [number, number, number];
  font_id?: string;
  repeat_header?: boolean;
}

export interface TableImage {
  data_base64: string;
  format: string;
  width: number;
  height: number;
}

export interface NestedTable {
  row_count: number;
  column_widths: number[];
  cells: TableCell[];
  row_height?: number;
  padding?: number;
  border_width?: number;
  border_color?: [number, number, number];
}

export type TableBlock =
  | { type: 'text'; text: string; text_color?: [number, number, number]; align?: 'left' | 'center' | 'right' }
  | ({ type: 'image' } & TableImage)
  | { type: 'spacer'; height: number }
  | { type: 'rule'; thickness: number; color?: [number, number, number] }
  | { type: 'table'; table: NestedTable };

export interface TableCell {
  row: number;
  column: number;
  row_span?: number;
  column_span?: number;
  text?: string;
  background_color?: [number, number, number];
  text_color?: [number, number, number];
  border_width?: number;
  border_color?: [number, number, number];
  padding?: [number, number, number, number];
  font_id?: string;
  font_size?: number;
  line_height?: number;
  letter_spacing?: number;
  bold?: boolean;
  italic?: boolean;
  underline?: boolean;
  strikethrough?: boolean;
  align?: 'left' | 'center' | 'right';
  vertical_align?: 'top' | 'center' | 'bottom';
  image?: TableImage;
  blocks?: TableBlock[];
}

export interface EncryptionInfo {
  id: string;
  scope: string;
  decrypt_seed_path: string;
  entries_map_path: string;
  usernames: string[];
  protection_case_id: string | null;
}

export interface PasswordRecipient {
  username: string;
  password: string;
  userType?: 'User' | 'Owner';
}

export interface CertificateRecipient {
  username: string;
  certificate: Buffer | Uint8Array;
  userType?: 'User' | 'Owner';
}

export interface KeywordPosition {
  page_index: number;
  keyword: string;
  box_mm: [number, number, number, number];
}

export interface PageSource {
  path: string;
  template_id: string | null;
  z_order: string | null;
}

export interface DocumentInfo {
  index: number;
  root: string;
  doc_id: string;
  title: string | null;
  author: string | null;
  subject: string | null;
  abstract: string | null;
  creation_date: string | null;
  mod_date: string | null;
  creator: string | null;
  creator_version: string | null;
  doc_usage: string | null;
  cover: string | null;
  keywords: string[];
  custom_data: Array<{name: string; value: string}>;
}

export interface DocumentPermissions {
  edit: boolean;
  annot: boolean;
  export: boolean;
  signature: boolean;
  watermark: boolean;
  print_screen: boolean;
  print: {printable: boolean; copies: number};
  valid_period: {start_date: string | null; end_date: string | null} | null;
}

export interface ViewPreferences {
  page_mode: string;
  page_layout: string;
  tab_display: string;
  hide_toolbar: boolean;
  hide_menubar: boolean;
  hide_window_ui: boolean;
  zoom_mode: string | null;
  zoom: number | null;
}

export interface Destination {
  page_id: string;
  kind: string;
  left?: number;
  top?: number;
  right?: number;
  bottom?: number;
  zoom?: number;
}

/** CLICK is the default event for annotation appearance graphics. */
export type AnnotationActionRequest =
  | { kind: 'uri'; event?: 'CLICK'; uri: string }
  | { kind: 'page'; event?: 'CLICK'; destination: Destination }
  | { kind: 'bookmark'; event?: 'CLICK'; name: string }
  | { kind: 'attachment'; event?: 'CLICK'; id: string; new_window?: boolean };
export type ActionRequest<Event extends string = 'DO' | 'PO' | 'CLICK'> =
  | {kind: 'uri'; event: Event; uri: string}
  | {kind: 'page'; event: Event; destination: Destination}
  | {kind: 'bookmark'; event: Event; name: string}
  | {kind: 'attachment'; event: Event; id: string; new_window?: boolean | null};

export interface ActionInfo {
  event: string;
  kind: string;
  attributes: Record<string, string>;
  destination: Destination | null;
  bookmark: string | null;
  region: Record<string, unknown> | null;
}

export interface ClipAreaInfo {
  kind: string;
  fill: boolean | null;
  stroke: boolean | null;
  boundary: [number, number, number, number] | null;
  area_ctm: number[] | null;
  shape_ctm: number[] | null;
  draw_param: string | null;
  rule: string | null;
  commands: string | null;
  font_id: string | null;
  size: number | null;
  text: string | null;
  h_scale: number | null;
  char_direction: number | null;
  text_codes: Array<{x: number; y: number; delta_x: string | null; delta_y: string | null; text: string}>;
  cg_transforms: Array<{code_position: number; code_count: number; glyph_count: number; glyphs: number[]}>;
  xml: string;
}

export interface PageObjectClipsInfo {
  id: string | null;
  kind: string;
  source_path: string;
  template_id: string | null;
  follows_ctm: boolean;
  clips: ClipAreaInfo[][];
}

export interface PathClipArea {
  boundary: [number, number, number, number];
  commands: string;
  area_ctm?: [number, number, number, number, number, number] | null;
  path_ctm?: [number, number, number, number, number, number] | null;
  even_odd?: boolean;
  fill?: boolean | null;
  stroke?: boolean | null;
  draw_param?: string | null;
}

export interface TextClipArea {
  kind: 'Text';
  boundary: [number, number, number, number];
  font_id: string;
  size: number;
  text: string;
  x: number;
  y: number;
  area_ctm?: [number, number, number, number, number, number] | null;
  text_ctm?: [number, number, number, number, number, number] | null;
  h_scale?: number;
  char_direction?: number;
  fill?: boolean | null;
  stroke?: boolean | null;
  draw_param?: string | null;
  text_codes?: Array<{x?: number | null; y?: number | null; delta_x?: string | null; delta_y?: string | null; text: string}>;
  cg_transforms?: Array<{code_position: number; code_count: number; glyph_count: number; glyphs: number[]}>;
}

export interface PaintInfo {
  kind: string;
  alpha: number;
  value: string | null;
  color_space: string | null;
  color_index: string | null;
  xml: string;
}

export interface PageObjectPaintInfo {
  id: string | null;
  kind: string;
  source_path: string;
  template_id: string | null;
  fill_enabled: boolean;
  stroke_enabled: boolean;
  fill: PaintInfo | null;
  stroke: PaintInfo | null;
  effective_fill: PaintInfo | null;
  effective_stroke: PaintInfo | null;
  effective_fill_draw_param: string | null;
  effective_stroke_draw_param: string | null;
}

export interface PageObjectStrokeStyleInfo {
  id: string | null;
  kind: string;
  source_path: string;
  template_id: string | null;
  stroke_enabled: boolean;
  style: {
    line_width: number;
    cap: string | null;
    join: string | null;
    miter_limit: number | null;
    dash_pattern: [number, number] | null;
    dash_sequence: number[] | null;
    dash_offset: number;
  };
}

export interface PathStrokeStyle {
  line_width: number;
  cap?: 'butt' | 'round' | 'square';
  join?: 'miter' | 'round' | 'bevel';
  miter_limit?: number;
  dash_pattern?: [number, number];
  dash_sequence?: number[];
  dash_offset?: number;
}

export interface TextTypographyStyle {
  size: number;
  h_scale?: number;
  read_direction?: number;
  char_direction?: number;
  weight?: number;
  italic?: boolean;
}

export interface TextCodeLayout {
  x?: number;
  y?: number;
  delta_x?: number[];
  delta_y?: number[];
}

export interface CustomTagInfo {
  document_index: number;
  type_id: string;
  namespace: string | null;
  schema_path: string | null;
  path: string;
}

export interface OutlineInfo {
  document_index: number;
  title: string;
  depth: number;
  parent_index: number | null;
  expanded: boolean;
  count: number | null;
  destination: Destination | null;
  bookmark_name: string | null;
}

export interface VersionInfo {
  document_index: number;
  id: string;
  index: number;
  current: boolean;
  description_path: string;
  doc_root: string;
  format_version: string | null;
  name: string | null;
  creation_date: string | null;
  files: Array<{id: string; path: string}>;
}

export interface ExtensionInfo {
  document_index: number;
  app_name: string;
  company: string | null;
  app_version: string | null;
  date: string | null;
  ref_id: number | null;
  properties: Array<{name: string; type_name: string | null; value: string}>;
  extend_data_paths: string[];
  data_xml: string[];
}

export type NewExtension = Pick<ExtensionInfo, 'document_index' | 'app_name'> &
  Partial<Omit<ExtensionInfo, 'document_index' | 'app_name'>>;

export interface ArchiveViolation {
  rule: string;
  severity: 'ERROR' | 'WARN' | 'INFO';
  message: string;
  path: string | null;
  actual: string | null;
  expected: string | null;
}

export interface RenderOptions {
  preferImageSubstitution?: boolean;
}

export class Document {
  static open(path: string): Document;
  static fromBytes(data: Buffer | Uint8Array): Document;
  static fromPdfBytes(data: Buffer | Uint8Array): Document;
  static create(): Document;
  close(): void;
  readonly pageCount: number;
  readonly isEncrypted: boolean;
  encryptionInfo(): EncryptionInfo[];
  decryptWithPassword(password: string, username?: string | null): Document;
  decryptWithSm2PrivateKey(privateKey: Buffer | Uint8Array, username?: string | null): Document;
  encryptWithPasswords(recipients: PasswordRecipient[]): Document;
  encryptWithCertificates(recipients: CertificateRecipient[]): Document;
  encryptWithRecipients(recipients: (PasswordRecipient & {type: 'password'} | CertificateRecipient & {type: 'certificate'})[]): Document;
  pages(): PageInfo[];
  documents(): DocumentInfo[];
  packageInfo(): {version: string | null; doc_type: string | null};
  setPackageHeader(version: string, docType: string): void;
  setDocumentInfoField(documentIndex: number, field: string, value: string): void;
  removeDocumentInfoField(documentIndex: number, field: string): void;
  addDocumentKeyword(documentIndex: number, keyword: string): void;
  removeDocumentKeyword(documentIndex: number, keywordIndex: number): void;
  setDocumentCustomData(documentIndex: number, name: string, value: string): void;
  removeDocumentCustomData(documentIndex: number, name: string): void;
  documentPermissions(documentIndex: number): DocumentPermissions;
  setDocumentPermissions(documentIndex: number, permissions: DocumentPermissions): void;
  viewPreferences(documentIndex: number): ViewPreferences;
  setViewPreferences(documentIndex: number, preferences: ViewPreferences): void;
  documentPageArea(documentIndex: number): PageArea | null;
  setDocumentPageArea(documentIndex: number, area: PageArea): void;
  setPageArea(page: number, area: PageArea): void;
  documentActions(documentIndex: number): ActionInfo[];
  addDocumentAction(documentIndex: number, action: ActionRequest): void;
  removeDocumentAction(documentIndex: number, actionIndex: number): void;
  pageActions(page: number): ActionInfo[];
  addPageAction(page: number, action: ActionRequest<'PO'>): void;
  removePageAction(page: number, actionIndex: number): void;
  customTags(): CustomTagInfo[];
  customTagData(tagIndex: number): Buffer;
  replaceCustomTagData(tagIndex: number, data: Buffer | Uint8Array): CustomTagInfo;
  addCustomTag(documentIndex: number, typeId: string, data: Buffer | Uint8Array, options?: {namespace?: string | null; schemaPath?: string | null}): CustomTagInfo;
  updateCustomTag(tagIndex: number, typeId: string, options?: {namespace?: string | null; schemaPath?: string | null}): CustomTagInfo;
  removeCustomTag(tagIndex: number): void;
  extensions(): ExtensionInfo[];
  addExtension(extension: NewExtension): void;
  updateExtension(extensionIndex: number, extension: NewExtension): void;
  removeExtension(extensionIndex: number): void;
  appendDocumentBody(): number;
  moveDocumentBody(from: number, to: number): void;
  removeDocumentBody(documentIndex: number): void;
  extractDocumentBody(documentIndex: number): Document;
  appendDocumentBodiesFrom(source: Document): number[];
  appendDocumentBodyFrom(source: Document, documentIndex: number): number;
  entries(): string[];
  entry(path: string): Buffer;
  setEntry(path: string, data: Buffer | Uint8Array): void;
  removeEntry(path: string): boolean;
  pageText(page: number): string[];
  allText(): string[][];
  pageTextInRegion(page: number, boundary: [number, number, number, number]): string[];
  keywordPositions(keywords: string[], pages?: number[] | null): KeywordPosition[];
  pageTextWithTemplates(page: number): string[];
  pageSources(page: number): PageSource[];
  pageLayers(page: number): PageLayerInfo[];
  addPageLayer(page: number, layerType: 'Background' | 'Body' | 'Foreground', drawParam?: string | null): string;
  removePageLayer(page: number, layerId: string): void;
  setPageLayerType(page: number, layerId: string, layerType: 'Background' | 'Body' | 'Foreground'): void;
  setPageLayerName(page: number, layerId: string, name?: string | null): void;
  setPageLayerDrawParam(page: number, layerId: string, drawParam?: string | null): void;
  movePageObjectToLayer(page: number, objectId: string, layerId: string): void;
  movePageObject(page: number, objectId: string, beforeId?: string | null): void;
  groupPageObjects(page: number, firstId: string, lastId: string): void;
  ungroupPageObjects(page: number, objectId: string): void;
  pageObjects(page: number): PageObjectInfo[];
  setPageObjectBoundary(page: number, objectId: string, box: [number, number, number, number]): void;
  setPageObjectCtm(page: number, objectId: string, ctm?: [number, number, number, number, number, number] | null): void;
  setPageObjectVisible(page: number, objectId: string, visible: boolean): void;
  setPageObjectAlpha(page: number, objectId: string, alpha: number): void;
  setPagePathData(page: number, objectId: string, commands: string): void;
  setPagePathStrokeStyle(page: number, objectId: string, style: PathStrokeStyle): void;
  setPagePathDashSequence(page: number, objectId: string, dashSequence?: number[] | null, dashOffset?: number | null): void;
  setPageTextTypography(page: number, objectId: string, style: TextTypographyStyle): void;
  setPageTextCode(page: number, objectId: string, codeIndex: number, value: string): void;
  setPageTextCodeLayout(page: number, objectId: string, codeIndex: number, layout: TextCodeLayout): void;
  appendPageTextCode(page: number, objectId: string, value: string, layout?: TextCodeLayout): void;
  removePageTextCode(page: number, objectId: string, codeIndex: number): void;
  setPageTextFont(page: number, objectId: string, fontId: string): void;
  setPageTextPaintFlags(page: number, objectId: string, options?: {fill?: boolean | null; stroke?: boolean | null}): void;
  setPageObjectName(page: number, objectId: string, name?: string | null): void;
  setPageObjectBlendMode(page: number, objectId: string, blendMode?: string | null): void;
  setPageObjectDrawParam(page: number, objectId: string, drawParam?: string | null): void;
  setPageObjectSolidFill(page: number, objectId: string, style: SolidPaintStyle): void;
  setPageObjectSolidStroke(page: number, objectId: string, style: SolidPaintStyle): void;
  setPageObjectGradientFill(page: number, objectId: string, style: GradientPaintStyle): void;
  setPageObjectGradientStroke(page: number, objectId: string, style: GradientPaintStyle): void;
  setPageObjectGouraudFill(page: number, objectId: string, style: GouraudPaintStyle): void;
  setPageObjectGouraudStroke(page: number, objectId: string, style: GouraudPaintStyle): void;
  setPageObjectPatternFill(page: number, objectId: string, style: PatternPaintStyle): void;
  setPageObjectPatternStroke(page: number, objectId: string, style: PatternPaintStyle): void;
  /** false: nearest; true: smooth; null/omitted: restore smooth default. */
  setPageImageInterpolate(page: number, objectId: string, interpolate?: boolean | null): void;
  setPageImageMask(page: number, objectId: string, resourceId?: string | null): void;
  setPageImageSubstitution(page: number, objectId: string, resourceId?: string | null): void;
  setPageImageBorder(page: number, objectId: string, style?: ImageBorderStyle | null): void;
  pageObjectClips(page: number): PageObjectClipsInfo[];
  setPageObjectClipRects(page: number, objectId: string, rects: Array<[number, number, number, number]>, followsCtm?: boolean): void;
  setPageObjectPathClips(page: number, objectId: string, clips: PathClipArea[][], followsCtm?: boolean): void;
  setPageObjectClips(page: number, objectId: string, clips: Array<Array<(PathClipArea & {kind: 'Path'}) | TextClipArea>>, followsCtm?: boolean): void;
  pageObjectPaints(page: number): PageObjectPaintInfo[];
  pageObjectStrokeStyles(page: number): PageObjectStrokeStyleInfo[];
  pageObjectActions(page: number, objectId: string): ActionInfo[];
  pageSourceObjectActions(page: number, sourcePath: string, objectId: string): ActionInfo[];
  addPageObjectUriAction(page: number, objectId: string, uri: string): void;
  addPageObjectGotoAction(page: number, objectId: string, destination: Destination): void;
  addPageObjectBookmarkAction(page: number, objectId: string, bookmarkName: string): void;
  addPageObjectAttachmentAction(page: number, objectId: string, attachmentId: string, newWindow?: boolean | null): void;
  addPageObjectMediaAction(page: number, objectId: string, action: MediaActionRequest): void;
  addPageObjectSoundAction(page: number, objectId: string, resourceId: string | number, options?: {volume?: number; repeat?: boolean; synchronous?: boolean}): void;
  addPageObjectMovieAction(page: number, objectId: string, resourceId: string | number, operator?: 'Play' | 'Stop' | 'Pause' | 'Resume' | null): void;
  removePageObjectAction(page: number, objectId: string, actionIndex: number): void;
  setPageObjectActionRegions(page: number, objectId: string, actionIndex: number, rectangles: Array<[number, number, number, number]>): void;
  pageTextObjects(page: number): Record<string, unknown>[];
  pagePaths(page: number): Record<string, unknown>[];
  pageImages(page: number): Record<string, unknown>[];
  pageCompositeImages(page: number): Record<string, unknown>[];
  pageCompositeObjects(page: number): Record<string, unknown>[];
  pageCompositeUnits(page: number): Record<string, unknown>[];
  setPageCompositeUnitImages(page: number, unitId: string, options?: {thumbnailId?: string | null; substitutionId?: string | null}): void;
  pageCompositeImageData(page: number, image: number): Buffer;
  pageImageData(page: number, image: number): Buffer;
  validateReferences(): Record<string, unknown>[];
  archiveViolations(): ArchiveViolation[];
  prepareArchive(): Document;
  bookmarks(): Array<{document_index: number; name: string; destination: Destination}>;
  addBookmark(documentIndex: number, name: string, destination: Destination): void;
  removeBookmark(documentIndex: number, name: string): void;
  outlines(): OutlineInfo[];
  addOutline(documentIndex: number, title: string, options?: {parentIndex?: number | null; destination?: Destination | null; expanded?: boolean}): number;
  removeOutline(outlineIndex: number): void;
  setOutlineTitle(outlineIndex: number, title: string): void;
  setOutlineExpanded(outlineIndex: number, expanded: boolean): void;
  setOutlineDestination(outlineIndex: number, destination: Destination): void;
  setOutlineBookmark(outlineIndex: number, name: string): void;
  versions(): VersionInfo[];
  addVersionSnapshot(documentIndex: number, id: string, name?: string | null): VersionInfo;
  extractVersionSnapshot(versionIndex: number): Document;
  annotations(): Record<string, unknown>[];
  annotationObjects(page: number, annotationId: string | number): AnnotationObject[];
  annotationObjectActions(page: number, annotationId: string | number, objectId: string | number): ActionInfo[];
  addAnnotationObjectAction(page: number, annotationId: string | number, objectId: string | number, action: AnnotationActionRequest): void;
  removeAnnotationObjectAction(page: number, annotationId: string | number, objectId: string | number, actionIndex: number): void;
  setAnnotationObjectActionRegions(page: number, annotationId: string | number, objectId: string | number, actionIndex: number, rectangles: Array<[number, number, number, number]>): void;

  appendAnnotationObjects(page: number, annotationId: string | number, objects: AnnotationGraphic[]): string[];
  updateAnnotationObject(page: number, annotationId: string | number, objectId: string | number, update: {boundary?: [number, number, number, number]; ctm?: [number, number, number, number, number, number] | null; visible?: boolean; alpha?: number}): void;
  setAnnotationObjectName(page: number, annotationId: string | number, objectId: string | number, name?: string | null): void;
  setAnnotationObjectBlendMode(page: number, annotationId: string | number, objectId: string | number, blendMode?: string | null): void;
  setAnnotationObjectDrawParam(page: number, annotationId: string | number, objectId: string | number, drawParam?: string | number | null): void;
  removeAnnotationObject(page: number, annotationId: string | number, objectId: string | number): void;
  setAnnotationPathData(page: number, annotationId: string | number, objectId: string | number, commands: string): void;
  setAnnotationPathStrokeStyle(page: number, annotationId: string | number, objectId: string | number, style: PathStrokeStyle): void;
  setAnnotationTextCode(page: number, annotationId: string | number, objectId: string | number, codeIndex: number, text: string): void;
  appendAnnotationTextCode(page: number, annotationId: string | number, objectId: string | number, text: string, layout?: TextCodeLayout): void;
  setAnnotationTextCodeLayout(page: number, annotationId: string | number, objectId: string | number, codeIndex: number, layout: TextCodeLayout): void;
  setAnnotationTextTypography(page: number, annotationId: string | number, objectId: string | number, style: TextTypographyStyle): void;
  setAnnotationTextPaintFlags(page: number, annotationId: string | number, objectId: string | number, options?: {fill?: boolean | null; stroke?: boolean | null}): void;
  removeAnnotationTextCode(page: number, annotationId: string | number, objectId: string | number, codeIndex: number): void;
  setAnnotationTextFont(page: number, annotationId: string | number, objectId: string | number, fontId: string | number): void;
  setAnnotationGraphicResource(page: number, annotationId: string | number, objectId: string | number, resourceId: string | number): void;
  addPathAnnotation(page: number, commands: string, boundary: [number, number, number, number], options?: Record<string, unknown>): string;
  addMultiPathAnnotation(page: number, boundary: [number, number, number, number], paths: AnnotationPath[]): string;
  addMixedAnnotation(page: number, boundary: [number, number, number, number], objects: AnnotationGraphic[]): string;
  addMixedStampAnnotation(page: number, boundary: [number, number, number, number], objects: AnnotationGraphic[]): string;
  addHighlightAnnotation(page: number, boundary: [number, number, number, number], color: [number, number, number], opacity?: number): string;
  addTextNoteAnnotation(page: number, text: string, boundary: [number, number, number, number], options?: {color?: [number, number, number]; opacity?: number; icon?: 'Note' | 'Comment' | 'Help' | 'Insert' | 'Key' | 'NewParagraph' | 'Paragraph'; open?: boolean}): string;
  addShapeAnnotation(page: number, kind: 'Square' | 'Circle', boundary: [number, number, number, number], options?: {strokeColor?: [number, number, number] | null; fillColor?: [number, number, number] | null; lineWidth?: number; dashSequence?: number[]; opacity?: number; insets?: [number, number, number, number]}): string;
  addLineAnnotation(page: number, boundary: [number, number, number, number], endpoints: [number, number, number, number], options?: {strokeColor?: [number, number, number] | null; lineWidth?: number; dashSequence?: number[]; opacity?: number}): string;
  addUriLinkAnnotation(page: number, uri: string, boundary: [number, number, number, number], quadPoints?: Array<[[number, number], [number, number], [number, number], [number, number]]>): string;
  addGotoLinkAnnotation(page: number, destination: Destination, boundary: [number, number, number, number], quadPoints?: Array<[[number, number], [number, number], [number, number], [number, number]]>): string;
  addBookmarkLinkAnnotation(page: number, bookmarkName: string, boundary: [number, number, number, number]): string;
  addAttachmentLinkAnnotation(page: number, attachmentId: string | number, boundary: [number, number, number, number], newWindow?: boolean | null): string;
  addMediaLinkAnnotation(page: number, action: MediaActionRequest & {boundary: [number, number, number, number]}): string;
  addSoundLinkAnnotation(page: number, resourceId: string | number, boundary: [number, number, number, number], options?: {volume?: number; repeat?: boolean; synchronous?: boolean}): string;
  addMovieLinkAnnotation(page: number, resourceId: string | number, boundary: [number, number, number, number], operator?: 'Play' | 'Stop' | 'Pause' | 'Resume' | null): string;
  addTextWatermark(page: number, text: string, boundary: [number, number, number, number], fontSize: number, style?: TextWatermarkStyle): string;
  addTextStampAnnotation(page: number, text: string, boundary: [number, number, number, number], fontSize: number, style?: TextWatermarkStyle): string;
  addTextWatermarkToAllPages(text: string, fontSize: number, style?: TextWatermarkStyle): string[];
  addImageWatermark(page: number, boundary: [number, number, number, number], source: ImageAnnotationSource, opacity?: number): string;
  addImageStampAnnotation(page: number, boundary: [number, number, number, number], source: ImageAnnotationSource, opacity?: number): string;
  addImageWatermarkToAllPages(data: Buffer | Uint8Array, format: string, boundary: [number, number, number, number], opacity?: number): string[];
  removeAnnotation(page: number, annotationId: string | number): void;
  setAnnotationCreator(page: number, annotationId: string | number, creator: string): void;
  setAnnotationSubtype(page: number, annotationId: string | number, subtype?: string | null): void;
  setAnnotationLastModDate(page: number, annotationId: string | number, date: string): void;
  setAnnotationAppearanceBoundary(page: number, annotationId: string | number, box: [number, number, number, number]): void;
  setAnnotationRemark(page: number, annotationId: string | number, remark?: string | null): void;
  setAnnotationParameter(page: number, annotationId: string | number, name: string, value: string): void;
  removeAnnotationParameter(page: number, annotationId: string | number, name: string): void;
  setAnnotationVisible(page: number, annotationId: string | number, visible: boolean): void;
  setAnnotationPrint(page: number, annotationId: string | number, print: boolean): void;
  setAnnotationFlag(page: number, annotationId: string | number, flag: 'NoZoom' | 'NoRotate' | 'ReadOnly', value: boolean): void;
  integrityManifest(): Record<string, unknown> | null;
  checkIntegrityManifest(): {missing_files: string[]; extra_files: string[]; complete: boolean} | null;
  verifyIntegrityManifestSm2(signatureDistid?: string | null): {manifest_complete: boolean; signature_valid: boolean; all_valid: boolean} | null;
  verifyIntegrityManifestSm2WithOrderedChain(signerCertificate: Buffer | Uint8Array, intermediates: Array<Buffer | Uint8Array>, anchorCertificate: Buffer | Uint8Array, atUnixSeconds: number, options?: Sm2ChainVerificationOptions): IntegrityTrustedVerificationInfo | null;
  verifyIntegrityManifestSm2WithCertificatePool(signerCertificate: Buffer | Uint8Array, candidates: Array<Buffer | Uint8Array>, anchorCertificate: Buffer | Uint8Array, atUnixSeconds: number, options?: Sm2ChainVerificationOptions): IntegrityTrustedVerificationInfo | null;
  attachments(): AttachmentInfo[];
  attachmentData(attachment: number): Buffer;
  signatures(): Record<string, unknown>[];
  signedValueData(signature: number): Buffer;
  sealData(signature: number): Buffer;
  checkSignatureReferences(signature: number): Array<{path: string; valid: boolean; expected: string; actual: string}>;
  checkSignatureDescriptionHash(signature: number): {path: string; valid: boolean; expected: string; actual: string};
  checkSignatureSealMatch(signature: number): boolean | null;
  checkDigitalSignatureWithCertificate(signature: number, certificate: Buffer | Uint8Array, signatureDistid?: string): boolean;
  verifySignatureWithDirectTrustAnchor(signature: number, signerCertificate: Buffer | Uint8Array, anchorCertificate: Buffer | Uint8Array, atUnixSeconds: number, options?: Sm2ChainVerificationOptions): SignatureDirectTrustVerificationInfo;
  verifySignatureWithOrderedSm2Chain(signature: number, signerCertificate: Buffer | Uint8Array, intermediates: Array<Buffer | Uint8Array>, anchorCertificate: Buffer | Uint8Array, atUnixSeconds: number, options?: Sm2ChainVerificationOptions): SignatureChainVerificationInfo;
  verifySignatureWithCertificatePool(signature: number, signerCertificate: Buffer | Uint8Array, candidates: Array<Buffer | Uint8Array>, anchorCertificate: Buffer | Uint8Array, atUnixSeconds: number, options?: Sm2ChainVerificationOptions): SignatureChainVerificationInfo;
  verifySignatureWithOrderedSm2Chains(signature: number, options: SignatureTrustOptions): SignatureTrustVerificationInfo;
  verifySignatureWithCertificatePools(signature: number, options: SignatureTrustOptions): SignatureTrustVerificationInfo;
  checkSesSignatureValue(signature: number): boolean;
  checkSesSignatureValueWithDistid(signature: number, signatureDistid: string): boolean;
  checkSesSealSignature(signature: number): boolean;
  checkSesSealSignatureWithDistid(signature: number, sealDistid: string): boolean;
  checkSesSealCertificateValidity(signature: number, atUnixSeconds: number): boolean;
  checkSignatureCertificateValidity(signature: number, certificate: Buffer | Uint8Array, atUnixSeconds: number): boolean;
  checkSignatureDirectTrustAnchor(signature: number, signerCertificate: Buffer | Uint8Array, anchorCertificate: Buffer | Uint8Array, atUnixSeconds: number, certificateDistid?: string): boolean;
  checkSesSealIssuerOrderedSm2Chain(signature: number, intermediates: Array<Buffer | Uint8Array>, anchorCertificate: Buffer | Uint8Array, atUnixSeconds: number, certificateDistid?: string): boolean;
  checkSesSealSignerAuthorization(signature: number): boolean;
  checkSesSealValidity(signature: number, atUnixSeconds: number): boolean;
  pageSvg(page: number, options?: RenderOptions): string;
  pagePng(page: number, dpi?: number, options?: RenderOptions): Buffer;
  toPdf(options?: RenderOptions): Buffer;
  toBytes(): Buffer;
  save(path: string): void;
  addTextPage(text: string, width?: number, height?: number): void;
  addTextPageWithFont(text: string, fontId: string | number, width?: number, height?: number): void;
  addBlankPage(width?: number, height?: number): void;
  addImagePage(data: Buffer | Uint8Array, format: string, width?: number, height?: number): void;
  addPathPage(commands: string, boundary: [number, number, number, number], pageWidth?: number, pageHeight?: number, fill?: boolean, stroke?: boolean): void;
  addFlowedText(documentIndex: number, text: string, options?: FlowTextOptions): number[];
  addFlowedSpans(documentIndex: number, spans: FlowSpan[], options?: FlowTextOptions): number[];
  addEmbeddedFont(name: string, data: Buffer | Uint8Array): string;
  addEmbeddedFontToDocument(documentIndex: number, name: string, data: Buffer | Uint8Array): string;
  addEmbeddedFontToPage(pageIndex: number, name: string, data: Buffer | Uint8Array): string;
  setFontAttribute(fontIndex: number, name: 'FontName' | 'FamilyName' | 'Charset' | 'Italic' | 'Bold' | 'Serif' | 'FixedWidth', value?: string | null): void;
  setPageFontAttribute(pageIndex: number, fontIndex: number, name: 'FontName' | 'FamilyName' | 'Charset' | 'Italic' | 'Bold' | 'Serif' | 'FixedWidth', value?: string | null): void;
  replaceFontData(fontIndex: number, data: Uint8Array): void;
  replacePageFontData(pageIndex: number, fontIndex: number, data: Uint8Array): void;
  fonts(): FontInfo[];
  fontData(fontIndex: number): Buffer;
  pageFonts(page: number): FontInfo[];
  pageFontData(page: number, fontIndex: number): Buffer;
  documentColorSpaces(documentIndex: number): ColorSpaceInfo[];
  pageColorSpaces(page: number): ColorSpaceInfo[];
  documentDrawParams(documentIndex: number): DrawParamInfo[];
  pageDrawParams(page: number): DrawParamInfo[];
  documentDefaultColorSpace(documentIndex: number): string | null;
  setDocumentDefaultColorSpace(documentIndex: number, colorSpaceId?: string | null): void;
  addDocumentColorSpace(documentIndex: number, spec: ColorSpaceSpec): string;
  addPageColorSpace(page: number, spec: ColorSpaceSpec): string;
  setDocumentColorSpace(documentIndex: number, id: string, spec: ColorSpaceSpec): void;
  setPageColorSpace(page: number, id: string, spec: ColorSpaceSpec): void;
  removeDocumentColorSpace(documentIndex: number, id: string): void;
  removePageColorSpace(page: number, id: string): void;
  addDocumentDrawParam(documentIndex: number, style: DrawParamStyle): string;
  addPageDrawParam(page: number, style: DrawParamStyle): string;
  setDocumentDrawParam(documentIndex: number, id: string, style: DrawParamStyle): void;
  setPageDrawParam(page: number, id: string, style: DrawParamStyle): void;
  removeDocumentDrawParam(documentIndex: number, id: string): void;
  removePageDrawParam(page: number, id: string): void;
  addTable(documentIndex: number, rows: string[][], options: TableOptions): number[];
  addTableCells(documentIndex: number, rowCount: number, cells: TableCell[], options: TableOptions): number[];
  appendTextPage(documentIndex: number, text: string, width?: number, height?: number): void;
  appendTextPageWithFont(documentIndex: number, text: string, fontId: string | number, width?: number, height?: number): void;
  appendBlankPage(documentIndex: number, width?: number, height?: number): void;
  appendImagePage(documentIndex: number, data: Buffer | Uint8Array, format: string, width?: number, height?: number): void;
  appendPathPage(documentIndex: number, commands: string, boundary: [number, number, number, number], pageWidth?: number, pageHeight?: number, fill?: boolean, stroke?: boolean): void;
  insertTextPage(documentIndex: number, position: number, text: string, width?: number, height?: number): number;
  insertBlankPage(documentIndex: number, position: number, width?: number, height?: number): number;
  addTextObject(page: number, text: string, boundary: [number, number, number, number], options?: TextObjectOptions): string;
  addPathObject(page: number, commands: string, boundary: [number, number, number, number], options?: Record<string, unknown>): string;
  addImageObject(page: number, data: Buffer | Uint8Array, format: string, boundary: [number, number, number, number]): string;
  addMultimediaResource(documentIndex: number, mediaType: 'Image' | 'Audio' | 'Video', format: string, data: Buffer | Uint8Array): string;
  pageMultimediaResources(page: number): MultimediaResourceInfo[];
  pageMultimediaData(page: number, resourceId: string): Buffer;
  documentMultimediaResources(documentIndex: number): MultimediaResourceInfo[];
  documentMultimediaData(documentIndex: number, resourceId: string): Buffer;
  addPageMultimediaResource(page: number, mediaType: 'Image' | 'Audio' | 'Video', format: string, data: Buffer | Uint8Array): string;
  replaceDocumentMultimediaData(documentIndex: number, resourceId: string, data: Buffer | Uint8Array): void;
  replacePageMultimediaData(page: number, resourceId: string, data: Buffer | Uint8Array): void;
  removeDocumentMultimediaResource(documentIndex: number, resourceId: string): void;
  removePageMultimediaResource(page: number, resourceId: string): void;
  addCompositePathObject(page: number, commands: string, boundary: [number, number, number, number], options?: Record<string, unknown>): string;
  addCompositeTextObject(page: number, text: string, boundary: [number, number, number, number], fontSize?: number, fontId?: string | number | null): string;
  addCompositeObject(page: number, resourceId: string, boundary: [number, number, number, number], options?: Record<string, unknown>): string;
  addCompositeImageObject(page: number, data: Buffer | Uint8Array, format: string, boundary: [number, number, number, number]): string;
  addAttachment(name: string, data: Buffer | Uint8Array): void;
  addAttachmentToDocument(documentIndex: number, name: string, data: Buffer | Uint8Array): void;
  removeAttachment(attachment: number): void;
  renameAttachment(attachment: number, name: string): void;
  replaceAttachmentData(attachment: number, data: Buffer | Uint8Array): void;
  setAttachmentMetadata(attachment: number, metadata: AttachmentMetadata): void;
  addDigitalSignature(privateKey: Buffer | Uint8Array, dateTime: string, signatureDistid?: string): number;
  addSesSignature(privateKey: Buffer | Uint8Array, certificate: Buffer | Uint8Array,
    seal: Buffer | Uint8Array, dateTime: string, options?: SesSignatureOptions): number;
  protectIntegritySm2(privateKey: Buffer | Uint8Array, certificate: Buffer | Uint8Array, signatureDistid?: string | null): void;
  duplicatePage(page: number): number;
  duplicatePageAt(page: number, position: number): number;
  movePage(from: number, to: number): void;
  reorderDocumentPages(documentIndex: number, order: number[]): void;
  removePage(page: number): void;
  promotePageToTemplate(page: number, additionalPages?: number[], zOrder?: 'Background' | 'Body' | 'Foreground'): string;
  applyPageTemplate(page: number, templateId: string, zOrder?: 'Background' | 'Body' | 'Foreground'): void;
  removePageTemplate(page: number, templateId: string): void;
  removeDocumentTemplate(documentIndex: number, templateId: string): void;
  duplicatePageObject(page: number, objectId: string): string;
  removePageObject(page: number, objectId: string): void;
}
