'use strict';

const path = require('node:path');
const koffi = require('koffi');

class OfdError extends Error {
  constructor(message) {
    super(message);
    this.name = 'OfdError';
  }
}

function libraryName() {
  if (process.platform === 'win32') return 'ofdrw_ffi.dll';
  if (process.platform === 'darwin') return 'libofdrw_ffi.dylib';
  return 'libofdrw_ffi.so';
}

const name = libraryName();
const override = process.env.OFDRW_LIB_PATH;
const candidates = override ? [override] : [
  path.join(__dirname, 'native', `${process.platform}-${process.arch}`, name),
  path.join(__dirname, name),
  path.resolve(__dirname, '../../target/debug', name),
  name,
];
let library;
for (const candidate of candidates) {
  try {
    library = koffi.load(candidate);
    break;
  } catch (error) {
    if (candidate === candidates.at(-1)) {
      if (override) throw new OfdError(`cannot load OFDRW_LIB_PATH: ${override}`);
      throw new OfdError('OFD native library not found; set OFDRW_LIB_PATH');
    }
  }
}

const OfdDocument = koffi.pointer('OfdDocument', koffi.opaque());
const OfdBuffer = koffi.struct('OfdBuffer', { data: 'uint8_t *', len: 'size_t' });
const api = {
  abiVersion: library.func('uint32_t ofd_abi_version(void)'),
  lastError: library.func('str ofd_last_error(void)'),
  open: library.func('int ofd_open(str path, _Out_ OfdDocument *output)'),
  fromBytes: library.func('int ofd_from_bytes(const uint8_t *data, size_t len, _Out_ OfdDocument *output)'),
  fromPdfBytes: library.func('int ofd_from_pdf_bytes(const uint8_t *data, size_t len, _Out_ OfdDocument *output)'),
  save: library.func('int ofd_save(OfdDocument handle, str path)'),
  create: library.func('int ofd_new(_Out_ OfdDocument *output)'),
  isEncrypted: library.func('int ofd_is_encrypted(OfdDocument handle, _Out_ int *output)'),
  encryptionInfo: library.func('int ofd_encryption_info_json(OfdDocument handle, _Out_ OfdBuffer *output)'),
  decryptWithPassword: library.func('int ofd_decrypt_with_password(OfdDocument handle, str password, str username, _Out_ OfdDocument *output)'),
  decryptWithSm2PrivateKey: library.func('int ofd_decrypt_with_sm2_private_key(OfdDocument handle, const uint8_t *key, size_t len, str username, _Out_ OfdDocument *output)'),
  encryptWithPasswords: library.func('int ofd_encrypt_with_passwords_json(OfdDocument handle, str recipients, _Out_ OfdDocument *output)'),
  encryptWithCertificates: library.func('int ofd_encrypt_with_certificates_json(OfdDocument handle, str recipients, _Out_ OfdDocument *output)'),
  encryptWithRecipients: library.func('int ofd_encrypt_with_recipients_json(OfdDocument handle, str recipients, _Out_ OfdDocument *output)'),
  close: library.func('void ofd_close(OfdDocument handle)'),
  pageCount: library.func('int ofd_page_count(OfdDocument handle, _Out_ size_t *output)'),
  documents: library.func('int ofd_documents_json(OfdDocument handle, _Out_ OfdBuffer *output)'),
  packageInfo: library.func('int ofd_package_info_json(OfdDocument handle, _Out_ OfdBuffer *output)'),
  setPackageHeader: library.func('int ofd_set_package_header(OfdDocument handle, str version, str doc_type)'),
  setDocumentInfoField: library.func('int ofd_set_document_info_field(OfdDocument handle, size_t document_index, str field, str value)'),
  removeDocumentInfoField: library.func('int ofd_remove_document_info_field(OfdDocument handle, size_t document_index, str field)'),
  addDocumentKeyword: library.func('int ofd_add_document_keyword(OfdDocument handle, size_t document_index, str keyword)'),
  removeDocumentKeyword: library.func('int ofd_remove_document_keyword(OfdDocument handle, size_t document_index, size_t keyword_index)'),
  setDocumentCustomData: library.func('int ofd_set_document_custom_data(OfdDocument handle, size_t document_index, str name, str value)'),
  removeDocumentCustomData: library.func('int ofd_remove_document_custom_data(OfdDocument handle, size_t document_index, str name)'),
  documentPermissions: library.func('int ofd_document_permissions_json(OfdDocument handle, size_t document_index, _Out_ OfdBuffer *output)'),
  setDocumentPermissions: library.func('int ofd_set_document_permissions_json(OfdDocument handle, size_t document_index, str permissions)'),
  viewPreferences: library.func('int ofd_view_preferences_json(OfdDocument handle, size_t document_index, _Out_ OfdBuffer *output)'),
  setViewPreferences: library.func('int ofd_set_view_preferences_json(OfdDocument handle, size_t document_index, str preferences)'),
  documentPageArea: library.func('int ofd_document_page_area_json(OfdDocument handle, size_t document_index, _Out_ OfdBuffer *output)'),
  setDocumentPageArea: library.func('int ofd_set_document_page_area_json(OfdDocument handle, size_t document_index, str area)'),
  setPageArea: library.func('int ofd_set_page_area_json(OfdDocument handle, size_t page, str area)'),
  documentActions: library.func('int ofd_document_actions_json(OfdDocument handle, size_t document_index, _Out_ OfdBuffer *output)'),
  addDocumentAction: library.func('int ofd_add_document_action_json(OfdDocument handle, size_t document_index, str action)'),
  removeDocumentAction: library.func('int ofd_remove_document_action(OfdDocument handle, size_t document_index, size_t action_index)'),
  pageActions: library.func('int ofd_page_actions_json(OfdDocument handle, size_t page, _Out_ OfdBuffer *output)'),
  addPageAction: library.func('int ofd_add_page_action_json(OfdDocument handle, size_t page, str action)'),
  removePageAction: library.func('int ofd_remove_page_action(OfdDocument handle, size_t page, size_t action_index)'),
  customTags: library.func('int ofd_custom_tags_json(OfdDocument handle, _Out_ OfdBuffer *output)'),
  customTagData: library.func('int ofd_custom_tag_data(OfdDocument handle, size_t index, _Out_ OfdBuffer *output)'),
  replaceCustomTagData: library.func('int ofd_replace_custom_tag_data(OfdDocument handle, size_t index, const uint8_t *data, size_t len, _Out_ OfdBuffer *output)'),
  addCustomTag: library.func('int ofd_add_custom_tag_json(OfdDocument handle, size_t document_index, str metadata, const uint8_t *data, size_t len, _Out_ OfdBuffer *output)'),
  updateCustomTag: library.func('int ofd_update_custom_tag_json(OfdDocument handle, size_t index, str metadata, _Out_ OfdBuffer *output)'),
  removeCustomTag: library.func('int ofd_remove_custom_tag(OfdDocument handle, size_t index)'),
  extensions: library.func('int ofd_extensions_json(OfdDocument handle, _Out_ OfdBuffer *output)'),
  addExtension: library.func('int ofd_add_extension_json(OfdDocument handle, str extension)'),
  updateExtension: library.func('int ofd_update_extension_json(OfdDocument handle, size_t index, str extension)'),
  removeExtension: library.func('int ofd_remove_extension(OfdDocument handle, size_t index)'),
  appendDocumentBody: library.func('int ofd_append_document_body(OfdDocument handle, _Out_ size_t *output)'),
  moveDocumentBody: library.func('int ofd_move_document_body(OfdDocument handle, size_t from, size_t to)'),
  removeDocumentBody: library.func('int ofd_remove_document_body(OfdDocument handle, size_t index)'),
  extractDocumentBody: library.func('int ofd_extract_document_body(OfdDocument handle, size_t index, _Out_ OfdDocument *output)'),
  appendDocumentBodiesFrom: library.func('int ofd_append_document_bodies_from_json(OfdDocument handle, OfdDocument source, _Out_ OfdBuffer *output)'),
  appendDocumentBodyFrom: library.func('int ofd_append_document_body_from(OfdDocument handle, OfdDocument source, size_t index, _Out_ size_t *output)'),
  pages: library.func('int ofd_pages_json(OfdDocument handle, _Out_ OfdBuffer *output)'),
  entries: library.func('int ofd_entries_json(OfdDocument handle, _Out_ OfdBuffer *output)'),
  getEntry: library.func('int ofd_get_entry(OfdDocument handle, str path, _Out_ OfdBuffer *output)'),
  setEntry: library.func('int ofd_set_entry(OfdDocument handle, str path, const uint8_t *data, size_t len)'),
  removeEntry: library.func('int ofd_remove_entry(OfdDocument handle, str path, _Out_ int *removed)'),
  pageText: library.func('int ofd_page_text_json(OfdDocument handle, size_t index, _Out_ OfdBuffer *output)'),
  allText: library.func('int ofd_all_text_json(OfdDocument handle, _Out_ OfdBuffer *output)'),
  pageTextInRegion: library.func('int ofd_page_text_in_region_json(OfdDocument handle, size_t index, double x, double y, double width, double height, _Out_ OfdBuffer *output)'),
  keywordPositions: library.func('int ofd_keyword_positions_json(OfdDocument handle, str query, _Out_ OfdBuffer *output)'),
  pageTextWithTemplates: library.func('int ofd_page_text_with_templates_json(OfdDocument handle, size_t index, _Out_ OfdBuffer *output)'),
  pageSources: library.func('int ofd_page_sources_json(OfdDocument handle, size_t index, _Out_ OfdBuffer *output)'),
  pageLayers: library.func('int ofd_page_layers_json(OfdDocument handle, size_t index, _Out_ OfdBuffer *output)'),
  addPageLayer: library.func('int ofd_add_page_layer(OfdDocument handle, size_t page, str layer_type, str draw_param, _Out_ size_t *output)'),
  removePageLayer: library.func('int ofd_remove_page_layer(OfdDocument handle, size_t page, str layer_id)'),
  setPageLayerType: library.func('int ofd_set_page_layer_type(OfdDocument handle, size_t page, str layer_id, str layer_type)'),
  setPageLayerName: library.func('int ofd_set_page_layer_name(OfdDocument handle, size_t page, str layer_id, str name)'),
  setPageLayerDrawParam: library.func('int ofd_set_page_layer_draw_param(OfdDocument handle, size_t page, str layer_id, str draw_param)'),
  movePageObjectToLayer: library.func('int ofd_move_page_object_to_layer(OfdDocument handle, size_t page, str object_id, str layer_id)'),
  movePageObject: library.func('int ofd_move_page_object(OfdDocument handle, size_t page, str object_id, str before_id)'),
  groupPageObjects: library.func('int ofd_group_page_objects(OfdDocument handle, size_t page, str first_id, str last_id)'),
  ungroupPageObjects: library.func('int ofd_ungroup_page_objects(OfdDocument handle, size_t page, str object_id)'),
  pageObjects: library.func('int ofd_page_objects_json(OfdDocument handle, size_t index, _Out_ OfdBuffer *output)'),
  pageObjectClips: library.func('int ofd_page_object_clips_json(OfdDocument handle, size_t index, _Out_ OfdBuffer *output)'),
  setPageObjectClipRects: library.func('int ofd_set_page_object_clip_rects_json(OfdDocument handle, size_t page, str object_id, str clips_json)'),
  setPageObjectPathClips: library.func('int ofd_set_page_object_path_clips_json(OfdDocument handle, size_t page, str object_id, str clips_json)'),
  setPageObjectClips: library.func('int ofd_set_page_object_clips_json(OfdDocument handle, size_t page, str object_id, str clips_json)'),
  pageObjectPaints: library.func('int ofd_page_object_paints_json(OfdDocument handle, size_t index, _Out_ OfdBuffer *output)'),
  pageObjectStrokeStyles: library.func('int ofd_page_object_stroke_styles_json(OfdDocument handle, size_t index, _Out_ OfdBuffer *output)'),
  pageObjectActions: library.func('int ofd_page_object_actions_json(OfdDocument handle, size_t page, str object_id, _Out_ OfdBuffer *output)'),
  setPageObjectBoundary: library.func('int ofd_set_page_object_boundary(OfdDocument handle, size_t page, str object_id, double x, double y, double width, double height)'),
  setPageObjectCtm: library.func('int ofd_set_page_object_ctm_json(OfdDocument handle, size_t page, str object_id, str ctm)'),
  setPageObjectVisible: library.func('int ofd_set_page_object_visible(OfdDocument handle, size_t page, str object_id, int visible)'),
  setPageObjectAlpha: library.func('int ofd_set_page_object_alpha(OfdDocument handle, size_t page, str object_id, int alpha)'),
  setPagePathData: library.func('int ofd_set_page_path_data(OfdDocument handle, size_t page, str object_id, str commands)'),
  setPagePathStrokeStyle: library.func('int ofd_set_page_path_stroke_style_json(OfdDocument handle, size_t page, str object_id, str style)'),
  setPagePathDashSequence: library.func('int ofd_set_page_path_dash_sequence_json(OfdDocument handle, size_t page, str object_id, str style)'),
  setPageTextTypography: library.func('int ofd_set_page_text_typography_json(OfdDocument handle, size_t page, str object_id, str style)'),
  editTextCode: library.func('int ofd_edit_text_code_json(OfdDocument handle, str request)'),
  setPageTextCodeLayout: library.func('int ofd_set_page_text_code_layout_json(OfdDocument handle, size_t page, str object_id, size_t code_index, str layout)'),
  removePageTextCode: library.func('int ofd_remove_page_text_code(OfdDocument handle, size_t page, str object_id, size_t code_index)'),
  setPageTextFont: library.func('int ofd_set_page_text_font(OfdDocument handle, size_t page, str object_id, str font_id)'),
  setPageTextPaintFlags: library.func('int ofd_set_page_text_paint_flags_json(OfdDocument handle, size_t page, str object_id, str flags)'),
  setPageObjectName: library.func('int ofd_set_page_object_name(OfdDocument handle, size_t page, str object_id, str name)'),
  setPageObjectBlendMode: library.func('int ofd_set_page_object_blend_mode(OfdDocument handle, size_t page, str object_id, str blend_mode)'),
  setPageObjectDrawParam: library.func('int ofd_set_page_object_draw_param(OfdDocument handle, size_t page, str object_id, str draw_param)'),
  setPageObjectSolidFill: library.func('int ofd_set_page_object_solid_fill_json(OfdDocument handle, size_t page, str object_id, str style)'),
  setPageObjectSolidStroke: library.func('int ofd_set_page_object_solid_stroke_json(OfdDocument handle, size_t page, str object_id, str style)'),
  setPageObjectGradientFill: library.func('int ofd_set_page_object_gradient_fill_json(OfdDocument handle, size_t page, str object_id, str style)'),
  setPageObjectGradientStroke: library.func('int ofd_set_page_object_gradient_stroke_json(OfdDocument handle, size_t page, str object_id, str style)'),
  setPageObjectGouraudFill: library.func('int ofd_set_page_object_gouraud_fill_json(OfdDocument handle, size_t page, str object_id, str style)'),
  setPageObjectGouraudStroke: library.func('int ofd_set_page_object_gouraud_stroke_json(OfdDocument handle, size_t page, str object_id, str style)'),
  setPageObjectPatternFill: library.func('int ofd_set_page_object_pattern_fill_json(OfdDocument handle, size_t page, str object_id, str style)'),
  setPageObjectPatternStroke: library.func('int ofd_set_page_object_pattern_stroke_json(OfdDocument handle, size_t page, str object_id, str style)'),
  setPageImageMask: library.func('int ofd_set_page_image_mask(OfdDocument handle, size_t page, str object_id, str mask_resource_id)'),
  setPageImageSubstitution: library.func('int ofd_set_page_image_substitution(OfdDocument handle, size_t page, str object_id, str substitution_resource_id)'),
  setPageImageBorder: library.func('int ofd_set_page_image_border_json(OfdDocument handle, size_t page, str object_id, str style)'),
  pageSourceObjectActions: library.func('int ofd_page_source_object_actions_json(OfdDocument handle, size_t page, str source_path, str object_id, _Out_ OfdBuffer *output)'),
  addPageObjectUriAction: library.func('int ofd_add_page_object_uri_action(OfdDocument handle, size_t page, str object_id, str uri)'),
  addPageObjectGotoAction: library.func('int ofd_add_page_object_goto_action_json(OfdDocument handle, size_t page, str object_id, str destination)'),
  addPageObjectBookmarkAction: library.func('int ofd_add_page_object_bookmark_action(OfdDocument handle, size_t page, str object_id, str bookmark_name)'),
  addPageObjectAttachmentAction: library.func('int ofd_add_page_object_attachment_action(OfdDocument handle, size_t page, str object_id, str attachment_id, int new_window)'),
  addPageObjectMediaAction: library.func('int ofd_add_page_object_media_action_json(OfdDocument handle, size_t page, str object_id, str action)'),
  removePageObjectAction: library.func('int ofd_remove_page_object_action(OfdDocument handle, size_t page, str object_id, size_t action_index)'),
  setPageObjectActionRegions: library.func('int ofd_set_page_object_action_regions_json(OfdDocument handle, size_t page, str object_id, size_t action_index, str rectangles)'),
  pageTextObjects: library.func('int ofd_page_text_objects_json(OfdDocument handle, size_t index, _Out_ OfdBuffer *output)'),
  pagePaths: library.func('int ofd_page_paths_json(OfdDocument handle, size_t index, _Out_ OfdBuffer *output)'),
  pageImages: library.func('int ofd_page_images_json(OfdDocument handle, size_t index, _Out_ OfdBuffer *output)'),
  fonts: library.func('int ofd_fonts_json(OfdDocument handle, _Out_ OfdBuffer *output)'),
  fontData: library.func('int ofd_font_data(OfdDocument handle, size_t index, _Out_ OfdBuffer *output)'),
  pageFonts: library.func('int ofd_page_fonts_json(OfdDocument handle, size_t page, _Out_ OfdBuffer *output)'),
  pageFontData: library.func('int ofd_page_font_data(OfdDocument handle, size_t page, size_t font_index, _Out_ OfdBuffer *output)'),
  documentColorSpaces: library.func('int ofd_document_color_spaces_json(OfdDocument handle, size_t document_index, _Out_ OfdBuffer *output)'),
  pageColorSpaces: library.func('int ofd_page_color_spaces_json(OfdDocument handle, size_t page, _Out_ OfdBuffer *output)'),
  documentDrawParams: library.func('int ofd_document_draw_params_json(OfdDocument handle, size_t document_index, _Out_ OfdBuffer *output)'),
  pageDrawParams: library.func('int ofd_page_draw_params_json(OfdDocument handle, size_t page, _Out_ OfdBuffer *output)'),
  documentDefaultColorSpace: library.func('int ofd_document_default_color_space_json(OfdDocument handle, size_t document_index, _Out_ OfdBuffer *output)'),
  setDocumentDefaultColorSpace: library.func('int ofd_set_document_default_color_space(OfdDocument handle, size_t document_index, str color_space_id)'),
  addDocumentColorSpace: library.func('int ofd_add_document_color_space_json(OfdDocument handle, size_t document_index, str spec, _Out_ size_t *output)'),
  addPageColorSpace: library.func('int ofd_add_page_color_space_json(OfdDocument handle, size_t page, str spec, _Out_ size_t *output)'),
  setDocumentColorSpace: library.func('int ofd_set_document_color_space_json(OfdDocument handle, size_t document_index, str id, str spec)'),
  setPageColorSpace: library.func('int ofd_set_page_color_space_json(OfdDocument handle, size_t page, str id, str spec)'),
  removeDocumentColorSpace: library.func('int ofd_remove_document_color_space(OfdDocument handle, size_t document_index, str id)'),
  removePageColorSpace: library.func('int ofd_remove_page_color_space(OfdDocument handle, size_t page, str id)'),
  addDocumentDrawParam: library.func('int ofd_add_document_draw_param_json(OfdDocument handle, size_t document_index, str style, _Out_ size_t *output)'),
  addPageDrawParam: library.func('int ofd_add_page_draw_param_json(OfdDocument handle, size_t page_index, str style, _Out_ size_t *output)'),
  setDocumentDrawParam: library.func('int ofd_set_document_draw_param_json(OfdDocument handle, size_t document_index, str id, str style)'),
  setPageDrawParam: library.func('int ofd_set_page_draw_param_json(OfdDocument handle, size_t page_index, str id, str style)'),
  removeDocumentDrawParam: library.func('int ofd_remove_document_draw_param(OfdDocument handle, size_t document_index, str id)'),
  removePageDrawParam: library.func('int ofd_remove_page_draw_param(OfdDocument handle, size_t page_index, str id)'),
  pageCompositeImages: library.func('int ofd_page_composite_images_json(OfdDocument handle, size_t index, _Out_ OfdBuffer *output)'),
  pageCompositeObjects: library.func('int ofd_page_composite_objects_json(OfdDocument handle, size_t index, _Out_ OfdBuffer *output)'),
  pageCompositeUnits: library.func('int ofd_page_composite_units_json(OfdDocument handle, size_t index, _Out_ OfdBuffer *output)'),
  setPageCompositeUnitImages: library.func('int ofd_set_page_composite_unit_images(OfdDocument handle, size_t page, str unit_id, str thumbnail_id, str substitution_id)'),
  pageCompositeImageData: library.func('int ofd_page_composite_image_data(OfdDocument handle, size_t page, size_t image, _Out_ OfdBuffer *output)'),
  pageImageData: library.func('int ofd_page_image_data(OfdDocument handle, size_t page, size_t image, _Out_ OfdBuffer *output)'),
  validateReferences: library.func('int ofd_validate_references_json(OfdDocument handle, _Out_ OfdBuffer *output)'),
  archiveViolations: library.func('int ofd_archive_violations_json(OfdDocument handle, _Out_ OfdBuffer *output)'),
  prepareArchive: library.func('int ofd_prepare_archive(OfdDocument handle, _Out_ OfdDocument *output)'),
  bookmarks: library.func('int ofd_bookmarks_json(OfdDocument handle, _Out_ OfdBuffer *output)'),
  addBookmark: library.func('int ofd_add_bookmark_json(OfdDocument handle, size_t document_index, str bookmark)'),
  removeBookmark: library.func('int ofd_remove_bookmark(OfdDocument handle, size_t document_index, str name)'),
  outlines: library.func('int ofd_outlines_json(OfdDocument handle, _Out_ OfdBuffer *output)'),
  addOutline: library.func('int ofd_add_outline_json(OfdDocument handle, size_t document_index, str outline, _Out_ size_t *output)'),
  removeOutline: library.func('int ofd_remove_outline(OfdDocument handle, size_t index)'),
  setOutlineTitle: library.func('int ofd_set_outline_title(OfdDocument handle, size_t index, str title)'),
  setOutlineExpanded: library.func('int ofd_set_outline_expanded(OfdDocument handle, size_t index, int expanded)'),
  setOutlineDestination: library.func('int ofd_set_outline_destination_json(OfdDocument handle, size_t index, str destination)'),
  setOutlineBookmark: library.func('int ofd_set_outline_bookmark(OfdDocument handle, size_t index, str name)'),
  versions: library.func('int ofd_versions_json(OfdDocument handle, _Out_ OfdBuffer *output)'),
  addVersionSnapshot: library.func('int ofd_add_version_snapshot_json(OfdDocument handle, size_t document_index, str snapshot, _Out_ OfdBuffer *output)'),
  extractVersionSnapshot: library.func('int ofd_extract_version_snapshot(OfdDocument handle, size_t index, _Out_ OfdDocument *output)'),
  annotations: library.func('int ofd_annotations_json(OfdDocument handle, _Out_ OfdBuffer *output)'),
  annotationObjects: library.func('int ofd_annotation_objects_json(OfdDocument handle, size_t page, str annotationId, _Out_ OfdBuffer *output)'),
  updateAnnotationObject: library.func('int ofd_update_annotation_object_json(OfdDocument handle, size_t page, str annotationId, str objectId, str update)'),
  setAnnotationObjectName: library.func('int ofd_set_annotation_object_name(OfdDocument handle, size_t page, str annotationId, str objectId, str name)'),
  setAnnotationObjectBlendMode: library.func('int ofd_set_annotation_object_blend_mode(OfdDocument handle, size_t page, str annotationId, str objectId, str blendMode)'),
  setAnnotationObjectDrawParam: library.func('int ofd_set_annotation_object_draw_param(OfdDocument handle, size_t page, str annotationId, str objectId, str drawParam)'),
  removeAnnotationObject: library.func('int ofd_remove_annotation_object(OfdDocument handle, size_t page, str annotationId, str objectId)'),
  setAnnotationPathData: library.func('int ofd_set_annotation_path_data(OfdDocument handle, size_t page, str annotationId, str objectId, str commands)'),
  setAnnotationPathStrokeStyle: library.func('int ofd_set_annotation_path_stroke_style_json(OfdDocument handle, size_t page, str annotationId, str objectId, str style)'),
  setAnnotationTextCode: library.func('int ofd_set_annotation_text_code(OfdDocument handle, size_t page, str annotationId, str objectId, size_t codeIndex, str text)'),
  editAnnotationTextCode: library.func('int ofd_edit_annotation_text_code_json(OfdDocument handle, str request)'),
  setAnnotationTextCodeLayout: library.func('int ofd_set_annotation_text_code_layout_json(OfdDocument handle, size_t page, str annotationId, str objectId, size_t codeIndex, str layout)'),
  setAnnotationTextTypography: library.func('int ofd_set_annotation_text_typography_json(OfdDocument handle, size_t page, str annotationId, str objectId, str style)'),
  setAnnotationTextPaintFlags: library.func('int ofd_set_annotation_text_paint_flags_json(OfdDocument handle, size_t page, str annotationId, str objectId, str flags)'),
  appendAnnotationTextCode: library.func('int ofd_append_annotation_text_code_json(OfdDocument handle, size_t page, str annotationId, str objectId, str text, str layout)'),
  removeAnnotationTextCode: library.func('int ofd_remove_annotation_text_code(OfdDocument handle, size_t page, str annotationId, str objectId, size_t codeIndex)'),
  setAnnotationTextFont: library.func('int ofd_set_annotation_text_font(OfdDocument handle, size_t page, str annotationId, str objectId, str fontId)'),
  setAnnotationGraphicResource: library.func('int ofd_set_annotation_graphic_resource(OfdDocument handle, size_t page, str annotationId, str objectId, str resourceId)'),
  addPathAnnotation: library.func('int ofd_add_path_annotation_json(OfdDocument handle, size_t page, str object, _Out_ size_t *output)'),
  addMultiPathAnnotation: library.func('int ofd_add_multi_path_annotation_json(OfdDocument handle, size_t page, str object, _Out_ size_t *output)'),
  addMixedAnnotation: library.func('int ofd_add_mixed_annotation_json(OfdDocument handle, size_t page, str object, _Out_ size_t *output)'),
  appendAnnotationObjects: library.func('int ofd_append_annotation_objects_json(OfdDocument handle, size_t page, str annotationId, str object, _Out_ OfdBuffer *output)'),
  addHighlightAnnotation: library.func('int ofd_add_highlight_annotation_json(OfdDocument handle, size_t page, str object, _Out_ size_t *output)'),
  addTextNoteAnnotation: library.func('int ofd_add_text_note_annotation_json(OfdDocument handle, size_t page, str object, _Out_ size_t *output)'),
  addShapeAnnotation: library.func('int ofd_add_shape_annotation_json(OfdDocument handle, size_t page, str object, _Out_ size_t *output)'),
  addLineAnnotation: library.func('int ofd_add_line_annotation_json(OfdDocument handle, size_t page, str object, _Out_ size_t *output)'),
  addUriLinkAnnotation: library.func('int ofd_add_uri_link_annotation_json(OfdDocument handle, size_t page, str object, _Out_ size_t *output)'),
  addGotoLinkAnnotation: library.func('int ofd_add_goto_link_annotation_json(OfdDocument handle, size_t page, str object, _Out_ size_t *output)'),
  addBookmarkLinkAnnotation: library.func('int ofd_add_bookmark_link_annotation_json(OfdDocument handle, size_t page, str object, _Out_ size_t *output)'),
  addAttachmentLinkAnnotation: library.func('int ofd_add_attachment_link_annotation_json(OfdDocument handle, size_t page, str object, _Out_ size_t *output)'),
  addMediaLinkAnnotation: library.func('int ofd_add_media_link_annotation_json(OfdDocument handle, size_t page, str object, _Out_ size_t *output)'),
  addTextWatermark: library.func('int ofd_add_text_watermark_json(OfdDocument handle, size_t page, str object, _Out_ size_t *output)'),
  addTextStampAnnotation: library.func('int ofd_add_text_stamp_annotation_json(OfdDocument handle, size_t page, str object, _Out_ size_t *output)'),
  addTextWatermarkAllPages: library.func('int ofd_add_text_watermark_all_pages_json(OfdDocument handle, str object, _Out_ OfdBuffer *output)'),
  addImageWatermark: library.func('int ofd_add_image_watermark_json(OfdDocument handle, size_t page, str object, _Out_ size_t *output)'),
  addImageStampAnnotation: library.func('int ofd_add_image_stamp_annotation_json(OfdDocument handle, size_t page, str object, _Out_ size_t *output)'),
  addImageWatermarkAllPages: library.func('int ofd_add_image_watermark_all_pages_json(OfdDocument handle, str object, _Out_ OfdBuffer *output)'),
  removeAnnotation: library.func('int ofd_remove_annotation(OfdDocument handle, size_t page, str id)'),
  setAnnotationCreator: library.func('int ofd_set_annotation_creator(OfdDocument handle, size_t page, str id, str creator)'),
  setAnnotationSubtype: library.func('int ofd_set_annotation_subtype(OfdDocument handle, size_t page, str id, str subtype)'),
  setAnnotationLastModDate: library.func('int ofd_set_annotation_last_mod_date(OfdDocument handle, size_t page, str id, str date)'),
  setAnnotationAppearanceBoundary: library.func('int ofd_set_annotation_appearance_boundary(OfdDocument handle, size_t page, str id, double x, double y, double width, double height)'),
  setAnnotationRemark: library.func('int ofd_set_annotation_remark(OfdDocument handle, size_t page, str id, str remark)'),
  setAnnotationParameter: library.func('int ofd_set_annotation_parameter(OfdDocument handle, size_t page, str id, str name, str value)'),
  removeAnnotationParameter: library.func('int ofd_remove_annotation_parameter(OfdDocument handle, size_t page, str id, str name)'),
  setAnnotationVisible: library.func('int ofd_set_annotation_visible(OfdDocument handle, size_t page, str id, int visible)'),
  setAnnotationPrint: library.func('int ofd_set_annotation_print(OfdDocument handle, size_t page, str id, int print)'),
  setAnnotationFlag: library.func('int ofd_set_annotation_flag(OfdDocument handle, size_t page, str id, str flag, int value)'),
  integrityManifest: library.func('int ofd_integrity_manifest_json(OfdDocument handle, _Out_ OfdBuffer *output)'),
  checkIntegrityManifest: library.func('int ofd_check_integrity_manifest_json(OfdDocument handle, _Out_ OfdBuffer *output)'),
  verifyIntegrityManifestSm2: library.func('int ofd_verify_integrity_manifest_sm2_json(OfdDocument handle, str signature_distid, _Out_ OfdBuffer *output)'),
  verifyIntegrityManifestSm2WithOrderedChain: library.func('int ofd_verify_integrity_manifest_sm2_with_ordered_chain_json(OfdDocument handle, str options, _Out_ OfdBuffer *output)'),
  verifyIntegrityManifestSm2WithCertificatePool: library.func('int ofd_verify_integrity_manifest_sm2_with_certificate_pool_json(OfdDocument handle, str options, _Out_ OfdBuffer *output)'),
  attachments: library.func('int ofd_attachments_json(OfdDocument handle, _Out_ OfdBuffer *output)'),
  attachmentData: library.func('int ofd_attachment_data(OfdDocument handle, size_t index, _Out_ OfdBuffer *output)'),
  addAttachmentToDocument: library.func('int ofd_add_attachment_to_document(OfdDocument handle, size_t document_index, str name, const uint8_t *data, size_t len)'),
  removeAttachment: library.func('int ofd_remove_attachment(OfdDocument handle, size_t index)'),
  renameAttachment: library.func('int ofd_rename_attachment(OfdDocument handle, size_t index, str name)'),
  replaceAttachmentData: library.func('int ofd_replace_attachment_data(OfdDocument handle, size_t index, const uint8_t *data, size_t len)'),
  setAttachmentMetadata: library.func('int ofd_set_attachment_metadata_json(OfdDocument handle, size_t index, str metadata)'),
  signatures: library.func('int ofd_signatures_json(OfdDocument handle, _Out_ OfdBuffer *output)'),
  signedValueData: library.func('int ofd_signed_value_data(OfdDocument handle, size_t index, _Out_ OfdBuffer *output)'),
  sealData: library.func('int ofd_seal_data(OfdDocument handle, size_t index, _Out_ OfdBuffer *output)'),
  checkSignatureReferences: library.func('int ofd_check_signature_references_json(OfdDocument handle, size_t index, _Out_ OfdBuffer *output)'),
  checkSignatureDescriptionHash: library.func('int ofd_check_signature_description_hash_json(OfdDocument handle, size_t index, _Out_ OfdBuffer *output)'),
  checkSignatureSealMatch: library.func('int ofd_check_signature_seal_match(OfdDocument handle, size_t index, _Out_ int *output)'),
  checkDigitalSignatureWithCertificate: library.func('int ofd_check_digital_signature_with_certificate(OfdDocument handle, size_t index, const uint8_t *certificate, size_t len, _Out_ int *output)'),
  checkDigitalSignatureWithCertificateAndDistid: library.func('int ofd_check_digital_signature_with_certificate_and_distid(OfdDocument handle, size_t index, const uint8_t *certificate, size_t len, str signature_distid, _Out_ int *output)'),
  verifySignatureWithDirectTrustAnchor: library.func('int ofd_verify_signature_with_direct_trust_anchor_and_distids_json(OfdDocument handle, size_t index, const uint8_t *signer, size_t signer_len, const uint8_t *anchor, size_t anchor_len, uint64_t at_unix_seconds, str certificate_distid, str signature_distid, _Out_ OfdBuffer *output)'),
  verifySignatureWithOrderedSm2Chain: library.func('int ofd_verify_signature_with_ordered_sm2_chain_and_distids_json(OfdDocument handle, size_t index, const uint8_t *signer, size_t signer_len, const uint8_t *intermediates, size_t intermediates_len, const uint8_t *anchor, size_t anchor_len, uint64_t at_unix_seconds, str certificate_distid, str signature_distid, _Out_ OfdBuffer *output)'),
  verifySignatureWithCertificatePool: library.func('int ofd_verify_signature_with_certificate_pool_json(OfdDocument handle, size_t index, const uint8_t *signer, size_t signer_len, const uint8_t *candidates, size_t candidates_len, const uint8_t *anchor, size_t anchor_len, uint64_t at_unix_seconds, str certificate_distid, str signature_distid, _Out_ OfdBuffer *output)'),
  verifySignatureWithOrderedSm2Chains: library.func('int ofd_verify_signature_with_ordered_sm2_chains_json(OfdDocument handle, size_t index, const uint8_t *options, size_t options_len, _Out_ OfdBuffer *output)'),
  verifySignatureWithCertificatePools: library.func('int ofd_verify_signature_with_certificate_pools_json(OfdDocument handle, size_t index, const uint8_t *options, size_t options_len, _Out_ OfdBuffer *output)'),
  checkSesSignatureValue: library.func('int ofd_check_ses_signature_value(OfdDocument handle, size_t index, _Out_ int *output)'),
  checkSesSignatureValueWithDistid: library.func('int ofd_check_ses_signature_value_with_distid(OfdDocument handle, size_t index, str signature_distid, _Out_ int *output)'),
  checkSesSealSignature: library.func('int ofd_check_ses_seal_signature(OfdDocument handle, size_t index, _Out_ int *output)'),
  checkSesSealSignatureWithDistid: library.func('int ofd_check_ses_seal_signature_with_distid(OfdDocument handle, size_t index, str seal_distid, _Out_ int *output)'),
  checkSesSealCertificateValidity: library.func('int ofd_check_ses_seal_certificate_validity(OfdDocument handle, size_t index, uint64_t at_unix_seconds, _Out_ int *output)'),
  checkSignatureCertificateValidity: library.func('int ofd_check_signature_certificate_validity(OfdDocument handle, size_t index, const uint8_t *certificate, size_t len, uint64_t at_unix_seconds, _Out_ int *output)'),
  checkSignatureDirectTrustAnchor: library.func('int ofd_check_signature_direct_trust_anchor(OfdDocument handle, size_t index, const uint8_t *signer, size_t signer_len, const uint8_t *anchor, size_t anchor_len, uint64_t at_unix_seconds, str certificate_distid, _Out_ int *output)'),
  checkSesSealIssuerOrderedSm2Chain: library.func('int ofd_check_ses_seal_issuer_ordered_sm2_chain(OfdDocument handle, size_t index, const uint8_t *intermediates, size_t intermediates_len, const uint8_t *anchor, size_t anchor_len, uint64_t at_unix_seconds, str certificate_distid, _Out_ int *output)'),
  checkSesSealSignerAuthorization: library.func('int ofd_check_ses_seal_signer_authorization(OfdDocument handle, size_t index, _Out_ int *output)'),
  checkSesSealValidity: library.func('int ofd_check_ses_seal_validity(OfdDocument handle, size_t index, uint64_t at_unix_seconds, _Out_ int *output)'),
  pageSvg: library.func('int ofd_page_svg(OfdDocument handle, size_t index, _Out_ OfdBuffer *output)'),
  pageSvgWithOptions: library.func('int ofd_page_svg_with_options(OfdDocument handle, size_t index, int prefer_image_substitution, _Out_ OfdBuffer *output)'),
  pagePng: library.func('int ofd_page_png(OfdDocument handle, size_t index, double dpi, _Out_ OfdBuffer *output)'),
  pagePngWithOptions: library.func('int ofd_page_png_with_options(OfdDocument handle, size_t index, double dpi, int prefer_image_substitution, _Out_ OfdBuffer *output)'),
  toPdf: library.func('int ofd_to_pdf(OfdDocument handle, _Out_ OfdBuffer *output)'),
  toPdfWithOptions: library.func('int ofd_to_pdf_with_options(OfdDocument handle, int prefer_image_substitution, _Out_ OfdBuffer *output)'),
  toBytes: library.func('int ofd_to_bytes(OfdDocument handle, _Out_ OfdBuffer *output)'),
  writeTextPage: library.func('int ofd_write_text_page_json(OfdDocument handle, str request, _Out_ size_t *output)'),
  addBlankPage: library.func('int ofd_add_blank_page(OfdDocument handle, double width, double height)'),
  addImagePage: library.func('int ofd_add_image_page(OfdDocument handle, const uint8_t *data, size_t len, str format, double width, double height)'),
  addPathPage: library.func('int ofd_add_path_page(OfdDocument handle, str commands, double x, double y, double width, double height, double page_width, double page_height, int fill, int stroke)'),
  addFlowedText: library.func('int ofd_add_flowed_text_json(OfdDocument handle, size_t document_index, str flow, _Out_ OfdBuffer *output)'),
  addFlowedSpans: library.func('int ofd_add_flowed_spans_json(OfdDocument handle, size_t document_index, str flow, _Out_ OfdBuffer *output)'),
  addTable: library.func('int ofd_add_table_json(OfdDocument handle, size_t document_index, str table, _Out_ OfdBuffer *output)'),
  addEmbeddedFont: library.func('int ofd_add_embedded_font(OfdDocument handle, str name, const uint8_t *data, size_t len, _Out_ size_t *output)'),
  addEmbeddedFontToDocument: library.func('int ofd_add_embedded_font_to_document(OfdDocument handle, size_t document_index, str name, const uint8_t *data, size_t len, _Out_ size_t *output)'),
  addEmbeddedFontToPage: library.func('int ofd_add_embedded_font_to_page(OfdDocument handle, size_t page_index, str name, const uint8_t *data, size_t len, _Out_ size_t *output)'),
  setFontAttribute: library.func('int ofd_set_font_attribute(OfdDocument handle, size_t font_index, str name, str value)'),
  setPageFontAttribute: library.func('int ofd_set_page_font_attribute(OfdDocument handle, size_t page_index, size_t font_index, str name, str value)'),
  replaceFontData: library.func('int ofd_replace_font_data(OfdDocument handle, size_t font_index, const uint8_t *data, size_t len)'),
  replacePageFontData: library.func('int ofd_replace_page_font_data(OfdDocument handle, size_t page_index, size_t font_index, const uint8_t *data, size_t len)'),
  appendBlankPage: library.func('int ofd_append_blank_page(OfdDocument handle, size_t document_index, double width, double height)'),
  appendImagePage: library.func('int ofd_append_image_page(OfdDocument handle, size_t document_index, const uint8_t *data, size_t len, str format, double width, double height)'),
  appendPathPage: library.func('int ofd_append_path_page(OfdDocument handle, size_t document_index, str commands, double x, double y, double width, double height, double page_width, double page_height, int fill, int stroke)'),
  insertBlankPage: library.func('int ofd_insert_blank_page(OfdDocument handle, size_t document_index, size_t position, double width, double height, _Out_ size_t *output)'),
  addTextObject: library.func('int ofd_add_text_object_json(OfdDocument handle, size_t page, str object, _Out_ size_t *output)'),
  addPathObject: library.func('int ofd_add_path_object_json(OfdDocument handle, size_t page, str object, _Out_ size_t *output)'),
  addImageObject: library.func('int ofd_add_image_object(OfdDocument handle, size_t page, const uint8_t *data, size_t len, str format, double x, double y, double width, double height, _Out_ size_t *output)'),
  addMultimediaResource: library.func('int ofd_add_multimedia_resource(OfdDocument handle, size_t documentIndex, str mediaType, str format, const uint8_t *data, size_t len, _Out_ size_t *output)'),
  pageMultimediaResources: library.func('int ofd_page_multimedia_resources_json(OfdDocument handle, size_t page, _Out_ OfdBuffer *output)'),
  pageMultimediaData: library.func('int ofd_page_multimedia_data(OfdDocument handle, size_t page, str resourceId, _Out_ OfdBuffer *output)'),
  documentMultimediaResources: library.func('int ofd_document_multimedia_resources_json(OfdDocument handle, size_t documentIndex, _Out_ OfdBuffer *output)'),
  documentMultimediaData: library.func('int ofd_document_multimedia_data(OfdDocument handle, size_t documentIndex, str resourceId, _Out_ OfdBuffer *output)'),
  addPageMultimediaResource: library.func('int ofd_add_page_multimedia_resource(OfdDocument handle, size_t page, str mediaType, str format, const uint8_t *data, size_t len, _Out_ size_t *output)'),
  replaceDocumentMultimediaData: library.func('int ofd_replace_document_multimedia_data(OfdDocument handle, size_t documentIndex, str resourceId, const uint8_t *data, size_t len)'),
  replacePageMultimediaData: library.func('int ofd_replace_page_multimedia_data(OfdDocument handle, size_t page, str resourceId, const uint8_t *data, size_t len)'),
  removeDocumentMultimediaResource: library.func('int ofd_remove_document_multimedia_resource(OfdDocument handle, size_t documentIndex, str resourceId)'),
  removePageMultimediaResource: library.func('int ofd_remove_page_multimedia_resource(OfdDocument handle, size_t page, str resourceId)'),
  addCompositePathObject: library.func('int ofd_add_composite_path_object_json(OfdDocument handle, size_t page, str object, _Out_ size_t *output)'),
  addCompositeTextObject: library.func('int ofd_add_composite_text_object_json(OfdDocument handle, size_t page, str object, _Out_ size_t *output)'),
  addCompositeObject: library.func('int ofd_add_composite_object_json(OfdDocument handle, size_t page, str object, _Out_ size_t *output)'),
  addCompositeImageObject: library.func('int ofd_add_composite_image_object(OfdDocument handle, size_t page, const uint8_t *data, size_t len, str format, double x, double y, double width, double height, _Out_ size_t *output)'),
  addAttachment: library.func('int ofd_add_attachment(OfdDocument handle, str name, const uint8_t *data, size_t len)'),
  addDigitalSignature: library.func('int ofd_add_digital_signature(OfdDocument handle, const uint8_t *key, size_t len, str datetime, _Out_ size_t *output)'),
  addDigitalSignatureWithDistid: library.func('int ofd_add_digital_signature_with_distid(OfdDocument handle, const uint8_t *key, size_t len, str datetime, str signature_distid, _Out_ size_t *output)'),
  createSesSeal: library.func('int ofd_create_ses_seal_json(const uint8_t *key, size_t key_len, const uint8_t *certificate, size_t certificate_len, const uint8_t *picture, size_t picture_len, const uint8_t *options, size_t options_len, str issuer_distid, _Out_ OfdBuffer *output)'),
  addSesSignature: library.func('int ofd_add_ses_signature_with_distids(OfdDocument handle, const uint8_t *key, size_t key_len, const uint8_t *certificate, size_t certificate_len, const uint8_t *seal, size_t seal_len, str datetime, str signature_distid, str seal_distid, _Out_ size_t *output)'),
  addSesSignatureAtPage: library.func('int ofd_add_ses_signature_with_stamp_and_distids(OfdDocument handle, const uint8_t *key, size_t key_len, const uint8_t *certificate, size_t certificate_len, const uint8_t *seal, size_t seal_len, str datetime, size_t page, double x, double y, double width, double height, str signature_distid, str seal_distid, _Out_ size_t *output)'),
  addSesSignatureAtPageJson: library.func('int ofd_add_ses_signature_with_stamp_json(OfdDocument handle, const uint8_t *key, size_t key_len, const uint8_t *certificate, size_t certificate_len, const uint8_t *seal, size_t seal_len, str options, _Out_ size_t *output)'),
  protectIntegritySm2: library.func('int ofd_protect_integrity_sm2(OfdDocument handle, const uint8_t *key, size_t key_len, const uint8_t *certificate, size_t certificate_len, str signature_distid)'),
  duplicatePage: library.func('int ofd_duplicate_page(OfdDocument handle, size_t index, _Out_ size_t *output)'),
  duplicatePageAt: library.func('int ofd_duplicate_page_at(OfdDocument handle, size_t index, size_t position, _Out_ size_t *output)'),
  movePage: library.func('int ofd_move_page(OfdDocument handle, size_t from, size_t to)'),
  reorderDocumentPages: library.func('int ofd_reorder_document_pages_json(OfdDocument handle, size_t document_index, str order_json)'),
  removePage: library.func('int ofd_remove_page(OfdDocument handle, size_t index)'),
  promotePageToTemplate: library.func('int ofd_promote_page_to_template(OfdDocument handle, size_t source, str additional_pages, str z_order, _Out_ size_t *output)'),
  applyPageTemplate: library.func('int ofd_apply_page_template(OfdDocument handle, size_t page, str template_id, str z_order)'),
  removePageTemplate: library.func('int ofd_remove_page_template(OfdDocument handle, size_t page, str template_id)'),
  removeDocumentTemplate: library.func('int ofd_remove_document_template(OfdDocument handle, size_t document_index, str template_id)'),
  duplicatePageObject: library.func('int ofd_duplicate_page_object(OfdDocument handle, size_t page, str id, _Out_ size_t *output)'),
  removePageObject: library.func('int ofd_remove_page_object(OfdDocument handle, size_t page, str id)'),
  freeBuffer: library.func('void ofd_buffer_free(OfdBuffer buffer)'),
  freeBufferParts: library.func('void ofd_buffer_free_parts(uint8_t *data, size_t len)'),
};

