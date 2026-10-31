"""Python wrapper for the shared OFD C ABI."""

from __future__ import annotations

import base64
import json
import os
from pathlib import Path

from cffi import FFI

_ffi = FFI()
_ffi.cdef(
    """
    typedef struct OfdDocument OfdDocument;
    typedef struct { unsigned char *data; size_t len; } OfdBuffer;
    unsigned int ofd_abi_version(void);
    const char *ofd_last_error(void);
    int ofd_open(const char *path, OfdDocument **output);
    int ofd_from_bytes(const unsigned char *data, size_t len, OfdDocument **output);
    int ofd_new(OfdDocument **output);
    int ofd_is_encrypted(const OfdDocument *handle, int *output);
    int ofd_encryption_info_json(const OfdDocument *handle, OfdBuffer *output);
    int ofd_decrypt_with_password(const OfdDocument *handle, const char *password, const char *username, OfdDocument **output);
    int ofd_decrypt_with_sm2_private_key(const OfdDocument *handle, const unsigned char *private_key, size_t key_len, const char *username, OfdDocument **output);
    int ofd_encrypt_with_passwords_json(const OfdDocument *handle, const char *recipients_json, OfdDocument **output);
    int ofd_encrypt_with_certificates_json(const OfdDocument *handle, const char *recipients_json, OfdDocument **output);
    int ofd_encrypt_with_recipients_json(const OfdDocument *handle, const char *recipients_json, OfdDocument **output);
    int ofd_from_pdf_bytes(const unsigned char *data, size_t len, OfdDocument **output);
    void ofd_close(OfdDocument *handle);
    int ofd_page_count(const OfdDocument *handle, size_t *output);
    int ofd_append_document_body(OfdDocument *handle, size_t *output_index);
    int ofd_move_document_body(OfdDocument *handle, size_t from_index, size_t to_index);
    int ofd_remove_document_body(OfdDocument *handle, size_t index);
    int ofd_extract_document_body(const OfdDocument *handle, size_t index, OfdDocument **output);
    int ofd_append_document_bodies_from_json(OfdDocument *handle, const OfdDocument *source_handle, OfdBuffer *output);
    int ofd_append_document_body_from(OfdDocument *handle, const OfdDocument *source_handle, size_t source_index, size_t *output_index);
    int ofd_pages_json(const OfdDocument *handle, OfdBuffer *output);
    int ofd_set_page_area_json(OfdDocument *handle, size_t index, const char *area_json);
    int ofd_document_page_area_json(const OfdDocument *handle, size_t document_index, OfdBuffer *output);
    int ofd_set_document_page_area_json(OfdDocument *handle, size_t document_index, const char *area_json);
    int ofd_validate_references_json(const OfdDocument *handle, OfdBuffer *output);
    int ofd_archive_violations_json(const OfdDocument *handle, OfdBuffer *output);
    int ofd_prepare_archive(const OfdDocument *handle, OfdDocument **output);
    int ofd_entries_json(const OfdDocument *handle, OfdBuffer *output);
    int ofd_documents_json(const OfdDocument *handle, OfdBuffer *output);
    int ofd_package_info_json(const OfdDocument *handle, OfdBuffer *output);
    int ofd_set_package_header(OfdDocument *handle, const char *version, const char *doc_type);
    int ofd_document_permissions_json(const OfdDocument *handle, size_t document_index, OfdBuffer *output);
    int ofd_set_document_permissions_json(OfdDocument *handle, size_t document_index, const char *permissions_json);
    int ofd_view_preferences_json(const OfdDocument *handle, size_t document_index, OfdBuffer *output);
    int ofd_set_view_preferences_json(OfdDocument *handle, size_t document_index, const char *preferences_json);
    int ofd_document_actions_json(const OfdDocument *handle, size_t document_index, OfdBuffer *output);
    int ofd_add_document_action_json(OfdDocument *handle, size_t document_index, const char *action_json);
    int ofd_remove_document_action(OfdDocument *handle, size_t document_index, size_t action_index);
    int ofd_page_actions_json(const OfdDocument *handle, size_t page_index, OfdBuffer *output);
    int ofd_add_page_action_json(OfdDocument *handle, size_t page_index, const char *action_json);
    int ofd_remove_page_action(OfdDocument *handle, size_t page_index, size_t action_index);
    int ofd_integrity_manifest_json(const OfdDocument *handle, OfdBuffer *output);
    int ofd_check_integrity_manifest_json(const OfdDocument *handle, OfdBuffer *output);
    int ofd_verify_integrity_manifest_sm2_json(const OfdDocument *handle, const char *signature_distid, OfdBuffer *output);
    int ofd_protect_integrity_sm2(OfdDocument *handle, const uint8_t *private_key, size_t private_key_len, const uint8_t *certificate_der, size_t certificate_len, const char *signature_distid);
    int ofd_verify_integrity_manifest_sm2_with_ordered_chain_json(const OfdDocument *handle, const char *options_json, OfdBuffer *output);
    int ofd_verify_integrity_manifest_sm2_with_certificate_pool_json(const OfdDocument *handle, const char *options_json, OfdBuffer *output);
    int ofd_versions_json(const OfdDocument *handle, OfdBuffer *output);
    int ofd_extract_version_snapshot(const OfdDocument *handle, size_t index, OfdDocument **output);
    int ofd_add_version_snapshot_json(OfdDocument *handle, size_t document_index, const char *snapshot_json, OfdBuffer *output);
    int ofd_bookmarks_json(const OfdDocument *handle, OfdBuffer *output);
    int ofd_add_bookmark_json(OfdDocument *handle, size_t document_index, const char *bookmark_json);
    int ofd_remove_bookmark(OfdDocument *handle, size_t document_index, const char *name);
    int ofd_add_outline_json(OfdDocument *handle, size_t document_index, const char *outline_json, size_t *output_index);
    int ofd_remove_outline(OfdDocument *handle, size_t index);
    int ofd_set_outline_title(OfdDocument *handle, size_t index, const char *title);
    int ofd_set_outline_expanded(OfdDocument *handle, size_t index, int expanded);
    int ofd_set_outline_destination_json(OfdDocument *handle, size_t index, const char *destination_json);
    int ofd_set_outline_bookmark(OfdDocument *handle, size_t index, const char *name);
    int ofd_outlines_json(const OfdDocument *handle, OfdBuffer *output);
    int ofd_set_document_info_field(OfdDocument *handle, size_t document_index, const char *field, const char *value);
    int ofd_remove_document_info_field(OfdDocument *handle, size_t document_index, const char *field);
    int ofd_add_document_keyword(OfdDocument *handle, size_t document_index, const char *keyword);
    int ofd_remove_document_keyword(OfdDocument *handle, size_t document_index, size_t keyword_index);
    int ofd_set_document_custom_data(OfdDocument *handle, size_t document_index, const char *name, const char *value);
    int ofd_remove_document_custom_data(OfdDocument *handle, size_t document_index, const char *name);
    int ofd_page_text_json(const OfdDocument *handle, size_t index, OfdBuffer *output);
    int ofd_all_text_json(const OfdDocument *handle, OfdBuffer *output);
    int ofd_page_text_in_region_json(const OfdDocument *handle, size_t index, double x, double y, double width, double height, OfdBuffer *output);
    int ofd_keyword_positions_json(const OfdDocument *handle, const char *query_json, OfdBuffer *output);
    int ofd_page_sources_json(const OfdDocument *handle, size_t index, OfdBuffer *output);
    int ofd_page_layers_json(const OfdDocument *handle, size_t index, OfdBuffer *output);
    int ofd_promote_page_to_template(OfdDocument *handle, size_t source_page_index, const char *additional_pages_json, const char *z_order, size_t *output_id);
    int ofd_apply_page_template(OfdDocument *handle, size_t page_index, const char *template_id, const char *z_order);
    int ofd_remove_page_template(OfdDocument *handle, size_t page_index, const char *template_id);
    int ofd_remove_document_template(OfdDocument *handle, size_t document_index, const char *template_id);
    int ofd_page_objects_json(const OfdDocument *handle, size_t index, OfdBuffer *output);
    int ofd_page_object_clips_json(const OfdDocument *handle, size_t index, OfdBuffer *output);
    int ofd_page_object_paints_json(const OfdDocument *handle, size_t index, OfdBuffer *output);
    int ofd_page_object_stroke_styles_json(const OfdDocument *handle, size_t index, OfdBuffer *output);
    int ofd_page_object_actions_json(const OfdDocument *handle, size_t page_index, const char *object_id, OfdBuffer *output);
    int ofd_page_source_object_actions_json(const OfdDocument *handle, size_t page_index, const char *source_path, const char *object_id, OfdBuffer *output);
    int ofd_add_page_object_uri_action(OfdDocument *handle, size_t page_index, const char *object_id, const char *uri);
    int ofd_add_page_object_goto_action_json(OfdDocument *handle, size_t page_index, const char *object_id, const char *destination_json);
    int ofd_add_page_object_bookmark_action(OfdDocument *handle, size_t page_index, const char *object_id, const char *bookmark_name);
    int ofd_add_page_object_attachment_action(OfdDocument *handle, size_t page_index, const char *object_id, const char *attachment_id, int new_window);
    int ofd_add_page_object_media_action_json(OfdDocument *handle, size_t page_index, const char *object_id, const char *action_json);
    int ofd_remove_page_object_action(OfdDocument *handle, size_t page_index, const char *object_id, size_t action_index);
    int ofd_set_page_object_action_regions_json(OfdDocument *handle, size_t page_index, const char *object_id, size_t action_index, const char *rectangles_json);
    int ofd_page_text_with_templates_json(const OfdDocument *handle, size_t index, OfdBuffer *output);
    int ofd_page_text_objects_json(const OfdDocument *handle, size_t index, OfdBuffer *output);
    int ofd_page_paths_json(const OfdDocument *handle, size_t index, OfdBuffer *output);
    int ofd_page_images_json(const OfdDocument *handle, size_t index, OfdBuffer *output);
    int ofd_page_multimedia_resources_json(const OfdDocument *handle, size_t index, OfdBuffer *output);
    int ofd_page_multimedia_data(const OfdDocument *handle, size_t page_index, const char *resource_id, OfdBuffer *output);
    int ofd_document_multimedia_resources_json(const OfdDocument *handle, size_t index, OfdBuffer *output);
    int ofd_document_multimedia_data(const OfdDocument *handle, size_t document_index, const char *resource_id, OfdBuffer *output);
    int ofd_page_composite_images_json(const OfdDocument *handle, size_t index, OfdBuffer *output);
    int ofd_page_composite_objects_json(const OfdDocument *handle, size_t index, OfdBuffer *output);
    int ofd_page_composite_units_json(const OfdDocument *handle, size_t index, OfdBuffer *output);
    int ofd_set_page_composite_unit_images(OfdDocument *handle, size_t page_index, const char *unit_id, const char *thumbnail_id, const char *substitution_id);
    int ofd_page_image_data(const OfdDocument *handle, size_t page_index, size_t image_index, OfdBuffer *output);
    int ofd_page_composite_image_data(const OfdDocument *handle, size_t page_index, size_t image_index, OfdBuffer *output);
    int ofd_page_svg(const OfdDocument *handle, size_t index, OfdBuffer *output);
    int ofd_page_png(const OfdDocument *handle, size_t index, double dpi, OfdBuffer *output);
    int ofd_to_pdf(const OfdDocument *handle, OfdBuffer *output);
    int ofd_page_svg_with_options(const OfdDocument *handle, size_t index, int prefer_image_substitution, OfdBuffer *output);
    int ofd_page_png_with_options(const OfdDocument *handle, size_t index, double dpi, int prefer_image_substitution, OfdBuffer *output);
    int ofd_to_pdf_with_options(const OfdDocument *handle, int prefer_image_substitution, OfdBuffer *output);
    int ofd_attachments_json(const OfdDocument *handle, OfdBuffer *output);
    int ofd_annotations_json(const OfdDocument *handle, OfdBuffer *output);
    int ofd_annotation_object_actions_json(const OfdDocument *handle, size_t page_index, const char *annotation_id, const char *object_id, OfdBuffer *output);
    int ofd_add_annotation_object_action_json(OfdDocument *handle, size_t page_index, const char *annotation_id, const char *object_id, const char *action_json);
    int ofd_remove_annotation_object_action(OfdDocument *handle, size_t page_index, const char *annotation_id, const char *object_id, size_t action_index);
    int ofd_set_annotation_object_action_regions_json(OfdDocument *handle, size_t page_index, const char *annotation_id, const char *object_id, size_t action_index, const char *rectangles_json);
    int ofd_annotation_objects_json(const OfdDocument *handle, size_t page_index, const char *annotation_id, OfdBuffer *output);
    int ofd_append_annotation_objects_json(OfdDocument *handle, size_t page_index, const char *annotation_id, const char *object_json, OfdBuffer *output);
    int ofd_update_annotation_object_json(OfdDocument *handle, size_t page_index, const char *annotation_id, const char *object_id, const char *update_json);
    int ofd_set_annotation_object_name(OfdDocument *handle, size_t page_index, const char *annotation_id, const char *object_id, const char *name);
    int ofd_set_annotation_object_blend_mode(OfdDocument *handle, size_t page_index, const char *annotation_id, const char *object_id, const char *blend_mode);
    int ofd_set_annotation_object_draw_param(OfdDocument *handle, size_t page_index, const char *annotation_id, const char *object_id, const char *draw_param);
    int ofd_remove_annotation_object(OfdDocument *handle, size_t page_index, const char *annotation_id, const char *object_id);
    int ofd_set_annotation_path_data(OfdDocument *handle, size_t page_index, const char *annotation_id, const char *object_id, const char *commands);
    int ofd_set_annotation_path_stroke_style_json(OfdDocument *handle, size_t page_index, const char *annotation_id, const char *object_id, const char *style_json);
    int ofd_set_annotation_text_code(OfdDocument *handle, size_t page_index, const char *annotation_id, const char *object_id, size_t code_index, const char *text);
    int ofd_edit_annotation_text_code_json(OfdDocument *handle, const char *request_json);
    int ofd_set_annotation_text_code_layout_json(OfdDocument *handle, size_t page_index, const char *annotation_id, const char *object_id, size_t code_index, const char *layout_json);
    int ofd_set_annotation_text_typography_json(OfdDocument *handle, size_t page_index, const char *annotation_id, const char *object_id, const char *style_json);
    int ofd_set_annotation_text_paint_flags_json(OfdDocument *handle, size_t page_index, const char *annotation_id, const char *object_id, const char *flags_json);
    int ofd_append_annotation_text_code_json(OfdDocument *handle, size_t page_index, const char *annotation_id, const char *object_id, const char *text, const char *layout_json);
    int ofd_remove_annotation_text_code(OfdDocument *handle, size_t page_index, const char *annotation_id, const char *object_id, size_t code_index);
    int ofd_set_annotation_text_font(OfdDocument *handle, size_t page_index, const char *annotation_id, const char *object_id, const char *font_id);
    int ofd_set_annotation_graphic_resource(OfdDocument *handle, size_t page_index, const char *annotation_id, const char *object_id, const char *resource_id);
    int ofd_remove_annotation(OfdDocument *handle, size_t page_index, const char *annotation_id);
    int ofd_set_annotation_creator(OfdDocument *handle, size_t page_index, const char *annotation_id, const char *creator);
    int ofd_set_annotation_subtype(OfdDocument *handle, size_t page_index, const char *annotation_id, const char *subtype);
    int ofd_set_annotation_last_mod_date(OfdDocument *handle, size_t page_index, const char *annotation_id, const char *date);
    int ofd_set_annotation_appearance_boundary(OfdDocument *handle, size_t page_index, const char *annotation_id, double x, double y, double width, double height);
    int ofd_set_annotation_remark(OfdDocument *handle, size_t page_index, const char *annotation_id, const char *remark);
    int ofd_set_annotation_parameter(OfdDocument *handle, size_t page_index, const char *annotation_id, const char *name, const char *value);
    int ofd_remove_annotation_parameter(OfdDocument *handle, size_t page_index, const char *annotation_id, const char *name);
    int ofd_set_annotation_visible(OfdDocument *handle, size_t page_index, const char *annotation_id, int visible);
    int ofd_set_annotation_print(OfdDocument *handle, size_t page_index, const char *annotation_id, int print);
    int ofd_set_annotation_flag(OfdDocument *handle, size_t page_index, const char *annotation_id, const char *flag, int value);
    int ofd_fonts_json(const OfdDocument *handle, OfdBuffer *output);
    int ofd_font_data(const OfdDocument *handle, size_t index, OfdBuffer *output);
    int ofd_page_fonts_json(const OfdDocument *handle, size_t page_index, OfdBuffer *output);
    int ofd_page_font_data(const OfdDocument *handle, size_t page_index, size_t font_index, OfdBuffer *output);
    int ofd_signatures_json(const OfdDocument *handle, OfdBuffer *output);
    int ofd_signed_value_data(const OfdDocument *handle, size_t index, OfdBuffer *output);
    int ofd_seal_data(const OfdDocument *handle, size_t index, OfdBuffer *output);
    int ofd_check_signature_references_json(const OfdDocument *handle, size_t index, OfdBuffer *output);
    int ofd_check_signature_description_hash_json(const OfdDocument *handle, size_t index, OfdBuffer *output);
    int ofd_check_signature_seal_match(const OfdDocument *handle, size_t index, int *output);
    int ofd_check_ses_seal_signature(const OfdDocument *handle, size_t index, int *output);
    int ofd_check_ses_seal_signature_with_distid(const OfdDocument *handle, size_t index, const char *seal_distid, int *output);
    int ofd_check_ses_seal_certificate_validity(const OfdDocument *handle, size_t index, unsigned long long at_unix_seconds, int *output);
    int ofd_check_ses_seal_signer_authorization(const OfdDocument *handle, size_t index, int *output);
    int ofd_check_ses_seal_validity(const OfdDocument *handle, size_t index, unsigned long long at_unix_seconds, int *output);
    int ofd_check_ses_seal_issuer_ordered_sm2_chain(const OfdDocument *handle, size_t index, const unsigned char *intermediates_json, size_t intermediates_json_len, const unsigned char *anchor_certificate, size_t anchor_len, unsigned long long at_unix_seconds, const char *certificate_distid, int *output);
    int ofd_check_ses_signature_value(const OfdDocument *handle, size_t index, int *output);
    int ofd_check_ses_signature_value_with_distid(const OfdDocument *handle, size_t index, const char *signature_distid, int *output);
    int ofd_check_digital_signature_with_certificate(const OfdDocument *handle, size_t index, const unsigned char *certificate, size_t len, int *output);
    int ofd_check_digital_signature_with_certificate_and_distid(const OfdDocument *handle, size_t index, const unsigned char *certificate, size_t len, const char *signature_distid, int *output);
    int ofd_check_signature_certificate_validity(const OfdDocument *handle, size_t index, const unsigned char *certificate, size_t len, unsigned long long at_unix_seconds, int *output);
    int ofd_check_signature_direct_trust_anchor(const OfdDocument *handle, size_t index, const unsigned char *signer_certificate, size_t signer_len, const unsigned char *anchor_certificate, size_t anchor_len, unsigned long long at_unix_seconds, const char *certificate_distid, int *output);
    int ofd_verify_signature_with_direct_trust_anchor_json(const OfdDocument *handle, size_t index, const unsigned char *signer_certificate, size_t signer_len, const unsigned char *anchor_certificate, size_t anchor_len, unsigned long long at_unix_seconds, const char *certificate_distid, OfdBuffer *output);
    int ofd_verify_signature_with_direct_trust_anchor_and_distids_json(const OfdDocument *handle, size_t index, const unsigned char *signer_certificate, size_t signer_len, const unsigned char *anchor_certificate, size_t anchor_len, unsigned long long at_unix_seconds, const char *certificate_distid, const char *signature_distid, OfdBuffer *output);
    int ofd_verify_signature_with_ordered_sm2_chain_json(const OfdDocument *handle, size_t index, const unsigned char *signer_certificate, size_t signer_len, const unsigned char *intermediates_json, size_t intermediates_json_len, const unsigned char *anchor_certificate, size_t anchor_len, unsigned long long at_unix_seconds, const char *certificate_distid, OfdBuffer *output);
    int ofd_verify_signature_with_ordered_sm2_chain_and_distids_json(const OfdDocument *handle, size_t index, const unsigned char *signer_certificate, size_t signer_len, const unsigned char *intermediates_json, size_t intermediates_json_len, const unsigned char *anchor_certificate, size_t anchor_len, unsigned long long at_unix_seconds, const char *certificate_distid, const char *signature_distid, OfdBuffer *output);
    int ofd_verify_signature_with_certificate_pool_json(const OfdDocument *handle, size_t index, const unsigned char *signer_certificate, size_t signer_len, const unsigned char *candidates_json, size_t candidates_json_len, const unsigned char *anchor_certificate, size_t anchor_len, unsigned long long at_unix_seconds, const char *certificate_distid, const char *signature_distid, OfdBuffer *output);
    int ofd_verify_signature_with_ordered_sm2_chains_json(const OfdDocument *handle, size_t index, const unsigned char *options_json, size_t options_json_len, OfdBuffer *output);
    int ofd_verify_signature_with_certificate_pools_json(const OfdDocument *handle, size_t index, const unsigned char *options_json, size_t options_json_len, OfdBuffer *output);
    int ofd_add_digital_signature(OfdDocument *handle, const unsigned char *private_key, size_t len, const char *signature_datetime, size_t *output_index);
    int ofd_add_digital_signature_with_distid(OfdDocument *handle, const unsigned char *private_key, size_t len, const char *signature_datetime, const char *signature_distid, size_t *output_index);
    int ofd_add_ses_signature(OfdDocument *handle, const unsigned char *private_key, size_t private_key_len, const unsigned char *signer_certificate, size_t signer_certificate_len, const unsigned char *seal, size_t seal_len, const char *signature_datetime, size_t *output_index);
    int ofd_add_ses_signature_with_distids(OfdDocument *handle, const unsigned char *private_key, size_t private_key_len, const unsigned char *signer_certificate, size_t signer_certificate_len, const unsigned char *seal, size_t seal_len, const char *signature_datetime, const char *signature_distid, const char *seal_distid, size_t *output_index);
    int ofd_add_ses_signature_with_stamp_and_distids(OfdDocument *handle, const unsigned char *private_key, size_t private_key_len, const unsigned char *signer_certificate, size_t signer_certificate_len, const unsigned char *seal, size_t seal_len, const char *signature_datetime, size_t page_index, double x_mm, double y_mm, double width_mm, double height_mm, const char *signature_distid, const char *seal_distid, size_t *output_index);
    int ofd_add_ses_signature_with_stamp_json(OfdDocument *handle, const unsigned char *private_key, size_t private_key_len, const unsigned char *signer_certificate, size_t signer_certificate_len, const unsigned char *seal, size_t seal_len, const char *options_json, size_t *output_index);
    int ofd_create_ses_seal_json(const unsigned char *issuer_private_key, size_t issuer_private_key_len, const unsigned char *issuer_certificate, size_t issuer_certificate_len, const unsigned char *picture, size_t picture_len, const unsigned char *options_json, size_t options_json_len, const char *issuer_distid, OfdBuffer *output);
    int ofd_attachment_data(const OfdDocument *handle, size_t index, OfdBuffer *output);
    int ofd_custom_tags_json(const OfdDocument *handle, OfdBuffer *output);
    int ofd_custom_tag_data(const OfdDocument *handle, size_t index, OfdBuffer *output);
    int ofd_replace_custom_tag_data(OfdDocument *handle, size_t index, const unsigned char *data, size_t len, OfdBuffer *output);
    int ofd_add_custom_tag_json(OfdDocument *handle, size_t document_index, const char *metadata_json, const unsigned char *data, size_t len, OfdBuffer *output);
    int ofd_update_custom_tag_json(OfdDocument *handle, size_t index, const char *metadata_json, OfdBuffer *output);
    int ofd_remove_custom_tag(OfdDocument *handle, size_t index);
    int ofd_extensions_json(const OfdDocument *handle, OfdBuffer *output);
    int ofd_add_extension_json(OfdDocument *handle, const char *extension_json);
    int ofd_update_extension_json(OfdDocument *handle, size_t index, const char *extension_json);
    int ofd_remove_extension(OfdDocument *handle, size_t index);
    int ofd_get_entry(const OfdDocument *handle, const char *path, OfdBuffer *output);
    int ofd_set_entry(OfdDocument *handle, const char *path, const unsigned char *data, size_t len);
    int ofd_remove_entry(OfdDocument *handle, const char *path, int *removed);
    int ofd_add_text_page(OfdDocument *handle, const char *text, double width_mm, double height_mm);
    int ofd_write_text_page_json(OfdDocument *handle, const char *request_json, size_t *output);
    int ofd_add_blank_page(OfdDocument *handle, double width_mm, double height_mm);
    int ofd_add_text_object_json(OfdDocument *handle, size_t page_index, const char *object_json, size_t *output_id);
    int ofd_add_composite_text_object_json(OfdDocument *handle, size_t page_index, const char *object_json, size_t *output_id);
    int ofd_remove_page_object(OfdDocument *handle, size_t page_index, const char *object_id);
    int ofd_duplicate_page_object(OfdDocument *handle, size_t page_index, const char *object_id, size_t *output_id);
    int ofd_add_page_layer(OfdDocument *handle, size_t page_index, const char *layer_type, const char *draw_param, size_t *output_id);
    int ofd_set_page_layer_type(OfdDocument *handle, size_t page_index, const char *layer_id, const char *layer_type);
    int ofd_set_page_layer_name(OfdDocument *handle, size_t page_index, const char *layer_id, const char *name);
    int ofd_set_page_layer_draw_param(OfdDocument *handle, size_t page_index, const char *layer_id, const char *draw_param);
    int ofd_add_document_draw_param_json(OfdDocument *handle, size_t document_index, const char *style_json, size_t *output_id);
    int ofd_add_page_draw_param_json(OfdDocument *handle, size_t page_index, const char *style_json, size_t *output_id);
    int ofd_set_document_draw_param_json(OfdDocument *handle, size_t document_index, const char *id, const char *style_json);
    int ofd_set_page_draw_param_json(OfdDocument *handle, size_t page_index, const char *id, const char *style_json);
    int ofd_remove_document_draw_param(OfdDocument *handle, size_t document_index, const char *id);
    int ofd_remove_page_draw_param(OfdDocument *handle, size_t page_index, const char *id);
    int ofd_page_color_spaces_json(const OfdDocument *handle, size_t page_index, OfdBuffer *output);
    int ofd_document_color_spaces_json(const OfdDocument *handle, size_t document_index, OfdBuffer *output);
    int ofd_document_draw_params_json(const OfdDocument *handle, size_t document_index, OfdBuffer *output);
    int ofd_page_draw_params_json(const OfdDocument *handle, size_t page_index, OfdBuffer *output);
    int ofd_add_document_color_space_json(OfdDocument *handle, size_t document_index, const char *spec_json, size_t *output_id);
    int ofd_add_page_color_space_json(OfdDocument *handle, size_t page_index, const char *spec_json, size_t *output_id);
    int ofd_set_document_color_space_json(OfdDocument *handle, size_t document_index, const char *id, const char *spec_json);
    int ofd_set_page_color_space_json(OfdDocument *handle, size_t page_index, const char *id, const char *spec_json);
    int ofd_remove_document_color_space(OfdDocument *handle, size_t document_index, const char *id);
    int ofd_remove_page_color_space(OfdDocument *handle, size_t page_index, const char *id);
    int ofd_document_default_color_space_json(const OfdDocument *handle, size_t document_index, OfdBuffer *output);
    int ofd_set_document_default_color_space(OfdDocument *handle, size_t document_index, const char *color_space_id);
    int ofd_set_page_object_draw_param(OfdDocument *handle, size_t page_index, const char *object_id, const char *draw_param);
    int ofd_move_page_object_to_layer(OfdDocument *handle, size_t page_index, const char *object_id, const char *layer_id);
    int ofd_group_page_objects(OfdDocument *handle, size_t page_index, const char *first_id, const char *last_id);
    int ofd_ungroup_page_objects(OfdDocument *handle, size_t page_index, const char *object_id);
    int ofd_move_page_object(OfdDocument *handle, size_t page_index, const char *object_id, const char *before_id);
    int ofd_set_page_object_name(OfdDocument *handle, size_t page_index, const char *object_id, const char *name);
    int ofd_set_page_object_blend_mode(OfdDocument *handle, size_t page_index, const char *object_id, const char *blend_mode);
    int ofd_set_page_object_clip_rects_json(OfdDocument *handle, size_t page_index, const char *object_id, const char *clips_json);
    int ofd_set_page_object_path_clips_json(OfdDocument *handle, size_t page_index, const char *object_id, const char *clips_json);
    int ofd_set_page_object_clips_json(OfdDocument *handle, size_t page_index, const char *object_id, const char *clips_json);
    int ofd_set_page_object_visible(OfdDocument *handle, size_t page_index, const char *object_id, int visible);
    int ofd_set_page_object_alpha(OfdDocument *handle, size_t page_index, const char *object_id, int alpha);
    int ofd_set_page_path_stroke_style_json(OfdDocument *handle, size_t page_index, const char *object_id, const char *style_json);
    int ofd_set_page_path_dash_sequence_json(OfdDocument *handle, size_t page_index, const char *object_id, const char *style_json);
    int ofd_set_page_path_data(OfdDocument *handle, size_t page_index, const char *object_id, const char *commands);
    int ofd_set_page_text_typography_json(OfdDocument *handle, size_t page_index, const char *object_id, const char *style_json);
    int ofd_set_page_text_code(OfdDocument *handle, size_t page_index, const char *object_id, size_t code_index, const char *text);
    int ofd_edit_text_code_json(OfdDocument *handle, const char *request_json);
    int ofd_set_page_text_code_layout_json(OfdDocument *handle, size_t page_index, const char *object_id, size_t code_index, const char *layout_json);
    int ofd_append_page_text_code_json(OfdDocument *handle, size_t page_index, const char *object_id, const char *text, const char *layout_json);
    int ofd_remove_page_text_code(OfdDocument *handle, size_t page_index, const char *object_id, size_t code_index);
    int ofd_set_page_text_font(OfdDocument *handle, size_t page_index, const char *object_id, const char *font_id);
    int ofd_set_page_text_paint_flags_json(OfdDocument *handle, size_t page_index, const char *object_id, const char *flags_json);
    int ofd_set_page_image_interpolate(OfdDocument *handle, size_t page_index, const char *object_id, int interpolate);
    int ofd_set_page_image_mask(OfdDocument *handle, size_t page_index, const char *object_id, const char *mask_resource_id);
    int ofd_set_page_image_substitution(OfdDocument *handle, size_t page_index, const char *object_id, const char *substitution_resource_id);
    int ofd_set_page_image_border_json(OfdDocument *handle, size_t page_index, const char *object_id, const char *style_json);
    int ofd_set_page_object_gouraud_fill_json(OfdDocument *handle, size_t page_index, const char *object_id, const char *style_json);
    int ofd_set_page_object_gouraud_stroke_json(OfdDocument *handle, size_t page_index, const char *object_id, const char *style_json);
    int ofd_set_page_object_gradient_fill_json(OfdDocument *handle, size_t page_index, const char *object_id, const char *style_json);
    int ofd_set_page_object_gradient_stroke_json(OfdDocument *handle, size_t page_index, const char *object_id, const char *style_json);
    int ofd_set_page_object_pattern_fill_json(OfdDocument *handle, size_t page_index, const char *object_id, const char *style_json);
    int ofd_set_page_object_pattern_stroke_json(OfdDocument *handle, size_t page_index, const char *object_id, const char *style_json);
    int ofd_set_page_object_solid_fill_json(OfdDocument *handle, size_t page_index, const char *object_id, const char *style_json);
    int ofd_set_page_object_solid_stroke_json(OfdDocument *handle, size_t page_index, const char *object_id, const char *style_json);
    int ofd_set_page_object_boundary(OfdDocument *handle, size_t page_index, const char *object_id, double x, double y, double width, double height);
    int ofd_set_page_object_ctm_json(OfdDocument *handle, size_t page_index, const char *object_id, const char *ctm_json);
    int ofd_remove_page_layer(OfdDocument *handle, size_t page_index, const char *layer_id);
    int ofd_move_page(OfdDocument *handle, size_t from_index, size_t to_index);
    int ofd_reorder_document_pages_json(OfdDocument *handle, size_t document_index, const char *order_json);
    int ofd_duplicate_page(OfdDocument *handle, size_t index, size_t *output);
    int ofd_duplicate_page_at(OfdDocument *handle, size_t index, size_t position, size_t *output);
    int ofd_remove_page(OfdDocument *handle, size_t index);
    int ofd_add_path_object_json(OfdDocument *handle, size_t page_index, const char *object_json, size_t *output_id);
    int ofd_add_composite_path_object_json(OfdDocument *handle, size_t page_index, const char *object_json, size_t *output_id);
    int ofd_add_composite_object_json(OfdDocument *handle, size_t page_index, const char *object_json, size_t *output_id);
    int ofd_add_text_watermark_json(OfdDocument *handle, size_t page_index, const char *object_json, size_t *output_id);
    int ofd_add_text_stamp_annotation_json(OfdDocument *handle, size_t page_index, const char *object_json, size_t *output_id);
    int ofd_add_text_watermark_all_pages_json(OfdDocument *handle, const char *object_json, OfdBuffer *output);
    int ofd_add_image_watermark_json(OfdDocument *handle, size_t page_index, const char *object_json, size_t *output_id);
    int ofd_add_image_stamp_annotation_json(OfdDocument *handle, size_t page_index, const char *object_json, size_t *output_id);
    int ofd_add_image_watermark_all_pages_json(OfdDocument *handle, const char *object_json, OfdBuffer *output);
    int ofd_add_path_annotation_json(OfdDocument *handle, size_t page_index, const char *object_json, size_t *output_id);
    int ofd_add_multi_path_annotation_json(OfdDocument *handle, size_t page_index, const char *object_json, size_t *output_id);
    int ofd_add_mixed_annotation_json(OfdDocument *handle, size_t page_index, const char *object_json, size_t *output_id);
    int ofd_add_highlight_annotation_json(OfdDocument *handle, size_t page_index, const char *object_json, size_t *output_id);
    int ofd_add_text_note_annotation_json(OfdDocument *handle, size_t page_index, const char *object_json, size_t *output_id);
    int ofd_add_shape_annotation_json(OfdDocument *handle, size_t page_index, const char *object_json, size_t *output_id);
    int ofd_add_line_annotation_json(OfdDocument *handle, size_t page_index, const char *object_json, size_t *output_id);
    int ofd_add_uri_link_annotation_json(OfdDocument *handle, size_t page_index, const char *object_json, size_t *output_id);
    int ofd_add_goto_link_annotation_json(OfdDocument *handle, size_t page_index, const char *object_json, size_t *output_id);
    int ofd_add_bookmark_link_annotation_json(OfdDocument *handle, size_t page_index, const char *object_json, size_t *output_id);
    int ofd_add_attachment_link_annotation_json(OfdDocument *handle, size_t page_index, const char *object_json, size_t *output_id);
    int ofd_add_media_link_annotation_json(OfdDocument *handle, size_t page_index, const char *object_json, size_t *output_id);
    int ofd_add_text_page_with_font(OfdDocument *handle, const char *text, double width_mm, double height_mm, size_t font_id);
    int ofd_add_embedded_font(OfdDocument *handle, const char *name, const unsigned char *data, size_t len, size_t *output);
    int ofd_add_embedded_font_to_document(OfdDocument *handle, size_t document_index, const char *name, const unsigned char *data, size_t len, size_t *output);
    int ofd_add_embedded_font_to_page(OfdDocument *handle, size_t page_index, const char *name, const unsigned char *data, size_t len, size_t *output);
    int ofd_set_font_attribute(OfdDocument *handle, size_t font_index, const char *name, const char *value);
    int ofd_set_page_font_attribute(OfdDocument *handle, size_t page_index, size_t font_index, const char *name, const char *value);
    int ofd_replace_font_data(OfdDocument *handle, size_t font_index, const uint8_t *data, size_t len);
    int ofd_replace_page_font_data(OfdDocument *handle, size_t page_index, size_t font_index, const uint8_t *data, size_t len);
    int ofd_append_text_page(OfdDocument *handle, size_t document_index, const char *text, double width_mm, double height_mm);
    int ofd_append_blank_page(OfdDocument *handle, size_t document_index, double width_mm, double height_mm);
    int ofd_insert_text_page(OfdDocument *handle, size_t document_index, size_t position, const char *text, double width_mm, double height_mm, size_t *output);
    int ofd_insert_blank_page(OfdDocument *handle, size_t document_index, size_t position, double width_mm, double height_mm, size_t *output);
    int ofd_add_flowed_text_json(OfdDocument *handle, size_t document_index, const char *flow_json, OfdBuffer *output);
    int ofd_add_table_json(OfdDocument *handle, size_t document_index, const char *table_json, OfdBuffer *output);
    int ofd_add_flowed_spans_json(OfdDocument *handle, size_t document_index, const char *flow_json, OfdBuffer *output);
    int ofd_append_text_page_with_font(OfdDocument *handle, size_t document_index, const char *text, double width_mm, double height_mm, size_t font_id);
    int ofd_append_path_page(OfdDocument *handle, size_t document_index, const char *commands, double x, double y, double width, double height, double page_width, double page_height, int fill, int stroke);
    int ofd_append_image_page(OfdDocument *handle, size_t document_index, const unsigned char *data, size_t len, const char *format, double width_mm, double height_mm);
    int ofd_add_image_page(OfdDocument *handle, const unsigned char *data, size_t len, const char *format, double width_mm, double height_mm);
    int ofd_add_image_object(OfdDocument *handle, size_t page_index, const unsigned char *data, size_t len, const char *format, double x, double y, double width, double height, size_t *output_id);
    int ofd_add_composite_image_object(OfdDocument *handle, size_t page_index, const unsigned char *data, size_t len, const char *format, double x, double y, double width, double height, size_t *output_id);
    int ofd_add_path_page(OfdDocument *handle, const char *commands, double x, double y, double width, double height, double page_width, double page_height, int fill, int stroke);
    int ofd_add_attachment(OfdDocument *handle, const char *name, const unsigned char *data, size_t len);
    int ofd_add_multimedia_resource(OfdDocument *handle, size_t document_index, const char *media_type, const char *format, const unsigned char *data, size_t len, size_t *output_id);
    int ofd_add_page_multimedia_resource(OfdDocument *handle, size_t page_index, const char *media_type, const char *format, const unsigned char *data, size_t len, size_t *output_id);
    int ofd_replace_document_multimedia_data(OfdDocument *handle, size_t document_index, const char *resource_id, const unsigned char *data, size_t len);
    int ofd_replace_page_multimedia_data(OfdDocument *handle, size_t page_index, const char *resource_id, const unsigned char *data, size_t len);
    int ofd_remove_document_multimedia_resource(OfdDocument *handle, size_t document_index, const char *resource_id);
    int ofd_remove_page_multimedia_resource(OfdDocument *handle, size_t page_index, const char *resource_id);
    int ofd_add_attachment_to_document(OfdDocument *handle, size_t document_index, const char *name, const unsigned char *data, size_t len);
    int ofd_remove_attachment(OfdDocument *handle, size_t index);
    int ofd_rename_attachment(OfdDocument *handle, size_t index, const char *name);
    int ofd_replace_attachment_data(OfdDocument *handle, size_t index, const unsigned char *data, size_t len);
    int ofd_set_attachment_metadata_json(OfdDocument *handle, size_t index, const char *metadata_json);
    int ofd_save(const OfdDocument *handle, const char *path);
    int ofd_to_bytes(const OfdDocument *handle, OfdBuffer *output);
    void ofd_buffer_free(OfdBuffer buffer);
    void ofd_buffer_free_parts(uint8_t *data, size_t len);
    """
)


