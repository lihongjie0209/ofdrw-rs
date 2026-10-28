#ifndef OFDRW_H
#define OFDRW_H

#include <stddef.h>
#include <stdint.h>

#ifdef __cplusplus
extern "C" {
#endif

/* Opaque token. Never dereference it. Calls on document handles are serialized;
   closing a handle waits for an in-flight call and repeated close is safe. */
typedef struct OfdDocument OfdDocument;

typedef struct OfdBuffer {
    uint8_t *data;
    size_t len;
} OfdBuffer;

uint32_t ofd_abi_version(void);
const char *ofd_last_error(void);
int32_t ofd_open(const char *path, OfdDocument **output);
int32_t ofd_from_bytes(const uint8_t *data, size_t len, OfdDocument **output);
int32_t ofd_new(OfdDocument **output);
int32_t ofd_is_encrypted(const OfdDocument *handle, int32_t *output);
int32_t ofd_encryption_info_json(const OfdDocument *handle, OfdBuffer *output);
/* username may be NULL; the resulting plaintext document owns a separate handle. */
int32_t ofd_decrypt_with_password(const OfdDocument *handle, const char *password, const char *username, OfdDocument **output);
/* private_key is exactly 32 raw SM2 scalar bytes; username may be NULL. */
int32_t ofd_decrypt_with_sm2_private_key(const OfdDocument *handle, const uint8_t *private_key, size_t key_len, const char *username, OfdDocument **output);
/* recipients_json: [{"username":"alice","password":"secret","user_type":"User"}]. */
int32_t ofd_encrypt_with_passwords_json(const OfdDocument *handle, const char *recipients_json, OfdDocument **output);
/* recipients_json: [{"username":"alice","certificate_der_base64":"...","user_type":"User"}]. */
int32_t ofd_encrypt_with_certificates_json(const OfdDocument *handle, const char *recipients_json, OfdDocument **output);
/* recipients_json contains objects tagged "type":"password" or "type":"certificate". */
int32_t ofd_encrypt_with_recipients_json(const OfdDocument *handle, const char *recipients_json, OfdDocument **output);
/* Imports basic PDF vector pages; unsupported drawing operations return an error. */
int32_t ofd_from_pdf_bytes(const uint8_t *data, size_t len, OfdDocument **output);
void ofd_close(OfdDocument *handle);
int32_t ofd_page_count(const OfdDocument *handle, size_t *output);
/* Append an empty DocBody and return its zero-based index. */
int32_t ofd_append_document_body(OfdDocument *handle, size_t *output_index);
int32_t ofd_move_document_body(OfdDocument *handle, size_t from_index, size_t to_index);
int32_t ofd_remove_document_body(OfdDocument *handle, size_t index);
int32_t ofd_extract_document_body(const OfdDocument *handle, size_t index, OfdDocument **output);
/* Append all bodies from an unsigned source. Returns zero-based indexes as JSON. */
int32_t ofd_append_document_bodies_from_json(OfdDocument *handle, const OfdDocument *source_handle, OfdBuffer *output);
int32_t ofd_append_document_body_from(OfdDocument *handle, const OfdDocument *source_handle, size_t source_index, size_t *output_index);
int32_t ofd_move_page(OfdDocument *handle, size_t from_index, size_t to_index);
int32_t ofd_reorder_document_pages_json(OfdDocument *handle, size_t document_index, const char *order_json);
int32_t ofd_duplicate_page(OfdDocument *handle, size_t index, size_t *output);
/* position is local to the source page's document body. */
int32_t ofd_duplicate_page_at(OfdDocument *handle, size_t index, size_t position, size_t *output);
int32_t ofd_remove_page(OfdDocument *handle, size_t index);
int32_t ofd_pages_json(const OfdDocument *handle, OfdBuffer *output);
/* area_json: {"physical_box":[x,y,w,h],"application_box":[x,y,w,h],"content_box":null,"bleed_box":null}; optional boxes may be omitted. */
int32_t ofd_set_page_area_json(OfdDocument *handle, size_t index, const char *area_json);
int32_t ofd_document_page_area_json(const OfdDocument *handle, size_t document_index, OfdBuffer *output);
int32_t ofd_set_document_page_area_json(OfdDocument *handle, size_t document_index, const char *area_json);
int32_t ofd_validate_references_json(const OfdDocument *handle, OfdBuffer *output);
/* Supported OFD-A rule violations; an empty array is not full certification. */
int32_t ofd_archive_violations_json(const OfdDocument *handle, OfdBuffer *output);
int32_t ofd_prepare_archive(const OfdDocument *handle, OfdDocument **output);
int32_t ofd_entries_json(const OfdDocument *handle, OfdBuffer *output);
int32_t ofd_documents_json(const OfdDocument *handle, OfdBuffer *output);
int32_t ofd_package_info_json(const OfdDocument *handle, OfdBuffer *output);
int32_t ofd_set_package_header(OfdDocument *handle, const char *version, const char *doc_type);
/* Effective permissions for one zero-based document body. */
int32_t ofd_document_permissions_json(const OfdDocument *handle, size_t document_index, OfdBuffer *output);
/* Full permission object with edit/annot/export/signature/watermark/print_screen booleans,
   print:{printable,copies}, and valid_period:null or {start_date,end_date}. */
int32_t ofd_set_document_permissions_json(OfdDocument *handle, size_t document_index, const char *permissions_json);
/* Effective initial viewer preferences for one zero-based document body. */
int32_t ofd_view_preferences_json(const OfdDocument *handle, size_t document_index, OfdBuffer *output);
/* Full preferences object with page_mode, page_layout, tab_display, hide_* flags,
   and mutually exclusive zoom_mode / zoom values. */
int32_t ofd_set_view_preferences_json(OfdDocument *handle, size_t document_index, const char *preferences_json);
/* Direct Document/Actions in execution order. */
int32_t ofd_document_actions_json(const OfdDocument *handle, size_t document_index, OfdBuffer *output);
/* action_json: {"kind":"uri","event":"DO","uri":"..."}, or kind page,
   bookmark, attachment with destination, name, or id/new_window respectively. */
int32_t ofd_add_document_action_json(OfdDocument *handle, size_t document_index, const char *action_json);
int32_t ofd_remove_document_action(OfdDocument *handle, size_t document_index, size_t action_index);
int32_t ofd_page_actions_json(const OfdDocument *handle, size_t page_index, OfdBuffer *output);
int32_t ofd_add_page_action_json(OfdDocument *handle, size_t page_index, const char *action_json);
int32_t ofd_remove_page_action(OfdDocument *handle, size_t page_index, size_t action_index);
int32_t ofd_integrity_manifest_json(const OfdDocument *handle, OfdBuffer *output);
int32_t ofd_check_integrity_manifest_json(const OfdDocument *handle, OfdBuffer *output);
int32_t ofd_verify_integrity_manifest_sm2_json(const OfdDocument *handle, const char *signature_distid, OfdBuffer *output);
int32_t ofd_protect_integrity_sm2(OfdDocument *handle, const uint8_t *private_key, size_t private_key_len, const uint8_t *certificate_der, size_t certificate_len, const char *signature_distid);
int32_t ofd_verify_integrity_manifest_sm2_with_ordered_chain_json(const OfdDocument *handle, const char *options_json, OfdBuffer *output);
int32_t ofd_verify_integrity_manifest_sm2_with_certificate_pool_json(const OfdDocument *handle, const char *options_json, OfdBuffer *output);
int32_t ofd_versions_json(const OfdDocument *handle, OfdBuffer *output);
int32_t ofd_extract_version_snapshot(const OfdDocument *handle, size_t index, OfdDocument **output);
/* snapshot_json: {"id":"v1","name":"optional"}; returns the created version as JSON. */
int32_t ofd_add_version_snapshot_json(OfdDocument *handle, size_t document_index, const char *snapshot_json, OfdBuffer *output);
int32_t ofd_bookmarks_json(const OfdDocument *handle, OfdBuffer *output);
/* bookmark_json: {"name":"...","page_id":"...","kind":"XYZ","left":12.5,...}; coordinates optional. */
int32_t ofd_add_bookmark_json(OfdDocument *handle, size_t document_index, const char *bookmark_json);
int32_t ofd_remove_bookmark(OfdDocument *handle, size_t document_index, const char *name);
/* outline_json: {"title":"...","parent_index":null,"expanded":true,"destination":{"page_id":"1","kind":"XYZ"}}; optional parent_index and destination. */
int32_t ofd_add_outline_json(OfdDocument *handle, size_t document_index, const char *outline_json, size_t *output_index);
int32_t ofd_remove_outline(OfdDocument *handle, size_t index);
int32_t ofd_set_outline_title(OfdDocument *handle, size_t index, const char *title);
/* expanded is 0 or 1. */
int32_t ofd_set_outline_expanded(OfdDocument *handle, size_t index, int32_t expanded);
int32_t ofd_set_outline_destination_json(OfdDocument *handle, size_t index, const char *destination_json);
int32_t ofd_set_outline_bookmark(OfdDocument *handle, size_t index, const char *name);
int32_t ofd_outlines_json(const OfdDocument *handle, OfdBuffer *output);
int32_t ofd_set_document_info_field(OfdDocument *handle, size_t document_index, const char *field, const char *value);
int32_t ofd_remove_document_info_field(OfdDocument *handle, size_t document_index, const char *field);
int32_t ofd_add_document_keyword(OfdDocument *handle, size_t document_index, const char *keyword);
int32_t ofd_remove_document_keyword(OfdDocument *handle, size_t document_index, size_t keyword_index);
int32_t ofd_set_document_custom_data(OfdDocument *handle, size_t document_index, const char *name, const char *value);
int32_t ofd_remove_document_custom_data(OfdDocument *handle, size_t document_index, const char *name);
int32_t ofd_page_text_json(const OfdDocument *handle, size_t index, OfdBuffer *output);
int32_t ofd_all_text_json(const OfdDocument *handle, OfdBuffer *output);
int32_t ofd_page_text_in_region_json(const OfdDocument *handle, size_t index, double x, double y, double width, double height, OfdBuffer *output);
/* query_json: {"keywords":["text"],"pages":[0]}; pages is optional and zero-based. */
int32_t ofd_keyword_positions_json(const OfdDocument *handle, const char *query_json, OfdBuffer *output);
int32_t ofd_page_sources_json(const OfdDocument *handle, size_t index, OfdBuffer *output);
int32_t ofd_page_layers_json(const OfdDocument *handle, size_t index, OfdBuffer *output);
/* Move source content to a template, apply it to source and additional pages. */
int32_t ofd_promote_page_to_template(OfdDocument *handle, size_t source_page_index, const char *additional_pages_json, const char *z_order, size_t *output_id);
int32_t ofd_apply_page_template(OfdDocument *handle, size_t page_index, const char *template_id, const char *z_order);
int32_t ofd_remove_page_template(OfdDocument *handle, size_t page_index, const char *template_id);
int32_t ofd_remove_document_template(OfdDocument *handle, size_t document_index, const char *template_id);
int32_t ofd_page_objects_json(const OfdDocument *handle, size_t index, OfdBuffer *output);
int32_t ofd_page_object_clips_json(const OfdDocument *handle, size_t index, OfdBuffer *output);
int32_t ofd_page_object_paints_json(const OfdDocument *handle, size_t index, OfdBuffer *output);
int32_t ofd_page_object_stroke_styles_json(const OfdDocument *handle, size_t index, OfdBuffer *output);
int32_t ofd_page_object_actions_json(const OfdDocument *handle, size_t page_index, const char *object_id, OfdBuffer *output);
int32_t ofd_page_source_object_actions_json(const OfdDocument *handle, size_t page_index, const char *source_path, const char *object_id, OfdBuffer *output);
int32_t ofd_add_page_object_uri_action(OfdDocument *handle, size_t page_index, const char *object_id, const char *uri);
int32_t ofd_add_page_object_goto_action_json(OfdDocument *handle, size_t page_index, const char *object_id, const char *destination_json);
int32_t ofd_add_page_object_bookmark_action(OfdDocument *handle, size_t page_index, const char *object_id, const char *bookmark_name);
/* new_window: -1 omits the attribute, 0 is false, 1 is true. */
int32_t ofd_add_page_object_attachment_action(OfdDocument *handle, size_t page_index, const char *object_id, const char *attachment_id, int32_t new_window);
int32_t ofd_add_page_object_media_action_json(OfdDocument *handle, size_t page_index, const char *object_id, const char *action_json);
int32_t ofd_remove_page_object_action(OfdDocument *handle, size_t page_index, const char *object_id, size_t action_index);
int32_t ofd_set_page_object_action_regions_json(OfdDocument *handle, size_t page_index, const char *object_id, size_t action_index, const char *rectangles_json);
int32_t ofd_page_text_with_templates_json(const OfdDocument *handle, size_t index, OfdBuffer *output);
int32_t ofd_page_text_objects_json(const OfdDocument *handle, size_t index, OfdBuffer *output);
int32_t ofd_page_paths_json(const OfdDocument *handle, size_t index, OfdBuffer *output);
int32_t ofd_page_images_json(const OfdDocument *handle, size_t index, OfdBuffer *output);
int32_t ofd_page_multimedia_resources_json(const OfdDocument *handle, size_t index, OfdBuffer *output);
int32_t ofd_page_multimedia_data(const OfdDocument *handle, size_t page_index, const char *resource_id, OfdBuffer *output);
int32_t ofd_document_multimedia_resources_json(const OfdDocument *handle, size_t index, OfdBuffer *output);
int32_t ofd_document_multimedia_data(const OfdDocument *handle, size_t document_index, const char *resource_id, OfdBuffer *output);
int32_t ofd_page_composite_images_json(const OfdDocument *handle, size_t index, OfdBuffer *output);
int32_t ofd_page_composite_objects_json(const OfdDocument *handle, size_t index, OfdBuffer *output);
int32_t ofd_page_composite_units_json(const OfdDocument *handle, size_t index, OfdBuffer *output);
int32_t ofd_set_page_composite_unit_images(OfdDocument *handle, size_t page_index, const char *unit_id, const char *thumbnail_id, const char *substitution_id);
int32_t ofd_page_image_data(const OfdDocument *handle, size_t page_index, size_t image_index, OfdBuffer *output);
int32_t ofd_page_composite_image_data(const OfdDocument *handle, size_t page_index, size_t image_index, OfdBuffer *output);
int32_t ofd_page_svg(const OfdDocument *handle, size_t index, OfdBuffer *output);
int32_t ofd_page_png(const OfdDocument *handle, size_t index, double dpi, OfdBuffer *output);
int32_t ofd_to_pdf(const OfdDocument *handle, OfdBuffer *output);
int32_t ofd_attachments_json(const OfdDocument *handle, OfdBuffer *output);
int32_t ofd_annotations_json(const OfdDocument *handle, OfdBuffer *output);
/* Direct annotation appearance objects in painting order. */
int32_t ofd_annotation_objects_json(const OfdDocument *handle, size_t page_index, const char *annotation_id, OfdBuffer *output);
/* JSON fields: boundary, ctm (array or null), visible, alpha. */
int32_t ofd_update_annotation_object_json(OfdDocument *handle, size_t page_index, const char *annotation_id, const char *object_id, const char *update_json);
int32_t ofd_set_annotation_object_name(OfdDocument *handle, size_t page_index, const char *annotation_id, const char *object_id, const char *name);
int32_t ofd_set_annotation_object_blend_mode(OfdDocument *handle, size_t page_index, const char *annotation_id, const char *object_id, const char *blend_mode);
int32_t ofd_set_annotation_object_draw_param(OfdDocument *handle, size_t page_index, const char *annotation_id, const char *object_id, const char *draw_param);
int32_t ofd_remove_annotation_object(OfdDocument *handle, size_t page_index, const char *annotation_id, const char *object_id);
int32_t ofd_set_annotation_path_data(OfdDocument *handle, size_t page_index, const char *annotation_id, const char *object_id, const char *commands);
int32_t ofd_set_annotation_path_stroke_style_json(OfdDocument *handle, size_t page_index, const char *annotation_id, const char *object_id, const char *style_json);
int32_t ofd_set_annotation_text_code(OfdDocument *handle, size_t page_index, const char *annotation_id, const char *object_id, size_t code_index, const char *text);
int32_t ofd_edit_annotation_text_code_json(OfdDocument *handle, const char *request_json);
int32_t ofd_set_annotation_text_code_layout_json(OfdDocument *handle, size_t page_index, const char *annotation_id, const char *object_id, size_t code_index, const char *layout_json);
int32_t ofd_set_annotation_text_typography_json(OfdDocument *handle, size_t page_index, const char *annotation_id, const char *object_id, const char *style_json);
int32_t ofd_set_annotation_text_paint_flags_json(OfdDocument *handle, size_t page_index, const char *annotation_id, const char *object_id, const char *flags_json);
int32_t ofd_append_annotation_text_code_json(OfdDocument *handle, size_t page_index, const char *annotation_id, const char *object_id, const char *text, const char *layout_json);
int32_t ofd_remove_annotation_text_code(OfdDocument *handle, size_t page_index, const char *annotation_id, const char *object_id, size_t code_index);
int32_t ofd_set_annotation_text_font(OfdDocument *handle, size_t page_index, const char *annotation_id, const char *object_id, const char *font_id);
int32_t ofd_set_annotation_graphic_resource(OfdDocument *handle, size_t page_index, const char *annotation_id, const char *object_id, const char *resource_id);
int32_t ofd_remove_annotation(OfdDocument *handle, size_t page_index, const char *annotation_id);
int32_t ofd_set_annotation_creator(OfdDocument *handle, size_t page_index, const char *annotation_id, const char *creator);
/* Pass NULL as subtype to remove the optional attribute. */
int32_t ofd_set_annotation_subtype(OfdDocument *handle, size_t page_index, const char *annotation_id, const char *subtype);
int32_t ofd_set_annotation_last_mod_date(OfdDocument *handle, size_t page_index, const char *annotation_id, const char *date);
int32_t ofd_set_annotation_appearance_boundary(OfdDocument *handle, size_t page_index, const char *annotation_id, double x, double y, double width, double height);
/* Pass NULL as remark to remove the optional Remark element. */
int32_t ofd_set_annotation_remark(OfdDocument *handle, size_t page_index, const char *annotation_id, const char *remark);
int32_t ofd_set_annotation_parameter(OfdDocument *handle, size_t page_index, const char *annotation_id, const char *name, const char *value);
int32_t ofd_remove_annotation_parameter(OfdDocument *handle, size_t page_index, const char *annotation_id, const char *name);
/* visible and print must be 0 or 1. */
int32_t ofd_set_annotation_visible(OfdDocument *handle, size_t page_index, const char *annotation_id, int32_t visible);
int32_t ofd_set_annotation_print(OfdDocument *handle, size_t page_index, const char *annotation_id, int32_t print);
/* flag: NoZoom, NoRotate, or ReadOnly; value: 0 or 1. */
int32_t ofd_set_annotation_flag(OfdDocument *handle, size_t page_index, const char *annotation_id, const char *flag, int32_t value);
int32_t ofd_fonts_json(const OfdDocument *handle, OfdBuffer *output);
int32_t ofd_font_data(const OfdDocument *handle, size_t index, OfdBuffer *output);
int32_t ofd_page_fonts_json(const OfdDocument *handle, size_t page_index, OfdBuffer *output);
int32_t ofd_page_font_data(const OfdDocument *handle, size_t page_index, size_t font_index, OfdBuffer *output);
int32_t ofd_signatures_json(const OfdDocument *handle, OfdBuffer *output);
int32_t ofd_signed_value_data(const OfdDocument *handle, size_t index, OfdBuffer *output);
int32_t ofd_seal_data(const OfdDocument *handle, size_t index, OfdBuffer *output);
int32_t ofd_check_signature_references_json(const OfdDocument *handle, size_t index, OfdBuffer *output);
int32_t ofd_check_signature_description_hash_json(const OfdDocument *handle, size_t index, OfdBuffer *output);
/* output: -1 if no detached SES seal applies, 0 mismatch, 1 match. */
int32_t ofd_check_signature_seal_match(const OfdDocument *handle, size_t index, int32_t *output);
int32_t ofd_check_ses_seal_signature(const OfdDocument *handle, size_t index, int32_t *output);
int32_t ofd_check_ses_seal_signature_with_distid(const OfdDocument *handle, size_t index, const char *seal_distid, int32_t *output);
int32_t ofd_check_ses_seal_certificate_validity(const OfdDocument *handle, size_t index, uint64_t at_unix_seconds, int32_t *output);
int32_t ofd_check_ses_seal_signer_authorization(const OfdDocument *handle, size_t index, int32_t *output);
int32_t ofd_check_ses_seal_validity(const OfdDocument *handle, size_t index, uint64_t at_unix_seconds, int32_t *output);
/* intermediates_json is a JSON array of base64 DER certificates, seal issuer to anchor. */
int32_t ofd_check_ses_seal_issuer_ordered_sm2_chain(const OfdDocument *handle, size_t index, const uint8_t *intermediates_json, size_t intermediates_json_len, const uint8_t *anchor_certificate, size_t anchor_len, uint64_t at_unix_seconds, const char *certificate_distid, int32_t *output);
int32_t ofd_check_ses_signature_value(const OfdDocument *handle, size_t index, int32_t *output);
int32_t ofd_check_ses_signature_value_with_distid(const OfdDocument *handle, size_t index, const char *signature_distid, int32_t *output);
int32_t ofd_check_digital_signature_with_certificate(const OfdDocument *handle, size_t index, const uint8_t *certificate, size_t len, int32_t *output);
int32_t ofd_check_digital_signature_with_certificate_and_distid(const OfdDocument *handle, size_t index, const uint8_t *certificate, size_t len, const char *signature_distid, int32_t *output);
int32_t ofd_check_signature_certificate_validity(const OfdDocument *handle, size_t index, const uint8_t *certificate, size_t len, uint64_t at_unix_seconds, int32_t *output);
int32_t ofd_check_signature_direct_trust_anchor(const OfdDocument *handle, size_t index, const uint8_t *signer_certificate, size_t signer_len, const uint8_t *anchor_certificate, size_t anchor_len, uint64_t at_unix_seconds, const char *certificate_distid, int32_t *output);
/* JSON report includes protected file digests, signature value, and direct issuer checks. */
int32_t ofd_verify_signature_with_direct_trust_anchor_json(const OfdDocument *handle, size_t index, const uint8_t *signer_certificate, size_t signer_len, const uint8_t *anchor_certificate, size_t anchor_len, uint64_t at_unix_seconds, const char *certificate_distid, OfdBuffer *output);
int32_t ofd_verify_signature_with_direct_trust_anchor_and_distids_json(const OfdDocument *handle, size_t index, const uint8_t *signer_certificate, size_t signer_len, const uint8_t *anchor_certificate, size_t anchor_len, uint64_t at_unix_seconds, const char *certificate_distid, const char *signature_distid, OfdBuffer *output);
/* intermediates_json is a JSON array of base64 DER certificates, signer to anchor. */
int32_t ofd_verify_signature_with_ordered_sm2_chain_json(const OfdDocument *handle, size_t index, const uint8_t *signer_certificate, size_t signer_len, const uint8_t *intermediates_json, size_t intermediates_json_len, const uint8_t *anchor_certificate, size_t anchor_len, uint64_t at_unix_seconds, const char *certificate_distid, OfdBuffer *output);
int32_t ofd_verify_signature_with_ordered_sm2_chain_and_distids_json(const OfdDocument *handle, size_t index, const uint8_t *signer_certificate, size_t signer_len, const uint8_t *intermediates_json, size_t intermediates_json_len, const uint8_t *anchor_certificate, size_t anchor_len, uint64_t at_unix_seconds, const char *certificate_distid, const char *signature_distid, OfdBuffer *output);
int32_t ofd_verify_signature_with_certificate_pool_json(const OfdDocument *handle, size_t index, const uint8_t *signer_certificate, size_t signer_len, const uint8_t *candidates_json, size_t candidates_json_len, const uint8_t *anchor_certificate, size_t anchor_len, uint64_t at_unix_seconds, const char *certificate_distid, const char *signature_distid, OfdBuffer *output);
/* options_json supplies base64 DER certificates, two anchors, time, and four SM2 IDs. */
int32_t ofd_verify_signature_with_ordered_sm2_chains_json(const OfdDocument *handle, size_t index, const uint8_t *options_json, size_t options_json_len, OfdBuffer *output);
int32_t ofd_verify_signature_with_certificate_pools_json(const OfdDocument *handle, size_t index, const uint8_t *options_json, size_t options_json_len, OfdBuffer *output);
/* private_key is a 32-byte SM2 secret scalar; signature_datetime is YYYY-MM-DD HH:MM:SS. */
int32_t ofd_add_digital_signature(OfdDocument *handle, const uint8_t *private_key, size_t len, const char *signature_datetime, size_t *output_index);
int32_t ofd_add_digital_signature_with_distid(OfdDocument *handle, const uint8_t *private_key, size_t len, const char *signature_datetime, const char *signature_distid, size_t *output_index);
/* SES input seal is a complete signed SESeal DER value. */
int32_t ofd_add_ses_signature(OfdDocument *handle, const uint8_t *private_key, size_t private_key_len, const uint8_t *signer_certificate, size_t signer_certificate_len, const uint8_t *seal, size_t seal_len, const char *signature_datetime, size_t *output_index);
int32_t ofd_add_ses_signature_with_distids(OfdDocument *handle, const uint8_t *private_key, size_t private_key_len, const uint8_t *signer_certificate, size_t signer_certificate_len, const uint8_t *seal, size_t seal_len, const char *signature_datetime, const char *signature_distid, const char *seal_distid, size_t *output_index);
int32_t ofd_add_ses_signature_with_stamp_and_distids(OfdDocument *handle, const uint8_t *private_key, size_t private_key_len, const uint8_t *signer_certificate, size_t signer_certificate_len, const uint8_t *seal, size_t seal_len, const char *signature_datetime, size_t page_index, double x_mm, double y_mm, double width_mm, double height_mm, const char *signature_distid, const char *seal_distid, size_t *output_index);
int32_t ofd_create_ses_seal_json(const uint8_t *issuer_private_key, size_t issuer_private_key_len, const uint8_t *issuer_certificate, size_t issuer_certificate_len, const uint8_t *picture, size_t picture_len, const uint8_t *options_json, size_t options_json_len, const char *issuer_distid, OfdBuffer *output);
int32_t ofd_attachment_data(const OfdDocument *handle, size_t index, OfdBuffer *output);
int32_t ofd_custom_tags_json(const OfdDocument *handle, OfdBuffer *output);
int32_t ofd_custom_tag_data(const OfdDocument *handle, size_t index, OfdBuffer *output);
/* Write a new XML payload and update FileLoc together; the old entry remains available. */
int32_t ofd_replace_custom_tag_data(OfdDocument *handle, size_t index, const uint8_t *data, size_t len, OfdBuffer *output);
/* metadata_json: {"type_id":"...","namespace":null,"schema_path":null};
   data is a complete XML document; output is the created tag metadata. */
int32_t ofd_add_custom_tag_json(OfdDocument *handle, size_t document_index, const char *metadata_json, const uint8_t *data, size_t len, OfdBuffer *output);
/* Update index metadata without changing its XML payload; output is tag metadata JSON. */
int32_t ofd_update_custom_tag_json(OfdDocument *handle, size_t index, const char *metadata_json, OfdBuffer *output);
int32_t ofd_remove_custom_tag(OfdDocument *handle, size_t index);
int32_t ofd_extensions_json(const OfdDocument *handle, OfdBuffer *output);
/* extension_json includes document_index, app_name, optional company/app_version/date/ref_id,
   properties, extend_data_paths, and complete standalone Data XML strings. */
int32_t ofd_add_extension_json(OfdDocument *handle, const char *extension_json);
int32_t ofd_update_extension_json(OfdDocument *handle, size_t index, const char *extension_json);
int32_t ofd_remove_extension(OfdDocument *handle, size_t index);
int32_t ofd_get_entry(const OfdDocument *handle, const char *path, OfdBuffer *output);
int32_t ofd_set_entry(OfdDocument *handle, const char *path, const uint8_t *data, size_t len);
/* removed is set to 1 when an entry existed, otherwise 0. */
int32_t ofd_remove_entry(OfdDocument *handle, const char *path, int32_t *removed);
int32_t ofd_add_text_page(OfdDocument *handle, const char *text, double width_mm, double height_mm);
/* JSON text transport supports embedded U+0000. "operation" is "add", "append",
 * or "insert"; all require text, width_mm, height_mm. Append/insert require
 * document_index, insert also requires position and writable output. Add and
 * append accept optional font_id; output may be NULL for those operations. */
int32_t ofd_write_text_page_json(OfdDocument *handle, const char *request_json, size_t *output);
int32_t ofd_add_blank_page(OfdDocument *handle, double width_mm, double height_mm);
/* object_json: {"text":"...","boundary":[x,y,width,height],"size":5,"font_id":"1"}; font_id optional. */
/* object_json accepts optional shape=true for single-line bidirectional text using an embedded font. */
int32_t ofd_add_text_object_json(OfdDocument *handle, size_t page_index, const char *object_json, size_t *output_id);
/* Same JSON fields as ofd_add_text_object_json; creates a document composite resource. */
int32_t ofd_add_composite_text_object_json(OfdDocument *handle, size_t page_index, const char *object_json, size_t *output_id);
int32_t ofd_remove_page_object(OfdDocument *handle, size_t page_index, const char *object_id);
int32_t ofd_duplicate_page_object(OfdDocument *handle, size_t page_index, const char *object_id, size_t *output_id);
/* layer_type: Background, Body, or Foreground; draw_param may be NULL. */
int32_t ofd_add_page_layer(OfdDocument *handle, size_t page_index, const char *layer_type, const char *draw_param, size_t *output_id);
int32_t ofd_set_page_layer_type(OfdDocument *handle, size_t page_index, const char *layer_id, const char *layer_type);
/* name may be NULL to clear it. */
int32_t ofd_set_page_layer_name(OfdDocument *handle, size_t page_index, const char *layer_id, const char *name);
int32_t ofd_set_page_layer_draw_param(OfdDocument *handle, size_t page_index, const char *layer_id, const char *draw_param);
/* style_json accepts optional relative, line_width, cap, join, miter_limit, dash_pattern, dash_offset, fill/stroke solid colors, fill_gradient/stroke_gradient, fill_gouraud/stroke_gouraud, or fill_pattern/stroke_pattern. Complex paint objects use the same schemas as the direct paint setters; only one paint kind per fill or stroke is allowed. Pattern cell resources must be document-visible. */
int32_t ofd_add_document_draw_param_json(OfdDocument *handle, size_t document_index, const char *style_json, size_t *output_id);
int32_t ofd_add_page_draw_param_json(OfdDocument *handle, size_t page_index, const char *style_json, size_t *output_id);
int32_t ofd_set_document_draw_param_json(OfdDocument *handle, size_t document_index, const char *id, const char *style_json);
int32_t ofd_set_page_draw_param_json(OfdDocument *handle, size_t page_index, const char *id, const char *style_json);
int32_t ofd_remove_document_draw_param(OfdDocument *handle, size_t document_index, const char *id);
int32_t ofd_remove_page_draw_param(OfdDocument *handle, size_t page_index, const char *id);
int32_t ofd_page_color_spaces_json(const OfdDocument *handle, size_t page_index, OfdBuffer *output);
int32_t ofd_document_color_spaces_json(const OfdDocument *handle, size_t document_index, OfdBuffer *output);
int32_t ofd_document_draw_params_json(const OfdDocument *handle, size_t document_index, OfdBuffer *output);
int32_t ofd_page_draw_params_json(const OfdDocument *handle, size_t page_index, OfdBuffer *output);
/* spec_json: {"color_type":"RGB","bits_per_component":8,"palette":[[255,0,0]]}; profile is optional. */
int32_t ofd_add_document_color_space_json(OfdDocument *handle, size_t document_index, const char *spec_json, size_t *output_id);
int32_t ofd_add_page_color_space_json(OfdDocument *handle, size_t page_index, const char *spec_json, size_t *output_id);
int32_t ofd_set_document_color_space_json(OfdDocument *handle, size_t document_index, const char *id, const char *spec_json);
int32_t ofd_set_page_color_space_json(OfdDocument *handle, size_t page_index, const char *id, const char *spec_json);
int32_t ofd_remove_document_color_space(OfdDocument *handle, size_t document_index, const char *id);
int32_t ofd_remove_page_color_space(OfdDocument *handle, size_t page_index, const char *id);
/* DefaultCS is a document resource ID; NULL clears it. */
int32_t ofd_document_default_color_space_json(const OfdDocument *handle, size_t document_index, OfdBuffer *output);
int32_t ofd_set_document_default_color_space(OfdDocument *handle, size_t document_index, const char *color_space_id);
/* draw_param may be NULL to clear the object's reference. */
int32_t ofd_set_page_object_draw_param(OfdDocument *handle, size_t page_index, const char *object_id, const char *draw_param);
int32_t ofd_move_page_object_to_layer(OfdDocument *handle, size_t page_index, const char *object_id, const char *layer_id);
/* Wrap an inclusive, ordered range of sibling graphic objects in a PageBlock. */
int32_t ofd_group_page_objects(OfdDocument *handle, size_t page_index, const char *first_id, const char *last_id);
/* Remove the immediate PageBlock containing object_id, preserving its children. */
int32_t ofd_ungroup_page_objects(OfdDocument *handle, size_t page_index, const char *object_id);
/* before_id may be NULL to place the object at the end of its current layer or PageBlock. */
int32_t ofd_move_page_object(OfdDocument *handle, size_t page_index, const char *object_id, const char *before_id);
/* visible must be 0 or 1; alpha must be 0..255. */
/* name may be NULL to clear the optional Name attribute. */
int32_t ofd_set_page_object_name(OfdDocument *handle, size_t page_index, const char *object_id, const char *name);
int32_t ofd_set_page_object_blend_mode(OfdDocument *handle, size_t page_index, const char *object_id, const char *blend_mode);
int32_t ofd_set_page_object_clip_rects_json(OfdDocument *handle, size_t page_index, const char *object_id, const char *clips_json);
int32_t ofd_set_page_object_path_clips_json(OfdDocument *handle, size_t page_index, const char *object_id, const char *clips_json);
int32_t ofd_set_page_object_clips_json(OfdDocument *handle, size_t page_index, const char *object_id, const char *clips_json);
int32_t ofd_set_page_object_visible(OfdDocument *handle, size_t page_index, const char *object_id, int32_t visible);
int32_t ofd_set_page_object_alpha(OfdDocument *handle, size_t page_index, const char *object_id, int32_t alpha);
int32_t ofd_set_page_path_stroke_style_json(OfdDocument *handle, size_t page_index, const char *object_id, const char *style_json);
int32_t ofd_set_page_path_dash_sequence_json(OfdDocument *handle, size_t page_index, const char *object_id, const char *style_json);
int32_t ofd_set_page_path_data(OfdDocument *handle, size_t page_index, const char *object_id, const char *commands);
int32_t ofd_set_page_text_typography_json(OfdDocument *handle, size_t page_index, const char *object_id, const char *style_json);
int32_t ofd_set_page_text_code(OfdDocument *handle, size_t page_index, const char *object_id, size_t code_index, const char *text);
/* JSON TextCode transport supports embedded U+0000. "set" requires page_index,
 * object_id, code_index, text. "append" requires page_index, object_id, text,
 * and layout (optional x, y, delta_x, delta_y). */
int32_t ofd_edit_text_code_json(OfdDocument *handle, const char *request_json);
int32_t ofd_set_page_text_code_layout_json(OfdDocument *handle, size_t page_index, const char *object_id, size_t code_index, const char *layout_json);
int32_t ofd_append_page_text_code_json(OfdDocument *handle, size_t page_index, const char *object_id, const char *text, const char *layout_json);
int32_t ofd_remove_page_text_code(OfdDocument *handle, size_t page_index, const char *object_id, size_t code_index);
int32_t ofd_set_page_text_font(OfdDocument *handle, size_t page_index, const char *object_id, const char *font_id);
int32_t ofd_set_page_text_paint_flags_json(OfdDocument *handle, size_t page_index, const char *object_id, const char *flags_json);
int32_t ofd_set_page_image_mask(OfdDocument *handle, size_t page_index, const char *object_id, const char *mask_resource_id);
int32_t ofd_set_page_image_substitution(OfdDocument *handle, size_t page_index, const char *object_id, const char *substitution_resource_id);
int32_t ofd_set_page_image_border_json(OfdDocument *handle, size_t page_index, const char *object_id, const char *style_json);
/* style_json: {"points":[{"x":0,"y":0,"color":[255,0,0],"alpha":255,"edge_flag":0},...],"vertices_per_row":2,"extend":true,"back_color":[0,0,0],"back_alpha":255}; vertices_per_row and optional fields may be omitted. */
int32_t ofd_set_page_object_gouraud_fill_json(OfdDocument *handle, size_t page_index, const char *object_id, const char *style_json);
int32_t ofd_set_page_object_gouraud_stroke_json(OfdDocument *handle, size_t page_index, const char *object_id, const char *style_json);
/* style_json: {"kind":"axial"|"radial","start_point":[x,y],"end_point":[x,y],"segments":[{"color":[r,g,b],"position":0,"alpha":255},...],"map_type":"direct"|"repeat"|"reflect","map_unit":5,"extend":3,"fill_alpha":255,"end_radius":10,"start_radius":0,"eccentricity":0,"angle":0}; radial fields apply only to radial kind. */
int32_t ofd_set_page_object_gradient_fill_json(OfdDocument *handle, size_t page_index, const char *object_id, const char *style_json);
int32_t ofd_set_page_object_gradient_stroke_json(OfdDocument *handle, size_t page_index, const char *object_id, const char *style_json);
/* style_json: {"width":4,"height":4,"x_step":6,"y_step":6,"reflect":"row_and_column","relative_to":"object","ctm":[1,0,0,1,0,0],"alpha":255,"cells":[{"kind":"path","commands":"M 0 0 L 4 0 L 4 4 C","boundary":[0,0,4,4],"fill":true,"fill_color":[255,0,0]}]}; cell kinds: path, text, image, composite. */
int32_t ofd_set_page_object_pattern_fill_json(OfdDocument *handle, size_t page_index, const char *object_id, const char *style_json);
int32_t ofd_set_page_object_pattern_stroke_json(OfdDocument *handle, size_t page_index, const char *object_id, const char *style_json);
/* style_json: {"color":[r,g,b],"alpha":255}; alpha is optional and defaults to 255. */
int32_t ofd_set_page_object_solid_fill_json(OfdDocument *handle, size_t page_index, const char *object_id, const char *style_json);
int32_t ofd_set_page_object_solid_stroke_json(OfdDocument *handle, size_t page_index, const char *object_id, const char *style_json);
int32_t ofd_page_svg_with_options(const OfdDocument *handle, size_t index, int32_t prefer_image_substitution, OfdBuffer *output);
int32_t ofd_page_png_with_options(const OfdDocument *handle, size_t index, double dpi, int32_t prefer_image_substitution, OfdBuffer *output);
int32_t ofd_to_pdf_with_options(const OfdDocument *handle, int32_t prefer_image_substitution, OfdBuffer *output);
int32_t ofd_set_page_object_boundary(OfdDocument *handle, size_t page_index, const char *object_id, double x, double y, double width, double height);
/* ctm_json: JSON array of six numbers, or NULL to remove CTM. */
int32_t ofd_set_page_object_ctm_json(OfdDocument *handle, size_t page_index, const char *object_id, const char *ctm_json);
int32_t ofd_remove_page_layer(OfdDocument *handle, size_t page_index, const char *layer_id);
/* object_json requires commands and boundary. Optional style fields: fill, even_odd, alpha (0..255), stroke, line_width, cap (butt|round|square), join (miter|round|bevel), miter_limit, dash_pattern [dash,gap], dash_offset, fill_color [r,g,b], fill_alpha (0..255), stroke_color [r,g,b], stroke_alpha (0..255). */
int32_t ofd_add_path_object_json(OfdDocument *handle, size_t page_index, const char *object_json, size_t *output_id);
/* Same JSON fields as ofd_add_path_object_json; creates a document composite resource. */
int32_t ofd_add_composite_path_object_json(OfdDocument *handle, size_t page_index, const char *object_json, size_t *output_id);
/* object_json: {"resource_id":"41","boundary":[x,y,width,height],"ctm":[a,b,c,d,e,f],"alpha":128,"draw_param":"42"}; style fields optional. */
int32_t ofd_add_composite_object_json(OfdDocument *handle, size_t page_index, const char *object_json, size_t *output_id);
/* object_json: {"text":"...","boundary":[x,y,width,height],"size":5}. */
/* Required JSON: text, boundary, size. Optional: font_id, color, opacity, angle_degrees, tile_spacing, bold, italic. */
int32_t ofd_add_text_watermark_json(OfdDocument *handle, size_t page_index, const char *object_json, size_t *output_id);
/* Same JSON fields as text watermark; creates a Stamp annotation. */
int32_t ofd_add_text_stamp_annotation_json(OfdDocument *handle, size_t page_index, const char *object_json, size_t *output_id);
/* Same JSON style fields without boundary, including optional tile_spacing; returns IDs for all pages as JSON. */
int32_t ofd_add_text_watermark_all_pages_json(OfdDocument *handle, const char *object_json, OfdBuffer *output);
/* JSON: boundary, optional opacity, and exactly one of resource_id or data_base64 plus format. */
int32_t ofd_add_image_watermark_json(OfdDocument *handle, size_t page_index, const char *object_json, size_t *output_id);
int32_t ofd_add_image_stamp_annotation_json(OfdDocument *handle, size_t page_index, const char *object_json, size_t *output_id);
/* JSON: data_base64, format, boundary, optional opacity; returns all annotation IDs as JSON. */
int32_t ofd_add_image_watermark_all_pages_json(OfdDocument *handle, const char *object_json, OfdBuffer *output);
/* JSON fields match ofd_add_path_object_json; output is the annotation ID. */
int32_t ofd_add_path_annotation_json(OfdDocument *handle, size_t page_index, const char *object_json, size_t *output_id);
int32_t ofd_add_multi_path_annotation_json(OfdDocument *handle, size_t page_index, const char *object_json, size_t *output_id);
/* JSON: boundary, ordered objects with kind path/text/image/composite, optional annotation_type Path or Stamp. */
int32_t ofd_add_mixed_annotation_json(OfdDocument *handle, size_t page_index, const char *object_json, size_t *output_id);
/* JSON: {"objects":[{"kind":"path"|"text"|"image"|"composite",...}]}; returns new object IDs as JSON. */
int32_t ofd_append_annotation_objects_json(OfdDocument *handle, size_t page_index, const char *annotation_id, const char *object_json, OfdBuffer *output);
/* object_json: {"boundary":[x,y,width,height],"color":[r,g,b],"opacity":96}; opacity defaults to 96. */
int32_t ofd_add_highlight_annotation_json(OfdDocument *handle, size_t page_index, const char *object_json, size_t *output_id);
int32_t ofd_add_shape_annotation_json(OfdDocument *handle, size_t page_index, const char *object_json, size_t *output_id);
int32_t ofd_add_line_annotation_json(OfdDocument *handle, size_t page_index, const char *object_json, size_t *output_id);
int32_t ofd_add_text_note_annotation_json(OfdDocument *handle, size_t page_index, const char *object_json, size_t *output_id);
/* JSON requires uri and boundary; output is the Link annotation ID. */
int32_t ofd_add_uri_link_annotation_json(OfdDocument *handle, size_t page_index, const char *object_json, size_t *output_id);
/* object_json: {"destination":{"page_id":"2","kind":"XYZ","left":4,"top":6},"boundary":[x,y,width,height]}. */
int32_t ofd_add_goto_link_annotation_json(OfdDocument *handle, size_t page_index, const char *object_json, size_t *output_id);
/* object_json: {"bookmark_name":"chapter","boundary":[x,y,width,height]}. */
int32_t ofd_add_bookmark_link_annotation_json(OfdDocument *handle, size_t page_index, const char *object_json, size_t *output_id);
int32_t ofd_add_attachment_link_annotation_json(OfdDocument *handle, size_t page_index, const char *object_json, size_t *output_id);
int32_t ofd_add_media_link_annotation_json(OfdDocument *handle, size_t page_index, const char *object_json, size_t *output_id);
int32_t ofd_add_text_page_with_font(OfdDocument *handle, const char *text, double width_mm, double height_mm, size_t font_id);
int32_t ofd_add_embedded_font(OfdDocument *handle, const char *name, const uint8_t *data, size_t len, size_t *output);
int32_t ofd_add_embedded_font_to_document(OfdDocument *handle, size_t document_index, const char *name, const uint8_t *data, size_t len, size_t *output);
int32_t ofd_add_embedded_font_to_page(OfdDocument *handle, size_t page_index, const char *name, const uint8_t *data, size_t len, size_t *output);
int32_t ofd_set_font_attribute(OfdDocument *handle, size_t font_index, const char *name, const char *value);
int32_t ofd_set_page_font_attribute(OfdDocument *handle, size_t page_index, size_t font_index, const char *name, const char *value);
int32_t ofd_replace_font_data(OfdDocument *handle, size_t font_index, const uint8_t *data, size_t len);
int32_t ofd_replace_page_font_data(OfdDocument *handle, size_t page_index, size_t font_index, const uint8_t *data, size_t len);
int32_t ofd_append_text_page(OfdDocument *handle, size_t document_index, const char *text, double width_mm, double height_mm);
int32_t ofd_append_blank_page(OfdDocument *handle, size_t document_index, double width_mm, double height_mm);
/* position is local to the document body; output is the global page index. */
int32_t ofd_insert_text_page(OfdDocument *handle, size_t document_index, size_t position, const char *text, double width_mm, double height_mm, size_t *output);
int32_t ofd_insert_blank_page(OfdDocument *handle, size_t document_index, size_t position, double width_mm, double height_mm, size_t *output);
/* flow_json: {"text":"...","page_size":[w,h],"margins":[top,right,bottom,left],"font_size":5,"line_height":6,"font_id":"1","align":"center","first_line_indent":5}; returns page indexes as JSON. */
/* flow_json accepts shape=true for embedded-font shaping before line breaks. */
int32_t ofd_add_flowed_text_json(OfdDocument *handle, size_t document_index, const char *flow_json, OfdBuffer *output);
/* table_json: rows of strings, column_widths, optional TableStyle fields. Returns new page indexes. */
int32_t ofd_add_table_json(OfdDocument *handle, size_t document_index, const char *table_json, OfdBuffer *output);
/* flow_json: {"spans":[{"text":"...","font_id":"1","font_size":5,"fill_color":[255,0,0],"bold":true,"italic":false,"letter_spacing":0.5,"underline":true,"underline_offset":1.2,"underline_width":0.2,"line_break_after":true,"keep_together":true}], plus the page style fields above}. */
int32_t ofd_add_flowed_spans_json(OfdDocument *handle, size_t document_index, const char *flow_json, OfdBuffer *output);
int32_t ofd_append_text_page_with_font(OfdDocument *handle, size_t document_index, const char *text, double width_mm, double height_mm, size_t font_id);
int32_t ofd_append_path_page(OfdDocument *handle, size_t document_index, const char *commands, double x, double y, double width, double height, double page_width, double page_height, int32_t fill, int32_t stroke);
int32_t ofd_append_image_page(OfdDocument *handle, size_t document_index, const uint8_t *data, size_t len, const char *format, double width_mm, double height_mm);
int32_t ofd_add_image_page(OfdDocument *handle, const uint8_t *data, size_t len, const char *format, double width_mm, double height_mm);
int32_t ofd_add_image_object(OfdDocument *handle, size_t page_index, const uint8_t *data, size_t len, const char *format, double x, double y, double width, double height, size_t *output_id);
int32_t ofd_add_composite_image_object(OfdDocument *handle, size_t page_index, const uint8_t *data, size_t len, const char *format, double x, double y, double width, double height, size_t *output_id);
int32_t ofd_add_path_page(OfdDocument *handle, const char *commands, double x, double y, double width, double height, double page_width, double page_height, int32_t fill, int32_t stroke);
int32_t ofd_add_attachment(OfdDocument *handle, const char *name, const uint8_t *data, size_t len);
int32_t ofd_add_multimedia_resource(OfdDocument *handle, size_t document_index, const char *media_type, const char *format, const uint8_t *data, size_t len, size_t *output_id);
int32_t ofd_add_page_multimedia_resource(OfdDocument *handle, size_t page_index, const char *media_type, const char *format, const uint8_t *data, size_t len, size_t *output_id);
int32_t ofd_replace_document_multimedia_data(OfdDocument *handle, size_t document_index, const char *resource_id, const uint8_t *data, size_t len);
int32_t ofd_replace_page_multimedia_data(OfdDocument *handle, size_t page_index, const char *resource_id, const uint8_t *data, size_t len);
int32_t ofd_remove_document_multimedia_resource(OfdDocument *handle, size_t document_index, const char *resource_id);
int32_t ofd_remove_page_multimedia_resource(OfdDocument *handle, size_t page_index, const char *resource_id);
int32_t ofd_add_attachment_to_document(OfdDocument *handle, size_t document_index, const char *name, const uint8_t *data, size_t len);
int32_t ofd_remove_attachment(OfdDocument *handle, size_t index);
int32_t ofd_rename_attachment(OfdDocument *handle, size_t index, const char *name);
int32_t ofd_replace_attachment_data(OfdDocument *handle, size_t index, const uint8_t *data, size_t len);
/* Replace optional attachment format, dates, visible, and usage from JSON. */
int32_t ofd_set_attachment_metadata_json(OfdDocument *handle, size_t index, const char *metadata_json);
int32_t ofd_save(const OfdDocument *handle, const char *path);
int32_t ofd_to_bytes(const OfdDocument *handle, OfdBuffer *output);
void ofd_buffer_free(OfdBuffer buffer);

#ifdef __cplusplus
}
#endif

#endif