if (api.abiVersion() !== 1) throw new OfdError('incompatible OFD C ABI version');

function check(status) {
  if (status !== 0) throw new OfdError(api.lastError());
}

function ownedBuffer(output) {
  try {
    return output.len ? Buffer.from(new Uint8Array(koffi.view(output.data, Number(output.len)))) : Buffer.alloc(0);
  } finally {
    api.freeBuffer(output);
  }
}

function utf8(value, name) {
  if (typeof value !== 'string' || value.includes('\0')) {
    throw new TypeError(`${name} must be a string without NUL`);
  }
  return value;
}

function textValue(value) {
  if (typeof value !== 'string') throw new TypeError('text must be a string');
  return value;
}

function bytes(value) {
  if (!Buffer.isBuffer(value) && !(value instanceof Uint8Array)) {
    throw new TypeError('data must be a Buffer or Uint8Array');
  }
  return Buffer.from(value.buffer, value.byteOffset, value.byteLength);
}

function index(value, name = 'index') {
  if (!Number.isSafeInteger(value) || value < 0) {
    throw new RangeError(`${name} must be a nonnegative safe integer`);
  }
  return value;
}

function fontIndex(value) {
  if (typeof value === 'string') {
    if (!/^[0-9]+$/.test(value)) throw new TypeError('fontId must be a numeric string or number');
    return index(Number(value), 'fontId');
  }
  if (typeof value !== 'number') throw new TypeError('fontId must be a numeric string or number');
  return index(value, 'fontId');
}