class OfdError(Exception):
    """An OFD operation failed."""


def _library():
    names = {
        "nt": "ofdrw_ffi.dll",
        "posix": "libofdrw_ffi.so",
    }
    name = names[os.name]
    if os.sys.platform == "darwin":
        name = "libofdrw_ffi.dylib"
    override = os.environ.get("OFDRW_LIB_PATH")
    candidates = [
        str(Path(__file__).parent / name),
        str(Path(__file__).resolve().parents[3] / "target" / "debug" / name),
        name,
    ]
    if override:
        candidates = [override]
    for candidate in candidates:
        try:
            lib = _ffi.dlopen(candidate)
            if lib.ofd_abi_version() != 1:
                raise OfdError("incompatible OFD C ABI version")
            return lib
        except OSError as error:
            if override:
                raise OfdError(f"cannot load OFDRW_LIB_PATH: {override}") from error
            continue
    raise OfdError("OFD native library not found; set OFDRW_LIB_PATH")


_lib = _library()


def _check(status: int) -> None:
    if status != 0:
        raise OfdError(_ffi.string(_lib.ofd_last_error()).decode("utf-8", "replace"))


def _utf8(value: str | os.PathLike[str]) -> bytes:
    encoded = os.fspath(value).encode("utf-8") if isinstance(value, os.PathLike) else value.encode("utf-8")
    if b"\x00" in encoded:
        raise ValueError("string contains NUL byte")
    return encoded


def create_ses_seal(
    issuer_private_key: bytes,
    issuer_certificate_der: bytes,
    picture: bytes,
    *,
    version: int,
    vendor_id: str,
    seal_id: str,
    name: str,
    seal_type: int,
    authorized_signer_certificates: list[bytes],
    created_at: int,
    valid_from: int,
    valid_until: int,
    picture_type: str,
    picture_width_mm: int,
    picture_height_mm: int,
    issuer_distid: str = "1234567812345678",
) -> bytes:
    """Create a signed SES v4/v5 electronic seal as DER bytes."""
    options = json.dumps({
        "version": version,
        "vendor_id": vendor_id,
        "seal_id": seal_id,
        "name": name,
        "seal_type": seal_type,
        "authorized_signer_certificates_base64": [
            base64.b64encode(bytes(cert)).decode("ascii")
            for cert in authorized_signer_certificates
        ],
        "created_at": created_at,
        "valid_from": valid_from,
        "valid_until": valid_until,
        "picture_type": picture_type,
        "picture_width_mm": picture_width_mm,
        "picture_height_mm": picture_height_mm,
    }).encode("utf-8")
    key = bytes(issuer_private_key)
    certificate = bytes(issuer_certificate_der)
    picture_data = bytes(picture)
    output = _ffi.new("OfdBuffer *")
    _check(_lib.ofd_create_ses_seal_json(
        key, len(key), certificate, len(certificate), picture_data, len(picture_data),
        options, len(options), _utf8(issuer_distid), output,
    ))
    try:
        return bytes(_ffi.buffer(output.data, output.len))
    finally:
        _lib.ofd_buffer_free(output[0])