function size(value, name) {
  if (typeof value !== 'number' || !Number.isFinite(value) || value <= 0) {
    throw new RangeError(`${name} must be a positive finite number`);
  }
  return value;
}

function boundary(value) {
  if (!Array.isArray(value) || value.length !== 4 ||
      value.some((part) => typeof part !== 'number' || !Number.isFinite(part)) ||
      value[2] <= 0 || value[3] <= 0) {
    throw new RangeError('boundary must contain finite x, y, positive width, and positive height');
  }
  return value;
}

function pageArea(value) {
  const area = optionsObject(value);
  const result = { physical_box: boundary(area.physical_box) };
  for (const name of ['application_box', 'content_box', 'bleed_box']) {
    if (area[name] !== undefined && area[name] !== null) result[name] = boundary(area[name]);
    else if (area[name] === null) result[name] = null;
  }
  return result;
}

function actionRequest(value) {
  const action = optionsObject(value);
  const event = utf8(action.event, 'event');
  switch (utf8(action.kind, 'kind')) {
    case 'uri': return { kind: 'uri', event, uri: utf8(action.uri, 'uri') };
    case 'page': return { kind: 'page', event, destination: optionsObject(action.destination) };
    case 'bookmark': return { kind: 'bookmark', event, name: utf8(action.name, 'name') };
    case 'attachment': return {
      kind: 'attachment', event, id: utf8(action.id, 'id'),
      new_window: action.new_window ?? null,
    };
    default: throw new TypeError('unsupported action kind');
  }
}

function optionsObject(value) {
  if (value === null || typeof value !== 'object' || Array.isArray(value)) {
    throw new TypeError('options must be an object');
  }
  return value;
}

function extensionRequest(extension) {
  const value = optionsObject(extension);
  return {
    document_index: index(value.document_index, 'document_index'),
    app_name: utf8(value.app_name, 'app_name'),
    company: value.company ?? null,
    app_version: value.app_version ?? null,
    date: value.date ?? null,
    ref_id: value.ref_id ?? null,
    properties: value.properties ?? [],
    extend_data_paths: value.extend_data_paths ?? [],
    data_xml: value.data_xml ?? [],
  };
}

function customTagMetadata(typeId, options) {
  const { namespace = null, schemaPath = null } = optionsObject(options);
  return {
    type_id: utf8(typeId, 'typeId'),
    namespace: namespace === null ? null : utf8(namespace, 'namespace'),
    schema_path: schemaPath === null ? null : utf8(schemaPath, 'schemaPath'),
  };
}

function textWatermarkRequest(text, fontSize, style) {
  const value = optionsObject(style);
  return {
    text: utf8(text, 'text'), size: size(fontSize, 'fontSize'),
    font_id: value.fontId == null ? null : utf8(String(value.fontId), 'fontId'),
    color: value.color, opacity: value.opacity, angle_degrees: value.angleDegrees,
    tile_spacing: value.tileSpacing, bold: value.bold, italic: value.italic,
  };
}

function imageAnnotationRequest(box, source, opacity) {
  const value = optionsObject(source);
  if (!Number.isInteger(opacity) || opacity < 0 || opacity > 255) {
    throw new RangeError('opacity must be an integer from 0 to 255');
  }
  if ((value.resourceId == null) === (value.data == null)) {
    throw new TypeError('supply exactly one of resourceId or data');
  }
  const result = {boundary: boundary(box), opacity};
  if (value.resourceId != null) result.resource_id = utf8(String(value.resourceId), 'resourceId');
  else {
    result.data_base64 = bytes(value.data).toString('base64');
    result.format = utf8(value.format, 'format');
  }
  return result;
}

function recipientsArray(value, kind) {
  if (!Array.isArray(value)) throw new TypeError('recipients must be an array');
  return value.map((recipient) => {
    if (recipient === null || typeof recipient !== 'object' || Array.isArray(recipient)) {
      throw new TypeError('each recipient must be an object');
    }
    const normalized = { username: utf8(recipient.username, 'username') };
    if (recipient.userType !== undefined && recipient.userType !== null) {
      normalized.user_type = utf8(recipient.userType, 'userType');
    }
    const recipientKind = kind === 'mixed' ? recipient.type : kind;
    if (recipientKind !== 'password' && recipientKind !== 'certificate') {
      throw new TypeError('recipient type must be password or certificate');
    }
    if (kind === 'mixed') normalized.type = recipientKind;
    if (recipientKind === 'password') normalized.password = utf8(recipient.password, 'password');
    else normalized.certificate_der_base64 = bytes(recipient.certificate).toString('base64');
    return normalized;
  });
}

function renderOptions(value) {
  const options = optionsObject(value);
  const prefer = options.preferImageSubstitution ?? false;
  if (typeof prefer !== 'boolean') throw new TypeError('preferImageSubstitution must be a boolean');
  return prefer;
}

class Document {
  constructor(handle) {
    this._handle = handle;
  }

  static open(file) {
    const output = [null];
    check(api.open(utf8(file, 'path'), output));
    return new Document(output[0]);
  }

  static fromBytes(data) {
    const input = bytes(data);
    const output = [null];
    check(api.fromBytes(input, input.length, output));
    return new Document(output[0]);
  }

  static fromPdfBytes(data) {
    const input = bytes(data);
    const output = [null];
    check(api.fromPdfBytes(input, input.length, output));
    return new Document(output[0]);
  }

  static create() {
    const output = [null];
    check(api.create(output));
    return new Document(output[0]);
  }

  _live() {
    if (!this._handle) throw new OfdError('document is closed');
    return this._handle;
  }

  _buffer(fn, ...args) {
    const output = {};
    check(fn(this._live(), ...args, output));
    return ownedBuffer(output);
  }

  _json(fn, ...args) {
    return JSON.parse(this._buffer(fn, ...args).toString('utf8'));
  }

  close() {
    if (this._handle) {
      api.close(this._handle);
      this._handle = null;
    }
  }

  get pageCount() {
    const output = [0];
    check(api.pageCount(this._live(), output));
    return Number(output[0]);
  }

  get isEncrypted() {
    const output = [0];
    check(api.isEncrypted(this._live(), output));
    return output[0] === 1;
  }

  encryptionInfo() { return this._json(api.encryptionInfo); }

  decryptWithPassword(password, username = null) {
    const output = [null];
    check(api.decryptWithPassword(this._live(), utf8(password, 'password'),
      username === null ? null : utf8(username, 'username'), output));
    return new Document(output[0]);
  }

  decryptWithSm2PrivateKey(privateKey, username = null) {
    const input = bytes(privateKey);
    const output = [null];
    check(api.decryptWithSm2PrivateKey(this._live(), input, input.length,
      username === null ? null : utf8(username, 'username'), output));
    return new Document(output[0]);
  }

  encryptWithPasswords(recipients) {
    const output = [null];
    check(api.encryptWithPasswords(this._live(), JSON.stringify(recipientsArray(recipients, 'password')), output));
    return new Document(output[0]);
  }

  encryptWithCertificates(recipients) {
    const output = [null];
    check(api.encryptWithCertificates(this._live(), JSON.stringify(recipientsArray(recipients, 'certificate')), output));
    return new Document(output[0]);
  }

  encryptWithRecipients(recipients) {
    const output = [null];
    check(api.encryptWithRecipients(this._live(), JSON.stringify(recipientsArray(recipients, 'mixed')), output));
    return new Document(output[0]);
  }