class Document:
    """Owns a Rust OFD document handle. Use as a context manager."""

    def __init__(self, handle):
        self._handle = handle

    @classmethod
    def open(cls, path: str | os.PathLike[str]) -> Document:
        output = _ffi.new("OfdDocument **")
        _check(_lib.ofd_open(_utf8(path), output))
        return cls(output[0])

    @classmethod
    def from_bytes(cls, data: bytes) -> Document:
        """Open an OFD package from ZIP bytes."""
        output = _ffi.new("OfdDocument **")
        _check(_lib.ofd_from_bytes(data, len(data), output))
        return cls(output[0])

    @classmethod
    def new(cls) -> Document:
        output = _ffi.new("OfdDocument **")
        _check(_lib.ofd_new(output))
        return cls(output[0])

    @classmethod
    def from_pdf_bytes(cls, data: bytes) -> Document:
        """Import a PDF containing supported vector drawing operations."""
        output = _ffi.new("OfdDocument **")
        _check(_lib.ofd_from_pdf_bytes(data, len(data), output))
        return cls(output[0])

    @property
    def is_encrypted(self) -> bool:
        """Whether this package contains OFD encryption metadata."""
        self._require_open()
        output = _ffi.new("int *")
        _check(_lib.ofd_is_encrypted(self._handle, output))
        return bool(output[0])

    def encryption_info(self) -> list[dict]:
        """List encryption operations and their user names."""
        self._require_open()
        output = _ffi.new("OfdBuffer *")
        _check(_lib.ofd_encryption_info_json(self._handle, output))
        try:
            return json.loads(bytes(_ffi.buffer(output.data, output.len)))
        finally:
            _lib.ofd_buffer_free(output[0])

    def decrypt_with_password(self, password: str, username: str | None = None) -> Document:
        """Return a separate plaintext document after password decryption."""
        self._require_open()
        output = _ffi.new("OfdDocument **")
        name = _ffi.NULL if username is None else _utf8(username)
        _check(_lib.ofd_decrypt_with_password(self._handle, _utf8(password), name, output))
        return Document(output[0])

    def prepare_archive(self) -> Document:
        """Return a separate package with supported OFD-A normalizations applied.

        Inspect the result with ``archive_violations()`` for remaining issues.
        """
        self._require_open()
        output = _ffi.new("OfdDocument **")
        _check(_lib.ofd_prepare_archive(self._handle, output))
        return Document(output[0])

    def encrypt_with_passwords(self, recipients: list[dict]) -> Document:
        """Return a separate OFD encrypted for the supplied password users."""
        self._require_open()
        output = _ffi.new("OfdDocument **")
        payload = json.dumps(recipients, ensure_ascii=False, allow_nan=False)
        _check(_lib.ofd_encrypt_with_passwords_json(self._handle, _utf8(payload), output))
        return Document(output[0])

    def decrypt_with_sm2_private_key(self, private_key: bytes, username: str | None = None) -> Document:
        """Decrypt with a raw 32-byte SM2 private key; return a separate document."""
        self._require_open()
        output = _ffi.new("OfdDocument **")
        name = _ffi.NULL if username is None else _utf8(username)
        _check(_lib.ofd_decrypt_with_sm2_private_key(
            self._handle, private_key, len(private_key), name, output))
        return Document(output[0])

    def encrypt_with_certificates(self, recipients: list[dict]) -> Document:
        """Encrypt for users with DER-encoded SM2 X.509 certificates."""
        self._require_open()
        encoded = []
        for recipient in recipients:
            item = dict(recipient)
            item["certificate_der_base64"] = base64.b64encode(
                item.pop("certificate_der")).decode("ascii")
            encoded.append(item)
        payload = json.dumps(encoded, ensure_ascii=False, allow_nan=False)
        output = _ffi.new("OfdDocument **")
        _check(_lib.ofd_encrypt_with_certificates_json(self._handle, _utf8(payload), output))
        return Document(output[0])

    def encrypt_with_recipients(self, recipients: list[dict]) -> Document:
        """Encrypt for password and SM2 certificate users in one package."""
        self._require_open()
        encoded = []
        for recipient in recipients:
            item = dict(recipient)
            if item.get("type") == "certificate":
                item["certificate_der_base64"] = base64.b64encode(
                    item.pop("certificate_der")).decode("ascii")
            encoded.append(item)
        payload = json.dumps(encoded, ensure_ascii=False, allow_nan=False)
        output = _ffi.new("OfdDocument **")
        _check(_lib.ofd_encrypt_with_recipients_json(self._handle, _utf8(payload), output))
        return Document(output[0])

    def close(self) -> None:
        if self._handle != _ffi.NULL:
            _lib.ofd_close(self._handle)
            self._handle = _ffi.NULL

    def __enter__(self) -> Document:
        return self

    def __exit__(self, _type, _value, _traceback) -> None:
        self.close()

    def __del__(self) -> None:
        if hasattr(self, "_handle"):
            self.close()

    def _require_open(self) -> None:
        if self._handle == _ffi.NULL:
            raise OfdError("document is closed")

    @property
    def page_count(self) -> int:
        self._require_open()
        output = _ffi.new("size_t *")
        _check(_lib.ofd_page_count(self._handle, output))
        return int(output[0])

    def move_page(self, from_index: int, to_index: int) -> None:
        """Move a page to another zero-based position in the same document body."""
        self._require_open()
        if from_index < 0 or to_index < 0:
            raise IndexError((from_index, to_index))
        _check(_lib.ofd_move_page(self._handle, from_index, to_index))

    def reorder_document_pages(self, document_index: int, order: list[int]) -> None:
        """Atomically reorder one body's pages using each local index once."""
        self._require_open()
        if document_index < 0 or any(index < 0 for index in order):
            raise IndexError((document_index, order))
        _check(_lib.ofd_reorder_document_pages_json(
            self._handle, document_index, _utf8(json.dumps(order))))

    def duplicate_page(self, index: int) -> int:
        """Append a copy of a page to its document body and return its index."""
        self._require_open()
        if index < 0:
            raise IndexError(index)
        output = _ffi.new("size_t *")
        _check(_lib.ofd_duplicate_page(self._handle, index, output))
        return int(output[0])

    def duplicate_page_at(self, index: int, position: int) -> int:
        """Copy a page into a position within its document body."""
        self._require_open()
        if index < 0 or position < 0:
            raise IndexError((index, position))
        output = _ffi.new("size_t *")
        _check(_lib.ofd_duplicate_page_at(self._handle, index, position, output))
        return int(output[0])

    def remove_page(self, index: int) -> None:
        """Remove a page from the document order if it has no dependencies."""
        self._require_open()
        if index < 0:
            raise IndexError(index)
        _check(_lib.ofd_remove_page(self._handle, index))

    def set_page_area(
        self,
        index: int,
        physical_box: tuple[float, float, float, float],
        application_box: tuple[float, float, float, float] | None = None,
        content_box: tuple[float, float, float, float] | None = None,
        bleed_box: tuple[float, float, float, float] | None = None,
    ) -> None:
        """Write page boxes in millimeters while retaining unrelated page XML."""
        self._require_open()
        if index < 0:
            raise IndexError(index)
        payload = json.dumps(
            {
                "physical_box": physical_box,
                "application_box": application_box,
                "content_box": content_box,
                "bleed_box": bleed_box,
            },
            allow_nan=False,
        )
        _check(_lib.ofd_set_page_area_json(self._handle, index, _utf8(payload)))

    def document_page_area(self, document_index: int) -> dict | None:
        """Return the explicit default page boxes of a document body, if present."""
        self._require_open()
        if document_index < 0:
            raise IndexError(document_index)
        output = _ffi.new("OfdBuffer *")
        _check(_lib.ofd_document_page_area_json(self._handle, document_index, output))
        try:
            return json.loads(bytes(_ffi.buffer(output.data, output.len)))
        finally:
            _lib.ofd_buffer_free(output[0])

    def set_document_page_area(
        self,
        document_index: int,
        physical_box: tuple[float, float, float, float],
        application_box: tuple[float, float, float, float] | None = None,
        content_box: tuple[float, float, float, float] | None = None,
        bleed_box: tuple[float, float, float, float] | None = None,
    ) -> None:
        """Write default page boxes in millimeters for one document body."""
        self._require_open()
        if document_index < 0:
            raise IndexError(document_index)
        payload = json.dumps(
            {
                "physical_box": physical_box,
                "application_box": application_box,
                "content_box": content_box,
                "bleed_box": bleed_box,
            },
            allow_nan=False,
        )
        _check(_lib.ofd_set_document_page_area_json(self._handle, document_index, _utf8(payload)))

    def page_text(self, index: int) -> list[str]:
        self._require_open()
        if index < 0:
            raise IndexError(index)
        output = _ffi.new("OfdBuffer *")
        _check(_lib.ofd_page_text_json(self._handle, index, output))
        try:
            return json.loads(bytes(_ffi.buffer(output.data, output.len)))
        finally:
            _lib.ofd_buffer_free(output[0])

    def all_text(self) -> list[list[str]]:
        """Return TextCode strings for every page in package order."""
        self._require_open()
        output = _ffi.new("OfdBuffer *")
        _check(_lib.ofd_all_text_json(self._handle, output))
        try:
            return json.loads(bytes(_ffi.buffer(output.data, output.len)))
        finally:
            _lib.ofd_buffer_free(output[0])

    def page_text_in_region(self, index: int, x: float, y: float, width: float, height: float) -> list[str]:
        """Return characters with text origins inside a page rectangle in millimeters."""
        self._require_open()
        if index < 0:
            raise IndexError(index)
        output = _ffi.new("OfdBuffer *")
        _check(_lib.ofd_page_text_in_region_json(self._handle, index, x, y, width, height, output))
        try:
            return json.loads(bytes(_ffi.buffer(output.data, output.len)))
        finally:
            _lib.ofd_buffer_free(output[0])

    def keyword_positions(self, keywords: list[str], pages: list[int] | None = None) -> list[dict]:
        """Find keyword boxes in page millimeters; page indexes start at zero."""
        self._require_open()
        query = json.dumps({"keywords": keywords, "pages": pages}, allow_nan=False)
        output = _ffi.new("OfdBuffer *")
        _check(_lib.ofd_keyword_positions_json(self._handle, _utf8(query), output))
        try:
            return json.loads(bytes(_ffi.buffer(output.data, output.len)))
        finally:
            _lib.ofd_buffer_free(output[0])

    def page_sources(self, index: int) -> list[dict]:
        """Return template XML sources followed by the page's own XML source."""
        self._require_open()
        if index < 0:
            raise IndexError(index)
        output = _ffi.new("OfdBuffer *")
        _check(_lib.ofd_page_sources_json(self._handle, index, output))
        try:
            return json.loads(bytes(_ffi.buffer(output.data, output.len)))
        finally:
            _lib.ofd_buffer_free(output[0])

    def page_layers(self, index: int) -> list[dict]:
        """Return page and template layers in painting order."""
        self._require_open()
        if index < 0:
            raise IndexError(index)
        output = _ffi.new("OfdBuffer *")
        _check(_lib.ofd_page_layers_json(self._handle, index, output))
        try:
            return json.loads(bytes(_ffi.buffer(output.data, output.len)))
        finally:
            _lib.ofd_buffer_free(output[0])

    def promote_page_to_template(self, source_page_index: int,
                                 additional_pages: list[int] | None = None,
                                 z_order: str = "Background") -> int:
        """Move source page content to a template and apply it to selected pages."""
        self._require_open()
        if source_page_index < 0 or any(index < 0 for index in (additional_pages or [])):
            raise IndexError("page index must be nonnegative")
        output = _ffi.new("size_t *")
        _check(_lib.ofd_promote_page_to_template(
            self._handle, source_page_index,
            _utf8(json.dumps(additional_pages or [])), _utf8(z_order), output))
        return int(output[0])

    def apply_page_template(self, page_index: int, template_id: str | int,
                            z_order: str = "Background") -> None:
        """Apply a document-defined template to another page."""
        self._require_open()
        if page_index < 0:
            raise IndexError(page_index)
        _check(_lib.ofd_apply_page_template(
            self._handle, page_index, _utf8(str(template_id)), _utf8(z_order)))

    def remove_page_template(self, page_index: int, template_id: str | int) -> None:
        """Remove one template reference from a page."""
        self._require_open()
        if page_index < 0:
            raise IndexError(page_index)
        _check(_lib.ofd_remove_page_template(
            self._handle, page_index, _utf8(str(template_id))))

    def remove_document_template(self, document_index: int, template_id: str | int) -> None:
        """Remove an unused template definition from a document body."""
        self._require_open()
        if document_index < 0:
            raise IndexError(document_index)
        _check(_lib.ofd_remove_document_template(
            self._handle, document_index, _utf8(str(template_id))))

    def page_objects(self, index: int) -> list[dict]:
        """Return page and template graphic objects in painting order."""
        self._require_open()
        if index < 0:
            raise IndexError(index)
        output = _ffi.new("OfdBuffer *")
        _check(_lib.ofd_page_objects_json(self._handle, index, output))
        try:
            return json.loads(bytes(_ffi.buffer(output.data, output.len)))
        finally:
            _lib.ofd_buffer_free(output[0])

    def page_object_clips(self, index: int) -> list[dict]:
        """Read page and template object clips in painting order.

        Each Area includes its source XML for content beyond the parsed fields.
        """
        self._require_open()
        if index < 0:
            raise IndexError(index)
        output = _ffi.new("OfdBuffer *")
        _check(_lib.ofd_page_object_clips_json(self._handle, index, output))
        try:
            return json.loads(bytes(_ffi.buffer(output.data, output.len)))
        finally:
            _lib.ofd_buffer_free(output[0])

    def page_object_paints(self, index: int) -> list[dict]:
        """Read direct path/text paints, including template objects, in painting order.

        Direct paints and effective DrawParam paints are returned separately.
        An absent effective paint leaves the renderer's black fallback unspecified.
        """
        self._require_open()
        if index < 0:
            raise IndexError(index)
        output = _ffi.new("OfdBuffer *")
        _check(_lib.ofd_page_object_paints_json(self._handle, index, output))
        try:
            return json.loads(bytes(_ffi.buffer(output.data, output.len)))
        finally:
            _lib.ofd_buffer_free(output[0])

    def page_object_stroke_styles(self, index: int) -> list[dict]:
        """Read effective path/text stroke geometry, including DrawParam inheritance."""
        self._require_open()
        if index < 0:
            raise IndexError(index)
        output = _ffi.new("OfdBuffer *")
        _check(_lib.ofd_page_object_stroke_styles_json(self._handle, index, output))
        try:
            return json.loads(bytes(_ffi.buffer(output.data, output.len)))
        finally:
            _lib.ofd_buffer_free(output[0])

    def page_object_actions(self, page_index: int, object_id: str | int) -> list[dict]:
        """Return actions attached to a direct page graphic object."""
        self._require_open()
        if page_index < 0:
            raise IndexError(page_index)
        output = _ffi.new("OfdBuffer *")
        _check(_lib.ofd_page_object_actions_json(
            self._handle, page_index, _utf8(str(object_id)), output))
        try:
            return json.loads(bytes(_ffi.buffer(output.data, output.len)))
        finally:
            _lib.ofd_buffer_free(output[0])

    def page_source_object_actions(
        self, page_index: int, source_path: str, object_id: str | int,
    ) -> list[dict]:
        """Return actions on an object in a page or referenced template source."""
        self._require_open()
        if page_index < 0:
            raise IndexError(page_index)
        output = _ffi.new("OfdBuffer *")
        _check(_lib.ofd_page_source_object_actions_json(
            self._handle, page_index, _utf8(source_path),
            _utf8(str(object_id)), output))
        try:
            return json.loads(bytes(_ffi.buffer(output.data, output.len)))
        finally:
            _lib.ofd_buffer_free(output[0])

    def add_page_object_uri_action(self, page_index: int, object_id: str | int,
                                   uri: str) -> None:
        """Append a click action that opens a URI from a page graphic object."""
        self._require_open()
        if page_index < 0:
            raise IndexError(page_index)
        _check(_lib.ofd_add_page_object_uri_action(
            self._handle, page_index, _utf8(str(object_id)), _utf8(uri)))

    def add_page_object_goto_action(
        self, page_index: int, object_id: str | int, page_id: str | int,
        kind: str = "XYZ", *, left: float | None = None, top: float | None = None,
        right: float | None = None, bottom: float | None = None,
        zoom: float | None = None,
    ) -> None:
        """Append a click action to a page in the same document body."""
        self._require_open()
        if page_index < 0:
            raise IndexError(page_index)
        payload = json.dumps({
            "page_id": str(page_id), "kind": kind,
            "left": left, "top": top, "right": right,
            "bottom": bottom, "zoom": zoom,
        }, ensure_ascii=False, allow_nan=False)
        _check(_lib.ofd_add_page_object_goto_action_json(
            self._handle, page_index, _utf8(str(object_id)), _utf8(payload)))

    def add_page_object_bookmark_action(self, page_index: int, object_id: str | int,
                                        bookmark_name: str) -> None:
        """Append a click action targeting a named bookmark."""
        self._require_open()
        if page_index < 0:
            raise IndexError(page_index)
        _check(_lib.ofd_add_page_object_bookmark_action(
            self._handle, page_index, _utf8(str(object_id)), _utf8(bookmark_name)))

    def add_page_object_attachment_action(self, page_index: int, object_id: str | int,
                                          attachment_id: str | int,
                                          new_window: bool | None = None) -> None:
        """Append a click action that opens an attachment in this document."""
        self._require_open()
        if page_index < 0:
            raise IndexError(page_index)
        if new_window is not None and not isinstance(new_window, bool):
            raise TypeError("new_window must be bool or None")
        flag = -1 if new_window is None else int(new_window)
        _check(_lib.ofd_add_page_object_attachment_action(
            self._handle, page_index, _utf8(str(object_id)),
            _utf8(str(attachment_id)), flag))

    def add_page_object_sound_action(
        self, page_index: int, object_id: str | int, resource_id: str | int,
        *, volume: int | None = None, repeat: bool | None = None,
        synchronous: bool | None = None,
    ) -> None:
        """Append a click Sound action using an Audio resource."""
        self._require_open()
        if page_index < 0:
            raise IndexError(page_index)
        payload = json.dumps({"kind": "Sound", "resource_id": str(resource_id),
                              "volume": volume, "repeat": repeat,
                              "synchronous": synchronous}, allow_nan=False)
        _check(_lib.ofd_add_page_object_media_action_json(
            self._handle, page_index, _utf8(str(object_id)), _utf8(payload)))

    def add_page_object_movie_action(
        self, page_index: int, object_id: str | int, resource_id: str | int,
        *, operator: str | None = None,
    ) -> None:
        """Append a click Movie action using a Video resource."""
        self._require_open()
        if page_index < 0:
            raise IndexError(page_index)
        payload = json.dumps({"kind": "Movie", "resource_id": str(resource_id),
                              "operator": operator}, allow_nan=False)
        _check(_lib.ofd_add_page_object_media_action_json(
            self._handle, page_index, _utf8(str(object_id)), _utf8(payload)))

    def remove_page_object_action(self, page_index: int, object_id: str | int,
                                  action_index: int) -> None:
        """Remove one action by its zero-based index on a page object."""
        self._require_open()
        if page_index < 0 or action_index < 0:
            raise IndexError((page_index, action_index))
        _check(_lib.ofd_remove_page_object_action(
            self._handle, page_index, _utf8(str(object_id)), action_index))

    def set_page_object_action_regions(
        self, page_index: int, object_id: str | int, action_index: int,
        rectangles: list[tuple[float, float, float, float]],
    ) -> None:
        """Set rectangular trigger areas; an empty list clears the explicit region."""
        self._require_open()
        if page_index < 0 or action_index < 0:
            raise IndexError((page_index, action_index))
        payload = json.dumps(rectangles, allow_nan=False)
        _check(_lib.ofd_set_page_object_action_regions_json(
            self._handle, page_index, _utf8(str(object_id)),
            action_index, _utf8(payload)))

    def page_text_with_templates(self, index: int) -> list[str]:
        """Return TextCode strings from templates and the page itself."""
        self._require_open()
        if index < 0:
            raise IndexError(index)
        output = _ffi.new("OfdBuffer *")
        _check(_lib.ofd_page_text_with_templates_json(self._handle, index, output))
        try:
            return json.loads(bytes(_ffi.buffer(output.data, output.len)))
        finally:
            _lib.ofd_buffer_free(output[0])

    def pages(self) -> list[dict]:
        """Return page identifiers, package paths, and physical boxes."""
        self._require_open()
        output = _ffi.new("OfdBuffer *")
        _check(_lib.ofd_pages_json(self._handle, output))
        try:
            return json.loads(bytes(_ffi.buffer(output.data, output.len)))
        finally:
            _lib.ofd_buffer_free(output[0])

    def append_document_body(self) -> int:
        """Append an empty document body and return its zero-based index."""
        self._require_open()
        output = _ffi.new("size_t *")
        _check(_lib.ofd_append_document_body(self._handle, output))
        return int(output[0])

    def move_document_body(self, from_index: int, to_index: int) -> None:
        """Move a document body to another zero-based position."""
        self._require_open()
        if from_index < 0 or to_index < 0:
            raise IndexError((from_index, to_index))
        _check(_lib.ofd_move_document_body(self._handle, from_index, to_index))

    def remove_document_body(self, index: int) -> None:
        """Remove a document body from the order while retaining its files."""
        self._require_open()
        if index < 0:
            raise IndexError(index)
        _check(_lib.ofd_remove_document_body(self._handle, index))

    def extract_document_body(self, index: int) -> Document:
        """Return a separate package with one active document body."""
        self._require_open()
        if index < 0:
            raise IndexError(index)
        output = _ffi.new("OfdDocument **")
        _check(_lib.ofd_extract_document_body(self._handle, index, output))
        return Document(output[0])

    def append_document_bodies_from(self, source: Document) -> list[int]:
        """Import every body from an unsigned OFD as separate document bodies."""
        self._require_open()
        source._require_open()
        output = _ffi.new("OfdBuffer *")
        _check(_lib.ofd_append_document_bodies_from_json(
            self._handle, source._handle, output))
        try:
            return json.loads(bytes(_ffi.buffer(output.data, output.len)))
        finally:
            _lib.ofd_buffer_free(output[0])

    def append_document_body_from(self, source: Document, source_index: int) -> int:
        """Import one selected document body from an unsigned OFD."""
        self._require_open()
        source._require_open()
        if source_index < 0:
            raise IndexError(source_index)
        output = _ffi.new("size_t *")
        _check(_lib.ofd_append_document_body_from(
            self._handle, source._handle, source_index, output))
        return int(output[0])

    def validate_references(self) -> list[dict]:
        """Report broken IDs, geometry, and supported resource references."""
        self._require_open()
        output = _ffi.new("OfdBuffer *")
        _check(_lib.ofd_validate_references_json(self._handle, output))
        try:
            return json.loads(bytes(_ffi.buffer(output.data, output.len)))
        finally:
            _lib.ofd_buffer_free(output[0])

    def archive_violations(self) -> list[dict]:
        """Return violations from the currently supported OFD-A rules."""
        self._require_open()
        output = _ffi.new("OfdBuffer *")
        _check(_lib.ofd_archive_violations_json(self._handle, output))
        try:
            return json.loads(bytes(_ffi.buffer(output.data, output.len)))
        finally:
            _lib.ofd_buffer_free(output[0])

    def page_text_objects(self, index: int) -> list[dict]:
        """Return positioned text objects, TextCode runs, and CGTransform mappings."""
        self._require_open()
        if index < 0:
            raise IndexError(index)
        output = _ffi.new("OfdBuffer *")
        _check(_lib.ofd_page_text_objects_json(self._handle, index, output))
        try:
            return json.loads(bytes(_ffi.buffer(output.data, output.len)))
        finally:
            _lib.ofd_buffer_free(output[0])

    def page_paths(self, index: int) -> list[dict]:
        """Return OFD path objects and their drawing commands."""
        self._require_open()
        if index < 0:
            raise IndexError(index)
        output = _ffi.new("OfdBuffer *")
        _check(_lib.ofd_page_paths_json(self._handle, index, output))
        try:
            return json.loads(bytes(_ffi.buffer(output.data, output.len)))
        finally:
            _lib.ofd_buffer_free(output[0])

    def page_images(self, index: int) -> list[dict]:
        """Return OFD image objects and resolved resource paths."""
        self._require_open()
        if index < 0:
            raise IndexError(index)
        output = _ffi.new("OfdBuffer *")
        _check(_lib.ofd_page_images_json(self._handle, index, output))
        try:
            return json.loads(bytes(_ffi.buffer(output.data, output.len)))
        finally:
            _lib.ofd_buffer_free(output[0])

    def page_multimedia_resources(self, index: int) -> list[dict]:
        """List image, audio and video resources visible to a page."""
        self._require_open()
        if index < 0:
            raise IndexError(index)
        output = _ffi.new("OfdBuffer *")
        _check(_lib.ofd_page_multimedia_resources_json(self._handle, index, output))
        try:
            return json.loads(bytes(_ffi.buffer(output.data, output.len)))
        finally:
            _lib.ofd_buffer_free(output[0])

    def page_multimedia_data(self, index: int, resource_id: str | int) -> bytes:
        """Return raw bytes of a multimedia resource visible to a page."""
        self._require_open()
        if index < 0:
            raise IndexError(index)
        output = _ffi.new("OfdBuffer *")
        _check(_lib.ofd_page_multimedia_data(self._handle, index, _utf8(str(resource_id)), output))
        try:
            return bytes(_ffi.buffer(output.data, output.len))
        finally:
            _lib.ofd_buffer_free(output[0])

    def document_multimedia_resources(self, index: int) -> list[dict]:
        """List multimedia resources declared by a document body."""
        self._require_open()
        if index < 0:
            raise IndexError(index)
        output = _ffi.new("OfdBuffer *")
        _check(_lib.ofd_document_multimedia_resources_json(self._handle, index, output))
        try:
            return json.loads(bytes(_ffi.buffer(output.data, output.len)))
        finally:
            _lib.ofd_buffer_free(output[0])

    def document_multimedia_data(self, index: int, resource_id: str | int) -> bytes:
        """Return raw bytes of a document-level multimedia resource."""
        self._require_open()
        if index < 0:
            raise IndexError(index)
        output = _ffi.new("OfdBuffer *")
        _check(_lib.ofd_document_multimedia_data(self._handle, index, _utf8(str(resource_id)), output))
        try:
            return bytes(_ffi.buffer(output.data, output.len))
        finally:
            _lib.ofd_buffer_free(output[0])

    def page_composite_objects(self, index: int) -> list[dict]:
        """Return OFD composite graphic objects and their resource IDs."""
        self._require_open()
        if index < 0:
            raise IndexError(index)
        output = _ffi.new("OfdBuffer *")
        _check(_lib.ofd_page_composite_objects_json(self._handle, index, output))
        try:
            return json.loads(bytes(_ffi.buffer(output.data, output.len)))
        finally:
            _lib.ofd_buffer_free(output[0])

    def page_composite_units(self, index: int) -> list[dict]:
        """Return composite unit dimensions and image fallback resource IDs."""
        self._require_open()
        if index < 0:
            raise IndexError(index)
        output = _ffi.new("OfdBuffer *")
        _check(_lib.ofd_page_composite_units_json(self._handle, index, output))
        try:
            return json.loads(bytes(_ffi.buffer(output.data, output.len)))
        finally:
            _lib.ofd_buffer_free(output[0])

    def set_page_composite_unit_images(self, page_index: int, unit_id: str | int,
                                       thumbnail_id: str | int | None,
                                       substitution_id: str | int | None) -> None:
        """Set or clear a composite unit's thumbnail and substitution images."""
        self._require_open()
        if page_index < 0:
            raise IndexError(page_index)
        thumbnail = _ffi.NULL if thumbnail_id is None else _utf8(str(thumbnail_id))
        substitution = _ffi.NULL if substitution_id is None else _utf8(str(substitution_id))
        _check(_lib.ofd_set_page_composite_unit_images(
            self._handle, page_index, _utf8(str(unit_id)), thumbnail, substitution))

    def page_composite_images(self, index: int) -> list[dict]:
        """Return images inside composite objects in page placement order."""
        self._require_open()
        if index < 0:
            raise IndexError(index)
        output = _ffi.new("OfdBuffer *")
        _check(_lib.ofd_page_composite_images_json(self._handle, index, output))
        try:
            return json.loads(bytes(_ffi.buffer(output.data, output.len)))
        finally:
            _lib.ofd_buffer_free(output[0])

    def page_image_data(self, page_index: int, image_index: int) -> bytes:
        """Return the original bytes of an image object."""
        self._require_open()
        if page_index < 0 or image_index < 0:
            raise IndexError((page_index, image_index))
        output = _ffi.new("OfdBuffer *")
        _check(_lib.ofd_page_image_data(self._handle, page_index, image_index, output))
        try:
            return bytes(_ffi.buffer(output.data, output.len))
        finally:
            _lib.ofd_buffer_free(output[0])

    def page_composite_image_data(self, page_index: int, image_index: int) -> bytes:
        """Return original bytes of an image inside a composite object."""
        self._require_open()
        if page_index < 0 or image_index < 0:
            raise IndexError((page_index, image_index))
        output = _ffi.new("OfdBuffer *")
        _check(_lib.ofd_page_composite_image_data(self._handle, page_index, image_index, output))
        try:
            return bytes(_ffi.buffer(output.data, output.len))
        finally:
            _lib.ofd_buffer_free(output[0])

    def page_svg(self, index: int, *, prefer_image_substitution: bool = False) -> str:
        """Return an SVG preview, optionally using alternative image resources."""
        self._require_open()
        if index < 0:
            raise IndexError(index)
        if not isinstance(prefer_image_substitution, bool):
            raise TypeError("prefer_image_substitution must be bool")
        output = _ffi.new("OfdBuffer *")
        if prefer_image_substitution:
            _check(_lib.ofd_page_svg_with_options(self._handle, index, 1, output))
        else:
            _check(_lib.ofd_page_svg(self._handle, index, output))
        try:
            return bytes(_ffi.buffer(output.data, output.len)).decode("utf-8")
        finally:
            _lib.ofd_buffer_free(output[0])

    def page_png(self, index: int, dpi: float = 96.0, *,
                 prefer_image_substitution: bool = False) -> bytes:
        """Render a page as PNG, optionally using alternative images."""
        self._require_open()
        if index < 0:
            raise IndexError(index)
        if not isinstance(prefer_image_substitution, bool):
            raise TypeError("prefer_image_substitution must be bool")
        output = _ffi.new("OfdBuffer *")
        if prefer_image_substitution:
            _check(_lib.ofd_page_png_with_options(self._handle, index, dpi, 1, output))
        else:
            _check(_lib.ofd_page_png(self._handle, index, dpi, output))
        try:
            return bytes(_ffi.buffer(output.data, output.len))
        finally:
            _lib.ofd_buffer_free(output[0])

    def to_pdf(self, *, prefer_image_substitution: bool = False) -> bytes:
        """Export to PDF, optionally using alternative image resources."""
        self._require_open()
        if not isinstance(prefer_image_substitution, bool):
            raise TypeError("prefer_image_substitution must be bool")
        output = _ffi.new("OfdBuffer *")
        if prefer_image_substitution:
            _check(_lib.ofd_to_pdf_with_options(self._handle, 1, output))
        else:
            _check(_lib.ofd_to_pdf(self._handle, output))
        try:
            return bytes(_ffi.buffer(output.data, output.len))
        finally:
            _lib.ofd_buffer_free(output[0])

    def entries(self) -> list[str]:
        """Return normalized package paths in sorted order."""
        self._require_open()
        output = _ffi.new("OfdBuffer *")
        _check(_lib.ofd_entries_json(self._handle, output))
        try:
            return json.loads(bytes(_ffi.buffer(output.data, output.len)))
        finally:
            _lib.ofd_buffer_free(output[0])

    def documents(self) -> list[dict]:
        """Return metadata for every document body."""
        self._require_open()
        output = _ffi.new("OfdBuffer *")
        _check(_lib.ofd_documents_json(self._handle, output))
        try:
            return json.loads(bytes(_ffi.buffer(output.data, output.len)))
        finally:
            _lib.ofd_buffer_free(output[0])

    def package_info(self) -> dict:
        """Return the OFD.xml version and document type."""
        self._require_open()
        output = _ffi.new("OfdBuffer *")
        _check(_lib.ofd_package_info_json(self._handle, output))
        try:
            return json.loads(bytes(_ffi.buffer(output.data, output.len)))
        finally:
            _lib.ofd_buffer_free(output[0])

    def set_package_header(self, version: str, doc_type: str) -> None:
        """Set root attributes in an unsigned package."""
        self._require_open()
        _check(_lib.ofd_set_package_header(self._handle, version.encode(), doc_type.encode()))

    def document_permissions(self, document_index: int) -> dict:
        """Return the effective permissions of one document body."""
        self._require_open()
        if document_index < 0:
            raise IndexError(document_index)
        output = _ffi.new("OfdBuffer *")
        _check(_lib.ofd_document_permissions_json(self._handle, document_index, output))
        try:
            return json.loads(bytes(_ffi.buffer(output.data, output.len)))
        finally:
            _lib.ofd_buffer_free(output[0])

    def set_document_permissions(self, document_index: int, permissions: dict) -> None:
        """Set a complete permission declaration on one document body."""
        self._require_open()
        if document_index < 0:
            raise IndexError(document_index)
        payload = json.dumps(permissions, ensure_ascii=False, allow_nan=False)
        _check(_lib.ofd_set_document_permissions_json(self._handle, document_index, _utf8(payload)))

    def view_preferences(self, document_index: int) -> dict:
        """Return the effective initial viewer preferences of one document body."""
        self._require_open()
        if document_index < 0:
            raise IndexError(document_index)
        output = _ffi.new("OfdBuffer *")
        _check(_lib.ofd_view_preferences_json(self._handle, document_index, output))
        try:
            return json.loads(bytes(_ffi.buffer(output.data, output.len)))
        finally:
            _lib.ofd_buffer_free(output[0])

    def set_view_preferences(self, document_index: int, preferences: dict) -> None:
        """Set complete initial viewer preferences for one document body."""
        self._require_open()
        if document_index < 0:
            raise IndexError(document_index)
        payload = json.dumps(preferences, ensure_ascii=False, allow_nan=False)
        _check(_lib.ofd_set_view_preferences_json(self._handle, document_index, _utf8(payload)))

    def document_actions(self, document_index: int) -> list[dict]:
        """Return direct document actions in execution order."""
        self._require_open()
        if document_index < 0:
            raise IndexError(document_index)
        output = _ffi.new("OfdBuffer *")
        _check(_lib.ofd_document_actions_json(self._handle, document_index, output))
        try:
            return json.loads(bytes(_ffi.buffer(output.data, output.len)))
        finally:
            _lib.ofd_buffer_free(output[0])

    def add_document_action(self, document_index: int, action: dict) -> None:
        """Append a URI, page, bookmark, or attachment action."""
        self._require_open()
        if document_index < 0:
            raise IndexError(document_index)
        payload = json.dumps(action, ensure_ascii=False, allow_nan=False)
        _check(_lib.ofd_add_document_action_json(self._handle, document_index, _utf8(payload)))

    def remove_document_action(self, document_index: int, action_index: int) -> None:
        """Remove one direct document action by zero-based index."""
        self._require_open()
        if document_index < 0:
            raise IndexError(document_index)
        if action_index < 0:
            raise IndexError(action_index)
        _check(_lib.ofd_remove_document_action(self._handle, document_index, action_index))

    def page_actions(self, page_index: int) -> list[dict]:
        """Return actions attached directly to a page in execution order."""
        self._require_open()
        if page_index < 0:
            raise IndexError(page_index)
        output = _ffi.new("OfdBuffer *")
        _check(_lib.ofd_page_actions_json(self._handle, page_index, output))
        try:
            return json.loads(bytes(_ffi.buffer(output.data, output.len)))
        finally:
            _lib.ofd_buffer_free(output[0])

    def add_page_action(self, page_index: int, action: dict) -> None:
        """Append a page-open action targeting a URI, page, bookmark, or attachment."""
        self._require_open()
        if page_index < 0:
            raise IndexError(page_index)
        payload = json.dumps(action, ensure_ascii=False, allow_nan=False)
        _check(_lib.ofd_add_page_action_json(self._handle, page_index, _utf8(payload)))

    def remove_page_action(self, page_index: int, action_index: int) -> None:
        """Remove one direct page action by zero-based index."""
        self._require_open()
        if page_index < 0:
            raise IndexError(page_index)
        if action_index < 0:
            raise IndexError(action_index)
        _check(_lib.ofd_remove_page_action(self._handle, page_index, action_index))

    def integrity_manifest(self) -> dict | None:
        """Read the optional OFDEntries.xml file manifest."""
        self._require_open()
        output = _ffi.new("OfdBuffer *")
        _check(_lib.ofd_integrity_manifest_json(self._handle, output))
        try:
            return json.loads(bytes(_ffi.buffer(output.data, output.len)))
        finally:
            _lib.ofd_buffer_free(output[0])

    def check_integrity_manifest(self) -> dict | None:
        """Report missing and extra package files; signature validity is separate."""
        self._require_open()
        output = _ffi.new("OfdBuffer *")
        _check(_lib.ofd_check_integrity_manifest_json(self._handle, output))
        try:
            return json.loads(bytes(_ffi.buffer(output.data, output.len)))
        finally:
            _lib.ofd_buffer_free(output[0])

    def verify_integrity_manifest_sm2(self, signature_distid: str | None = None) -> dict | None:
        """Check the manifest's SM2 signature and file list, without certificate trust."""
        self._require_open()
        output = _ffi.new("OfdBuffer *")
        distid = _ffi.NULL if signature_distid is None else _utf8(signature_distid)
        _check(_lib.ofd_verify_integrity_manifest_sm2_json(self._handle, distid, output))
        try:
            return json.loads(bytes(_ffi.buffer(output.data, output.len)))
        finally:
            _lib.ofd_buffer_free(output[0])

    def protect_integrity_sm2(
        self, private_key: bytes, certificate_der: bytes,
        signature_distid: str | None = None,
    ) -> None:
        """Add a signed root integrity manifest using an SM2 key and certificate."""
        self._require_open()
        key = bytes(private_key)
        certificate = bytes(certificate_der)
        distid = _ffi.NULL if signature_distid is None else _utf8(signature_distid)
        _check(_lib.ofd_protect_integrity_sm2(
            self._handle, key, len(key), certificate, len(certificate), distid,
        ))

    def verify_integrity_manifest_sm2_with_ordered_chain(
        self, signer_certificate_der: bytes, intermediates_der: list[bytes],
        anchor_certificate_der: bytes, at_unix_seconds: int,
        certificate_distid: str = "", signature_distid: str = "1234567812345678",
    ) -> dict | None:
        """Check integrity and an ordered SM2 chain to a caller-trusted anchor."""
        return self._verify_integrity_manifest_chain(
            signer_certificate_der, intermediates_der, anchor_certificate_der,
            at_unix_seconds, certificate_distid, signature_distid, False,
        )

    def verify_integrity_manifest_sm2_with_certificate_pool(
        self, signer_certificate_der: bytes, candidate_certificates_der: list[bytes],
        anchor_certificate_der: bytes, at_unix_seconds: int,
        certificate_distid: str = "", signature_distid: str = "1234567812345678",
    ) -> dict | None:
        """Find a valid SM2 chain among unordered intermediate certificates."""
        return self._verify_integrity_manifest_chain(
            signer_certificate_der, candidate_certificates_der, anchor_certificate_der,
            at_unix_seconds, certificate_distid, signature_distid, True,
        )

    def _verify_integrity_manifest_chain(
        self, signer_certificate_der: bytes, intermediates_der: list[bytes],
        anchor_certificate_der: bytes, at_unix_seconds: int,
        certificate_distid: str, signature_distid: str, unordered: bool,
    ) -> dict | None:
        self._require_open()
        options = {
            "signer_certificate_der_base64": base64.b64encode(signer_certificate_der).decode(),
            "intermediates_der_base64": [base64.b64encode(item).decode()
                                         for item in intermediates_der],
            "anchor_certificate_der_base64": base64.b64encode(anchor_certificate_der).decode(),
            "at_unix_seconds": at_unix_seconds,
            "certificate_distid": certificate_distid,
            "signature_distid": signature_distid,
        }
        output = _ffi.new("OfdBuffer *")
        function = (_lib.ofd_verify_integrity_manifest_sm2_with_certificate_pool_json
                    if unordered else _lib.ofd_verify_integrity_manifest_sm2_with_ordered_chain_json)
        _check(function(
            self._handle, _utf8(json.dumps(options)), output,
        ))
        try:
            return json.loads(bytes(_ffi.buffer(output.data, output.len)))
        finally:
            _lib.ofd_buffer_free(output[0])

    def versions(self) -> list[dict]:
        """Return document versions and their package file lists."""
        self._require_open()
        output = _ffi.new("OfdBuffer *")
        _check(_lib.ofd_versions_json(self._handle, output))
        try:
            return json.loads(bytes(_ffi.buffer(output.data, output.len)))
        finally:
            _lib.ofd_buffer_free(output[0])

    def extract_version_snapshot(self, index: int) -> Document:
        """Open one archived version as an independent OFD package."""
        self._require_open()
        if index < 0:
            raise IndexError(index)
        output = _ffi.new("OfdDocument **")
        _check(_lib.ofd_extract_version_snapshot(self._handle, index, output))
        return Document(output[0])

    def add_version_snapshot(self, id: str, document_index: int = 0, name: str | None = None) -> dict:
        """Archive the current package files as a document version."""
        self._require_open()
        if document_index < 0:
            raise IndexError(document_index)
        payload = json.dumps({"id": id, "name": name}, ensure_ascii=False, allow_nan=False)
        output = _ffi.new("OfdBuffer *")
        _check(_lib.ofd_add_version_snapshot_json(self._handle, document_index, _utf8(payload), output))
        try:
            return json.loads(bytes(_ffi.buffer(output.data, output.len)))
        finally:
            _lib.ofd_buffer_free(output[0])

    def bookmarks(self) -> list[dict]:
        """Return named bookmarks from every document body."""
        self._require_open()
        output = _ffi.new("OfdBuffer *")
        _check(_lib.ofd_bookmarks_json(self._handle, output))
        try:
            return json.loads(bytes(_ffi.buffer(output.data, output.len)))
        finally:
            _lib.ofd_buffer_free(output[0])

    def add_bookmark(
        self,
        name: str,
        page_id: str,
        document_index: int = 0,
        kind: str = "XYZ",
        *,
        left: float | None = None,
        top: float | None = None,
        right: float | None = None,
        bottom: float | None = None,
        zoom: float | None = None,
    ) -> None:
        """Add a named destination to an existing page."""
        self._require_open()
        if document_index < 0:
            raise IndexError(document_index)
        value = {
            "name": name, "page_id": page_id, "kind": kind,
            "left": left, "top": top, "right": right,
            "bottom": bottom, "zoom": zoom,
        }
        payload = json.dumps(value, ensure_ascii=False, allow_nan=False)
        _check(_lib.ofd_add_bookmark_json(self._handle, document_index, _utf8(payload)))

    def remove_bookmark(self, name: str, document_index: int = 0) -> None:
        """Remove a named bookmark from a document body."""
        self._require_open()
        if document_index < 0:
            raise IndexError(document_index)
        _check(_lib.ofd_remove_bookmark(self._handle, document_index, _utf8(name)))

    def outlines(self) -> list[dict]:
        """Return outline nodes in depth-first order with parent indexes."""
        self._require_open()
        output = _ffi.new("OfdBuffer *")
        _check(_lib.ofd_outlines_json(self._handle, output))
        try:
            return json.loads(bytes(_ffi.buffer(output.data, output.len)))
        finally:
            _lib.ofd_buffer_free(output[0])

    def add_outline(
        self, title: str, document_index: int = 0, *,
        parent_index: int | None = None,
        destination: dict | None = None,
        expanded: bool = True,
    ) -> int:
        """Add a navigation node and return its index in ``outlines()``."""
        self._require_open()
        if document_index < 0:
            raise IndexError(document_index)
        if parent_index is not None and parent_index < 0:
            raise IndexError(parent_index)
        payload = json.dumps(
            {"title": title, "parent_index": parent_index,
             "destination": destination, "expanded": expanded},
            ensure_ascii=False, allow_nan=False,
        )
        output = _ffi.new("size_t *")
        _check(_lib.ofd_add_outline_json(self._handle, document_index, _utf8(payload), output))
        return int(output[0])

    def remove_outline(self, index: int) -> None:
        """Remove an outline node and all its descendants."""
        self._require_open()
        if index < 0:
            raise IndexError(index)
        _check(_lib.ofd_remove_outline(self._handle, index))

    def set_outline_title(self, index: int, title: str) -> None:
        """Change an outline title while preserving its actions and children."""
        self._require_open()
        if index < 0:
            raise IndexError(index)
        _check(_lib.ofd_set_outline_title(self._handle, index, _utf8(title)))

    def set_outline_expanded(self, index: int, expanded: bool) -> None:
        """Change an outline's initial expanded state."""
        self._require_open()
        if index < 0:
            raise IndexError(index)
        if not isinstance(expanded, bool):
            raise TypeError("expanded must be a bool")
        _check(_lib.ofd_set_outline_expanded(self._handle, index, int(expanded)))

    def set_outline_destination(self, index: int, destination: dict) -> None:
        """Retarget an outline's direct GoTo action to a page in its document."""
        self._require_open()
        if index < 0:
            raise IndexError(index)
        payload = json.dumps(destination, ensure_ascii=False, allow_nan=False)
        _check(_lib.ofd_set_outline_destination_json(self._handle, index, _utf8(payload)))

    def set_outline_bookmark(self, index: int, name: str) -> None:
        """Point an outline at an existing bookmark in the same document body."""
        self._require_open()
        if index < 0:
            raise IndexError(index)
        _check(_lib.ofd_set_outline_bookmark(self._handle, index, _utf8(name)))

    def set_document_info_field(self, document_index: int, field: str, value: str) -> None:
        """Update one standard plain-text DocInfo field."""
        self._require_open()
        if document_index < 0:
            raise IndexError(document_index)
        _check(_lib.ofd_set_document_info_field(self._handle, document_index, _utf8(field), _utf8(value)))

    def remove_document_info_field(self, document_index: int, field: str) -> None:
        """Remove an optional standard plain-text DocInfo field."""
        self._require_open()
        if document_index < 0:
            raise IndexError(document_index)
        _check(_lib.ofd_remove_document_info_field(self._handle, document_index, _utf8(field)))

    def add_document_keyword(self, document_index: int, keyword: str) -> None:
        """Append a keyword to a document's DocInfo."""
        self._require_open()
        if document_index < 0:
            raise IndexError(document_index)
        _check(_lib.ofd_add_document_keyword(self._handle, document_index, _utf8(keyword)))

    def remove_document_keyword(self, document_index: int, keyword_index: int) -> None:
        """Remove a keyword by its zero-based index."""
        self._require_open()
        if document_index < 0:
            raise IndexError(document_index)
        if keyword_index < 0:
            raise IndexError(keyword_index)
        _check(_lib.ofd_remove_document_keyword(self._handle, document_index, keyword_index))

    def set_document_custom_data(self, document_index: int, name: str, value: str) -> None:
        """Create or update one named custom DocInfo value."""
        self._require_open()
        if document_index < 0:
            raise IndexError(document_index)
        _check(_lib.ofd_set_document_custom_data(
            self._handle, document_index, _utf8(name), _utf8(value)))

    def remove_document_custom_data(self, document_index: int, name: str) -> None:
        """Remove one named custom DocInfo value."""
        self._require_open()
        if document_index < 0:
            raise IndexError(document_index)
        _check(_lib.ofd_remove_document_custom_data(
            self._handle, document_index, _utf8(name)))

    def entry(self, path: str) -> bytes:
        self._require_open()
        output = _ffi.new("OfdBuffer *")
        _check(_lib.ofd_get_entry(self._handle, _utf8(path), output))
        try:
            return bytes(_ffi.buffer(output.data, output.len))
        finally:
            _lib.ofd_buffer_free(output[0])

    def attachments(self) -> list[dict]:
        """Return attachment metadata in document order."""
        self._require_open()
        output = _ffi.new("OfdBuffer *")
        _check(_lib.ofd_attachments_json(self._handle, output))
        try:
            return json.loads(bytes(_ffi.buffer(output.data, output.len)))
        finally:
            _lib.ofd_buffer_free(output[0])

    def custom_tags(self) -> list[dict]:
        """Return custom tag index entries from all document bodies."""
        self._require_open()
        output = _ffi.new("OfdBuffer *")
        _check(_lib.ofd_custom_tags_json(self._handle, output))
        try:
            return json.loads(bytes(_ffi.buffer(output.data, output.len)))
        finally:
            _lib.ofd_buffer_free(output[0])

    def extensions(self) -> list[dict]:
        """Return extension entries from all document bodies."""
        self._require_open()
        output = _ffi.new("OfdBuffer *")
        _check(_lib.ofd_extensions_json(self._handle, output))
        try:
            return json.loads(bytes(_ffi.buffer(output.data, output.len)))
        finally:
            _lib.ofd_buffer_free(output[0])

    def add_extension(self, extension: dict) -> None:
        """Append a document extension with properties, file links, and Data XML."""
        self._require_open()
        payload = json.dumps(extension, ensure_ascii=False, allow_nan=False)
        _check(_lib.ofd_add_extension_json(self._handle, _utf8(payload)))

    def update_extension(self, index: int, extension: dict) -> None:
        """Replace an extension record in place; its document index cannot change."""
        self._require_open()
        if index < 0:
            raise IndexError(index)
        payload = json.dumps(extension, ensure_ascii=False, allow_nan=False)
        _check(_lib.ofd_update_extension_json(self._handle, index, _utf8(payload)))

    def remove_extension(self, index: int) -> None:
        """Remove an extension record while retaining referenced package files."""
        self._require_open()
        if index < 0:
            raise IndexError(index)
        _check(_lib.ofd_remove_extension(self._handle, index))

    def custom_tag_data(self, index: int) -> bytes:
        """Return the original XML bytes for one custom tag entry."""
        self._require_open()
        if index < 0:
            raise IndexError(index)
        output = _ffi.new("OfdBuffer *")
        _check(_lib.ofd_custom_tag_data(self._handle, index, output))
        try:
            return bytes(_ffi.buffer(output.data, output.len))
        finally:
            _lib.ofd_buffer_free(output[0])

    def replace_custom_tag_data(self, index: int, data: bytes) -> dict:
        """Replace a custom tag XML payload and return its updated index entry."""
        self._require_open()
        if index < 0:
            raise IndexError(index)
        output = _ffi.new("OfdBuffer *")
        _check(_lib.ofd_replace_custom_tag_data(self._handle, index, data, len(data), output))
        try:
            return json.loads(bytes(_ffi.buffer(output.data, output.len)))
        finally:
            _lib.ofd_buffer_free(output[0])

    def remove_custom_tag(self, index: int) -> None:
        """Remove a custom tag record while retaining its XML payload."""
        self._require_open()
        if index < 0:
            raise IndexError(index)
        _check(_lib.ofd_remove_custom_tag(self._handle, index))

    def annotations(self) -> list[dict]:
        """Return annotation metadata from all document bodies."""
        self._require_open()
        output = _ffi.new("OfdBuffer *")
        _check(_lib.ofd_annotations_json(self._handle, output))
        try:
            return json.loads(bytes(_ffi.buffer(output.data, output.len)))
        finally:
            _lib.ofd_buffer_free(output[0])

    def annotation_objects(self, page_index: int, annotation_id: str | int) -> list[dict]:
        """Return an annotation's direct appearance objects in paint order."""
        self._require_open()
        if page_index < 0:
            raise IndexError(page_index)
        output = _ffi.new("OfdBuffer *")
        _check(_lib.ofd_annotation_objects_json(
            self._handle, page_index, _utf8(str(annotation_id)), output))
        try:
            return json.loads(bytes(_ffi.buffer(output.data, output.len)))
        finally:
            _lib.ofd_buffer_free(output[0])

    def annotation_object_actions(self, page_index: int, annotation_id: str | int, object_id: str | int) -> list[dict]:
        """Read ordered actions on an annotation appearance graphic."""
        self._require_open()
        if page_index < 0:
            raise IndexError(page_index)
        output = _ffi.new("OfdBuffer *")
        _check(_lib.ofd_annotation_object_actions_json(self._handle, page_index,
               _utf8(str(annotation_id)), _utf8(str(object_id)), output))
        try:
            return json.loads(bytes(_ffi.buffer(output.data, output.len)))
        finally:
            _lib.ofd_buffer_free(output[0])

    def add_annotation_object_action(self, page_index: int, annotation_id: str | int, object_id: str | int, action: dict) -> None:
        """Append a URI, page, bookmark, or attachment CLICK action."""
        self._require_open()
        if page_index < 0:
            raise IndexError(page_index)
        request = dict(action)
        request.setdefault("event", "CLICK")
        payload = json.dumps(request, ensure_ascii=False, allow_nan=False)
        _check(_lib.ofd_add_annotation_object_action_json(self._handle, page_index,
               _utf8(str(annotation_id)), _utf8(str(object_id)), _utf8(payload)))

    def remove_annotation_object_action(self, page_index: int, annotation_id: str | int, object_id: str | int, action_index: int) -> None:
        """Remove an annotation graphic action by zero-based index."""
        self._require_open()
        if page_index < 0 or action_index < 0:
            raise IndexError((page_index, action_index))
        _check(_lib.ofd_remove_annotation_object_action(self._handle, page_index,
               _utf8(str(annotation_id)), _utf8(str(object_id)), action_index))

    def set_annotation_object_action_regions(self, page_index: int, annotation_id: str | int, object_id: str | int, action_index: int, rectangles: list[list[float]]) -> None:
        """Set appearance-coordinate rectangles; an empty list removes Region."""
        self._require_open()
        if page_index < 0 or action_index < 0:
            raise IndexError((page_index, action_index))
        payload = json.dumps(rectangles, allow_nan=False)
        _check(_lib.ofd_set_annotation_object_action_regions_json(self._handle, page_index,
               _utf8(str(annotation_id)), _utf8(str(object_id)), action_index, _utf8(payload)))

    def append_annotation_objects(self, page_index: int, annotation_id: str | int,
                                  objects: list[dict]) -> list[str]:
        """Append path, text, image, or composite objects and return their OFD IDs."""
        self._require_open()
        if page_index < 0:
            raise IndexError(page_index)
        payload = json.dumps({"objects": objects}, allow_nan=False)
        output = _ffi.new("OfdBuffer *")
        _check(_lib.ofd_append_annotation_objects_json(
            self._handle, page_index, _utf8(str(annotation_id)),
            _utf8(payload), output))
        try:
            return json.loads(bytes(_ffi.buffer(output.data, output.len)))
        finally:
            _lib.ofd_buffer_free(output[0])

    def update_annotation_object(self, page_index: int, annotation_id: str | int,
                                 object_id: str | int, update: dict) -> None:
        """Atomically edit boundary, ctm, visible, or alpha of one object.

        Set ``ctm`` to ``None`` to remove that attribute.
        """
        self._require_open()
        if page_index < 0:
            raise IndexError(page_index)
        payload = json.dumps(update, allow_nan=False)
        _check(_lib.ofd_update_annotation_object_json(
            self._handle, page_index, _utf8(str(annotation_id)),
            _utf8(str(object_id)), _utf8(payload)))

    def remove_annotation_object(self, page_index: int, annotation_id: str | int,
                                 object_id: str | int) -> None:
        """Remove one object from an annotation appearance."""
        self._require_open()
        if page_index < 0:
            raise IndexError(page_index)
        _check(_lib.ofd_remove_annotation_object(
            self._handle, page_index, _utf8(str(annotation_id)), _utf8(str(object_id))))

    def set_annotation_object_name(self, page_index: int, annotation_id: str | int,
                                   object_id: str | int, name: str | None) -> None:
        """Set or clear an annotation graphic object's name."""
        self._require_open()
        if page_index < 0:
            raise IndexError(page_index)
        if name is not None and not isinstance(name, str):
            raise TypeError("name must be a string or None")
        encoded = _ffi.NULL if name is None else _utf8(name)
        _check(_lib.ofd_set_annotation_object_name(
            self._handle, page_index, _utf8(str(annotation_id)),
            _utf8(str(object_id)), encoded))

    def set_annotation_object_blend_mode(self, page_index: int, annotation_id: str | int,
                                         object_id: str | int,
                                         blend_mode: str | None) -> None:
        """Set or clear an annotation graphic object's compositing mode."""
        self._require_open()
        if page_index < 0:
            raise IndexError(page_index)
        if blend_mode is not None and not isinstance(blend_mode, str):
            raise TypeError("blend_mode must be a string or None")
        encoded = _ffi.NULL if blend_mode is None else _utf8(blend_mode)
        _check(_lib.ofd_set_annotation_object_blend_mode(
            self._handle, page_index, _utf8(str(annotation_id)),
            _utf8(str(object_id)), encoded))

    def set_annotation_object_draw_param(self, page_index: int, annotation_id: str | int,
                                         object_id: str | int,
                                         draw_param: str | int | None) -> None:
        """Set or clear a reusable DrawParam on an annotation graphic object."""
        self._require_open()
        if page_index < 0:
            raise IndexError(page_index)
        reference = _ffi.NULL if draw_param is None else _utf8(str(draw_param))
        _check(_lib.ofd_set_annotation_object_draw_param(
            self._handle, page_index, _utf8(str(annotation_id)),
            _utf8(str(object_id)), reference))

    def set_annotation_path_data(self, page_index: int, annotation_id: str | int,
                                 object_id: str | int, commands: str) -> None:
        """Replace one annotation path's abbreviated drawing commands."""
        self._require_open()
        if page_index < 0:
            raise IndexError(page_index)
        _check(_lib.ofd_set_annotation_path_data(
            self._handle, page_index, _utf8(str(annotation_id)),
            _utf8(str(object_id)), _utf8(commands)))

    def set_annotation_path_stroke_style(self, page_index: int, annotation_id: str | int,
                                         object_id: str | int, style: dict) -> None:
        """Replace direct annotation path stroke geometry atomically."""
        self._require_open()
        if page_index < 0:
            raise IndexError(page_index)
        _check(_lib.ofd_set_annotation_path_stroke_style_json(
            self._handle, page_index, _utf8(str(annotation_id)),
            _utf8(str(object_id)), _utf8(json.dumps(style))))

    def set_annotation_text_typography(self, page_index: int, annotation_id: str | int,
                                       object_id: str | int, style: dict) -> None:
        """Replace direct typography on one annotation TextObject."""
        self._require_open()
        if page_index < 0:
            raise IndexError(page_index)
        _check(_lib.ofd_set_annotation_text_typography_json(
            self._handle, page_index, _utf8(str(annotation_id)),
            _utf8(str(object_id)), _utf8(json.dumps(style))))

    def set_annotation_text_paint_flags(self, page_index: int,
                                        annotation_id: str | int,
                                        object_id: str | int,
                                        fill: bool | None = None,
                                        stroke: bool | None = None) -> None:
        """Set annotation text fill/stroke; None restores OFD defaults."""
        self._require_open()
        if page_index < 0:
            raise IndexError(page_index)
        _check(_lib.ofd_set_annotation_text_paint_flags_json(
            self._handle, page_index, _utf8(str(annotation_id)),
            _utf8(str(object_id)),
            _utf8(json.dumps({"fill": fill, "stroke": stroke}))))

    def set_annotation_text_code_layout(self, page_index: int, annotation_id: str | int,
                                        object_id: str | int, code_index: int,
                                        layout: dict) -> None:
        """Replace one annotation text run's direct coordinates and advances."""
        self._require_open()
        if page_index < 0 or code_index < 0:
            raise IndexError((page_index, code_index))
        _check(_lib.ofd_set_annotation_text_code_layout_json(
            self._handle, page_index, _utf8(str(annotation_id)),
            _utf8(str(object_id)), code_index, _utf8(json.dumps(layout))))

    def append_annotation_text_code(self, page_index: int, annotation_id: str | int,
                                    object_id: str | int, text: str,
                                    layout: dict | None = None) -> None:
        """Append one positioned run to an unshaped annotation TextObject."""
        self._require_open()
        if page_index < 0:
            raise IndexError(page_index)
        _check(_lib.ofd_edit_annotation_text_code_json(self._handle, _utf8(json.dumps({
            "operation": "append", "page_index": page_index,
            "annotation_id": str(annotation_id), "object_id": str(object_id),
            "text": text, "layout": layout or {},
        }))))

    def remove_annotation_text_code(self, page_index: int, annotation_id: str | int,
                                    object_id: str | int, code_index: int) -> None:
        """Remove a text run while preserving the next run's position."""
        self._require_open()
        if page_index < 0 or code_index < 0:
            raise IndexError((page_index, code_index))
        _check(_lib.ofd_remove_annotation_text_code(
            self._handle, page_index, _utf8(str(annotation_id)),
            _utf8(str(object_id)), code_index))

    def set_annotation_text_font(self, page_index: int, annotation_id: str | int,
                                 object_id: str | int, font_id: str | int) -> None:
        """Set the font of an annotation TextObject."""
        self._require_open()
        if page_index < 0:
            raise IndexError(page_index)
        _check(_lib.ofd_set_annotation_text_font(
            self._handle, page_index, _utf8(str(annotation_id)),
            _utf8(str(object_id)), _utf8(str(font_id))))

    def set_annotation_text_code(self, page_index: int, annotation_id: str | int,
                                 object_id: str | int, code_index: int, text: str) -> None:
        """Replace one unshaped annotation TextCode run."""
        self._require_open()
        if page_index < 0 or code_index < 0:
            raise IndexError((page_index, code_index))
        _check(_lib.ofd_edit_annotation_text_code_json(self._handle, _utf8(json.dumps({
            "operation": "set", "page_index": page_index,
            "annotation_id": str(annotation_id), "object_id": str(object_id),
            "code_index": code_index, "text": text,
        }))))

    def set_annotation_graphic_resource(self, page_index: int, annotation_id: str | int,
                                        object_id: str | int, resource_id: str | int) -> None:
        """Replace an annotation image or composite resource reference."""
        self._require_open()
        if page_index < 0:
            raise IndexError(page_index)
        _check(_lib.ofd_set_annotation_graphic_resource(
            self._handle, page_index, _utf8(str(annotation_id)),
            _utf8(str(object_id)), _utf8(str(resource_id))))

    def remove_annotation(self, page_index: int, annotation_id: str | int) -> None:
        """Remove an annotation from a page by its OFD ID."""
        self._require_open()
        if page_index < 0:
            raise IndexError(page_index)
        _check(_lib.ofd_remove_annotation(
            self._handle, page_index, _utf8(str(annotation_id))))

    def set_annotation_creator(self, page_index: int, annotation_id: str | int,
                               creator: str) -> None:
        """Set an annotation's Creator attribute."""
        self._require_open()
        if page_index < 0:
            raise IndexError(page_index)
        _check(_lib.ofd_set_annotation_creator(
            self._handle, page_index, _utf8(str(annotation_id)), _utf8(creator)))

    def set_annotation_subtype(self, page_index: int, annotation_id: str | int,
                               subtype: str | None) -> None:
        """Set or remove an annotation's optional Subtype."""
        self._require_open()
        if page_index < 0:
            raise IndexError(page_index)
        value = _ffi.NULL if subtype is None else _utf8(subtype)
        _check(_lib.ofd_set_annotation_subtype(
            self._handle, page_index, _utf8(str(annotation_id)), value))

    def set_annotation_last_mod_date(self, page_index: int, annotation_id: str | int,
                                     date: str) -> None:
        """Set LastModDate using a valid YYYY-MM-DD date."""
        self._require_open()
        if page_index < 0:
            raise IndexError(page_index)
        _check(_lib.ofd_set_annotation_last_mod_date(
            self._handle, page_index, _utf8(str(annotation_id)), _utf8(date)))

    def set_annotation_appearance_boundary(self, page_index: int,
                                           annotation_id: str | int,
                                           boundary: tuple[float, float, float, float]) -> None:
        """Move or resize an annotation's appearance boundary in millimeters."""
        self._require_open()
        if page_index < 0:
            raise IndexError(page_index)
        if len(boundary) != 4:
            raise ValueError("boundary must have four numbers")
        _check(_lib.ofd_set_annotation_appearance_boundary(
            self._handle, page_index, _utf8(str(annotation_id)), *boundary))

    def set_annotation_remark(self, page_index: int, annotation_id: str | int,
                              remark: str | None) -> None:
        """Set or remove an annotation's optional Remark content."""
        self._require_open()
        if page_index < 0:
            raise IndexError(page_index)
        value = _ffi.NULL if remark is None else _utf8(remark)
        _check(_lib.ofd_set_annotation_remark(
            self._handle, page_index, _utf8(str(annotation_id)), value))

    def set_annotation_parameter(self, page_index: int, annotation_id: str | int,
                                 name: str, value: str) -> None:
        """Add or replace a named annotation parameter."""
        self._require_open()
        if page_index < 0:
            raise IndexError(page_index)
        _check(_lib.ofd_set_annotation_parameter(
            self._handle, page_index, _utf8(str(annotation_id)), _utf8(name), _utf8(value)))

    def remove_annotation_parameter(self, page_index: int, annotation_id: str | int,
                                    name: str) -> None:
        """Remove a named annotation parameter."""
        self._require_open()
        if page_index < 0:
            raise IndexError(page_index)
        _check(_lib.ofd_remove_annotation_parameter(
            self._handle, page_index, _utf8(str(annotation_id)), _utf8(name)))

    def set_annotation_visible(self, page_index: int, annotation_id: str | int,
                               visible: bool) -> None:
        """Control whether an annotation appears in previews and PDF output."""
        self._require_open()
        if page_index < 0:
            raise IndexError(page_index)
        if not isinstance(visible, bool):
            raise TypeError("visible must be a bool")
        _check(_lib.ofd_set_annotation_visible(
            self._handle, page_index, _utf8(str(annotation_id)), int(visible)))

    def set_annotation_print(self, page_index: int, annotation_id: str | int,
                             print: bool) -> None:
        """Control whether an annotation is included in PDF output."""
        self._require_open()
        if page_index < 0:
            raise IndexError(page_index)
        if not isinstance(print, bool):
            raise TypeError("print must be a bool")
        _check(_lib.ofd_set_annotation_print(
            self._handle, page_index, _utf8(str(annotation_id)), int(print)))

    def set_annotation_flag(self, page_index: int, annotation_id: str | int,
                            flag: str, value: bool) -> None:
        """Set NoZoom, NoRotate, or ReadOnly metadata on an annotation."""
        self._require_open()
        if page_index < 0:
            raise IndexError(page_index)
        if flag not in ("NoZoom", "NoRotate", "ReadOnly"):
            raise ValueError("unsupported annotation flag")
        if not isinstance(value, bool):
            raise TypeError("value must be a bool")
        _check(_lib.ofd_set_annotation_flag(
            self._handle, page_index, _utf8(str(annotation_id)),
            _utf8(flag), int(value)))

    def fonts(self) -> list[dict]:
        """Return font resource metadata."""
        self._require_open()
        output = _ffi.new("OfdBuffer *")
        _check(_lib.ofd_fonts_json(self._handle, output))
        try:
            return json.loads(bytes(_ffi.buffer(output.data, output.len)))
        finally:
            _lib.ofd_buffer_free(output[0])

    def font_data(self, index: int) -> bytes:
        """Return the original bytes of an embedded font."""
        self._require_open()
        if index < 0:
            raise IndexError(index)
        output = _ffi.new("OfdBuffer *")
        _check(_lib.ofd_font_data(self._handle, index, output))
        try:
            return bytes(_ffi.buffer(output.data, output.len))
        finally:
            _lib.ofd_buffer_free(output[0])

    def page_fonts(self, page_index: int) -> list[dict]:
        """Return fonts available to a page and its templates."""
        self._require_open()
        if page_index < 0:
            raise IndexError(page_index)
        output = _ffi.new("OfdBuffer *")
        _check(_lib.ofd_page_fonts_json(self._handle, page_index, output))
        try:
            return json.loads(bytes(_ffi.buffer(output.data, output.len)))
        finally:
            _lib.ofd_buffer_free(output[0])

    def page_font_data(self, page_index: int, font_index: int) -> bytes:
        """Return embedded bytes for an index in :meth:`page_fonts`."""
        self._require_open()
        if page_index < 0:
            raise IndexError(page_index)
        if font_index < 0:
            raise IndexError(font_index)
        output = _ffi.new("OfdBuffer *")
        _check(_lib.ofd_page_font_data(self._handle, page_index, font_index, output))
        try:
            return bytes(_ffi.buffer(output.data, output.len))
        finally:
            _lib.ofd_buffer_free(output[0])

    def signatures(self) -> list[dict]:
        """Return signature descriptions without verifying their authenticity."""
        self._require_open()
        output = _ffi.new("OfdBuffer *")
        _check(_lib.ofd_signatures_json(self._handle, output))
        try:
            return json.loads(bytes(_ffi.buffer(output.data, output.len)))
        finally:
            _lib.ofd_buffer_free(output[0])

    def signed_value_data(self, index: int) -> bytes:
        """Return the opaque cryptographic signature value."""
        self._require_open()
        if index < 0:
            raise IndexError(index)
        output = _ffi.new("OfdBuffer *")
        _check(_lib.ofd_signed_value_data(self._handle, index, output))
        try:
            return bytes(_ffi.buffer(output.data, output.len))
        finally:
            _lib.ofd_buffer_free(output[0])

    def seal_data(self, index: int) -> bytes:
        """Return the electronic seal attached to a signature."""
        self._require_open()
        if index < 0:
            raise IndexError(index)
        output = _ffi.new("OfdBuffer *")
        _check(_lib.ofd_seal_data(self._handle, index, output))
        try:
            return bytes(_ffi.buffer(output.data, output.len))
        finally:
            _lib.ofd_buffer_free(output[0])

    def check_signature_references(self, index: int) -> list[dict]:
        """Compare protected file digests without authenticating the signature."""
        self._require_open()
        if index < 0:
            raise IndexError(index)
        output = _ffi.new("OfdBuffer *")
        _check(_lib.ofd_check_signature_references_json(self._handle, index, output))
        try:
            return json.loads(bytes(_ffi.buffer(output.data, output.len)))
        finally:
            _lib.ofd_buffer_free(output[0])

    def check_signature_description_hash(self, index: int) -> dict:
        """Compare Signature.xml with the hash in an SES v4/v5 seal."""
        self._require_open()
        if index < 0:
            raise IndexError(index)
        output = _ffi.new("OfdBuffer *")
        _check(_lib.ofd_check_signature_description_hash_json(self._handle, index, output))
        try:
            return json.loads(bytes(_ffi.buffer(output.data, output.len)))
        finally:
            _lib.ofd_buffer_free(output[0])

    def check_signature_seal_match(self, index: int) -> bool | None:
        """Compare a detached SES seal with the copy inside SignedValue.dat."""
        self._require_open()
        if index < 0:
            raise IndexError(index)
        output = _ffi.new("int *")
        _check(_lib.ofd_check_signature_seal_match(self._handle, index, output))
        return None if output[0] == -1 else bool(output[0])

    def check_ses_seal_signature(
        self, index: int, seal_distid: str = "1234567812345678"
    ) -> bool:
        """Verify the embedded electronic seal's SM2 signature, without issuer trust."""
        self._require_open()
        if index < 0:
            raise IndexError(index)
        if "\x00" in seal_distid:
            raise ValueError("seal_distid contains NUL")
        output = _ffi.new("int *")
        _check(_lib.ofd_check_ses_seal_signature_with_distid(
            self._handle, index, seal_distid.encode("utf-8"), output,
        ))
        return bool(output[0])

    def check_ses_seal_certificate_validity(
        self, index: int, at_unix_seconds: int
    ) -> bool:
        """Check the embedded seal issuer certificate validity at a supplied UTC time."""
        self._require_open()
        if index < 0:
            raise IndexError(index)
        if not 0 <= at_unix_seconds <= (1 << 64) - 1:
            raise ValueError("at_unix_seconds must fit an unsigned 64-bit integer")
        output = _ffi.new("int *")
        _check(_lib.ofd_check_ses_seal_certificate_validity(
            self._handle, index, at_unix_seconds, output,
        ))
        return bool(output[0])

    def check_ses_seal_signer_authorization(self, index: int) -> bool:
        """Check whether the signer certificate is listed in the SES seal."""
        self._require_open()
        if index < 0:
            raise IndexError(index)
        output = _ffi.new("int *")
        _check(_lib.ofd_check_ses_seal_signer_authorization(self._handle, index, output))
        return bool(output[0])

    def check_ses_seal_validity(self, index: int, at_unix_seconds: int) -> bool:
        """Check the electronic seal's validity at a caller-supplied UTC time."""
        self._require_open()
        if index < 0:
            raise IndexError(index)
        if not 0 <= at_unix_seconds <= (1 << 64) - 1:
            raise ValueError("at_unix_seconds must fit an unsigned 64-bit integer")
        output = _ffi.new("int *")
        _check(_lib.ofd_check_ses_seal_validity(
            self._handle, index, at_unix_seconds, output,
        ))
        return bool(output[0])

    def check_ses_seal_issuer_ordered_sm2_chain(
        self,
        index: int,
        intermediates_der: list[bytes],
        anchor_certificate_der: bytes,
        at_unix_seconds: int,
        certificate_distid: str = "",
    ) -> bool:
        """Check the embedded seal issuer's ordered SM2 chain to a trusted anchor."""
        self._require_open()
        if index < 0:
            raise IndexError(index)
        if not 0 <= at_unix_seconds <= (1 << 64) - 1:
            raise ValueError("at_unix_seconds must fit an unsigned 64-bit integer")
        if "\x00" in certificate_distid:
            raise ValueError("certificate_distid contains NUL")
        certificates = json.dumps([
            base64.b64encode(bytes(cert)).decode("ascii") for cert in intermediates_der
        ]).encode("ascii")
        anchor = bytes(anchor_certificate_der)
        output = _ffi.new("int *")
        _check(_lib.ofd_check_ses_seal_issuer_ordered_sm2_chain(
            self._handle, index, certificates, len(certificates), anchor, len(anchor),
            at_unix_seconds, certificate_distid.encode("utf-8"), output,
        ))
        return bool(output[0])

    def check_ses_signature_value(
        self, index: int, signature_distid: str = "1234567812345678"
    ) -> bool:
        """Check the SES SM2 signature using its embedded certificate public key."""
        self._require_open()
        if index < 0:
            raise IndexError(index)
        if "\x00" in signature_distid:
            raise ValueError("signature_distid contains NUL")
        output = _ffi.new("int *")
        _check(_lib.ofd_check_ses_signature_value_with_distid(
            self._handle, index, signature_distid.encode("utf-8"), output,
        ))
        return bool(output[0])

    def check_digital_signature_with_certificate(
        self, index: int, certificate_der: bytes, signature_distid: str = "1234567812345678"
    ) -> bool:
        """Check a raw SM2 signature with a caller-supplied DER certificate."""
        self._require_open()
        if index < 0:
            raise IndexError(index)
        if "\x00" in signature_distid:
            raise ValueError("signature_distid contains NUL")
        data = bytes(certificate_der)
        output = _ffi.new("int *")
        _check(_lib.ofd_check_digital_signature_with_certificate_and_distid(
            self._handle, index, data, len(data), signature_distid.encode("utf-8"), output,
        ))
        return bool(output[0])

    def check_signature_certificate_validity(
        self, index: int, at_unix_seconds: int, certificate_der: bytes = b""
    ) -> bool:
        """Check the signer's certificate validity at a supplied UTC Unix time."""
        self._require_open()
        if index < 0:
            raise IndexError(index)
        if not 0 <= at_unix_seconds <= (1 << 64) - 1:
            raise ValueError("at_unix_seconds must fit an unsigned 64-bit integer")
        data = bytes(certificate_der)
        output = _ffi.new("int *")
        _check(_lib.ofd_check_signature_certificate_validity(
            self._handle, index, data, len(data), at_unix_seconds, output
        ))
        return bool(output[0])

    def check_signature_direct_trust_anchor(
        self,
        index: int,
        signer_certificate_der: bytes,
        anchor_certificate_der: bytes,
        at_unix_seconds: int,
        certificate_distid: str = "",
    ) -> bool:
        """Check a direct SM2 issuer, validity, CA usage, and signature value."""
        self._require_open()
        if index < 0:
            raise IndexError(index)
        if not 0 <= at_unix_seconds <= (1 << 64) - 1:
            raise ValueError("at_unix_seconds must fit an unsigned 64-bit integer")
        if "\x00" in certificate_distid:
            raise ValueError("certificate_distid contains NUL")
        signer = bytes(signer_certificate_der)
        anchor = bytes(anchor_certificate_der)
        distid = certificate_distid.encode("utf-8")
        output = _ffi.new("int *")
        _check(_lib.ofd_check_signature_direct_trust_anchor(
            self._handle, index, signer, len(signer), anchor, len(anchor),
            at_unix_seconds, distid, output,
        ))
        return bool(output[0])

    def verify_signature_with_direct_trust_anchor(
        self,
        index: int,
        signer_certificate_der: bytes,
        anchor_certificate_der: bytes,
        at_unix_seconds: int,
        certificate_distid: str = "",
        signature_distid: str = "1234567812345678",
    ) -> dict:
        """Return combined digest, signature, validity, and direct trust checks."""
        self._require_open()
        if index < 0:
            raise IndexError(index)
        if not 0 <= at_unix_seconds <= (1 << 64) - 1:
            raise ValueError("at_unix_seconds must fit an unsigned 64-bit integer")
        if "\x00" in certificate_distid:
            raise ValueError("certificate_distid contains NUL")
        if "\x00" in signature_distid:
            raise ValueError("signature_distid contains NUL")
        signer = bytes(signer_certificate_der)
        anchor = bytes(anchor_certificate_der)
        distid = certificate_distid.encode("utf-8")
        output = _ffi.new("OfdBuffer *")
        _check(_lib.ofd_verify_signature_with_direct_trust_anchor_and_distids_json(
            self._handle, index, signer, len(signer), anchor, len(anchor),
            at_unix_seconds, distid, signature_distid.encode("utf-8"), output,
        ))
        try:
            return json.loads(bytes(_ffi.buffer(output.data, output.len)))
        finally:
            _lib.ofd_buffer_free(output[0])

    def verify_signature_with_ordered_sm2_chain(
        self,
        index: int,
        signer_certificate_der: bytes,
        intermediates_der: list[bytes],
        anchor_certificate_der: bytes,
        at_unix_seconds: int,
        certificate_distid: str = "",
        signature_distid: str = "1234567812345678",
    ) -> dict:
        """Return combined checks for an ordered signer-to-anchor SM2 chain."""
        return self._verify_signature_chain(
            index, signer_certificate_der, intermediates_der,
            anchor_certificate_der, at_unix_seconds,
            certificate_distid, signature_distid, False,
        )

    def verify_signature_with_certificate_pool(
        self, index: int, signer_certificate_der: bytes,
        candidate_certificates_der: list[bytes], anchor_certificate_der: bytes,
        at_unix_seconds: int, certificate_distid: str = "",
        signature_distid: str = "1234567812345678",
    ) -> dict:
        """Return signature checks after finding a valid chain among candidates."""
        return self._verify_signature_chain(
            index, signer_certificate_der, candidate_certificates_der,
            anchor_certificate_der, at_unix_seconds,
            certificate_distid, signature_distid, True,
        )

    def _verify_signature_chain(
        self, index: int, signer_certificate_der: bytes,
        intermediates_der: list[bytes], anchor_certificate_der: bytes,
        at_unix_seconds: int, certificate_distid: str,
        signature_distid: str, unordered: bool,
    ) -> dict:
        self._require_open()
        if index < 0:
            raise IndexError(index)
        if not 0 <= at_unix_seconds <= (1 << 64) - 1:
            raise ValueError("at_unix_seconds must fit an unsigned 64-bit integer")
        if "\x00" in certificate_distid:
            raise ValueError("certificate_distid contains NUL")
        if "\x00" in signature_distid:
            raise ValueError("signature_distid contains NUL")
        signer = bytes(signer_certificate_der)
        anchor = bytes(anchor_certificate_der)
        certificates = json.dumps([
            base64.b64encode(bytes(cert)).decode("ascii") for cert in intermediates_der
        ]).encode("ascii")
        distid = certificate_distid.encode("utf-8")
        output = _ffi.new("OfdBuffer *")
        function = (_lib.ofd_verify_signature_with_certificate_pool_json
                    if unordered else _lib.ofd_verify_signature_with_ordered_sm2_chain_and_distids_json)
        _check(function(
            self._handle, index, signer, len(signer), certificates, len(certificates),
            anchor, len(anchor), at_unix_seconds, distid,
            signature_distid.encode("utf-8"), output,
        ))
        try:
            return json.loads(bytes(_ffi.buffer(output.data, output.len)))
        finally:
            _lib.ofd_buffer_free(output[0])

    def verify_signature_with_ordered_sm2_chains(
        self,
        index: int,
        signer_certificate_der: bytes,
        signer_intermediates_der: list[bytes],
        signer_anchor_der: bytes,
        seal_intermediates_der: list[bytes],
        seal_anchor_der: bytes,
        at_unix_seconds: int,
        signer_certificate_distid: str = "",
        signer_signature_distid: str = "1234567812345678",
        seal_certificate_distid: str = "",
        seal_signature_distid: str = "1234567812345678",
    ) -> dict:
        """Verify signer and seal issuer with separate trusted SM2 chains."""
        return self._verify_signature_trust(
            index, signer_certificate_der, signer_intermediates_der,
            signer_anchor_der, seal_intermediates_der, seal_anchor_der,
            at_unix_seconds, signer_certificate_distid, signer_signature_distid,
            seal_certificate_distid, seal_signature_distid, False,
        )

    def verify_signature_with_certificate_pools(
        self, index: int, signer_certificate_der: bytes,
        signer_candidates_der: list[bytes], signer_anchor_der: bytes,
        seal_candidates_der: list[bytes], seal_anchor_der: bytes,
        at_unix_seconds: int, signer_certificate_distid: str = "",
        signer_signature_distid: str = "1234567812345678",
        seal_certificate_distid: str = "",
        seal_signature_distid: str = "1234567812345678",
    ) -> dict:
        """Find valid signer and SES seal issuer paths in separate candidate pools."""
        return self._verify_signature_trust(
            index, signer_certificate_der, signer_candidates_der,
            signer_anchor_der, seal_candidates_der, seal_anchor_der,
            at_unix_seconds, signer_certificate_distid, signer_signature_distid,
            seal_certificate_distid, seal_signature_distid, True,
        )

    def _verify_signature_trust(
        self, index: int, signer_certificate_der: bytes,
        signer_intermediates_der: list[bytes], signer_anchor_der: bytes,
        seal_intermediates_der: list[bytes], seal_anchor_der: bytes,
        at_unix_seconds: int, signer_certificate_distid: str,
        signer_signature_distid: str, seal_certificate_distid: str,
        seal_signature_distid: str, unordered: bool,
    ) -> dict:
        self._require_open()
        if index < 0:
            raise IndexError(index)
        if not 0 <= at_unix_seconds <= (1 << 64) - 1:
            raise ValueError("at_unix_seconds must fit an unsigned 64-bit integer")
        payload = {
            "signer_certificate_der_base64": base64.b64encode(bytes(signer_certificate_der)).decode("ascii"),
            "signer_intermediates_der_base64": [
                base64.b64encode(bytes(cert)).decode("ascii") for cert in signer_intermediates_der
            ],
            "signer_anchor_der_base64": base64.b64encode(bytes(signer_anchor_der)).decode("ascii"),
            "seal_intermediates_der_base64": [
                base64.b64encode(bytes(cert)).decode("ascii") for cert in seal_intermediates_der
            ],
            "seal_anchor_der_base64": base64.b64encode(bytes(seal_anchor_der)).decode("ascii"),
            "at_unix_seconds": at_unix_seconds,
            "signer_certificate_distid": signer_certificate_distid,
            "signer_signature_distid": signer_signature_distid,
            "seal_certificate_distid": seal_certificate_distid,
            "seal_signature_distid": seal_signature_distid,
        }
        encoded = json.dumps(payload, ensure_ascii=False).encode("utf-8")
        output = _ffi.new("OfdBuffer *")
        function = (_lib.ofd_verify_signature_with_certificate_pools_json
                    if unordered else _lib.ofd_verify_signature_with_ordered_sm2_chains_json)
        _check(function(
            self._handle, index, encoded, len(encoded), output,
        ))
        try:
            return json.loads(bytes(_ffi.buffer(output.data, output.len)))
        finally:
            _lib.ofd_buffer_free(output[0])

    def add_digital_signature(
        self, private_key: bytes, signature_datetime: str,
        signature_distid: str = "1234567812345678",
    ) -> int:
        """Add or append an SM2 signature with a 32-byte private key scalar."""
        self._require_open()
        if "\x00" in signature_datetime:
            raise ValueError("signature_datetime contains NUL")
        if "\x00" in signature_distid:
            raise ValueError("signature_distid contains NUL")
        key = bytes(private_key)
        date = signature_datetime.encode("utf-8")
        output = _ffi.new("size_t *")
        _check(_lib.ofd_add_digital_signature_with_distid(
            self._handle, key, len(key), date, signature_distid.encode("utf-8"), output,
        ))
        return int(output[0])

    def add_ses_signature(
        self,
        private_key: bytes,
        signer_certificate_der: bytes,
        seal_der: bytes,
        signature_datetime: str,
        signature_distid: str = "1234567812345678",
        seal_distid: str = "1234567812345678",
        page_index: int | None = None,
        boundary: tuple[float, float, float, float] | None = None,
    ) -> int:
        """Add an SES v4/v5 signature, optionally showing its seal on a page."""
        self._require_open()
        for name, value in (
            ("signature_datetime", signature_datetime),
            ("signature_distid", signature_distid),
            ("seal_distid", seal_distid),
        ):
            if "\x00" in value:
                raise ValueError(f"{name} contains NUL")
        key = bytes(private_key)
        certificate = bytes(signer_certificate_der)
        seal = bytes(seal_der)
        output = _ffi.new("size_t *")
        if (page_index is None) != (boundary is None):
            raise ValueError("page_index and boundary must be supplied together")
        if page_index is None:
            _check(_lib.ofd_add_ses_signature_with_distids(
                self._handle, key, len(key), certificate, len(certificate), seal, len(seal),
                signature_datetime.encode("utf-8"), signature_distid.encode("utf-8"),
                seal_distid.encode("utf-8"), output,
            ))
        else:
            if page_index < 0:
                raise IndexError(page_index)
            if len(boundary) != 4:
                raise ValueError("boundary must contain x, y, width, height")
            x, y, width, height = map(float, boundary)
            _check(_lib.ofd_add_ses_signature_with_stamp_and_distids(
                self._handle, key, len(key), certificate, len(certificate), seal, len(seal),
                signature_datetime.encode("utf-8"), page_index, x, y, width, height,
                signature_distid.encode("utf-8"), seal_distid.encode("utf-8"), output,
            ))
        return int(output[0])

    def attachment_data(self, index: int) -> bytes:
        """Return bytes of a zero-based attachment."""
        self._require_open()
        if index < 0:
            raise IndexError(index)
        output = _ffi.new("OfdBuffer *")
        _check(_lib.ofd_attachment_data(self._handle, index, output))
        try:
            return bytes(_ffi.buffer(output.data, output.len))
        finally:
            _lib.ofd_buffer_free(output[0])

    def set_entry(self, path: str, data: bytes) -> None:
        """Replace raw package bytes, including signed entries; verify signatures afterward."""
        self._require_open()
        _check(_lib.ofd_set_entry(self._handle, _utf8(path), data, len(data)))

    def remove_entry(self, path: str) -> bool:
        """Remove a raw package entry and report whether it existed."""
        self._require_open()
        removed = _ffi.new("int *")
        _check(_lib.ofd_remove_entry(self._handle, _utf8(path), removed))
        return bool(removed[0])

    def add_text_page(self, text: str, width_mm: float = 210, height_mm: float = 297) -> None:
        self._require_open()
        payload = {"operation": "add", "text": text,
                   "width_mm": width_mm, "height_mm": height_mm}
        _check(_lib.ofd_write_text_page_json(
            self._handle, _utf8(json.dumps(payload)), _ffi.NULL))

    def add_blank_page(self, width_mm: float = 210, height_mm: float = 297) -> None:
        """Append an empty page to a newly created document."""
        self._require_open()
        _check(_lib.ofd_add_blank_page(self._handle, width_mm, height_mm))

    def add_text_object(
        self,
        page_index: int,
        text: str,
        boundary: tuple[float, float, float, float],
        size: float = 5,
        font_id: str | int | None = None,
        shape: bool = False,
        language: str | None = None,
        fill_color: tuple[int, int, int] | None = None,
    ) -> int:
        """Add positioned text; shape=True uses an embedded font for one line."""
        self._require_open()
        if page_index < 0:
            raise IndexError(page_index)
        options = {"text": text, "boundary": boundary, "size": size,
                   "font_id": None if font_id is None else str(font_id), "shape": shape}
        if language is not None:
            options["language"] = language
        if fill_color is not None:
            options["fill_color"] = fill_color
        payload = json.dumps(options, ensure_ascii=False, allow_nan=False)
        output = _ffi.new("size_t *")
        _check(_lib.ofd_add_text_object_json(self._handle, page_index, _utf8(payload), output))
        return int(output[0])

    def add_composite_text_object(
        self,
        page_index: int,
        text: str,
        boundary: tuple[float, float, float, float],
        size: float = 5,
        font_id: str | int | None = None,
    ) -> int:
        """Create a composite resource with one text object and place it."""
        self._require_open()
        if page_index < 0:
            raise IndexError(page_index)
        payload = json.dumps(
            {"text": text, "boundary": boundary, "size": size,
             "font_id": None if font_id is None else str(font_id)},
            ensure_ascii=False,
            allow_nan=False,
        )
        output = _ffi.new("size_t *")
        _check(_lib.ofd_add_composite_text_object_json(self._handle, page_index, _utf8(payload), output))
        return int(output[0])

    def remove_page_object(self, page_index: int, object_id: str | int) -> None:
        """Remove one text, path, or image object from a page by OFD ID."""
        self._require_open()
        if page_index < 0:
            raise IndexError(page_index)
        _check(_lib.ofd_remove_page_object(self._handle, page_index, _utf8(str(object_id))))

    def duplicate_page_object(self, page_index: int, object_id: str | int) -> int:
        """Copy a page graphic object after itself and return its new ID."""
        self._require_open()
        if page_index < 0:
            raise IndexError(page_index)
        output = _ffi.new("size_t *")
        _check(_lib.ofd_duplicate_page_object(
            self._handle, page_index, _utf8(str(object_id)), output))
        return int(output[0])

    def page_color_spaces(self, page_index: int) -> list[dict]:
        """List effective page color spaces, including their source resource files."""
        self._require_open()
        if page_index < 0:
            raise IndexError(page_index)
        output = _ffi.new("OfdBuffer *")
        _check(_lib.ofd_page_color_spaces_json(self._handle, page_index, output))
        try:
            return json.loads(bytes(_ffi.buffer(output.data, output.len)))
        finally:
            _lib.ofd_buffer_free(output[0])

    def document_color_spaces(self, document_index: int) -> list[dict]:
        """List color spaces declared in a document's resources."""
        self._require_open()
        if document_index < 0:
            raise IndexError(document_index)
        output = _ffi.new("OfdBuffer *")
        _check(_lib.ofd_document_color_spaces_json(self._handle, document_index, output))
        try:
            return json.loads(bytes(_ffi.buffer(output.data, output.len)))
        finally:
            _lib.ofd_buffer_free(output[0])

    def document_draw_params(self, document_index: int) -> list[dict]:
        """List drawing parameters declared in a document's resources."""
        self._require_open()
        if document_index < 0:
            raise IndexError(document_index)
        output = _ffi.new("OfdBuffer *")
        _check(_lib.ofd_document_draw_params_json(self._handle, document_index, output))
        try:
            return json.loads(bytes(_ffi.buffer(output.data, output.len)))
        finally:
            _lib.ofd_buffer_free(output[0])

    def page_draw_params(self, page_index: int) -> list[dict]:
        """List drawing parameters visible on a page, including document resources."""
        self._require_open()
        if page_index < 0:
            raise IndexError(page_index)
        output = _ffi.new("OfdBuffer *")
        _check(_lib.ofd_page_draw_params_json(self._handle, page_index, output))
        try:
            return json.loads(bytes(_ffi.buffer(output.data, output.len)))
        finally:
            _lib.ofd_buffer_free(output[0])

    def add_document_color_space(self, document_index: int, spec: dict) -> int:
        """Create a document color space and return its OFD resource ID."""
        self._require_open()
        if document_index < 0:
            raise IndexError(document_index)
        output = _ffi.new("size_t *")
        payload = json.dumps(spec, ensure_ascii=False, allow_nan=False)
        _check(_lib.ofd_add_document_color_space_json(
            self._handle, document_index, _utf8(payload), output))
        return int(output[0])

    def add_page_color_space(self, page_index: int, spec: dict) -> int:
        """Create a page-local color space and return its OFD resource ID."""
        self._require_open()
        if page_index < 0:
            raise IndexError(page_index)
        output = _ffi.new("size_t *")
        payload = json.dumps(spec, ensure_ascii=False, allow_nan=False)
        _check(_lib.ofd_add_page_color_space_json(
            self._handle, page_index, _utf8(payload), output))
        return int(output[0])

    def set_document_color_space(self, document_index: int, color_space_id: str | int,
                                 spec: dict) -> None:
        """Replace a document ColorSpace while preserving its resource ID."""
        self._require_open()
        if document_index < 0:
            raise IndexError(document_index)
        payload = json.dumps(spec, ensure_ascii=False, allow_nan=False)
        _check(_lib.ofd_set_document_color_space_json(
            self._handle, document_index, _utf8(str(color_space_id)), _utf8(payload)))

    def set_page_color_space(self, page_index: int, color_space_id: str | int,
                             spec: dict) -> None:
        """Replace a page-local ColorSpace while preserving its resource ID."""
        self._require_open()
        if page_index < 0:
            raise IndexError(page_index)
        payload = json.dumps(spec, ensure_ascii=False, allow_nan=False)
        _check(_lib.ofd_set_page_color_space_json(
            self._handle, page_index, _utf8(str(color_space_id)), _utf8(payload)))

    def remove_document_color_space(self, document_index: int, color_space_id: str | int) -> None:
        """Remove an unreferenced document ColorSpace definition."""
        self._require_open()
        if document_index < 0:
            raise IndexError(document_index)
        _check(_lib.ofd_remove_document_color_space(
            self._handle, document_index, _utf8(str(color_space_id))))

    def remove_page_color_space(self, page_index: int, color_space_id: str | int) -> None:
        """Remove an unreferenced page ColorSpace definition."""
        self._require_open()
        if page_index < 0:
            raise IndexError(page_index)
        _check(_lib.ofd_remove_page_color_space(
            self._handle, page_index, _utf8(str(color_space_id))))

    def document_default_color_space(self, document_index: int) -> str | None:
        """Return the document's DefaultCS ID, if present."""
        self._require_open()
        if document_index < 0:
            raise IndexError(document_index)
        output = _ffi.new("OfdBuffer *")
        _check(_lib.ofd_document_default_color_space_json(self._handle, document_index, output))
        try:
            return json.loads(bytes(_ffi.buffer(output.data, output.len)))
        finally:
            _lib.ofd_buffer_free(output[0])

    def set_document_default_color_space(self, document_index: int,
                                         color_space_id: str | int | None) -> None:
        """Set or clear DefaultCS using a document-level color space ID."""
        self._require_open()
        if document_index < 0:
            raise IndexError(document_index)
        reference = _ffi.NULL if color_space_id is None else _utf8(str(color_space_id))
        _check(_lib.ofd_set_document_default_color_space(
            self._handle, document_index, reference))

    def add_document_draw_param(self, document_index: int, style: dict) -> int:
        """Create a reusable DrawParam with solid, gradient, mesh or pattern paints."""
        self._require_open()
        if document_index < 0:
            raise IndexError(document_index)
        output = _ffi.new("size_t *")
        _check(_lib.ofd_add_document_draw_param_json(
            self._handle, document_index, _utf8(json.dumps(style)), output))
        return int(output[0])

    def add_page_draw_param(self, page_index: int, style: dict) -> int:
        """Create a DrawParam in one page's resource scope."""
        self._require_open()
        if page_index < 0:
            raise IndexError(page_index)
        output = _ffi.new("size_t *")
        _check(_lib.ofd_add_page_draw_param_json(
            self._handle, page_index, _utf8(json.dumps(style)), output))
        return int(output[0])

    def set_document_draw_param(self, document_index: int, draw_param_id: str | int,
                                style: dict) -> None:
        """Replace a document DrawParam while preserving its resource ID."""
        self._require_open()
        if document_index < 0:
            raise IndexError(document_index)
        _check(_lib.ofd_set_document_draw_param_json(
            self._handle, document_index, _utf8(str(draw_param_id)), _utf8(json.dumps(style))))

    def set_page_draw_param(self, page_index: int, draw_param_id: str | int,
                            style: dict) -> None:
        """Replace a DrawParam declared in the page's resources."""
        self._require_open()
        if page_index < 0:
            raise IndexError(page_index)
        _check(_lib.ofd_set_page_draw_param_json(
            self._handle, page_index, _utf8(str(draw_param_id)), _utf8(json.dumps(style))))

    def remove_document_draw_param(self, document_index: int,
                                   draw_param_id: str | int) -> None:
        """Remove an unreferenced document DrawParam."""
        self._require_open()
        if document_index < 0:
            raise IndexError(document_index)
        _check(_lib.ofd_remove_document_draw_param(
            self._handle, document_index, _utf8(str(draw_param_id))))

    def remove_page_draw_param(self, page_index: int,
                               draw_param_id: str | int) -> None:
        """Remove an unreferenced page-local DrawParam."""
        self._require_open()
        if page_index < 0:
            raise IndexError(page_index)
        _check(_lib.ofd_remove_page_draw_param(
            self._handle, page_index, _utf8(str(draw_param_id))))

    def set_page_object_draw_param(self, page_index: int, object_id: str | int,
                                   draw_param: str | int | None) -> None:
        """Set or clear a direct page object's reusable DrawParam reference."""
        self._require_open()
        if page_index < 0:
            raise IndexError(page_index)
        reference = _ffi.NULL if draw_param is None else _utf8(str(draw_param))
        _check(_lib.ofd_set_page_object_draw_param(
            self._handle, page_index, _utf8(str(object_id)), reference))

    def add_page_layer(self, page_index: int, layer_type: str, draw_param: str | int | None = None) -> int:
        """Create a Background, Body, or Foreground layer and return its ID."""
        self._require_open()
        if page_index < 0:
            raise IndexError(page_index)
        output = _ffi.new("size_t *")
        reference = _ffi.NULL if draw_param is None else _utf8(str(draw_param))
        _check(_lib.ofd_add_page_layer(self._handle, page_index, _utf8(layer_type), reference, output))
        return int(output[0])

    def set_page_layer_type(self, page_index: int, layer_id: str | int, layer_type: str) -> None:
        """Change a direct page layer's Background, Body, or Foreground type."""
        self._require_open()
        if page_index < 0:
            raise IndexError(page_index)
        _check(_lib.ofd_set_page_layer_type(
            self._handle, page_index, _utf8(str(layer_id)), _utf8(layer_type)))

    def set_page_layer_name(self, page_index: int, layer_id: str | int, name: str | None) -> None:
        """Set or clear a direct page layer's optional name."""
        self._require_open()
        if page_index < 0:
            raise IndexError(page_index)
        value = _ffi.NULL if name is None else _utf8(name)
        _check(_lib.ofd_set_page_layer_name(
            self._handle, page_index, _utf8(str(layer_id)), value))

    def set_page_layer_draw_param(self, page_index: int, layer_id: str | int,
                                  draw_param: str | int | None) -> None:
        """Set or clear a direct page layer's reusable DrawParam reference."""
        self._require_open()
        if page_index < 0:
            raise IndexError(page_index)
        reference = _ffi.NULL if draw_param is None else _utf8(str(draw_param))
        _check(_lib.ofd_set_page_layer_draw_param(
            self._handle, page_index, _utf8(str(layer_id)), reference))

    def move_page_object_to_layer(self, page_index: int, object_id: str | int, layer_id: str | int) -> None:
        """Move one page graphic object into an existing layer."""
        self._require_open()
        if page_index < 0:
            raise IndexError(page_index)
        _check(_lib.ofd_move_page_object_to_layer(
            self._handle, page_index, _utf8(str(object_id)), _utf8(str(layer_id))))

    def group_page_objects(self, page_index: int, first_id: str | int,
                           last_id: str | int) -> None:
        """Wrap inclusive, ordered sibling objects in a nested PageBlock."""
        self._require_open()
        if page_index < 0:
            raise IndexError(page_index)
        _check(_lib.ofd_group_page_objects(
            self._handle, page_index, _utf8(str(first_id)), _utf8(str(last_id))))

    def ungroup_page_objects(self, page_index: int, object_id: str | int) -> None:
        """Remove the PageBlock directly containing an object."""
        self._require_open()
        if page_index < 0:
            raise IndexError(page_index)
        _check(_lib.ofd_ungroup_page_objects(
            self._handle, page_index, _utf8(str(object_id))))

    def move_page_object(self, page_index: int, object_id: str | int,
                         before_id: str | int | None = None) -> None:
        """Move an object before another object, or to the end of its parent."""
        self._require_open()
        if page_index < 0:
            raise IndexError(page_index)
        before = _ffi.NULL if before_id is None else _utf8(str(before_id))
        _check(_lib.ofd_move_page_object(
            self._handle, page_index, _utf8(str(object_id)), before))

    def set_page_object_name(self, page_index: int, object_id: str | int,
                             name: str | None) -> None:
        """Set or clear a direct page object's optional name."""
        self._require_open()
        if page_index < 0:
            raise IndexError(page_index)
        if name is not None and not isinstance(name, str):
            raise TypeError("name must be a string or None")
        encoded = _ffi.NULL if name is None else _utf8(name)
        _check(_lib.ofd_set_page_object_name(
            self._handle, page_index, _utf8(str(object_id)), encoded))

    def set_page_object_blend_mode(self, page_index: int, object_id: str | int,
                                   blend_mode: str | None) -> None:
        """Set or clear a direct page object's compositing mode."""
        self._require_open()
        if page_index < 0:
            raise IndexError(page_index)
        if blend_mode is not None and not isinstance(blend_mode, str):
            raise TypeError("blend_mode must be a string or None")
        encoded = _ffi.NULL if blend_mode is None else _utf8(blend_mode)
        _check(_lib.ofd_set_page_object_blend_mode(
            self._handle, page_index, _utf8(str(object_id)), encoded))

    def set_page_object_clip_rects(self, page_index: int, object_id: str | int,
                                   rects: list[list[float]], follows_ctm: bool = False) -> None:
        """Replace object-local rectangular clips; an empty list clears them."""
        self._require_open()
        if page_index < 0:
            raise IndexError(page_index)
        if not isinstance(follows_ctm, bool):
            raise TypeError("follows_ctm must be a bool")
        payload = json.dumps({"rects": rects, "follows_ctm": follows_ctm}, allow_nan=False)
        _check(_lib.ofd_set_page_object_clip_rects_json(
            self._handle, page_index, _utf8(str(object_id)), _utf8(payload)))

    def set_page_object_path_clips(self, page_index: int, object_id: str | int,
                                   clips: list[list[dict]], follows_ctm: bool = False) -> None:
        """Replace path clips; areas in each clip unite, and clips intersect."""
        self._require_open()
        if page_index < 0:
            raise IndexError(page_index)
        if not isinstance(follows_ctm, bool):
            raise TypeError("follows_ctm must be a bool")
        payload = json.dumps({"clips": clips, "follows_ctm": follows_ctm}, allow_nan=False)
        _check(_lib.ofd_set_page_object_path_clips_json(
            self._handle, page_index, _utf8(str(object_id)), _utf8(payload)))

    def set_page_object_clips(self, page_index: int, object_id: str | int,
                              clips: list[list[dict]], follows_ctm: bool = False) -> None:
        """Replace path and text clip areas; an empty list clears the clips."""
        self._require_open()
        if page_index < 0:
            raise IndexError(page_index)
        if not isinstance(follows_ctm, bool):
            raise TypeError("follows_ctm must be a bool")
        payload = json.dumps({"clips": clips, "follows_ctm": follows_ctm},
                             ensure_ascii=False, allow_nan=False)
        _check(_lib.ofd_set_page_object_clips_json(
            self._handle, page_index, _utf8(str(object_id)), _utf8(payload)))

    def set_page_object_visible(self, page_index: int, object_id: str | int,
                                visible: bool) -> None:
        """Show or hide a page object in previews and text extraction."""
        self._require_open()
        if page_index < 0:
            raise IndexError(page_index)
        if not isinstance(visible, bool):
            raise TypeError("visible must be a bool")
        _check(_lib.ofd_set_page_object_visible(
            self._handle, page_index, _utf8(str(object_id)), int(visible)))

    def set_page_object_alpha(self, page_index: int, object_id: str | int,
                              alpha: int) -> None:
        """Set whole-object opacity from 0 (transparent) to 255 (opaque)."""
        self._require_open()
        if page_index < 0:
            raise IndexError(page_index)
        if not isinstance(alpha, int) or isinstance(alpha, bool) or not 0 <= alpha <= 255:
            raise ValueError("alpha must be an integer in 0..255")
        _check(_lib.ofd_set_page_object_alpha(
            self._handle, page_index, _utf8(str(object_id)), alpha))

    def set_page_path_stroke_style(self, page_index: int, object_id: str | int,
                                   style: dict) -> None:
        """Atomically set direct path stroke geometry; omitted options inherit."""
        self._require_open()
        if page_index < 0:
            raise IndexError(page_index)
        _check(_lib.ofd_set_page_path_stroke_style_json(
            self._handle, page_index, _utf8(str(object_id)),
            _utf8(json.dumps(style))))

    def set_page_path_dash_sequence(self, page_index: int, object_id: str | int,
                                    dash_sequence: list[float] | None,
                                    dash_offset: float | None = None) -> None:
        """Set 1..256 positive dash lengths, or clear direct dashes with None."""
        self._require_open()
        if page_index < 0:
            raise IndexError(page_index)
        payload = {"dash_sequence": dash_sequence, "dash_offset": dash_offset}
        _check(_lib.ofd_set_page_path_dash_sequence_json(
            self._handle, page_index, _utf8(str(object_id)),
            _utf8(json.dumps(payload))))

    def set_page_path_data(self, page_index: int, object_id: str | int,
                           commands: str) -> None:
        """Replace one page path's abbreviated OFD drawing commands."""
        self._require_open()
        if page_index < 0:
            raise IndexError(page_index)
        _check(_lib.ofd_set_page_path_data(
            self._handle, page_index, _utf8(str(object_id)), _utf8(commands)))

    def set_page_text_typography(self, page_index: int, object_id: str | int,
                                 style: dict) -> None:
        """Atomically set text size and optional typography attributes."""
        self._require_open()
        if page_index < 0:
            raise IndexError(page_index)
        _check(_lib.ofd_set_page_text_typography_json(
            self._handle, page_index, _utf8(str(object_id)),
            _utf8(json.dumps(style))))

    def set_page_text_code(self, page_index: int, object_id: str | int,
                           code_index: int, text: str) -> None:
        """Replace one TextCode run on an unshaped page text object."""
        self._require_open()
        if page_index < 0:
            raise IndexError(page_index)
        if code_index < 0:
            raise IndexError(code_index)
        payload = {"operation": "set", "page_index": page_index,
                   "object_id": str(object_id), "code_index": code_index,
                   "text": text}
        _check(_lib.ofd_edit_text_code_json(
            self._handle, _utf8(json.dumps(payload))))

    def set_page_text_code_layout(self, page_index: int,
                                  object_id: str | int, code_index: int,
                                  layout: dict) -> None:
        """Set one TextCode run's X, Y, DeltaX and DeltaY attributes."""
        self._require_open()
        if page_index < 0:
            raise IndexError(page_index)
        if code_index < 0:
            raise IndexError(code_index)
        _check(_lib.ofd_set_page_text_code_layout_json(
            self._handle, page_index, _utf8(str(object_id)), code_index,
            _utf8(json.dumps(layout))))

    def append_page_text_code(self, page_index: int, object_id: str | int,
                              text: str, layout: dict) -> None:
        """Append a positioned TextCode run to one unshaped text object."""
        self._require_open()
        if page_index < 0:
            raise IndexError(page_index)
        payload = {"operation": "append", "page_index": page_index,
                   "object_id": str(object_id), "text": text,
                   "layout": layout}
        _check(_lib.ofd_edit_text_code_json(
            self._handle, _utf8(json.dumps(payload))))

    def remove_page_text_code(self, page_index: int, object_id: str | int,
                              code_index: int) -> None:
        """Remove one run while preserving the next run's effective coordinates."""
        self._require_open()
        if page_index < 0:
            raise IndexError(page_index)
        if code_index < 0:
            raise IndexError(code_index)
        _check(_lib.ofd_remove_page_text_code(
            self._handle, page_index, _utf8(str(object_id)), code_index))

    def set_page_text_font(self, page_index: int, object_id: str | int,
                           font_id: str | int) -> None:
        """Use a font resource visible to this page for one text object."""
        self._require_open()
        if page_index < 0:
            raise IndexError(page_index)
        _check(_lib.ofd_set_page_text_font(
            self._handle, page_index, _utf8(str(object_id)),
            _utf8(str(font_id))))

    def set_page_text_paint_flags(self, page_index: int,
                                  object_id: str | int,
                                  fill: bool | None = None,
                                  stroke: bool | None = None) -> None:
        """Set text fill/stroke; None restores the corresponding OFD default."""
        self._require_open()
        if page_index < 0:
            raise IndexError(page_index)
        _check(_lib.ofd_set_page_text_paint_flags_json(
            self._handle, page_index, _utf8(str(object_id)),
            _utf8(json.dumps({"fill": fill, "stroke": stroke}))))

    def set_page_image_interpolate(self, page_index: int, object_id: str | int,
                                   interpolate: bool | None = None) -> None:
        """Choose nearest (False), smooth (True), or restore smooth default (None)."""
        self._require_open()
        if page_index < 0:
            raise IndexError(page_index)
        if interpolate is not None and not isinstance(interpolate, bool):
            raise TypeError("interpolate must be a bool or None")
        _check(_lib.ofd_set_page_image_interpolate(
            self._handle, page_index, _utf8(str(object_id)),
            -1 if interpolate is None else int(interpolate)))

    def set_page_image_mask(self, page_index: int, object_id: str | int,
                            mask_resource_id: str | int | None) -> None:
        """Set an image mask resource ID, or clear it with None."""
        self._require_open()
        if page_index < 0:
            raise IndexError(page_index)
        mask = _ffi.NULL if mask_resource_id is None else _utf8(str(mask_resource_id))
        _check(_lib.ofd_set_page_image_mask(
            self._handle, page_index, _utf8(str(object_id)), mask))

    def set_page_image_substitution(self, page_index: int, object_id: str | int,
                                    substitution_resource_id: str | int | None) -> None:
        """Set an alternative image resource ID, or clear it with None."""
        self._require_open()
        if page_index < 0:
            raise IndexError(page_index)
        substitution = (_ffi.NULL if substitution_resource_id is None
                        else _utf8(str(substitution_resource_id)))
        _check(_lib.ofd_set_page_image_substitution(
            self._handle, page_index, _utf8(str(object_id)), substitution))

    def set_page_image_border(self, page_index: int, object_id: str | int,
                              style: dict | None) -> None:
        """Replace an image border, or remove it with None.

        Style fields include line_width, horizontal_corner_radius,
        vertical_corner_radius, dash_pattern, dash_offset, color (RGB),
        and color_alpha. Omitted fields use OFD defaults.
        """
        self._require_open()
        if page_index < 0:
            raise IndexError(page_index)
        if style is not None and not isinstance(style, dict):
            raise TypeError("image border style must be a dict or None")
        _check(_lib.ofd_set_page_image_border_json(
            self._handle, page_index, _utf8(str(object_id)),
            _utf8(json.dumps(style))))

    def set_page_object_gouraud_fill(self, page_index: int, object_id: str | int,
                                     style: dict) -> None:
        """Set a path or text fill to a Gouraud mesh of local point coordinates."""
        self._require_open()
        if page_index < 0:
            raise IndexError(page_index)
        if not isinstance(style, dict):
            raise TypeError("Gouraud style must be a dict")
        _check(_lib.ofd_set_page_object_gouraud_fill_json(
            self._handle, page_index, _utf8(str(object_id)),
            _utf8(json.dumps(style))))

    def set_page_object_gouraud_stroke(self, page_index: int, object_id: str | int,
                                       style: dict) -> None:
        """Set a path or text stroke to a Gouraud mesh of local point coordinates."""
        self._require_open()
        if page_index < 0:
            raise IndexError(page_index)
        if not isinstance(style, dict):
            raise TypeError("Gouraud style must be a dict")
        _check(_lib.ofd_set_page_object_gouraud_stroke_json(
            self._handle, page_index, _utf8(str(object_id)),
            _utf8(json.dumps(style))))

    def set_page_object_gradient_fill(self, page_index: int, object_id: str | int,
                                      style: dict) -> None:
        """Set a path or text fill to an axial or radial color gradient."""
        self._require_open()
        if page_index < 0:
            raise IndexError(page_index)
        if not isinstance(style, dict):
            raise TypeError("gradient style must be a dict")
        _check(_lib.ofd_set_page_object_gradient_fill_json(
            self._handle, page_index, _utf8(str(object_id)),
            _utf8(json.dumps(style))))

    def set_page_object_gradient_stroke(self, page_index: int, object_id: str | int,
                                        style: dict) -> None:
        """Set a path or text stroke to an axial or radial color gradient."""
        self._require_open()
        if page_index < 0:
            raise IndexError(page_index)
        if not isinstance(style, dict):
            raise TypeError("gradient style must be a dict")
        _check(_lib.ofd_set_page_object_gradient_stroke_json(
            self._handle, page_index, _utf8(str(object_id)),
            _utf8(json.dumps(style))))

    def set_page_object_pattern_fill(self, page_index: int, object_id: str | int,
                                     style: dict) -> None:
        """Set a path or text fill to a repeated pattern of graphic cells."""
        self._require_open()
        if page_index < 0:
            raise IndexError(page_index)
        if not isinstance(style, dict):
            raise TypeError("pattern style must be a dict")
        _check(_lib.ofd_set_page_object_pattern_fill_json(
            self._handle, page_index, _utf8(str(object_id)),
            _utf8(json.dumps(style))))

    def set_page_object_pattern_stroke(self, page_index: int, object_id: str | int,
                                       style: dict) -> None:
        """Set a path or text stroke to a repeated pattern of graphic cells."""
        self._require_open()
        if page_index < 0:
            raise IndexError(page_index)
        if not isinstance(style, dict):
            raise TypeError("pattern style must be a dict")
        _check(_lib.ofd_set_page_object_pattern_stroke_json(
            self._handle, page_index, _utf8(str(object_id)),
            _utf8(json.dumps(style))))

    def set_page_object_solid_fill(self, page_index: int, object_id: str | int,
                                   style: dict) -> None:
        """Replace a path or text fill with RGB color and optional byte alpha."""
        self._require_open()
        if page_index < 0:
            raise IndexError(page_index)
        if not isinstance(style, dict):
            raise TypeError("solid paint style must be a dict")
        _check(_lib.ofd_set_page_object_solid_fill_json(
            self._handle, page_index, _utf8(str(object_id)),
            _utf8(json.dumps(style))))

    def set_page_object_solid_stroke(self, page_index: int, object_id: str | int,
                                     style: dict) -> None:
        """Replace a path or text stroke with RGB color and optional byte alpha."""
        self._require_open()
        if page_index < 0:
            raise IndexError(page_index)
        if not isinstance(style, dict):
            raise TypeError("solid paint style must be a dict")
        _check(_lib.ofd_set_page_object_solid_stroke_json(
            self._handle, page_index, _utf8(str(object_id)),
            _utf8(json.dumps(style))))

    def set_page_object_boundary(self, page_index: int, object_id: str | int,
                                 boundary: tuple[float, float, float, float]) -> None:
        """Set an object's page boundary in millimeters."""
        self._require_open()
        if page_index < 0:
            raise IndexError(page_index)
        if len(boundary) != 4:
            raise ValueError("boundary must contain four numbers")
        _check(_lib.ofd_set_page_object_boundary(
            self._handle, page_index, _utf8(str(object_id)), *boundary))

    def set_page_object_ctm(self, page_index: int, object_id: str | int,
                            ctm: tuple[float, float, float, float, float, float] | None) -> None:
        """Set a local transform, or pass None to remove it."""
        self._require_open()
        if page_index < 0:
            raise IndexError(page_index)
        if ctm is not None and len(ctm) != 6:
            raise ValueError("ctm must contain six numbers")
        encoded = _ffi.NULL if ctm is None else _utf8(json.dumps(ctm, allow_nan=False))
        _check(_lib.ofd_set_page_object_ctm_json(
            self._handle, page_index, _utf8(str(object_id)), encoded))

    def remove_page_layer(self, page_index: int, layer_id: str | int) -> None:
        """Remove an empty page layer; keep its allocated ID reserved."""
        self._require_open()
        if page_index < 0:
            raise IndexError(page_index)
        _check(_lib.ofd_remove_page_layer(self._handle, page_index, _utf8(str(layer_id))))

    def add_path_object(
        self,
        page_index: int,
        commands: str,
        boundary: tuple[float, float, float, float],
        *,
        fill: bool = False,
        even_odd: bool = False,
        alpha: int | None = None,
        stroke: bool = True,
        line_width: float = 0.4,
        cap: str | None = None,
        join: str | None = None,
        miter_limit: float | None = None,
        dash_pattern: tuple[float, float] | None = None,
        dash_sequence: list[float] | None = None,
        dash_offset: float | None = None,
        fill_color: tuple[int, int, int] | None = None,
        fill_alpha: int | None = None,
        stroke_color: tuple[int, int, int] | None = None,
        stroke_alpha: int | None = None,
    ) -> int:
        """Add a vector path to a page and return its OFD object ID."""
        self._require_open()
        if page_index < 0:
            raise IndexError(page_index)
        value = {"commands": commands, "boundary": boundary, "fill": fill,
                 "even_odd": even_odd,
                 "stroke": stroke, "line_width": line_width}
        if alpha is not None:
            value["alpha"] = alpha
        for name, option in (("cap", cap), ("join", join),
                             ("miter_limit", miter_limit),
                             ("dash_pattern", dash_pattern),
                             ("dash_sequence", dash_sequence),
                             ("dash_offset", dash_offset)):
            if option is not None:
                value[name] = option
        if fill_color is not None:
            value["fill_color"] = fill_color
        if fill_alpha is not None:
            value["fill_alpha"] = fill_alpha
        if stroke_color is not None:
            value["stroke_color"] = stroke_color
        if stroke_alpha is not None:
            value["stroke_alpha"] = stroke_alpha
        payload = json.dumps(value, allow_nan=False)
        output = _ffi.new("size_t *")
        _check(_lib.ofd_add_path_object_json(self._handle, page_index, _utf8(payload), output))
        return int(output[0])

    def add_composite_object(
        self,
        page_index: int,
        resource_id: str,
        boundary: tuple[float, float, float, float],
        *,
        ctm: tuple[float, float, float, float, float, float] | None = None,
        alpha: int | None = None,
        draw_param: str | None = None,
    ) -> int:
        """Place an existing composite graphic resource on a page."""
        self._require_open()
        if page_index < 0:
            raise IndexError(page_index)
        value = {"resource_id": resource_id, "boundary": boundary}
        if ctm is not None:
            value["ctm"] = ctm
        if alpha is not None:
            value["alpha"] = alpha
        if draw_param is not None:
            value["draw_param"] = draw_param
        payload = json.dumps(value, allow_nan=False)
        output = _ffi.new("size_t *")
        _check(_lib.ofd_add_composite_object_json(self._handle, page_index, _utf8(payload), output))
        return int(output[0])

    def add_composite_path_object(
        self,
        page_index: int,
        commands: str,
        boundary: tuple[float, float, float, float],
        *,
        fill: bool = False,
        even_odd: bool = False,
        alpha: int | None = None,
        stroke: bool = True,
        line_width: float = 0.4,
        cap: str | None = None,
        join: str | None = None,
        miter_limit: float | None = None,
        dash_pattern: tuple[float, float] | None = None,
        dash_sequence: list[float] | None = None,
        dash_offset: float | None = None,
        fill_color: tuple[int, int, int] | None = None,
        fill_alpha: int | None = None,
        stroke_color: tuple[int, int, int] | None = None,
        stroke_alpha: int | None = None,
    ) -> int:
        """Create a composite resource with one vector path and place it."""
        self._require_open()
        if page_index < 0:
            raise IndexError(page_index)
        value = {"commands": commands, "boundary": boundary, "fill": fill,
                 "even_odd": even_odd,
                 "stroke": stroke, "line_width": line_width}
        if alpha is not None:
            value["alpha"] = alpha
        for name, option in (("cap", cap), ("join", join),
                             ("miter_limit", miter_limit),
                             ("dash_pattern", dash_pattern),
                             ("dash_sequence", dash_sequence),
                             ("dash_offset", dash_offset)):
            if option is not None:
                value[name] = option
        if fill_color is not None:
            value["fill_color"] = fill_color
        if fill_alpha is not None:
            value["fill_alpha"] = fill_alpha
        if stroke_color is not None:
            value["stroke_color"] = stroke_color
        if stroke_alpha is not None:
            value["stroke_alpha"] = stroke_alpha
        payload = json.dumps(value, allow_nan=False)
        output = _ffi.new("size_t *")
        _check(_lib.ofd_add_composite_path_object_json(self._handle, page_index, _utf8(payload), output))
        return int(output[0])

    def add_text_watermark(
        self, page_index: int, text: str,
        boundary: tuple[float, float, float, float], size: float = 5,
        *, font_id: str | None = None,
        color: tuple[int, int, int] = (0, 0, 0), opacity: int = 255,
        angle_degrees: float = 0, bold: bool = False, italic: bool = False,
        tile_spacing: tuple[float, float] | None = None,
    ) -> int:
        """Add a text watermark annotation and return its OFD annotation ID."""
        self._require_open()
        if page_index < 0:
            raise IndexError(page_index)
        payload = json.dumps(
            {"text": text, "boundary": boundary, "size": size,
             "font_id": font_id, "color": color, "opacity": opacity,
             "angle_degrees": angle_degrees, "tile_spacing": tile_spacing,
             "bold": bold, "italic": italic},
            ensure_ascii=False, allow_nan=False,
        )
        output = _ffi.new("size_t *")
        _check(_lib.ofd_add_text_watermark_json(self._handle, page_index, _utf8(payload), output))
        return int(output[0])

    def add_text_stamp_annotation(
        self, page_index: int, text: str,
        boundary: tuple[float, float, float, float], size: float = 5,
        *, font_id: str | None = None,
        color: tuple[int, int, int] = (0, 0, 0), opacity: int = 255,
        angle_degrees: float = 0, bold: bool = False, italic: bool = False,
        tile_spacing: tuple[float, float] | None = None,
    ) -> int:
        """Add a text Stamp annotation and return its OFD annotation ID."""
        self._require_open()
        if page_index < 0:
            raise IndexError(page_index)
        payload = json.dumps(
            {"text": text, "boundary": boundary, "size": size,
             "font_id": font_id, "color": color, "opacity": opacity,
             "angle_degrees": angle_degrees, "tile_spacing": tile_spacing,
             "bold": bold, "italic": italic},
            ensure_ascii=False, allow_nan=False,
        )
        output = _ffi.new("size_t *")
        _check(_lib.ofd_add_text_stamp_annotation_json(
            self._handle, page_index, _utf8(payload), output))
        return int(output[0])

    def add_text_watermark_to_all_pages(
        self, text: str, size: float = 5,
        *, font_id: str | None = None,
        color: tuple[int, int, int] = (0, 0, 0), opacity: int = 255,
        angle_degrees: float = 0, bold: bool = False, italic: bool = False,
        tile_spacing: tuple[float, float] | None = None,
    ) -> list[int]:
        """Atomically add a watermark covering each existing page."""
        self._require_open()
        payload = json.dumps(
            {"text": text, "size": size, "font_id": font_id,
             "color": color, "opacity": opacity,
             "angle_degrees": angle_degrees, "tile_spacing": tile_spacing,
             "bold": bold, "italic": italic},
            ensure_ascii=False, allow_nan=False,
        )
        output = _ffi.new("OfdBuffer *")
        _check(_lib.ofd_add_text_watermark_all_pages_json(
            self._handle, _utf8(payload), output))
        try:
            return json.loads(bytes(_ffi.buffer(output.data, output.len)))
        finally:
            _lib.ofd_buffer_free(output[0])

    def add_image_watermark(
        self, page_index: int, boundary: tuple[float, float, float, float],
        *, data: bytes | None = None, format: str | None = None,
        resource_id: str | None = None, opacity: int = 255,
    ) -> int:
        """Add an image watermark from bytes or a visible image resource."""
        self._require_open()
        if page_index < 0:
            raise IndexError(page_index)
        payload = json.dumps(
            {"boundary": boundary, "opacity": opacity,
             "resource_id": resource_id,
             "data_base64": base64.b64encode(data).decode("ascii") if data is not None else None,
             "format": format},
            allow_nan=False,
        )
        output = _ffi.new("size_t *")
        _check(_lib.ofd_add_image_watermark_json(
            self._handle, page_index, _utf8(payload), output))
        return int(output[0])

    def add_image_stamp_annotation(
        self, page_index: int, boundary: tuple[float, float, float, float],
        *, data: bytes | None = None, format: str | None = None,
        resource_id: str | None = None, opacity: int = 255,
    ) -> int:
        """Add a Stamp annotation from image bytes or a visible image resource."""
        self._require_open()
        if page_index < 0:
            raise IndexError(page_index)
        payload = json.dumps(
            {"boundary": boundary, "opacity": opacity,
             "resource_id": resource_id,
             "data_base64": base64.b64encode(data).decode("ascii") if data is not None else None,
             "format": format},
            allow_nan=False,
        )
        output = _ffi.new("size_t *")
        _check(_lib.ofd_add_image_stamp_annotation_json(
            self._handle, page_index, _utf8(payload), output))
        return int(output[0])

    def add_image_watermark_to_all_pages(
        self, data: bytes, format: str,
        boundary: tuple[float, float, float, float], *, opacity: int = 255,
    ) -> list[int]:
        """Atomically add the same image watermark to each existing page."""
        self._require_open()
        payload = json.dumps(
            {"boundary": boundary, "opacity": opacity,
             "data_base64": base64.b64encode(data).decode("ascii"), "format": format},
            allow_nan=False,
        )
        output = _ffi.new("OfdBuffer *")
        _check(_lib.ofd_add_image_watermark_all_pages_json(
            self._handle, _utf8(payload), output))
        try:
            return json.loads(bytes(_ffi.buffer(output.data, output.len)))
        finally:
            _lib.ofd_buffer_free(output[0])

    def add_path_annotation(
        self,
        page_index: int,
        commands: str,
        boundary: tuple[float, float, float, float],
        *,
        fill: bool = False,
        even_odd: bool = False,
        alpha: int | None = None,
        stroke: bool = True,
        line_width: float = 0.4,
        cap: str | None = None,
        join: str | None = None,
        miter_limit: float | None = None,
        dash_pattern: tuple[float, float] | None = None,
        dash_offset: float | None = None,
        fill_color: tuple[int, int, int] | None = None,
        fill_alpha: int | None = None,
        stroke_color: tuple[int, int, int] | None = None,
        stroke_alpha: int | None = None,
    ) -> int:
        """Add a styled vector Path annotation and return its annotation ID."""
        self._require_open()
        if page_index < 0:
            raise IndexError(page_index)
        value = {"commands": commands, "boundary": boundary, "fill": fill,
                 "even_odd": even_odd, "stroke": stroke,
                 "line_width": line_width}
        for name, option in (
            ("alpha", alpha), ("cap", cap), ("join", join),
            ("miter_limit", miter_limit), ("dash_pattern", dash_pattern),
            ("dash_offset", dash_offset), ("fill_color", fill_color),
            ("fill_alpha", fill_alpha), ("stroke_color", stroke_color),
            ("stroke_alpha", stroke_alpha),
        ):
            if option is not None:
                value[name] = option
        payload = json.dumps(value, allow_nan=False)
        output = _ffi.new("size_t *")
        _check(_lib.ofd_add_path_annotation_json(
            self._handle, page_index, _utf8(payload), output))
        return int(output[0])

    def add_multi_path_annotation(
        self, page_index: int,
        boundary: tuple[float, float, float, float],
        paths: list[dict],
    ) -> int:
        """Add one Path annotation with several styled path objects.

        Each path has ``commands``, an appearance-local ``boundary``, and
        optional style fields accepted by ``add_path_annotation``.
        """
        self._require_open()
        if page_index < 0:
            raise IndexError(page_index)
        payload = json.dumps({"boundary": boundary, "paths": paths}, allow_nan=False)
        output = _ffi.new("size_t *")
        _check(_lib.ofd_add_multi_path_annotation_json(
            self._handle, page_index, _utf8(payload), output))
        return int(output[0])

    def add_mixed_annotation(
        self, page_index: int,
        boundary: tuple[float, float, float, float],
        objects: list[dict],
    ) -> int:
        """Add ordered path, text, image, and composite objects to one annotation.

        Each object has an appearance-local ``boundary`` and a ``kind`` of
        ``path``, ``text``, ``image``, or ``composite``. Referenced resources must
        be visible to the page.
        """
        return self._add_mixed_annotation(page_index, boundary, objects, "Path")

    def add_mixed_stamp_annotation(
        self, page_index: int,
        boundary: tuple[float, float, float, float],
        objects: list[dict],
    ) -> int:
        """Add ordered path, text, and image objects to one Stamp annotation."""
        return self._add_mixed_annotation(page_index, boundary, objects, "Stamp")

    def _add_mixed_annotation(
        self, page_index: int,
        boundary: tuple[float, float, float, float],
        objects: list[dict], annotation_type: str,
    ) -> int:
        self._require_open()
        if page_index < 0:
            raise IndexError(page_index)
        payload = json.dumps({"boundary": boundary, "objects": objects,
                              "annotation_type": annotation_type}, allow_nan=False)
        output = _ffi.new("size_t *")
        _check(_lib.ofd_add_mixed_annotation_json(
            self._handle, page_index, _utf8(payload), output))
        return int(output[0])

    def add_highlight_annotation(
        self, page_index: int,
        boundary: tuple[float, float, float, float],
        color: tuple[int, int, int] = (255, 255, 0),
        opacity: int = 96,
    ) -> int:
        """Add a translucent rectangular Highlight annotation."""
        self._require_open()
        if page_index < 0:
            raise IndexError(page_index)
        payload = json.dumps(
            {"boundary": boundary, "color": color, "opacity": opacity},
            allow_nan=False,
        )
        output = _ffi.new("size_t *")
        _check(_lib.ofd_add_highlight_annotation_json(
            self._handle, page_index, _utf8(payload), output))
        return int(output[0])

    def add_text_note_annotation(
        self, page_index: int, text: str,
        boundary: tuple[float, float, float, float],
        *, color: tuple[int, int, int] = (255, 255, 0),
        opacity: int = 255, icon: str = "Note", open: bool = False,
    ) -> int:
        """Add an editable Text note with a simple OFD preview marker."""
        self._require_open()
        if page_index < 0:
            raise IndexError(page_index)
        payload = json.dumps(
            {"text": text, "boundary": boundary, "color": color,
             "opacity": opacity, "icon": icon, "open": open},
            allow_nan=False,
        )
        output = _ffi.new("size_t *")
        _check(_lib.ofd_add_text_note_annotation_json(
            self._handle, page_index, _utf8(payload), output))
        return int(output[0])

    def add_shape_annotation(
        self, page_index: int, kind: str,
        boundary: tuple[float, float, float, float],
        *, stroke_color: tuple[int, int, int] | None = (0, 0, 0),
        fill_color: tuple[int, int, int] | None = None,
        line_width: float = 25.4 / 72.0,
        dash_sequence: list[float] | None = None,
        opacity: int = 255,
        insets: tuple[float, float, float, float] = (0, 0, 0, 0),
    ) -> int:
        """Add an editable Square or Circle annotation; lengths use millimeters."""
        self._require_open()
        if page_index < 0:
            raise IndexError(page_index)
        payload = json.dumps({
            "kind": kind, "boundary": boundary, "stroke": stroke_color is not None,
            "stroke_color": stroke_color, "fill_color": fill_color,
            "line_width": line_width, "dash_sequence": dash_sequence,
            "opacity": opacity, "insets": insets,
        }, allow_nan=False)
        output = _ffi.new("size_t *")
        _check(_lib.ofd_add_shape_annotation_json(
            self._handle, page_index, _utf8(payload), output))
        return int(output[0])

    def add_line_annotation(
        self, page_index: int,
        boundary: tuple[float, float, float, float],
        endpoints: tuple[float, float, float, float],
        *, stroke_color: tuple[int, int, int] | None = (0, 0, 0),
        line_width: float = 25.4 / 72.0,
        dash_sequence: list[float] | None = None,
        opacity: int = 255,
    ) -> int:
        """Add a Line annotation; endpoints are boundary-local millimeters."""
        self._require_open()
        if page_index < 0:
            raise IndexError(page_index)
        payload = json.dumps({
            "boundary": boundary, "endpoints": endpoints,
            "stroke": stroke_color is not None, "stroke_color": stroke_color,
            "line_width": line_width, "dash_sequence": dash_sequence,
            "opacity": opacity,
        }, allow_nan=False)
        output = _ffi.new("size_t *")
        _check(_lib.ofd_add_line_annotation_json(
            self._handle, page_index, _utf8(payload), output))
        return int(output[0])

    def add_uri_link_annotation(
        self, page_index: int, uri: str,
        boundary: tuple[float, float, float, float],
        *, quad_points: list[tuple[tuple[float, float], tuple[float, float],
                                   tuple[float, float], tuple[float, float]]] | None = None,
    ) -> int:
        """Add an invisible clickable URI Link annotation."""
        self._require_open()
        if page_index < 0:
            raise IndexError(page_index)
        payload = json.dumps(
            {"uri": uri, "boundary": boundary, "quad_points": quad_points or []},
            ensure_ascii=False, allow_nan=False,
        )
        output = _ffi.new("size_t *")
        _check(_lib.ofd_add_uri_link_annotation_json(
            self._handle, page_index, _utf8(payload), output))
        return int(output[0])

    def add_goto_link_annotation(
        self, page_index: int, page_id: str | int,
        boundary: tuple[float, float, float, float], kind: str = "XYZ",
        *, left: float | None = None, top: float | None = None,
        right: float | None = None, bottom: float | None = None,
        zoom: float | None = None,
        quad_points: list[tuple[tuple[float, float], tuple[float, float],
                                tuple[float, float], tuple[float, float]]] | None = None,
    ) -> int:
        """Add an invisible Link annotation to a page in the same document."""
        self._require_open()
        if page_index < 0:
            raise IndexError(page_index)
        payload = json.dumps({
            "destination": {"page_id": str(page_id), "kind": kind,
                            "left": left, "top": top, "right": right,
                            "bottom": bottom, "zoom": zoom},
            "boundary": boundary,
            "quad_points": quad_points or [],
        }, ensure_ascii=False, allow_nan=False)
        output = _ffi.new("size_t *")
        _check(_lib.ofd_add_goto_link_annotation_json(
            self._handle, page_index, _utf8(payload), output))
        return int(output[0])

    def add_bookmark_link_annotation(
        self, page_index: int, bookmark_name: str,
        boundary: tuple[float, float, float, float],
    ) -> int:
        """Add an invisible Link annotation to a named bookmark."""
        self._require_open()
        if page_index < 0:
            raise IndexError(page_index)
        payload = json.dumps({"bookmark_name": bookmark_name, "boundary": boundary},
                             ensure_ascii=False, allow_nan=False)
        output = _ffi.new("size_t *")
        _check(_lib.ofd_add_bookmark_link_annotation_json(
            self._handle, page_index, _utf8(payload), output))
        return int(output[0])

    def add_attachment_link_annotation(
        self, page_index: int, attachment_id: str | int,
        boundary: tuple[float, float, float, float],
        new_window: bool | None = None,
    ) -> int:
        """Add an invisible Link annotation that opens an attachment."""
        self._require_open()
        if page_index < 0:
            raise IndexError(page_index)
        if new_window is not None and not isinstance(new_window, bool):
            raise TypeError("new_window must be bool or None")
        payload = json.dumps({"attachment_id": str(attachment_id), "boundary": boundary,
                              "new_window": new_window}, ensure_ascii=False, allow_nan=False)
        output = _ffi.new("size_t *")
        _check(_lib.ofd_add_attachment_link_annotation_json(
            self._handle, page_index, _utf8(payload), output))
        return int(output[0])

    def add_sound_link_annotation(
        self, page_index: int, resource_id: str | int,
        boundary: tuple[float, float, float, float],
        *, volume: int | None = None, repeat: bool | None = None,
        synchronous: bool | None = None,
    ) -> int:
        """Add an invisible Link annotation that plays an Audio resource."""
        self._require_open()
        if page_index < 0:
            raise IndexError(page_index)
        payload = json.dumps({"kind": "Sound", "resource_id": str(resource_id),
                              "boundary": boundary, "volume": volume, "repeat": repeat,
                              "synchronous": synchronous}, allow_nan=False)
        output = _ffi.new("size_t *")
        _check(_lib.ofd_add_media_link_annotation_json(
            self._handle, page_index, _utf8(payload), output))
        return int(output[0])

    def add_movie_link_annotation(
        self, page_index: int, resource_id: str | int,
        boundary: tuple[float, float, float, float], *, operator: str | None = None,
    ) -> int:
        """Add an invisible Link annotation that controls a Video resource."""
        self._require_open()
        if page_index < 0:
            raise IndexError(page_index)
        payload = json.dumps({"kind": "Movie", "resource_id": str(resource_id),
                              "boundary": boundary, "operator": operator}, allow_nan=False)
        output = _ffi.new("size_t *")
        _check(_lib.ofd_add_media_link_annotation_json(
            self._handle, page_index, _utf8(payload), output))
        return int(output[0])

    def add_text_page_with_font(
        self, text: str, font_id: int, width_mm: float = 210, height_mm: float = 297
    ) -> None:
        """Append a text page that references a declared font resource."""
        self._require_open()
        if font_id < 0:
            raise ValueError("font_id must be nonnegative")
        payload = {"operation": "add", "text": text,
                   "width_mm": width_mm, "height_mm": height_mm,
                   "font_id": font_id}
        _check(_lib.ofd_write_text_page_json(
            self._handle, _utf8(json.dumps(payload)), _ffi.NULL))

    def add_embedded_font(self, name: str, data: bytes) -> int:
        """Embed a TrueType/OpenType font and return its resource identifier."""
        self._require_open()
        output = _ffi.new("size_t *")
        _check(_lib.ofd_add_embedded_font(self._handle, _utf8(name), data, len(data), output))
        return int(output[0])

    def add_embedded_font_to_document(self, document_index: int, name: str, data: bytes) -> int:
        """Embed a TrueType/OpenType font in a document body and return its resource ID."""
        self._require_open()
        if document_index < 0:
            raise IndexError(document_index)
        output = _ffi.new("size_t *")
        _check(
            _lib.ofd_add_embedded_font_to_document(
                self._handle, document_index, _utf8(name), data, len(data), output
            )
        )
        return int(output[0])

    def add_embedded_font_to_page(self, page_index: int, name: str, data: bytes) -> int:
        """Embed a TrueType/OpenType font in one page and return its resource ID."""
        self._require_open()
        if page_index < 0:
            raise IndexError(page_index)
        data = bytes(data)
        output = _ffi.new("size_t *")
        _check(_lib.ofd_add_embedded_font_to_page(
            self._handle, page_index, _utf8(name), data, len(data), output))
        return int(output[0])

    def set_font_attribute(self, font_index: int, name: str, value: str | None) -> None:
        """Set or clear a standard font attribute by index in ``fonts()``."""
        self._require_open()
        if font_index < 0:
            raise IndexError(font_index)
        _check(_lib.ofd_set_font_attribute(
            self._handle, font_index, _utf8(name),
            _ffi.NULL if value is None else _utf8(value)))

    def set_page_font_attribute(
        self, page_index: int, font_index: int, name: str, value: str | None
    ) -> None:
        """Set or clear an attribute by index in ``page_fonts(page_index)``."""
        self._require_open()
        if page_index < 0 or font_index < 0:
            raise IndexError((page_index, font_index))
        _check(_lib.ofd_set_page_font_attribute(
            self._handle, page_index, font_index, _utf8(name),
            _ffi.NULL if value is None else _utf8(value)))

    def replace_font_data(self, font_index: int, data: bytes) -> None:
        """Replace an embedded font by index in ``fonts()``."""
        self._require_open()
        if font_index < 0:
            raise IndexError(font_index)
        data = bytes(data)
        _check(_lib.ofd_replace_font_data(self._handle, font_index, data, len(data)))

    def replace_page_font_data(self, page_index: int, font_index: int, data: bytes) -> None:
        """Replace an embedded font by index in ``page_fonts(page_index)``."""
        self._require_open()
        if page_index < 0 or font_index < 0:
            raise IndexError((page_index, font_index))
        data = bytes(data)
        _check(_lib.ofd_replace_page_font_data(
            self._handle, page_index, font_index, data, len(data)))

    def append_text_page(
        self, document_index: int, text: str, width_mm: float = 210, height_mm: float = 297
    ) -> None:
        """Append a text page to a document body in a new or existing package."""
        self._require_open()
        if document_index < 0:
            raise IndexError(document_index)
        payload = {"operation": "append", "document_index": document_index,
                   "text": text, "width_mm": width_mm, "height_mm": height_mm}
        _check(_lib.ofd_write_text_page_json(
            self._handle, _utf8(json.dumps(payload)), _ffi.NULL))

    def append_blank_page(
        self, document_index: int, width_mm: float = 210, height_mm: float = 297
    ) -> None:
        """Append an empty page to a new or existing document body."""
        self._require_open()
        if document_index < 0:
            raise IndexError(document_index)
        _check(_lib.ofd_append_blank_page(self._handle, document_index, width_mm, height_mm))

    def insert_text_page(
        self, document_index: int, position: int, text: str,
        width_mm: float = 210, height_mm: float = 297,
    ) -> int:
        """Insert a text page and return its package-wide zero-based index."""
        self._require_open()
        if document_index < 0 or position < 0:
            raise IndexError((document_index, position))
        output = _ffi.new("size_t *")
        payload = {"operation": "insert", "document_index": document_index,
                   "position": position, "text": text,
                   "width_mm": width_mm, "height_mm": height_mm}
        _check(_lib.ofd_write_text_page_json(
            self._handle, _utf8(json.dumps(payload)), output))
        return int(output[0])

    def insert_blank_page(
        self, document_index: int, position: int,
        width_mm: float = 210, height_mm: float = 297,
    ) -> int:
        """Insert an empty page and return its package-wide zero-based index."""
        self._require_open()
        if document_index < 0 or position < 0:
            raise IndexError((document_index, position))
        output = _ffi.new("size_t *")
        _check(_lib.ofd_insert_blank_page(
            self._handle, document_index, position, width_mm, height_mm, output
        ))
        return int(output[0])

    def add_flowed_text(
        self,
        document_index: int,
        text: str,
        page_size: tuple[float, float] = (210, 297),
        margins: tuple[float, float, float, float] = (25.4, 31.7, 25.4, 31.7),
        font_size: float = 5,
        line_height: float = 6,
        paragraph_gap: float = 0,
        keep_paragraphs_together: bool = False,
        font_id: str | int | None = None,
        align: str = "left",
        first_line_indent: float = 0,
        shape: bool = False,
        first_line_indent_chars: int = 0,
        indent_each_paragraph: bool = False,
        language: str | None = None,
        rest_line_indent: float = 0,
    ) -> list[int]:
        """Wrap plain text into new pages; shape with an embedded font when requested."""
        self._require_open()
        if document_index < 0:
            raise IndexError(document_index)
        if len(page_size) != 2 or len(margins) != 4:
            raise ValueError("page_size needs two values and margins need four values")
        payload = {
            "text": text,
            "page_size": page_size,
            "margins": margins,
            "font_size": font_size,
            "line_height": line_height,
            "paragraph_gap": paragraph_gap,
            "keep_paragraphs_together": keep_paragraphs_together,
            "align": align,
            "first_line_indent": first_line_indent,
            "rest_line_indent": rest_line_indent,
            "first_line_indent_chars": first_line_indent_chars,
            "indent_each_paragraph": indent_each_paragraph,
            "shape": shape,
        }
        if font_id is not None:
            payload["font_id"] = str(font_id)
        if language is not None:
            payload["language"] = language
        output = _ffi.new("OfdBuffer *")
        _check(_lib.ofd_add_flowed_text_json(
            self._handle, document_index, _utf8(json.dumps(payload)), output,
        ))
        try:
            return json.loads(bytes(_ffi.buffer(output.data, output.len)))
        finally:
            _lib.ofd_buffer_free(output[0])

    def add_table(
        self,
        document_index: int,
        rows: list[list[str]],
        column_widths: list[float],
        *,
        page_size: tuple[float, float] = (210, 297),
        margins: tuple[float, float, float, float] = (25.4, 31.7, 25.4, 31.7),
        row_height: float = 8,
        font_size: float = 5,
        line_height: float = 6,
        padding: float = 1,
        border_width: float = 0.2,
        border_color: tuple[int, int, int] = (0, 0, 0),
        font_id: str | int | None = None,
        repeat_header: bool = False,
    ) -> list[int]:
        """Append a table, splitting between rows; row_height=0 sizes rows from content."""
        self._require_open()
        if document_index < 0:
            raise IndexError(document_index)
        payload = {
            "rows": rows, "column_widths": column_widths,
            "page_size": page_size, "margins": margins,
            "row_height": row_height, "font_size": font_size,
            "line_height": line_height, "padding": padding,
            "border_width": border_width, "border_color": border_color,
            "repeat_header": repeat_header,
        }
        if font_id is not None:
            payload["font_id"] = str(font_id)
        output = _ffi.new("OfdBuffer *")
        _check(_lib.ofd_add_table_json(
            self._handle, document_index,
            _utf8(json.dumps(payload, allow_nan=False)), output,
        ))
        try:
            return json.loads(bytes(_ffi.buffer(output.data, output.len)))
        finally:
            _lib.ofd_buffer_free(output[0])

    def add_table_cells(
        self,
        document_index: int,
        row_count: int,
        cells: list[dict],
        column_widths: list[float],
        *,
        page_size: tuple[float, float] = (210, 297),
        margins: tuple[float, float, float, float] = (25.4, 31.7, 25.4, 31.7),
        row_height: float = 8,
        font_size: float = 5,
        line_height: float = 6,
        padding: float = 1,
        border_width: float = 0.2,
        border_color: tuple[int, int, int] = (0, 0, 0),
        font_id: str | int | None = None,
        repeat_header: bool = False,
    ) -> list[int]:
        """Append positioned cells with spans and optional image dictionaries.

        An image may contain ``data`` as bytes, plus ``format``, ``width`` and
        ``height``. Ordered ``blocks`` may contain text, image, spacer,
        rule, and nested table dictionaries. A table block contains a ``table``
        dictionary with ``row_count``, ``column_widths``, and ``cells``.
        Width and height are in millimeters. Cells may override ``font_size``
        and ``line_height``; omitted line height keeps the table's leading.
        ``bold`` and ``italic`` apply to all text in a cell.
        ``underline`` and ``strikethrough`` draw text decorations per line.
        ``letter_spacing`` adds millimeter spacing to text layout and glyph positions.
        ``font_id`` selects an existing document font and may be a string or int.
        ``padding`` accepts top, right, bottom, left millimeter values.
        ``border_width`` and ``border_color`` override one cell's border.
        """
        self._require_open()
        if document_index < 0:
            raise IndexError(document_index)
        if row_count < 0:
            raise IndexError(row_count)
        def serialize_cell(cell: dict) -> dict:
            item = dict(cell)
            if "font_id" in item:
                if item["font_id"] is None:
                    del item["font_id"]
                else:
                    item["font_id"] = str(item["font_id"])
            if item.get("image") is not None:
                cell_image = dict(item["image"])
                if isinstance(cell_image.get("data"), bytes):
                    cell_image["data_base64"] = base64.b64encode(cell_image.pop("data")).decode("ascii")
                item["image"] = cell_image
            if item.get("blocks") is not None:
                blocks = []
                for block in item["blocks"]:
                    content = dict(block)
                    if content.get("type") == "image" and isinstance(content.get("data"), bytes):
                        content["data_base64"] = base64.b64encode(content.pop("data")).decode("ascii")
                    if content.get("type") == "table" and content.get("table") is not None:
                        table = dict(content["table"])
                        table["cells"] = [serialize_cell(nested) for nested in table["cells"]]
                        content["table"] = table
                    blocks.append(content)
                item["blocks"] = blocks
            return item

        serialized_cells = [serialize_cell(cell) for cell in cells]
        payload = {
            "row_count": row_count, "cells": serialized_cells,
            "column_widths": column_widths, "page_size": page_size,
            "margins": margins, "row_height": row_height,
            "font_size": font_size, "line_height": line_height,
            "padding": padding, "border_width": border_width,
            "border_color": border_color, "repeat_header": repeat_header,
        }
        if font_id is not None:
            payload["font_id"] = str(font_id)
        output = _ffi.new("OfdBuffer *")
        _check(_lib.ofd_add_table_json(
            self._handle, document_index,
            _utf8(json.dumps(payload, allow_nan=False)), output,
        ))
        try:
            return json.loads(bytes(_ffi.buffer(output.data, output.len)))
        finally:
            _lib.ofd_buffer_free(output[0])

    def add_flowed_spans(
        self,
        document_index: int,
        spans: list[dict],
        page_size: tuple[float, float] = (210, 297),
        margins: tuple[float, float, float, float] = (25.4, 31.7, 25.4, 31.7),
        font_size: float = 5,
        line_height: float = 6,
        paragraph_gap: float = 0,
        keep_paragraphs_together: bool = False,
        font_id: str | int | None = None,
        align: str = "left",
        first_line_indent: float = 0,
        shape: bool = False,
        first_line_indent_chars: int = 0,
        indent_each_paragraph: bool = False,
        language: str | None = None,
        rest_line_indent: float = 0,
    ) -> list[int]:
        """Flow styled spans into new pages, sizing each line by its tallest font."""
        self._require_open()
        if document_index < 0:
            raise IndexError(document_index)
        if len(page_size) != 2 or len(margins) != 4:
            raise ValueError("page_size needs two values and margins need four values")
        serialized_spans = []
        for span in spans:
            item = dict(span)
            if item.get("font_id") is not None:
                item["font_id"] = str(item["font_id"])
            serialized_spans.append(item)
        payload = {
            "spans": serialized_spans,
            "page_size": page_size,
            "margins": margins,
            "font_size": font_size,
            "line_height": line_height,
            "paragraph_gap": paragraph_gap,
            "keep_paragraphs_together": keep_paragraphs_together,
            "align": align,
            "first_line_indent": first_line_indent,
            "rest_line_indent": rest_line_indent,
            "first_line_indent_chars": first_line_indent_chars,
            "indent_each_paragraph": indent_each_paragraph,
            "shape": shape,
        }
        if font_id is not None:
            payload["font_id"] = str(font_id)
        if language is not None:
            payload["language"] = language
        output = _ffi.new("OfdBuffer *")
        _check(_lib.ofd_add_flowed_spans_json(
            self._handle, document_index, _utf8(json.dumps(payload)), output,
        ))
        try:
            return json.loads(bytes(_ffi.buffer(output.data, output.len)))
        finally:
            _lib.ofd_buffer_free(output[0])

    def append_text_page_with_font(
        self,
        document_index: int,
        text: str,
        font_id: int,
        width_mm: float = 210,
        height_mm: float = 297,
    ) -> None:
        """Append a text page using a declared font in the selected document."""
        self._require_open()
        if document_index < 0:
            raise IndexError(document_index)
        if font_id < 0:
            raise ValueError("font_id must be nonnegative")
        payload = {"operation": "append", "document_index": document_index,
                   "text": text, "width_mm": width_mm, "height_mm": height_mm,
                   "font_id": font_id}
        _check(_lib.ofd_write_text_page_json(
            self._handle, _utf8(json.dumps(payload)), _ffi.NULL))

    def append_path_page(
        self,
        document_index: int,
        commands: str,
        boundary: tuple[float, float, float, float],
        page_width_mm: float = 210,
        page_height_mm: float = 297,
        fill: bool = False,
        stroke: bool = True,
    ) -> None:
        """Append a vector path page to a new or existing document body."""
        self._require_open()
        if document_index < 0:
            raise IndexError(document_index)
        if len(boundary) != 4:
            raise ValueError("boundary must contain four numbers")
        _check(
            _lib.ofd_append_path_page(
                self._handle, document_index, _utf8(commands), *boundary,
                page_width_mm, page_height_mm, int(fill), int(stroke),
            )
        )

    def append_image_page(
        self,
        document_index: int,
        data: bytes,
        format: str,
        width_mm: float = 210,
        height_mm: float = 297,
    ) -> None:
        """Append a PNG, JPEG, BMP, single-page TIFF, or JBIG2 image page."""
        self._require_open()
        if document_index < 0:
            raise IndexError(document_index)
        _check(
            _lib.ofd_append_image_page(
                self._handle, document_index, data, len(data),
                _utf8(format), width_mm, height_mm,
            )
        )

    def add_image_page(
        self, data: bytes, format: str, width_mm: float = 210, height_mm: float = 297
    ) -> None:
        """Append a page containing a supported raster image, including JPEG2000."""
        self._require_open()
        _check(_lib.ofd_add_image_page(self._handle, data, len(data), _utf8(format), width_mm, height_mm))

    def add_image_object(
        self, page_index: int, data: bytes, format: str,
        boundary: tuple[float, float, float, float],
    ) -> int:
        """Add a supported raster image to a page."""
        self._require_open()
        if page_index < 0:
            raise IndexError(page_index)
        if len(boundary) != 4:
            raise ValueError("boundary must contain four numbers")
        output = _ffi.new("size_t *")
        _check(_lib.ofd_add_image_object(self._handle, page_index, data, len(data),
                                         _utf8(format), *boundary, output))
        return int(output[0])

    def add_composite_image_object(
        self, page_index: int, data: bytes, format: str,
        boundary: tuple[float, float, float, float],
    ) -> int:
        """Add a PNG, JPEG, BMP, single-page TIFF, or JBIG2 composite image."""
        self._require_open()
        if page_index < 0:
            raise IndexError(page_index)
        if len(boundary) != 4:
            raise ValueError("boundary must contain four numbers")
        output = _ffi.new("size_t *")
        _check(_lib.ofd_add_composite_image_object(
            self._handle, page_index, data, len(data), _utf8(format), *boundary, output,
        ))
        return int(output[0])

    def add_path_page(
        self,
        commands: str,
        boundary: tuple[float, float, float, float],
        page_width_mm: float = 210,
        page_height_mm: float = 297,
        fill: bool = False,
        stroke: bool = True,
    ) -> None:
        """Append a page containing one OFD vector path."""
        self._require_open()
        if len(boundary) != 4:
            raise ValueError("boundary must contain four numbers")
        _check(
            _lib.ofd_add_path_page(
                self._handle, _utf8(commands), *boundary,
                page_width_mm, page_height_mm, int(fill), int(stroke),
            )
        )

    def add_attachment(self, name: str, data: bytes) -> None:
        """Embed an attachment in a newly created document."""
        self._require_open()
        _check(_lib.ofd_add_attachment(self._handle, _utf8(name), data, len(data)))

    def add_custom_tag(self, document_index: int, type_id: str, data: bytes,
                       namespace: str | None = None, schema_path: str | None = None) -> dict:
        """Add a custom tag XML payload and return its index entry."""
        self._require_open()
        if document_index < 0:
            raise IndexError(document_index)
        metadata = json.dumps({"type_id": type_id, "namespace": namespace,
                               "schema_path": schema_path}, ensure_ascii=False)
        output = _ffi.new("OfdBuffer *")
        _check(_lib.ofd_add_custom_tag_json(self._handle, document_index,
                                             _utf8(metadata), data, len(data), output))
        try:
            return json.loads(bytes(_ffi.buffer(output.data, output.len)))
        finally:
            _lib.ofd_buffer_free(output[0])

    def update_custom_tag(self, index: int, type_id: str,
                          namespace: str | None = None,
                          schema_path: str | None = None) -> dict:
        """Update tag metadata without changing its XML payload."""
        self._require_open()
        if index < 0:
            raise IndexError(index)
        metadata = json.dumps({"type_id": type_id, "namespace": namespace,
                               "schema_path": schema_path}, ensure_ascii=False)
        output = _ffi.new("OfdBuffer *")
        _check(_lib.ofd_update_custom_tag_json(self._handle, index,
                                                _utf8(metadata), output))
        try:
            return json.loads(bytes(_ffi.buffer(output.data, output.len)))
        finally:
            _lib.ofd_buffer_free(output[0])

    def add_multimedia_resource(self, document_index: int, media_type: str,
                                format: str, data: bytes) -> int:
        """Add an Image, Audio or Video resource and return its OFD ID."""
        self._require_open()
        if document_index < 0:
            raise IndexError(document_index)
        output = _ffi.new("size_t *")
        _check(_lib.ofd_add_multimedia_resource(
            self._handle, document_index, _utf8(media_type), _utf8(format),
            data, len(data), output))
        return int(output[0])

    def add_page_multimedia_resource(self, page_index: int, media_type: str,
                                     format: str, data: bytes) -> int:
        """Add a page-local Image, Audio or Video resource and return its ID."""
        self._require_open()
        if page_index < 0:
            raise IndexError(page_index)
        output = _ffi.new("size_t *")
        _check(_lib.ofd_add_page_multimedia_resource(
            self._handle, page_index, _utf8(media_type), _utf8(format),
            data, len(data), output))
        return int(output[0])

    def replace_document_multimedia_data(self, document_index: int,
                                         resource_id: str | int, data: bytes) -> None:
        """Replace resource bytes while preserving the OFD resource ID."""
        self._require_open()
        if document_index < 0:
            raise IndexError(document_index)
        _check(_lib.ofd_replace_document_multimedia_data(
            self._handle, document_index, _utf8(str(resource_id)), data, len(data)))

    def replace_page_multimedia_data(self, page_index: int,
                                     resource_id: str | int, data: bytes) -> None:
        """Replace the multimedia resource visible to a page."""
        self._require_open()
        if page_index < 0:
            raise IndexError(page_index)
        _check(_lib.ofd_replace_page_multimedia_data(
            self._handle, page_index, _utf8(str(resource_id)), data, len(data)))

    def remove_document_multimedia_resource(self, document_index: int,
                                            resource_id: str | int) -> None:
        """Remove an unreferenced multimedia resource definition."""
        self._require_open()
        if document_index < 0:
            raise IndexError(document_index)
        _check(_lib.ofd_remove_document_multimedia_resource(
            self._handle, document_index, _utf8(str(resource_id))))

    def remove_page_multimedia_resource(self, page_index: int,
                                        resource_id: str | int) -> None:
        """Remove an unreferenced multimedia definition declared by a page."""
        self._require_open()
        if page_index < 0:
            raise IndexError(page_index)
        _check(_lib.ofd_remove_page_multimedia_resource(
            self._handle, page_index, _utf8(str(resource_id))))

    def add_attachment_to_document(self, document_index: int, name: str, data: bytes) -> None:
        """Embed an attachment in a selected document body."""
        self._require_open()
        if document_index < 0:
            raise IndexError(document_index)
        _check(
            _lib.ofd_add_attachment_to_document(
                self._handle, document_index, _utf8(name), data, len(data)
            )
        )

    def remove_attachment(self, index: int) -> None:
        """Remove one attachment by global index, retaining its package bytes."""
        self._require_open()
        if index < 0:
            raise IndexError(index)
        _check(_lib.ofd_remove_attachment(self._handle, index))

    def rename_attachment(self, index: int, name: str) -> None:
        """Rename an attachment while preserving its ID and bytes."""
        self._require_open()
        if index < 0:
            raise IndexError(index)
        _check(_lib.ofd_rename_attachment(self._handle, index, _utf8(name)))

    def replace_attachment_data(self, index: int, data: bytes) -> None:
        """Replace attachment bytes while preserving its ID and metadata."""
        self._require_open()
        if index < 0:
            raise IndexError(index)
        _check(_lib.ofd_replace_attachment_data(self._handle, index, data, len(data)))

    def set_attachment_metadata(
        self, index: int, *, format: str | None = None,
        creation_date: str | None = None, mod_date: str | None = None,
        visible: bool = True, usage: str | None = None,
    ) -> None:
        """Replace an attachment's standard metadata by global index."""
        self._require_open()
        if index < 0:
            raise IndexError(index)
        payload = json.dumps({
            "format": format, "creation_date": creation_date, "mod_date": mod_date,
            "visible": visible, "usage": usage,
        }, ensure_ascii=False)
        _check(_lib.ofd_set_attachment_metadata_json(
            self._handle, index, _utf8(payload)))

    def save(self, path: str | os.PathLike[str]) -> None:
        self._require_open()
        _check(_lib.ofd_save(self._handle, _utf8(path)))

    def to_bytes(self) -> bytes:
        """Serialize this OFD package to ZIP bytes."""
        self._require_open()
        output = _ffi.new("OfdBuffer *")
        _check(_lib.ofd_to_bytes(self._handle, output))
        try:
            return bytes(_ffi.buffer(output.data, output.len))
        finally:
            _lib.ofd_buffer_free(output[0])


__all__ = ["Document", "OfdError", "create_ses_seal"]