  pages() { return this._json(api.pages); }
  documents() { return this._json(api.documents); }
  packageInfo() { return this._json(api.packageInfo); }
  setPackageHeader(version, docType) {
    check(api.setPackageHeader(this._live(), utf8(version, 'version'), utf8(docType, 'docType')));
  }
  setDocumentInfoField(documentIndex, field, value) {
    check(api.setDocumentInfoField(this._live(), index(documentIndex, 'documentIndex'),
      utf8(field, 'field'), utf8(value, 'value')));
  }
  removeDocumentInfoField(documentIndex, field) {
    check(api.removeDocumentInfoField(this._live(), index(documentIndex, 'documentIndex'), utf8(field, 'field')));
  }
  addDocumentKeyword(documentIndex, keyword) {
    check(api.addDocumentKeyword(this._live(), index(documentIndex, 'documentIndex'), utf8(keyword, 'keyword')));
  }
  removeDocumentKeyword(documentIndex, keywordIndex) {
    check(api.removeDocumentKeyword(this._live(), index(documentIndex, 'documentIndex'), index(keywordIndex, 'keywordIndex')));
  }
  setDocumentCustomData(documentIndex, name, value) {
    check(api.setDocumentCustomData(this._live(), index(documentIndex, 'documentIndex'),
      utf8(name, 'name'), utf8(value, 'value')));
  }
  removeDocumentCustomData(documentIndex, name) {
    check(api.removeDocumentCustomData(this._live(), index(documentIndex, 'documentIndex'), utf8(name, 'name')));
  }
  documentPermissions(documentIndex) {
    return this._json(api.documentPermissions, index(documentIndex, 'documentIndex'));
  }
  setDocumentPermissions(documentIndex, permissions) {
    check(api.setDocumentPermissions(this._live(), index(documentIndex, 'documentIndex'),
      JSON.stringify(optionsObject(permissions))));
  }
  viewPreferences(documentIndex) {
    return this._json(api.viewPreferences, index(documentIndex, 'documentIndex'));
  }
  setViewPreferences(documentIndex, preferences) {
    check(api.setViewPreferences(this._live(), index(documentIndex, 'documentIndex'),
      JSON.stringify(optionsObject(preferences))));
  }
  documentPageArea(documentIndex) {
    return this._json(api.documentPageArea, index(documentIndex, 'documentIndex'));
  }
  setDocumentPageArea(documentIndex, area) {
    check(api.setDocumentPageArea(this._live(), index(documentIndex, 'documentIndex'), JSON.stringify(pageArea(area))));
  }
  setPageArea(page, area) {
    check(api.setPageArea(this._live(), index(page, 'page'), JSON.stringify(pageArea(area))));
  }
  documentActions(documentIndex) {
    return this._json(api.documentActions, index(documentIndex, 'documentIndex'));
  }
  addDocumentAction(documentIndex, action) {
    check(api.addDocumentAction(this._live(), index(documentIndex, 'documentIndex'), JSON.stringify(actionRequest(action))));
  }
  removeDocumentAction(documentIndex, actionIndex) {
    check(api.removeDocumentAction(this._live(), index(documentIndex, 'documentIndex'), index(actionIndex, 'actionIndex')));
  }
  pageActions(page) { return this._json(api.pageActions, index(page, 'page')); }
  addPageAction(page, action) {
    check(api.addPageAction(this._live(), index(page, 'page'), JSON.stringify(actionRequest(action))));
  }
  removePageAction(page, actionIndex) {
    check(api.removePageAction(this._live(), index(page, 'page'), index(actionIndex, 'actionIndex')));
  }
  customTags() { return this._json(api.customTags); }
  customTagData(tagIndex) { return this._buffer(api.customTagData, index(tagIndex, 'tagIndex')); }
  replaceCustomTagData(tagIndex, data) {
    const input = bytes(data);
    return this._json(api.replaceCustomTagData, index(tagIndex, 'tagIndex'), input, input.length);
  }
  addCustomTag(documentIndex, typeId, data, options = {}) {
    const input = bytes(data);
    return this._json(api.addCustomTag, index(documentIndex, 'documentIndex'),
      JSON.stringify(customTagMetadata(typeId, options)), input, input.length);
  }
  updateCustomTag(tagIndex, typeId, options = {}) {
    return this._json(api.updateCustomTag, index(tagIndex, 'tagIndex'),
      JSON.stringify(customTagMetadata(typeId, options)));
  }
  removeCustomTag(tagIndex) {
    check(api.removeCustomTag(this._live(), index(tagIndex, 'tagIndex')));
  }
  extensions() { return this._json(api.extensions); }
  addExtension(extension) {
    check(api.addExtension(this._live(), JSON.stringify(extensionRequest(extension))));
  }
  updateExtension(extensionIndex, extension) {
    check(api.updateExtension(this._live(), index(extensionIndex, 'extensionIndex'),
      JSON.stringify(extensionRequest(extension))));
  }
  removeExtension(extensionIndex) {
    check(api.removeExtension(this._live(), index(extensionIndex, 'extensionIndex')));
  }
  appendDocumentBody() {
    const output = [0];
    check(api.appendDocumentBody(this._live(), output));
    return Number(output[0]);
  }
  moveDocumentBody(from, to) {
    check(api.moveDocumentBody(this._live(), index(from, 'from'), index(to, 'to')));
  }
  removeDocumentBody(documentIndex) {
    check(api.removeDocumentBody(this._live(), index(documentIndex, 'documentIndex')));
  }
  extractDocumentBody(documentIndex) {
    const output = [null];
    check(api.extractDocumentBody(this._live(), index(documentIndex, 'documentIndex'), output));
    return new Document(output[0]);
  }
  appendDocumentBodiesFrom(source) {
    if (!(source instanceof Document)) throw new TypeError('source must be a Document');
    return this._json(api.appendDocumentBodiesFrom, source._live());
  }
  appendDocumentBodyFrom(source, documentIndex) {
    if (!(source instanceof Document)) throw new TypeError('source must be a Document');
    const output = [0];
    check(api.appendDocumentBodyFrom(this._live(), source._live(), index(documentIndex, 'documentIndex'), output));
    return Number(output[0]);
  }
  entries() { return this._json(api.entries); }
  entry(name) { return this._buffer(api.getEntry, utf8(name, 'path')); }
  setEntry(name, data) {
    const input = bytes(data);
    check(api.setEntry(this._live(), utf8(name, 'path'), input, input.length));
  }
  removeEntry(name) {
    const removed = [0];
    check(api.removeEntry(this._live(), utf8(name, 'path'), removed));
    return removed[0] === 1;
  }
  pageText(page) { return this._json(api.pageText, index(page)); }
  allText() { return this._json(api.allText); }
  pageTextInRegion(page, box) {
    return this._json(api.pageTextInRegion, index(page), ...boundary(box));
  }
  keywordPositions(keywords, pages = null) {
    if (!Array.isArray(keywords)) throw new TypeError('keywords must be an array');
    const query = { keywords: keywords.map((word) => utf8(word, 'keyword')) };
    if (pages !== null) {
      if (!Array.isArray(pages)) throw new TypeError('pages must be an array');
      query.pages = pages.map((page) => index(page, 'page'));
    }
    return this._json(api.keywordPositions, JSON.stringify(query));
  }
  pageTextWithTemplates(page) { return this._json(api.pageTextWithTemplates, index(page)); }
  pageSources(page) { return this._json(api.pageSources, index(page)); }
  pageLayers(page) { return this._json(api.pageLayers, index(page)); }
  addPageLayer(page, layerType, drawParam = null) {
    const output = [0];
    check(api.addPageLayer(this._live(), index(page), utf8(layerType, 'layerType'),
      drawParam === null ? null : utf8(drawParam, 'drawParam'), output));
    return String(output[0]);
  }
  removePageLayer(page, layerId) {
    check(api.removePageLayer(this._live(), index(page), utf8(layerId, 'layerId')));
  }
  setPageLayerType(page, layerId, layerType) {
    check(api.setPageLayerType(this._live(), index(page), utf8(layerId, 'layerId'), utf8(layerType, 'layerType')));
  }
  setPageLayerName(page, layerId, name = null) {
    check(api.setPageLayerName(this._live(), index(page), utf8(layerId, 'layerId'),
      name === null ? null : utf8(name, 'name')));
  }
  setPageLayerDrawParam(page, layerId, drawParam = null) {
    check(api.setPageLayerDrawParam(this._live(), index(page), utf8(layerId, 'layerId'),
      drawParam === null ? null : utf8(drawParam, 'drawParam')));
  }
  movePageObjectToLayer(page, objectId, layerId) {
    check(api.movePageObjectToLayer(this._live(), index(page), utf8(objectId, 'objectId'), utf8(layerId, 'layerId')));
  }
  movePageObject(page, objectId, beforeId = null) {
    check(api.movePageObject(this._live(), index(page), utf8(objectId, 'objectId'),
      beforeId === null ? null : utf8(beforeId, 'beforeId')));
  }
  groupPageObjects(page, firstId, lastId) {
    check(api.groupPageObjects(this._live(), index(page), utf8(firstId, 'firstId'), utf8(lastId, 'lastId')));
  }
  ungroupPageObjects(page, objectId) {
    check(api.ungroupPageObjects(this._live(), index(page), utf8(objectId, 'objectId')));
  }
  pageObjects(page) { return this._json(api.pageObjects, index(page)); }
  pageObjectClips(page) { return this._json(api.pageObjectClips, index(page)); }
  _setPageObjectClips(fn, page, objectId, property, clips, followsCtm) {
    if (typeof followsCtm !== 'boolean') throw new TypeError('followsCtm must be a boolean');
    check(fn(this._live(), index(page), utf8(objectId, 'objectId'),
      JSON.stringify({[property]: clips, follows_ctm: followsCtm})));
  }
  setPageObjectClipRects(page, objectId, rects, followsCtm = false) {
    this._setPageObjectClips(api.setPageObjectClipRects, page, objectId, 'rects', rects, followsCtm);
  }
  setPageObjectPathClips(page, objectId, clips, followsCtm = false) {
    this._setPageObjectClips(api.setPageObjectPathClips, page, objectId, 'clips', clips, followsCtm);
  }
  setPageObjectClips(page, objectId, clips, followsCtm = false) {
    this._setPageObjectClips(api.setPageObjectClips, page, objectId, 'clips', clips, followsCtm);
  }
  pageObjectPaints(page) { return this._json(api.pageObjectPaints, index(page)); }
  pageObjectStrokeStyles(page) { return this._json(api.pageObjectStrokeStyles, index(page)); }
  pageObjectActions(page, objectId) {
    return this._json(api.pageObjectActions, index(page), utf8(objectId, 'objectId'));
  }
  setPageObjectBoundary(page, objectId, box) {
    check(api.setPageObjectBoundary(this._live(), index(page), utf8(objectId, 'objectId'), ...boundary(box)));
  }
  setPageObjectCtm(page, objectId, ctm = null) {
    if (ctm !== null && (!Array.isArray(ctm) || ctm.length !== 6 ||
      ctm.some((part) => typeof part !== 'number' || !Number.isFinite(part)))) {
      throw new RangeError('ctm must contain six finite numbers or be null');
    }
    check(api.setPageObjectCtm(this._live(), index(page), utf8(objectId, 'objectId'),
      ctm === null ? null : JSON.stringify(ctm)));
  }
  setPageObjectVisible(page, objectId, visible) {
    if (typeof visible !== 'boolean') throw new TypeError('visible must be a boolean');
    check(api.setPageObjectVisible(this._live(), index(page), utf8(objectId, 'objectId'), Number(visible)));
  }
  setPageObjectAlpha(page, objectId, alpha) {
    if (!Number.isInteger(alpha) || alpha < 0 || alpha > 255) throw new RangeError('alpha must be an integer from 0 to 255');
    check(api.setPageObjectAlpha(this._live(), index(page), utf8(objectId, 'objectId'), alpha));
  }
  setPagePathData(page, objectId, commands) {
    check(api.setPagePathData(this._live(), index(page), utf8(objectId, 'objectId'), utf8(commands, 'commands')));
  }
  setPagePathStrokeStyle(page, objectId, style) {
    check(api.setPagePathStrokeStyle(this._live(), index(page), utf8(objectId, 'objectId'),
      JSON.stringify(optionsObject(style))));
  }
  setPagePathDashSequence(page, objectId, dashSequence = null, dashOffset = null) {
    if (dashSequence !== null && !Array.isArray(dashSequence)) {
      throw new TypeError('dashSequence must be an array or null');
    }
    check(api.setPagePathDashSequence(this._live(), index(page), utf8(objectId, 'objectId'),
      JSON.stringify({ dash_sequence: dashSequence, dash_offset: dashOffset })));
  }
  setPageTextTypography(page, objectId, style) {
    check(api.setPageTextTypography(this._live(), index(page), utf8(objectId, 'objectId'),
      JSON.stringify(optionsObject(style))));
  }
  setPageTextCode(page, objectId, codeIndex, value) {
    check(api.editTextCode(this._live(), JSON.stringify({ operation: 'set', page_index: index(page),
      object_id: utf8(objectId, 'objectId'), code_index: index(codeIndex, 'codeIndex'), text: textValue(value) })));
  }
  setPageTextCodeLayout(page, objectId, codeIndex, layout) {
    check(api.setPageTextCodeLayout(this._live(), index(page), utf8(objectId, 'objectId'),
      index(codeIndex, 'codeIndex'), JSON.stringify(optionsObject(layout))));
  }
  appendPageTextCode(page, objectId, value, layout = {}) {
    check(api.editTextCode(this._live(), JSON.stringify({ operation: 'append', page_index: index(page),
      object_id: utf8(objectId, 'objectId'), text: textValue(value), layout: optionsObject(layout) })));
  }
  removePageTextCode(page, objectId, codeIndex) {
    check(api.removePageTextCode(this._live(), index(page), utf8(objectId, 'objectId'), index(codeIndex, 'codeIndex')));
  }
  setPageTextFont(page, objectId, fontId) {
    check(api.setPageTextFont(this._live(), index(page), utf8(objectId, 'objectId'), utf8(fontId, 'fontId')));
  }
  setPageTextPaintFlags(page, objectId, options = {}) {
    check(api.setPageTextPaintFlags(this._live(), index(page), utf8(objectId, 'objectId'),
      JSON.stringify(optionsObject(options))));
  }
  setPageObjectName(page, objectId, name = null) {
    check(api.setPageObjectName(this._live(), index(page), utf8(objectId, 'objectId'),
      name === null ? null : utf8(name, 'name')));
  }
  setPageObjectBlendMode(page, objectId, blendMode = null) {
    check(api.setPageObjectBlendMode(this._live(), index(page), utf8(objectId, 'objectId'),
      blendMode === null ? null : utf8(blendMode, 'blendMode')));
  }
  setPageObjectDrawParam(page, objectId, drawParam = null) {
    check(api.setPageObjectDrawParam(this._live(), index(page), utf8(objectId, 'objectId'),
      drawParam === null ? null : utf8(drawParam, 'drawParam')));
  }
  _setPageObjectPaint(fn, page, objectId, style) {
    check(fn(this._live(), index(page), utf8(objectId, 'objectId'), JSON.stringify(optionsObject(style))));
  }
  setPageObjectSolidFill(page, objectId, style) {
    this._setPageObjectPaint(api.setPageObjectSolidFill, page, objectId, style);
  }
  setPageObjectSolidStroke(page, objectId, style) {
    this._setPageObjectPaint(api.setPageObjectSolidStroke, page, objectId, style);
  }
  setPageObjectGradientFill(page, objectId, style) {
    this._setPageObjectPaint(api.setPageObjectGradientFill, page, objectId, style);
  }
  setPageObjectGradientStroke(page, objectId, style) {
    this._setPageObjectPaint(api.setPageObjectGradientStroke, page, objectId, style);
  }
  setPageObjectGouraudFill(page, objectId, style) {
    this._setPageObjectPaint(api.setPageObjectGouraudFill, page, objectId, style);
  }
  setPageObjectGouraudStroke(page, objectId, style) {
    this._setPageObjectPaint(api.setPageObjectGouraudStroke, page, objectId, style);
  }
  setPageObjectPatternFill(page, objectId, style) {
    this._setPageObjectPaint(api.setPageObjectPatternFill, page, objectId, style);
  }
  setPageObjectPatternStroke(page, objectId, style) {
    this._setPageObjectPaint(api.setPageObjectPatternStroke, page, objectId, style);
  }
  setPageImageMask(page, objectId, resourceId = null) {
    check(api.setPageImageMask(this._live(), index(page), utf8(objectId, 'objectId'),
      resourceId === null ? null : utf8(resourceId, 'resourceId')));
  }
  setPageImageSubstitution(page, objectId, resourceId = null) {
    check(api.setPageImageSubstitution(this._live(), index(page), utf8(objectId, 'objectId'),
      resourceId === null ? null : utf8(resourceId, 'resourceId')));
  }
  setPageImageBorder(page, objectId, style = null) {
    check(api.setPageImageBorder(this._live(), index(page), utf8(objectId, 'objectId'),
      JSON.stringify(style === null ? null : optionsObject(style))));
  }
  pageSourceObjectActions(page, sourcePath, objectId) {
    return this._json(api.pageSourceObjectActions, index(page), utf8(sourcePath, 'sourcePath'), utf8(objectId, 'objectId'));
  }
  addPageObjectUriAction(page, objectId, uri) {
    check(api.addPageObjectUriAction(this._live(), index(page), utf8(objectId, 'objectId'), utf8(uri, 'uri')));
  }
  addPageObjectGotoAction(page, objectId, destination) {
    check(api.addPageObjectGotoAction(this._live(), index(page), utf8(objectId, 'objectId'),
      JSON.stringify(optionsObject(destination))));
  }
  addPageObjectBookmarkAction(page, objectId, bookmarkName) {
    check(api.addPageObjectBookmarkAction(this._live(), index(page), utf8(objectId, 'objectId'),
      utf8(bookmarkName, 'bookmarkName')));
  }
  addPageObjectAttachmentAction(page, objectId, attachmentId, newWindow = null) {
    if (newWindow !== null && typeof newWindow !== 'boolean') throw new TypeError('newWindow must be a boolean or null');
    check(api.addPageObjectAttachmentAction(this._live(), index(page), utf8(objectId, 'objectId'),
      utf8(attachmentId, 'attachmentId'), newWindow === null ? -1 : Number(newWindow)));
  }
  addPageObjectMediaAction(page, objectId, action) {
    check(api.addPageObjectMediaAction(this._live(), index(page, 'page'), utf8(objectId, 'objectId'),
      JSON.stringify(optionsObject(action))));
  }
  addPageObjectSoundAction(page, objectId, resourceId, options = {}) {
    const {volume, repeat, synchronous} = optionsObject(options);
    this.addPageObjectMediaAction(page, objectId,
      {kind: 'Sound', resource_id: String(resourceId), volume, repeat, synchronous});
  }
  addPageObjectMovieAction(page, objectId, resourceId, operator = null) {
    this.addPageObjectMediaAction(page, objectId,
      {kind: 'Movie', resource_id: String(resourceId), operator});
  }
  removePageObjectAction(page, objectId, actionIndex) {
    check(api.removePageObjectAction(this._live(), index(page), utf8(objectId, 'objectId'), index(actionIndex, 'actionIndex')));
  }
  setPageObjectActionRegions(page, objectId, actionIndex, rectangles) {
    if (!Array.isArray(rectangles)) throw new TypeError('rectangles must be an array');
    check(api.setPageObjectActionRegions(this._live(), index(page), utf8(objectId, 'objectId'),
      index(actionIndex, 'actionIndex'), JSON.stringify(rectangles.map(boundary))));
  }
  pageTextObjects(page) { return this._json(api.pageTextObjects, index(page)); }
  fonts() { return this._json(api.fonts); }
  fontData(fontIndex) { return this._buffer(api.fontData, index(fontIndex, 'fontIndex')); }
  pageFonts(page) { return this._json(api.pageFonts, index(page)); }
  pageFontData(page, fontIndex) {
    return this._buffer(api.pageFontData, index(page), index(fontIndex, 'fontIndex'));
  }
  documentColorSpaces(documentIndex) {
    return this._json(api.documentColorSpaces, index(documentIndex, 'documentIndex'));
  }
  pageColorSpaces(page) { return this._json(api.pageColorSpaces, index(page)); }
  documentDrawParams(documentIndex) {
    return this._json(api.documentDrawParams, index(documentIndex, 'documentIndex'));
  }
  pageDrawParams(page) { return this._json(api.pageDrawParams, index(page)); }
  documentDefaultColorSpace(documentIndex) {
    return this._json(api.documentDefaultColorSpace, index(documentIndex, 'documentIndex'));
  }
  setDocumentDefaultColorSpace(documentIndex, colorSpaceId = null) {
    check(api.setDocumentDefaultColorSpace(this._live(), index(documentIndex, 'documentIndex'),
      colorSpaceId === null ? null : utf8(colorSpaceId, 'colorSpaceId')));
  }
  addDocumentColorSpace(documentIndex, spec) {
    const output = [0];
    check(api.addDocumentColorSpace(this._live(), index(documentIndex, 'documentIndex'),
      JSON.stringify(optionsObject(spec)), output));
    return String(output[0]);
  }
  addPageColorSpace(page, spec) {
    const output = [0];
    check(api.addPageColorSpace(this._live(), index(page), JSON.stringify(optionsObject(spec)), output));
    return String(output[0]);
  }
  setDocumentColorSpace(documentIndex, id, spec) {
    check(api.setDocumentColorSpace(this._live(), index(documentIndex, 'documentIndex'),
      utf8(id, 'id'), JSON.stringify(optionsObject(spec))));
  }
  setPageColorSpace(page, id, spec) {
    check(api.setPageColorSpace(this._live(), index(page), utf8(id, 'id'),
      JSON.stringify(optionsObject(spec))));
  }
  removeDocumentColorSpace(documentIndex, id) {
    check(api.removeDocumentColorSpace(this._live(), index(documentIndex, 'documentIndex'), utf8(id, 'id')));
  }
  removePageColorSpace(page, id) {
    check(api.removePageColorSpace(this._live(), index(page), utf8(id, 'id')));
  }
  addDocumentDrawParam(documentIndex, style) {
    const output = [0];
    check(api.addDocumentDrawParam(this._live(), index(documentIndex, 'documentIndex'),
      JSON.stringify(optionsObject(style)), output));
    return String(output[0]);
  }
  addPageDrawParam(page, style) {
    const output = [0];
    check(api.addPageDrawParam(this._live(), index(page), JSON.stringify(optionsObject(style)), output));
    return String(output[0]);
  }
  setDocumentDrawParam(documentIndex, id, style) {
    check(api.setDocumentDrawParam(this._live(), index(documentIndex, 'documentIndex'),
      utf8(id, 'id'), JSON.stringify(optionsObject(style))));
  }
  setPageDrawParam(page, id, style) {
    check(api.setPageDrawParam(this._live(), index(page), utf8(id, 'id'),
      JSON.stringify(optionsObject(style))));
  }
  removeDocumentDrawParam(documentIndex, id) {
    check(api.removeDocumentDrawParam(this._live(), index(documentIndex, 'documentIndex'), utf8(id, 'id')));
  }
  removePageDrawParam(page, id) {
    check(api.removePageDrawParam(this._live(), index(page), utf8(id, 'id')));
  }
  pagePaths(page) { return this._json(api.pagePaths, index(page)); }
  pageImages(page) { return this._json(api.pageImages, index(page)); }
  pageCompositeImages(page) { return this._json(api.pageCompositeImages, index(page)); }
  pageCompositeObjects(page) { return this._json(api.pageCompositeObjects, index(page)); }
  pageCompositeUnits(page) { return this._json(api.pageCompositeUnits, index(page)); }
  setPageCompositeUnitImages(page, unitId, options = {}) {
    const { thumbnailId = null, substitutionId = null } = optionsObject(options);
    check(api.setPageCompositeUnitImages(this._live(), index(page), utf8(unitId, 'unitId'),
      thumbnailId === null ? null : utf8(thumbnailId, 'thumbnailId'),
      substitutionId === null ? null : utf8(substitutionId, 'substitutionId')));
  }
  pageCompositeImageData(page, image) { return this._buffer(api.pageCompositeImageData, index(page), index(image)); }
  pageImageData(page, image) { return this._buffer(api.pageImageData, index(page), index(image)); }
  validateReferences() { return this._json(api.validateReferences); }
  archiveViolations() { return this._json(api.archiveViolations); }
  prepareArchive() {
    const output = [null];
    check(api.prepareArchive(this._live(), output));
    return new Document(output[0]);
  }
  bookmarks() { return this._json(api.bookmarks); }
  addBookmark(documentIndex, name, destination) {
    const request = { ...optionsObject(destination), name: utf8(name, 'name') };
    check(api.addBookmark(this._live(), index(documentIndex, 'documentIndex'), JSON.stringify(request)));
  }
  removeBookmark(documentIndex, name) {
    check(api.removeBookmark(this._live(), index(documentIndex, 'documentIndex'), utf8(name, 'name')));
  }
  outlines() { return this._json(api.outlines); }
  addOutline(documentIndex, title, options = {}) {
    const { parentIndex = null, destination = null, expanded = true } = optionsObject(options);
    if (typeof expanded !== 'boolean') throw new TypeError('expanded must be a boolean');
    const outline = {
      title: utf8(title, 'title'),
      parent_index: parentIndex === null ? null : index(parentIndex, 'parentIndex'),
      destination: destination === null ? null : optionsObject(destination),
      expanded,
    };
    const output = [0];
    check(api.addOutline(this._live(), index(documentIndex, 'documentIndex'), JSON.stringify(outline), output));
    return Number(output[0]);
  }
  removeOutline(outlineIndex) {
    check(api.removeOutline(this._live(), index(outlineIndex, 'outlineIndex')));
  }
  setOutlineTitle(outlineIndex, title) {
    check(api.setOutlineTitle(this._live(), index(outlineIndex, 'outlineIndex'), utf8(title, 'title')));
  }
  setOutlineExpanded(outlineIndex, expanded) {
    if (typeof expanded !== 'boolean') throw new TypeError('expanded must be a boolean');
    check(api.setOutlineExpanded(this._live(), index(outlineIndex, 'outlineIndex'), Number(expanded)));
  }
  setOutlineDestination(outlineIndex, destination) {
    check(api.setOutlineDestination(this._live(), index(outlineIndex, 'outlineIndex'),
      JSON.stringify(optionsObject(destination))));
  }
  setOutlineBookmark(outlineIndex, name) {
    check(api.setOutlineBookmark(this._live(), index(outlineIndex, 'outlineIndex'), utf8(name, 'name')));
  }
  versions() { return this._json(api.versions); }
  addVersionSnapshot(documentIndex, id, name = null) {
    const request = { id: utf8(id, 'id'), name: name === null ? null : utf8(name, 'name') };
    return this._json(api.addVersionSnapshot, index(documentIndex, 'documentIndex'), JSON.stringify(request));
  }
  extractVersionSnapshot(versionIndex) {
    const output = [null];
    check(api.extractVersionSnapshot(this._live(), index(versionIndex, 'versionIndex'), output));
    return new Document(output[0]);
  }
  annotations() { return this._json(api.annotations); }
  annotationObjects(page, annotationId) {
    return this._json(api.annotationObjects, index(page), utf8(String(annotationId), 'annotationId'));
  }
  appendAnnotationObjects(page, annotationId, objects) {
    if (!Array.isArray(objects)) throw new TypeError('objects must be an array');
    return this._json(api.appendAnnotationObjects, index(page),
      utf8(String(annotationId), 'annotationId'), JSON.stringify({objects}));
  }
  updateAnnotationObject(page, annotationId, objectId, update) {
    check(api.updateAnnotationObject(this._live(), index(page),
      utf8(String(annotationId), 'annotationId'), utf8(String(objectId), 'objectId'),
      JSON.stringify(optionsObject(update))));
  }
  setAnnotationObjectName(page, annotationId, objectId, name = null) {
    check(api.setAnnotationObjectName(this._live(), index(page),
      utf8(String(annotationId), 'annotationId'), utf8(String(objectId), 'objectId'),
      name === null ? null : utf8(name, 'name')));
  }
  setAnnotationObjectBlendMode(page, annotationId, objectId, blendMode = null) {
    check(api.setAnnotationObjectBlendMode(this._live(), index(page),
      utf8(String(annotationId), 'annotationId'), utf8(String(objectId), 'objectId'),
      blendMode === null ? null : utf8(blendMode, 'blendMode')));
  }
  setAnnotationObjectDrawParam(page, annotationId, objectId, drawParam = null) {
    check(api.setAnnotationObjectDrawParam(this._live(), index(page),
      utf8(String(annotationId), 'annotationId'), utf8(String(objectId), 'objectId'),
      drawParam === null ? null : utf8(String(drawParam), 'drawParam')));
  }
  removeAnnotationObject(page, annotationId, objectId) {
    check(api.removeAnnotationObject(this._live(), index(page),
      utf8(String(annotationId), 'annotationId'), utf8(String(objectId), 'objectId')));
  }
  setAnnotationPathData(page, annotationId, objectId, commands) {
    check(api.setAnnotationPathData(this._live(), index(page),
      utf8(String(annotationId), 'annotationId'), utf8(String(objectId), 'objectId'),
      utf8(commands, 'commands')));
  }
  setAnnotationPathStrokeStyle(page, annotationId, objectId, style) {
    check(api.setAnnotationPathStrokeStyle(this._live(), index(page),
      utf8(String(annotationId), 'annotationId'), utf8(String(objectId), 'objectId'),
      JSON.stringify(optionsObject(style))));
  }
  setAnnotationTextTypography(page, annotationId, objectId, style) {
    check(api.setAnnotationTextTypography(this._live(), index(page),
      utf8(String(annotationId), 'annotationId'), utf8(String(objectId), 'objectId'),
      JSON.stringify(optionsObject(style))));
  }
  setAnnotationTextPaintFlags(page, annotationId, objectId, options = {}) {
    check(api.setAnnotationTextPaintFlags(this._live(), index(page),
      utf8(String(annotationId), 'annotationId'), utf8(String(objectId), 'objectId'),
      JSON.stringify(optionsObject(options))));
  }
  setAnnotationTextCodeLayout(page, annotationId, objectId, codeIndex, layout) {
    check(api.setAnnotationTextCodeLayout(this._live(), index(page),
      utf8(String(annotationId), 'annotationId'), utf8(String(objectId), 'objectId'),
      index(codeIndex, 'codeIndex'), JSON.stringify(optionsObject(layout))));
  }
  appendAnnotationTextCode(page, annotationId, objectId, text, layout = {}) {
    check(api.editAnnotationTextCode(this._live(), JSON.stringify({
      operation: 'append', page_index: index(page),
      annotation_id: utf8(String(annotationId), 'annotationId'),
      object_id: utf8(String(objectId), 'objectId'),
      text: textValue(text), layout: optionsObject(layout),
    })));
  }
  removeAnnotationTextCode(page, annotationId, objectId, codeIndex) {
    check(api.removeAnnotationTextCode(this._live(), index(page),
      utf8(String(annotationId), 'annotationId'), utf8(String(objectId), 'objectId'),
      index(codeIndex, 'codeIndex')));
  }
  setAnnotationTextFont(page, annotationId, objectId, fontId) {
    check(api.setAnnotationTextFont(this._live(), index(page),
      utf8(String(annotationId), 'annotationId'), utf8(String(objectId), 'objectId'),
      utf8(String(fontId), 'fontId')));
  }
  setAnnotationTextCode(page, annotationId, objectId, codeIndex, text) {
    check(api.editAnnotationTextCode(this._live(), JSON.stringify({
      operation: 'set', page_index: index(page),
      annotation_id: utf8(String(annotationId), 'annotationId'),
      object_id: utf8(String(objectId), 'objectId'),
      code_index: index(codeIndex, 'codeIndex'), text: textValue(text),
    })));
  }
  setAnnotationGraphicResource(page, annotationId, objectId, resourceId) {
    check(api.setAnnotationGraphicResource(this._live(), index(page),
      utf8(String(annotationId), 'annotationId'), utf8(String(objectId), 'objectId'),
      utf8(String(resourceId), 'resourceId')));
  }
  addPathAnnotation(page, commands, box, options = {}) {
    const request = { ...optionsObject(options), commands: utf8(commands, 'commands'), boundary: boundary(box) };
    const output = [0];
    check(api.addPathAnnotation(this._live(), index(page), JSON.stringify(request), output));
    return String(output[0]);
  }
  addMultiPathAnnotation(page, box, paths) {
    if (!Array.isArray(paths)) throw new TypeError('paths must be an array');
    const output = [0];
    check(api.addMultiPathAnnotation(this._live(), index(page),
      JSON.stringify({boundary: boundary(box), paths}), output));
    return String(output[0]);
  }
  addMixedAnnotation(page, box, objects) {
    return this._addMixedAnnotation(page, box, objects, 'Path');
  }
  addMixedStampAnnotation(page, box, objects) {
    return this._addMixedAnnotation(page, box, objects, 'Stamp');
  }
  _addMixedAnnotation(page, box, objects, annotationType) {
    if (!Array.isArray(objects)) throw new TypeError('objects must be an array');
    const output = [0];
    check(api.addMixedAnnotation(this._live(), index(page),
      JSON.stringify({boundary: boundary(box), objects, annotation_type: annotationType}), output));
    return String(output[0]);
  }
  addHighlightAnnotation(page, box, color, opacity = 96) {
    const output = [0];
    check(api.addHighlightAnnotation(this._live(), index(page),
      JSON.stringify({ boundary: boundary(box), color, opacity }), output));
    return String(output[0]);
  }
  addTextNoteAnnotation(page, text, box, options = {}) {
    const request = { ...optionsObject(options), text: utf8(text, 'text'), boundary: boundary(box) };
    const output = [0];
    check(api.addTextNoteAnnotation(this._live(), index(page), JSON.stringify(request), output));
    return String(output[0]);
  }
  addShapeAnnotation(page, kind, box, options = {}) {
    const settings = optionsObject(options);
    const request = { ...settings, kind: utf8(kind, 'kind'), boundary: boundary(box) };
    if (Object.hasOwn(settings, 'strokeColor')) {
      request.stroke = settings.strokeColor !== null;
      request.stroke_color = settings.strokeColor;
      delete request.strokeColor;
    }
    if (Object.hasOwn(settings, 'fillColor')) { request.fill_color = settings.fillColor; delete request.fillColor; }
    if (Object.hasOwn(settings, 'lineWidth')) { request.line_width = settings.lineWidth; delete request.lineWidth; }
    if (Object.hasOwn(settings, 'dashSequence')) { request.dash_sequence = settings.dashSequence; delete request.dashSequence; }
    const output = [0];
    check(api.addShapeAnnotation(this._live(), index(page), JSON.stringify(request), output));
    return String(output[0]);
  }
  addLineAnnotation(page, box, endpoints, options = {}) {
    if (!Array.isArray(endpoints) || endpoints.length !== 4 ||
        endpoints.some((part) => typeof part !== 'number' || !Number.isFinite(part))) {
      throw new RangeError('endpoints must contain four finite numbers');
    }
    const settings = optionsObject(options);
    const request = { ...settings, boundary: boundary(box), endpoints };
    if (Object.hasOwn(settings, 'strokeColor')) {
      request.stroke = settings.strokeColor !== null;
      request.stroke_color = settings.strokeColor;
      delete request.strokeColor;
    }
    if (Object.hasOwn(settings, 'lineWidth')) { request.line_width = settings.lineWidth; delete request.lineWidth; }
    if (Object.hasOwn(settings, 'dashSequence')) { request.dash_sequence = settings.dashSequence; delete request.dashSequence; }
    const output = [0];
    check(api.addLineAnnotation(this._live(), index(page), JSON.stringify(request), output));
    return String(output[0]);
  }
  addUriLinkAnnotation(page, uri, box, quadPoints = []) {
    if (!Array.isArray(quadPoints)) throw new TypeError('quadPoints must be an array');
    const output = [0];
    check(api.addUriLinkAnnotation(this._live(), index(page),
      JSON.stringify({ uri: utf8(uri, 'uri'), boundary: boundary(box), quad_points: quadPoints }), output));
    return String(output[0]);
  }
  addGotoLinkAnnotation(page, destination, box, quadPoints = []) {
    if (!Array.isArray(quadPoints)) throw new TypeError('quadPoints must be an array');
    const output = [0];
    check(api.addGotoLinkAnnotation(this._live(), index(page),
      JSON.stringify({destination: optionsObject(destination), boundary: boundary(box), quad_points: quadPoints}), output));
    return String(output[0]);
  }
  addBookmarkLinkAnnotation(page, bookmarkName, box) {
    const output = [0];
    check(api.addBookmarkLinkAnnotation(this._live(), index(page),
      JSON.stringify({ bookmark_name: utf8(bookmarkName, 'bookmarkName'), boundary: boundary(box) }), output));
    return String(output[0]);
  }
  addAttachmentLinkAnnotation(page, attachmentId, box, newWindow = null) {
    if (newWindow !== null && typeof newWindow !== 'boolean') throw new TypeError('newWindow must be a boolean or null');
    const output = [0];
    check(api.addAttachmentLinkAnnotation(this._live(), index(page),
      JSON.stringify({attachment_id: utf8(String(attachmentId), 'attachmentId'),
        boundary: boundary(box), new_window: newWindow}), output));
    return String(output[0]);
  }
  addMediaLinkAnnotation(page, action) {
    const input = optionsObject(action);
    const output = [0];
    check(api.addMediaLinkAnnotation(this._live(), index(page, 'page'),
      JSON.stringify({ ...input, boundary: boundary(input.boundary) }), output));
    return String(output[0]);
  }
  addSoundLinkAnnotation(page, resourceId, box, options = {}) {
    const {volume, repeat, synchronous} = optionsObject(options);
    return this.addMediaLinkAnnotation(page,
      {kind: 'Sound', resource_id: String(resourceId), boundary: box, volume, repeat, synchronous});
  }
  addMovieLinkAnnotation(page, resourceId, box, operator = null) {
    return this.addMediaLinkAnnotation(page,
      {kind: 'Movie', resource_id: String(resourceId), boundary: box, operator});
  }
  _addTextAnnotation(fn, page, text, box, fontSize, style) {
    const output = [0];
    check(fn(this._live(), index(page),
      JSON.stringify({...textWatermarkRequest(text, fontSize, style), boundary: boundary(box)}), output));
    return String(output[0]);
  }
  addTextWatermark(page, text, box, fontSize, style = {}) {
    return this._addTextAnnotation(api.addTextWatermark, page, text, box, fontSize, style);
  }
  addTextStampAnnotation(page, text, box, fontSize, style = {}) {
    return this._addTextAnnotation(api.addTextStampAnnotation, page, text, box, fontSize, style);
  }
  addTextWatermarkToAllPages(text, fontSize, style = {}) {
    return this._json(api.addTextWatermarkAllPages, JSON.stringify(textWatermarkRequest(text, fontSize, style)))
      .map(String);
  }
  _addImageAnnotation(fn, page, box, source, opacity) {
    const output = [0];
    check(fn(this._live(), index(page), JSON.stringify(imageAnnotationRequest(box, source, opacity)), output));
    return String(output[0]);
  }
  addImageWatermark(page, box, source, opacity = 255) {
    return this._addImageAnnotation(api.addImageWatermark, page, box, source, opacity);
  }
  addImageStampAnnotation(page, box, source, opacity = 255) {
    return this._addImageAnnotation(api.addImageStampAnnotation, page, box, source, opacity);
  }
  addImageWatermarkToAllPages(data, format, box, opacity = 255) {
    const request = imageAnnotationRequest(box, {data, format}, opacity);
    return this._json(api.addImageWatermarkAllPages, JSON.stringify(request)).map(String);
  }
  removeAnnotation(page, annotationId) {
    check(api.removeAnnotation(this._live(), index(page), utf8(String(annotationId), 'annotationId')));
  }
  setAnnotationCreator(page, annotationId, creator) {
    check(api.setAnnotationCreator(this._live(), index(page), utf8(String(annotationId), 'annotationId'), utf8(creator, 'creator')));
  }
  setAnnotationSubtype(page, annotationId, subtype = null) {
    check(api.setAnnotationSubtype(this._live(), index(page), utf8(String(annotationId), 'annotationId'),
      subtype === null ? null : utf8(subtype, 'subtype')));
  }
  setAnnotationLastModDate(page, annotationId, date) {
    check(api.setAnnotationLastModDate(this._live(), index(page), utf8(String(annotationId), 'annotationId'), utf8(date, 'date')));
  }
  setAnnotationAppearanceBoundary(page, annotationId, box) {
    check(api.setAnnotationAppearanceBoundary(this._live(), index(page), utf8(String(annotationId), 'annotationId'), ...boundary(box)));
  }
  setAnnotationRemark(page, annotationId, remark = null) {
    check(api.setAnnotationRemark(this._live(), index(page), utf8(String(annotationId), 'annotationId'),
      remark === null ? null : utf8(remark, 'remark')));
  }
  setAnnotationParameter(page, annotationId, name, value) {
    check(api.setAnnotationParameter(this._live(), index(page), utf8(String(annotationId), 'annotationId'),
      utf8(name, 'name'), utf8(value, 'value')));
  }
  removeAnnotationParameter(page, annotationId, name) {
    check(api.removeAnnotationParameter(this._live(), index(page), utf8(String(annotationId), 'annotationId'), utf8(name, 'name')));
  }
  setAnnotationVisible(page, annotationId, visible) {
    if (typeof visible !== 'boolean') throw new TypeError('visible must be a boolean');
    check(api.setAnnotationVisible(this._live(), index(page), utf8(String(annotationId), 'annotationId'), Number(visible)));
  }
  setAnnotationPrint(page, annotationId, print) {
    if (typeof print !== 'boolean') throw new TypeError('print must be a boolean');
    check(api.setAnnotationPrint(this._live(), index(page), utf8(String(annotationId), 'annotationId'), Number(print)));
  }
  setAnnotationFlag(page, annotationId, flag, value) {
    if (!['NoZoom', 'NoRotate', 'ReadOnly'].includes(flag)) throw new RangeError('unsupported annotation flag');
    if (typeof value !== 'boolean') throw new TypeError('value must be a boolean');
    check(api.setAnnotationFlag(this._live(), index(page), utf8(String(annotationId), 'annotationId'), flag, Number(value)));
  }
  integrityManifest() { return this._json(api.integrityManifest); }
  checkIntegrityManifest() { return this._json(api.checkIntegrityManifest); }
  verifyIntegrityManifestSm2(signatureDistid = null) {
    return this._json(api.verifyIntegrityManifestSm2,
      signatureDistid === null ? null : utf8(signatureDistid, 'signatureDistid'));
  }
  _verifyIntegrityManifestSm2Chain(fn, signerCertificate, intermediates, anchorCertificate,
    atUnixSeconds, options) {
    if (!Array.isArray(intermediates)) throw new TypeError('intermediates must be an array');
    const settings = optionsObject(options);
    const payload = {
      signer_certificate_der_base64: bytes(signerCertificate).toString('base64'),
      intermediates_der_base64: intermediates.map((item) => bytes(item).toString('base64')),
      anchor_certificate_der_base64: bytes(anchorCertificate).toString('base64'),
      at_unix_seconds: index(atUnixSeconds, 'atUnixSeconds'),
      certificate_distid: utf8(settings.certificateDistid ?? '', 'certificateDistid'),
      signature_distid: utf8(settings.signatureDistid ?? '1234567812345678', 'signatureDistid'),
    };
    return this._json(fn, JSON.stringify(payload));
  }
  verifyIntegrityManifestSm2WithOrderedChain(signerCertificate, intermediates,
    anchorCertificate, atUnixSeconds, options = {}) {
    return this._verifyIntegrityManifestSm2Chain(api.verifyIntegrityManifestSm2WithOrderedChain,
      signerCertificate, intermediates, anchorCertificate, atUnixSeconds, options);
  }
  verifyIntegrityManifestSm2WithCertificatePool(signerCertificate, candidates,
    anchorCertificate, atUnixSeconds, options = {}) {
    return this._verifyIntegrityManifestSm2Chain(api.verifyIntegrityManifestSm2WithCertificatePool,
      signerCertificate, candidates, anchorCertificate, atUnixSeconds, options);
  }
  attachments() { return this._json(api.attachments); }
  attachmentData(attachment) { return this._buffer(api.attachmentData, index(attachment)); }
  signatures() { return this._json(api.signatures); }
  signedValueData(signature) { return this._buffer(api.signedValueData, index(signature)); }
  sealData(signature) { return this._buffer(api.sealData, index(signature)); }
  checkSignatureReferences(signature) { return this._json(api.checkSignatureReferences, index(signature)); }
  checkSignatureDescriptionHash(signature) {
    return this._json(api.checkSignatureDescriptionHash, index(signature));
  }
  checkSignatureSealMatch(signature) {
    const output = [0];
    check(api.checkSignatureSealMatch(this._live(), index(signature), output));
    return output[0] === -1 ? null : output[0] === 1;
  }
  checkDigitalSignatureWithCertificate(signature, certificate, signatureDistid) {
    const input = bytes(certificate);
    const output = [0];
    if (signatureDistid === undefined) {
      check(api.checkDigitalSignatureWithCertificate(this._live(), index(signature), input, input.length, output));
    } else {
      check(api.checkDigitalSignatureWithCertificateAndDistid(this._live(), index(signature),
        input, input.length, utf8(signatureDistid, 'signatureDistid'), output));
    }
    return output[0] === 1;
  }
  verifySignatureWithDirectTrustAnchor(signature, signerCertificate, anchorCertificate,
    atUnixSeconds, options = {}) {
    const settings = optionsObject(options);
    const signer = bytes(signerCertificate);
    const anchor = bytes(anchorCertificate);
    return this._json(api.verifySignatureWithDirectTrustAnchor, index(signature, 'signature'),
      signer, signer.length, anchor, anchor.length, index(atUnixSeconds, 'atUnixSeconds'),
      utf8(settings.certificateDistid ?? '', 'certificateDistid'),
      utf8(settings.signatureDistid ?? '1234567812345678', 'signatureDistid'));
  }
  _verifySignatureChain(fn, signature, signerCertificate, intermediates,
    anchorCertificate, atUnixSeconds, options) {
    if (!Array.isArray(intermediates)) throw new TypeError('intermediates must be an array');
    const settings = optionsObject(options);
    const signer = bytes(signerCertificate);
    const encoded = Buffer.from(JSON.stringify(intermediates.map((certificate) =>
      bytes(certificate).toString('base64'))), 'utf8');
    const anchor = bytes(anchorCertificate);
    return this._json(fn, index(signature, 'signature'), signer, signer.length,
      encoded, encoded.length, anchor, anchor.length, index(atUnixSeconds, 'atUnixSeconds'),
      utf8(settings.certificateDistid ?? '', 'certificateDistid'),
      utf8(settings.signatureDistid ?? '1234567812345678', 'signatureDistid'));
  }
  verifySignatureWithOrderedSm2Chain(signature, signerCertificate, intermediates,
    anchorCertificate, atUnixSeconds, options = {}) {
    return this._verifySignatureChain(api.verifySignatureWithOrderedSm2Chain, signature,
      signerCertificate, intermediates, anchorCertificate, atUnixSeconds, options);
  }
  verifySignatureWithCertificatePool(signature, signerCertificate, candidates,
    anchorCertificate, atUnixSeconds, options = {}) {
    return this._verifySignatureChain(api.verifySignatureWithCertificatePool, signature,
      signerCertificate, candidates, anchorCertificate, atUnixSeconds, options);
  }
  _verifySignatureTrust(fn, signature, options) {
    const settings = optionsObject(options);
    const signerIntermediates = settings.signerIntermediates ?? [];
    const sealIntermediates = settings.sealIntermediates ?? [];
    if (!Array.isArray(signerIntermediates) || !Array.isArray(sealIntermediates)) {
      throw new TypeError('certificate intermediates must be arrays');
    }
    const payload = {
      signer_certificate_der_base64: bytes(settings.signerCertificate).toString('base64'),
      signer_intermediates_der_base64: signerIntermediates.map((item) => bytes(item).toString('base64')),
      signer_anchor_der_base64: bytes(settings.signerAnchor).toString('base64'),
      seal_intermediates_der_base64: sealIntermediates.map((item) => bytes(item).toString('base64')),
      seal_anchor_der_base64: bytes(settings.sealAnchor ?? Buffer.alloc(0)).toString('base64'),
      at_unix_seconds: index(settings.atUnixSeconds, 'atUnixSeconds'),
      signer_certificate_distid: utf8(settings.signerCertificateDistid ?? '', 'signerCertificateDistid'),
      signer_signature_distid: utf8(settings.signerSignatureDistid ?? '1234567812345678', 'signerSignatureDistid'),
      seal_certificate_distid: utf8(settings.sealCertificateDistid ?? '', 'sealCertificateDistid'),
      seal_signature_distid: utf8(settings.sealSignatureDistid ?? '1234567812345678', 'sealSignatureDistid'),
    };
    const encoded = Buffer.from(JSON.stringify(payload), 'utf8');
    return this._json(fn, index(signature, 'signature'), encoded, encoded.length);
  }
  verifySignatureWithOrderedSm2Chains(signature, options) {
    return this._verifySignatureTrust(api.verifySignatureWithOrderedSm2Chains, signature, options);
  }
  verifySignatureWithCertificatePools(signature, options) {
    return this._verifySignatureTrust(api.verifySignatureWithCertificatePools, signature, options);
  }
  checkSesSignatureValue(signature) {
    const output = [0];
    check(api.checkSesSignatureValue(this._live(), index(signature), output));
    return output[0] === 1;
  }
  checkSesSignatureValueWithDistid(signature, signatureDistid) {
    const output = [0];
    check(api.checkSesSignatureValueWithDistid(this._live(), index(signature),
      utf8(signatureDistid, 'signatureDistid'), output));
    return output[0] === 1;
  }
  checkSesSealSignature(signature) {
    const output = [0];
    check(api.checkSesSealSignature(this._live(), index(signature), output));
    return output[0] === 1;
  }
  checkSesSealSignatureWithDistid(signature, sealDistid) {
    const output = [0];
    check(api.checkSesSealSignatureWithDistid(this._live(), index(signature),
      utf8(sealDistid, 'sealDistid'), output));
    return output[0] === 1;
  }
  checkSesSealCertificateValidity(signature, atUnixSeconds) {
    const output = [0];
    check(api.checkSesSealCertificateValidity(this._live(), index(signature),
      index(atUnixSeconds, 'atUnixSeconds'), output));
    return output[0] === 1;
  }
  checkSignatureCertificateValidity(signature, certificate, atUnixSeconds) {
    const input = bytes(certificate);
    const output = [0];
    check(api.checkSignatureCertificateValidity(this._live(), index(signature), input, input.length,
      index(atUnixSeconds, 'atUnixSeconds'), output));
    return output[0] === 1;
  }
  checkSignatureDirectTrustAnchor(signature, signerCertificate, anchorCertificate,
    atUnixSeconds, certificateDistid = '') {
    const signer = bytes(signerCertificate);
    const anchor = bytes(anchorCertificate);
    const output = [0];
    check(api.checkSignatureDirectTrustAnchor(this._live(), index(signature),
      signer, signer.length, anchor, anchor.length, index(atUnixSeconds, 'atUnixSeconds'),
      utf8(certificateDistid, 'certificateDistid'), output));
    return output[0] === 1;
  }
  checkSesSealIssuerOrderedSm2Chain(signature, intermediates, anchorCertificate,
    atUnixSeconds, certificateDistid = '') {
    if (!Array.isArray(intermediates)) throw new TypeError('intermediates must be an array');
    const encoded = Buffer.from(JSON.stringify(intermediates.map((certificate) => bytes(certificate).toString('base64'))));
    const anchor = bytes(anchorCertificate);
    const output = [0];
    check(api.checkSesSealIssuerOrderedSm2Chain(this._live(), index(signature),
      encoded, encoded.length, anchor, anchor.length, index(atUnixSeconds, 'atUnixSeconds'),
      utf8(certificateDistid, 'certificateDistid'), output));
    return output[0] === 1;
  }
  checkSesSealSignerAuthorization(signature) {
    const output = [0];
    check(api.checkSesSealSignerAuthorization(this._live(), index(signature), output));
    return output[0] === 1;
  }
  checkSesSealValidity(signature, atUnixSeconds) {
    const output = [0];
    check(api.checkSesSealValidity(this._live(), index(signature),
      index(atUnixSeconds, 'atUnixSeconds'), output));
    return output[0] === 1;
  }
  pageSvg(page, options = {}) {
    const prefer = renderOptions(options);
    return this._buffer(prefer ? api.pageSvgWithOptions : api.pageSvg,
      index(page), ...(prefer ? [1] : [])).toString('utf8');
  }
  pagePng(page, dpi = 96, options = {}) {
    const prefer = renderOptions(options);
    return this._buffer(prefer ? api.pagePngWithOptions : api.pagePng,
      index(page), size(dpi, 'dpi'), ...(prefer ? [1] : []));
  }
  toPdf(options = {}) {
    const prefer = renderOptions(options);
    return this._buffer(prefer ? api.toPdfWithOptions : api.toPdf, ...(prefer ? [1] : []));
  }
  toBytes() { return this._buffer(api.toBytes); }
  save(file) { check(api.save(this._live(), utf8(file, 'path'))); }

  addTextPage(text, width = 210, height = 297) {
    const request = { operation: 'add', text: textValue(text), width_mm: size(width, 'width'), height_mm: size(height, 'height') };
    check(api.writeTextPage(this._live(), JSON.stringify(request), null));
  }

  addTextPageWithFont(text, fontId, width = 210, height = 297) {
    const request = { operation: 'add', text: textValue(text), font_id: fontIndex(fontId),
      width_mm: size(width, 'width'), height_mm: size(height, 'height') };
    check(api.writeTextPage(this._live(), JSON.stringify(request), null));
  }

  addBlankPage(width = 210, height = 297) {
    check(api.addBlankPage(this._live(), size(width, 'width'), size(height, 'height')));
  }

  addImagePage(data, format, width = 210, height = 297) {
    const input = bytes(data);
    check(api.addImagePage(this._live(), input, input.length, utf8(format, 'format'),
      size(width, 'width'), size(height, 'height')));
  }

  addPathPage(commands, box, pageWidth = 210, pageHeight = 297, fill = false, stroke = true) {
    if (typeof fill !== 'boolean' || typeof stroke !== 'boolean') {
      throw new TypeError('fill and stroke must be booleans');
    }
    const [x, y, width, height] = boundary(box);
    check(api.addPathPage(this._live(), utf8(commands, 'commands'), x, y, width, height,
      size(pageWidth, 'pageWidth'), size(pageHeight, 'pageHeight'), fill ? 1 : 0,
      stroke ? 1 : 0));
  }

  addFlowedText(documentIndex, value, options = {}) {
    const request = { ...optionsObject(options), text: textValue(value) };
    return this._json(api.addFlowedText, index(documentIndex, 'documentIndex'), JSON.stringify(request));
  }

  addFlowedSpans(documentIndex, spans, options = {}) {
    if (!Array.isArray(spans)) throw new TypeError('spans must be an array');
    const request = { ...optionsObject(options), spans };
    return this._json(api.addFlowedSpans, index(documentIndex, 'documentIndex'), JSON.stringify(request));
  }

  addEmbeddedFont(name, data) {
    const input = bytes(data);
    const output = [0];
    check(api.addEmbeddedFont(this._live(), utf8(name, 'name'), input, input.length, output));
    return String(output[0]);
  }

  addEmbeddedFontToDocument(documentIndex, name, data) {
    const input = bytes(data);
    const output = [0];
    check(api.addEmbeddedFontToDocument(this._live(), index(documentIndex, 'documentIndex'),
      utf8(name, 'name'), input, input.length, output));
    return String(output[0]);
  }

  addEmbeddedFontToPage(pageIndex, name, data) {
    const input = bytes(data);
    const output = [0];
    check(api.addEmbeddedFontToPage(this._live(), index(pageIndex, 'pageIndex'),
      utf8(name, 'name'), input, input.length, output));
    return String(output[0]);
  }

  setFontAttribute(fontIndex, name, value = null) {
    check(api.setFontAttribute(this._live(), index(fontIndex, 'fontIndex'),
      utf8(name, 'name'), value === null ? null : utf8(value, 'value')));
  }

  setPageFontAttribute(pageIndex, fontIndex, name, value = null) {
    check(api.setPageFontAttribute(this._live(), index(pageIndex, 'pageIndex'),
      index(fontIndex, 'fontIndex'), utf8(name, 'name'), value === null ? null : utf8(value, 'value')));
  }

  replaceFontData(fontIndex, data) {
    const input = bytes(data);
    check(api.replaceFontData(this._live(), index(fontIndex, 'fontIndex'), input, input.length));
  }

  replacePageFontData(pageIndex, fontIndex, data) {
    const input = bytes(data);
    check(api.replacePageFontData(this._live(), index(pageIndex, 'pageIndex'),
      index(fontIndex, 'fontIndex'), input, input.length));
  }

  addTable(documentIndex, rows, options = {}) {
    if (!Array.isArray(rows)) throw new TypeError('rows must be an array');
    const request = { ...optionsObject(options), rows };
    return this._json(api.addTable, index(documentIndex, 'documentIndex'), JSON.stringify(request));
  }

  addTableCells(documentIndex, rowCount, cells, options = {}) {
    if (!Array.isArray(cells)) throw new TypeError('cells must be an array');
    const request = { ...optionsObject(options), row_count: index(rowCount, 'rowCount'), cells };
    return this._json(api.addTable, index(documentIndex, 'documentIndex'), JSON.stringify(request));
  }

  appendTextPage(documentIndex, text, width = 210, height = 297) {
    const request = { operation: 'append', document_index: index(documentIndex, 'documentIndex'),
      text: textValue(text), width_mm: size(width, 'width'), height_mm: size(height, 'height') };
    check(api.writeTextPage(this._live(), JSON.stringify(request), null));
  }

  appendTextPageWithFont(documentIndex, text, fontId, width = 210, height = 297) {
    const request = { operation: 'append', document_index: index(documentIndex, 'documentIndex'),
      text: textValue(text), font_id: fontIndex(fontId),
      width_mm: size(width, 'width'), height_mm: size(height, 'height') };
    check(api.writeTextPage(this._live(), JSON.stringify(request), null));
  }

  appendBlankPage(documentIndex, width = 210, height = 297) {
    check(api.appendBlankPage(this._live(), index(documentIndex, 'documentIndex'), size(width, 'width'), size(height, 'height')));
  }

  appendImagePage(documentIndex, data, format, width = 210, height = 297) {
    const input = bytes(data);
    check(api.appendImagePage(this._live(), index(documentIndex, 'documentIndex'), input, input.length,
      utf8(format, 'format'), size(width, 'width'), size(height, 'height')));
  }

  appendPathPage(documentIndex, commands, box, pageWidth = 210, pageHeight = 297,
    fill = false, stroke = true) {
    if (typeof fill !== 'boolean' || typeof stroke !== 'boolean') {
      throw new TypeError('fill and stroke must be booleans');
    }
    const [x, y, width, height] = boundary(box);
    check(api.appendPathPage(this._live(), index(documentIndex, 'documentIndex'),
      utf8(commands, 'commands'), x, y, width, height, size(pageWidth, 'pageWidth'),
      size(pageHeight, 'pageHeight'), fill ? 1 : 0, stroke ? 1 : 0));
  }

  insertTextPage(documentIndex, position, text, width = 210, height = 297) {
    const request = { operation: 'insert', document_index: index(documentIndex, 'documentIndex'),
      position: index(position, 'position'), text: textValue(text),
      width_mm: size(width, 'width'), height_mm: size(height, 'height') };
    const output = [0];
    check(api.writeTextPage(this._live(), JSON.stringify(request), output));
    return Number(output[0]);
  }

  insertBlankPage(documentIndex, position, width = 210, height = 297) {
    const output = [0];
    check(api.insertBlankPage(this._live(), index(documentIndex, 'documentIndex'), index(position, 'position'),
      size(width, 'width'), size(height, 'height'), output));
    return Number(output[0]);
  }

  addTextObject(page, value, box, options = {}) {
    const request = { size: 5, ...optionsObject(options), text: textValue(value), boundary: boundary(box) };
    if (request.font_id !== undefined && request.font_id !== null) request.font_id = String(request.font_id);
    const output = [0];
    check(api.addTextObject(this._live(), index(page), JSON.stringify(request), output));
    return String(output[0]);
  }

  addPathObject(page, commands, box, options = {}) {
    const request = { ...optionsObject(options), commands: utf8(commands, 'commands'), boundary: boundary(box) };
    const output = [0];
    check(api.addPathObject(this._live(), index(page), JSON.stringify(request), output));
    return String(output[0]);
  }

  addImageObject(page, data, format, box) {
    const input = bytes(data);
    const [x, y, width, height] = boundary(box);
    const output = [0];
    check(api.addImageObject(this._live(), index(page), input, input.length, utf8(format, 'format'), x, y, width, height, output));
    return String(output[0]);
  }

  addMultimediaResource(documentIndex, mediaType, format, data) {
    const input = bytes(data);
    const output = [0];
    check(api.addMultimediaResource(this._live(), index(documentIndex, 'documentIndex'),
      utf8(mediaType, 'mediaType'), utf8(format, 'format'), input, input.length, output));
    return String(output[0]);
  }

  pageMultimediaResources(page) { return this._json(api.pageMultimediaResources, index(page, 'page')); }
  pageMultimediaData(page, resourceId) {
    return this._buffer(api.pageMultimediaData, index(page, 'page'), utf8(resourceId, 'resourceId'));
  }
  documentMultimediaResources(documentIndex) {
    return this._json(api.documentMultimediaResources, index(documentIndex, 'documentIndex'));
  }
  documentMultimediaData(documentIndex, resourceId) {
    return this._buffer(api.documentMultimediaData, index(documentIndex, 'documentIndex'), utf8(resourceId, 'resourceId'));
  }
  addPageMultimediaResource(page, mediaType, format, data) {
    const input = bytes(data);
    const output = [0];
    check(api.addPageMultimediaResource(this._live(), index(page, 'page'),
      utf8(mediaType, 'mediaType'), utf8(format, 'format'), input, input.length, output));
    return String(output[0]);
  }
  replaceDocumentMultimediaData(documentIndex, resourceId, data) {
    const input = bytes(data);
    check(api.replaceDocumentMultimediaData(this._live(), index(documentIndex, 'documentIndex'),
      utf8(resourceId, 'resourceId'), input, input.length));
  }
  replacePageMultimediaData(page, resourceId, data) {
    const input = bytes(data);
    check(api.replacePageMultimediaData(this._live(), index(page, 'page'),
      utf8(resourceId, 'resourceId'), input, input.length));
  }
  removeDocumentMultimediaResource(documentIndex, resourceId) {
    check(api.removeDocumentMultimediaResource(this._live(), index(documentIndex, 'documentIndex'),
      utf8(resourceId, 'resourceId')));
  }
  removePageMultimediaResource(page, resourceId) {
    check(api.removePageMultimediaResource(this._live(), index(page, 'page'), utf8(resourceId, 'resourceId')));
  }

  addCompositePathObject(page, commands, box, options = {}) {
    const request = { ...optionsObject(options), commands: utf8(commands, 'commands'), boundary: boundary(box) };
    const output = [0];
    check(api.addCompositePathObject(this._live(), index(page), JSON.stringify(request), output));
    return String(output[0]);
  }
  addCompositeTextObject(page, text, box, fontSize = 5, fontId = null) {
    const request = {text: textValue(text), boundary: boundary(box),
      size: size(fontSize, 'fontSize'), font_id: fontId === null ? null : utf8(String(fontId), 'fontId')};
    const output = [0];
    check(api.addCompositeTextObject(this._live(), index(page), JSON.stringify(request), output));
    return String(output[0]);
  }

  addCompositeObject(page, resourceId, box, options = {}) {
    const request = { ...optionsObject(options), resource_id: utf8(resourceId, 'resourceId'), boundary: boundary(box) };
    const output = [0];
    check(api.addCompositeObject(this._live(), index(page), JSON.stringify(request), output));
    return String(output[0]);
  }

  addCompositeImageObject(page, data, format, box) {
    const input = bytes(data);
    const [x, y, width, height] = boundary(box);
    const output = [0];
    check(api.addCompositeImageObject(this._live(), index(page), input, input.length,
      utf8(format, 'format'), x, y, width, height, output));
    return String(output[0]);
  }

  addAttachment(name, data) {
    const input = bytes(data);
    check(api.addAttachment(this._live(), utf8(name, 'name'), input, input.length));
  }

  addAttachmentToDocument(documentIndex, name, data) {
    const input = bytes(data);
    check(api.addAttachmentToDocument(this._live(), index(documentIndex), utf8(name, 'name'), input, input.length));
  }

  removeAttachment(attachment) {
    check(api.removeAttachment(this._live(), index(attachment)));
  }

  renameAttachment(attachment, name) {
    check(api.renameAttachment(this._live(), index(attachment), utf8(name, 'name')));
  }

  replaceAttachmentData(attachment, data) {
    const input = bytes(data);
    check(api.replaceAttachmentData(this._live(), index(attachment), input, input.length));
  }

  setAttachmentMetadata(attachment, metadata) {
    check(api.setAttachmentMetadata(this._live(), index(attachment), JSON.stringify(optionsObject(metadata))));
  }

  addDigitalSignature(privateKey, dateTime, signatureDistid) {
    const input = bytes(privateKey);
    const output = [0];
    if (signatureDistid === undefined) {
      check(api.addDigitalSignature(this._live(), input, input.length, utf8(dateTime, 'dateTime'), output));
    } else {
      check(api.addDigitalSignatureWithDistid(this._live(), input, input.length,
        utf8(dateTime, 'dateTime'), utf8(signatureDistid, 'signatureDistid'), output));
    }
    return Number(output[0]);
  }

  addSesSignature(privateKey, certificate, seal, dateTime, options = {}) {
    const key = bytes(privateKey);
    const cert = bytes(certificate);
    const sealBytes = bytes(seal);
    const {signatureDistid = '1234567812345678', sealDistid = '1234567812345678', pageIndex, boundary: stampBoundary} = optionsObject(options);
    const args = [this._live(), key, key.length, cert, cert.length, sealBytes, sealBytes.length,
      utf8(dateTime, 'dateTime')];
    const output = [0];
    if (pageIndex === undefined && stampBoundary === undefined) {
      check(api.addSesSignature(...args, utf8(signatureDistid, 'signatureDistid'),
        utf8(sealDistid, 'sealDistid'), output));
    } else {
      if (pageIndex === undefined || stampBoundary === undefined) {
        throw new TypeError('pageIndex and boundary must be supplied together');
      }
      check(api.addSesSignatureAtPage(...args, index(pageIndex, 'pageIndex'),
        ...boundary(stampBoundary), utf8(signatureDistid, 'signatureDistid'),
        utf8(sealDistid, 'sealDistid'), output));
    }
    return Number(output[0]);
  }

  protectIntegritySm2(privateKey, certificate, signatureDistid = null) {
    const key = bytes(privateKey);
    const cert = bytes(certificate);
    check(api.protectIntegritySm2(this._live(), key, key.length, cert, cert.length,
      signatureDistid === null ? null : utf8(signatureDistid, 'signatureDistid')));
  }

  duplicatePage(page) {
    const output = [0];
    check(api.duplicatePage(this._live(), index(page), output));
    return Number(output[0]);
  }

  duplicatePageAt(page, position) {
    const output = [0];
    check(api.duplicatePageAt(this._live(), index(page), index(position, 'position'), output));
    return Number(output[0]);
  }

  movePage(from, to) {
    check(api.movePage(this._live(), index(from, 'from'), index(to, 'to')));
  }

  reorderDocumentPages(documentIndex, order) {
    if (!Array.isArray(order)) throw new TypeError('order must be an array');
    check(api.reorderDocumentPages(this._live(), index(documentIndex, 'documentIndex'),
      JSON.stringify(order.map((value) => index(value, 'page index')))));
  }

  removePage(page) {
    check(api.removePage(this._live(), index(page)));
  }

  promotePageToTemplate(page, additionalPages = [], zOrder = 'Background') {
    if (!Array.isArray(additionalPages)) throw new TypeError('additionalPages must be an array');
    const output = [0];
    check(api.promotePageToTemplate(this._live(), index(page),
      JSON.stringify(additionalPages.map((item) => index(item, 'page'))), utf8(zOrder, 'zOrder'), output));
    return String(output[0]);
  }

  applyPageTemplate(page, templateId, zOrder = 'Background') {
    check(api.applyPageTemplate(this._live(), index(page), utf8(templateId, 'templateId'), utf8(zOrder, 'zOrder')));
  }

  removePageTemplate(page, templateId) {
    check(api.removePageTemplate(this._live(), index(page), utf8(templateId, 'templateId')));
  }
  removeDocumentTemplate(documentIndex, templateId) {
    check(api.removeDocumentTemplate(this._live(), index(documentIndex, 'documentIndex'), utf8(templateId, 'templateId')));
  }

  duplicatePageObject(page, objectId) {
    const output = [0];
    check(api.duplicatePageObject(this._live(), index(page), utf8(objectId, 'objectId'), output));
    return String(output[0]);
  }

  removePageObject(page, objectId) {
    check(api.removePageObject(this._live(), index(page), utf8(objectId, 'objectId')));
  }
}

function createSesSeal(issuerPrivateKey, issuerCertificate, picture, spec, issuerDistid = '1234567812345678') {
  const key = bytes(issuerPrivateKey);
  const cert = bytes(issuerCertificate);
  const image = bytes(picture);
  const options = optionsObject(spec);
  if (!Array.isArray(options.authorizedSignerCertificates)) {
    throw new TypeError('authorizedSignerCertificates must be an array');
  }
  const normalized = {
    version: options.version, vendor_id: options.vendorId, seal_id: options.sealId,
    name: options.name, seal_type: options.sealType,
    authorized_signer_certificates_base64: options.authorizedSignerCertificates
      .map((certificate) => bytes(certificate).toString('base64')),
    created_at: options.createdAt, valid_from: options.validFrom, valid_until: options.validUntil,
    picture_type: options.pictureType, picture_width_mm: options.pictureWidthMm,
    picture_height_mm: options.pictureHeightMm,
  };
  const request = Buffer.from(JSON.stringify(normalized));
  const output = {};
  check(api.createSesSeal(key, key.length, cert, cert.length, image, image.length,
    request, request.length, utf8(issuerDistid, 'issuerDistid'), output));
  return ownedBuffer(output);
}

module.exports = { Document, OfdError, createSesSeal };
