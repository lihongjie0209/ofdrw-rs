//! Versioned C ABI for OFD package operations.

use std::cell::RefCell;
use std::collections::HashMap;
use std::ffi::{c_char, CStr, CString};
use std::panic::{catch_unwind, AssertUnwindSafe};
use std::ptr;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Mutex, OnceLock};

use base64::{engine::general_purpose::STANDARD, Engine as _};
use ofdrw::{
    create_ses_seal_with_distid, ActionRegionInfo, AnnotationGraphic, AnnotationObjectUpdate,
    AnnotationPath, ArchiveSeverity, AttachmentMetadata, CertificateRecipient, CgTransformInfo,
    ClipArea, ColorSpaceInfo, ColorSpaceSpec, ColoredShapedTextOptions, CompositeStyle,
    CustomTagInfo, DestinationInfo, DocumentActionTarget, DocumentPermissions, DrawParamStyle,
    EncryptionRecipient, ExtensionInfo, ExtensionProperty, FlowSpan, FlowTextAlign, FlowTextStyle,
    FontInfo, GouraudControlPoint, GouraudFillStyle, GradientFillStyle, GradientKind,
    GradientMapType, GradientSegment, ImageBorderStyle, NestedTable, OfdDocument, PageArea,
    PageObjectActionInfo, PasswordRecipient, PathClipArea, PathLineCap, PathLineJoin,
    PathStrokeStyle, PathStyle, PatternCell, PatternReflectMethod, PatternRelativeTo, PatternStyle,
    PrintPermission, RenderOptions, SesSealSpec, ShapeAnnotationStyle,
    SignatureChainVerificationReport, Sm2DistinguishingIds, SolidPaintStyle, SoundActionOptions,
    TableCell, TableCellBlock, TableCellImage, TableStyle, TableVerticalAlign, TextClipArea,
    TextCodeInfo, TextCodeLayout, TextNoteStyle, TextTypographyStyle, TextWatermarkStyle,
    ValidPeriod, VersionInfo, ViewPreferences,
};
use serde::Deserialize;

/// Current ABI major version.
pub const ABI_VERSION: u32 = 1;
const DEFAULT_SM2_DISTID_C: &[u8] = b"1234567812345678\0";

/// A byte buffer owned by the Rust library.
#[repr(C)]
pub struct OfdBuffer {
    /// Byte pointer; null if the buffer is empty.
    pub data: *mut u8,
    /// Number of bytes at `data`.
    pub len: usize,
}

thread_local! {
    static LAST_ERROR: RefCell<CString> = RefCell::new(CString::new("").unwrap());
}

// All ABI calls are serialized while they borrow documents from the registry.
// Handles are monotonically increasing tokens, so a closed handle cannot refer
// to a newly opened document even when the allocator reuses memory.
static CALL_LOCK: Mutex<()> = Mutex::new(());
static DOCUMENTS: OnceLock<Mutex<HashMap<usize, Box<OfdDocument>>>> = OnceLock::new();
static NEXT_HANDLE: AtomicUsize = AtomicUsize::new(1);

fn documents() -> &'static Mutex<HashMap<usize, Box<OfdDocument>>> {
    DOCUMENTS.get_or_init(|| Mutex::new(HashMap::new()))
}

fn register_document(document: OfdDocument) -> Result<*mut OfdDocument, String> {
    let id = NEXT_HANDLE
        .fetch_update(Ordering::Relaxed, Ordering::Relaxed, |id| id.checked_add(1))
        .map_err(|_| "document handle space exhausted")?;
    documents().lock().unwrap().insert(id, Box::new(document));
    Ok(ptr::without_provenance_mut(id))
}

fn set_error(message: &str) {
    let clean = message.replace('\0', " ");
    LAST_ERROR.with(|slot| *slot.borrow_mut() = CString::new(clean).unwrap_or_default());
}

fn run<F: FnOnce() -> Result<(), String>>(f: F) -> i32 {
    let _call = CALL_LOCK.lock().unwrap();
    match catch_unwind(AssertUnwindSafe(f)) {
        Ok(Ok(())) => {
            set_error("");
            0
        }
        Ok(Err(error)) => {
            set_error(&error);
            2
        }
        Err(_) => {
            set_error("internal Rust panic");
            3
        }
    }
}

unsafe fn input_string<'a>(ptr: *const c_char) -> Result<&'a str, String> {
    if ptr.is_null() {
        return Err("null string pointer".into());
    }
    CStr::from_ptr(ptr).to_str().map_err(|e| e.to_string())
}

unsafe fn input_bytes<'a>(ptr: *const u8, len: usize) -> Result<&'a [u8], String> {
    if len == 0 {
        return Ok(&[]);
    }
    if ptr.is_null() {
        return Err("null byte pointer".into());
    }
    Ok(std::slice::from_raw_parts(ptr, len))
}

unsafe fn document_ref<'a>(ptr: *const OfdDocument) -> Result<&'a OfdDocument, String> {
    if ptr.is_null() {
        return Err("null document handle".into());
    }
    let registry = documents().lock().unwrap();
    let document = registry
        .get(&ptr.addr())
        .ok_or("closed or invalid document handle")?;
    let address = document.as_ref() as *const OfdDocument;
    // The enclosing run() holds CALL_LOCK, preventing ofd_close from removing
    // the Box until this reference is no longer used.
    Ok(&*address)
}

unsafe fn document_mut<'a>(ptr: *mut OfdDocument) -> Result<&'a mut OfdDocument, String> {
    if ptr.is_null() {
        return Err("null document handle".into());
    }
    let mut registry = documents().lock().unwrap();
    let document = registry
        .get_mut(&ptr.addr())
        .ok_or("closed or invalid document handle")?;
    let address = document.as_mut() as *mut OfdDocument;
    // The enclosing run() holds CALL_LOCK, preventing another ABI call from
    // borrowing or removing this document while the mutable borrow is live.
    Ok(&mut *address)
}

fn output_buffer(bytes: Vec<u8>, output: *mut OfdBuffer) -> Result<(), String> {
    let output = unsafe { output.as_mut() }.ok_or("null output buffer")?;
    if bytes.is_empty() {
        *output = OfdBuffer {
            data: ptr::null_mut(),
            len: 0,
        };
    } else {
        let mut boxed = bytes.into_boxed_slice();
        *output = OfdBuffer {
            data: boxed.as_mut_ptr(),
            len: boxed.len(),
        };
        std::mem::forget(boxed);
    }
    Ok(())
}

fn destination_json(destination: &DestinationInfo) -> serde_json::Value {
    serde_json::json!({
        "page_id": destination.page_id,
        "kind": destination.kind,
        "left": destination.left,
        "top": destination.top,
        "right": destination.right,
        "bottom": destination.bottom,
        "zoom": destination.zoom,
    })
}

fn action_region_json(region: &ActionRegionInfo) -> serde_json::Value {
    serde_json::json!({
        "areas": region.areas.iter().map(|area| serde_json::json!({
            "start": area.start,
            "commands": area.commands.iter().map(|command| serde_json::json!({
                "kind": command.kind,
                "attributes": command.attributes,
            })).collect::<Vec<_>>(),
        })).collect::<Vec<_>>(),
    })
}

fn page_object_action_json(action: &PageObjectActionInfo) -> serde_json::Value {
    serde_json::json!({
        "event": action.event,
        "kind": action.kind,
        "attributes": action.attributes,
        "destination": action.destination.as_ref().map(destination_json),
        "bookmark": action.bookmark,
        "region": action.region.as_ref().map(action_region_json),
    })
}

fn parse_destination_value(input: &serde_json::Value) -> Result<DestinationInfo, String> {
    let string = |key: &str| -> Result<String, String> {
        input
            .get(key)
            .and_then(serde_json::Value::as_str)
            .map(str::to_owned)
            .ok_or_else(|| format!("destination {key} must be a string"))
    };
    let number = |key: &str| -> Result<Option<f64>, String> {
        match input.get(key) {
            None | Some(serde_json::Value::Null) => Ok(None),
            Some(value) => value
                .as_f64()
                .map(Some)
                .ok_or_else(|| format!("destination {key} must be a number")),
        }
    };
    Ok(DestinationInfo {
        page_id: string("page_id")?,
        kind: string("kind")?,
        left: number("left")?,
        top: number("top")?,
        right: number("right")?,
        bottom: number("bottom")?,
        zoom: number("zoom")?,
    })
}

fn version_json(version: VersionInfo) -> serde_json::Value {
    let files: Vec<_> = version
        .files
        .into_iter()
        .map(|file| serde_json::json!({"id": file.id, "path": file.path}))
        .collect();
    serde_json::json!({
        "document_index": version.document_index,
        "id": version.id,
        "index": version.index,
        "current": version.current,
        "description_path": version.description_path,
        "doc_root": version.doc_root,
        "format_version": version.format_version,
        "name": version.name,
        "creation_date": version.creation_date,
        "files": files,
    })
}

/// Return the ABI major version.
#[no_mangle]
pub extern "C" fn ofd_abi_version() -> u32 {
    ABI_VERSION
}

/// Return the most recent error for the current thread.
///
/// The pointer remains valid until the next ABI call on this thread.
#[no_mangle]
pub extern "C" fn ofd_last_error() -> *const c_char {
    LAST_ERROR.with(|slot| slot.borrow().as_ptr())
}

/// Open an OFD file. The caller must release the returned handle with `ofd_close`.
///
/// # Safety
/// `path` must be a valid NUL-terminated string and `output` must point to writable memory.
#[no_mangle]
pub unsafe extern "C" fn ofd_open(path: *const c_char, output: *mut *mut OfdDocument) -> i32 {
    run(|| {
        let output = output.as_mut().ok_or("null output handle")?;
        *output = ptr::null_mut();
        let path = input_string(path)?;
        let document = OfdDocument::open(path).map_err(|e| e.to_string())?;
        *output = register_document(document)?;
        Ok(())
    })
}

/// Open an OFD package from ZIP bytes.
///
/// # Safety
/// `data` must point to `len` readable bytes, unless `len` is zero. `output`
/// must point to writable memory; release the returned handle with `ofd_close`.
#[no_mangle]
pub unsafe extern "C" fn ofd_from_bytes(
    data: *const u8,
    len: usize,
    output: *mut *mut OfdDocument,
) -> i32 {
    run(|| {
        let output = output.as_mut().ok_or("null output handle")?;
        *output = ptr::null_mut();
        let document =
            OfdDocument::from_bytes(input_bytes(data, len)?).map_err(|error| error.to_string())?;
        *output = register_document(document)?;
        Ok(())
    })
}

/// Create an empty OFD package.
///
/// # Safety
/// `output` must point to writable memory.
#[no_mangle]
pub unsafe extern "C" fn ofd_new(output: *mut *mut OfdDocument) -> i32 {
    run(|| {
        let output = output.as_mut().ok_or("null output handle")?;
        *output = register_document(OfdDocument::new())?;
        Ok(())
    })
}

/// Report whether a package has OFD 2.0 encryption metadata.
///
/// # Safety
/// `handle` must be live and `output` writable.
#[no_mangle]
pub unsafe extern "C" fn ofd_is_encrypted(handle: *const OfdDocument, output: *mut i32) -> i32 {
    run(|| {
        let output = output.as_mut().ok_or("null encryption status output")?;
        *output = i32::from(document_ref(handle)?.is_encrypted());
        Ok(())
    })
}

/// List encryption operations and user names as JSON.
///
/// # Safety
/// `handle` must be live and `output` writable.
#[no_mangle]
pub unsafe extern "C" fn ofd_encryption_info_json(
    handle: *const OfdDocument,
    output: *mut OfdBuffer,
) -> i32 {
    run(|| {
        let values: Vec<_> = document_ref(handle)?
            .encryption_info()
            .map_err(|error| error.to_string())?
            .into_iter()
            .map(|info| {
                serde_json::json!({
                    "id": info.id, "scope": info.scope,
                    "decrypt_seed_path": info.decrypt_seed_path,
                    "entries_map_path": info.entries_map_path,
                    "usernames": info.usernames,
                    "protection_case_id": info.protection_case_id,
                })
            })
            .collect();
        output_buffer(
            serde_json::to_vec(&values).map_err(|error| error.to_string())?,
            output,
        )
    })
}

/// Decrypt a password-protected OFD into a new document handle.
///
/// # Safety
/// `handle` must be live, `password` NUL-terminated, `username` null or
/// NUL-terminated, and `output` writable. Close a successful handle with `ofd_close`.
#[no_mangle]
pub unsafe extern "C" fn ofd_decrypt_with_password(
    handle: *const OfdDocument,
    password: *const c_char,
    username: *const c_char,
    output: *mut *mut OfdDocument,
) -> i32 {
    run(|| {
        let output = output.as_mut().ok_or("null decrypted document output")?;
        *output = ptr::null_mut();
        let username = if username.is_null() {
            None
        } else {
            Some(input_string(username)?)
        };
        let document = document_ref(handle)?
            .decrypt_with_password(input_string(password)?, username)
            .map_err(|error| error.to_string())?;
        *output = register_document(document)?;
        Ok(())
    })
}

/// Decrypt an SM2 certificate-protected OFD with a raw 32-byte private key.
///
/// # Safety
/// `handle` must be live, `private_key` must point to `key_len` readable bytes,
/// `username` must be null or NUL-terminated, and `output` writable.
#[no_mangle]
pub unsafe extern "C" fn ofd_decrypt_with_sm2_private_key(
    handle: *const OfdDocument,
    private_key: *const u8,
    key_len: usize,
    username: *const c_char,
    output: *mut *mut OfdDocument,
) -> i32 {
    run(|| {
        let output = output.as_mut().ok_or("null decrypted document output")?;
        *output = ptr::null_mut();
        let username = if username.is_null() {
            None
        } else {
            Some(input_string(username)?)
        };
        let document = document_ref(handle)?
            .decrypt_with_sm2_private_key(input_bytes(private_key, key_len)?, username)
            .map_err(|error| error.to_string())?;
        *output = register_document(document)?;
        Ok(())
    })
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct PasswordRecipientInput {
    username: String,
    password: String,
    #[serde(default)]
    user_type: Option<String>,
}

/// Encrypt an OFD with password users into a new document handle.
///
/// `recipients_json` is an array of objects with username, password and
/// optional user_type (User or Owner).
///
/// # Safety
/// `handle` must be live, `recipients_json` NUL-terminated and `output`
/// writable. Close a successful handle with `ofd_close`.
#[no_mangle]
pub unsafe extern "C" fn ofd_encrypt_with_passwords_json(
    handle: *const OfdDocument,
    recipients_json: *const c_char,
    output: *mut *mut OfdDocument,
) -> i32 {
    run(|| {
        let output = output.as_mut().ok_or("null encrypted document output")?;
        *output = ptr::null_mut();
        let recipients: Vec<PasswordRecipientInput> =
            serde_json::from_str(input_string(recipients_json)?)
                .map_err(|error| error.to_string())?;
        let recipients: Vec<_> = recipients
            .into_iter()
            .map(|user| PasswordRecipient {
                username: user.username,
                password: user.password,
                user_type: user.user_type,
            })
            .collect();
        let document = document_ref(handle)?
            .encrypt_with_passwords(&recipients)
            .map_err(|error| error.to_string())?;
        *output = register_document(document)?;
        Ok(())
    })
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct CertificateRecipientInput {
    username: String,
    certificate_der_base64: String,
    #[serde(default)]
    user_type: Option<String>,
}

/// Encrypt an OFD for SM2 X.509 certificate users.
///
/// `recipients_json` contains username, certificate_der_base64 and optional
/// user_type (User or Owner) for each recipient.
///
/// # Safety
/// `handle` must be live, `recipients_json` NUL-terminated and `output` writable.
#[no_mangle]
pub unsafe extern "C" fn ofd_encrypt_with_certificates_json(
    handle: *const OfdDocument,
    recipients_json: *const c_char,
    output: *mut *mut OfdDocument,
) -> i32 {
    run(|| {
        let output = output.as_mut().ok_or("null encrypted document output")?;
        *output = ptr::null_mut();
        let recipients: Vec<CertificateRecipientInput> =
            serde_json::from_str(input_string(recipients_json)?)
                .map_err(|error| error.to_string())?;
        let recipients = recipients
            .into_iter()
            .map(|user| {
                Ok(CertificateRecipient {
                    username: user.username,
                    certificate_der: STANDARD
                        .decode(user.certificate_der_base64)
                        .map_err(|_| "invalid certificate_der_base64")?,
                    user_type: user.user_type,
                })
            })
            .collect::<Result<Vec<_>, &str>>()?;
        let document = document_ref(handle)?
            .encrypt_with_certificates(&recipients)
            .map_err(|error| error.to_string())?;
        *output = register_document(document)?;
        Ok(())
    })
}

#[derive(Deserialize)]
#[serde(tag = "type", rename_all = "lowercase", deny_unknown_fields)]
enum EncryptionRecipientInput {
    Password {
        username: String,
        password: String,
        #[serde(default)]
        user_type: Option<String>,
    },
    Certificate {
        username: String,
        certificate_der_base64: String,
        #[serde(default)]
        user_type: Option<String>,
    },
}

/// Encrypt an OFD for any mix of password and SM2 certificate recipients.
///
/// # Safety
/// `handle` must be live, `recipients_json` NUL-terminated and `output` writable.
#[no_mangle]
pub unsafe extern "C" fn ofd_encrypt_with_recipients_json(
    handle: *const OfdDocument,
    recipients_json: *const c_char,
    output: *mut *mut OfdDocument,
) -> i32 {
    run(|| {
        let output = output.as_mut().ok_or("null encrypted document output")?;
        *output = ptr::null_mut();
        let recipients: Vec<EncryptionRecipientInput> =
            serde_json::from_str(input_string(recipients_json)?)
                .map_err(|error| error.to_string())?;
        let recipients = recipients
            .into_iter()
            .map(|user| match user {
                EncryptionRecipientInput::Password {
                    username,
                    password,
                    user_type,
                } => Ok(EncryptionRecipient::Password(PasswordRecipient {
                    username,
                    password,
                    user_type,
                })),
                EncryptionRecipientInput::Certificate {
                    username,
                    certificate_der_base64,
                    user_type,
                } => Ok(EncryptionRecipient::Certificate(CertificateRecipient {
                    username,
                    certificate_der: STANDARD
                        .decode(certificate_der_base64)
                        .map_err(|_| "invalid certificate_der_base64")?,
                    user_type,
                })),
            })
            .collect::<Result<Vec<_>, &str>>()?;
        let document = document_ref(handle)?
            .encrypt_with_recipients(&recipients)
            .map_err(|error| error.to_string())?;
        *output = register_document(document)?;
        Ok(())
    })
}

/// Import supported PDF vector pages into a new OFD document.
///
/// # Safety
/// `data` must point to `len` readable bytes unless `len` is zero; `output`
/// must point to writable memory. Release a successful handle with `ofd_close`.
#[no_mangle]
pub unsafe extern "C" fn ofd_from_pdf_bytes(
    data: *const u8,
    len: usize,
    output: *mut *mut OfdDocument,
) -> i32 {
    run(|| {
        let output = output.as_mut().ok_or("null output handle")?;
        *output = ptr::null_mut();
        let document = OfdDocument::from_pdf_bytes(input_bytes(data, len)?)
            .map_err(|error| error.to_string())?;
        *output = register_document(document)?;
        Ok(())
    })
}

/// Release a document handle. Null is accepted.
///
/// # Safety
/// An unknown or already closed handle is ignored.
#[no_mangle]
pub unsafe extern "C" fn ofd_close(handle: *mut OfdDocument) {
    if !handle.is_null() {
        let _call = CALL_LOCK.lock().unwrap();
        documents().lock().unwrap().remove(&handle.addr());
    }
}

/// Get the number of pages in an OFD package.
///
/// # Safety
/// `handle` must be live and `output` must point to writable memory.
#[no_mangle]
pub unsafe extern "C" fn ofd_page_count(handle: *const OfdDocument, output: *mut usize) -> i32 {
    run(|| {
        let output = output.as_mut().ok_or("null page count output")?;
        *output = document_ref(handle)?
            .pages()
            .map_err(|e| e.to_string())?
            .len();
        Ok(())
    })
}

/// Append an empty document body and return its zero-based index.
///
/// # Safety
/// `handle` must be live and `output_index` must point to writable memory.
#[no_mangle]
pub unsafe extern "C" fn ofd_append_document_body(
    handle: *mut OfdDocument,
    output_index: *mut usize,
) -> i32 {
    run(|| {
        let output_index = output_index.as_mut().ok_or("null document index output")?;
        *output_index = document_mut(handle)?
            .append_document_body()
            .map_err(|error| error.to_string())?;
        Ok(())
    })
}

/// Move a document body to another zero-based position in OFD.xml.
///
/// # Safety
/// `handle` must point to a live document.
#[no_mangle]
pub unsafe extern "C" fn ofd_move_document_body(
    handle: *mut OfdDocument,
    from_index: usize,
    to_index: usize,
) -> i32 {
    run(|| {
        document_mut(handle)?
            .move_document_body(from_index, to_index)
            .map_err(|error| error.to_string())
    })
}

/// Remove a document body from OFD.xml while retaining its package entries.
///
/// # Safety
/// `handle` must point to a live document.
#[no_mangle]
pub unsafe extern "C" fn ofd_remove_document_body(handle: *mut OfdDocument, index: usize) -> i32 {
    run(|| {
        document_mut(handle)?
            .remove_document_body(index)
            .map_err(|error| error.to_string())
    })
}

/// Extract one active document body into a separate OFD handle.
///
/// # Safety
/// `handle` must be live and `output` writable. Release the returned handle
/// with `ofd_close`.
#[no_mangle]
pub unsafe extern "C" fn ofd_extract_document_body(
    handle: *const OfdDocument,
    index: usize,
    output: *mut *mut OfdDocument,
) -> i32 {
    run(|| {
        let output = output.as_mut().ok_or("null extracted document output")?;
        *output = ptr::null_mut();
        let extracted = document_ref(handle)?
            .extract_document_body(index)
            .map_err(|error| error.to_string())?;
        *output = register_document(extracted)?;
        Ok(())
    })
}

/// Append all document bodies from an unsigned source package.
///
/// # Safety
/// Both handles must be live and `output` writable. The source may equal the
/// target; its state is cloned before the target is changed.
#[no_mangle]
pub unsafe extern "C" fn ofd_append_document_bodies_from_json(
    handle: *mut OfdDocument,
    source_handle: *const OfdDocument,
    output: *mut OfdBuffer,
) -> i32 {
    run(|| {
        if output.is_null() {
            return Err("null imported indexes output".into());
        }
        let source = document_ref(source_handle)?.clone();
        let indexes = document_mut(handle)?
            .append_document_bodies_from(&source)
            .map_err(|error| error.to_string())?;
        output_buffer(
            serde_json::to_vec(&indexes).map_err(|error| error.to_string())?,
            output,
        )
    })
}

/// Append one selected body from another unsigned OFD package.
///
/// # Safety
/// Both handles must be live and `output_index` writable. The source may equal
/// the target; its state is cloned before mutation.
#[no_mangle]
pub unsafe extern "C" fn ofd_append_document_body_from(
    handle: *mut OfdDocument,
    source_handle: *const OfdDocument,
    source_index: usize,
    output_index: *mut usize,
) -> i32 {
    run(|| {
        let output = output_index.as_mut().ok_or("null imported index output")?;
        let source = document_ref(source_handle)?.clone();
        *output = document_mut(handle)?
            .append_document_body_from(&source, source_index)
            .map_err(|error| error.to_string())?;
        Ok(())
    })
}

/// Get page metadata as a UTF-8 JSON array.
///
/// # Safety
/// `handle` must be live and `output` must point to writable memory.
#[no_mangle]
pub unsafe extern "C" fn ofd_pages_json(handle: *const OfdDocument, output: *mut OfdBuffer) -> i32 {
    run(|| {
        let pages = document_ref(handle)?.pages().map_err(|e| e.to_string())?;
        let values: Vec<_> = pages
            .into_iter()
            .map(|page| {
                serde_json::json!({
                    "index": page.index,
                    "document_index": page.document_index,
                    "id": page.id,
                    "path": page.path,
                    "physical_box": page.physical_box,
                    "application_box": page.application_box,
                    "content_box": page.content_box,
                    "bleed_box": page.bleed_box,
                })
            })
            .collect();
        output_buffer(
            serde_json::to_vec(&values).map_err(|e| e.to_string())?,
            output,
        )
    })
}

fn page_area_box(value: &serde_json::Value, name: &str) -> Result<Option<[f64; 4]>, String> {
    let Some(value) = value.get(name) else {
        return Ok(None);
    };
    if value.is_null() {
        return Ok(None);
    }
    let array = value
        .as_array()
        .ok_or_else(|| format!("{name} must be an array of four numbers"))?;
    if array.len() != 4 {
        return Err(format!("{name} must have four numbers"));
    }
    let mut bounds = [0.0; 4];
    for (index, item) in array.iter().enumerate() {
        bounds[index] = item
            .as_f64()
            .ok_or_else(|| format!("{name} must contain numbers"))?;
    }
    Ok(Some(bounds))
}

unsafe fn parse_page_area_json(area_json: *const c_char) -> Result<PageArea, String> {
    let value: serde_json::Value =
        serde_json::from_str(input_string(area_json)?).map_err(|error| error.to_string())?;
    if !value.is_object() {
        return Err("page area must be a JSON object".into());
    }
    Ok(PageArea {
        physical_box: page_area_box(&value, "physical_box")?.ok_or("physical_box is required")?,
        application_box: page_area_box(&value, "application_box")?,
        content_box: page_area_box(&value, "content_box")?,
        bleed_box: page_area_box(&value, "bleed_box")?,
    })
}

/// Read one document body's explicit default page area as JSON, or `null`.
///
/// # Safety
/// `handle` must be live and `output` must point to writable memory.
#[no_mangle]
pub unsafe extern "C" fn ofd_document_page_area_json(
    handle: *const OfdDocument,
    document_index: usize,
    output: *mut OfdBuffer,
) -> i32 {
    run(|| {
        let area = document_ref(handle)?
            .document_page_area(document_index)
            .map_err(|error| error.to_string())?;
        let value = area.map(|area| {
            serde_json::json!({
                "physical_box": area.physical_box,
                "application_box": area.application_box,
                "content_box": area.content_box,
                "bleed_box": area.bleed_box,
            })
        });
        output_buffer(
            serde_json::to_vec(&value).map_err(|error| error.to_string())?,
            output,
        )
    })
}

/// Set one document body's default page area from UTF-8 JSON.
///
/// # Safety
/// `handle` must be live and `area_json` a NUL-terminated string.
#[no_mangle]
pub unsafe extern "C" fn ofd_set_document_page_area_json(
    handle: *mut OfdDocument,
    document_index: usize,
    area_json: *const c_char,
) -> i32 {
    run(|| {
        let area = parse_page_area_json(area_json)?;
        document_mut(handle)?
            .set_document_page_area(document_index, area)
            .map_err(|error| error.to_string())
    })
}

/// Replace the explicit area of a zero-based page with four OFD boxes.
///
/// `area_json` is a NUL-terminated object with a required `physical_box`
/// array and optional `application_box`, `content_box`, and `bleed_box` arrays.
/// Each array contains four numbers in millimeters.
///
/// # Safety
/// `handle` must be live and `area_json` must be a valid NUL-terminated string.
#[no_mangle]
pub unsafe extern "C" fn ofd_set_page_area_json(
    handle: *mut OfdDocument,
    index: usize,
    area_json: *const c_char,
) -> i32 {
    run(|| {
        let area = parse_page_area_json(area_json)?;
        document_mut(handle)?
            .set_page_area(index, area)
            .map_err(|error| error.to_string())
    })
}

/// Check OFD object IDs, geometry, and supported resource references.
///
/// # Safety
/// `handle` must be live and `output` must point to writable memory.
#[no_mangle]
pub unsafe extern "C" fn ofd_validate_references_json(
    handle: *const OfdDocument,
    output: *mut OfdBuffer,
) -> i32 {
    run(|| {
        let issues = document_ref(handle)?
            .validate_references()
            .map_err(|e| e.to_string())?;
        let values: Vec<_> = issues
            .into_iter()
            .map(|issue| {
                serde_json::json!({
                    "path": issue.path,
                    "code": issue.code,
                    "message": issue.message,
                })
            })
            .collect();
        output_buffer(
            serde_json::to_vec(&values).map_err(|e| e.to_string())?,
            output,
        )
    })
}

/// Return the supported OFD-A archival violations as JSON.
///
/// # Safety
/// `handle` must be live and `output` writable.
#[no_mangle]
pub unsafe extern "C" fn ofd_archive_violations_json(
    handle: *const OfdDocument,
    output: *mut OfdBuffer,
) -> i32 {
    run(|| {
        let issues = document_ref(handle)?
            .archive_violations()
            .map_err(|error| error.to_string())?;
        let values = issues
            .into_iter()
            .map(|issue| {
                serde_json::json!({
                    "rule": issue.rule,
                    "severity": match issue.severity {
                        ArchiveSeverity::Error => "ERROR",
                        ArchiveSeverity::Warn => "WARN",
                        ArchiveSeverity::Info => "INFO",
                    },
                    "message": issue.message,
                    "path": issue.path,
                    "actual": issue.actual,
                    "expected": issue.expected,
                })
            })
            .collect::<Vec<_>>();
        output_buffer(
            serde_json::to_vec(&values).map_err(|error| error.to_string())?,
            output,
        )
    })
}

/// Return a separate OFD package with supported archival normalizations applied.
///
/// # Safety
/// `handle` must be live and `output` writable. Close the returned handle with `ofd_close`.
#[no_mangle]
pub unsafe extern "C" fn ofd_prepare_archive(
    handle: *const OfdDocument,
    output: *mut *mut OfdDocument,
) -> i32 {
    run(|| {
        let output = output.as_mut().ok_or("null archival document output")?;
        *output = ptr::null_mut();
        let document = document_ref(handle)?
            .prepare_archive()
            .map_err(|error| error.to_string())?;
        *output = register_document(document)?;
        Ok(())
    })
}

/// Get normalized package entry names as a UTF-8 JSON array.
///
/// # Safety
/// `handle` must be live and `output` must point to writable memory.
#[no_mangle]
pub unsafe extern "C" fn ofd_entries_json(
    handle: *const OfdDocument,
    output: *mut OfdBuffer,
) -> i32 {
    run(|| {
        let entries: Vec<_> = document_ref(handle)?.entry_names().collect();
        output_buffer(
            serde_json::to_vec(&entries).map_err(|e| e.to_string())?,
            output,
        )
    })
}

/// Get document metadata as a UTF-8 JSON array.
///
/// # Safety
/// `handle` must be live and `output` must point to writable memory.
#[no_mangle]
pub unsafe extern "C" fn ofd_documents_json(
    handle: *const OfdDocument,
    output: *mut OfdBuffer,
) -> i32 {
    run(|| {
        let infos = document_ref(handle)?
            .document_info()
            .map_err(|e| e.to_string())?;
        let values: Vec<_> = infos
            .into_iter()
            .map(|info| {
                let custom_data: Vec<_> = info
                    .custom_data
                    .into_iter()
                    .map(|(name, value)| serde_json::json!({ "name": name, "value": value }))
                    .collect();
                serde_json::json!({
                    "index": info.index, "root": info.root, "doc_id": info.doc_id,
                    "title": info.title, "author": info.author, "subject": info.subject,
                    "abstract": info.abstract_text, "creation_date": info.creation_date,
                    "mod_date": info.mod_date, "creator": info.creator,
                    "creator_version": info.creator_version, "doc_usage": info.doc_usage,
                    "cover": info.cover,
                    "keywords": info.keywords, "custom_data": custom_data,
                })
            })
            .collect();
        output_buffer(
            serde_json::to_vec(&values).map_err(|e| e.to_string())?,
            output,
        )
    })
}

/// Return the root OFD.xml version and document type as JSON.
///
/// # Safety
/// `handle` must be live and `output` writable.
#[no_mangle]
pub unsafe extern "C" fn ofd_package_info_json(
    handle: *const OfdDocument,
    output: *mut OfdBuffer,
) -> i32 {
    run(|| {
        let info = document_ref(handle)?
            .package_info()
            .map_err(|e| e.to_string())?;
        output_buffer(
            serde_json::to_vec(
                &serde_json::json!({"version": info.version, "doc_type": info.doc_type}),
            )
            .map_err(|e| e.to_string())?,
            output,
        )
    })
}

/// Update the root OFD.xml version and document type.
///
/// # Safety
/// `handle` must be live; strings must be NUL-terminated.
#[no_mangle]
pub unsafe extern "C" fn ofd_set_package_header(
    handle: *mut OfdDocument,
    version: *const c_char,
    doc_type: *const c_char,
) -> i32 {
    run(|| {
        document_mut(handle)?
            .set_package_header(input_string(version)?, input_string(doc_type)?)
            .map_err(|e| e.to_string())
    })
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct PrintPermissionInput {
    printable: bool,
    copies: i32,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct ValidPeriodInput {
    start_date: Option<String>,
    end_date: Option<String>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct DocumentPermissionsInput {
    edit: bool,
    annot: bool,
    export: bool,
    signature: bool,
    watermark: bool,
    print_screen: bool,
    print: PrintPermissionInput,
    valid_period: Option<ValidPeriodInput>,
}

/// Get effective document permissions as UTF-8 JSON.
///
/// # Safety
/// `handle` must be live and `output` must point to writable memory.
#[no_mangle]
pub unsafe extern "C" fn ofd_document_permissions_json(
    handle: *const OfdDocument,
    document_index: usize,
    output: *mut OfdBuffer,
) -> i32 {
    run(|| {
        let p = document_ref(handle)?
            .document_permissions(document_index)
            .map_err(|e| e.to_string())?;
        let value = serde_json::json!({
            "edit": p.edit, "annot": p.annot, "export": p.export,
            "signature": p.signature, "watermark": p.watermark,
            "print_screen": p.print_screen,
            "print": {"printable": p.print.printable, "copies": p.print.copies},
            "valid_period": p.valid_period.as_ref().map(|v| serde_json::json!({
                "start_date": v.start_date, "end_date": v.end_date,
            })),
        });
        output_buffer(
            serde_json::to_vec(&value).map_err(|e| e.to_string())?,
            output,
        )
    })
}

/// Replace the known document permission fields from UTF-8 JSON.
///
/// # Safety
/// `handle` must be live and `permissions_json` a NUL-terminated string.
#[no_mangle]
pub unsafe extern "C" fn ofd_set_document_permissions_json(
    handle: *mut OfdDocument,
    document_index: usize,
    permissions_json: *const c_char,
) -> i32 {
    run(|| {
        let input: DocumentPermissionsInput =
            serde_json::from_str(input_string(permissions_json)?).map_err(|e| e.to_string())?;
        let permissions = DocumentPermissions {
            edit: input.edit,
            annot: input.annot,
            export: input.export,
            signature: input.signature,
            watermark: input.watermark,
            print_screen: input.print_screen,
            print: PrintPermission {
                printable: input.print.printable,
                copies: input.print.copies,
            },
            valid_period: input.valid_period.map(|v| ValidPeriod {
                start_date: v.start_date,
                end_date: v.end_date,
            }),
        };
        document_mut(handle)?
            .set_document_permissions(document_index, &permissions)
            .map_err(|e| e.to_string())
    })
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct ViewPreferencesInput {
    page_mode: String,
    page_layout: String,
    tab_display: String,
    hide_toolbar: bool,
    hide_menubar: bool,
    hide_window_ui: bool,
    zoom_mode: Option<String>,
    zoom: Option<f64>,
}

/// Get effective initial viewer preferences as UTF-8 JSON.
///
/// # Safety
/// `handle` must be live and `output` must point to writable memory.
#[no_mangle]
pub unsafe extern "C" fn ofd_view_preferences_json(
    handle: *const OfdDocument,
    document_index: usize,
    output: *mut OfdBuffer,
) -> i32 {
    run(|| {
        let p = document_ref(handle)?
            .view_preferences(document_index)
            .map_err(|e| e.to_string())?;
        let value = serde_json::json!({
            "page_mode": p.page_mode, "page_layout": p.page_layout,
            "tab_display": p.tab_display,
            "hide_toolbar": p.hide_toolbar, "hide_menubar": p.hide_menubar,
            "hide_window_ui": p.hide_window_ui,
            "zoom_mode": p.zoom_mode, "zoom": p.zoom,
        });
        output_buffer(
            serde_json::to_vec(&value).map_err(|e| e.to_string())?,
            output,
        )
    })
}

/// Replace the known initial viewer preference fields from UTF-8 JSON.
///
/// # Safety
/// `handle` must be live and `preferences_json` a NUL-terminated string.
#[no_mangle]
pub unsafe extern "C" fn ofd_set_view_preferences_json(
    handle: *mut OfdDocument,
    document_index: usize,
    preferences_json: *const c_char,
) -> i32 {
    run(|| {
        let input: ViewPreferencesInput =
            serde_json::from_str(input_string(preferences_json)?).map_err(|e| e.to_string())?;
        let preferences = ViewPreferences {
            page_mode: input.page_mode,
            page_layout: input.page_layout,
            tab_display: input.tab_display,
            hide_toolbar: input.hide_toolbar,
            hide_menubar: input.hide_menubar,
            hide_window_ui: input.hide_window_ui,
            zoom_mode: input.zoom_mode,
            zoom: input.zoom,
        };
        document_mut(handle)?
            .set_view_preferences(document_index, &preferences)
            .map_err(|e| e.to_string())
    })
}

#[derive(Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
enum DocumentActionInput {
    Uri {
        event: String,
        uri: String,
    },
    Page {
        event: String,
        destination: serde_json::Value,
    },
    Bookmark {
        event: String,
        name: String,
    },
    Attachment {
        event: String,
        id: String,
        new_window: Option<bool>,
    },
}

/// Get actions declared directly on one document body as a UTF-8 JSON array.
///
/// # Safety
/// `handle` must be live and `output` must point to writable memory.
#[no_mangle]
pub unsafe extern "C" fn ofd_document_actions_json(
    handle: *const OfdDocument,
    document_index: usize,
    output: *mut OfdBuffer,
) -> i32 {
    run(|| {
        let actions = document_ref(handle)?
            .document_actions(document_index)
            .map_err(|e| e.to_string())?;
        let values: Vec<_> = actions.iter().map(page_object_action_json).collect();
        output_buffer(
            serde_json::to_vec(&values).map_err(|e| e.to_string())?,
            output,
        )
    })
}

/// Append one document action from UTF-8 JSON.
///
/// # Safety
/// `handle` must be live and `action_json` a NUL-terminated string.
#[no_mangle]
pub unsafe extern "C" fn ofd_add_document_action_json(
    handle: *mut OfdDocument,
    document_index: usize,
    action_json: *const c_char,
) -> i32 {
    run(|| {
        let input: DocumentActionInput =
            serde_json::from_str(input_string(action_json)?).map_err(|e| e.to_string())?;
        let (event, target) = match input {
            DocumentActionInput::Uri { event, uri } => (event, DocumentActionTarget::Uri(uri)),
            DocumentActionInput::Page { event, destination } => (
                event,
                DocumentActionTarget::Page(parse_destination_value(&destination)?),
            ),
            DocumentActionInput::Bookmark { event, name } => {
                (event, DocumentActionTarget::Bookmark(name))
            }
            DocumentActionInput::Attachment {
                event,
                id,
                new_window,
            } => (event, DocumentActionTarget::Attachment { id, new_window }),
        };
        document_mut(handle)?
            .add_document_action(document_index, &event, target)
            .map_err(|e| e.to_string())
    })
}

/// Remove one direct document action by zero-based index.
///
/// # Safety
/// `handle` must be live.
#[no_mangle]
pub unsafe extern "C" fn ofd_remove_document_action(
    handle: *mut OfdDocument,
    document_index: usize,
    action_index: usize,
) -> i32 {
    run(|| {
        document_mut(handle)?
            .remove_document_action(document_index, action_index)
            .map_err(|e| e.to_string())
    })
}

/// Get direct page actions as a UTF-8 JSON array.
///
/// # Safety
/// `handle` must be live and `output` must point to writable memory.
#[no_mangle]
pub unsafe extern "C" fn ofd_page_actions_json(
    handle: *const OfdDocument,
    page_index: usize,
    output: *mut OfdBuffer,
) -> i32 {
    run(|| {
        let actions = document_ref(handle)?
            .page_actions(page_index)
            .map_err(|e| e.to_string())?;
        let values: Vec<_> = actions.iter().map(page_object_action_json).collect();
        output_buffer(
            serde_json::to_vec(&values).map_err(|e| e.to_string())?,
            output,
        )
    })
}

/// Append a direct page action from UTF-8 JSON with Event `PO`.
///
/// # Safety
/// `handle` must be live and `action_json` a NUL-terminated string.
#[no_mangle]
pub unsafe extern "C" fn ofd_add_page_action_json(
    handle: *mut OfdDocument,
    page_index: usize,
    action_json: *const c_char,
) -> i32 {
    run(|| {
        let input: DocumentActionInput =
            serde_json::from_str(input_string(action_json)?).map_err(|e| e.to_string())?;
        let (event, target) = match input {
            DocumentActionInput::Uri { event, uri } => (event, DocumentActionTarget::Uri(uri)),
            DocumentActionInput::Page { event, destination } => (
                event,
                DocumentActionTarget::Page(parse_destination_value(&destination)?),
            ),
            DocumentActionInput::Bookmark { event, name } => {
                (event, DocumentActionTarget::Bookmark(name))
            }
            DocumentActionInput::Attachment {
                event,
                id,
                new_window,
            } => (event, DocumentActionTarget::Attachment { id, new_window }),
        };
        if event != "PO" {
            return Err("page action Event must be PO".into());
        }
        document_mut(handle)?
            .add_page_action(page_index, target)
            .map_err(|e| e.to_string())
    })
}

/// Remove a direct page action by zero-based index.
///
/// # Safety
/// `handle` must be live.
#[no_mangle]
pub unsafe extern "C" fn ofd_remove_page_action(
    handle: *mut OfdDocument,
    page_index: usize,
    action_index: usize,
) -> i32 {
    run(|| {
        document_mut(handle)?
            .remove_page_action(page_index, action_index)
            .map_err(|e| e.to_string())
    })
}

/// Read the optional root integrity manifest as JSON, or `null`.
///
/// # Safety
/// `handle` must be live and `output` must point to writable memory.
#[no_mangle]
pub unsafe extern "C" fn ofd_integrity_manifest_json(
    handle: *const OfdDocument,
    output: *mut OfdBuffer,
) -> i32 {
    run(|| {
        let manifest = document_ref(handle)?
            .integrity_manifest()
            .map_err(|error| error.to_string())?;
        let value = manifest.map(|manifest| {
            let files: Vec<_> = manifest
                .files
                .into_iter()
                .map(|file| serde_json::json!({"id": file.id, "path": file.path}))
                .collect();
            serde_json::json!({
                "id": manifest.id,
                "creator_name": manifest.creator_name,
                "version": manifest.version,
                "creation_date": manifest.creation_date,
                "signed_value_path": manifest.signed_value_path,
                "files": files,
            })
        });
        output_buffer(
            serde_json::to_vec(&value).map_err(|error| error.to_string())?,
            output,
        )
    })
}

/// Compare the optional integrity manifest with package file names as JSON.
/// This does not authenticate the manifest signature.
///
/// # Safety
/// `handle` must be live and `output` must point to writable memory.
#[no_mangle]
pub unsafe extern "C" fn ofd_check_integrity_manifest_json(
    handle: *const OfdDocument,
    output: *mut OfdBuffer,
) -> i32 {
    run(|| {
        let report = document_ref(handle)?
            .check_integrity_manifest()
            .map_err(|error| error.to_string())?;
        let value = report.map(|report| {
            serde_json::json!({
                "missing_files": report.missing_files,
                "extra_files": report.extra_files,
                "complete": report.complete,
            })
        });
        output_buffer(
            serde_json::to_vec(&value).map_err(|error| error.to_string())?,
            output,
        )
    })
}

/// Verify a root integrity manifest's SM2 signature and file list as JSON.
/// A null `signature_distid` uses the OFD default. Certificate trust remains
/// the caller's responsibility.
///
/// # Safety
/// `handle` must be live, `output` writable, and a nonnull `signature_distid`
/// must be a NUL-terminated UTF-8 string.
#[no_mangle]
pub unsafe extern "C" fn ofd_verify_integrity_manifest_sm2_json(
    handle: *const OfdDocument,
    signature_distid: *const c_char,
    output: *mut OfdBuffer,
) -> i32 {
    run(|| {
        let document = document_ref(handle)?;
        let report = if signature_distid.is_null() {
            document.verify_integrity_manifest_sm2()
        } else {
            document.verify_integrity_manifest_sm2_with_distid(input_string(signature_distid)?)
        }
        .map_err(|error| error.to_string())?;
        let value = report.map(|report| {
            serde_json::json!({
                "manifest_complete": report.manifest_complete,
                "signature_valid": report.signature_valid,
                "all_valid": report.all_valid,
            })
        });
        output_buffer(
            serde_json::to_vec(&value).map_err(|error| error.to_string())?,
            output,
        )
    })
}

/// Create a root SM2/SM3 integrity manifest and signed value.
/// A null `signature_distid` uses the default SM2 ID.
///
/// # Safety
/// `handle` must be live, byte pointers readable for their lengths, and a
/// nonnull `signature_distid` must be a NUL-terminated UTF-8 string.
#[no_mangle]
pub unsafe extern "C" fn ofd_protect_integrity_sm2(
    handle: *mut OfdDocument,
    private_key: *const u8,
    private_key_len: usize,
    certificate_der: *const u8,
    certificate_len: usize,
    signature_distid: *const c_char,
) -> i32 {
    run(|| {
        let key = input_bytes(private_key, private_key_len)?;
        let certificate = input_bytes(certificate_der, certificate_len)?;
        let document = document_mut(handle)?;
        if signature_distid.is_null() {
            document.protect_integrity_sm2(key, certificate)
        } else {
            document.protect_integrity_sm2_with_distid(
                key,
                certificate,
                input_string(signature_distid)?,
            )
        }
        .map_err(|error| error.to_string())
    })
}

// Shared JSON bridge for ordered and unordered certificate-chain checks.
unsafe fn verify_integrity_manifest_chain_json(
    handle: *const OfdDocument,
    options_json: *const c_char,
    output: *mut OfdBuffer,
    unordered: bool,
) -> i32 {
    run(|| {
        #[derive(Deserialize)]
        struct Options {
            signer_certificate_der_base64: String,
            #[serde(default)]
            intermediates_der_base64: Vec<String>,
            anchor_certificate_der_base64: String,
            at_unix_seconds: u64,
            #[serde(default)]
            certificate_distid: String,
            signature_distid: Option<String>,
        }
        let options: Options =
            serde_json::from_str(input_string(options_json)?).map_err(|error| error.to_string())?;
        let signer = STANDARD
            .decode(options.signer_certificate_der_base64)
            .map_err(|error| error.to_string())?;
        let intermediates = options
            .intermediates_der_base64
            .into_iter()
            .map(|item| STANDARD.decode(item).map_err(|error| error.to_string()))
            .collect::<Result<Vec<_>, _>>()?;
        let intermediate_refs = intermediates.iter().map(Vec::as_slice).collect::<Vec<_>>();
        let anchor = STANDARD
            .decode(options.anchor_certificate_der_base64)
            .map_err(|error| error.to_string())?;
        let ids = Sm2DistinguishingIds {
            certificate: &options.certificate_distid,
            signature: options
                .signature_distid
                .as_deref()
                .unwrap_or(ofdrw::DEFAULT_SM2_DISTID),
        };
        let document = document_ref(handle)?;
        let report = if unordered {
            document.verify_integrity_manifest_sm2_with_certificate_pool(
                &signer,
                &intermediate_refs,
                &anchor,
                options.at_unix_seconds,
                ids,
            )
        } else {
            document.verify_integrity_manifest_sm2_with_ordered_chain(
                &signer,
                &intermediate_refs,
                &anchor,
                options.at_unix_seconds,
                ids,
            )
        }
        .map_err(|error| error.to_string())?;
        let value = report.map(|report| {
            serde_json::json!({
                "manifest_complete": report.manifest_complete,
                "signature_valid": report.signature_valid,
                "certificate_chain_valid": report.certificate_chain_valid,
                "all_valid": report.all_valid,
            })
        });
        output_buffer(
            serde_json::to_vec(&value).map_err(|error| error.to_string())?,
            output,
        )
    })
}

/// Verify integrity with a caller-trusted ordered SM2 certificate chain.
///
/// # Safety
/// `handle` must be live, `options_json` NUL-terminated UTF-8, and `output`
/// writable.
#[no_mangle]
pub unsafe extern "C" fn ofd_verify_integrity_manifest_sm2_with_ordered_chain_json(
    handle: *const OfdDocument,
    options_json: *const c_char,
    output: *mut OfdBuffer,
) -> i32 {
    verify_integrity_manifest_chain_json(handle, options_json, output, false)
}

/// Verify integrity after building a certificate path from an unordered pool.
/// JSON fields match the ordered-chain function; intermediates are candidates.
///
/// # Safety
/// `handle` must be live, `options_json` NUL-terminated UTF-8, and `output`
/// writable.
#[no_mangle]
pub unsafe extern "C" fn ofd_verify_integrity_manifest_sm2_with_certificate_pool_json(
    handle: *const OfdDocument,
    options_json: *const c_char,
    output: *mut OfdBuffer,
) -> i32 {
    verify_integrity_manifest_chain_json(handle, options_json, output, true)
}

/// Get document versions and their file lists as a UTF-8 JSON array.
///
/// # Safety
/// `handle` must be live and `output` must point to writable memory.
#[no_mangle]
pub unsafe extern "C" fn ofd_versions_json(
    handle: *const OfdDocument,
    output: *mut OfdBuffer,
) -> i32 {
    run(|| {
        let versions = document_ref(handle)?
            .versions()
            .map_err(|e| e.to_string())?;
        let values: Vec<_> = versions.into_iter().map(version_json).collect();
        output_buffer(
            serde_json::to_vec(&values).map_err(|e| e.to_string())?,
            output,
        )
    })
}

/// Extract one version snapshot into an independent OFD handle.
///
/// # Safety
/// `handle` must be live and `output` writable. Release the returned handle
/// with `ofd_close`.
#[no_mangle]
pub unsafe extern "C" fn ofd_extract_version_snapshot(
    handle: *const OfdDocument,
    index: usize,
    output: *mut *mut OfdDocument,
) -> i32 {
    run(|| {
        let output = output.as_mut().ok_or("null extracted version output")?;
        *output = ptr::null_mut();
        let extracted = document_ref(handle)?
            .extract_version_snapshot(index)
            .map_err(|error| error.to_string())?;
        *output = register_document(extracted)?;
        Ok(())
    })
}

/// Archive current package files into a new independent document version.
///
/// `snapshot_json` requires an `id` string and accepts an optional `name`.
/// The returned buffer contains the created version as one JSON object.
///
/// # Safety
/// `handle` must be live; `snapshot_json` must be NUL-terminated; `output`
/// must point to writable memory.
#[no_mangle]
pub unsafe extern "C" fn ofd_add_version_snapshot_json(
    handle: *mut OfdDocument,
    document_index: usize,
    snapshot_json: *const c_char,
    output: *mut OfdBuffer,
) -> i32 {
    run(|| {
        if output.is_null() {
            return Err("null output buffer".into());
        }
        let value: serde_json::Value = serde_json::from_str(input_string(snapshot_json)?)
            .map_err(|error| error.to_string())?;
        let id = value
            .get("id")
            .and_then(serde_json::Value::as_str)
            .ok_or("snapshot id is required")?;
        let name = value
            .get("name")
            .filter(|value| !value.is_null())
            .map(|value| {
                value
                    .as_str()
                    .ok_or_else(|| "snapshot name must be a string".to_owned())
            })
            .transpose()?;
        let version = document_mut(handle)?
            .add_version_snapshot(document_index, id, name)
            .map_err(|error| error.to_string())?;
        output_buffer(
            serde_json::to_vec(&version_json(version)).map_err(|error| error.to_string())?,
            output,
        )
    })
}

/// Get named bookmarks as a UTF-8 JSON array.
///
/// # Safety
/// `handle` must be live and `output` must point to writable memory.
#[no_mangle]
pub unsafe extern "C" fn ofd_bookmarks_json(
    handle: *const OfdDocument,
    output: *mut OfdBuffer,
) -> i32 {
    run(|| {
        let bookmarks = document_ref(handle)?
            .bookmarks()
            .map_err(|e| e.to_string())?;
        let values: Vec<_> = bookmarks
            .into_iter()
            .map(|bookmark| {
                serde_json::json!({
                    "document_index": bookmark.document_index,
                    "name": bookmark.name,
                    "destination": destination_json(&bookmark.destination),
                })
            })
            .collect();
        output_buffer(
            serde_json::to_vec(&values).map_err(|e| e.to_string())?,
            output,
        )
    })
}

/// Add a named bookmark using a JSON object with `name`, `page_id`, `kind`,
/// and optional numeric `left`, `top`, `right`, `bottom`, and `zoom` fields.
///
/// # Safety
/// `handle` must be live and `bookmark_json` must be a NUL-terminated string.
#[no_mangle]
pub unsafe extern "C" fn ofd_add_bookmark_json(
    handle: *mut OfdDocument,
    document_index: usize,
    bookmark_json: *const c_char,
) -> i32 {
    run(|| {
        let input: serde_json::Value = serde_json::from_str(input_string(bookmark_json)?)
            .map_err(|error| error.to_string())?;
        let string = |key: &str| -> Result<String, String> {
            input
                .get(key)
                .and_then(serde_json::Value::as_str)
                .map(str::to_owned)
                .ok_or_else(|| format!("bookmark {key} must be a string"))
        };
        let number = |key: &str| -> Result<Option<f64>, String> {
            match input.get(key) {
                None | Some(serde_json::Value::Null) => Ok(None),
                Some(value) => value
                    .as_f64()
                    .map(Some)
                    .ok_or_else(|| format!("bookmark {key} must be a number")),
            }
        };
        let name = string("name")?;
        let destination = DestinationInfo {
            page_id: string("page_id")?,
            kind: string("kind")?,
            left: number("left")?,
            top: number("top")?,
            right: number("right")?,
            bottom: number("bottom")?,
            zoom: number("zoom")?,
        };
        document_mut(handle)?
            .add_bookmark(document_index, &name, destination)
            .map_err(|error| error.to_string())
    })
}

/// Remove a named bookmark from a document body.
///
/// # Safety
/// `handle` must be live and `name` must be a NUL-terminated string.
#[no_mangle]
pub unsafe extern "C" fn ofd_remove_bookmark(
    handle: *mut OfdDocument,
    document_index: usize,
    name: *const c_char,
) -> i32 {
    run(|| {
        document_mut(handle)?
            .remove_bookmark(document_index, input_string(name)?)
            .map_err(|error| error.to_string())
    })
}

/// Add an outline node using JSON fields `title`, optional `parent_index`,
/// optional `destination`, and optional boolean `expanded`.
///
/// # Safety
/// `handle` must be live, `outline_json` NUL-terminated, and `output_index` writable.
#[no_mangle]
pub unsafe extern "C" fn ofd_add_outline_json(
    handle: *mut OfdDocument,
    document_index: usize,
    outline_json: *const c_char,
    output_index: *mut usize,
) -> i32 {
    run(|| {
        let output = output_index.as_mut().ok_or("null outline index output")?;
        *output = 0;
        let input: serde_json::Value =
            serde_json::from_str(input_string(outline_json)?).map_err(|error| error.to_string())?;
        let title = input
            .get("title")
            .and_then(serde_json::Value::as_str)
            .ok_or("outline title must be a string")?;
        let parent_index = match input.get("parent_index") {
            None | Some(serde_json::Value::Null) => None,
            Some(value) => Some(
                value
                    .as_u64()
                    .and_then(|value| usize::try_from(value).ok())
                    .ok_or("outline parent_index must be a nonnegative integer")?,
            ),
        };
        let expanded = match input.get("expanded") {
            None => true,
            Some(value) => value
                .as_bool()
                .ok_or("outline expanded must be a boolean")?,
        };
        let destination = input
            .get("destination")
            .filter(|value| !value.is_null())
            .map(parse_outline_destination)
            .transpose()?;
        *output = document_mut(handle)?
            .add_outline(document_index, title, parent_index, destination, expanded)
            .map_err(|error| error.to_string())?;
        Ok(())
    })
}

fn parse_outline_destination(value: &serde_json::Value) -> Result<DestinationInfo, String> {
    let string = |key: &str| -> Result<String, String> {
        value
            .get(key)
            .and_then(serde_json::Value::as_str)
            .map(str::to_owned)
            .ok_or_else(|| format!("outline destination {key} must be a string"))
    };
    let number = |key: &str| -> Result<Option<f64>, String> {
        match value.get(key) {
            None | Some(serde_json::Value::Null) => Ok(None),
            Some(value) => value
                .as_f64()
                .map(Some)
                .ok_or_else(|| format!("outline destination {key} must be a number")),
        }
    };
    Ok(DestinationInfo {
        page_id: string("page_id")?,
        kind: string("kind")?,
        left: number("left")?,
        top: number("top")?,
        right: number("right")?,
        bottom: number("bottom")?,
        zoom: number("zoom")?,
    })
}

/// Remove an outline node and all descendants by its flattened index.
///
/// # Safety
/// `handle` must be live.
#[no_mangle]
pub unsafe extern "C" fn ofd_remove_outline(handle: *mut OfdDocument, index: usize) -> i32 {
    run(|| {
        document_mut(handle)?
            .remove_outline(index)
            .map_err(|error| error.to_string())
    })
}

/// Change an outline title by flattened index.
///
/// # Safety
/// `handle` must be live and `title` NUL-terminated.
#[no_mangle]
pub unsafe extern "C" fn ofd_set_outline_title(
    handle: *mut OfdDocument,
    index: usize,
    title: *const c_char,
) -> i32 {
    run(|| {
        document_mut(handle)?
            .set_outline_title(index, input_string(title)?)
            .map_err(|error| error.to_string())
    })
}

/// Set an outline's initial expanded state by flattened index.
///
/// # Safety
/// `handle` must be live.
#[no_mangle]
pub unsafe extern "C" fn ofd_set_outline_expanded(
    handle: *mut OfdDocument,
    index: usize,
    expanded: i32,
) -> i32 {
    run(|| {
        if !matches!(expanded, 0 | 1) {
            return Err("expanded must be 0 or 1".into());
        }
        document_mut(handle)?
            .set_outline_expanded(index, expanded == 1)
            .map_err(|error| error.to_string())
    })
}

/// Set an outline's direct GoTo destination from JSON.
///
/// # Safety
/// `handle` must be live and `destination_json` NUL-terminated.
#[no_mangle]
pub unsafe extern "C" fn ofd_set_outline_destination_json(
    handle: *mut OfdDocument,
    index: usize,
    destination_json: *const c_char,
) -> i32 {
    run(|| {
        let value: serde_json::Value = serde_json::from_str(input_string(destination_json)?)
            .map_err(|error| error.to_string())?;
        let destination = parse_outline_destination(&value)?;
        document_mut(handle)?
            .set_outline_destination(index, &destination)
            .map_err(|error| error.to_string())
    })
}

/// Point an outline's direct GoTo action at a named bookmark.
///
/// # Safety
/// `handle` must be live and `name` NUL-terminated.
#[no_mangle]
pub unsafe extern "C" fn ofd_set_outline_bookmark(
    handle: *mut OfdDocument,
    index: usize,
    name: *const c_char,
) -> i32 {
    run(|| {
        document_mut(handle)?
            .set_outline_bookmark(index, input_string(name)?)
            .map_err(|error| error.to_string())
    })
}

/// Get outline nodes in depth-first order as a UTF-8 JSON array.
///
/// # Safety
/// `handle` must be live and `output` must point to writable memory.
#[no_mangle]
pub unsafe extern "C" fn ofd_outlines_json(
    handle: *const OfdDocument,
    output: *mut OfdBuffer,
) -> i32 {
    run(|| {
        let outlines = document_ref(handle)?
            .outlines()
            .map_err(|e| e.to_string())?;
        let values: Vec<_> = outlines
            .into_iter()
            .map(|item| {
                serde_json::json!({
                    "document_index": item.document_index,
                    "title": item.title,
                    "depth": item.depth,
                    "parent_index": item.parent_index,
                    "expanded": item.expanded,
                    "count": item.count,
                    "destination": item.destination.as_ref().map(destination_json),
                    "bookmark_name": item.bookmark_name,
                })
            })
            .collect();
        output_buffer(
            serde_json::to_vec(&values).map_err(|e| e.to_string())?,
            output,
        )
    })
}

/// Update one standard plain-text DocInfo field.
///
/// # Safety
/// `handle` must be live; `field` and `value` must be NUL-terminated strings.
#[no_mangle]
pub unsafe extern "C" fn ofd_set_document_info_field(
    handle: *mut OfdDocument,
    document_index: usize,
    field: *const c_char,
    value: *const c_char,
) -> i32 {
    run(|| {
        document_mut(handle)?
            .set_document_info_field(document_index, input_string(field)?, input_string(value)?)
            .map_err(|e| e.to_string())
    })
}

/// Remove one optional standard DocInfo field.
///
/// # Safety
/// `handle` must be live and `field` NUL-terminated.
#[no_mangle]
pub unsafe extern "C" fn ofd_remove_document_info_field(
    handle: *mut OfdDocument,
    document_index: usize,
    field: *const c_char,
) -> i32 {
    run(|| {
        document_mut(handle)?
            .remove_document_info_field(document_index, input_string(field)?)
            .map_err(|error| error.to_string())
    })
}

/// Append one keyword to a document's DocInfo.
///
/// # Safety
/// `handle` must be live and `keyword` NUL-terminated.
#[no_mangle]
pub unsafe extern "C" fn ofd_add_document_keyword(
    handle: *mut OfdDocument,
    document_index: usize,
    keyword: *const c_char,
) -> i32 {
    run(|| {
        document_mut(handle)?
            .add_document_keyword(document_index, input_string(keyword)?)
            .map_err(|error| error.to_string())
    })
}

/// Remove one keyword by its position in DocInfo.
///
/// # Safety
/// `handle` must be live.
#[no_mangle]
pub unsafe extern "C" fn ofd_remove_document_keyword(
    handle: *mut OfdDocument,
    document_index: usize,
    keyword_index: usize,
) -> i32 {
    run(|| {
        document_mut(handle)?
            .remove_document_keyword(document_index, keyword_index)
            .map_err(|error| error.to_string())
    })
}

/// Set one named custom DocInfo value.
///
/// # Safety
/// `handle` must be live; `name` and `value` NUL-terminated.
#[no_mangle]
pub unsafe extern "C" fn ofd_set_document_custom_data(
    handle: *mut OfdDocument,
    document_index: usize,
    name: *const c_char,
    value: *const c_char,
) -> i32 {
    run(|| {
        document_mut(handle)?
            .set_document_custom_data(document_index, input_string(name)?, input_string(value)?)
            .map_err(|error| error.to_string())
    })
}

/// Remove one named custom DocInfo value.
///
/// # Safety
/// `handle` must be live and `name` NUL-terminated.
#[no_mangle]
pub unsafe extern "C" fn ofd_remove_document_custom_data(
    handle: *mut OfdDocument,
    document_index: usize,
    name: *const c_char,
) -> i32 {
    run(|| {
        document_mut(handle)?
            .remove_document_custom_data(document_index, input_string(name)?)
            .map_err(|error| error.to_string())
    })
}

/// Get a page's TextCode values as a UTF-8 JSON array. Indexes are zero-based.
///
/// # Safety
/// `handle` must be live and `output` must point to writable memory.
#[no_mangle]
pub unsafe extern "C" fn ofd_page_text_json(
    handle: *const OfdDocument,
    index: usize,
    output: *mut OfdBuffer,
) -> i32 {
    run(|| {
        let text = document_ref(handle)?
            .page_text(index)
            .map_err(|e| e.to_string())?;
        let bytes = serde_json::to_vec(&text).map_err(|e| e.to_string())?;
        output_buffer(bytes, output)
    })
}

/// Get TextCode strings from every page in package order as nested JSON arrays.
///
/// # Safety
/// `handle` must be live and `output` must point to writable memory.
#[no_mangle]
pub unsafe extern "C" fn ofd_all_text_json(
    handle: *const OfdDocument,
    output: *mut OfdBuffer,
) -> i32 {
    run(|| {
        let text = document_ref(handle)?
            .all_text()
            .map_err(|error| error.to_string())?;
        output_buffer(
            serde_json::to_vec(&text).map_err(|error| error.to_string())?,
            output,
        )
    })
}

/// Extract text whose positioned origins fall inside a page rectangle in millimeters.
///
/// # Safety
/// `handle` must be live and `output` must point to writable memory.
#[no_mangle]
pub unsafe extern "C" fn ofd_page_text_in_region_json(
    handle: *const OfdDocument,
    index: usize,
    x: f64,
    y: f64,
    width: f64,
    height: f64,
    output: *mut OfdBuffer,
) -> i32 {
    run(|| {
        let text = document_ref(handle)?
            .page_text_in_region(index, [x, y, width, height])
            .map_err(|error| error.to_string())?;
        output_buffer(
            serde_json::to_vec(&text).map_err(|error| error.to_string())?,
            output,
        )
    })
}

/// Find positioned keywords. Input is `{"keywords":["text"],"pages":[0]}`;
/// `pages` may be omitted to search every page. Page indexes are zero-based.
///
/// # Safety
/// `handle` must be live; `query_json` must be NUL-terminated; `output`
/// must point to writable memory.
#[no_mangle]
pub unsafe extern "C" fn ofd_keyword_positions_json(
    handle: *const OfdDocument,
    query_json: *const c_char,
    output: *mut OfdBuffer,
) -> i32 {
    run(|| {
        let query: serde_json::Value =
            serde_json::from_str(input_string(query_json)?).map_err(|error| error.to_string())?;
        let keywords: Vec<_> = query
            .get("keywords")
            .and_then(serde_json::Value::as_array)
            .ok_or("keywords must be an array")?
            .iter()
            .map(|value| value.as_str().ok_or("keyword must be a string"))
            .collect::<Result<_, _>>()?;
        let pages: Option<Vec<usize>> = match query.get("pages") {
            None | Some(serde_json::Value::Null) => None,
            Some(value) => Some(
                value
                    .as_array()
                    .ok_or("pages must be an array")?
                    .iter()
                    .map(|value| {
                        value
                            .as_u64()
                            .and_then(|index| usize::try_from(index).ok())
                            .ok_or("page index must be a nonnegative integer")
                    })
                    .collect::<Result<_, _>>()?,
            ),
        };
        let matches = document_ref(handle)?
            .keyword_positions(&keywords, pages.as_deref())
            .map_err(|error| error.to_string())?;
        let values: Vec<_> = matches
            .into_iter()
            .map(|position| {
                serde_json::json!({
                    "page_index": position.page_index,
                    "keyword": position.keyword,
                    "box_mm": position.box_mm,
                })
            })
            .collect();
        output_buffer(
            serde_json::to_vec(&values).map_err(|error| error.to_string())?,
            output,
        )
    })
}

/// Get page and template source paths as a UTF-8 JSON array.
///
/// # Safety
/// `handle` must be live and `output` must point to writable memory.
#[no_mangle]
pub unsafe extern "C" fn ofd_page_sources_json(
    handle: *const OfdDocument,
    index: usize,
    output: *mut OfdBuffer,
) -> i32 {
    run(|| {
        let sources = document_ref(handle)?
            .page_sources(index)
            .map_err(|e| e.to_string())?;
        let values: Vec<_> = sources
            .into_iter()
            .map(|source| {
                serde_json::json!({
                    "path": source.path,
                    "template_id": source.template_id,
                    "z_order": source.z_order,
                })
            })
            .collect();
        output_buffer(
            serde_json::to_vec(&values).map_err(|e| e.to_string())?,
            output,
        )
    })
}

/// Get page and template layers in painting order as a UTF-8 JSON array.
///
/// # Safety
/// `handle` must be live and `output` must point to writable memory.
#[no_mangle]
pub unsafe extern "C" fn ofd_page_layers_json(
    handle: *const OfdDocument,
    index: usize,
    output: *mut OfdBuffer,
) -> i32 {
    run(|| {
        let layers = document_ref(handle)?
            .page_layers(index)
            .map_err(|e| e.to_string())?;
        let values: Vec<_> = layers
            .into_iter()
            .map(|layer| {
                serde_json::json!({
                    "id": layer.id,
                    "name": layer.name,
                    "layer_type": layer.layer_type,
                    "draw_param": layer.draw_param,
                    "source_path": layer.source_path,
                    "template_id": layer.template_id,
                    "source_layer_index": layer.source_layer_index,
                    "object_count": layer.object_count,
                })
            })
            .collect();
        output_buffer(
            serde_json::to_vec(&values).map_err(|e| e.to_string())?,
            output,
        )
    })
}

/// Move source page content into a reusable template and attach it to pages.
/// `additional_pages_json` is a JSON array of zero-based page indexes.
///
/// # Safety
/// `handle` must be live, strings NUL-terminated, and `output_id` writable.
#[no_mangle]
pub unsafe extern "C" fn ofd_promote_page_to_template(
    handle: *mut OfdDocument,
    source_page_index: usize,
    additional_pages_json: *const c_char,
    z_order: *const c_char,
    output_id: *mut usize,
) -> i32 {
    run(|| {
        let output = output_id
            .as_mut()
            .ok_or("null template identifier output")?;
        let additional: Vec<usize> = serde_json::from_str(input_string(additional_pages_json)?)
            .map_err(|error| error.to_string())?;
        let id = document_mut(handle)?
            .promote_page_to_template(source_page_index, &additional, input_string(z_order)?)
            .map_err(|error| error.to_string())?;
        *output = id.parse().map_err(|_| "invalid template identifier")?;
        Ok(())
    })
}

/// Attach a defined template to a page in its document body.
///
/// # Safety
/// `handle` must be live and strings NUL-terminated.
#[no_mangle]
pub unsafe extern "C" fn ofd_apply_page_template(
    handle: *mut OfdDocument,
    page_index: usize,
    template_id: *const c_char,
    z_order: *const c_char,
) -> i32 {
    run(|| {
        document_mut(handle)?
            .apply_page_template(
                page_index,
                input_string(template_id)?,
                input_string(z_order)?,
            )
            .map_err(|error| error.to_string())
    })
}

/// Remove a template reference from one page.
///
/// # Safety
/// `handle` must be live and `template_id` NUL-terminated.
#[no_mangle]
pub unsafe extern "C" fn ofd_remove_page_template(
    handle: *mut OfdDocument,
    page_index: usize,
    template_id: *const c_char,
) -> i32 {
    run(|| {
        document_mut(handle)?
            .remove_page_template(page_index, input_string(template_id)?)
            .map_err(|error| error.to_string())
    })
}

/// Remove a template definition once no page in its document uses it.
///
/// # Safety
/// `handle` must be live and `template_id` NUL-terminated.
#[no_mangle]
pub unsafe extern "C" fn ofd_remove_document_template(
    handle: *mut OfdDocument,
    document_index: usize,
    template_id: *const c_char,
) -> i32 {
    run(|| {
        document_mut(handle)?
            .remove_document_template(document_index, input_string(template_id)?)
            .map_err(|error| error.to_string())
    })
}

/// Get direct page and template graphic objects in painting order as JSON.
///
/// # Safety
/// `handle` must be live and `output` must point to writable memory.
#[no_mangle]
pub unsafe extern "C" fn ofd_page_objects_json(
    handle: *const OfdDocument,
    index: usize,
    output: *mut OfdBuffer,
) -> i32 {
    run(|| {
        let objects = document_ref(handle)?
            .page_objects(index)
            .map_err(|error| error.to_string())?;
        let values: Vec<_> = objects
            .into_iter()
            .map(|object| {
                serde_json::json!({
                    "id": object.id,
                    "name": object.name,
                    "blend_mode": object.blend_mode,
                    "kind": object.kind,
                    "layer_id": object.layer_id,
                    "layer_type": object.layer_type,
                    "source_layer_index": object.source_layer_index,
                    "source_path": object.source_path,
                    "template_id": object.template_id,
                    "boundary": object.boundary,
                    "ctm": object.ctm,
                    "visible": object.visible,
                    "alpha": object.alpha,
                    "draw_param": object.draw_param,
                    "resource_id": object.resource_id,
                })
            })
            .collect();
        output_buffer(
            serde_json::to_vec(&values).map_err(|error| error.to_string())?,
            output,
        )
    })
}

/// Get page and template graphic object clips in painting order as JSON.
/// Each area includes its original XML.
///
/// # Safety
/// `handle` must be live and `output` must point to writable memory.
#[no_mangle]
pub unsafe extern "C" fn ofd_page_object_clips_json(
    handle: *const OfdDocument,
    index: usize,
    output: *mut OfdBuffer,
) -> i32 {
    run(|| {
        let objects = document_ref(handle)?
            .page_object_clips(index)
            .map_err(|error| error.to_string())?;
        let values: Vec<_> = objects
            .into_iter()
            .map(|object| {
                let clips: Vec<_> = object
                    .clips
                    .into_iter()
                    .map(|clip| {
                        clip.into_iter()
                            .map(|area| {
                                serde_json::json!({
                                    "kind": area.kind,
                                    "fill": area.fill,
                                    "stroke": area.stroke,
                                    "boundary": area.boundary,
                                    "area_ctm": area.area_ctm,
                                    "shape_ctm": area.shape_ctm,
                                    "draw_param": area.draw_param,
                                    "rule": area.rule,
                                    "commands": area.commands,
                                    "font_id": area.font_id,
                                    "size": area.size,
                                    "text": area.text,
                        "h_scale": area.h_scale,
                        "char_direction": area.char_direction,
                        "text_codes": area.text_codes.into_iter().map(|code| serde_json::json!({
                            "x": code.x,
                            "y": code.y,
                            "delta_x": code.delta_x,
                            "delta_y": code.delta_y,
                            "text": code.text,
                        })).collect::<Vec<_>>(),
                        "cg_transforms": area.cg_transforms.into_iter().map(|mapping| serde_json::json!({
                            "code_position": mapping.code_position,
                            "code_count": mapping.code_count,
                            "glyph_count": mapping.glyph_count,
                            "glyphs": mapping.glyphs,
                        })).collect::<Vec<_>>(),
                                    "xml": area.xml,
                                })
                            })
                            .collect::<Vec<_>>()
                    })
                    .collect();
                serde_json::json!({
                    "id": object.id,
                    "kind": object.kind,
                    "source_path": object.source_path,
                    "template_id": object.template_id,
                    "follows_ctm": object.follows_ctm,
                    "clips": clips,
                })
            })
            .collect();
        output_buffer(
            serde_json::to_vec(&values).map_err(|error| error.to_string())?,
            output,
        )
    })
}

/// Get direct path/text paint declarations in page painting order as JSON.
///
/// # Safety
/// `handle` must be live and `output` must point to writable memory.
#[no_mangle]
pub unsafe extern "C" fn ofd_page_object_paints_json(
    handle: *const OfdDocument,
    index: usize,
    output: *mut OfdBuffer,
) -> i32 {
    run(|| {
        let paints = document_ref(handle)?
            .page_object_paints(index)
            .map_err(|error| error.to_string())?;
        let values: Vec<_> = paints
            .into_iter()
            .map(|item| {
                let paint = |value: Option<ofdrw::PaintInfo>| {
                    value.map(|paint| {
                        serde_json::json!({
                            "kind": paint.kind,
                            "alpha": paint.alpha,
                            "value": paint.value,
                            "color_space": paint.color_space,
                            "color_index": paint.color_index,
                            "xml": paint.xml,
                        })
                    })
                };
                serde_json::json!({
                    "id": item.id,
                    "kind": item.kind,
                    "source_path": item.source_path,
                    "template_id": item.template_id,
                    "fill_enabled": item.fill_enabled,
                    "stroke_enabled": item.stroke_enabled,
                    "fill": paint(item.fill),
                    "stroke": paint(item.stroke),
                    "effective_fill": paint(item.effective_fill),
                    "effective_stroke": paint(item.effective_stroke),
                    "effective_fill_draw_param": item.effective_fill_draw_param,
                    "effective_stroke_draw_param": item.effective_stroke_draw_param,
                })
            })
            .collect();
        output_buffer(
            serde_json::to_vec(&values).map_err(|error| error.to_string())?,
            output,
        )
    })
}

/// Get resolved path/text stroke geometry in page painting order as JSON.
///
/// # Safety
/// `handle` must be live and `output` must point to writable memory.
#[no_mangle]
pub unsafe extern "C" fn ofd_page_object_stroke_styles_json(
    handle: *const OfdDocument,
    index: usize,
    output: *mut OfdBuffer,
) -> i32 {
    run(|| {
        let styles = document_ref(handle)?
            .page_object_stroke_styles(index)
            .map_err(|error| error.to_string())?;
        let values: Vec<_> = styles
            .into_iter()
            .map(|item| {
                serde_json::json!({
                    "id": item.id,
                    "kind": item.kind,
                    "source_path": item.source_path,
                    "template_id": item.template_id,
                    "stroke_enabled": item.stroke_enabled,
                    "style": {
                        "line_width": item.style.line_width,
                        "cap": item.style.cap,
                        "join": item.style.join,
                        "miter_limit": item.style.miter_limit,
                        "dash_pattern": item.style.dash_pattern,
                        "dash_sequence": item.style.dash_sequence,
                        "dash_offset": item.style.dash_offset,
                    },
                })
            })
            .collect();
        output_buffer(
            serde_json::to_vec(&values).map_err(|error| error.to_string())?,
            output,
        )
    })
}

/// Get actions attached to a direct page graphic object as JSON.
///
/// # Safety
/// `handle` must be live, `object_id` NUL-terminated, and `output` writable.
#[no_mangle]
pub unsafe extern "C" fn ofd_page_object_actions_json(
    handle: *const OfdDocument,
    page_index: usize,
    object_id: *const c_char,
    output: *mut OfdBuffer,
) -> i32 {
    run(|| {
        let actions = document_ref(handle)?
            .page_object_actions(page_index, input_string(object_id)?)
            .map_err(|error| error.to_string())?;
        let values: Vec<_> = actions.iter().map(page_object_action_json).collect();
        output_buffer(
            serde_json::to_vec(&values).map_err(|error| error.to_string())?,
            output,
        )
    })
}

/// Get actions attached to a page or template source object as JSON.
///
/// # Safety
/// `handle` must be live, strings NUL-terminated, and `output` writable.
#[no_mangle]
pub unsafe extern "C" fn ofd_page_source_object_actions_json(
    handle: *const OfdDocument,
    page_index: usize,
    source_path: *const c_char,
    object_id: *const c_char,
    output: *mut OfdBuffer,
) -> i32 {
    run(|| {
        let actions = document_ref(handle)?
            .page_source_object_actions(
                page_index,
                input_string(source_path)?,
                input_string(object_id)?,
            )
            .map_err(|error| error.to_string())?;
        let values: Vec<_> = actions.iter().map(page_object_action_json).collect();
        output_buffer(
            serde_json::to_vec(&values).map_err(|error| error.to_string())?,
            output,
        )
    })
}

/// Append a click URI action to a direct page graphic object.
///
/// # Safety
/// `handle` must be live and both strings NUL-terminated.
#[no_mangle]
pub unsafe extern "C" fn ofd_add_page_object_uri_action(
    handle: *mut OfdDocument,
    page_index: usize,
    object_id: *const c_char,
    uri: *const c_char,
) -> i32 {
    run(|| {
        document_mut(handle)?
            .add_page_object_uri_action(page_index, input_string(object_id)?, input_string(uri)?)
            .map_err(|error| error.to_string())
    })
}

/// Append a click Goto action using a JSON destination object.
///
/// # Safety
/// `handle` must be live and both strings NUL-terminated.
#[no_mangle]
pub unsafe extern "C" fn ofd_add_page_object_goto_action_json(
    handle: *mut OfdDocument,
    page_index: usize,
    object_id: *const c_char,
    destination_json: *const c_char,
) -> i32 {
    run(|| {
        let input: serde_json::Value = serde_json::from_str(input_string(destination_json)?)
            .map_err(|error| error.to_string())?;
        let destination = parse_destination_value(&input)?;
        document_mut(handle)?
            .add_page_object_goto_action(page_index, input_string(object_id)?, destination)
            .map_err(|error| error.to_string())
    })
}

/// Append a click Goto action targeting a named bookmark.
///
/// # Safety
/// `handle` must be live and both strings NUL-terminated.
#[no_mangle]
pub unsafe extern "C" fn ofd_add_page_object_bookmark_action(
    handle: *mut OfdDocument,
    page_index: usize,
    object_id: *const c_char,
    bookmark_name: *const c_char,
) -> i32 {
    run(|| {
        document_mut(handle)?
            .add_page_object_bookmark_action(
                page_index,
                input_string(object_id)?,
                input_string(bookmark_name)?,
            )
            .map_err(|error| error.to_string())
    })
}

/// Append a click GotoA action targeting an attachment in the same document.
/// `new_window` is -1 to omit the attribute, 0 for false, or 1 for true.
///
/// # Safety
/// `handle` must be live and both strings NUL-terminated.
#[no_mangle]
pub unsafe extern "C" fn ofd_add_page_object_attachment_action(
    handle: *mut OfdDocument,
    page_index: usize,
    object_id: *const c_char,
    attachment_id: *const c_char,
    new_window: i32,
) -> i32 {
    run(|| {
        let new_window = match new_window {
            -1 => None,
            0 => Some(false),
            1 => Some(true),
            _ => return Err("new_window must be -1, 0, or 1".into()),
        };
        document_mut(handle)?
            .add_page_object_attachment_action(
                page_index,
                input_string(object_id)?,
                input_string(attachment_id)?,
                new_window,
            )
            .map_err(|error| error.to_string())
    })
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct MediaActionInput {
    kind: String,
    resource_id: String,
    #[serde(default)]
    volume: Option<u8>,
    #[serde(default)]
    repeat: Option<bool>,
    #[serde(default)]
    synchronous: Option<bool>,
    #[serde(default)]
    operator: Option<String>,
    #[serde(default)]
    boundary: Option<[f64; 4]>,
}

impl MediaActionInput {
    fn sound_options(&self) -> Result<SoundActionOptions, String> {
        if self.operator.is_some() {
            return Err("Sound action cannot have Operator".into());
        }
        Ok(SoundActionOptions {
            volume: self.volume,
            repeat: self.repeat,
            synchronous: self.synchronous,
        })
    }

    fn movie_operator(&self) -> Result<Option<&str>, String> {
        if self.volume.is_some() || self.repeat.is_some() || self.synchronous.is_some() {
            return Err("Movie action cannot have Sound attributes".into());
        }
        Ok(self.operator.as_deref())
    }
}

/// Append a Sound or Movie click action to a direct page object.
/// JSON requires `kind` and `resource_id`, with the kind-specific optional
/// `volume`, `repeat`, `synchronous`, or `operator` fields.
///
/// # Safety
/// `handle` must be live and both strings NUL-terminated.
#[no_mangle]
pub unsafe extern "C" fn ofd_add_page_object_media_action_json(
    handle: *mut OfdDocument,
    page_index: usize,
    object_id: *const c_char,
    action_json: *const c_char,
) -> i32 {
    run(|| {
        let input: MediaActionInput =
            serde_json::from_str(input_string(action_json)?).map_err(|error| error.to_string())?;
        if input.boundary.is_some() {
            return Err("page object action cannot have boundary".into());
        }
        let document = document_mut(handle)?;
        let object_id = input_string(object_id)?;
        match input.kind.as_str() {
            "Sound" => document.add_page_object_sound_action(
                page_index,
                object_id,
                &input.resource_id,
                input.sound_options()?,
            ),
            "Movie" => document.add_page_object_movie_action(
                page_index,
                object_id,
                &input.resource_id,
                input.movie_operator()?,
            ),
            _ => return Err("media action kind must be Sound or Movie".into()),
        }
        .map_err(|error| error.to_string())
    })
}

/// Remove one action from a direct page graphic object.
///
/// # Safety
/// `handle` must be live and `object_id` NUL-terminated.
#[no_mangle]
pub unsafe extern "C" fn ofd_remove_page_object_action(
    handle: *mut OfdDocument,
    page_index: usize,
    object_id: *const c_char,
    action_index: usize,
) -> i32 {
    run(|| {
        document_mut(handle)?
            .remove_page_object_action(page_index, input_string(object_id)?, action_index)
            .map_err(|error| error.to_string())
    })
}

/// Set rectangular trigger areas on a direct page object action from a JSON array.
/// An empty array removes the explicit Region.
///
/// # Safety
/// `handle` must be live and both strings NUL-terminated.
#[no_mangle]
pub unsafe extern "C" fn ofd_set_page_object_action_regions_json(
    handle: *mut OfdDocument,
    page_index: usize,
    object_id: *const c_char,
    action_index: usize,
    rectangles_json: *const c_char,
) -> i32 {
    run(|| {
        let rectangles: Vec<[f64; 4]> = serde_json::from_str(input_string(rectangles_json)?)
            .map_err(|error| error.to_string())?;
        document_mut(handle)?
            .set_page_object_action_regions(
                page_index,
                input_string(object_id)?,
                action_index,
                &rectangles,
            )
            .map_err(|error| error.to_string())
    })
}

/// Get TextCode strings from templates and the page itself as UTF-8 JSON.
///
/// # Safety
/// `handle` must be live and `output` must point to writable memory.
#[no_mangle]
pub unsafe extern "C" fn ofd_page_text_with_templates_json(
    handle: *const OfdDocument,
    index: usize,
    output: *mut OfdBuffer,
) -> i32 {
    run(|| {
        let text = document_ref(handle)?
            .page_text_with_templates(index)
            .map_err(|e| e.to_string())?;
        output_buffer(
            serde_json::to_vec(&text).map_err(|e| e.to_string())?,
            output,
        )
    })
}

/// Get positioned page text objects as a UTF-8 JSON array.
///
/// # Safety
/// `handle` must be live and `output` must point to writable memory.
#[no_mangle]
pub unsafe extern "C" fn ofd_page_text_objects_json(
    handle: *const OfdDocument,
    index: usize,
    output: *mut OfdBuffer,
) -> i32 {
    run(|| {
        let objects = document_ref(handle)?
            .page_text_objects(index)
            .map_err(|e| e.to_string())?;
        let values: Vec<_> = objects
            .into_iter()
            .map(|object| {
                let codes: Vec<_> = object
                    .codes
                    .into_iter()
                    .map(|code| {
                        serde_json::json!({
                            "x": code.x, "y": code.y, "delta_x": code.delta_x,
                            "delta_y": code.delta_y, "text": code.text,
                        })
                    })
                    .collect();
                let cg_transforms: Vec<_> = object
                    .cg_transforms
                    .into_iter()
                    .map(|transform| {
                        serde_json::json!({
                            "code_position": transform.code_position,
                            "code_count": transform.code_count,
                            "glyph_count": transform.glyph_count,
                            "glyphs": transform.glyphs,
                        })
                    })
                    .collect();
                serde_json::json!({
                    "id": object.id, "boundary": object.boundary,
                    "font_id": object.font_id, "size": object.size,
                    "h_scale": object.h_scale,
                    "read_direction": object.read_direction,
                    "char_direction": object.char_direction,
                    "weight": object.weight,
                    "italic": object.italic,
                    "codes": codes, "cg_transforms": cg_transforms,
                })
            })
            .collect();
        output_buffer(
            serde_json::to_vec(&values).map_err(|e| e.to_string())?,
            output,
        )
    })
}

/// Get path objects from one page as a UTF-8 JSON array.
///
/// # Safety
/// `handle` must be live and `output` must point to writable memory.
#[no_mangle]
pub unsafe extern "C" fn ofd_page_paths_json(
    handle: *const OfdDocument,
    index: usize,
    output: *mut OfdBuffer,
) -> i32 {
    run(|| {
        let paths = document_ref(handle)?
            .page_paths(index)
            .map_err(|e| e.to_string())?;
        let values: Vec<_> = paths
            .into_iter()
            .map(|path| {
                serde_json::json!({
                    "id": path.id, "boundary": path.boundary, "ctm": path.ctm,
                    "stroke": path.stroke, "fill": path.fill,
                    "abbreviated_data": path.abbreviated_data,
                })
            })
            .collect();
        output_buffer(
            serde_json::to_vec(&values).map_err(|e| e.to_string())?,
            output,
        )
    })
}

/// Get image objects and resolved resources from one page as a UTF-8 JSON array.
///
/// # Safety
/// `handle` must be live and `output` must point to writable memory.
#[no_mangle]
pub unsafe extern "C" fn ofd_page_images_json(
    handle: *const OfdDocument,
    index: usize,
    output: *mut OfdBuffer,
) -> i32 {
    run(|| {
        let images = document_ref(handle)?
            .page_images(index)
            .map_err(|e| e.to_string())?;
        let values: Vec<_> = images
            .into_iter()
            .map(|image| {
                serde_json::json!({
                    "id": image.id, "boundary": image.boundary, "ctm": image.ctm,
                    "resource_id": image.resource_id, "substitution": image.substitution,
                    "image_mask": image.image_mask,
                    "border": image.border.as_ref().map(|border| serde_json::json!({
                        "line_width": border.line_width,
                        "horizontal_corner_radius": border.horizontal_corner_radius,
                        "vertical_corner_radius": border.vertical_corner_radius,
                        "dash_pattern": border.dash_pattern,
                        "dash_offset": border.dash_offset,
                        "color_value": border.color_value,
                        "color_space": border.color_space,
                        "color_index": border.color_index,
                        "color_alpha": border.color_alpha,
                        "color_xml": border.color_xml,
                    })),
                    "media_path": image.media_path,
                    "format": image.format,
                })
            })
            .collect();
        output_buffer(
            serde_json::to_vec(&values).map_err(|e| e.to_string())?,
            output,
        )
    })
}

/// List multimedia resources visible to a page as UTF-8 JSON.
///
/// # Safety
/// `handle` must be live and `output` must point to writable memory.
#[no_mangle]
pub unsafe extern "C" fn ofd_page_multimedia_resources_json(
    handle: *const OfdDocument,
    index: usize,
    output: *mut OfdBuffer,
) -> i32 {
    run(|| {
        let resources = document_ref(handle)?
            .page_multimedia_resources(index)
            .map_err(|e| e.to_string())?;
        let values: Vec<_> = resources
            .into_iter()
            .map(|resource| {
                serde_json::json!({
                    "id": resource.id,
                    "media_type": resource.media_type,
                    "format": resource.format,
                    "path": resource.path,
                    "size": resource.size,
                })
            })
            .collect();
        output_buffer(
            serde_json::to_vec(&values).map_err(|e| e.to_string())?,
            output,
        )
    })
}

/// Return raw bytes of a multimedia resource visible to a page.
///
/// # Safety
/// `handle` must be live, `resource_id` NUL-terminated and `output` writable.
#[no_mangle]
pub unsafe extern "C" fn ofd_page_multimedia_data(
    handle: *const OfdDocument,
    page_index: usize,
    resource_id: *const c_char,
    output: *mut OfdBuffer,
) -> i32 {
    run(|| {
        let data = document_ref(handle)?
            .page_multimedia_data(page_index, input_string(resource_id)?)
            .map_err(|e| e.to_string())?;
        output_buffer(data.to_vec(), output)
    })
}

/// List document-level multimedia resources as UTF-8 JSON.
///
/// # Safety
/// `handle` must be live and `output` must point to writable memory.
#[no_mangle]
pub unsafe extern "C" fn ofd_document_multimedia_resources_json(
    handle: *const OfdDocument,
    index: usize,
    output: *mut OfdBuffer,
) -> i32 {
    run(|| {
        let resources = document_ref(handle)?
            .document_multimedia_resources(index)
            .map_err(|e| e.to_string())?;
        let values: Vec<_> = resources
            .into_iter()
            .map(|resource| {
                serde_json::json!({
                    "id": resource.id,
                    "media_type": resource.media_type,
                    "format": resource.format,
                    "path": resource.path,
                    "size": resource.size,
                })
            })
            .collect();
        output_buffer(
            serde_json::to_vec(&values).map_err(|e| e.to_string())?,
            output,
        )
    })
}

/// Return raw bytes of a document-level multimedia resource.
///
/// # Safety
/// `handle` must be live, `resource_id` NUL-terminated and `output` writable.
#[no_mangle]
pub unsafe extern "C" fn ofd_document_multimedia_data(
    handle: *const OfdDocument,
    document_index: usize,
    resource_id: *const c_char,
    output: *mut OfdBuffer,
) -> i32 {
    run(|| {
        let data = document_ref(handle)?
            .document_multimedia_data(document_index, input_string(resource_id)?)
            .map_err(|e| e.to_string())?;
        output_buffer(data.to_vec(), output)
    })
}

/// Get images inside composite graphic objects from one page as JSON.
///
/// # Safety
/// `handle` must be live and `output` must point to writable memory.
#[no_mangle]
pub unsafe extern "C" fn ofd_page_composite_images_json(
    handle: *const OfdDocument,
    index: usize,
    output: *mut OfdBuffer,
) -> i32 {
    run(|| {
        let images = document_ref(handle)?
            .page_composite_images(index)
            .map_err(|e| e.to_string())?;
        let values: Vec<_> = images
            .into_iter()
            .map(|image| {
                serde_json::json!({
                    "id": image.id, "boundary": image.boundary, "ctm": image.ctm,
                    "resource_id": image.resource_id, "substitution": image.substitution,
                    "image_mask": image.image_mask,
                    "border": image.border.as_ref().map(|border| serde_json::json!({
                        "line_width": border.line_width,
                        "horizontal_corner_radius": border.horizontal_corner_radius,
                        "vertical_corner_radius": border.vertical_corner_radius,
                        "dash_pattern": border.dash_pattern,
                        "dash_offset": border.dash_offset,
                        "color_value": border.color_value,
                        "color_space": border.color_space,
                        "color_index": border.color_index,
                        "color_alpha": border.color_alpha,
                        "color_xml": border.color_xml,
                    })),
                    "media_path": image.media_path,
                    "format": image.format,
                })
            })
            .collect();
        output_buffer(
            serde_json::to_vec(&values).map_err(|e| e.to_string())?,
            output,
        )
    })
}

/// Get composite graphic objects from one page as a UTF-8 JSON array.
///
/// # Safety
/// `handle` must be live and `output` must point to writable memory.
#[no_mangle]
pub unsafe extern "C" fn ofd_page_composite_objects_json(
    handle: *const OfdDocument,
    index: usize,
    output: *mut OfdBuffer,
) -> i32 {
    run(|| {
        let objects = document_ref(handle)?
            .page_composite_objects(index)
            .map_err(|e| e.to_string())?;
        let values: Vec<_> = objects
            .into_iter()
            .map(|object| {
                serde_json::json!({
                    "id": object.id, "boundary": object.boundary, "ctm": object.ctm,
                    "resource_id": object.resource_id, "alpha": object.alpha,
                    "draw_param": object.draw_param,
                })
            })
            .collect();
        output_buffer(
            serde_json::to_vec(&values).map_err(|e| e.to_string())?,
            output,
        )
    })
}

/// Get composite graphic unit metadata available to one page as a UTF-8 JSON array.
///
/// # Safety
/// `handle` must be live and `output` must point to writable memory.
#[no_mangle]
pub unsafe extern "C" fn ofd_page_composite_units_json(
    handle: *const OfdDocument,
    index: usize,
    output: *mut OfdBuffer,
) -> i32 {
    run(|| {
        let units = document_ref(handle)?
            .page_composite_units(index)
            .map_err(|e| e.to_string())?;
        let values: Vec<_> = units
            .into_iter()
            .map(|unit| {
                serde_json::json!({
                    "id": unit.id, "width": unit.width, "height": unit.height,
                    "thumbnail": unit.thumbnail, "substitution": unit.substitution,
                })
            })
            .collect();
        output_buffer(
            serde_json::to_vec(&values).map_err(|e| e.to_string())?,
            output,
        )
    })
}

/// Set or clear both optional image references of a composite graphic unit.
///
/// # Safety
/// `handle` must be live, `unit_id` NUL-terminated, and optional image IDs
/// either null or NUL-terminated.
#[no_mangle]
pub unsafe extern "C" fn ofd_set_page_composite_unit_images(
    handle: *mut OfdDocument,
    page_index: usize,
    unit_id: *const c_char,
    thumbnail_id: *const c_char,
    substitution_id: *const c_char,
) -> i32 {
    run(|| {
        let thumbnail = if thumbnail_id.is_null() {
            None
        } else {
            Some(input_string(thumbnail_id)?)
        };
        let substitution = if substitution_id.is_null() {
            None
        } else {
            Some(input_string(substitution_id)?)
        };
        document_mut(handle)?
            .set_page_composite_unit_images(
                page_index,
                input_string(unit_id)?,
                thumbnail,
                substitution,
            )
            .map_err(|error| error.to_string())
    })
}

/// Get the bytes of a zero-based image object on a zero-based page.
///
/// # Safety
/// `handle` must be live and `output` must point to writable memory.
#[no_mangle]
pub unsafe extern "C" fn ofd_page_image_data(
    handle: *const OfdDocument,
    page_index: usize,
    image_index: usize,
    output: *mut OfdBuffer,
) -> i32 {
    run(|| {
        let bytes = document_ref(handle)?
            .page_image_data(page_index, image_index)
            .map_err(|e| e.to_string())?;
        output_buffer(bytes.to_vec(), output)
    })
}

/// Get original bytes of an image inside a composite graphic object.
///
/// # Safety
/// `handle` must be live and `output` must point to writable memory.
#[no_mangle]
pub unsafe extern "C" fn ofd_page_composite_image_data(
    handle: *const OfdDocument,
    page_index: usize,
    image_index: usize,
    output: *mut OfdBuffer,
) -> i32 {
    run(|| {
        let bytes = document_ref(handle)?
            .page_composite_image_data(page_index, image_index)
            .map_err(|e| e.to_string())?;
        output_buffer(bytes.to_vec(), output)
    })
}

/// Export one page and its templates as UTF-8 SVG markup.
///
/// # Safety
/// `handle` must be live and `output` must point to writable memory.
#[no_mangle]
pub unsafe extern "C" fn ofd_page_svg(
    handle: *const OfdDocument,
    index: usize,
    output: *mut OfdBuffer,
) -> i32 {
    run(|| {
        let svg = document_ref(handle)?
            .page_svg(index)
            .map_err(|error| error.to_string())?;
        output_buffer(svg.into_bytes(), output)
    })
}

/// Render one page as PNG at the requested DPI.
///
/// # Safety
/// `handle` must be live and `output` must point to writable memory.
#[no_mangle]
pub unsafe extern "C" fn ofd_page_png(
    handle: *const OfdDocument,
    index: usize,
    dpi: f64,
    output: *mut OfdBuffer,
) -> i32 {
    run(|| {
        let png = document_ref(handle)?
            .page_png(index, dpi)
            .map_err(|error| error.to_string())?;
        output_buffer(png, output)
    })
}

/// Export all pages as a vector PDF.
///
/// # Safety
/// `handle` must be live and `output` must point to writable memory.
#[no_mangle]
pub unsafe extern "C" fn ofd_to_pdf(handle: *const OfdDocument, output: *mut OfdBuffer) -> i32 {
    run(|| {
        let pdf = document_ref(handle)?
            .to_pdf()
            .map_err(|error| error.to_string())?;
        output_buffer(pdf, output)
    })
}

fn render_options(prefer_image_substitution: i32) -> Result<RenderOptions, String> {
    match prefer_image_substitution {
        0 => Ok(RenderOptions::default()),
        1 => Ok(RenderOptions {
            prefer_image_substitution: true,
        }),
        _ => Err("prefer_image_substitution must be 0 or 1".into()),
    }
}

/// Export a page to SVG, optionally preferring image substitutions.
///
/// # Safety
/// `handle` must be live and `output` must point to writable memory.
#[no_mangle]
pub unsafe extern "C" fn ofd_page_svg_with_options(
    handle: *const OfdDocument,
    index: usize,
    prefer_image_substitution: i32,
    output: *mut OfdBuffer,
) -> i32 {
    run(|| {
        let options = render_options(prefer_image_substitution)?;
        let svg = document_ref(handle)?
            .page_svg_with_options(index, options)
            .map_err(|error| error.to_string())?;
        output_buffer(svg.into_bytes(), output)
    })
}

/// Render a page to PNG, optionally preferring image substitutions.
///
/// # Safety
/// `handle` must be live and `output` must point to writable memory.
#[no_mangle]
pub unsafe extern "C" fn ofd_page_png_with_options(
    handle: *const OfdDocument,
    index: usize,
    dpi: f64,
    prefer_image_substitution: i32,
    output: *mut OfdBuffer,
) -> i32 {
    run(|| {
        let options = render_options(prefer_image_substitution)?;
        let png = document_ref(handle)?
            .page_png_with_options(index, dpi, options)
            .map_err(|error| error.to_string())?;
        output_buffer(png, output)
    })
}

/// Export to PDF, optionally preferring image substitutions.
///
/// # Safety
/// `handle` must be live and `output` must point to writable memory.
#[no_mangle]
pub unsafe extern "C" fn ofd_to_pdf_with_options(
    handle: *const OfdDocument,
    prefer_image_substitution: i32,
    output: *mut OfdBuffer,
) -> i32 {
    run(|| {
        let options = render_options(prefer_image_substitution)?;
        let pdf = document_ref(handle)?
            .to_pdf_with_options(options)
            .map_err(|error| error.to_string())?;
        output_buffer(pdf, output)
    })
}

/// Get attachment metadata as a UTF-8 JSON array.
///
/// # Safety
/// `handle` must be live and `output` must point to writable memory.
#[no_mangle]
pub unsafe extern "C" fn ofd_attachments_json(
    handle: *const OfdDocument,
    output: *mut OfdBuffer,
) -> i32 {
    run(|| {
        let attachments = document_ref(handle)?
            .attachments()
            .map_err(|e| e.to_string())?;
        let values: Vec<_> = attachments
            .into_iter()
            .map(|item| {
                serde_json::json!({
                    "document_index": item.document_index,
                    "id": item.id,
                    "name": item.name,
                    "path": item.path,
                    "size": item.size,
                    "format": item.format,
                    "creation_date": item.creation_date,
                    "mod_date": item.mod_date,
                    "visible": item.visible,
                    "usage": item.usage,
                })
            })
            .collect();
        output_buffer(
            serde_json::to_vec(&values).map_err(|e| e.to_string())?,
            output,
        )
    })
}

/// Get annotation metadata as a UTF-8 JSON array.
///
/// # Safety
/// `handle` must be live and `output` must point to writable memory.
#[no_mangle]
pub unsafe extern "C" fn ofd_annotations_json(
    handle: *const OfdDocument,
    output: *mut OfdBuffer,
) -> i32 {
    run(|| {
        let annotations = document_ref(handle)?
            .annotations()
            .map_err(|e| e.to_string())?;
        let values: Vec<_> = annotations
            .into_iter()
            .map(|item| {
                serde_json::json!({
                    "document_index": item.document_index,
                    "page_id": item.page_id,
                    "id": item.id,
                    "type": item.annotation_type,
                    "subtype": item.subtype,
                    "last_mod_date": item.last_mod_date,
                    "remark": item.remark,
                    "parameters": item.parameters,
                    "source_path": item.source_path,
                    "appearance_box": item.appearance_box,
                    "attributes": item.attributes,
                    "text": item.text,
                    "actions": item.actions.iter().map(page_object_action_json).collect::<Vec<_>>(),
                })
            })
            .collect();
        output_buffer(
            serde_json::to_vec(&values).map_err(|e| e.to_string())?,
            output,
        )
    })
}

/// Read one annotation's direct appearance objects in painting order as JSON.
///
/// # Safety
/// `handle` must be live, `annotation_id` NUL-terminated, and `output` writable.
#[no_mangle]
pub unsafe extern "C" fn ofd_annotation_objects_json(
    handle: *const OfdDocument,
    page_index: usize,
    annotation_id: *const c_char,
    output: *mut OfdBuffer,
) -> i32 {
    run(|| {
        let objects = document_ref(handle)?
            .annotation_objects(page_index, input_string(annotation_id)?)
            .map_err(|error| error.to_string())?;
        let values = objects
            .into_iter()
            .map(|object| {
                let codes = object
                    .codes
                    .into_iter()
                    .map(|code| {
                        serde_json::json!({
                            "x": code.x, "y": code.y, "delta_x": code.delta_x,
                            "delta_y": code.delta_y, "text": code.text,
                        })
                    })
                    .collect::<Vec<_>>();
                let cg_transforms = object
                    .cg_transforms
                    .into_iter()
                    .map(|transform| {
                        serde_json::json!({
                            "code_position": transform.code_position,
                            "code_count": transform.code_count,
                            "glyph_count": transform.glyph_count,
                            "glyphs": transform.glyphs,
                        })
                    })
                    .collect::<Vec<_>>();
                serde_json::json!({
                    "id": object.id,
                    "name": object.name,
                    "blend_mode": object.blend_mode,
                    "kind": object.kind,
                    "boundary": object.boundary,
                    "ctm": object.ctm,
                    "visible": object.visible,
                    "alpha": object.alpha,
                    "draw_param": object.draw_param,
                    "resource_id": object.resource_id,
                    "font_id": object.font_id,
                    "size": object.size,
                    "h_scale": object.h_scale,
                    "fill": object.fill,
                    "stroke": object.stroke,
                    "read_direction": object.read_direction,
                    "char_direction": object.char_direction,
                    "weight": object.weight,
                    "italic": object.italic,
                    "text": object.text,
                    "codes": codes,
                    "cg_transforms": cg_transforms,
                    "commands": object.commands,
                })
            })
            .collect::<Vec<_>>();
        output_buffer(
            serde_json::to_vec(&values).map_err(|error| error.to_string())?,
            output,
        )
    })
}

/// Update one annotation appearance object's geometry, visibility, or alpha.
/// JSON accepts boundary, ctm (array or null to clear), visible, and alpha.
///
/// # Safety
/// `handle` must be live and all input strings NUL-terminated.
#[no_mangle]
pub unsafe extern "C" fn ofd_update_annotation_object_json(
    handle: *mut OfdDocument,
    page_index: usize,
    annotation_id: *const c_char,
    object_id: *const c_char,
    update_json: *const c_char,
) -> i32 {
    run(|| {
        let input: serde_json::Value =
            serde_json::from_str(input_string(update_json)?).map_err(|error| error.to_string())?;
        let fields = input
            .as_object()
            .ok_or("annotation object update must be an object")?;
        for name in fields.keys() {
            if !matches!(name.as_str(), "boundary" | "ctm" | "visible" | "alpha") {
                return Err(format!("unsupported annotation object update field {name}"));
            }
        }
        let boundary = fields
            .get("boundary")
            .map(|value| {
                serde_json::from_value::<[f64; 4]>(value.clone())
                    .map_err(|error| format!("annotation object boundary: {error}"))
            })
            .transpose()?;
        let ctm = fields
            .get("ctm")
            .map(|value| {
                if value.is_null() {
                    Ok(None)
                } else {
                    serde_json::from_value::<[f64; 6]>(value.clone())
                        .map(Some)
                        .map_err(|error| format!("annotation object ctm: {error}"))
                }
            })
            .transpose()?;
        let visible = fields
            .get("visible")
            .map(|value| {
                value
                    .as_bool()
                    .ok_or("annotation object visible must be a boolean")
            })
            .transpose()?;
        let alpha = fields
            .get("alpha")
            .map(|value| {
                serde_json::from_value::<u8>(value.clone())
                    .map_err(|error| format!("annotation object alpha: {error}"))
            })
            .transpose()?;
        document_mut(handle)?
            .update_annotation_object(
                page_index,
                input_string(annotation_id)?,
                input_string(object_id)?,
                &AnnotationObjectUpdate {
                    boundary,
                    ctm,
                    visible,
                    alpha,
                },
            )
            .map_err(|error| error.to_string())
    })
}

/// Set or clear an annotation graphic object's optional name.
///
/// # Safety
/// `handle` must be live; IDs and non-null `name` must be NUL-terminated.
#[no_mangle]
pub unsafe extern "C" fn ofd_set_annotation_object_name(
    handle: *mut OfdDocument,
    page_index: usize,
    annotation_id: *const c_char,
    object_id: *const c_char,
    name: *const c_char,
) -> i32 {
    run(|| {
        let name = if name.is_null() {
            None
        } else {
            Some(input_string(name)?)
        };
        document_mut(handle)?
            .set_annotation_object_name(
                page_index,
                input_string(annotation_id)?,
                input_string(object_id)?,
                name,
            )
            .map_err(|error| error.to_string())
    })
}

/// Set or clear an annotation graphic object's compositing mode.
///
/// # Safety
/// `handle` must be live; IDs and non-null `blend_mode` must be NUL-terminated.
#[no_mangle]
pub unsafe extern "C" fn ofd_set_annotation_object_blend_mode(
    handle: *mut OfdDocument,
    page_index: usize,
    annotation_id: *const c_char,
    object_id: *const c_char,
    blend_mode: *const c_char,
) -> i32 {
    run(|| {
        let blend_mode = if blend_mode.is_null() {
            None
        } else {
            Some(input_string(blend_mode)?)
        };
        document_mut(handle)?
            .set_annotation_object_blend_mode(
                page_index,
                input_string(annotation_id)?,
                input_string(object_id)?,
                blend_mode,
            )
            .map_err(|error| error.to_string())
    })
}

/// Set or clear an annotation graphic object's DrawParam resource reference.
///
/// # Safety
/// `handle` must be live; IDs and non-null `draw_param` must be NUL-terminated.
#[no_mangle]
pub unsafe extern "C" fn ofd_set_annotation_object_draw_param(
    handle: *mut OfdDocument,
    page_index: usize,
    annotation_id: *const c_char,
    object_id: *const c_char,
    draw_param: *const c_char,
) -> i32 {
    run(|| {
        let draw_param = if draw_param.is_null() {
            None
        } else {
            Some(input_string(draw_param)?)
        };
        document_mut(handle)?
            .set_annotation_object_draw_param(
                page_index,
                input_string(annotation_id)?,
                input_string(object_id)?,
                draw_param,
            )
            .map_err(|error| error.to_string())
    })
}

/// Remove one direct annotation appearance object by ID.
///
/// # Safety
/// `handle` must be live and both IDs NUL-terminated.
#[no_mangle]
pub unsafe extern "C" fn ofd_remove_annotation_object(
    handle: *mut OfdDocument,
    page_index: usize,
    annotation_id: *const c_char,
    object_id: *const c_char,
) -> i32 {
    run(|| {
        document_mut(handle)?
            .remove_annotation_object(
                page_index,
                input_string(annotation_id)?,
                input_string(object_id)?,
            )
            .map_err(|error| error.to_string())
    })
}

/// Replace one annotation PathObject's abbreviated drawing commands.
///
/// # Safety
/// `handle` must be live and all strings NUL-terminated.
#[no_mangle]
pub unsafe extern "C" fn ofd_set_annotation_path_data(
    handle: *mut OfdDocument,
    page_index: usize,
    annotation_id: *const c_char,
    object_id: *const c_char,
    commands: *const c_char,
) -> i32 {
    run(|| {
        document_mut(handle)?
            .set_annotation_path_data(
                page_index,
                input_string(annotation_id)?,
                input_string(object_id)?,
                input_string(commands)?,
            )
            .map_err(|error| error.to_string())
    })
}

/// Replace direct stroke geometry on an annotation PathObject.
/// JSON uses the same schema as `ofd_set_page_path_stroke_style_json`.
///
/// # Safety
/// `handle` must be live and all strings NUL-terminated.
#[no_mangle]
pub unsafe extern "C" fn ofd_set_annotation_path_stroke_style_json(
    handle: *mut OfdDocument,
    page_index: usize,
    annotation_id: *const c_char,
    object_id: *const c_char,
    style_json: *const c_char,
) -> i32 {
    run(|| {
        let style = parse_path_stroke_style(input_string(style_json)?)?;
        document_mut(handle)?
            .set_annotation_path_stroke_style(
                page_index,
                input_string(annotation_id)?,
                input_string(object_id)?,
                style,
            )
            .map_err(|error| error.to_string())
    })
}

/// Atomically replace direct typography on an annotation TextObject.
/// JSON requires size; omitted optional fields clear their direct attributes.
///
/// # Safety
/// `handle` must be live and all strings NUL-terminated.
#[no_mangle]
pub unsafe extern "C" fn ofd_set_annotation_text_typography_json(
    handle: *mut OfdDocument,
    page_index: usize,
    annotation_id: *const c_char,
    object_id: *const c_char,
    style_json: *const c_char,
) -> i32 {
    #[derive(Deserialize)]
    #[serde(deny_unknown_fields)]
    struct Input {
        size: f64,
        h_scale: Option<f64>,
        read_direction: Option<u16>,
        char_direction: Option<u16>,
        weight: Option<u16>,
        italic: Option<bool>,
    }
    run(|| {
        let input: Input =
            serde_json::from_str(input_string(style_json)?).map_err(|error| error.to_string())?;
        document_mut(handle)?
            .set_annotation_text_typography(
                page_index,
                input_string(annotation_id)?,
                input_string(object_id)?,
                TextTypographyStyle {
                    size: input.size,
                    h_scale: input.h_scale,
                    read_direction: input.read_direction,
                    char_direction: input.char_direction,
                    weight: input.weight,
                    italic: input.italic,
                },
            )
            .map_err(|error| error.to_string())
    })
}

/// Set direct annotation text fill and stroke switches; missing JSON fields
/// restore OFD defaults.
///
/// # Safety
/// `handle` must be live and all strings NUL-terminated.
#[no_mangle]
pub unsafe extern "C" fn ofd_set_annotation_text_paint_flags_json(
    handle: *mut OfdDocument,
    page_index: usize,
    annotation_id: *const c_char,
    object_id: *const c_char,
    flags_json: *const c_char,
) -> i32 {
    #[derive(Deserialize)]
    #[serde(deny_unknown_fields)]
    struct Input {
        fill: Option<bool>,
        stroke: Option<bool>,
    }
    run(|| {
        let input: Input =
            serde_json::from_str(input_string(flags_json)?).map_err(|error| error.to_string())?;
        document_mut(handle)?
            .set_annotation_text_paint_flags(
                page_index,
                input_string(annotation_id)?,
                input_string(object_id)?,
                input.fill,
                input.stroke,
            )
            .map_err(|error| error.to_string())
    })
}

/// Replace one annotation TextCode's direct positioning attributes from JSON.
/// Missing fields remove direct values; the first run must retain X and Y.
///
/// # Safety
/// `handle` must be live and all strings NUL-terminated.
#[no_mangle]
pub unsafe extern "C" fn ofd_set_annotation_text_code_layout_json(
    handle: *mut OfdDocument,
    page_index: usize,
    annotation_id: *const c_char,
    object_id: *const c_char,
    code_index: usize,
    layout_json: *const c_char,
) -> i32 {
    #[derive(Deserialize)]
    #[serde(deny_unknown_fields)]
    struct Input {
        x: Option<f64>,
        y: Option<f64>,
        delta_x: Option<Vec<f64>>,
        delta_y: Option<Vec<f64>>,
    }
    run(|| {
        let input: Input =
            serde_json::from_str(input_string(layout_json)?).map_err(|error| error.to_string())?;
        document_mut(handle)?
            .set_annotation_text_code_layout(
                page_index,
                input_string(annotation_id)?,
                input_string(object_id)?,
                code_index,
                &TextCodeLayout {
                    x: input.x,
                    y: input.y,
                    delta_x: input.delta_x,
                    delta_y: input.delta_y,
                },
            )
            .map_err(|error| error.to_string())
    })
}

/// Set or append an annotation TextCode from JSON, including embedded NUL text.
/// `set` requires page_index, annotation_id, object_id, code_index, and text.
/// `append` requires page_index, annotation_id, object_id, text, and layout.
///
/// # Safety
/// `handle` must be live and `request_json` NUL-terminated.
#[no_mangle]
pub unsafe extern "C" fn ofd_edit_annotation_text_code_json(
    handle: *mut OfdDocument,
    request_json: *const c_char,
) -> i32 {
    #[derive(Deserialize)]
    #[serde(deny_unknown_fields)]
    struct Layout {
        x: Option<f64>,
        y: Option<f64>,
        delta_x: Option<Vec<f64>>,
        delta_y: Option<Vec<f64>>,
    }
    #[derive(Deserialize)]
    #[serde(tag = "operation", rename_all = "snake_case", deny_unknown_fields)]
    enum Input {
        Set {
            page_index: usize,
            annotation_id: String,
            object_id: String,
            code_index: usize,
            text: String,
        },
        Append {
            page_index: usize,
            annotation_id: String,
            object_id: String,
            text: String,
            layout: Layout,
        },
    }
    run(|| {
        let input: Input =
            serde_json::from_str(input_string(request_json)?).map_err(|error| error.to_string())?;
        let document = document_mut(handle)?;
        match input {
            Input::Set {
                page_index,
                annotation_id,
                object_id,
                code_index,
                text,
            } => document
                .set_annotation_text_code(page_index, &annotation_id, &object_id, code_index, &text)
                .map_err(|error| error.to_string()),
            Input::Append {
                page_index,
                annotation_id,
                object_id,
                text,
                layout,
            } => document
                .append_annotation_text_code(
                    page_index,
                    &annotation_id,
                    &object_id,
                    &text,
                    &TextCodeLayout {
                        x: layout.x,
                        y: layout.y,
                        delta_x: layout.delta_x,
                        delta_y: layout.delta_y,
                    },
                )
                .map_err(|error| error.to_string()),
        }
    })
}

/// Append one positioned TextCode run to an annotation TextObject.
/// `layout_json` accepts optional x, y, delta_x, and delta_y fields.
///
/// # Safety
/// `handle` must be live and all strings NUL-terminated.
#[no_mangle]
pub unsafe extern "C" fn ofd_append_annotation_text_code_json(
    handle: *mut OfdDocument,
    page_index: usize,
    annotation_id: *const c_char,
    object_id: *const c_char,
    text: *const c_char,
    layout_json: *const c_char,
) -> i32 {
    #[derive(Deserialize)]
    #[serde(deny_unknown_fields)]
    struct Input {
        x: Option<f64>,
        y: Option<f64>,
        delta_x: Option<Vec<f64>>,
        delta_y: Option<Vec<f64>>,
    }
    run(|| {
        let input: Input =
            serde_json::from_str(input_string(layout_json)?).map_err(|error| error.to_string())?;
        document_mut(handle)?
            .append_annotation_text_code(
                page_index,
                input_string(annotation_id)?,
                input_string(object_id)?,
                input_string(text)?,
                &TextCodeLayout {
                    x: input.x,
                    y: input.y,
                    delta_x: input.delta_x,
                    delta_y: input.delta_y,
                },
            )
            .map_err(|error| error.to_string())
    })
}

/// Remove one TextCode run from an annotation TextObject.
///
/// # Safety
/// `handle` must be live and all strings NUL-terminated.
#[no_mangle]
pub unsafe extern "C" fn ofd_remove_annotation_text_code(
    handle: *mut OfdDocument,
    page_index: usize,
    annotation_id: *const c_char,
    object_id: *const c_char,
    code_index: usize,
) -> i32 {
    run(|| {
        document_mut(handle)?
            .remove_annotation_text_code(
                page_index,
                input_string(annotation_id)?,
                input_string(object_id)?,
                code_index,
            )
            .map_err(|error| error.to_string())
    })
}

/// Set an annotation TextObject's font to a page-visible resource.
///
/// # Safety
/// `handle` must be live and all strings NUL-terminated.
#[no_mangle]
pub unsafe extern "C" fn ofd_set_annotation_text_font(
    handle: *mut OfdDocument,
    page_index: usize,
    annotation_id: *const c_char,
    object_id: *const c_char,
    font_id: *const c_char,
) -> i32 {
    run(|| {
        document_mut(handle)?
            .set_annotation_text_font(
                page_index,
                input_string(annotation_id)?,
                input_string(object_id)?,
                input_string(font_id)?,
            )
            .map_err(|error| error.to_string())
    })
}

/// Replace one TextCode run in an annotation TextObject.
///
/// # Safety
/// `handle` must be live and all strings NUL-terminated.
#[no_mangle]
pub unsafe extern "C" fn ofd_set_annotation_text_code(
    handle: *mut OfdDocument,
    page_index: usize,
    annotation_id: *const c_char,
    object_id: *const c_char,
    code_index: usize,
    text: *const c_char,
) -> i32 {
    run(|| {
        document_mut(handle)?
            .set_annotation_text_code(
                page_index,
                input_string(annotation_id)?,
                input_string(object_id)?,
                code_index,
                input_string(text)?,
            )
            .map_err(|error| error.to_string())
    })
}

/// Replace an annotation ImageObject or CompositeObject resource reference.
///
/// # Safety
/// `handle` must be live and all strings NUL-terminated.
#[no_mangle]
pub unsafe extern "C" fn ofd_set_annotation_graphic_resource(
    handle: *mut OfdDocument,
    page_index: usize,
    annotation_id: *const c_char,
    object_id: *const c_char,
    resource_id: *const c_char,
) -> i32 {
    run(|| {
        document_mut(handle)?
            .set_annotation_graphic_resource(
                page_index,
                input_string(annotation_id)?,
                input_string(object_id)?,
                input_string(resource_id)?,
            )
            .map_err(|error| error.to_string())
    })
}

/// Remove one annotation from a page by its OFD ID.
///
/// # Safety
/// `handle` must be live and `annotation_id` NUL-terminated.
#[no_mangle]
pub unsafe extern "C" fn ofd_remove_annotation(
    handle: *mut OfdDocument,
    page_index: usize,
    annotation_id: *const c_char,
) -> i32 {
    run(|| {
        document_mut(handle)?
            .remove_annotation(page_index, input_string(annotation_id)?)
            .map_err(|error| error.to_string())
    })
}

/// Set an annotation's required Creator attribute.
///
/// # Safety
/// `handle` must be live; `annotation_id` and `creator` must be NUL-terminated.
#[no_mangle]
pub unsafe extern "C" fn ofd_set_annotation_creator(
    handle: *mut OfdDocument,
    page_index: usize,
    annotation_id: *const c_char,
    creator: *const c_char,
) -> i32 {
    run(|| {
        document_mut(handle)?
            .set_annotation_creator(
                page_index,
                input_string(annotation_id)?,
                input_string(creator)?,
            )
            .map_err(|error| error.to_string())
    })
}

/// Set or remove an annotation's optional Subtype; null removes it.
///
/// # Safety
/// `handle` must be live and `annotation_id` NUL-terminated. A nonnull
/// `subtype` must be NUL-terminated.
#[no_mangle]
pub unsafe extern "C" fn ofd_set_annotation_subtype(
    handle: *mut OfdDocument,
    page_index: usize,
    annotation_id: *const c_char,
    subtype: *const c_char,
) -> i32 {
    run(|| {
        let value = if subtype.is_null() {
            None
        } else {
            Some(input_string(subtype)?)
        };
        document_mut(handle)?
            .set_annotation_subtype(page_index, input_string(annotation_id)?, value)
            .map_err(|error| error.to_string())
    })
}

/// Set an annotation's LastModDate in YYYY-MM-DD form.
///
/// # Safety
/// `handle` must be live and both strings NUL-terminated.
#[no_mangle]
pub unsafe extern "C" fn ofd_set_annotation_last_mod_date(
    handle: *mut OfdDocument,
    page_index: usize,
    annotation_id: *const c_char,
    date: *const c_char,
) -> i32 {
    run(|| {
        document_mut(handle)?
            .set_annotation_last_mod_date(
                page_index,
                input_string(annotation_id)?,
                input_string(date)?,
            )
            .map_err(|error| error.to_string())
    })
}

/// Set an annotation appearance boundary in page millimeters.
///
/// # Safety
/// `handle` must be live and `annotation_id` NUL-terminated.
#[no_mangle]
pub unsafe extern "C" fn ofd_set_annotation_appearance_boundary(
    handle: *mut OfdDocument,
    page_index: usize,
    annotation_id: *const c_char,
    x: f64,
    y: f64,
    width: f64,
    height: f64,
) -> i32 {
    run(|| {
        document_mut(handle)?
            .set_annotation_appearance_boundary(
                page_index,
                input_string(annotation_id)?,
                [x, y, width, height],
            )
            .map_err(|error| error.to_string())
    })
}

/// Set or remove an annotation Remark; a null `remark` removes it.
///
/// # Safety
/// `handle` must be live and `annotation_id` NUL-terminated. A nonnull
/// `remark` must be NUL-terminated.
#[no_mangle]
pub unsafe extern "C" fn ofd_set_annotation_remark(
    handle: *mut OfdDocument,
    page_index: usize,
    annotation_id: *const c_char,
    remark: *const c_char,
) -> i32 {
    run(|| {
        let value = if remark.is_null() {
            None
        } else {
            Some(input_string(remark)?)
        };
        document_mut(handle)?
            .set_annotation_remark(page_index, input_string(annotation_id)?, value)
            .map_err(|error| error.to_string())
    })
}

/// Add or replace a named annotation Parameter.
///
/// # Safety
/// `handle` must be live and all strings NUL-terminated.
#[no_mangle]
pub unsafe extern "C" fn ofd_set_annotation_parameter(
    handle: *mut OfdDocument,
    page_index: usize,
    annotation_id: *const c_char,
    name: *const c_char,
    value: *const c_char,
) -> i32 {
    run(|| {
        document_mut(handle)?
            .set_annotation_parameter(
                page_index,
                input_string(annotation_id)?,
                input_string(name)?,
                input_string(value)?,
            )
            .map_err(|error| error.to_string())
    })
}

/// Remove a named annotation Parameter.
///
/// # Safety
/// `handle` must be live and both strings NUL-terminated.
#[no_mangle]
pub unsafe extern "C" fn ofd_remove_annotation_parameter(
    handle: *mut OfdDocument,
    page_index: usize,
    annotation_id: *const c_char,
    name: *const c_char,
) -> i32 {
    run(|| {
        document_mut(handle)?
            .remove_annotation_parameter(
                page_index,
                input_string(annotation_id)?,
                input_string(name)?,
            )
            .map_err(|error| error.to_string())
    })
}

/// Set an annotation's Visible attribute (0 or 1).
///
/// # Safety
/// `handle` must be live and `annotation_id` NUL-terminated.
#[no_mangle]
pub unsafe extern "C" fn ofd_set_annotation_visible(
    handle: *mut OfdDocument,
    page_index: usize,
    annotation_id: *const c_char,
    visible: i32,
) -> i32 {
    run(|| {
        let visible = match visible {
            0 => false,
            1 => true,
            _ => return Err("visible must be 0 or 1".into()),
        };
        document_mut(handle)?
            .set_annotation_visible(page_index, input_string(annotation_id)?, visible)
            .map_err(|error| error.to_string())
    })
}

/// Set an annotation's Print attribute (0 or 1).
///
/// # Safety
/// `handle` must be live and `annotation_id` NUL-terminated.
#[no_mangle]
pub unsafe extern "C" fn ofd_set_annotation_print(
    handle: *mut OfdDocument,
    page_index: usize,
    annotation_id: *const c_char,
    print: i32,
) -> i32 {
    run(|| {
        let print = match print {
            0 => false,
            1 => true,
            _ => return Err("print must be 0 or 1".into()),
        };
        document_mut(handle)?
            .set_annotation_print(page_index, input_string(annotation_id)?, print)
            .map_err(|error| error.to_string())
    })
}

/// Set one annotation metadata flag: NoZoom, NoRotate, or ReadOnly.
/// `value` must be 0 or 1.
///
/// # Safety
/// `handle` must be live; `annotation_id` and `flag` must be NUL-terminated.
#[no_mangle]
pub unsafe extern "C" fn ofd_set_annotation_flag(
    handle: *mut OfdDocument,
    page_index: usize,
    annotation_id: *const c_char,
    flag: *const c_char,
    value: i32,
) -> i32 {
    run(|| {
        let value = match value {
            0 => false,
            1 => true,
            _ => return Err("annotation flag value must be 0 or 1".into()),
        };
        let flag = input_string(flag)?;
        let id = input_string(annotation_id)?;
        let document = document_mut(handle)?;
        let result = match flag {
            "NoZoom" => document.set_annotation_no_zoom(page_index, id, value),
            "NoRotate" => document.set_annotation_no_rotate(page_index, id, value),
            "ReadOnly" => document.set_annotation_read_only(page_index, id, value),
            _ => return Err("unsupported annotation flag".into()),
        };
        result.map_err(|error| error.to_string())
    })
}

/// Get font resource metadata as a UTF-8 JSON array.
///
/// # Safety
/// `handle` must be live and `output` must point to writable memory.
#[no_mangle]
pub unsafe extern "C" fn ofd_fonts_json(handle: *const OfdDocument, output: *mut OfdBuffer) -> i32 {
    run(|| {
        let fonts = document_ref(handle)?.fonts().map_err(|e| e.to_string())?;
        output_buffer(font_infos_json(fonts)?, output)
    })
}

/// Set or clear a standard OFD font attribute by index in `ofd_fonts_json`.
///
/// # Safety
/// `handle` must be live, `name` must be NUL-terminated, and `value` must
/// either be null or point to a NUL-terminated string.
#[no_mangle]
pub unsafe extern "C" fn ofd_set_font_attribute(
    handle: *mut OfdDocument,
    font_index: usize,
    name: *const c_char,
    value: *const c_char,
) -> i32 {
    run(|| {
        let value = if value.is_null() {
            None
        } else {
            Some(input_string(value)?)
        };
        document_mut(handle)?
            .set_font_attribute(font_index, input_string(name)?, value)
            .map_err(|error| error.to_string())
    })
}

/// Set or clear an attribute by index in `ofd_page_fonts_json`.
///
/// # Safety
/// `handle` must be live, `name` NUL-terminated, and `value` null or
/// NUL-terminated.
#[no_mangle]
pub unsafe extern "C" fn ofd_set_page_font_attribute(
    handle: *mut OfdDocument,
    page_index: usize,
    font_index: usize,
    name: *const c_char,
    value: *const c_char,
) -> i32 {
    run(|| {
        let value = if value.is_null() {
            None
        } else {
            Some(input_string(value)?)
        };
        document_mut(handle)?
            .set_page_font_attribute(page_index, font_index, input_string(name)?, value)
            .map_err(|error| error.to_string())
    })
}

/// Replace an embedded font by index in `ofd_fonts_json`.
///
/// # Safety
/// `handle` must be live and `data` valid for `len` bytes.
#[no_mangle]
pub unsafe extern "C" fn ofd_replace_font_data(
    handle: *mut OfdDocument,
    font_index: usize,
    data: *const u8,
    len: usize,
) -> i32 {
    run(|| {
        document_mut(handle)?
            .replace_font_data(font_index, input_bytes(data, len)?)
            .map_err(|error| error.to_string())
    })
}

/// Replace an embedded font by index in `ofd_page_fonts_json`.
///
/// # Safety
/// `handle` must be live and `data` valid for `len` bytes.
#[no_mangle]
pub unsafe extern "C" fn ofd_replace_page_font_data(
    handle: *mut OfdDocument,
    page_index: usize,
    font_index: usize,
    data: *const u8,
    len: usize,
) -> i32 {
    run(|| {
        document_mut(handle)?
            .replace_page_font_data(page_index, font_index, input_bytes(data, len)?)
            .map_err(|error| error.to_string())
    })
}

fn font_infos_json(fonts: Vec<FontInfo>) -> Result<Vec<u8>, String> {
    let values: Vec<_> = fonts
        .into_iter()
        .map(|font| {
            serde_json::json!({
                "document_index": font.document_index,
                "id": font.id,
                "font_name": font.font_name,
                "family_name": font.family_name,
                "resource_path": font.resource_path,
                "embedded_path": font.embedded_path,
                "attributes": font.attributes,
            })
        })
        .collect();
    serde_json::to_vec(&values).map_err(|error| error.to_string())
}

/// Get fonts available to a page and its templates as a UTF-8 JSON array.
///
/// # Safety
/// `handle` must be live and `output` must point to writable memory.
#[no_mangle]
pub unsafe extern "C" fn ofd_page_fonts_json(
    handle: *const OfdDocument,
    page_index: usize,
    output: *mut OfdBuffer,
) -> i32 {
    run(|| {
        let fonts = document_ref(handle)?
            .page_fonts(page_index)
            .map_err(|error| error.to_string())?;
        output_buffer(font_infos_json(fonts)?, output)
    })
}

/// Get embedded font bytes for an index in `ofd_page_fonts_json`.
///
/// # Safety
/// `handle` must be live and `output` must point to writable memory.
#[no_mangle]
pub unsafe extern "C" fn ofd_page_font_data(
    handle: *const OfdDocument,
    page_index: usize,
    font_index: usize,
    output: *mut OfdBuffer,
) -> i32 {
    run(|| {
        let bytes = document_ref(handle)?
            .page_font_data(page_index, font_index)
            .map_err(|error| error.to_string())?;
        output_buffer(bytes.to_vec(), output)
    })
}

/// Get an embedded font's original bytes by zero-based font index.
///
/// # Safety
/// `handle` must be live and `output` must point to writable memory.
#[no_mangle]
pub unsafe extern "C" fn ofd_font_data(
    handle: *const OfdDocument,
    index: usize,
    output: *mut OfdBuffer,
) -> i32 {
    run(|| {
        let bytes = document_ref(handle)?
            .font_data(index)
            .map_err(|e| e.to_string())?;
        output_buffer(bytes.to_vec(), output)
    })
}

/// Get signature descriptions as a UTF-8 JSON array. This does not verify signatures.
///
/// # Safety
/// `handle` must be live and `output` must point to writable memory.
#[no_mangle]
pub unsafe extern "C" fn ofd_signatures_json(
    handle: *const OfdDocument,
    output: *mut OfdBuffer,
) -> i32 {
    run(|| {
        let signatures = document_ref(handle)?
            .signatures()
            .map_err(|e| e.to_string())?;
        let values: Vec<_> = signatures
            .into_iter()
            .map(|signature| {
                serde_json::json!({
                    "document_index": signature.document_index,
                    "id": signature.id,
                    "type": signature.signature_type,
                    "path": signature.path,
                    "signed_value_path": signature.signed_value_path,
                    "seal_path": signature.seal_path,
                    "provider_name": signature.provider_name,
                    "signature_method": signature.signature_method,
                    "signature_datetime": signature.signature_datetime,
                    "check_method": signature.check_method,
                    "references": signature.references.into_iter().map(|reference| {
                        serde_json::json!({
                            "path": reference.path,
                            "check_value": reference.check_value,
                        })
                    }).collect::<Vec<_>>(),
                    "stamps": signature.stamps.into_iter().map(|stamp| {
                        serde_json::json!({
                            "id": stamp.id,
                            "page_ref": stamp.page_ref,
                            "boundary": stamp.boundary,
                            "clip": stamp.clip,
                        })
                    }).collect::<Vec<_>>(),
                })
            })
            .collect();
        output_buffer(
            serde_json::to_vec(&values).map_err(|e| e.to_string())?,
            output,
        )
    })
}

/// Get opaque signature value bytes by zero-based signature index.
///
/// # Safety
/// `handle` must be live and `output` must point to writable memory.
#[no_mangle]
pub unsafe extern "C" fn ofd_signed_value_data(
    handle: *const OfdDocument,
    index: usize,
    output: *mut OfdBuffer,
) -> i32 {
    run(|| {
        let bytes = document_ref(handle)?
            .signed_value_data(index)
            .map_err(|e| e.to_string())?;
        output_buffer(bytes.to_vec(), output)
    })
}

/// Get electronic seal bytes by zero-based signature index.
///
/// # Safety
/// `handle` must be live and `output` must point to writable memory.
#[no_mangle]
pub unsafe extern "C" fn ofd_seal_data(
    handle: *const OfdDocument,
    index: usize,
    output: *mut OfdBuffer,
) -> i32 {
    run(|| {
        let bytes = document_ref(handle)?
            .seal_data(index)
            .map_err(|e| e.to_string())?;
        output_buffer(bytes.to_vec(), output)
    })
}

/// Compare protected file digests and return a UTF-8 JSON array.
///
/// This does not authenticate the cryptographic signature value.
///
/// # Safety
/// `handle` must be live and `output` must point to writable memory.
#[no_mangle]
pub unsafe extern "C" fn ofd_check_signature_references_json(
    handle: *const OfdDocument,
    index: usize,
    output: *mut OfdBuffer,
) -> i32 {
    run(|| {
        let checks = document_ref(handle)?
            .check_signature_references(index)
            .map_err(|e| e.to_string())?;
        let values: Vec<_> = checks
            .into_iter()
            .map(|check| {
                serde_json::json!({
                    "path": check.path,
                    "valid": check.valid,
                    "expected": check.expected,
                    "actual": check.actual,
                })
            })
            .collect();
        output_buffer(
            serde_json::to_vec(&values).map_err(|e| e.to_string())?,
            output,
        )
    })
}

/// Compare Signature.xml with the digest embedded in an SES v4/v5 seal.
/// This does not verify the seal's cryptographic signature value.
///
/// # Safety
/// `handle` must be live and `output` must point to writable memory.
#[no_mangle]
pub unsafe extern "C" fn ofd_check_signature_description_hash_json(
    handle: *const OfdDocument,
    index: usize,
    output: *mut OfdBuffer,
) -> i32 {
    run(|| {
        let check = document_ref(handle)?
            .check_signature_description_hash(index)
            .map_err(|error| error.to_string())?;
        let value = serde_json::json!({
            "path": check.path,
            "valid": check.valid,
            "expected": check.expected,
            "actual": check.actual,
        });
        output_buffer(
            serde_json::to_vec(&value).map_err(|error| error.to_string())?,
            output,
        )
    })
}

/// Compare a detached SES seal with the seal embedded in SignedValue.dat.
/// `output` receives -1 when the check does not apply, 0 for a mismatch, or
/// 1 for a match.
///
/// # Safety
/// `handle` must be live and `output` must point to writable memory.
#[no_mangle]
pub unsafe extern "C" fn ofd_check_signature_seal_match(
    handle: *const OfdDocument,
    index: usize,
    output: *mut i32,
) -> i32 {
    run(|| {
        let output = output.as_mut().ok_or("null seal match output")?;
        *output = match document_ref(handle)?
            .check_signature_seal_match(index)
            .map_err(|error| error.to_string())?
        {
            None => -1,
            Some(false) => 0,
            Some(true) => 1,
        };
        Ok(())
    })
}

/// Verify the embedded electronic seal's SM2 signature against its issuer
/// certificate. Certificate trust and validity are not checked.
///
/// # Safety
/// `handle` must be live and `output` must point to writable memory.
#[no_mangle]
pub unsafe extern "C" fn ofd_check_ses_seal_signature(
    handle: *const OfdDocument,
    index: usize,
    output: *mut i32,
) -> i32 {
    run(|| {
        let output = output.as_mut().ok_or("null seal signature output")?;
        *output = i32::from(
            document_ref(handle)?
                .check_ses_seal_signature(index)
                .map_err(|error| error.to_string())?,
        );
        Ok(())
    })
}

/// Verify the seal signature with a caller-supplied SM2 distinguishing ID.
///
/// # Safety
/// `handle` must be live, `seal_distid` NUL-terminated, and `output` writable.
#[no_mangle]
pub unsafe extern "C" fn ofd_check_ses_seal_signature_with_distid(
    handle: *const OfdDocument,
    index: usize,
    seal_distid: *const c_char,
    output: *mut i32,
) -> i32 {
    run(|| {
        let output = output.as_mut().ok_or("null seal signature output")?;
        *output = i32::from(
            document_ref(handle)?
                .check_ses_seal_signature_with_distid(index, input_string(seal_distid)?)
                .map_err(|error| error.to_string())?,
        );
        Ok(())
    })
}

/// Check the embedded SES seal issuer certificate's X.509 validity at a
/// caller-supplied Unix timestamp. This does not establish issuer trust.
///
/// # Safety
/// `handle` must be live and `output` must point to writable memory.
#[no_mangle]
pub unsafe extern "C" fn ofd_check_ses_seal_certificate_validity(
    handle: *const OfdDocument,
    index: usize,
    at_unix_seconds: u64,
    output: *mut i32,
) -> i32 {
    run(|| {
        let output = output
            .as_mut()
            .ok_or("null seal certificate validity output")?;
        *output = i32::from(
            document_ref(handle)?
                .check_ses_seal_certificate_validity(index, at_unix_seconds)
                .map_err(|error| error.to_string())?,
        );
        Ok(())
    })
}

/// Check whether the SES signer's certificate is authorized by the embedded
/// seal's certificate list or certificate digest list.
///
/// # Safety
/// `handle` must be live and `output` must point to writable memory.
#[no_mangle]
pub unsafe extern "C" fn ofd_check_ses_seal_signer_authorization(
    handle: *const OfdDocument,
    index: usize,
    output: *mut i32,
) -> i32 {
    run(|| {
        let output = output
            .as_mut()
            .ok_or("null seal signer authorization output")?;
        *output = i32::from(
            document_ref(handle)?
                .check_ses_seal_signer_authorization(index)
                .map_err(|error| error.to_string())?,
        );
        Ok(())
    })
}

/// Check the embedded seal's validity interval at a caller-supplied Unix time.
///
/// # Safety
/// `handle` must be live and `output` must point to writable memory.
#[no_mangle]
pub unsafe extern "C" fn ofd_check_ses_seal_validity(
    handle: *const OfdDocument,
    index: usize,
    at_unix_seconds: u64,
    output: *mut i32,
) -> i32 {
    run(|| {
        let output = output.as_mut().ok_or("null seal validity output")?;
        *output = i32::from(
            document_ref(handle)?
                .check_ses_seal_validity(index, at_unix_seconds)
                .map_err(|error| error.to_string())?,
        );
        Ok(())
    })
}

/// Check an explicitly ordered SM2 X.509 chain from the embedded seal issuer
/// certificate to a caller-trusted anchor. This is independent of the seal
/// signature check. `intermediates_json` is an array of base64 DER values.
///
/// # Safety
/// `handle` must be live, byte pointers readable for their lengths,
/// `certificate_distid` NUL-terminated, and `output` writable.
#[no_mangle]
pub unsafe extern "C" fn ofd_check_ses_seal_issuer_ordered_sm2_chain(
    handle: *const OfdDocument,
    index: usize,
    intermediates_json: *const u8,
    intermediates_json_len: usize,
    anchor_certificate: *const u8,
    anchor_len: usize,
    at_unix_seconds: u64,
    certificate_distid: *const c_char,
    output: *mut i32,
) -> i32 {
    run(|| {
        if intermediates_json_len > 128 * 1024 {
            return Err("intermediate certificate JSON exceeds 128 KiB".into());
        }
        let encoded: Vec<String> =
            serde_json::from_slice(input_bytes(intermediates_json, intermediates_json_len)?)
                .map_err(|error| format!("invalid intermediate certificate JSON: {error}"))?;
        if encoded.len() > 8 {
            return Err("SM2 certificate chain exceeds eight intermediates".into());
        }
        let certificates = encoded
            .iter()
            .enumerate()
            .map(|(position, value)| {
                STANDARD.decode(value).map_err(|error| {
                    format!("invalid base64 intermediate certificate {position}: {error}")
                })
            })
            .collect::<Result<Vec<_>, _>>()?;
        let intermediates: Vec<&[u8]> = certificates.iter().map(Vec::as_slice).collect();
        let output = output.as_mut().ok_or("null seal issuer chain output")?;
        *output = i32::from(
            document_ref(handle)?
                .check_ses_seal_issuer_ordered_sm2_chain(
                    index,
                    &intermediates,
                    input_bytes(anchor_certificate, anchor_len)?,
                    at_unix_seconds,
                    input_string(certificate_distid)?,
                )
                .map_err(|error| error.to_string())?,
        );
        Ok(())
    })
}

/// Verify the SM2 signature value in an SES v4/v5 seal. `output` receives 1
/// for a matching signature and 0 otherwise; certificate trust is not checked.
///
/// # Safety
/// `handle` must be live and `output` must point to writable memory.
#[no_mangle]
pub unsafe extern "C" fn ofd_check_ses_signature_value(
    handle: *const OfdDocument,
    index: usize,
    output: *mut i32,
) -> i32 {
    run(|| {
        let output = output.as_mut().ok_or("null signature result output")?;
        *output = i32::from(
            document_ref(handle)?
                .check_ses_signature_value(index)
                .map_err(|error| error.to_string())?,
        );
        Ok(())
    })
}

/// Verify an SES signature value with a caller-supplied SM2 distinguishing ID.
/// Certificate trust and protected file digests are separate checks.
///
/// # Safety
/// `handle` must be live, `signature_distid` NUL-terminated, and `output` writable.
#[no_mangle]
pub unsafe extern "C" fn ofd_check_ses_signature_value_with_distid(
    handle: *const OfdDocument,
    index: usize,
    signature_distid: *const c_char,
    output: *mut i32,
) -> i32 {
    run(|| {
        let output = output.as_mut().ok_or("null signature result output")?;
        *output = i32::from(
            document_ref(handle)?
                .check_ses_signature_value_with_distid(index, input_string(signature_distid)?)
                .map_err(|error| error.to_string())?,
        );
        Ok(())
    })
}

/// Verify a raw SM2 signature with a caller-supplied DER X.509 certificate.
/// `output` receives 1 for a matching signature and 0 otherwise; certificate
/// trust and validity are not checked.
///
/// # Safety
/// `handle` must be live, `certificate` readable for `len` bytes, and
/// `output` must point to writable memory.
#[no_mangle]
pub unsafe extern "C" fn ofd_check_digital_signature_with_certificate(
    handle: *const OfdDocument,
    index: usize,
    certificate: *const u8,
    len: usize,
    output: *mut i32,
) -> i32 {
    run(|| {
        let output = output.as_mut().ok_or("null signature result output")?;
        *output = i32::from(
            document_ref(handle)?
                .check_digital_signature_with_certificate(index, input_bytes(certificate, len)?)
                .map_err(|error| error.to_string())?,
        );
        Ok(())
    })
}

/// Verify a raw SM2 signature with a DER certificate and distinguishing ID.
///
/// # Safety
/// `handle` must be live, certificate readable for `len` bytes,
/// `signature_distid` NUL-terminated, and `output` writable.
#[no_mangle]
pub unsafe extern "C" fn ofd_check_digital_signature_with_certificate_and_distid(
    handle: *const OfdDocument,
    index: usize,
    certificate: *const u8,
    len: usize,
    signature_distid: *const c_char,
    output: *mut i32,
) -> i32 {
    run(|| {
        let output = output.as_mut().ok_or("null signature result output")?;
        *output = i32::from(
            document_ref(handle)?
                .check_digital_signature_with_certificate_and_distid(
                    index,
                    input_bytes(certificate, len)?,
                    input_string(signature_distid)?,
                )
                .map_err(|error| error.to_string())?,
        );
        Ok(())
    })
}

/// Check signer certificate validity at a caller-supplied Unix timestamp.
/// SES signatures use the embedded certificate and require an empty input;
/// raw digital signatures require a supplied DER certificate.
///
/// # Safety
/// `handle` must be live, `certificate` readable for `len` bytes, and
/// `output` must point to writable memory.
#[no_mangle]
pub unsafe extern "C" fn ofd_check_signature_certificate_validity(
    handle: *const OfdDocument,
    index: usize,
    certificate: *const u8,
    len: usize,
    at_unix_seconds: u64,
    output: *mut i32,
) -> i32 {
    run(|| {
        let output = output.as_mut().ok_or("null certificate validity output")?;
        *output = i32::from(
            document_ref(handle)?
                .check_signature_certificate_validity(
                    index,
                    input_bytes(certificate, len)?,
                    at_unix_seconds,
                )
                .map_err(|error| error.to_string())?,
        );
        Ok(())
    })
}

/// Check one-step SM2 certificate issuance by a caller-trusted anchor,
/// signer and issuer validity, CA usage, and the OFD signature value.
/// Protected file digests and revocation are not checked.
///
/// # Safety
/// `handle` must be live, byte pointers readable for their lengths,
/// `certificate_distid` a valid NUL-terminated string, and `output` writable.
#[no_mangle]
pub unsafe extern "C" fn ofd_check_signature_direct_trust_anchor(
    handle: *const OfdDocument,
    index: usize,
    signer_certificate: *const u8,
    signer_len: usize,
    anchor_certificate: *const u8,
    anchor_len: usize,
    at_unix_seconds: u64,
    certificate_distid: *const c_char,
    output: *mut i32,
) -> i32 {
    run(|| {
        let output = output.as_mut().ok_or("null trust anchor result output")?;
        *output = i32::from(
            document_ref(handle)?
                .check_signature_direct_trust_anchor(
                    index,
                    input_bytes(signer_certificate, signer_len)?,
                    input_bytes(anchor_certificate, anchor_len)?,
                    at_unix_seconds,
                    input_string(certificate_distid)?,
                )
                .map_err(|error| error.to_string())?,
        );
        Ok(())
    })
}

/// Return a UTF-8 JSON report covering protected files, signature value,
/// certificate validity, and a directly trusted issuer. The caller supplies
/// the trust anchor and evaluation time. Revocation is not checked.
///
/// # Safety
/// `handle` must be live, byte pointers readable for their lengths,
/// `certificate_distid` a valid NUL-terminated string, and `output` writable.
#[no_mangle]
pub unsafe extern "C" fn ofd_verify_signature_with_direct_trust_anchor_json(
    handle: *const OfdDocument,
    index: usize,
    signer_certificate: *const u8,
    signer_len: usize,
    anchor_certificate: *const u8,
    anchor_len: usize,
    at_unix_seconds: u64,
    certificate_distid: *const c_char,
    output: *mut OfdBuffer,
) -> i32 {
    unsafe {
        ofd_verify_signature_with_direct_trust_anchor_and_distids_json(
            handle,
            index,
            signer_certificate,
            signer_len,
            anchor_certificate,
            anchor_len,
            at_unix_seconds,
            certificate_distid,
            DEFAULT_SM2_DISTID_C.as_ptr().cast(),
            output,
        )
    }
}

/// Return a direct-anchor report with separate certificate and OFD SM2 IDs.
///
/// # Safety
/// `handle` must be live, byte pointers readable for their lengths, both IDs
/// NUL-terminated, and `output` writable.
#[no_mangle]
pub unsafe extern "C" fn ofd_verify_signature_with_direct_trust_anchor_and_distids_json(
    handle: *const OfdDocument,
    index: usize,
    signer_certificate: *const u8,
    signer_len: usize,
    anchor_certificate: *const u8,
    anchor_len: usize,
    at_unix_seconds: u64,
    certificate_distid: *const c_char,
    signature_distid: *const c_char,
    output: *mut OfdBuffer,
) -> i32 {
    run(|| {
        let report = document_ref(handle)?
            .verify_signature_with_direct_trust_anchor_and_distids(
                index,
                input_bytes(signer_certificate, signer_len)?,
                input_bytes(anchor_certificate, anchor_len)?,
                at_unix_seconds,
                Sm2DistinguishingIds {
                    certificate: input_string(certificate_distid)?,
                    signature: input_string(signature_distid)?,
                },
            )
            .map_err(|error| error.to_string())?;
        let references: Vec<_> = report
            .references
            .into_iter()
            .map(|check| {
                serde_json::json!({
                    "path": check.path,
                    "valid": check.valid,
                    "expected": check.expected,
                    "actual": check.actual,
                })
            })
            .collect();
        let description_hash = report.description_hash.map(|check| {
            serde_json::json!({
                "path": check.path,
                "valid": check.valid,
                "expected": check.expected,
                "actual": check.actual,
            })
        });
        let value = serde_json::json!({
            "references": references,
            "references_valid": report.references_valid,
            "description_hash": description_hash,
            "seal_match": report.seal_match,
            "seal_signature_valid": report.seal_signature_valid,
            "seal_certificate_validity_valid": report.seal_certificate_validity_valid,
            "seal_signer_authorized": report.seal_signer_authorized,
            "seal_validity_valid": report.seal_validity_valid,
            "signature_value_valid": report.signature_value_valid,
            "certificate_validity_valid": report.certificate_validity_valid,
            "direct_trust_anchor_valid": report.direct_trust_anchor_valid,
            "all_valid": report.all_valid,
        });
        output_buffer(
            serde_json::to_vec(&value).map_err(|error| error.to_string())?,
            output,
        )
    })
}

/// Return a JSON report for an explicitly ordered SM2 certificate chain.
/// `intermediates_json` is a UTF-8 JSON array of base64 DER certificates,
/// ordered from the signer toward the trusted anchor. It is limited to 128 KiB.
/// Revocation, path building, policies, and name constraints are not checked.
///
/// # Safety
/// `handle` must be live, byte pointers readable for their lengths,
/// `certificate_distid` a valid NUL-terminated string, and `output` writable.
#[no_mangle]
pub unsafe extern "C" fn ofd_verify_signature_with_ordered_sm2_chain_json(
    handle: *const OfdDocument,
    index: usize,
    signer_certificate: *const u8,
    signer_len: usize,
    intermediates_json: *const u8,
    intermediates_json_len: usize,
    anchor_certificate: *const u8,
    anchor_len: usize,
    at_unix_seconds: u64,
    certificate_distid: *const c_char,
    output: *mut OfdBuffer,
) -> i32 {
    unsafe {
        ofd_verify_signature_with_ordered_sm2_chain_and_distids_json(
            handle,
            index,
            signer_certificate,
            signer_len,
            intermediates_json,
            intermediates_json_len,
            anchor_certificate,
            anchor_len,
            at_unix_seconds,
            certificate_distid,
            DEFAULT_SM2_DISTID_C.as_ptr().cast(),
            output,
        )
    }
}

// Shared bridge for ordered-chain and unordered-candidate verification.
#[expect(
    clippy::too_many_arguments,
    reason = "mirrors the existing C ABI parameter list"
)]
unsafe fn verify_signature_chain_json(
    handle: *const OfdDocument,
    index: usize,
    signer_certificate: *const u8,
    signer_len: usize,
    intermediates_json: *const u8,
    intermediates_json_len: usize,
    anchor_certificate: *const u8,
    anchor_len: usize,
    at_unix_seconds: u64,
    certificate_distid: *const c_char,
    signature_distid: *const c_char,
    output: *mut OfdBuffer,
    unordered: bool,
) -> i32 {
    run(|| {
        if intermediates_json_len > 128 * 1024 {
            return Err("intermediate certificate JSON exceeds 128 KiB".into());
        }
        let encoded: Vec<String> =
            serde_json::from_slice(input_bytes(intermediates_json, intermediates_json_len)?)
                .map_err(|error| format!("invalid intermediate certificate JSON: {error}"))?;
        if encoded.len() > if unordered { 16 } else { 8 } {
            return Err("SM2 certificate chain has too many intermediates".into());
        }
        let certificates = encoded
            .iter()
            .enumerate()
            .map(|(position, value)| {
                STANDARD.decode(value).map_err(|error| {
                    format!("invalid base64 intermediate certificate {position}: {error}")
                })
            })
            .collect::<Result<Vec<_>, _>>()?;
        let intermediates: Vec<&[u8]> = certificates.iter().map(Vec::as_slice).collect();
        let ids = Sm2DistinguishingIds {
            certificate: input_string(certificate_distid)?,
            signature: input_string(signature_distid)?,
        };
        let document = document_ref(handle)?;
        let signer = input_bytes(signer_certificate, signer_len)?;
        let anchor = input_bytes(anchor_certificate, anchor_len)?;
        let report = if unordered {
            document.verify_signature_with_certificate_pool(
                index,
                signer,
                &intermediates,
                anchor,
                at_unix_seconds,
                ids,
            )
        } else {
            document.verify_signature_with_ordered_sm2_chain_and_distids(
                index,
                signer,
                &intermediates,
                anchor,
                at_unix_seconds,
                ids,
            )
        }
        .map_err(|error| error.to_string())?;
        let value = signature_chain_report_json(report);
        output_buffer(
            serde_json::to_vec(&value).map_err(|error| error.to_string())?,
            output,
        )
    })
}

/// Return an ordered-chain report with separate certificate and OFD SM2 IDs.
///
/// # Safety
/// `handle` must be live, byte pointers readable for their lengths, both IDs
/// NUL-terminated, and `output` writable.
#[no_mangle]
pub unsafe extern "C" fn ofd_verify_signature_with_ordered_sm2_chain_and_distids_json(
    handle: *const OfdDocument,
    index: usize,
    signer_certificate: *const u8,
    signer_len: usize,
    intermediates_json: *const u8,
    intermediates_json_len: usize,
    anchor_certificate: *const u8,
    anchor_len: usize,
    at_unix_seconds: u64,
    certificate_distid: *const c_char,
    signature_distid: *const c_char,
    output: *mut OfdBuffer,
) -> i32 {
    verify_signature_chain_json(
        handle,
        index,
        signer_certificate,
        signer_len,
        intermediates_json,
        intermediates_json_len,
        anchor_certificate,
        anchor_len,
        at_unix_seconds,
        certificate_distid,
        signature_distid,
        output,
        false,
    )
}

/// Return a verification report after searching an unordered certificate pool.
///
/// # Safety
/// `handle` must be live, byte pointers readable for their lengths, both IDs
/// NUL-terminated, and `output` writable.
#[no_mangle]
pub unsafe extern "C" fn ofd_verify_signature_with_certificate_pool_json(
    handle: *const OfdDocument,
    index: usize,
    signer_certificate: *const u8,
    signer_len: usize,
    candidates_json: *const u8,
    candidates_json_len: usize,
    anchor_certificate: *const u8,
    anchor_len: usize,
    at_unix_seconds: u64,
    certificate_distid: *const c_char,
    signature_distid: *const c_char,
    output: *mut OfdBuffer,
) -> i32 {
    verify_signature_chain_json(
        handle,
        index,
        signer_certificate,
        signer_len,
        candidates_json,
        candidates_json_len,
        anchor_certificate,
        anchor_len,
        at_unix_seconds,
        certificate_distid,
        signature_distid,
        output,
        true,
    )
}

fn signature_chain_report_json(report: SignatureChainVerificationReport) -> serde_json::Value {
    let references: Vec<_> = report
        .references
        .into_iter()
        .map(|check| {
            serde_json::json!({
                "path": check.path,
                "valid": check.valid,
                "expected": check.expected,
                "actual": check.actual,
            })
        })
        .collect();
    let description_hash = report.description_hash.map(|check| {
        serde_json::json!({
            "path": check.path,
            "valid": check.valid,
            "expected": check.expected,
            "actual": check.actual,
        })
    });
    serde_json::json!({
        "references": references,
        "references_valid": report.references_valid,
        "description_hash": description_hash,
        "seal_match": report.seal_match,
        "seal_signature_valid": report.seal_signature_valid,
        "seal_certificate_validity_valid": report.seal_certificate_validity_valid,
        "seal_signer_authorized": report.seal_signer_authorized,
        "seal_validity_valid": report.seal_validity_valid,
        "signature_value_valid": report.signature_value_valid,
        "certificate_validity_valid": report.certificate_validity_valid,
        "certificate_chain_valid": report.certificate_chain_valid,
        "all_valid": report.all_valid,
    })
}

#[derive(Deserialize)]
struct SignatureTrustOptions {
    signer_certificate_der_base64: String,
    signer_intermediates_der_base64: Vec<String>,
    signer_anchor_der_base64: String,
    seal_intermediates_der_base64: Vec<String>,
    seal_anchor_der_base64: String,
    at_unix_seconds: u64,
    signer_certificate_distid: String,
    signer_signature_distid: String,
    seal_certificate_distid: String,
    seal_signature_distid: String,
}

// Shared bridge for ordered signer/seal chains and unordered certificate pools.
unsafe fn verify_signature_trust_json(
    handle: *const OfdDocument,
    index: usize,
    options_json: *const u8,
    options_json_len: usize,
    output: *mut OfdBuffer,
    unordered: bool,
) -> i32 {
    run(|| {
        if options_json_len > 256 * 1024 {
            return Err("signature trust options JSON exceeds 256 KiB".into());
        }
        let options: SignatureTrustOptions =
            serde_json::from_slice(input_bytes(options_json, options_json_len)?)
                .map_err(|error| format!("invalid signature trust options JSON: {error}"))?;
        let limit = if unordered { 16 } else { 8 };
        if options.signer_intermediates_der_base64.len() > limit
            || options.seal_intermediates_der_base64.len() > limit
        {
            return Err("SM2 certificate chain has too many intermediates".into());
        }
        let decode = |value: &str, name: &str| {
            STANDARD
                .decode(value)
                .map_err(|error| format!("invalid base64 {name}: {error}"))
        };
        let signer = decode(&options.signer_certificate_der_base64, "signer certificate")?;
        let signer_anchor = decode(&options.signer_anchor_der_base64, "signer anchor")?;
        let seal_anchor = decode(&options.seal_anchor_der_base64, "seal anchor")?;
        let signer_intermediates = options
            .signer_intermediates_der_base64
            .iter()
            .enumerate()
            .map(|(index, value)| decode(value, &format!("signer intermediate {index}")))
            .collect::<Result<Vec<_>, _>>()?;
        let seal_intermediates = options
            .seal_intermediates_der_base64
            .iter()
            .enumerate()
            .map(|(index, value)| decode(value, &format!("seal intermediate {index}")))
            .collect::<Result<Vec<_>, _>>()?;
        let signer_intermediates: Vec<&[u8]> =
            signer_intermediates.iter().map(Vec::as_slice).collect();
        let seal_intermediates: Vec<&[u8]> = seal_intermediates.iter().map(Vec::as_slice).collect();
        let signer_ids = Sm2DistinguishingIds {
            certificate: &options.signer_certificate_distid,
            signature: &options.signer_signature_distid,
        };
        let seal_ids = Sm2DistinguishingIds {
            certificate: &options.seal_certificate_distid,
            signature: &options.seal_signature_distid,
        };
        let document = document_ref(handle)?;
        let report = if unordered {
            document.verify_signature_with_certificate_pools(
                index,
                &signer,
                &signer_intermediates,
                &signer_anchor,
                &seal_intermediates,
                &seal_anchor,
                options.at_unix_seconds,
                signer_ids,
                seal_ids,
            )
        } else {
            document.verify_signature_with_ordered_sm2_chains_and_distids(
                index,
                &signer,
                &signer_intermediates,
                &signer_anchor,
                &seal_intermediates,
                &seal_anchor,
                options.at_unix_seconds,
                signer_ids,
                seal_ids,
            )
        }
        .map_err(|error| error.to_string())?;
        let value = serde_json::json!({
            "signature": signature_chain_report_json(report.signature),
            "seal_issuer_chain_valid": report.seal_issuer_chain_valid,
            "all_valid": report.all_valid,
        });
        output_buffer(
            serde_json::to_vec(&value).map_err(|error| error.to_string())?,
            output,
        )
    })
}

/// Return a combined signer and seal issuer trust report from ordered chains.
///
/// # Safety
/// `handle` must be live, `options_json` readable for its length, and
/// `output` must point to writable memory.
#[no_mangle]
pub unsafe extern "C" fn ofd_verify_signature_with_ordered_sm2_chains_json(
    handle: *const OfdDocument,
    index: usize,
    options_json: *const u8,
    options_json_len: usize,
    output: *mut OfdBuffer,
) -> i32 {
    verify_signature_trust_json(handle, index, options_json, options_json_len, output, false)
}

/// Return signer and seal issuer trust results after searching both candidate pools.
/// JSON fields match the ordered-chain function; intermediates are candidates.
///
/// # Safety
/// `handle` must be live, `options_json` readable for its length, and
/// `output` must point to writable memory.
#[no_mangle]
pub unsafe extern "C" fn ofd_verify_signature_with_certificate_pools_json(
    handle: *const OfdDocument,
    index: usize,
    options_json: *const u8,
    options_json_len: usize,
    output: *mut OfdBuffer,
) -> i32 {
    verify_signature_trust_json(handle, index, options_json, options_json_len, output, true)
}

/// Add or append a raw SM2 signature with a 32-byte private key scalar.
///
/// # Safety
/// `handle` must be live, `private_key` readable for `len` bytes,
/// `signature_datetime` a valid NUL-terminated string, and `output_index`
/// writable.
#[no_mangle]
pub unsafe extern "C" fn ofd_add_digital_signature(
    handle: *mut OfdDocument,
    private_key: *const u8,
    len: usize,
    signature_datetime: *const c_char,
    output_index: *mut usize,
) -> i32 {
    run(|| {
        let output = output_index.as_mut().ok_or("null signature index output")?;
        *output = document_mut(handle)?
            .add_digital_signature(
                input_bytes(private_key, len)?,
                input_string(signature_datetime)?,
            )
            .map_err(|error| error.to_string())?;
        Ok(())
    })
}

/// Add or append a raw SM2 signature with a caller-supplied distinguishing ID.
///
/// # Safety
/// `handle` must be live, `private_key` readable for `len` bytes, both strings
/// NUL-terminated, and `output_index` writable.
#[no_mangle]
pub unsafe extern "C" fn ofd_add_digital_signature_with_distid(
    handle: *mut OfdDocument,
    private_key: *const u8,
    len: usize,
    signature_datetime: *const c_char,
    signature_distid: *const c_char,
    output_index: *mut usize,
) -> i32 {
    run(|| {
        let output = output_index.as_mut().ok_or("null signature index output")?;
        *output = document_mut(handle)?
            .add_digital_signature_with_distid(
                input_bytes(private_key, len)?,
                input_string(signature_datetime)?,
                input_string(signature_distid)?,
            )
            .map_err(|error| error.to_string())?;
        Ok(())
    })
}

/// Add or append an SES v4/v5 signature using an already signed seal.
/// Standard SM2 distinguishing IDs are used for the signer and seal.
///
/// # Safety
/// `handle` must be live, byte pointers readable for their lengths, date
/// NUL-terminated, and `output_index` writable.
#[no_mangle]
pub unsafe extern "C" fn ofd_add_ses_signature(
    handle: *mut OfdDocument,
    private_key: *const u8,
    private_key_len: usize,
    signer_certificate: *const u8,
    signer_certificate_len: usize,
    seal: *const u8,
    seal_len: usize,
    signature_datetime: *const c_char,
    output_index: *mut usize,
) -> i32 {
    run(|| {
        let output = output_index.as_mut().ok_or("null signature index output")?;
        *output = document_mut(handle)?
            .add_ses_signature(
                input_bytes(private_key, private_key_len)?,
                input_bytes(signer_certificate, signer_certificate_len)?,
                input_bytes(seal, seal_len)?,
                input_string(signature_datetime)?,
            )
            .map_err(|error| error.to_string())?;
        Ok(())
    })
}

/// Add or append an SES signature with separate signer and seal SM2 IDs.
///
/// # Safety
/// `handle` must be live, byte pointers readable for their lengths, strings
/// NUL-terminated, and `output_index` writable.
#[no_mangle]
pub unsafe extern "C" fn ofd_add_ses_signature_with_distids(
    handle: *mut OfdDocument,
    private_key: *const u8,
    private_key_len: usize,
    signer_certificate: *const u8,
    signer_certificate_len: usize,
    seal: *const u8,
    seal_len: usize,
    signature_datetime: *const c_char,
    signature_distid: *const c_char,
    seal_distid: *const c_char,
    output_index: *mut usize,
) -> i32 {
    run(|| {
        let output = output_index.as_mut().ok_or("null signature index output")?;
        *output = document_mut(handle)?
            .add_ses_signature_with_distids(
                input_bytes(private_key, private_key_len)?,
                input_bytes(signer_certificate, signer_certificate_len)?,
                input_bytes(seal, seal_len)?,
                input_string(signature_datetime)?,
                input_string(signature_distid)?,
                input_string(seal_distid)?,
            )
            .map_err(|error| error.to_string())?;
        Ok(())
    })
}

/// Add or append a positioned SES stamp on a zero-based page index.
///
/// # Safety
/// `handle` must be live, byte pointers readable for their lengths, strings
/// NUL-terminated, and `output_index` writable.
#[no_mangle]
pub unsafe extern "C" fn ofd_add_ses_signature_with_stamp_and_distids(
    handle: *mut OfdDocument,
    private_key: *const u8,
    private_key_len: usize,
    signer_certificate: *const u8,
    signer_certificate_len: usize,
    seal: *const u8,
    seal_len: usize,
    signature_datetime: *const c_char,
    page_index: usize,
    x_mm: f64,
    y_mm: f64,
    width_mm: f64,
    height_mm: f64,
    signature_distid: *const c_char,
    seal_distid: *const c_char,
    output_index: *mut usize,
) -> i32 {
    run(|| {
        let output = output_index.as_mut().ok_or("null signature index output")?;
        *output = document_mut(handle)?
            .add_ses_signature_with_stamp_and_distids(
                input_bytes(private_key, private_key_len)?,
                input_bytes(signer_certificate, signer_certificate_len)?,
                input_bytes(seal, seal_len)?,
                input_string(signature_datetime)?,
                page_index,
                [x_mm, y_mm, width_mm, height_mm],
                input_string(signature_distid)?,
                input_string(seal_distid)?,
            )
            .map_err(|error| error.to_string())?;
        Ok(())
    })
}

#[derive(Deserialize)]
struct SesStampInput {
    signature_datetime: String,
    page_index: usize,
    boundary: [f64; 4],
    signature_distid: String,
    seal_distid: String,
}

/// Add a positioned SES stamp using JSON placement and distinguishing IDs.
/// This variant keeps the argument count within Windows FFI limits.
///
/// # Safety
/// `handle` must be live, byte pointers readable for their lengths, `options_json`
/// NUL-terminated, and `output_index` writable.
#[no_mangle]
pub unsafe extern "C" fn ofd_add_ses_signature_with_stamp_json(
    handle: *mut OfdDocument,
    private_key: *const u8,
    private_key_len: usize,
    signer_certificate: *const u8,
    signer_certificate_len: usize,
    seal: *const u8,
    seal_len: usize,
    options_json: *const c_char,
    output_index: *mut usize,
) -> i32 {
    run(|| {
        let output = output_index.as_mut().ok_or("null signature index output")?;
        let input: SesStampInput = serde_json::from_str(input_string(options_json)?)
            .map_err(|error| format!("invalid SES stamp options JSON: {error}"))?;
        *output = document_mut(handle)?
            .add_ses_signature_with_stamp_and_distids(
                input_bytes(private_key, private_key_len)?,
                input_bytes(signer_certificate, signer_certificate_len)?,
                input_bytes(seal, seal_len)?,
                &input.signature_datetime,
                input.page_index,
                input.boundary,
                &input.signature_distid,
                &input.seal_distid,
            )
            .map_err(|error| error.to_string())?;
        Ok(())
    })
}

#[derive(Deserialize)]
struct SesSealSpecInput {
    version: u8,
    vendor_id: String,
    seal_id: String,
    name: String,
    seal_type: u8,
    authorized_signer_certificates_base64: Vec<String>,
    created_at: u64,
    valid_from: u64,
    valid_until: u64,
    picture_type: String,
    picture_width_mm: u32,
    picture_height_mm: u32,
}

/// Create an SES v4/v5 electronic seal. The JSON options include base64 DER
/// entries in `authorized_signer_certificates_base64`.
///
/// # Safety
/// Byte pointers must be readable for their lengths, `issuer_distid` must be
/// NUL-terminated, and `output` must be writable and later freed.
#[no_mangle]
pub unsafe extern "C" fn ofd_create_ses_seal_json(
    issuer_private_key: *const u8,
    issuer_private_key_len: usize,
    issuer_certificate: *const u8,
    issuer_certificate_len: usize,
    picture: *const u8,
    picture_len: usize,
    options_json: *const u8,
    options_json_len: usize,
    issuer_distid: *const c_char,
    output: *mut OfdBuffer,
) -> i32 {
    run(|| {
        let input: SesSealSpecInput =
            serde_json::from_slice(input_bytes(options_json, options_json_len)?)
                .map_err(|error| format!("invalid SES seal options JSON: {error}"))?;
        let authorized_signer_certificates = input
            .authorized_signer_certificates_base64
            .iter()
            .map(|encoded| {
                STANDARD
                    .decode(encoded)
                    .map_err(|error| format!("invalid authorized certificate base64: {error}"))
            })
            .collect::<Result<Vec<_>, _>>()?;
        let spec = SesSealSpec {
            version: input.version,
            vendor_id: input.vendor_id,
            seal_id: input.seal_id,
            name: input.name,
            seal_type: input.seal_type,
            authorized_signer_certificates,
            created_at: input.created_at,
            valid_from: input.valid_from,
            valid_until: input.valid_until,
            picture_type: input.picture_type,
            picture_width_mm: input.picture_width_mm,
            picture_height_mm: input.picture_height_mm,
        };
        let seal = create_ses_seal_with_distid(
            input_bytes(issuer_private_key, issuer_private_key_len)?,
            input_bytes(issuer_certificate, issuer_certificate_len)?,
            input_bytes(picture, picture_len)?,
            &spec,
            input_string(issuer_distid)?,
        )
        .map_err(|error| error.to_string())?;
        output_buffer(seal, output)
    })
}

/// Get attachment bytes by zero-based attachment index.
///
/// # Safety
/// `handle` must be live and `output` must point to writable memory.
#[no_mangle]
pub unsafe extern "C" fn ofd_attachment_data(
    handle: *const OfdDocument,
    index: usize,
    output: *mut OfdBuffer,
) -> i32 {
    run(|| {
        let data = document_ref(handle)?
            .attachment_data(index)
            .map_err(|e| e.to_string())?;
        output_buffer(data.to_vec(), output)
    })
}

fn custom_tag_json(tag: &CustomTagInfo) -> serde_json::Value {
    serde_json::json!({
        "document_index": tag.document_index, "type_id": tag.type_id,
        "namespace": tag.namespace, "schema_path": tag.schema_path, "path": tag.path,
    })
}

fn extension_json(extension: &ExtensionInfo) -> serde_json::Value {
    serde_json::json!({
        "document_index": extension.document_index,
        "app_name": extension.app_name, "company": extension.company,
        "app_version": extension.app_version, "date": extension.date,
        "ref_id": extension.ref_id,
        "properties": extension.properties.iter().map(|property| serde_json::json!({
            "name": property.name, "type_name": property.type_name, "value": property.value,
        })).collect::<Vec<_>>(),
        "extend_data_paths": extension.extend_data_paths,
        "data_xml": extension.data_xml,
    })
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct ExtensionPropertyInput {
    name: String,
    type_name: Option<String>,
    value: String,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct ExtensionInput {
    document_index: usize,
    app_name: String,
    company: Option<String>,
    app_version: Option<String>,
    date: Option<String>,
    ref_id: Option<u64>,
    properties: Vec<ExtensionPropertyInput>,
    extend_data_paths: Vec<String>,
    data_xml: Vec<String>,
}

/// List document extensions as a UTF-8 JSON array.
///
/// # Safety
/// `handle` must be live and `output` writable.
#[no_mangle]
pub unsafe extern "C" fn ofd_extensions_json(
    handle: *const OfdDocument,
    output: *mut OfdBuffer,
) -> i32 {
    run(|| {
        let extensions = document_ref(handle)?
            .extensions()
            .map_err(|e| e.to_string())?;
        let values: Vec<_> = extensions.iter().map(extension_json).collect();
        output_buffer(
            serde_json::to_vec(&values).map_err(|e| e.to_string())?,
            output,
        )
    })
}

/// Append one document extension from UTF-8 JSON.
///
/// # Safety
/// `handle` must be live and `extension_json` NUL-terminated.
#[no_mangle]
pub unsafe extern "C" fn ofd_add_extension_json(
    handle: *mut OfdDocument,
    extension_json: *const c_char,
) -> i32 {
    run(|| {
        let input: ExtensionInput =
            serde_json::from_str(input_string(extension_json)?).map_err(|e| e.to_string())?;
        let extension = ExtensionInfo {
            document_index: input.document_index,
            app_name: input.app_name,
            company: input.company,
            app_version: input.app_version,
            date: input.date,
            ref_id: input.ref_id,
            properties: input
                .properties
                .into_iter()
                .map(|property| ExtensionProperty {
                    name: property.name,
                    type_name: property.type_name,
                    value: property.value,
                })
                .collect(),
            extend_data_paths: input.extend_data_paths,
            data_xml: input.data_xml,
        };
        document_mut(handle)?
            .add_document_extension(&extension)
            .map_err(|e| e.to_string())
    })
}

/// Replace one extension record from UTF-8 JSON by global index.
///
/// # Safety
/// `handle` must be live and `extension_json` NUL-terminated.
#[no_mangle]
pub unsafe extern "C" fn ofd_update_extension_json(
    handle: *mut OfdDocument,
    index: usize,
    extension_json: *const c_char,
) -> i32 {
    run(|| {
        let input: ExtensionInput =
            serde_json::from_str(input_string(extension_json)?).map_err(|e| e.to_string())?;
        let extension = ExtensionInfo {
            document_index: input.document_index,
            app_name: input.app_name,
            company: input.company,
            app_version: input.app_version,
            date: input.date,
            ref_id: input.ref_id,
            properties: input
                .properties
                .into_iter()
                .map(|property| ExtensionProperty {
                    name: property.name,
                    type_name: property.type_name,
                    value: property.value,
                })
                .collect(),
            extend_data_paths: input.extend_data_paths,
            data_xml: input.data_xml,
        };
        document_mut(handle)?
            .update_document_extension(index, &extension)
            .map_err(|e| e.to_string())
    })
}

/// Remove an extension by global index, retaining referenced package files.
///
/// # Safety
/// `handle` must be live.
#[no_mangle]
pub unsafe extern "C" fn ofd_remove_extension(handle: *mut OfdDocument, index: usize) -> i32 {
    run(|| {
        document_mut(handle)?
            .remove_document_extension(index)
            .map_err(|e| e.to_string())
    })
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct CustomTagInput {
    type_id: String,
    namespace: Option<String>,
    schema_path: Option<String>,
}

/// List custom tag entries as a UTF-8 JSON array.
///
/// # Safety
/// `handle` must be live and `output` writable.
#[no_mangle]
pub unsafe extern "C" fn ofd_custom_tags_json(
    handle: *const OfdDocument,
    output: *mut OfdBuffer,
) -> i32 {
    run(|| {
        let tags = document_ref(handle)?
            .custom_tags()
            .map_err(|e| e.to_string())?;
        let values: Vec<_> = tags.iter().map(custom_tag_json).collect();
        output_buffer(
            serde_json::to_vec(&values).map_err(|e| e.to_string())?,
            output,
        )
    })
}

/// Get custom tag payload bytes by global index.
///
/// # Safety
/// `handle` must be live and `output` writable.
#[no_mangle]
pub unsafe extern "C" fn ofd_custom_tag_data(
    handle: *const OfdDocument,
    index: usize,
    output: *mut OfdBuffer,
) -> i32 {
    run(|| {
        let data = document_ref(handle)?
            .custom_tag_data(index)
            .map_err(|e| e.to_string())?;
        output_buffer(data.to_vec(), output)
    })
}

/// Replace a custom tag's XML payload and return its updated metadata as JSON.
///
/// # Safety
/// `handle` must be live, `data` readable for `len` bytes, and `output` writable.
#[no_mangle]
pub unsafe extern "C" fn ofd_replace_custom_tag_data(
    handle: *mut OfdDocument,
    index: usize,
    data: *const u8,
    len: usize,
    output: *mut OfdBuffer,
) -> i32 {
    run(|| {
        if output.is_null() {
            return Err("null output buffer".into());
        }
        let tag = document_mut(handle)?
            .replace_custom_tag_data(index, input_bytes(data, len)?)
            .map_err(|e| e.to_string())?;
        output_buffer(
            serde_json::to_vec(&custom_tag_json(&tag)).map_err(|e| e.to_string())?,
            output,
        )
    })
}

/// Add a custom tag and XML payload; return its metadata as UTF-8 JSON.
///
/// # Safety
/// `handle` must be live, `metadata_json` NUL-terminated, `data` readable for
/// `len` bytes, and `output` writable.
#[no_mangle]
pub unsafe extern "C" fn ofd_add_custom_tag_json(
    handle: *mut OfdDocument,
    document_index: usize,
    metadata_json: *const c_char,
    data: *const u8,
    len: usize,
    output: *mut OfdBuffer,
) -> i32 {
    run(|| {
        if output.is_null() {
            return Err("null output buffer".into());
        }
        let input: CustomTagInput =
            serde_json::from_str(input_string(metadata_json)?).map_err(|e| e.to_string())?;
        let tag = document_mut(handle)?
            .add_custom_tag_to_document(
                document_index,
                &input.type_id,
                input.namespace.as_deref(),
                input.schema_path.as_deref(),
                input_bytes(data, len)?,
            )
            .map_err(|e| e.to_string())?;
        output_buffer(
            serde_json::to_vec(&custom_tag_json(&tag)).map_err(|e| e.to_string())?,
            output,
        )
    })
}

/// Update custom tag metadata by global index; return the updated metadata.
///
/// # Safety
/// `handle` must be live, `metadata_json` NUL-terminated, and `output` writable.
#[no_mangle]
pub unsafe extern "C" fn ofd_update_custom_tag_json(
    handle: *mut OfdDocument,
    index: usize,
    metadata_json: *const c_char,
    output: *mut OfdBuffer,
) -> i32 {
    run(|| {
        if output.is_null() {
            return Err("null output buffer".into());
        }
        let input: CustomTagInput =
            serde_json::from_str(input_string(metadata_json)?).map_err(|e| e.to_string())?;
        let tag = document_mut(handle)?
            .update_custom_tag(
                index,
                &input.type_id,
                input.namespace.as_deref(),
                input.schema_path.as_deref(),
            )
            .map_err(|e| e.to_string())?;
        output_buffer(
            serde_json::to_vec(&custom_tag_json(&tag)).map_err(|e| e.to_string())?,
            output,
        )
    })
}

/// Remove a custom tag by global index, retaining its payload and schema.
///
/// # Safety
/// `handle` must be live.
#[no_mangle]
pub unsafe extern "C" fn ofd_remove_custom_tag(handle: *mut OfdDocument, index: usize) -> i32 {
    run(|| {
        document_mut(handle)?
            .remove_custom_tag(index)
            .map_err(|e| e.to_string())
    })
}

/// Get a package entry by its normalized package path.
///
/// # Safety
/// `handle` must be live, `path` must be NUL-terminated, and `output` writable.
#[no_mangle]
pub unsafe extern "C" fn ofd_get_entry(
    handle: *const OfdDocument,
    path: *const c_char,
    output: *mut OfdBuffer,
) -> i32 {
    run(|| {
        let path = input_string(path)?;
        let bytes = document_ref(handle)?
            .entry(path)
            .map_err(|e| e.to_string())?
            .ok_or_else(|| format!("entry not found: {path}"))?;
        output_buffer(bytes.to_vec(), output)
    })
}

/// Insert or replace a package entry.
///
/// # Safety
/// `handle` must be live, `path` NUL-terminated, and `data` valid for `len` bytes.
#[no_mangle]
pub unsafe extern "C" fn ofd_set_entry(
    handle: *mut OfdDocument,
    path: *const c_char,
    data: *const u8,
    len: usize,
) -> i32 {
    run(|| {
        let path = input_string(path)?;
        let bytes = input_bytes(data, len)?;
        document_mut(handle)?
            .set_entry(path, bytes.to_vec())
            .map_err(|e| e.to_string())
    })
}

/// Remove a package entry and report whether it existed.
///
/// # Safety
/// `handle` must be live, `path` NUL-terminated, and `removed` writable.
#[no_mangle]
pub unsafe extern "C" fn ofd_remove_entry(
    handle: *mut OfdDocument,
    path: *const c_char,
    removed: *mut i32,
) -> i32 {
    run(|| {
        let output = removed.as_mut().ok_or("null removed output")?;
        *output = 0;
        *output = i32::from(
            document_mut(handle)?
                .remove_entry(input_string(path)?)
                .map_err(|error| error.to_string())?,
        );
        Ok(())
    })
}

/// Add a fixed-layout text page to a newly created package.
///
/// # Safety
/// `handle` must be live and `text` must be a valid NUL-terminated string.
#[no_mangle]
pub unsafe extern "C" fn ofd_add_text_page(
    handle: *mut OfdDocument,
    text: *const c_char,
    width_mm: f64,
    height_mm: f64,
) -> i32 {
    run(|| {
        document_mut(handle)?
            .add_text_page(input_string(text)?, width_mm, height_mm)
            .map_err(|e| e.to_string())
    })
}

/// Write a text page from JSON, preserving all UTF-8 characters in `text`.
///
/// `operation` is `add`, `append`, or `insert`. `add` accepts an optional
/// `font_id`; `append` requires `document_index` and accepts `font_id`;
/// `insert` requires `document_index`, `position`, and a writable `output`.
/// All operations require `text`, `width_mm`, and `height_mm`.
///
/// # Safety
/// `handle` must be live and `request_json` NUL-terminated. For `insert`,
/// `output` must be writable; it may be null for the other operations.
#[no_mangle]
pub unsafe extern "C" fn ofd_write_text_page_json(
    handle: *mut OfdDocument,
    request_json: *const c_char,
    output: *mut usize,
) -> i32 {
    #[derive(Deserialize)]
    #[serde(tag = "operation", rename_all = "snake_case", deny_unknown_fields)]
    enum Input {
        Add {
            text: String,
            width_mm: f64,
            height_mm: f64,
            font_id: Option<usize>,
        },
        Append {
            document_index: usize,
            text: String,
            width_mm: f64,
            height_mm: f64,
            font_id: Option<usize>,
        },
        Insert {
            document_index: usize,
            position: usize,
            text: String,
            width_mm: f64,
            height_mm: f64,
        },
    }
    run(|| {
        let input: Input =
            serde_json::from_str(input_string(request_json)?).map_err(|e| e.to_string())?;
        let document = document_mut(handle)?;
        match input {
            Input::Add {
                text,
                width_mm,
                height_mm,
                font_id,
            } => match font_id {
                Some(font_id) => {
                    document.add_text_page_with_font(&text, width_mm, height_mm, font_id)
                }
                None => document.add_text_page(&text, width_mm, height_mm),
            }
            .map_err(|e| e.to_string()),
            Input::Append {
                document_index,
                text,
                width_mm,
                height_mm,
                font_id,
            } => match font_id {
                Some(font_id) => document.append_text_page_with_font(
                    document_index,
                    &text,
                    width_mm,
                    height_mm,
                    font_id,
                ),
                None => document.append_text_page(document_index, &text, width_mm, height_mm),
            }
            .map_err(|e| e.to_string()),
            Input::Insert {
                document_index,
                position,
                text,
                width_mm,
                height_mm,
            } => {
                if output.is_null() {
                    return Err("null output pointer".into());
                }
                let index = document
                    .insert_text_page(document_index, position, &text, width_mm, height_mm)
                    .map_err(|e| e.to_string())?;
                unsafe { *output = index };
                Ok(())
            }
        }
    })
}

/// Add an empty page to a newly created package.
///
/// # Safety
/// `handle` must be a live document handle.
#[no_mangle]
pub unsafe extern "C" fn ofd_add_blank_page(
    handle: *mut OfdDocument,
    width_mm: f64,
    height_mm: f64,
) -> i32 {
    run(|| {
        document_mut(handle)?
            .add_blank_page(width_mm, height_mm)
            .map_err(|e| e.to_string())
    })
}

struct TextObjectArguments<'a> {
    text: &'a str,
    boundary: [f64; 4],
    size: f64,
    font_id: Option<&'a str>,
}

fn text_object_arguments(input: &serde_json::Value) -> Result<TextObjectArguments<'_>, String> {
    let text = input
        .get("text")
        .and_then(serde_json::Value::as_str)
        .ok_or("text object text must be a string")?;
    let size = input
        .get("size")
        .and_then(serde_json::Value::as_f64)
        .ok_or("text object size must be a number")?;
    let values = input
        .get("boundary")
        .and_then(serde_json::Value::as_array)
        .ok_or("text object boundary must be an array")?;
    let boundary: [f64; 4] = values
        .iter()
        .map(|value| {
            value
                .as_f64()
                .ok_or("text object boundary must contain numbers")
        })
        .collect::<Result<Vec<_>, _>>()?
        .try_into()
        .map_err(|_| "text object boundary must contain four numbers")?;
    let font_id = match input.get("font_id") {
        None | Some(serde_json::Value::Null) => None,
        Some(value) => Some(
            value
                .as_str()
                .ok_or("text object font_id must be a string")?,
        ),
    };
    Ok(TextObjectArguments {
        text,
        boundary,
        size,
        font_id,
    })
}

/// Add a positioned TextObject to an existing page.
///
/// `object_json` has string `text`, four-number `boundary`, number `size`,
/// optional string `font_id`, optional RGB byte array `fill_color`, optional
/// boolean `shape`, and optional string `language`. Shaping requires an embedded font and one line. `language`
/// requires `shape=true` and selects language-specific OpenType features.
/// The new OFD object ID is
/// written to `output_id`.
///
/// # Safety
/// `handle` must be live, `object_json` NUL-terminated, and `output_id` writable.
#[no_mangle]
pub unsafe extern "C" fn ofd_add_text_object_json(
    handle: *mut OfdDocument,
    page_index: usize,
    object_json: *const c_char,
    output_id: *mut usize,
) -> i32 {
    run(|| {
        let output = output_id.as_mut().ok_or("null text object ID output")?;
        *output = 0;
        let input: serde_json::Value =
            serde_json::from_str(input_string(object_json)?).map_err(|error| error.to_string())?;
        let args = text_object_arguments(&input)?;
        let shape = input
            .get("shape")
            .map(|value| value.as_bool().ok_or("shape must be a boolean"))
            .transpose()?
            .unwrap_or(false);
        let language = input
            .get("language")
            .map(|value| value.as_str().ok_or("language must be a string"))
            .transpose()?;
        if language.is_some() && !shape {
            return Err("language requires shape=true".into());
        }
        let fill_color = input
            .get("fill_color")
            .map(|value| {
                value
                    .as_array()
                    .ok_or("fill_color must be an RGB array")?
                    .iter()
                    .map(|channel| {
                        channel
                            .as_u64()
                            .and_then(|value| u8::try_from(value).ok())
                            .ok_or("fill_color must contain RGB bytes")
                    })
                    .collect::<Result<Vec<_>, _>>()?
                    .try_into()
                    .map_err(|_| "fill_color must contain three RGB bytes")
            })
            .transpose()?;
        let id = if shape {
            let font_id = args.font_id.ok_or("shaping requires font_id")?;
            if let Some(color) = fill_color {
                document_mut(handle)?.add_colored_shaped_text_object(
                    page_index,
                    args.text,
                    args.boundary,
                    args.size,
                    font_id,
                    ColoredShapedTextOptions { color, language },
                )
            } else if let Some(language) = language {
                document_mut(handle)?.add_shaped_text_object_with_language(
                    page_index,
                    args.text,
                    args.boundary,
                    args.size,
                    font_id,
                    language,
                )
            } else {
                document_mut(handle)?.add_shaped_text_object(
                    page_index,
                    args.text,
                    args.boundary,
                    args.size,
                    font_id,
                )
            }
        } else {
            document_mut(handle)?.add_colored_text_object(
                page_index,
                args.text,
                args.boundary,
                args.size,
                args.font_id,
                fill_color,
            )
        }
        .map_err(|error| error.to_string())?;
        *output = id.parse().map_err(|_| "invalid generated object ID")?;
        Ok(())
    })
}

/// Create a document composite resource containing one text object and place it.
///
/// JSON fields match `ofd_add_text_object_json`.
///
/// # Safety
/// `handle` must be live, `object_json` NUL-terminated, and `output_id` writable.
#[no_mangle]
pub unsafe extern "C" fn ofd_add_composite_text_object_json(
    handle: *mut OfdDocument,
    page_index: usize,
    object_json: *const c_char,
    output_id: *mut usize,
) -> i32 {
    run(|| {
        let output = output_id.as_mut().ok_or("null composite text ID output")?;
        *output = 0;
        let input: serde_json::Value =
            serde_json::from_str(input_string(object_json)?).map_err(|error| error.to_string())?;
        let args = text_object_arguments(&input)?;
        let id = document_mut(handle)?
            .add_composite_text_object(
                page_index,
                args.text,
                args.boundary,
                args.size,
                args.font_id,
            )
            .map_err(|error| error.to_string())?;
        *output = id.parse().map_err(|_| "invalid generated object ID")?;
        Ok(())
    })
}

/// Duplicate one direct page graphic object immediately after the original.
/// A fresh OFD object ID is written to `output_id`.
///
/// # Safety
/// `handle` must be live; `object_id` NUL-terminated and `output_id` writable.
#[no_mangle]
pub unsafe extern "C" fn ofd_duplicate_page_object(
    handle: *mut OfdDocument,
    page_index: usize,
    object_id: *const c_char,
    output_id: *mut usize,
) -> i32 {
    run(|| {
        let output = output_id.as_mut().ok_or("null page object ID output")?;
        *output = 0;
        let id = document_mut(handle)?
            .duplicate_page_object(page_index, input_string(object_id)?)
            .map_err(|error| error.to_string())?;
        *output = id.parse().map_err(|_| "invalid generated object ID")?;
        Ok(())
    })
}

/// Remove one text, path, image, or composite object from a page by its OFD ID.
///
/// # Safety
/// `handle` must be live and `object_id` a NUL-terminated string.
#[no_mangle]
pub unsafe extern "C" fn ofd_remove_page_object(
    handle: *mut OfdDocument,
    page_index: usize,
    object_id: *const c_char,
) -> i32 {
    run(|| {
        document_mut(handle)?
            .remove_page_object(page_index, input_string(object_id)?)
            .map_err(|error| error.to_string())
    })
}

/// Create a page layer. `layer_type` is Background, Body, or Foreground.
/// `draw_param` may be null. The generated ID is written to `output_id`.
///
/// # Safety
/// `handle` must be live, strings NUL-terminated, and `output_id` writable.
#[no_mangle]
pub unsafe extern "C" fn ofd_add_page_layer(
    handle: *mut OfdDocument,
    page_index: usize,
    layer_type: *const c_char,
    draw_param: *const c_char,
    output_id: *mut usize,
) -> i32 {
    run(|| {
        let output = output_id.as_mut().ok_or("null layer ID output")?;
        *output = 0;
        let kind = input_string(layer_type)?;
        let draw_param = if draw_param.is_null() {
            None
        } else {
            Some(input_string(draw_param)?)
        };
        let id = document_mut(handle)?
            .add_page_layer(page_index, kind, draw_param)
            .map_err(|error| error.to_string())?;
        *output = id.parse().map_err(|_| "invalid generated layer ID")?;
        Ok(())
    })
}

/// Change a direct page layer's type.
///
/// # Safety
/// `handle` must be live and strings NUL-terminated.
#[no_mangle]
pub unsafe extern "C" fn ofd_set_page_layer_type(
    handle: *mut OfdDocument,
    page_index: usize,
    layer_id: *const c_char,
    layer_type: *const c_char,
) -> i32 {
    run(|| {
        document_mut(handle)?
            .set_page_layer_type(
                page_index,
                input_string(layer_id)?,
                input_string(layer_type)?,
            )
            .map_err(|error| error.to_string())
    })
}

/// Set or clear a direct page layer's name. Null clears it.
///
/// # Safety
/// `handle` must be live; `layer_id` and non-null `name` must be NUL-terminated.
#[no_mangle]
pub unsafe extern "C" fn ofd_set_page_layer_name(
    handle: *mut OfdDocument,
    page_index: usize,
    layer_id: *const c_char,
    name: *const c_char,
) -> i32 {
    run(|| {
        let name = if name.is_null() {
            None
        } else {
            Some(input_string(name)?)
        };
        document_mut(handle)?
            .set_page_layer_name(page_index, input_string(layer_id)?, name)
            .map_err(|error| error.to_string())
    })
}

/// Set or clear a direct page layer's DrawParam reference. Null clears it.
///
/// # Safety
/// `handle` must be live; `layer_id` and non-null `draw_param` must be NUL-terminated.
#[no_mangle]
pub unsafe extern "C" fn ofd_set_page_layer_draw_param(
    handle: *mut OfdDocument,
    page_index: usize,
    layer_id: *const c_char,
    draw_param: *const c_char,
) -> i32 {
    run(|| {
        let draw_param = if draw_param.is_null() {
            None
        } else {
            Some(input_string(draw_param)?)
        };
        document_mut(handle)?
            .set_page_layer_draw_param(page_index, input_string(layer_id)?, draw_param)
            .map_err(|error| error.to_string())
    })
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct DrawParamJson {
    relative: Option<String>,
    line_width: Option<f64>,
    cap: Option<String>,
    join: Option<String>,
    miter_limit: Option<f64>,
    dash_pattern: Option<[f64; 2]>,
    dash_sequence: Option<Vec<f64>>,
    dash_offset: Option<f64>,
    fill: Option<SolidPaintJson>,
    stroke: Option<SolidPaintJson>,
    fill_gradient: Option<serde_json::Value>,
    stroke_gradient: Option<serde_json::Value>,
    fill_gouraud: Option<serde_json::Value>,
    stroke_gouraud: Option<serde_json::Value>,
    fill_pattern: Option<serde_json::Value>,
    stroke_pattern: Option<serde_json::Value>,
}

fn parse_draw_param_style_json(source: &str) -> Result<DrawParamStyle, String> {
    let input: DrawParamJson = serde_json::from_str(source).map_err(|error| error.to_string())?;
    let cap = input
        .cap
        .map(|value| match value.to_ascii_lowercase().as_str() {
            "butt" => Ok(PathLineCap::Butt),
            "round" => Ok(PathLineCap::Round),
            "square" => Ok(PathLineCap::Square),
            _ => Err("DrawParam cap must be butt, round, or square".to_owned()),
        })
        .transpose()?;
    let join = input
        .join
        .map(|value| match value.to_ascii_lowercase().as_str() {
            "miter" => Ok(PathLineJoin::Miter),
            "round" => Ok(PathLineJoin::Round),
            "bevel" => Ok(PathLineJoin::Bevel),
            _ => Err("DrawParam join must be miter, round, or bevel".to_owned()),
        })
        .transpose()?;
    let paint = |value: SolidPaintJson| SolidPaintStyle {
        color: value.color,
        alpha: value.alpha.unwrap_or(255),
    };
    Ok(DrawParamStyle {
        relative: input.relative,
        line_width: input.line_width,
        cap,
        join,
        miter_limit: input.miter_limit,
        dash_pattern: input.dash_pattern,
        dash_sequence: input.dash_sequence,
        dash_offset: input.dash_offset,
        fill: input.fill.map(paint),
        stroke: input.stroke.map(paint),
        fill_gradient: input
            .fill_gradient
            .map(|value| parse_gradient_style_json(&value.to_string()))
            .transpose()?,
        stroke_gradient: input
            .stroke_gradient
            .map(|value| parse_gradient_style_json(&value.to_string()))
            .transpose()?,
        fill_gouraud: input
            .fill_gouraud
            .map(|value| parse_gouraud_style_json(&value.to_string()))
            .transpose()?,
        stroke_gouraud: input
            .stroke_gouraud
            .map(|value| parse_gouraud_style_json(&value.to_string()))
            .transpose()?,
        fill_pattern: input
            .fill_pattern
            .map(|value| parse_pattern_style_json(&value.to_string()))
            .transpose()?,
        stroke_pattern: input
            .stroke_pattern
            .map(|value| parse_pattern_style_json(&value.to_string()))
            .transpose()?,
    })
}

fn color_space_json(info: ColorSpaceInfo) -> serde_json::Value {
    serde_json::json!({
        "id": info.id, "color_type": info.color_type,
        "bits_per_component": info.bits_per_component,
        "profile": info.profile, "palette": info.palette,
        "resource_path": info.resource_path,
    })
}

/// List color spaces visible to a page as JSON.
///
/// # Safety
/// `handle` must be live and `output` writable.
#[no_mangle]
pub unsafe extern "C" fn ofd_page_color_spaces_json(
    handle: *const OfdDocument,
    page_index: usize,
    output: *mut OfdBuffer,
) -> i32 {
    run(|| {
        let values: Vec<_> = document_ref(handle)?
            .page_color_spaces(page_index)
            .map_err(|error| error.to_string())?
            .into_iter()
            .map(color_space_json)
            .collect();
        output_buffer(
            serde_json::to_vec(&values).map_err(|error| error.to_string())?,
            output,
        )
    })
}

/// List document-level color spaces as JSON.
///
/// # Safety
/// `handle` must be live and `output` writable.
#[no_mangle]
pub unsafe extern "C" fn ofd_document_color_spaces_json(
    handle: *const OfdDocument,
    document_index: usize,
    output: *mut OfdBuffer,
) -> i32 {
    run(|| {
        let values: Vec<_> = document_ref(handle)?
            .document_color_spaces(document_index)
            .map_err(|error| error.to_string())?
            .into_iter()
            .map(color_space_json)
            .collect();
        output_buffer(
            serde_json::to_vec(&values).map_err(|error| error.to_string())?,
            output,
        )
    })
}

fn draw_param_json(info: ofdrw::DrawParamInfo) -> serde_json::Value {
    serde_json::json!({
        "id": info.id,
        "relative": info.relative,
        "xml": info.xml,
        "resource_path": info.resource_path,
    })
}

/// List document-visible drawing parameters as JSON.
///
/// # Safety
/// `handle` must be live and `output` writable.
#[no_mangle]
pub unsafe extern "C" fn ofd_document_draw_params_json(
    handle: *const OfdDocument,
    document_index: usize,
    output: *mut OfdBuffer,
) -> i32 {
    run(|| {
        let values: Vec<_> = document_ref(handle)?
            .document_draw_params(document_index)
            .map_err(|error| error.to_string())?
            .into_iter()
            .map(draw_param_json)
            .collect();
        output_buffer(
            serde_json::to_vec(&values).map_err(|error| error.to_string())?,
            output,
        )
    })
}

/// List drawing parameters visible on a page as JSON.
///
/// # Safety
/// `handle` must be live and `output` writable.
#[no_mangle]
pub unsafe extern "C" fn ofd_page_draw_params_json(
    handle: *const OfdDocument,
    page_index: usize,
    output: *mut OfdBuffer,
) -> i32 {
    run(|| {
        let values: Vec<_> = document_ref(handle)?
            .page_draw_params(page_index)
            .map_err(|error| error.to_string())?
            .into_iter()
            .map(draw_param_json)
            .collect();
        output_buffer(
            serde_json::to_vec(&values).map_err(|error| error.to_string())?,
            output,
        )
    })
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct ColorSpaceInput {
    color_type: String,
    #[serde(default = "default_color_bits")]
    bits_per_component: u8,
    #[serde(default)]
    profile: Option<String>,
    #[serde(default)]
    palette: Vec<Vec<u16>>,
}

fn default_color_bits() -> u8 {
    8
}

fn parse_color_space_spec_json(source: &str) -> Result<ColorSpaceSpec, String> {
    let input: ColorSpaceInput = serde_json::from_str(source).map_err(|error| error.to_string())?;
    Ok(ColorSpaceSpec {
        color_type: input.color_type,
        bits_per_component: input.bits_per_component,
        profile: input.profile,
        palette: input.palette,
    })
}

/// Get the document's optional DefaultCS ID as JSON string or null.
///
/// # Safety
/// `handle` must be live and `output` writable.
#[no_mangle]
pub unsafe extern "C" fn ofd_document_default_color_space_json(
    handle: *const OfdDocument,
    document_index: usize,
    output: *mut OfdBuffer,
) -> i32 {
    run(|| {
        let value = document_ref(handle)?
            .document_default_color_space(document_index)
            .map_err(|error| error.to_string())?;
        output_buffer(
            serde_json::to_vec(&value).map_err(|error| error.to_string())?,
            output,
        )
    })
}

/// Set or clear the document's DefaultCS reference.
///
/// # Safety
/// `handle` must be live; `color_space_id` must be null or NUL-terminated.
#[no_mangle]
pub unsafe extern "C" fn ofd_set_document_default_color_space(
    handle: *mut OfdDocument,
    document_index: usize,
    color_space_id: *const c_char,
) -> i32 {
    run(|| {
        let id = if color_space_id.is_null() {
            None
        } else {
            Some(input_string(color_space_id)?)
        };
        document_mut(handle)?
            .set_document_default_color_space(document_index, id)
            .map_err(|error| error.to_string())
    })
}

/// Add a page-local color space from JSON and return its resource ID.
///
/// # Safety
/// `handle` must be live, `spec_json` NUL-terminated and `output_id` writable.
#[no_mangle]
pub unsafe extern "C" fn ofd_add_page_color_space_json(
    handle: *mut OfdDocument,
    page_index: usize,
    spec_json: *const c_char,
    output_id: *mut usize,
) -> i32 {
    run(|| {
        let output = output_id.as_mut().ok_or("null ColorSpace ID output")?;
        *output = 0;
        let spec = parse_color_space_spec_json(input_string(spec_json)?)?;
        let id = document_mut(handle)?
            .add_page_color_space(page_index, &spec)
            .map_err(|error| error.to_string())?;
        *output = id.parse().map_err(|_| "invalid generated ColorSpace ID")?;
        Ok(())
    })
}

/// Add a document color space from JSON and return its resource ID.
///
/// # Safety
/// `handle` must be live, `spec_json` NUL-terminated and `output_id` writable.
#[no_mangle]
pub unsafe extern "C" fn ofd_add_document_color_space_json(
    handle: *mut OfdDocument,
    document_index: usize,
    spec_json: *const c_char,
    output_id: *mut usize,
) -> i32 {
    run(|| {
        let output = output_id.as_mut().ok_or("null ColorSpace ID output")?;
        *output = 0;
        let spec = parse_color_space_spec_json(input_string(spec_json)?)?;
        let id = document_mut(handle)?
            .add_document_color_space(document_index, &spec)
            .map_err(|error| error.to_string())?;
        *output = id.parse().map_err(|_| "invalid generated ColorSpace ID")?;
        Ok(())
    })
}

/// Replace a document ColorSpace without changing its resource ID.
///
/// # Safety
/// `handle` must be live; `id` and `spec_json` must be NUL-terminated.
#[no_mangle]
pub unsafe extern "C" fn ofd_set_document_color_space_json(
    handle: *mut OfdDocument,
    document_index: usize,
    id: *const c_char,
    spec_json: *const c_char,
) -> i32 {
    run(|| {
        let spec = parse_color_space_spec_json(input_string(spec_json)?)?;
        document_mut(handle)?
            .set_document_color_space(document_index, input_string(id)?, &spec)
            .map_err(|error| error.to_string())
    })
}

/// Replace a page ColorSpace without changing its resource ID.
///
/// # Safety
/// `handle` must be live; `id` and `spec_json` must be NUL-terminated.
#[no_mangle]
pub unsafe extern "C" fn ofd_set_page_color_space_json(
    handle: *mut OfdDocument,
    page_index: usize,
    id: *const c_char,
    spec_json: *const c_char,
) -> i32 {
    run(|| {
        let spec = parse_color_space_spec_json(input_string(spec_json)?)?;
        document_mut(handle)?
            .set_page_color_space(page_index, input_string(id)?, &spec)
            .map_err(|error| error.to_string())
    })
}

/// Remove an unreferenced document ColorSpace definition.
///
/// # Safety
/// `handle` must be live and `id` must be NUL-terminated.
#[no_mangle]
pub unsafe extern "C" fn ofd_remove_document_color_space(
    handle: *mut OfdDocument,
    document_index: usize,
    id: *const c_char,
) -> i32 {
    run(|| {
        document_mut(handle)?
            .remove_document_color_space(document_index, input_string(id)?)
            .map_err(|error| error.to_string())
    })
}

/// Remove an unreferenced page ColorSpace definition.
///
/// # Safety
/// `handle` must be live and `id` must be NUL-terminated.
#[no_mangle]
pub unsafe extern "C" fn ofd_remove_page_color_space(
    handle: *mut OfdDocument,
    page_index: usize,
    id: *const c_char,
) -> i32 {
    run(|| {
        document_mut(handle)?
            .remove_page_color_space(page_index, input_string(id)?)
            .map_err(|error| error.to_string())
    })
}

/// Create a reusable document DrawParam from JSON and return its resource ID.
///
/// # Safety
/// `handle` must be live, `style_json` NUL-terminated, and `output_id` writable.
#[no_mangle]
pub unsafe extern "C" fn ofd_add_document_draw_param_json(
    handle: *mut OfdDocument,
    document_index: usize,
    style_json: *const c_char,
    output_id: *mut usize,
) -> i32 {
    run(|| {
        let output = output_id.as_mut().ok_or("null DrawParam ID output")?;
        *output = 0;
        let style = parse_draw_param_style_json(input_string(style_json)?)?;
        let id = document_mut(handle)?
            .add_document_draw_param(document_index, &style)
            .map_err(|error| error.to_string())?;
        *output = id.parse().map_err(|_| "invalid generated DrawParam ID")?;
        Ok(())
    })
}

/// Create a page-local DrawParam from JSON and return its resource ID.
///
/// # Safety
/// `handle` must be live, `style_json` NUL-terminated, and `output_id` writable.
#[no_mangle]
pub unsafe extern "C" fn ofd_add_page_draw_param_json(
    handle: *mut OfdDocument,
    page_index: usize,
    style_json: *const c_char,
    output_id: *mut usize,
) -> i32 {
    run(|| {
        let output = output_id.as_mut().ok_or("null DrawParam ID output")?;
        *output = 0;
        let style = parse_draw_param_style_json(input_string(style_json)?)?;
        let id = document_mut(handle)?
            .add_page_draw_param(page_index, &style)
            .map_err(|error| error.to_string())?;
        *output = id.parse().map_err(|_| "invalid generated DrawParam ID")?;
        Ok(())
    })
}

/// Replace a document resource DrawParam without changing its ID.
///
/// # Safety
/// `handle` must be live; `id` and `style_json` must be NUL-terminated.
#[no_mangle]
pub unsafe extern "C" fn ofd_set_document_draw_param_json(
    handle: *mut OfdDocument,
    document_index: usize,
    id: *const c_char,
    style_json: *const c_char,
) -> i32 {
    run(|| {
        let style = parse_draw_param_style_json(input_string(style_json)?)?;
        document_mut(handle)?
            .set_document_draw_param(document_index, input_string(id)?, &style)
            .map_err(|error| error.to_string())
    })
}

/// Replace a page resource DrawParam without changing its ID.
///
/// # Safety
/// `handle` must be live; `id` and `style_json` must be NUL-terminated.
#[no_mangle]
pub unsafe extern "C" fn ofd_set_page_draw_param_json(
    handle: *mut OfdDocument,
    page_index: usize,
    id: *const c_char,
    style_json: *const c_char,
) -> i32 {
    run(|| {
        let style = parse_draw_param_style_json(input_string(style_json)?)?;
        document_mut(handle)?
            .set_page_draw_param(page_index, input_string(id)?, &style)
            .map_err(|error| error.to_string())
    })
}

/// Remove an unreferenced document resource DrawParam.
///
/// # Safety
/// `handle` must be live and `id` NUL-terminated.
#[no_mangle]
pub unsafe extern "C" fn ofd_remove_document_draw_param(
    handle: *mut OfdDocument,
    document_index: usize,
    id: *const c_char,
) -> i32 {
    run(|| {
        document_mut(handle)?
            .remove_document_draw_param(document_index, input_string(id)?)
            .map_err(|error| error.to_string())
    })
}

/// Remove an unreferenced page resource DrawParam.
///
/// # Safety
/// `handle` must be live and `id` NUL-terminated.
#[no_mangle]
pub unsafe extern "C" fn ofd_remove_page_draw_param(
    handle: *mut OfdDocument,
    page_index: usize,
    id: *const c_char,
) -> i32 {
    run(|| {
        document_mut(handle)?
            .remove_page_draw_param(page_index, input_string(id)?)
            .map_err(|error| error.to_string())
    })
}

/// Set or clear a direct page object's DrawParam reference.
///
/// # Safety
/// `handle` must be live, `object_id` NUL-terminated, and `draw_param` null or NUL-terminated.
#[no_mangle]
pub unsafe extern "C" fn ofd_set_page_object_draw_param(
    handle: *mut OfdDocument,
    page_index: usize,
    object_id: *const c_char,
    draw_param: *const c_char,
) -> i32 {
    run(|| {
        let draw_param = if draw_param.is_null() {
            None
        } else {
            Some(input_string(draw_param)?)
        };
        document_mut(handle)?
            .set_page_object_draw_param(page_index, input_string(object_id)?, draw_param)
            .map_err(|error| error.to_string())
    })
}

/// Move one page graphic object into an existing layer on the same page.
///
/// # Safety
/// `handle` must be live and both IDs NUL-terminated.
#[no_mangle]
pub unsafe extern "C" fn ofd_move_page_object_to_layer(
    handle: *mut OfdDocument,
    page_index: usize,
    object_id: *const c_char,
    layer_id: *const c_char,
) -> i32 {
    run(|| {
        document_mut(handle)?
            .move_page_object_to_layer(
                page_index,
                input_string(object_id)?,
                input_string(layer_id)?,
            )
            .map_err(|error| error.to_string())
    })
}

/// Wrap inclusive, ordered sibling graphic objects in a PageBlock.
///
/// # Safety
/// `handle` must be live and both IDs NUL-terminated.
#[no_mangle]
pub unsafe extern "C" fn ofd_group_page_objects(
    handle: *mut OfdDocument,
    page_index: usize,
    first_id: *const c_char,
    last_id: *const c_char,
) -> i32 {
    run(|| {
        document_mut(handle)?
            .group_page_objects(page_index, input_string(first_id)?, input_string(last_id)?)
            .map_err(|error| error.to_string())
    })
}

/// Remove the immediate PageBlock containing the named graphic object.
///
/// # Safety
/// `handle` must be live and `object_id` NUL-terminated.
#[no_mangle]
pub unsafe extern "C" fn ofd_ungroup_page_objects(
    handle: *mut OfdDocument,
    page_index: usize,
    object_id: *const c_char,
) -> i32 {
    run(|| {
        document_mut(handle)?
            .ungroup_page_objects(page_index, input_string(object_id)?)
            .map_err(|error| error.to_string())
    })
}

/// Move a graphic object before another object, or to the end of its parent.
///
/// Pass a null `before_id` to append within the current layer or PageBlock.
/// A target in another container also moves the object into that container.
///
/// # Safety
/// `handle` must be live, `object_id` NUL-terminated, and a non-null
/// `before_id` NUL-terminated.
#[no_mangle]
pub unsafe extern "C" fn ofd_move_page_object(
    handle: *mut OfdDocument,
    page_index: usize,
    object_id: *const c_char,
    before_id: *const c_char,
) -> i32 {
    run(|| {
        let before = if before_id.is_null() {
            None
        } else {
            Some(input_string(before_id)?)
        };
        document_mut(handle)?
            .move_page_object(page_index, input_string(object_id)?, before)
            .map_err(|error| error.to_string())
    })
}

/// Set or clear a direct page object's optional name. Null clears it.
///
/// # Safety
/// `handle` must be live and `object_id` NUL-terminated. Non-null `name` must
/// be NUL-terminated.
#[no_mangle]
pub unsafe extern "C" fn ofd_set_page_object_name(
    handle: *mut OfdDocument,
    page_index: usize,
    object_id: *const c_char,
    name: *const c_char,
) -> i32 {
    run(|| {
        let name = if name.is_null() {
            None
        } else {
            Some(input_string(name)?)
        };
        document_mut(handle)?
            .set_page_object_name(page_index, input_string(object_id)?, name)
            .map_err(|error| error.to_string())
    })
}

/// Set or clear a direct page object's BlendMode. Null clears it.
///
/// # Safety
/// `handle` must be live; `object_id` and non-null `blend_mode` must be NUL-terminated.
#[no_mangle]
pub unsafe extern "C" fn ofd_set_page_object_blend_mode(
    handle: *mut OfdDocument,
    page_index: usize,
    object_id: *const c_char,
    blend_mode: *const c_char,
) -> i32 {
    run(|| {
        let blend_mode = if blend_mode.is_null() {
            None
        } else {
            Some(input_string(blend_mode)?)
        };
        document_mut(handle)?
            .set_page_object_blend_mode(page_index, input_string(object_id)?, blend_mode)
            .map_err(|error| error.to_string())
    })
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct PageObjectClipRectsJson {
    rects: Vec<[f64; 4]>,
    #[serde(default)]
    follows_ctm: bool,
}

/// Replace an object's clips with rectangular paths. An empty `rects` array clears them.
/// `clips_json` is `{ "rects": [[x, y, width, height]], "follows_ctm": false }`.
///
/// # Safety
/// `handle` must be live and strings NUL-terminated.
#[no_mangle]
pub unsafe extern "C" fn ofd_set_page_object_clip_rects_json(
    handle: *mut OfdDocument,
    page_index: usize,
    object_id: *const c_char,
    clips_json: *const c_char,
) -> i32 {
    run(|| {
        let request: PageObjectClipRectsJson =
            serde_json::from_str(input_string(clips_json)?).map_err(|error| error.to_string())?;
        document_mut(handle)?
            .set_page_object_clip_rects(
                page_index,
                input_string(object_id)?,
                &request.rects,
                request.follows_ctm,
            )
            .map_err(|error| error.to_string())
    })
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct PathClipAreaJson {
    boundary: [f64; 4],
    commands: String,
    #[serde(default)]
    area_ctm: Option<[f64; 6]>,
    #[serde(default)]
    path_ctm: Option<[f64; 6]>,
    #[serde(default)]
    even_odd: bool,
    #[serde(default)]
    fill: Option<bool>,
    #[serde(default)]
    stroke: Option<bool>,
    #[serde(default)]
    draw_param: Option<String>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct PageObjectPathClipsJson {
    clips: Vec<Vec<PathClipAreaJson>>,
    #[serde(default)]
    follows_ctm: bool,
}

/// Replace an object's clips with OFD path areas. An empty `clips` array clears them.
///
/// # Safety
/// `handle` must be live and strings NUL-terminated.
#[no_mangle]
pub unsafe extern "C" fn ofd_set_page_object_path_clips_json(
    handle: *mut OfdDocument,
    page_index: usize,
    object_id: *const c_char,
    clips_json: *const c_char,
) -> i32 {
    run(|| {
        let request: PageObjectPathClipsJson =
            serde_json::from_str(input_string(clips_json)?).map_err(|error| error.to_string())?;
        let clips = request
            .clips
            .into_iter()
            .map(|clip| {
                clip.into_iter()
                    .map(|area| PathClipArea {
                        boundary: area.boundary,
                        commands: area.commands,
                        area_ctm: area.area_ctm,
                        path_ctm: area.path_ctm,
                        even_odd: area.even_odd,
                        fill: area.fill,
                        stroke: area.stroke,
                        draw_param: area.draw_param,
                    })
                    .collect::<Vec<_>>()
            })
            .collect::<Vec<_>>();
        document_mut(handle)?
            .set_page_object_path_clips(
                page_index,
                input_string(object_id)?,
                &clips,
                request.follows_ctm,
            )
            .map_err(|error| error.to_string())
    })
}

#[derive(Deserialize)]
#[serde(tag = "kind", deny_unknown_fields)]
enum ClipAreaJson {
    Path {
        boundary: [f64; 4],
        commands: String,
        #[serde(default)]
        area_ctm: Option<[f64; 6]>,
        #[serde(default)]
        path_ctm: Option<[f64; 6]>,
        #[serde(default)]
        even_odd: bool,
        #[serde(default)]
        fill: Option<bool>,
        #[serde(default)]
        stroke: Option<bool>,
        #[serde(default)]
        draw_param: Option<String>,
    },
    Text {
        boundary: [f64; 4],
        font_id: String,
        size: f64,
        text: String,
        x: f64,
        y: f64,
        #[serde(default)]
        area_ctm: Option<[f64; 6]>,
        #[serde(default)]
        text_ctm: Option<[f64; 6]>,
        #[serde(default = "default_clip_h_scale")]
        h_scale: f64,
        #[serde(default)]
        char_direction: u16,
        #[serde(default)]
        fill: Option<bool>,
        #[serde(default)]
        stroke: Option<bool>,
        #[serde(default)]
        draw_param: Option<String>,
        #[serde(default)]
        text_codes: Vec<ClipTextCodeJson>,
        #[serde(default)]
        cg_transforms: Vec<ClipCgTransformJson>,
    },
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct ClipTextCodeJson {
    #[serde(default)]
    x: Option<f64>,
    #[serde(default)]
    y: Option<f64>,
    #[serde(default)]
    delta_x: Option<String>,
    #[serde(default)]
    delta_y: Option<String>,
    text: String,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct ClipCgTransformJson {
    code_position: usize,
    code_count: usize,
    glyph_count: usize,
    glyphs: Vec<u32>,
}

fn default_clip_h_scale() -> f64 {
    1.0
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct PageObjectClipsJson {
    clips: Vec<Vec<ClipAreaJson>>,
    #[serde(default)]
    follows_ctm: bool,
}

/// Replace an object's clips with path and text areas. Empty `clips` clears them.
///
/// # Safety
/// `handle` must be live and strings NUL-terminated.
#[no_mangle]
pub unsafe extern "C" fn ofd_set_page_object_clips_json(
    handle: *mut OfdDocument,
    page_index: usize,
    object_id: *const c_char,
    clips_json: *const c_char,
) -> i32 {
    run(|| {
        let request: PageObjectClipsJson =
            serde_json::from_str(input_string(clips_json)?).map_err(|error| error.to_string())?;
        let clips = request
            .clips
            .into_iter()
            .map(|clip| {
                clip.into_iter()
                    .map(|area| match area {
                        ClipAreaJson::Path {
                            boundary,
                            commands,
                            area_ctm,
                            path_ctm,
                            even_odd,
                            fill,
                            stroke,
                            draw_param,
                        } => ClipArea::Path(PathClipArea {
                            boundary,
                            commands,
                            area_ctm,
                            path_ctm,
                            even_odd,
                            fill,
                            stroke,
                            draw_param,
                        }),
                        ClipAreaJson::Text {
                            boundary,
                            font_id,
                            size,
                            text,
                            x,
                            y,
                            area_ctm,
                            text_ctm,
                            h_scale,
                            char_direction,
                            fill,
                            stroke,
                            draw_param,
                            text_codes,
                            cg_transforms,
                        } => ClipArea::Text(TextClipArea {
                            boundary,
                            font_id,
                            size,
                            text,
                            x,
                            y,
                            area_ctm,
                            text_ctm,
                            h_scale,
                            char_direction,
                            fill,
                            stroke,
                            draw_param,
                            text_codes: text_codes
                                .into_iter()
                                .map(|code| TextCodeInfo {
                                    x: code.x,
                                    y: code.y,
                                    delta_x: code.delta_x,
                                    delta_y: code.delta_y,
                                    text: code.text,
                                })
                                .collect(),
                            cg_transforms: cg_transforms
                                .into_iter()
                                .map(|mapping| CgTransformInfo {
                                    code_position: mapping.code_position,
                                    code_count: mapping.code_count,
                                    glyph_count: mapping.glyph_count,
                                    glyphs: mapping.glyphs,
                                })
                                .collect(),
                        }),
                    })
                    .collect::<Vec<_>>()
            })
            .collect::<Vec<_>>();
        document_mut(handle)?
            .set_page_object_clips(
                page_index,
                input_string(object_id)?,
                &clips,
                request.follows_ctm,
            )
            .map_err(|error| error.to_string())
    })
}

/// Set a page object's visibility (`visible` must be 0 or 1).
///
/// # Safety
/// `handle` must be live and `object_id` NUL-terminated.
#[no_mangle]
pub unsafe extern "C" fn ofd_set_page_object_visible(
    handle: *mut OfdDocument,
    page_index: usize,
    object_id: *const c_char,
    visible: i32,
) -> i32 {
    run(|| {
        let visible = match visible {
            0 => false,
            1 => true,
            _ => return Err("visible must be 0 or 1".into()),
        };
        document_mut(handle)?
            .set_page_object_visible(page_index, input_string(object_id)?, visible)
            .map_err(|error| error.to_string())
    })
}

/// Set a page object's whole-object alpha (0..255).
///
/// # Safety
/// `handle` must be live and `object_id` NUL-terminated.
#[no_mangle]
pub unsafe extern "C" fn ofd_set_page_object_alpha(
    handle: *mut OfdDocument,
    page_index: usize,
    object_id: *const c_char,
    alpha: i32,
) -> i32 {
    run(|| {
        let alpha = u8::try_from(alpha).map_err(|_| "alpha must be 0..255")?;
        document_mut(handle)?
            .set_page_object_alpha(page_index, input_string(object_id)?, alpha)
            .map_err(|error| error.to_string())
    })
}

fn parse_path_stroke_style(input: &str) -> Result<PathStrokeStyle, String> {
    #[derive(Deserialize)]
    #[serde(deny_unknown_fields)]
    struct Input {
        line_width: f64,
        cap: Option<String>,
        join: Option<String>,
        miter_limit: Option<f64>,
        dash_pattern: Option<[f64; 2]>,
        dash_sequence: Option<Vec<f64>>,
        dash_offset: Option<f64>,
    }
    let input: Input = serde_json::from_str(input).map_err(|error| error.to_string())?;
    let cap = input
        .cap
        .as_deref()
        .map(|value| match value.to_ascii_lowercase().as_str() {
            "butt" => Ok(PathLineCap::Butt),
            "round" => Ok(PathLineCap::Round),
            "square" => Ok(PathLineCap::Square),
            _ => Err("cap must be butt, round, or square"),
        })
        .transpose()?;
    let join = input
        .join
        .as_deref()
        .map(|value| match value.to_ascii_lowercase().as_str() {
            "miter" => Ok(PathLineJoin::Miter),
            "round" => Ok(PathLineJoin::Round),
            "bevel" => Ok(PathLineJoin::Bevel),
            _ => Err("join must be miter, round, or bevel"),
        })
        .transpose()?;
    Ok(PathStrokeStyle {
        line_width: input.line_width,
        cap,
        join,
        miter_limit: input.miter_limit,
        dash_pattern: input.dash_pattern,
        dash_sequence: input.dash_sequence,
        dash_offset: input.dash_offset,
    })
}

/// Atomically replace direct stroke geometry on a page PathObject.
///
/// JSON requires `line_width`; optional `cap`, `join`, `miter_limit`,
/// `dash_pattern`, `dash_sequence`, and `dash_offset` clear their direct attributes when omitted.
///
/// # Safety
/// `handle` must be live; `object_id` and `style_json` must be NUL-terminated.
#[no_mangle]
pub unsafe extern "C" fn ofd_set_page_path_stroke_style_json(
    handle: *mut OfdDocument,
    page_index: usize,
    object_id: *const c_char,
    style_json: *const c_char,
) -> i32 {
    run(|| {
        let style = parse_path_stroke_style(input_string(style_json)?)?;
        document_mut(handle)?
            .set_page_path_stroke_style(page_index, input_string(object_id)?, style)
            .map_err(|error| error.to_string())
    })
}

/// Replace a page PathObject's direct dash sequence and offset atomically.
/// JSON accepts `dash_sequence` (1..256 positive numbers) and optional
/// `dash_offset`; an omitted or null sequence clears both attributes.
///
/// # Safety
/// `handle` must be live; `object_id` and `style_json` must be NUL-terminated.
#[no_mangle]
pub unsafe extern "C" fn ofd_set_page_path_dash_sequence_json(
    handle: *mut OfdDocument,
    page_index: usize,
    object_id: *const c_char,
    style_json: *const c_char,
) -> i32 {
    #[derive(Deserialize)]
    #[serde(deny_unknown_fields)]
    struct Input {
        dash_sequence: Option<Vec<f64>>,
        dash_offset: Option<f64>,
    }
    run(|| {
        let input: Input =
            serde_json::from_str(input_string(style_json)?).map_err(|error| error.to_string())?;
        document_mut(handle)?
            .set_page_path_dash_sequence(
                page_index,
                input_string(object_id)?,
                input.dash_sequence.as_deref(),
                input.dash_offset,
            )
            .map_err(|error| error.to_string())
    })
}

/// Replace a direct page PathObject's abbreviated drawing commands.
///
/// # Safety
/// `handle` must be live; `object_id` and `commands` must be NUL-terminated.
#[no_mangle]
pub unsafe extern "C" fn ofd_set_page_path_data(
    handle: *mut OfdDocument,
    page_index: usize,
    object_id: *const c_char,
    commands: *const c_char,
) -> i32 {
    run(|| {
        document_mut(handle)?
            .set_page_path_data(
                page_index,
                input_string(object_id)?,
                input_string(commands)?,
            )
            .map_err(|error| error.to_string())
    })
}

/// Atomically replace direct typography on a page TextObject.
///
/// JSON requires `size`; omitted `h_scale`, `read_direction`,
/// `char_direction`, `weight`, and `italic` clear their direct attributes.
///
/// # Safety
/// `handle` must be live; `object_id` and `style_json` must be NUL-terminated.
#[no_mangle]
pub unsafe extern "C" fn ofd_set_page_text_typography_json(
    handle: *mut OfdDocument,
    page_index: usize,
    object_id: *const c_char,
    style_json: *const c_char,
) -> i32 {
    #[derive(Deserialize)]
    #[serde(deny_unknown_fields)]
    struct Input {
        size: f64,
        h_scale: Option<f64>,
        read_direction: Option<u16>,
        char_direction: Option<u16>,
        weight: Option<u16>,
        italic: Option<bool>,
    }
    run(|| {
        let input: Input =
            serde_json::from_str(input_string(style_json)?).map_err(|error| error.to_string())?;
        document_mut(handle)?
            .set_page_text_typography(
                page_index,
                input_string(object_id)?,
                TextTypographyStyle {
                    size: input.size,
                    h_scale: input.h_scale,
                    read_direction: input.read_direction,
                    char_direction: input.char_direction,
                    weight: input.weight,
                    italic: input.italic,
                },
            )
            .map_err(|error| error.to_string())
    })
}

/// Replace one `TextCode` run on an unshaped page TextObject.
///
/// # Safety
/// `handle` must be live; `object_id` and `text` must be NUL-terminated.
#[no_mangle]
pub unsafe extern "C" fn ofd_set_page_text_code(
    handle: *mut OfdDocument,
    page_index: usize,
    object_id: *const c_char,
    code_index: usize,
    text: *const c_char,
) -> i32 {
    run(|| {
        document_mut(handle)?
            .set_page_text_code(
                page_index,
                input_string(object_id)?,
                code_index,
                input_string(text)?,
            )
            .map_err(|error| error.to_string())
    })
}

/// Replace one direct TextCode run's positioning attributes from JSON.
///
/// Optional fields are `x`, `y`, `delta_x`, and `delta_y`. Missing fields
/// remove the corresponding attributes; the first run must retain X and Y.
///
/// # Safety
/// `handle` must be live; `object_id` and `layout_json` must be NUL-terminated.
#[no_mangle]
pub unsafe extern "C" fn ofd_set_page_text_code_layout_json(
    handle: *mut OfdDocument,
    page_index: usize,
    object_id: *const c_char,
    code_index: usize,
    layout_json: *const c_char,
) -> i32 {
    #[derive(Deserialize)]
    #[serde(deny_unknown_fields)]
    struct Input {
        x: Option<f64>,
        y: Option<f64>,
        delta_x: Option<Vec<f64>>,
        delta_y: Option<Vec<f64>>,
    }
    run(|| {
        let input: Input =
            serde_json::from_str(input_string(layout_json)?).map_err(|error| error.to_string())?;
        document_mut(handle)?
            .set_page_text_code_layout(
                page_index,
                input_string(object_id)?,
                code_index,
                &TextCodeLayout {
                    x: input.x,
                    y: input.y,
                    delta_x: input.delta_x,
                    delta_y: input.delta_y,
                },
            )
            .map_err(|error| error.to_string())
    })
}

/// Append one positioned TextCode run to a direct, unshaped text object.
///
/// `layout_json` accepts optional `x`, `y`, `delta_x`, and `delta_y` fields.
///
/// # Safety
/// `handle` must be live; strings must be NUL-terminated.
#[no_mangle]
pub unsafe extern "C" fn ofd_append_page_text_code_json(
    handle: *mut OfdDocument,
    page_index: usize,
    object_id: *const c_char,
    text: *const c_char,
    layout_json: *const c_char,
) -> i32 {
    #[derive(Deserialize)]
    #[serde(deny_unknown_fields)]
    struct Input {
        x: Option<f64>,
        y: Option<f64>,
        delta_x: Option<Vec<f64>>,
        delta_y: Option<Vec<f64>>,
    }
    run(|| {
        let input: Input =
            serde_json::from_str(input_string(layout_json)?).map_err(|error| error.to_string())?;
        document_mut(handle)?
            .append_page_text_code(
                page_index,
                input_string(object_id)?,
                input_string(text)?,
                &TextCodeLayout {
                    x: input.x,
                    y: input.y,
                    delta_x: input.delta_x,
                    delta_y: input.delta_y,
                },
            )
            .map_err(|error| error.to_string())
    })
}

/// Set or append a `TextCode` run from JSON without C string character limits.
///
/// `set` requires `page_index`, `object_id`, `code_index`, and `text`.
/// `append` requires `page_index`, `object_id`, `text`, and `layout` with
/// optional `x`, `y`, `delta_x`, and `delta_y` fields.
///
/// # Safety
/// `handle` must be live and `request_json` NUL-terminated.
#[no_mangle]
pub unsafe extern "C" fn ofd_edit_text_code_json(
    handle: *mut OfdDocument,
    request_json: *const c_char,
) -> i32 {
    #[derive(Deserialize)]
    #[serde(deny_unknown_fields)]
    struct Layout {
        x: Option<f64>,
        y: Option<f64>,
        delta_x: Option<Vec<f64>>,
        delta_y: Option<Vec<f64>>,
    }
    #[derive(Deserialize)]
    #[serde(tag = "operation", rename_all = "snake_case", deny_unknown_fields)]
    enum Input {
        Set {
            page_index: usize,
            object_id: String,
            code_index: usize,
            text: String,
        },
        Append {
            page_index: usize,
            object_id: String,
            text: String,
            layout: Layout,
        },
    }
    run(|| {
        let input: Input =
            serde_json::from_str(input_string(request_json)?).map_err(|e| e.to_string())?;
        let document = document_mut(handle)?;
        match input {
            Input::Set {
                page_index,
                object_id,
                code_index,
                text,
            } => document
                .set_page_text_code(page_index, &object_id, code_index, &text)
                .map_err(|e| e.to_string()),
            Input::Append {
                page_index,
                object_id,
                text,
                layout,
            } => document
                .append_page_text_code(
                    page_index,
                    &object_id,
                    &text,
                    &TextCodeLayout {
                        x: layout.x,
                        y: layout.y,
                        delta_x: layout.delta_x,
                        delta_y: layout.delta_y,
                    },
                )
                .map_err(|e| e.to_string()),
        }
    })
}

/// Remove one unshaped TextCode run, preserving following coordinates.
///
/// # Safety
/// `handle` must be live and `object_id` must be NUL-terminated.
#[no_mangle]
pub unsafe extern "C" fn ofd_remove_page_text_code(
    handle: *mut OfdDocument,
    page_index: usize,
    object_id: *const c_char,
    code_index: usize,
) -> i32 {
    run(|| {
        document_mut(handle)?
            .remove_page_text_code(page_index, input_string(object_id)?, code_index)
            .map_err(|error| error.to_string())
    })
}

/// Set a direct page TextObject's font resource ID.
///
/// # Safety
/// `handle` must be live; `object_id` and `font_id` must be NUL-terminated.
#[no_mangle]
pub unsafe extern "C" fn ofd_set_page_text_font(
    handle: *mut OfdDocument,
    page_index: usize,
    object_id: *const c_char,
    font_id: *const c_char,
) -> i32 {
    run(|| {
        document_mut(handle)?
            .set_page_text_font(page_index, input_string(object_id)?, input_string(font_id)?)
            .map_err(|error| error.to_string())
    })
}

/// Set direct text fill and stroke switches; missing JSON fields restore defaults.
///
/// # Safety
/// `handle` must be live; `object_id` and `flags_json` must be NUL-terminated.
#[no_mangle]
pub unsafe extern "C" fn ofd_set_page_text_paint_flags_json(
    handle: *mut OfdDocument,
    page_index: usize,
    object_id: *const c_char,
    flags_json: *const c_char,
) -> i32 {
    #[derive(Deserialize)]
    #[serde(deny_unknown_fields)]
    struct Input {
        fill: Option<bool>,
        stroke: Option<bool>,
    }
    run(|| {
        let input: Input =
            serde_json::from_str(input_string(flags_json)?).map_err(|error| error.to_string())?;
        document_mut(handle)?
            .set_page_text_paint_flags(
                page_index,
                input_string(object_id)?,
                input.fill,
                input.stroke,
            )
            .map_err(|error| error.to_string())
    })
}

/// Set or clear a direct page image object's `ImageMask` resource reference.
///
/// # Safety
/// `handle` must be live, `object_id` NUL-terminated, and `mask_resource_id`
/// either null or NUL-terminated.
#[no_mangle]
pub unsafe extern "C" fn ofd_set_page_image_mask(
    handle: *mut OfdDocument,
    page_index: usize,
    object_id: *const c_char,
    mask_resource_id: *const c_char,
) -> i32 {
    run(|| {
        let mask_id = if mask_resource_id.is_null() {
            None
        } else {
            Some(input_string(mask_resource_id)?)
        };
        document_mut(handle)?
            .set_page_image_mask(page_index, input_string(object_id)?, mask_id)
            .map_err(|error| error.to_string())
    })
}

/// Set or clear a direct page image's alternative multimedia resource.
///
/// # Safety
/// `handle` must be live, `object_id` NUL-terminated, and
/// `substitution_resource_id` either null or NUL-terminated.
#[no_mangle]
pub unsafe extern "C" fn ofd_set_page_image_substitution(
    handle: *mut OfdDocument,
    page_index: usize,
    object_id: *const c_char,
    substitution_resource_id: *const c_char,
) -> i32 {
    run(|| {
        let substitution_id = if substitution_resource_id.is_null() {
            None
        } else {
            Some(input_string(substitution_resource_id)?)
        };
        document_mut(handle)?
            .set_page_image_substitution(page_index, input_string(object_id)?, substitution_id)
            .map_err(|error| error.to_string())
    })
}

/// Replace an image border using a JSON style, or remove it with JSON `null`.
/// The optional style fields are `line_width`, `horizontal_corner_radius`,
/// `vertical_corner_radius`, `dash_pattern`, `dash_offset`, `color`, and
/// `color_alpha`.
///
/// # Safety
/// `handle` must be live; `object_id` and `style_json` must be NUL-terminated.
#[no_mangle]
pub unsafe extern "C" fn ofd_set_page_image_border_json(
    handle: *mut OfdDocument,
    page_index: usize,
    object_id: *const c_char,
    style_json: *const c_char,
) -> i32 {
    run(|| {
        let input: serde_json::Value =
            serde_json::from_str(input_string(style_json)?).map_err(|error| error.to_string())?;
        let style = if input.is_null() {
            None
        } else {
            let object = input
                .as_object()
                .ok_or("image border style must be an object or null")?;
            let mut style = ImageBorderStyle::default();
            for (name, target) in [
                ("line_width", &mut style.line_width),
                (
                    "horizontal_corner_radius",
                    &mut style.horizontal_corner_radius,
                ),
                ("vertical_corner_radius", &mut style.vertical_corner_radius),
                ("dash_offset", &mut style.dash_offset),
            ] {
                if let Some(value) = object.get(name) {
                    *target = value
                        .as_f64()
                        .ok_or_else(|| format!("{name} must be a number"))?;
                }
            }
            if let Some(value) = object.get("dash_pattern") {
                let values = value.as_array().ok_or("dash_pattern must be an array")?;
                style.dash_pattern = values
                    .iter()
                    .map(|item| {
                        item.as_f64()
                            .ok_or_else(|| "dash_pattern must contain numbers".to_owned())
                    })
                    .collect::<Result<_, _>>()?;
            }
            if let Some(value) = object.get("color") {
                let values = value.as_array().ok_or("color must be an RGB array")?;
                let channels = values
                    .iter()
                    .map(|item| {
                        item.as_u64()
                            .and_then(|value| u8::try_from(value).ok())
                            .ok_or_else(|| "color channels must be 0..255 integers".to_owned())
                    })
                    .collect::<Result<Vec<_>, _>>()?;
                style.color = channels
                    .try_into()
                    .map_err(|_| "color must contain three channels")?;
            }
            if let Some(value) = object.get("color_alpha") {
                style.color_alpha = value
                    .as_u64()
                    .and_then(|value| u8::try_from(value).ok())
                    .ok_or("color_alpha must be a 0..255 integer")?;
            }
            Some(style)
        };
        document_mut(handle)?
            .set_page_image_border(page_index, input_string(object_id)?, style.as_ref())
            .map_err(|error| error.to_string())
    })
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct GouraudPointJson {
    x: f64,
    y: f64,
    color: [u8; 3],
    alpha: Option<u8>,
    #[serde(default)]
    edge_flag: u8,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct GouraudFillJson {
    points: Vec<GouraudPointJson>,
    vertices_per_row: Option<usize>,
    #[serde(default)]
    extend: bool,
    back_color: Option<[u8; 3]>,
    back_alpha: Option<u8>,
}

fn parse_gouraud_style_json(source: &str) -> Result<GouraudFillStyle, String> {
    let input: GouraudFillJson = serde_json::from_str(source).map_err(|error| error.to_string())?;
    Ok(GouraudFillStyle {
        points: input
            .points
            .into_iter()
            .map(|point| GouraudControlPoint {
                x: point.x,
                y: point.y,
                color: point.color,
                alpha: point.alpha,
                edge_flag: point.edge_flag,
            })
            .collect(),
        vertices_per_row: input.vertices_per_row,
        extend: input.extend,
        back_color: input.back_color,
        back_alpha: input.back_alpha,
    })
}

/// Replace a direct page path or text object's fill with Gouraud shading.
/// Coordinates are in object-local millimeters; JSON fields match
/// `GouraudFillJson` and its point records.
///
/// # Safety
/// `handle` must be live; `object_id` and `style_json` must be NUL-terminated.
#[no_mangle]
pub unsafe extern "C" fn ofd_set_page_object_gouraud_fill_json(
    handle: *mut OfdDocument,
    page_index: usize,
    object_id: *const c_char,
    style_json: *const c_char,
) -> i32 {
    run(|| {
        let style = parse_gouraud_style_json(input_string(style_json)?)?;
        document_mut(handle)?
            .set_page_object_gouraud_fill(page_index, input_string(object_id)?, &style)
            .map_err(|error| error.to_string())
    })
}

/// Replace a direct page path or text stroke with Gouraud shading.
///
/// # Safety
/// `handle` must be live; `object_id` and `style_json` must be NUL-terminated.
#[no_mangle]
pub unsafe extern "C" fn ofd_set_page_object_gouraud_stroke_json(
    handle: *mut OfdDocument,
    page_index: usize,
    object_id: *const c_char,
    style_json: *const c_char,
) -> i32 {
    run(|| {
        let style = parse_gouraud_style_json(input_string(style_json)?)?;
        document_mut(handle)?
            .set_page_object_gouraud_stroke(page_index, input_string(object_id)?, &style)
            .map_err(|error| error.to_string())
    })
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct GradientSegmentJson {
    position: Option<f64>,
    color: [u8; 3],
    alpha: Option<u8>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct GradientFillJson {
    kind: String,
    start_point: [f64; 2],
    end_point: [f64; 2],
    segments: Vec<GradientSegmentJson>,
    map_type: Option<String>,
    map_unit: Option<f64>,
    extend: Option<u8>,
    fill_alpha: Option<u8>,
    start_radius: Option<f64>,
    end_radius: Option<f64>,
    eccentricity: Option<f64>,
    angle: Option<f64>,
}

fn parse_gradient_style_json(source: &str) -> Result<GradientFillStyle, String> {
    let input: GradientFillJson =
        serde_json::from_str(source).map_err(|error| error.to_string())?;
    let kind = match input.kind.as_str() {
        "axial" => {
            if input.start_radius.is_some()
                || input.end_radius.is_some()
                || input.eccentricity.is_some()
                || input.angle.is_some()
            {
                return Err("axial gradient cannot have radial geometry".into());
            }
            GradientKind::Axial
        }
        "radial" => GradientKind::Radial {
            start_radius: input.start_radius.unwrap_or(0.0),
            end_radius: input
                .end_radius
                .ok_or("radial gradient requires end_radius")?,
            eccentricity: input.eccentricity.unwrap_or(0.0),
            angle: input.angle.unwrap_or(0.0),
        },
        _ => return Err("gradient kind must be axial or radial".into()),
    };
    let map_type = match input.map_type.as_deref().unwrap_or("direct") {
        "direct" => GradientMapType::Direct,
        "repeat" => GradientMapType::Repeat,
        "reflect" => GradientMapType::Reflect,
        _ => return Err("map_type must be direct, repeat, or reflect".into()),
    };
    Ok(GradientFillStyle {
        kind,
        start_point: input.start_point,
        end_point: input.end_point,
        segments: input
            .segments
            .into_iter()
            .map(|segment| GradientSegment {
                position: segment.position,
                color: segment.color,
                alpha: segment.alpha,
            })
            .collect(),
        map_type,
        map_unit: input.map_unit,
        extend: input.extend.unwrap_or(3),
        fill_alpha: input.fill_alpha,
    })
}

/// Replace a direct page path or text fill with axial or radial shading.
/// `style_json` includes `kind`, `start_point`, `end_point`, and `segments`.
/// Radial shading also requires `end_radius`.
///
/// # Safety
/// `handle` must be live; `object_id` and `style_json` must be NUL-terminated.
#[no_mangle]
pub unsafe extern "C" fn ofd_set_page_object_gradient_fill_json(
    handle: *mut OfdDocument,
    page_index: usize,
    object_id: *const c_char,
    style_json: *const c_char,
) -> i32 {
    run(|| {
        let style = parse_gradient_style_json(input_string(style_json)?)?;
        document_mut(handle)?
            .set_page_object_gradient_fill(page_index, input_string(object_id)?, &style)
            .map_err(|error| error.to_string())
    })
}

/// Replace a direct page path or text stroke with axial or radial shading.
///
/// # Safety
/// `handle` must be live; `object_id` and `style_json` must be NUL-terminated.
#[no_mangle]
pub unsafe extern "C" fn ofd_set_page_object_gradient_stroke_json(
    handle: *mut OfdDocument,
    page_index: usize,
    object_id: *const c_char,
    style_json: *const c_char,
) -> i32 {
    run(|| {
        let style = parse_gradient_style_json(input_string(style_json)?)?;
        document_mut(handle)?
            .set_page_object_gradient_stroke(page_index, input_string(object_id)?, &style)
            .map_err(|error| error.to_string())
    })
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct PatternStyleJson {
    width: f64,
    height: f64,
    x_step: Option<f64>,
    y_step: Option<f64>,
    reflect: Option<String>,
    relative_to: Option<String>,
    ctm: Option<[f64; 6]>,
    alpha: Option<u8>,
    thumbnail: Option<String>,
    #[serde(default)]
    cells: Vec<serde_json::Value>,
}

fn parse_pattern_style_json(source: &str) -> Result<PatternStyle, String> {
    let input: PatternStyleJson =
        serde_json::from_str(source).map_err(|error| error.to_string())?;
    let reflect = match input.reflect.as_deref().unwrap_or("normal") {
        "normal" => PatternReflectMethod::Normal,
        "row" => PatternReflectMethod::Row,
        "column" => PatternReflectMethod::Column,
        "row_and_column" => PatternReflectMethod::RowAndColumn,
        _ => return Err("reflect must be normal, row, column, or row_and_column".into()),
    };
    let relative_to = match input.relative_to.as_deref().unwrap_or("object") {
        "object" => PatternRelativeTo::Object,
        "page" => PatternRelativeTo::Page,
        _ => return Err("relative_to must be object or page".into()),
    };
    let mut cells = Vec::with_capacity(input.cells.len());
    for cell in &input.cells {
        let kind = cell
            .get("kind")
            .and_then(|value| value.as_str())
            .ok_or("Pattern cell requires kind")?;
        let boundary = || -> Result<[f64; 4], String> {
            let value = cell
                .get("boundary")
                .ok_or("Pattern cell requires boundary")?;
            serde_json::from_value(value.clone())
                .map_err(|error| format!("invalid Pattern cell boundary: {error}"))
        };
        cells.push(match kind {
            "path" => {
                let (commands, boundary, style) = path_object_arguments(cell)?;
                PatternCell::Path {
                    commands: commands.to_owned(),
                    boundary,
                    style,
                }
            }
            "text" => PatternCell::Text {
                text: cell
                    .get("text")
                    .and_then(|value| value.as_str())
                    .ok_or("Pattern text cell requires text")?
                    .to_owned(),
                boundary: boundary()?,
                size: cell
                    .get("size")
                    .and_then(|value| value.as_f64())
                    .ok_or("Pattern text cell requires size")?,
                font_id: cell
                    .get("font_id")
                    .map(|value| {
                        value
                            .as_str()
                            .ok_or("Pattern font_id must be a string")
                            .map(str::to_owned)
                    })
                    .transpose()?,
            },
            "image" => PatternCell::Image {
                resource_id: cell
                    .get("resource_id")
                    .and_then(|value| value.as_str())
                    .ok_or("Pattern image cell requires resource_id")?
                    .to_owned(),
                boundary: boundary()?,
            },
            "composite" => PatternCell::Composite {
                resource_id: cell
                    .get("resource_id")
                    .and_then(|value| value.as_str())
                    .ok_or("Pattern composite cell requires resource_id")?
                    .to_owned(),
                boundary: boundary()?,
            },
            _ => return Err("Pattern cell kind must be path, text, image, or composite".into()),
        });
    }
    Ok(PatternStyle {
        width: input.width,
        height: input.height,
        x_step: input.x_step,
        y_step: input.y_step,
        reflect,
        relative_to,
        ctm: input.ctm,
        alpha: input.alpha,
        thumbnail: input.thumbnail,
        cells,
    })
}

/// Replace a direct path or text fill with a repeated pattern.
///
/// # Safety
/// `handle` must be live; `object_id` and `style_json` must be NUL-terminated.
#[no_mangle]
pub unsafe extern "C" fn ofd_set_page_object_pattern_fill_json(
    handle: *mut OfdDocument,
    page_index: usize,
    object_id: *const c_char,
    style_json: *const c_char,
) -> i32 {
    run(|| {
        let style = parse_pattern_style_json(input_string(style_json)?)?;
        document_mut(handle)?
            .set_page_object_pattern_fill(page_index, input_string(object_id)?, &style)
            .map_err(|error| error.to_string())
    })
}

/// Replace a direct path or text stroke with a repeated pattern.
///
/// # Safety
/// `handle` must be live; `object_id` and `style_json` must be NUL-terminated.
#[no_mangle]
pub unsafe extern "C" fn ofd_set_page_object_pattern_stroke_json(
    handle: *mut OfdDocument,
    page_index: usize,
    object_id: *const c_char,
    style_json: *const c_char,
) -> i32 {
    run(|| {
        let style = parse_pattern_style_json(input_string(style_json)?)?;
        document_mut(handle)?
            .set_page_object_pattern_stroke(page_index, input_string(object_id)?, &style)
            .map_err(|error| error.to_string())
    })
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct SolidPaintJson {
    color: [u8; 3],
    alpha: Option<u8>,
}

fn parse_solid_paint_json(source: &str) -> Result<SolidPaintStyle, String> {
    let input: SolidPaintJson = serde_json::from_str(source).map_err(|error| error.to_string())?;
    Ok(SolidPaintStyle {
        color: input.color,
        alpha: input.alpha.unwrap_or(255),
    })
}

/// Replace a direct page path or text fill with an RGB color.
///
/// # Safety
/// `handle` must be live; `object_id` and `style_json` must be NUL-terminated.
#[no_mangle]
pub unsafe extern "C" fn ofd_set_page_object_solid_fill_json(
    handle: *mut OfdDocument,
    page_index: usize,
    object_id: *const c_char,
    style_json: *const c_char,
) -> i32 {
    run(|| {
        let style = parse_solid_paint_json(input_string(style_json)?)?;
        document_mut(handle)?
            .set_page_object_solid_fill(page_index, input_string(object_id)?, style)
            .map_err(|error| error.to_string())
    })
}

/// Replace a direct page path or text stroke with an RGB color.
///
/// # Safety
/// `handle` must be live; `object_id` and `style_json` must be NUL-terminated.
#[no_mangle]
pub unsafe extern "C" fn ofd_set_page_object_solid_stroke_json(
    handle: *mut OfdDocument,
    page_index: usize,
    object_id: *const c_char,
    style_json: *const c_char,
) -> i32 {
    run(|| {
        let style = parse_solid_paint_json(input_string(style_json)?)?;
        document_mut(handle)?
            .set_page_object_solid_stroke(page_index, input_string(object_id)?, style)
            .map_err(|error| error.to_string())
    })
}

/// Set a page object's boundary in millimeters.
///
/// # Safety
/// `handle` must be live and `object_id` NUL-terminated.
#[no_mangle]
pub unsafe extern "C" fn ofd_set_page_object_boundary(
    handle: *mut OfdDocument,
    page_index: usize,
    object_id: *const c_char,
    x: f64,
    y: f64,
    width: f64,
    height: f64,
) -> i32 {
    run(|| {
        document_mut(handle)?
            .set_page_object_boundary(page_index, input_string(object_id)?, [x, y, width, height])
            .map_err(|error| error.to_string())
    })
}

/// Set a page object's CTM from a JSON array of six numbers, or clear it with null.
///
/// # Safety
/// `handle` must be live, `object_id` NUL-terminated, and a non-null
/// `ctm_json` NUL-terminated.
#[no_mangle]
pub unsafe extern "C" fn ofd_set_page_object_ctm_json(
    handle: *mut OfdDocument,
    page_index: usize,
    object_id: *const c_char,
    ctm_json: *const c_char,
) -> i32 {
    run(|| {
        let ctm = if ctm_json.is_null() {
            None
        } else {
            Some(
                serde_json::from_str::<[f64; 6]>(input_string(ctm_json)?)
                    .map_err(|error| error.to_string())?,
            )
        };
        document_mut(handle)?
            .set_page_object_ctm(page_index, input_string(object_id)?, ctm)
            .map_err(|error| error.to_string())
    })
}

/// Remove an empty page layer by ID.
///
/// # Safety
/// `handle` must be live and `layer_id` NUL-terminated.
#[no_mangle]
pub unsafe extern "C" fn ofd_remove_page_layer(
    handle: *mut OfdDocument,
    page_index: usize,
    layer_id: *const c_char,
) -> i32 {
    run(|| {
        document_mut(handle)?
            .remove_page_layer(page_index, input_string(layer_id)?)
            .map_err(|error| error.to_string())
    })
}

/// Move a page to another position in its document body.
///
/// # Safety
/// `handle` must point to a live document.
#[no_mangle]
pub unsafe extern "C" fn ofd_move_page(
    handle: *mut OfdDocument,
    from_index: usize,
    to_index: usize,
) -> i32 {
    run(|| {
        document_mut(handle)?
            .move_page(from_index, to_index)
            .map_err(|error| error.to_string())
    })
}

/// Reorder all pages in one document body from a JSON array of local indexes.
///
/// # Safety
/// `handle` must be live and `order_json` NUL-terminated.
#[no_mangle]
pub unsafe extern "C" fn ofd_reorder_document_pages_json(
    handle: *mut OfdDocument,
    document_index: usize,
    order_json: *const c_char,
) -> i32 {
    run(|| {
        let order: Vec<usize> =
            serde_json::from_str(input_string(order_json)?).map_err(|error| error.to_string())?;
        document_mut(handle)?
            .reorder_document_pages(document_index, &order)
            .map_err(|error| error.to_string())
    })
}

/// Append a copy of one page to the same document body and return its index.
///
/// # Safety
/// `handle` must point to a live document and `output` must be writable.
#[no_mangle]
pub unsafe extern "C" fn ofd_duplicate_page(
    handle: *mut OfdDocument,
    index: usize,
    output: *mut usize,
) -> i32 {
    run(|| {
        if output.is_null() {
            return Err("null output pointer".into());
        }
        let duplicated = document_mut(handle)?
            .duplicate_page(index)
            .map_err(|error| error.to_string())?;
        unsafe { *output = duplicated };
        Ok(())
    })
}

/// Copy a page into a body-local position and return its global index.
///
/// # Safety
/// `handle` must be live and `output` writable.
#[no_mangle]
pub unsafe extern "C" fn ofd_duplicate_page_at(
    handle: *mut OfdDocument,
    index: usize,
    position: usize,
    output: *mut usize,
) -> i32 {
    run(|| {
        if output.is_null() {
            return Err("null output pointer".into());
        }
        let copied = document_mut(handle)?
            .duplicate_page_at(index, position)
            .map_err(|error| error.to_string())?;
        unsafe { *output = copied };
        Ok(())
    })
}

/// Remove a page from the document order. Referenced pages are rejected.
///
/// # Safety
/// `handle` must point to a live document.
#[no_mangle]
pub unsafe extern "C" fn ofd_remove_page(handle: *mut OfdDocument, index: usize) -> i32 {
    run(|| {
        document_mut(handle)?
            .remove_page(index)
            .map_err(|error| error.to_string())
    })
}

fn path_object_arguments(input: &serde_json::Value) -> Result<(&str, [f64; 4], PathStyle), String> {
    let commands = input
        .get("commands")
        .and_then(serde_json::Value::as_str)
        .ok_or("path object commands must be a string")?;
    let values = input
        .get("boundary")
        .and_then(serde_json::Value::as_array)
        .ok_or("path object boundary must be an array")?;
    let boundary: [f64; 4] = values
        .iter()
        .map(|value| {
            value
                .as_f64()
                .ok_or("path object boundary must contain numbers")
        })
        .collect::<Result<Vec<_>, _>>()?
        .try_into()
        .map_err(|_| "path object boundary must contain four numbers")?;
    let mut style = PathStyle::default();
    if let Some(value) = input.get("fill") {
        style.fill = value
            .as_bool()
            .ok_or("path object fill must be a boolean")?;
    }
    if let Some(value) = input.get("even_odd") {
        style.even_odd = value
            .as_bool()
            .ok_or("path object even_odd must be a boolean")?;
    }
    if let Some(value) = input.get("alpha") {
        style.alpha = Some(
            value
                .as_u64()
                .and_then(|alpha| u8::try_from(alpha).ok())
                .ok_or("path object alpha must be a byte")?,
        );
    }
    if let Some(value) = input.get("stroke") {
        style.stroke = value
            .as_bool()
            .ok_or("path object stroke must be a boolean")?;
    }
    if let Some(value) = input.get("line_width") {
        style.line_width = value
            .as_f64()
            .ok_or("path object line_width must be a number")?;
    }
    if let Some(value) = input.get("cap") {
        let name = value.as_str().ok_or("path object cap must be a string")?;
        style.cap = Some(match name.to_ascii_lowercase().as_str() {
            "butt" => PathLineCap::Butt,
            "round" => PathLineCap::Round,
            "square" => PathLineCap::Square,
            _ => return Err("path object cap must be butt, round, or square".into()),
        });
    }
    if let Some(value) = input.get("join") {
        let name = value.as_str().ok_or("path object join must be a string")?;
        style.join = Some(match name.to_ascii_lowercase().as_str() {
            "miter" => PathLineJoin::Miter,
            "round" => PathLineJoin::Round,
            "bevel" => PathLineJoin::Bevel,
            _ => return Err("path object join must be miter, round, or bevel".into()),
        });
    }
    if let Some(value) = input.get("miter_limit") {
        style.miter_limit = Some(
            value
                .as_f64()
                .ok_or("path object miter_limit must be a number")?,
        );
    }
    if let Some(value) = input.get("dash_pattern") {
        let values = value
            .as_array()
            .ok_or("path object dash_pattern must be an array")?;
        style.dash_pattern = Some(
            values
                .iter()
                .map(|item| {
                    item.as_f64()
                        .ok_or("path object dash_pattern values must be numbers")
                })
                .collect::<Result<Vec<_>, _>>()?
                .try_into()
                .map_err(|_| "path object dash_pattern must contain two values")?,
        );
    }
    if let Some(value) = input.get("dash_sequence").filter(|value| !value.is_null()) {
        let values = value
            .as_array()
            .ok_or("path object dash_sequence must be an array")?;
        style.dash_sequence = Some(
            values
                .iter()
                .map(|item| {
                    item.as_f64()
                        .ok_or("path object dash_sequence values must be numbers")
                })
                .collect::<Result<Vec<_>, _>>()?,
        );
    }
    if let Some(value) = input.get("dash_offset") {
        style.dash_offset = Some(
            value
                .as_f64()
                .ok_or("path object dash_offset must be a number")?,
        );
    }
    for (field, target) in [
        ("fill_alpha", &mut style.fill_alpha),
        ("stroke_alpha", &mut style.stroke_alpha),
    ] {
        if let Some(value) = input.get(field) {
            *target = Some(
                value
                    .as_u64()
                    .and_then(|alpha| u8::try_from(alpha).ok())
                    .ok_or_else(|| format!("{field} must be a byte"))?,
            );
        }
    }
    for (field, target) in [
        ("fill_color", &mut style.fill_color),
        ("stroke_color", &mut style.stroke_color),
    ] {
        if let Some(value) = input.get(field) {
            let values = value
                .as_array()
                .ok_or_else(|| format!("{field} must be an RGB array"))?;
            let rgb: [u8; 3] = values
                .iter()
                .map(|value| {
                    value
                        .as_u64()
                        .and_then(|value| u8::try_from(value).ok())
                        .ok_or_else(|| format!("{field} must contain RGB bytes"))
                })
                .collect::<Result<Vec<_>, _>>()?
                .try_into()
                .map_err(|_| format!("{field} must contain three bytes"))?;
            *target = Some(rgb);
        }
    }
    Ok((commands, boundary, style))
}

/// Add a positioned PathObject to an existing page from JSON.
///
/// Required fields are `commands` and a four-number `boundary`. Optional
/// fields are `fill`, `even_odd`, `alpha`, `stroke`, `line_width`, `cap`,
/// `join`, `miter_limit`, `dash_pattern`, `dash_sequence`, `dash_offset`, `fill_color`,
/// `fill_alpha`, `stroke_color`, and `stroke_alpha`.
///
/// # Safety
/// `handle` must be live, `object_json` NUL-terminated, and `output_id` writable.
#[no_mangle]
pub unsafe extern "C" fn ofd_add_path_object_json(
    handle: *mut OfdDocument,
    page_index: usize,
    object_json: *const c_char,
    output_id: *mut usize,
) -> i32 {
    run(|| {
        let output = output_id.as_mut().ok_or("null path object ID output")?;
        *output = 0;
        let input: serde_json::Value =
            serde_json::from_str(input_string(object_json)?).map_err(|error| error.to_string())?;
        let (commands, boundary, style) = path_object_arguments(&input)?;
        let id = document_mut(handle)?
            .add_path_object(page_index, commands, boundary, style)
            .map_err(|error| error.to_string())?;
        *output = id.parse().map_err(|_| "invalid generated object ID")?;
        Ok(())
    })
}

/// Create a document composite resource containing one path and place it on a page.
///
/// JSON fields match `ofd_add_path_object_json`.
///
/// # Safety
/// `handle` must be live, `object_json` NUL-terminated, and `output_id` writable.
#[no_mangle]
pub unsafe extern "C" fn ofd_add_composite_path_object_json(
    handle: *mut OfdDocument,
    page_index: usize,
    object_json: *const c_char,
    output_id: *mut usize,
) -> i32 {
    run(|| {
        let output = output_id.as_mut().ok_or("null composite path ID output")?;
        *output = 0;
        let input: serde_json::Value =
            serde_json::from_str(input_string(object_json)?).map_err(|error| error.to_string())?;
        let (commands, boundary, style) = path_object_arguments(&input)?;
        let id = document_mut(handle)?
            .add_composite_path_object(page_index, commands, boundary, style)
            .map_err(|error| error.to_string())?;
        *output = id.parse().map_err(|_| "invalid generated object ID")?;
        Ok(())
    })
}

/// Place an existing composite graphic unit on a page from JSON.
///
/// Required fields are string `resource_id` and four-number `boundary`.
/// Optional fields are six-number `ctm`, byte `alpha`, and string `draw_param`.
///
/// # Safety
/// `handle` must be live, `object_json` NUL-terminated, and `output_id` writable.
#[no_mangle]
pub unsafe extern "C" fn ofd_add_composite_object_json(
    handle: *mut OfdDocument,
    page_index: usize,
    object_json: *const c_char,
    output_id: *mut usize,
) -> i32 {
    run(|| {
        let output = output_id
            .as_mut()
            .ok_or("null composite object ID output")?;
        *output = 0;
        let input: serde_json::Value =
            serde_json::from_str(input_string(object_json)?).map_err(|error| error.to_string())?;
        let resource_id = input
            .get("resource_id")
            .and_then(serde_json::Value::as_str)
            .ok_or("composite resource_id must be a string")?;
        let values = input
            .get("boundary")
            .and_then(serde_json::Value::as_array)
            .ok_or("composite boundary must be an array")?;
        let boundary: [f64; 4] = values
            .iter()
            .map(|value| {
                value
                    .as_f64()
                    .ok_or("composite boundary must contain numbers")
            })
            .collect::<Result<Vec<_>, _>>()?
            .try_into()
            .map_err(|_| "composite boundary must contain four numbers")?;
        let mut style = CompositeStyle::default();
        if let Some(value) = input.get("ctm") {
            let values = value.as_array().ok_or("composite ctm must be an array")?;
            style.ctm = Some(
                values
                    .iter()
                    .map(|value| value.as_f64().ok_or("composite ctm must contain numbers"))
                    .collect::<Result<Vec<_>, _>>()?
                    .try_into()
                    .map_err(|_| "composite ctm must contain six numbers")?,
            );
        }
        if let Some(value) = input.get("alpha") {
            style.alpha = Some(
                value
                    .as_u64()
                    .and_then(|value| u8::try_from(value).ok())
                    .ok_or("composite alpha must be a byte")?,
            );
        }
        if let Some(value) = input.get("draw_param") {
            style.draw_param = Some(
                value
                    .as_str()
                    .ok_or("composite draw_param must be a string")?
                    .to_owned(),
            );
        }
        let id = document_mut(handle)?
            .add_composite_object(page_index, resource_id, boundary, style)
            .map_err(|error| error.to_string())?;
        *output = id.parse().map_err(|_| "invalid generated object ID")?;
        Ok(())
    })
}

/// Add a text watermark annotation to a page from JSON.
///
/// Required fields are string `text`, four-number `boundary`, and number
/// `size`. Optional fields are `font_id`, RGB `color`, byte `opacity`,
/// `angle_degrees`, two-number `tile_spacing`, `bold`, and `italic`. The new annotation ID is written to
/// `output_id`.
///
/// # Safety
/// `handle` must be live, `object_json` NUL-terminated, and `output_id` writable.
#[no_mangle]
pub unsafe extern "C" fn ofd_add_text_watermark_json(
    handle: *mut OfdDocument,
    page_index: usize,
    object_json: *const c_char,
    output_id: *mut usize,
) -> i32 {
    add_text_annotation_json(handle, page_index, object_json, output_id, false)
}

/// Add a text Stamp annotation using the same JSON fields as a text watermark.
///
/// # Safety
/// `handle` must be live, `object_json` NUL-terminated, and `output_id` writable.
#[no_mangle]
pub unsafe extern "C" fn ofd_add_text_stamp_annotation_json(
    handle: *mut OfdDocument,
    page_index: usize,
    object_json: *const c_char,
    output_id: *mut usize,
) -> i32 {
    add_text_annotation_json(handle, page_index, object_json, output_id, true)
}

unsafe fn add_text_annotation_json(
    handle: *mut OfdDocument,
    page_index: usize,
    object_json: *const c_char,
    output_id: *mut usize,
    stamp: bool,
) -> i32 {
    run(|| {
        let output = output_id.as_mut().ok_or("null watermark ID output")?;
        *output = 0;
        let input: serde_json::Value =
            serde_json::from_str(input_string(object_json)?).map_err(|error| error.to_string())?;
        let text = input
            .get("text")
            .and_then(serde_json::Value::as_str)
            .ok_or("watermark text must be a string")?;
        let size = input
            .get("size")
            .and_then(serde_json::Value::as_f64)
            .ok_or("watermark size must be a number")?;
        let values = input
            .get("boundary")
            .and_then(serde_json::Value::as_array)
            .ok_or("watermark boundary must be an array")?;
        let boundary: [f64; 4] = values
            .iter()
            .map(|value| {
                value
                    .as_f64()
                    .ok_or("watermark boundary must contain numbers")
            })
            .collect::<Result<Vec<_>, _>>()?
            .try_into()
            .map_err(|_| "watermark boundary must contain four numbers")?;
        let style = parse_text_watermark_style(&input)?;
        let document = document_mut(handle)?;
        let id = if stamp {
            document.add_styled_text_stamp_annotation(page_index, text, boundary, size, &style)
        } else {
            document.add_styled_text_watermark(page_index, text, boundary, size, &style)
        }
        .map_err(|error| error.to_string())?;
        *output = id.parse().map_err(|_| "invalid generated annotation ID")?;
        Ok(())
    })
}

fn parse_text_watermark_style(input: &serde_json::Value) -> Result<TextWatermarkStyle, String> {
    let mut style = TextWatermarkStyle::default();
    if let Some(value) = input.get("font_id").filter(|value| !value.is_null()) {
        style.font_id = Some(
            value
                .as_str()
                .ok_or("watermark font_id must be a string")?
                .to_owned(),
        );
    }
    if let Some(value) = input.get("color") {
        style.color = serde_json::from_value(value.clone())
            .map_err(|_| "watermark color must contain three bytes")?;
    }
    if let Some(value) = input.get("opacity") {
        style.opacity = value
            .as_u64()
            .and_then(|number| u8::try_from(number).ok())
            .ok_or("watermark opacity must be a byte")?;
    }
    if let Some(value) = input.get("angle_degrees") {
        style.angle_degrees = value
            .as_f64()
            .ok_or("watermark angle_degrees must be a number")?;
    }
    if let Some(value) = input.get("tile_spacing").filter(|value| !value.is_null()) {
        style.tile_spacing = Some(
            serde_json::from_value(value.clone())
                .map_err(|_| "watermark tile_spacing must contain two numbers")?,
        );
    }
    if let Some(value) = input.get("bold") {
        style.bold = value.as_bool().ok_or("watermark bold must be a boolean")?;
    }
    if let Some(value) = input.get("italic") {
        style.italic = value
            .as_bool()
            .ok_or("watermark italic must be a boolean")?;
    }
    Ok(style)
}

/// Add a styled watermark annotation to every current page.
///
/// `object_json` uses the single-page watermark fields except `boundary`.
/// The output is a JSON array of numeric annotation IDs in page order.
///
/// # Safety
/// `handle` must be live, `object_json` NUL-terminated, and `output` writable.
#[no_mangle]
pub unsafe extern "C" fn ofd_add_text_watermark_all_pages_json(
    handle: *mut OfdDocument,
    object_json: *const c_char,
    output: *mut OfdBuffer,
) -> i32 {
    run(|| {
        if output.is_null() {
            return Err("null watermark IDs output".into());
        }
        let input: serde_json::Value =
            serde_json::from_str(input_string(object_json)?).map_err(|error| error.to_string())?;
        let text = input
            .get("text")
            .and_then(serde_json::Value::as_str)
            .ok_or("watermark text must be a string")?;
        let size = input
            .get("size")
            .and_then(serde_json::Value::as_f64)
            .ok_or("watermark size must be a number")?;
        let style = parse_text_watermark_style(&input)?;
        let ids = document_mut(handle)?
            .add_text_watermark_to_all_pages(text, size, &style)
            .map_err(|error| error.to_string())?
            .into_iter()
            .map(|id| {
                id.parse::<usize>()
                    .map_err(|_| "invalid generated annotation ID".to_owned())
            })
            .collect::<Result<Vec<_>, _>>()?;
        output_buffer(
            serde_json::to_vec(&ids).map_err(|error| error.to_string())?,
            output,
        )
    })
}

/// Add an image watermark from an existing image resource or base64 image data.
///
/// JSON requires `boundary`, with optional byte `opacity` (default 255).
/// Supply exactly one of `resource_id` or `data_base64` plus `format`.
///
/// # Safety
/// `handle` must be live, `object_json` NUL-terminated, and `output_id` writable.
#[no_mangle]
pub unsafe extern "C" fn ofd_add_image_watermark_json(
    handle: *mut OfdDocument,
    page_index: usize,
    object_json: *const c_char,
    output_id: *mut usize,
) -> i32 {
    run(|| {
        let output = output_id.as_mut().ok_or("null watermark ID output")?;
        *output = 0;
        let input: serde_json::Value =
            serde_json::from_str(input_string(object_json)?).map_err(|error| error.to_string())?;
        *output = add_image_annotation_json(document_mut(handle)?, page_index, &input, false)?;
        Ok(())
    })
}

/// Add a `Stamp` image annotation from an existing image resource or base64 image data.
/// The JSON fields match `ofd_add_image_watermark_json`.
///
/// # Safety
/// `handle` must be live, `object_json` NUL-terminated, and `output_id` writable.
#[no_mangle]
pub unsafe extern "C" fn ofd_add_image_stamp_annotation_json(
    handle: *mut OfdDocument,
    page_index: usize,
    object_json: *const c_char,
    output_id: *mut usize,
) -> i32 {
    run(|| {
        let output = output_id.as_mut().ok_or("null stamp ID output")?;
        *output = 0;
        let input: serde_json::Value =
            serde_json::from_str(input_string(object_json)?).map_err(|error| error.to_string())?;
        *output = add_image_annotation_json(document_mut(handle)?, page_index, &input, true)?;
        Ok(())
    })
}

fn add_image_annotation_json(
    document: &mut OfdDocument,
    page_index: usize,
    input: &serde_json::Value,
    stamp: bool,
) -> Result<usize, String> {
    let boundary: [f64; 4] = serde_json::from_value(
        input
            .get("boundary")
            .cloned()
            .ok_or("image watermark boundary is missing")?,
    )
    .map_err(|_| "image watermark boundary must contain four numbers")?;
    let opacity = match input.get("opacity") {
        Some(value) => value
            .as_u64()
            .and_then(|number| u8::try_from(number).ok())
            .ok_or("image watermark opacity must be a byte")?,
        None => 255,
    };
    let resource_id = input.get("resource_id").and_then(serde_json::Value::as_str);
    let data = input.get("data_base64").and_then(serde_json::Value::as_str);
    let id = match (resource_id, data) {
        (Some(resource_id), None) if stamp => {
            document.add_image_stamp_annotation(page_index, resource_id, boundary, opacity)
        }
        (Some(resource_id), None) => {
            document.add_image_watermark(page_index, resource_id, boundary, opacity)
        }
        (None, Some(data)) => {
            let format = input
                .get("format")
                .and_then(serde_json::Value::as_str)
                .ok_or("image watermark format must be a string")?;
            let bytes = STANDARD
                .decode(data)
                .map_err(|_| "invalid image data_base64")?;
            if stamp {
                document.add_image_stamp_annotation_from_bytes(
                    page_index, &bytes, format, boundary, opacity,
                )
            } else {
                document
                    .add_image_watermark_from_bytes(page_index, &bytes, format, boundary, opacity)
            }
        }
        _ => return Err("supply exactly one image annotation source".into()),
    }
    .map_err(|error| error.to_string())?;
    id.parse()
        .map_err(|_| "invalid generated annotation ID".into())
}

/// Add an image watermark from base64 image data to every current page.
/// JSON requires `data_base64`, `format`, and `boundary`; `opacity` defaults to 255.
/// The output is a JSON array of annotation IDs in page order.
///
/// # Safety
/// `handle` must be live, `object_json` NUL-terminated, and `output` writable.
#[no_mangle]
pub unsafe extern "C" fn ofd_add_image_watermark_all_pages_json(
    handle: *mut OfdDocument,
    object_json: *const c_char,
    output: *mut OfdBuffer,
) -> i32 {
    run(|| {
        if output.is_null() {
            return Err("null watermark IDs output".into());
        }
        let input: serde_json::Value =
            serde_json::from_str(input_string(object_json)?).map_err(|error| error.to_string())?;
        let boundary: [f64; 4] = serde_json::from_value(
            input
                .get("boundary")
                .cloned()
                .ok_or("image watermark boundary is missing")?,
        )
        .map_err(|_| "image watermark boundary must contain four numbers")?;
        let opacity = match input.get("opacity") {
            Some(value) => value
                .as_u64()
                .and_then(|number| u8::try_from(number).ok())
                .ok_or("image watermark opacity must be a byte")?,
            None => 255,
        };
        let data = input
            .get("data_base64")
            .and_then(serde_json::Value::as_str)
            .ok_or("image watermark data_base64 must be a string")?;
        let format = input
            .get("format")
            .and_then(serde_json::Value::as_str)
            .ok_or("image watermark format must be a string")?;
        let bytes = STANDARD
            .decode(data)
            .map_err(|_| "invalid image data_base64")?;
        let ids = document_mut(handle)?
            .add_image_watermark_to_all_pages(&bytes, format, boundary, opacity)
            .map_err(|error| error.to_string())?
            .into_iter()
            .map(|id| {
                id.parse::<usize>()
                    .map_err(|_| "invalid generated annotation ID".to_owned())
            })
            .collect::<Result<Vec<_>, _>>()?;
        output_buffer(
            serde_json::to_vec(&ids).map_err(|error| error.to_string())?,
            output,
        )
    })
}

/// Add a vector Path annotation using the same JSON fields as a page path.
///
/// # Safety
/// `handle` must be live, `object_json` NUL-terminated, and `output_id` writable.
#[no_mangle]
pub unsafe extern "C" fn ofd_add_path_annotation_json(
    handle: *mut OfdDocument,
    page_index: usize,
    object_json: *const c_char,
    output_id: *mut usize,
) -> i32 {
    run(|| {
        let output = output_id.as_mut().ok_or("null annotation ID output")?;
        *output = 0;
        let input: serde_json::Value =
            serde_json::from_str(input_string(object_json)?).map_err(|error| error.to_string())?;
        let (commands, boundary, style) = path_object_arguments(&input)?;
        let id = document_mut(handle)?
            .add_path_annotation(page_index, commands, boundary, style)
            .map_err(|error| error.to_string())?;
        *output = id.parse().map_err(|_| "invalid generated annotation ID")?;
        Ok(())
    })
}

/// Add one `Path` annotation containing multiple styled vector paths.
/// JSON requires a page-coordinate `boundary` and a nonempty `paths` array.
/// Each path has `commands`, an appearance-local `boundary`, and PathStyle fields.
///
/// # Safety
/// `handle` must be live, `object_json` NUL-terminated, and `output_id` writable.
#[no_mangle]
pub unsafe extern "C" fn ofd_add_multi_path_annotation_json(
    handle: *mut OfdDocument,
    page_index: usize,
    object_json: *const c_char,
    output_id: *mut usize,
) -> i32 {
    run(|| {
        let output = output_id.as_mut().ok_or("null annotation ID output")?;
        *output = 0;
        let input: serde_json::Value =
            serde_json::from_str(input_string(object_json)?).map_err(|error| error.to_string())?;
        let values = input
            .get("boundary")
            .and_then(serde_json::Value::as_array)
            .ok_or("annotation boundary must be an array")?;
        let boundary: [f64; 4] = values
            .iter()
            .map(|value| {
                value
                    .as_f64()
                    .ok_or("annotation boundary must contain numbers")
            })
            .collect::<Result<Vec<_>, _>>()?
            .try_into()
            .map_err(|_| "annotation boundary must contain four numbers")?;
        let objects = input
            .get("paths")
            .and_then(serde_json::Value::as_array)
            .ok_or("annotation paths must be an array")?;
        if objects.is_empty() || objects.len() > 4096 {
            return Err("annotation needs 1..4096 paths".into());
        }
        let paths = objects
            .iter()
            .map(|object| {
                let (commands, boundary, style) = path_object_arguments(object)?;
                Ok(AnnotationPath {
                    commands: commands.to_owned(),
                    boundary,
                    style,
                })
            })
            .collect::<Result<Vec<_>, String>>()?;
        let id = document_mut(handle)?
            .add_multi_path_annotation(page_index, boundary, &paths)
            .map_err(|error| error.to_string())?;
        *output = id.parse().map_err(|_| "invalid generated annotation ID")?;
        Ok(())
    })
}

fn annotation_graphics_from_json(
    input: &serde_json::Value,
) -> Result<Vec<AnnotationGraphic>, String> {
    let values = input
        .get("objects")
        .and_then(serde_json::Value::as_array)
        .ok_or("annotation objects must be an array")?;
    if values.is_empty() || values.len() > 4096 {
        return Err("annotation needs 1..4096 objects".into());
    }
    values
        .iter()
        .map(|value| -> Result<AnnotationGraphic, String> {
            match value.get("kind").and_then(serde_json::Value::as_str) {
                Some("path") => {
                    let (commands, boundary, style) = path_object_arguments(value)?;
                    Ok(AnnotationGraphic::Path(AnnotationPath {
                        commands: commands.to_owned(),
                        boundary,
                        style,
                    }))
                }
                Some("text") | Some("image") => {
                    let object_boundary: [f64; 4] = serde_json::from_value(
                        value
                            .get("boundary")
                            .cloned()
                            .ok_or("object boundary is required")?,
                    )
                    .map_err(|error| format!("object boundary: {error}"))?;
                    let opacity = value
                        .get("opacity")
                        .map(|value| {
                            serde_json::from_value::<u8>(value.clone())
                                .map_err(|error| format!("object opacity: {error}"))
                        })
                        .transpose()?
                        .unwrap_or(255);
                    if value.get("kind").and_then(serde_json::Value::as_str) == Some("text") {
                        let text = value
                            .get("text")
                            .and_then(serde_json::Value::as_str)
                            .ok_or("annotation text must be a string")?;
                        let size = value
                            .get("size")
                            .and_then(serde_json::Value::as_f64)
                            .ok_or("annotation text size must be a number")?;
                        let font_id = value
                            .get("font_id")
                            .map(|value| value.as_str().ok_or("font_id must be a string"))
                            .transpose()?
                            .map(str::to_owned);
                        let color = value
                            .get("color")
                            .map(|value| {
                                serde_json::from_value::<[u8; 3]>(value.clone())
                                    .map_err(|error| format!("annotation text color: {error}"))
                            })
                            .transpose()?;
                        Ok(AnnotationGraphic::Text {
                            text: text.to_owned(),
                            boundary: object_boundary,
                            size,
                            font_id,
                            color,
                            opacity,
                        })
                    } else {
                        let resource_id = value
                            .get("resource_id")
                            .and_then(serde_json::Value::as_str)
                            .ok_or("annotation image resource_id must be a string")?;
                        Ok(AnnotationGraphic::Image {
                            resource_id: resource_id.to_owned(),
                            boundary: object_boundary,
                            opacity,
                        })
                    }
                }
                Some("composite") => {
                    let boundary: [f64; 4] = serde_json::from_value(
                        value
                            .get("boundary")
                            .cloned()
                            .ok_or("object boundary is required")?,
                    )
                    .map_err(|error| format!("object boundary: {error}"))?;
                    let resource_id = value
                        .get("resource_id")
                        .and_then(serde_json::Value::as_str)
                        .ok_or("annotation composite resource_id must be a string")?;
                    let ctm = value
                        .get("ctm")
                        .map(|value| {
                            serde_json::from_value::<[f64; 6]>(value.clone())
                                .map_err(|error| format!("composite ctm: {error}"))
                        })
                        .transpose()?;
                    let alpha = value
                        .get("alpha")
                        .map(|value| {
                            serde_json::from_value::<u8>(value.clone())
                                .map_err(|error| format!("composite alpha: {error}"))
                        })
                        .transpose()?;
                    let draw_param = value
                        .get("draw_param")
                        .map(|value| {
                            value
                                .as_str()
                                .ok_or("composite draw_param must be a string")
                        })
                        .transpose()?
                        .map(str::to_owned);
                    Ok(AnnotationGraphic::Composite {
                        resource_id: resource_id.to_owned(),
                        boundary,
                        style: CompositeStyle {
                            ctm,
                            alpha,
                            draw_param,
                        },
                    })
                }
                _ => Err("annotation object kind must be path, text, image, or composite".into()),
            }
        })
        .collect::<Result<Vec<_>, _>>()
}

/// Add one `Path` or `Stamp` annotation with ordered path, text, image, and composite graphics.
/// JSON: `{"annotation_type":"Path|Stamp","boundary":[x,y,w,h],"objects":[{"kind":"path",...},
/// {"kind":"text","text":"...","boundary":[...],"size":4,...},
/// {"kind":"image","resource_id":"...","boundary":[...],...}]}`.
///
/// # Safety
/// `handle` must be live, `object_json` NUL-terminated, and `output_id` writable.
#[no_mangle]
pub unsafe extern "C" fn ofd_add_mixed_annotation_json(
    handle: *mut OfdDocument,
    page_index: usize,
    object_json: *const c_char,
    output_id: *mut usize,
) -> i32 {
    run(|| {
        let output = output_id.as_mut().ok_or("null annotation ID output")?;
        *output = 0;
        let input: serde_json::Value =
            serde_json::from_str(input_string(object_json)?).map_err(|error| error.to_string())?;
        let boundary: [f64; 4] = serde_json::from_value(
            input
                .get("boundary")
                .cloned()
                .ok_or("annotation boundary is required")?,
        )
        .map_err(|error| format!("annotation boundary: {error}"))?;
        let objects = annotation_graphics_from_json(&input)?;
        let annotation_type = input
            .get("annotation_type")
            .map(|value| value.as_str().ok_or("annotation_type must be a string"))
            .transpose()?
            .unwrap_or("Path");
        let document = document_mut(handle)?;
        let id = match annotation_type {
            "Path" => document.add_mixed_annotation(page_index, boundary, &objects),
            "Stamp" => document.add_mixed_stamp_annotation(page_index, boundary, &objects),
            _ => return Err("annotation_type must be Path or Stamp".into()),
        }
        .map_err(|error| error.to_string())?;
        *output = id.parse().map_err(|_| "invalid generated annotation ID")?;
        Ok(())
    })
}

/// Append ordered path, text, image, or composite objects to an existing annotation.
/// JSON requires an `objects` array; output is a JSON array of new object IDs.
///
/// # Safety
/// `handle` must be live, strings NUL-terminated, and `output` writable.
#[no_mangle]
pub unsafe extern "C" fn ofd_append_annotation_objects_json(
    handle: *mut OfdDocument,
    page_index: usize,
    annotation_id: *const c_char,
    object_json: *const c_char,
    output: *mut OfdBuffer,
) -> i32 {
    run(|| {
        let input: serde_json::Value =
            serde_json::from_str(input_string(object_json)?).map_err(|error| error.to_string())?;
        let objects = annotation_graphics_from_json(&input)?;
        let ids = document_mut(handle)?
            .append_annotation_objects(page_index, input_string(annotation_id)?, &objects)
            .map_err(|error| error.to_string())?;
        output_buffer(
            serde_json::to_vec(&ids).map_err(|error| error.to_string())?,
            output,
        )
    })
}

/// Add a translucent rectangular `Highlight` annotation.
/// JSON requires four-number `boundary` and three-byte RGB `color`;
/// `opacity` defaults to 96.
///
/// # Safety
/// `handle` must be live, `object_json` NUL-terminated, and `output_id` writable.
#[no_mangle]
pub unsafe extern "C" fn ofd_add_highlight_annotation_json(
    handle: *mut OfdDocument,
    page_index: usize,
    object_json: *const c_char,
    output_id: *mut usize,
) -> i32 {
    #[derive(Deserialize)]
    #[serde(deny_unknown_fields)]
    struct Input {
        boundary: [f64; 4],
        color: [u8; 3],
        #[serde(default = "default_opacity")]
        opacity: u8,
    }
    fn default_opacity() -> u8 {
        96
    }
    run(|| {
        let output = output_id.as_mut().ok_or("null annotation ID output")?;
        *output = 0;
        let input: Input =
            serde_json::from_str(input_string(object_json)?).map_err(|error| error.to_string())?;
        let id = document_mut(handle)?
            .add_highlight_annotation(page_index, input.boundary, input.color, input.opacity)
            .map_err(|error| error.to_string())?;
        *output = id.parse().map_err(|_| "invalid generated annotation ID")?;
        Ok(())
    })
}

/// Add an editable `Text` note annotation from JSON.
/// Required fields are `text` and `boundary`; optional fields are RGB `color`,
/// alpha-byte `opacity`, `icon`, and `open`.
///
/// # Safety
/// `handle` must be live, `object_json` NUL-terminated, and `output_id` writable.
#[no_mangle]
pub unsafe extern "C" fn ofd_add_text_note_annotation_json(
    handle: *mut OfdDocument,
    page_index: usize,
    object_json: *const c_char,
    output_id: *mut usize,
) -> i32 {
    #[derive(Deserialize)]
    #[serde(deny_unknown_fields)]
    struct Input {
        text: String,
        boundary: [f64; 4],
        #[serde(default = "default_color")]
        color: [u8; 3],
        #[serde(default = "default_opacity")]
        opacity: u8,
        #[serde(default = "default_icon")]
        icon: String,
        #[serde(default)]
        open: bool,
    }
    fn default_color() -> [u8; 3] {
        [255, 255, 0]
    }
    fn default_opacity() -> u8 {
        255
    }
    fn default_icon() -> String {
        "Note".into()
    }
    run(|| {
        let output = output_id.as_mut().ok_or("null annotation ID output")?;
        *output = 0;
        let input: Input =
            serde_json::from_str(input_string(object_json)?).map_err(|error| error.to_string())?;
        let style = TextNoteStyle {
            color: Some(input.color),
            opacity: Some(input.opacity),
            icon: input.icon,
            open: input.open,
        };
        let id = document_mut(handle)?
            .add_text_note_annotation(page_index, &input.text, input.boundary, &style)
            .map_err(|error| error.to_string())?;
        *output = id.parse().map_err(|_| "invalid generated annotation ID")?;
        Ok(())
    })
}

/// Add an editable Square or Circle annotation from JSON. The required fields
/// are `kind` and `boundary`; all appearance fields are optional.
///
/// # Safety
/// `handle` must be live, `object_json` NUL-terminated, and `output_id` writable.
#[no_mangle]
pub unsafe extern "C" fn ofd_add_shape_annotation_json(
    handle: *mut OfdDocument,
    page_index: usize,
    object_json: *const c_char,
    output_id: *mut usize,
) -> i32 {
    #[derive(Deserialize)]
    #[serde(deny_unknown_fields)]
    struct Input {
        kind: String,
        boundary: [f64; 4],
        stroke_color: Option<[u8; 3]>,
        stroke: Option<bool>,
        fill_color: Option<[u8; 3]>,
        line_width: Option<f64>,
        dash_sequence: Option<Vec<f64>>,
        opacity: Option<u8>,
        insets: Option<[f64; 4]>,
    }
    run(|| {
        let output = output_id.as_mut().ok_or("null annotation ID output")?;
        *output = 0;
        let input: Input =
            serde_json::from_str(input_string(object_json)?).map_err(|error| error.to_string())?;
        let mut style = ShapeAnnotationStyle::default();
        if let Some(value) = input.stroke_color {
            style.stroke_color = Some(value);
        }
        if input.stroke == Some(false) {
            style.stroke_color = None;
        }
        style.fill_color = input.fill_color;
        if let Some(value) = input.line_width {
            style.line_width = value;
        }
        style.dash_sequence = input.dash_sequence;
        if let Some(value) = input.opacity {
            style.opacity = value;
        }
        if let Some(value) = input.insets {
            style.insets = value;
        }
        let id = document_mut(handle)?
            .add_shape_annotation(page_index, &input.kind, input.boundary, &style)
            .map_err(|error| error.to_string())?;
        *output = id.parse().map_err(|_| "invalid generated annotation ID")?;
        Ok(())
    })
}

/// Add an editable Line annotation from JSON. `endpoints` contains two
/// boundary-local points in millimeters.
///
/// # Safety
/// `handle` must be live, `object_json` NUL-terminated, and `output_id` writable.
#[no_mangle]
pub unsafe extern "C" fn ofd_add_line_annotation_json(
    handle: *mut OfdDocument,
    page_index: usize,
    object_json: *const c_char,
    output_id: *mut usize,
) -> i32 {
    #[derive(Deserialize)]
    #[serde(deny_unknown_fields)]
    struct Input {
        boundary: [f64; 4],
        endpoints: [f64; 4],
        stroke_color: Option<[u8; 3]>,
        stroke: Option<bool>,
        line_width: Option<f64>,
        dash_sequence: Option<Vec<f64>>,
        opacity: Option<u8>,
    }
    run(|| {
        let output = output_id.as_mut().ok_or("null annotation ID output")?;
        *output = 0;
        let input: Input =
            serde_json::from_str(input_string(object_json)?).map_err(|error| error.to_string())?;
        let mut style = ShapeAnnotationStyle::default();
        if let Some(value) = input.stroke_color {
            style.stroke_color = Some(value);
        }
        if input.stroke == Some(false) {
            style.stroke_color = None;
        }
        if let Some(value) = input.line_width {
            style.line_width = value;
        }
        style.dash_sequence = input.dash_sequence;
        if let Some(value) = input.opacity {
            style.opacity = value;
        }
        let id = document_mut(handle)?
            .add_line_annotation(page_index, input.boundary, input.endpoints, &style)
            .map_err(|error| error.to_string())?;
        *output = id.parse().map_err(|_| "invalid generated annotation ID")?;
        Ok(())
    })
}

/// Add an invisible Link annotation that opens a URI on click.
/// JSON requires `uri` and a four-number `boundary`, and optionally accepts
/// `quad_points` as an array of four page-coordinate points per area.
///
/// # Safety
/// `handle` must be live, `object_json` NUL-terminated, and `output_id` writable.
#[no_mangle]
pub unsafe extern "C" fn ofd_add_uri_link_annotation_json(
    handle: *mut OfdDocument,
    page_index: usize,
    object_json: *const c_char,
    output_id: *mut usize,
) -> i32 {
    #[derive(Deserialize)]
    #[serde(deny_unknown_fields)]
    struct Input {
        uri: String,
        boundary: [f64; 4],
        #[serde(default)]
        quad_points: Vec<[[f64; 2]; 4]>,
    }
    run(|| {
        let output = output_id.as_mut().ok_or("null annotation ID output")?;
        *output = 0;
        let input: Input =
            serde_json::from_str(input_string(object_json)?).map_err(|error| error.to_string())?;
        let id = document_mut(handle)?
            .add_uri_link_annotation_with_quads(
                page_index,
                &input.uri,
                input.boundary,
                &input.quad_points,
            )
            .map_err(|error| error.to_string())?;
        *output = id.parse().map_err(|_| "invalid generated annotation ID")?;
        Ok(())
    })
}

/// Add an invisible Link annotation with a direct Goto destination.
/// JSON requires `destination` and a four-number `boundary`, and optionally
/// accepts `quad_points` as an array of four page-coordinate points per area.
///
/// # Safety
/// `handle` must be live, `object_json` NUL-terminated, and `output_id` writable.
#[no_mangle]
pub unsafe extern "C" fn ofd_add_goto_link_annotation_json(
    handle: *mut OfdDocument,
    page_index: usize,
    object_json: *const c_char,
    output_id: *mut usize,
) -> i32 {
    run(|| {
        let output = output_id.as_mut().ok_or("null annotation ID output")?;
        *output = 0;
        let input: serde_json::Value =
            serde_json::from_str(input_string(object_json)?).map_err(|error| error.to_string())?;
        let boundary: [f64; 4] = serde_json::from_value(
            input
                .get("boundary")
                .cloned()
                .ok_or("missing link boundary")?,
        )
        .map_err(|error| error.to_string())?;
        let destination =
            parse_destination_value(input.get("destination").ok_or("missing link destination")?)?;
        let quads: Vec<[[f64; 2]; 4]> = input
            .get("quad_points")
            .map(|value| serde_json::from_value(value.clone()).map_err(|error| error.to_string()))
            .transpose()?
            .unwrap_or_default();
        let id = document_mut(handle)?
            .add_goto_link_annotation_with_quads(page_index, destination, boundary, &quads)
            .map_err(|error| error.to_string())?;
        *output = id.parse().map_err(|_| "invalid generated annotation ID")?;
        Ok(())
    })
}

/// Add an invisible Link annotation targeting a named bookmark.
/// JSON requires `bookmark_name` and a four-number `boundary`.
///
/// # Safety
/// `handle` must be live, `object_json` NUL-terminated, and `output_id` writable.
#[no_mangle]
pub unsafe extern "C" fn ofd_add_bookmark_link_annotation_json(
    handle: *mut OfdDocument,
    page_index: usize,
    object_json: *const c_char,
    output_id: *mut usize,
) -> i32 {
    #[derive(Deserialize)]
    #[serde(deny_unknown_fields)]
    struct Input {
        bookmark_name: String,
        boundary: [f64; 4],
    }
    run(|| {
        let output = output_id.as_mut().ok_or("null annotation ID output")?;
        *output = 0;
        let input: Input =
            serde_json::from_str(input_string(object_json)?).map_err(|error| error.to_string())?;
        let id = document_mut(handle)?
            .add_bookmark_link_annotation(page_index, &input.bookmark_name, input.boundary)
            .map_err(|error| error.to_string())?;
        *output = id.parse().map_err(|_| "invalid generated annotation ID")?;
        Ok(())
    })
}

/// Add an invisible Link annotation targeting an attachment in the same document.
/// JSON requires `attachment_id` and `boundary`; `new_window` is optional.
///
/// # Safety
/// `handle` must be live, `object_json` NUL-terminated, and `output_id` writable.
#[no_mangle]
pub unsafe extern "C" fn ofd_add_attachment_link_annotation_json(
    handle: *mut OfdDocument,
    page_index: usize,
    object_json: *const c_char,
    output_id: *mut usize,
) -> i32 {
    #[derive(Deserialize)]
    #[serde(deny_unknown_fields)]
    struct Input {
        attachment_id: String,
        boundary: [f64; 4],
        new_window: Option<bool>,
    }
    run(|| {
        let output = output_id.as_mut().ok_or("null annotation ID output")?;
        *output = 0;
        let input: Input =
            serde_json::from_str(input_string(object_json)?).map_err(|error| error.to_string())?;
        let id = document_mut(handle)?
            .add_attachment_link_annotation(
                page_index,
                &input.attachment_id,
                input.boundary,
                input.new_window,
            )
            .map_err(|error| error.to_string())?;
        *output = id.parse().map_err(|_| "invalid generated annotation ID")?;
        Ok(())
    })
}

/// Add an invisible Link annotation with a Sound or Movie click action.
/// JSON requires `kind`, `resource_id`, and `boundary`.
///
/// # Safety
/// `handle` must be live, `object_json` NUL-terminated, and `output_id` writable.
#[no_mangle]
pub unsafe extern "C" fn ofd_add_media_link_annotation_json(
    handle: *mut OfdDocument,
    page_index: usize,
    object_json: *const c_char,
    output_id: *mut usize,
) -> i32 {
    run(|| {
        let output = output_id.as_mut().ok_or("null annotation ID output")?;
        *output = 0;
        let input: MediaActionInput =
            serde_json::from_str(input_string(object_json)?).map_err(|error| error.to_string())?;
        let boundary = input.boundary.ok_or("missing link boundary")?;
        let document = document_mut(handle)?;
        let id = match input.kind.as_str() {
            "Sound" => document.add_sound_link_annotation(
                page_index,
                &input.resource_id,
                boundary,
                input.sound_options()?,
            ),
            "Movie" => document.add_movie_link_annotation(
                page_index,
                &input.resource_id,
                boundary,
                input.movie_operator()?,
            ),
            _ => return Err("media action kind must be Sound or Movie".into()),
        }
        .map_err(|error| error.to_string())?;
        *output = id.parse().map_err(|_| "invalid generated annotation ID")?;
        Ok(())
    })
}

/// Add a text page using a declared font resource.
///
/// # Safety
/// `handle` must be live and `text` a valid NUL-terminated string.
#[no_mangle]
pub unsafe extern "C" fn ofd_add_text_page_with_font(
    handle: *mut OfdDocument,
    text: *const c_char,
    width_mm: f64,
    height_mm: f64,
    font_id: usize,
) -> i32 {
    run(|| {
        document_mut(handle)?
            .add_text_page_with_font(input_string(text)?, width_mm, height_mm, font_id)
            .map_err(|e| e.to_string())
    })
}

/// Embed a TrueType or OpenType font in a newly created document.
///
/// # Safety
/// `handle` must be live, `name` NUL-terminated, `data` valid for `len` bytes,
/// and `output` writable.
#[no_mangle]
pub unsafe extern "C" fn ofd_add_embedded_font(
    handle: *mut OfdDocument,
    name: *const c_char,
    data: *const u8,
    len: usize,
    output: *mut usize,
) -> i32 {
    run(|| {
        let output = output.as_mut().ok_or("null font identifier output")?;
        *output = document_mut(handle)?
            .add_embedded_font(input_string(name)?, input_bytes(data, len)?)
            .map_err(|e| e.to_string())?;
        Ok(())
    })
}

/// Embed a TrueType or OpenType font in an existing document body.
///
/// # Safety
/// `handle` must be live, `name` NUL-terminated, `data` valid for `len` bytes,
/// and `output` writable.
#[no_mangle]
pub unsafe extern "C" fn ofd_add_embedded_font_to_document(
    handle: *mut OfdDocument,
    document_index: usize,
    name: *const c_char,
    data: *const u8,
    len: usize,
    output: *mut usize,
) -> i32 {
    run(|| {
        let output = output.as_mut().ok_or("null font identifier output")?;
        *output = document_mut(handle)?
            .add_embedded_font_to_document(
                document_index,
                input_string(name)?,
                input_bytes(data, len)?,
            )
            .map_err(|e| e.to_string())?;
        Ok(())
    })
}

/// Embed a TrueType or OpenType font in one page's resources.
///
/// # Safety
/// `handle` must be live, `name` NUL-terminated, `data` valid for `len` bytes,
/// and `output` writable.
#[no_mangle]
pub unsafe extern "C" fn ofd_add_embedded_font_to_page(
    handle: *mut OfdDocument,
    page_index: usize,
    name: *const c_char,
    data: *const u8,
    len: usize,
    output: *mut usize,
) -> i32 {
    run(|| {
        let output = output.as_mut().ok_or("null font identifier output")?;
        *output = document_mut(handle)?
            .add_embedded_font_to_page(page_index, input_string(name)?, input_bytes(data, len)?)
            .map_err(|error| error.to_string())?;
        Ok(())
    })
}

/// Append a text page to a document body in an existing or new package.
///
/// # Safety
/// `handle` must be live and `text` a valid NUL-terminated string.
#[no_mangle]
pub unsafe extern "C" fn ofd_append_text_page(
    handle: *mut OfdDocument,
    document_index: usize,
    text: *const c_char,
    width_mm: f64,
    height_mm: f64,
) -> i32 {
    run(|| {
        document_mut(handle)?
            .append_text_page(document_index, input_string(text)?, width_mm, height_mm)
            .map_err(|e| e.to_string())
    })
}

/// Append an empty page to a document body.
///
/// # Safety
/// `handle` must be a live document handle.
#[no_mangle]
pub unsafe extern "C" fn ofd_append_blank_page(
    handle: *mut OfdDocument,
    document_index: usize,
    width_mm: f64,
    height_mm: f64,
) -> i32 {
    run(|| {
        document_mut(handle)?
            .append_blank_page(document_index, width_mm, height_mm)
            .map_err(|e| e.to_string())
    })
}

/// Insert a text page at a local position in one document body.
///
/// # Safety
/// `handle` must be live, `text` NUL-terminated, and `output` writable.
#[no_mangle]
pub unsafe extern "C" fn ofd_insert_text_page(
    handle: *mut OfdDocument,
    document_index: usize,
    position: usize,
    text: *const c_char,
    width_mm: f64,
    height_mm: f64,
    output: *mut usize,
) -> i32 {
    run(|| {
        if output.is_null() {
            return Err("null output pointer".into());
        }
        let index = document_mut(handle)?
            .insert_text_page(
                document_index,
                position,
                input_string(text)?,
                width_mm,
                height_mm,
            )
            .map_err(|error| error.to_string())?;
        unsafe { *output = index };
        Ok(())
    })
}

/// Insert an empty page at a local position in one document body.
///
/// # Safety
/// `handle` must be live and `output` writable.
#[no_mangle]
pub unsafe extern "C" fn ofd_insert_blank_page(
    handle: *mut OfdDocument,
    document_index: usize,
    position: usize,
    width_mm: f64,
    height_mm: f64,
    output: *mut usize,
) -> i32 {
    run(|| {
        if output.is_null() {
            return Err("null output pointer".into());
        }
        let index = document_mut(handle)?
            .insert_blank_page(document_index, position, width_mm, height_mm)
            .map_err(|error| error.to_string())?;
        unsafe { *output = index };
        Ok(())
    })
}

fn parse_flow_style(input: &serde_json::Value) -> Result<FlowTextStyle, String> {
    let mut style = FlowTextStyle::default();
    if let Some(value) = input.get("page_size") {
        style.page_size = serde_json::from_value(value.clone())
            .map_err(|_| "page_size must contain two numbers")?;
    }
    if let Some(value) = input.get("margins") {
        style.margins = serde_json::from_value(value.clone())
            .map_err(|_| "margins must contain four numbers")?;
    }
    if let Some(value) = input.get("font_size") {
        style.font_size = value.as_f64().ok_or("font_size must be a number")?;
    }
    if let Some(value) = input.get("line_height") {
        style.line_height = value.as_f64().ok_or("line_height must be a number")?;
    }
    if let Some(value) = input.get("paragraph_gap") {
        style.paragraph_gap = value.as_f64().ok_or("paragraph_gap must be a number")?;
    }
    if let Some(value) = input.get("keep_paragraphs_together") {
        style.keep_paragraphs_together = value
            .as_bool()
            .ok_or("keep_paragraphs_together must be a boolean")?;
    }
    if let Some(value) = input.get("font_id") {
        style.font_id = Some(value.as_str().ok_or("font_id must be a string")?.to_owned());
    }
    if let Some(value) = input.get("align") {
        style.align = match value.as_str() {
            Some("left") => FlowTextAlign::Left,
            Some("center") => FlowTextAlign::Center,
            Some("right") => FlowTextAlign::Right,
            _ => return Err("align must be left, center, or right".into()),
        };
    }
    if let Some(value) = input.get("first_line_indent") {
        style.first_line_indent = value.as_f64().ok_or("first_line_indent must be a number")?;
    }
    if let Some(value) = input.get("rest_line_indent") {
        style.rest_line_indent = value.as_f64().ok_or("rest_line_indent must be a number")?;
    }
    if let Some(value) = input.get("first_line_indent_chars") {
        style.first_line_indent_chars = value
            .as_u64()
            .and_then(|count| usize::try_from(count).ok())
            .ok_or("first_line_indent_chars must be a nonnegative integer")?;
    }
    if let Some(value) = input.get("indent_each_paragraph") {
        style.indent_each_paragraph = value
            .as_bool()
            .ok_or("indent_each_paragraph must be a boolean")?;
    }
    if let Some(value) = input.get("shape") {
        style.shape = value.as_bool().ok_or("shape must be a boolean")?;
    }
    if let Some(value) = input.get("language") {
        style.language = Some(
            value
                .as_str()
                .ok_or("language must be a string")?
                .to_owned(),
        );
    }
    Ok(style)
}

fn parse_table_cell_align(
    value: Option<&serde_json::Value>,
) -> Result<Option<FlowTextAlign>, String> {
    match value {
        None => Ok(None),
        Some(serde_json::Value::String(value)) => match value.as_str() {
            "left" => Ok(Some(FlowTextAlign::Left)),
            "center" => Ok(Some(FlowTextAlign::Center)),
            "right" => Ok(Some(FlowTextAlign::Right)),
            _ => Err("table cell align must be left, center, or right".into()),
        },
        _ => Err("table cell align must be a string".into()),
    }
}

fn parse_table_vertical_align(
    value: Option<&serde_json::Value>,
) -> Result<TableVerticalAlign, String> {
    match value {
        None | Some(serde_json::Value::Null) => Ok(TableVerticalAlign::Top),
        Some(serde_json::Value::String(value)) => match value.as_str() {
            "top" => Ok(TableVerticalAlign::Top),
            "center" => Ok(TableVerticalAlign::Center),
            "bottom" => Ok(TableVerticalAlign::Bottom),
            _ => Err("table cell vertical_align must be top, center, or bottom".into()),
        },
        _ => Err("table cell vertical_align must be a string".into()),
    }
}

fn parse_table_cell_image(value: &serde_json::Value) -> Result<TableCellImage, String> {
    let data = value
        .get("data_base64")
        .and_then(serde_json::Value::as_str)
        .ok_or("table cell image data_base64 must be a string")?;
    Ok(TableCellImage {
        data: STANDARD
            .decode(data)
            .map_err(|_| "table cell image data_base64 is invalid")?,
        format: value
            .get("format")
            .and_then(serde_json::Value::as_str)
            .ok_or("table cell image format must be a string")?
            .to_owned(),
        width: value
            .get("width")
            .and_then(serde_json::Value::as_f64)
            .ok_or("table cell image width must be a number")?,
        height: value
            .get("height")
            .and_then(serde_json::Value::as_f64)
            .ok_or("table cell image height must be a number")?,
    })
}

fn parse_table_cells(values: &serde_json::Value, depth: usize) -> Result<Vec<TableCell>, String> {
    if depth > 8 {
        return Err("table nesting exceeds eight levels".into());
    }
    let values = values.as_array().ok_or("cells must be an array")?;
    values
        .iter()
        .map(|value| {
            let index = |key: &str, default: Option<usize>| -> Result<usize, String> {
                match value.get(key) {
                    Some(number) => number
                        .as_u64()
                        .and_then(|number| usize::try_from(number).ok())
                        .ok_or_else(|| format!("table cell {key} must be a nonnegative integer")),
                    None => default.ok_or_else(|| format!("table cell {key} is required")),
                }
            };
            let blocks = match value.get("blocks") {
                None => Vec::new(),
                Some(blocks) => blocks
                    .as_array()
                    .ok_or("table cell blocks must be an array")?
                    .iter()
                    .map(|block| parse_table_block(block, depth))
                    .collect::<Result<_, _>>()?,
            };
            Ok(TableCell {
                row: index("row", None)?,
                column: index("column", None)?,
                row_span: index("row_span", Some(1))?,
                column_span: index("column_span", Some(1))?,
                text: match value.get("text") {
                    None => String::new(),
                    Some(text) => text
                        .as_str()
                        .ok_or("table cell text must be a string")?
                        .to_owned(),
                },
                background_color: value
                    .get("background_color")
                    .map(|color| {
                        serde_json::from_value(color.clone()).map_err(|_| {
                            "table cell background_color must be three RGB bytes".to_owned()
                        })
                    })
                    .transpose()?,
                text_color: value
                    .get("text_color")
                    .map(|color| {
                        serde_json::from_value(color.clone())
                            .map_err(|_| "table cell text_color must be three RGB bytes".to_owned())
                    })
                    .transpose()?,
                border_width: value
                    .get("border_width")
                    .map(|width| {
                        width
                            .as_f64()
                            .ok_or("table cell border_width must be a number")
                    })
                    .transpose()?,
                border_color: value
                    .get("border_color")
                    .map(|color| {
                        serde_json::from_value(color.clone()).map_err(|_| {
                            "table cell border_color must be three RGB bytes".to_owned()
                        })
                    })
                    .transpose()?,
                padding: value
                    .get("padding")
                    .map(|padding| {
                        serde_json::from_value(padding.clone())
                            .map_err(|_| "table cell padding must be four numbers".to_owned())
                    })
                    .transpose()?,
                font_id: value
                    .get("font_id")
                    .map(|value| {
                        value
                            .as_str()
                            .map(str::to_owned)
                            .ok_or("table cell font_id must be a string")
                    })
                    .transpose()?,
                font_size: value
                    .get("font_size")
                    .map(|value| {
                        value
                            .as_f64()
                            .ok_or("table cell font_size must be a number")
                    })
                    .transpose()?,
                line_height: value
                    .get("line_height")
                    .map(|value| {
                        value
                            .as_f64()
                            .ok_or("table cell line_height must be a number")
                    })
                    .transpose()?,
                letter_spacing: value
                    .get("letter_spacing")
                    .map(|value| {
                        value
                            .as_f64()
                            .ok_or("table cell letter_spacing must be a number")
                    })
                    .transpose()?,
                bold: value
                    .get("bold")
                    .map(|value| value.as_bool().ok_or("table cell bold must be a boolean"))
                    .transpose()?
                    .unwrap_or(false),
                italic: value
                    .get("italic")
                    .map(|value| value.as_bool().ok_or("table cell italic must be a boolean"))
                    .transpose()?
                    .unwrap_or(false),
                underline: value
                    .get("underline")
                    .map(|value| {
                        value
                            .as_bool()
                            .ok_or("table cell underline must be a boolean")
                    })
                    .transpose()?
                    .unwrap_or(false),
                strikethrough: value
                    .get("strikethrough")
                    .map(|value| {
                        value
                            .as_bool()
                            .ok_or("table cell strikethrough must be a boolean")
                    })
                    .transpose()?
                    .unwrap_or(false),
                align: parse_table_cell_align(value.get("align"))?.unwrap_or(FlowTextAlign::Left),
                vertical_align: parse_table_vertical_align(value.get("vertical_align"))?,
                image: value.get("image").map(parse_table_cell_image).transpose()?,
                blocks,
            })
        })
        .collect()
}

fn parse_table_block(value: &serde_json::Value, depth: usize) -> Result<TableCellBlock, String> {
    match value.get("type").and_then(serde_json::Value::as_str) {
        Some("text") => Ok(TableCellBlock::Text {
            text: value
                .get("text")
                .and_then(serde_json::Value::as_str)
                .ok_or("table text block text must be a string")?
                .to_owned(),
            color: value
                .get("text_color")
                .map(|color| {
                    serde_json::from_value(color.clone()).map_err(|_| {
                        "table text block text_color must be three RGB bytes".to_owned()
                    })
                })
                .transpose()?,
            align: parse_table_cell_align(value.get("align"))?,
        }),
        Some("image") => Ok(TableCellBlock::Image(parse_table_cell_image(value)?)),
        Some("spacer") => Ok(TableCellBlock::Spacer {
            height: value
                .get("height")
                .and_then(serde_json::Value::as_f64)
                .ok_or("table spacer height must be numeric")?,
        }),
        Some("rule") => Ok(TableCellBlock::Rule {
            thickness: value
                .get("thickness")
                .and_then(serde_json::Value::as_f64)
                .ok_or("table rule thickness must be numeric")?,
            color: value
                .get("color")
                .map(|color| {
                    serde_json::from_value(color.clone())
                        .map_err(|_| "table rule color must be three RGB bytes".to_owned())
                })
                .transpose()?,
        }),
        Some("table") => {
            let grid = value.get("table").ok_or("table block table is required")?;
            let row_count = grid
                .get("row_count")
                .and_then(serde_json::Value::as_u64)
                .and_then(|number| usize::try_from(number).ok())
                .ok_or("nested table row_count must be a nonnegative integer")?;
            let column_widths = serde_json::from_value(
                grid.get("column_widths")
                    .ok_or("nested table column_widths are required")?
                    .clone(),
            )
            .map_err(|_| "nested table column_widths must be numbers")?;
            let cells = parse_table_cells(
                grid.get("cells").ok_or("nested table cells are required")?,
                depth + 1,
            )?;
            let number = |key: &str, default: f64| -> Result<f64, String> {
                grid.get(key)
                    .map(|value| {
                        value
                            .as_f64()
                            .ok_or_else(|| format!("nested table {key} must be numeric"))
                    })
                    .unwrap_or(Ok(default))
            };
            let border_color = grid
                .get("border_color")
                .map(|color| {
                    serde_json::from_value(color.clone())
                        .map_err(|_| "nested table border_color must be three RGB bytes".to_owned())
                })
                .transpose()?
                .unwrap_or([0, 0, 0]);
            Ok(TableCellBlock::Table(Box::new(NestedTable {
                row_count,
                column_widths,
                cells,
                row_height: number("row_height", 0.0)?,
                padding: number("padding", 1.0)?,
                border_width: number("border_width", 0.2)?,
                border_color,
            })))
        }
        _ => Err("table cell block type must be text, image, spacer, rule, or table".into()),
    }
}

/// Append a fixed-grid table. Returns appended page indexes as JSON.
///
/// `table_json` requires `column_widths` and either `rows` (array of string
/// arrays) or `row_count` with `cells` (positioned spanning cells). Other
/// optional fields match `TableStyle`.
///
/// # Safety
/// `handle` must be live, `table_json` NUL-terminated, and `output` writable.
#[no_mangle]
pub unsafe extern "C" fn ofd_add_table_json(
    handle: *mut OfdDocument,
    document_index: usize,
    table_json: *const c_char,
    output: *mut OfdBuffer,
) -> i32 {
    run(|| {
        if output.is_null() {
            return Err("null table output".into());
        }
        let input: serde_json::Value =
            serde_json::from_str(input_string(table_json)?).map_err(|error| error.to_string())?;
        let mut style = TableStyle {
            column_widths: serde_json::from_value(
                input
                    .get("column_widths")
                    .ok_or("column_widths are required")?
                    .clone(),
            )
            .map_err(|_| "column_widths must be numbers")?,
            ..TableStyle::default()
        };
        for (key, target) in [
            ("row_height", &mut style.row_height),
            ("font_size", &mut style.font_size),
            ("line_height", &mut style.line_height),
            ("padding", &mut style.padding),
            ("border_width", &mut style.border_width),
        ] {
            if let Some(value) = input.get(key) {
                *target = value
                    .as_f64()
                    .ok_or_else(|| format!("{key} must be a number"))?;
            }
        }
        if let Some(value) = input.get("page_size") {
            style.page_size = serde_json::from_value(value.clone())
                .map_err(|_| "page_size must contain two numbers")?;
        }
        if let Some(value) = input.get("margins") {
            style.margins = serde_json::from_value(value.clone())
                .map_err(|_| "margins must contain four numbers")?;
        }
        if let Some(value) = input.get("border_color") {
            style.border_color = serde_json::from_value(value.clone())
                .map_err(|_| "border_color must be three RGB bytes")?;
        }
        if let Some(value) = input.get("font_id") {
            style.font_id = Some(value.as_str().ok_or("font_id must be a string")?.to_owned());
        }
        if let Some(value) = input.get("repeat_header") {
            style.repeat_header = value.as_bool().ok_or("repeat_header must be a boolean")?;
        }
        let indexes = if let Some(values) = input.get("cells") {
            if input.get("rows").is_some() {
                return Err("table rows and cells are mutually exclusive".into());
            }
            let row_count = input
                .get("row_count")
                .and_then(serde_json::Value::as_u64)
                .and_then(|value| usize::try_from(value).ok())
                .ok_or("row_count must be a nonnegative integer")?;
            let cells = parse_table_cells(values, 0)?;
            document_mut(handle)?
                .add_table_cells(document_index, row_count, &cells, style)
                .map_err(|error| error.to_string())?
        } else {
            let rows: Vec<Vec<String>> =
                serde_json::from_value(input.get("rows").ok_or("table rows are required")?.clone())
                    .map_err(|_| "table rows must be arrays of strings")?;
            document_mut(handle)?
                .add_table(document_index, &rows, style)
                .map_err(|error| error.to_string())?
        };
        output_buffer(
            serde_json::to_vec(&indexes).map_err(|error| error.to_string())?,
            output,
        )
    })
}

/// Flow plain text into new pages. Returns page indexes as a JSON array.
///
/// `flow_json` accepts `text` and optional `page_size`, `margins`,
/// `font_size`, `line_height`, `paragraph_gap`, `keep_paragraphs_together`,
/// `font_id`, `first_line_indent`,
/// `first_line_indent_chars`, `indent_each_paragraph`, and `shape` fields.
///
/// # Safety
/// `handle` must be live, `flow_json` NUL-terminated, and `output` writable.
#[no_mangle]
pub unsafe extern "C" fn ofd_add_flowed_text_json(
    handle: *mut OfdDocument,
    document_index: usize,
    flow_json: *const c_char,
    output: *mut OfdBuffer,
) -> i32 {
    run(|| {
        let input: serde_json::Value =
            serde_json::from_str(input_string(flow_json)?).map_err(|e| e.to_string())?;
        let text = input
            .get("text")
            .and_then(serde_json::Value::as_str)
            .ok_or("flowed text must be a string")?;
        let style = parse_flow_style(&input)?;
        let indexes = document_mut(handle)?
            .add_flowed_text(document_index, text, style)
            .map_err(|e| e.to_string())?;
        output_buffer(
            serde_json::to_vec(&indexes).map_err(|e| e.to_string())?,
            output,
        )
    })
}

/// Flow styled spans into new pages. Returns page indexes as a JSON array.
///
/// `flow_json` accepts `spans`, whose entries contain `text` and optional
/// `font_id`, `font_size`, `fill_color`, `fill`, `stroke`, `stroke_color`,
/// `stroke_width`, `bold`, `weight`, `italic`, `letter_spacing`,
/// `underline`, `underline_offset`, `underline_width`, `line_break_after`,
/// and `keep_together` fields. Page style
/// fields are the same as for `ofd_add_flowed_text_json`.
///
/// # Safety
/// `handle` must be live, `flow_json` NUL-terminated, and `output` writable.
#[no_mangle]
pub unsafe extern "C" fn ofd_add_flowed_spans_json(
    handle: *mut OfdDocument,
    document_index: usize,
    flow_json: *const c_char,
    output: *mut OfdBuffer,
) -> i32 {
    run(|| {
        let input: serde_json::Value =
            serde_json::from_str(input_string(flow_json)?).map_err(|e| e.to_string())?;
        let style = parse_flow_style(&input)?;
        let items = input
            .get("spans")
            .and_then(serde_json::Value::as_array)
            .ok_or("spans must be an array")?;
        let mut spans = Vec::with_capacity(items.len());
        for item in items {
            let text = item
                .get("text")
                .and_then(serde_json::Value::as_str)
                .ok_or("span text must be a string")?;
            let mut span = FlowSpan::new(text);
            if let Some(value) = item.get("font_id") {
                span.font_id = Some(
                    value
                        .as_str()
                        .ok_or("span font_id must be a string")?
                        .into(),
                );
            }
            if let Some(value) = item.get("font_size") {
                span.font_size = Some(value.as_f64().ok_or("span font_size must be a number")?);
            }
            if let Some(value) = item.get("language") {
                span.language = Some(
                    value
                        .as_str()
                        .ok_or("span language must be a string")?
                        .to_owned(),
                );
            }
            if let Some(value) = item.get("fill_color") {
                span.fill_color = Some(
                    serde_json::from_value(value.clone())
                        .map_err(|_| "span fill_color must contain three RGB bytes")?,
                );
            }
            if let Some(value) = item.get("fill") {
                span.fill = value.as_bool().ok_or("span fill must be a boolean")?;
            }
            if let Some(value) = item.get("stroke") {
                span.stroke = value.as_bool().ok_or("span stroke must be a boolean")?;
            }
            if let Some(value) = item.get("stroke_color") {
                span.stroke_color = Some(
                    serde_json::from_value(value.clone())
                        .map_err(|_| "span stroke_color must contain three RGB bytes")?,
                );
            }
            if let Some(value) = item.get("stroke_width") {
                span.stroke_width =
                    Some(value.as_f64().ok_or("span stroke_width must be a number")?);
            }
            if let Some(value) = item.get("bold") {
                span.bold = value.as_bool().ok_or("span bold must be a boolean")?;
            }
            if let Some(value) = item.get("weight") {
                span.weight = Some(
                    value
                        .as_u64()
                        .and_then(|weight| weight.try_into().ok())
                        .ok_or("span weight must be an unsigned 16-bit integer")?,
                );
            }
            if let Some(value) = item.get("italic") {
                span.italic = value.as_bool().ok_or("span italic must be a boolean")?;
            }
            if let Some(value) = item.get("letter_spacing") {
                span.letter_spacing = value
                    .as_f64()
                    .ok_or("span letter_spacing must be a number")?;
            }
            if let Some(value) = item.get("underline") {
                span.underline = value.as_bool().ok_or("span underline must be a boolean")?;
            }
            if let Some(value) = item.get("underline_offset") {
                span.underline_offset = value
                    .as_f64()
                    .ok_or("span underline_offset must be a number")?;
            }
            if let Some(value) = item.get("underline_width") {
                span.underline_width = value
                    .as_f64()
                    .ok_or("span underline_width must be a number")?;
            }
            if let Some(value) = item.get("line_break_after") {
                span.line_break_after = value
                    .as_bool()
                    .ok_or("span line_break_after must be a boolean")?;
            }
            if let Some(value) = item.get("keep_together") {
                span.keep_together = value
                    .as_bool()
                    .ok_or("span keep_together must be a boolean")?;
            }
            spans.push(span);
        }
        let indexes = document_mut(handle)?
            .add_flowed_spans(document_index, &spans, style)
            .map_err(|e| e.to_string())?;
        output_buffer(
            serde_json::to_vec(&indexes).map_err(|e| e.to_string())?,
            output,
        )
    })
}

/// Append a text page using a selected document font.
///
/// # Safety
/// `handle` must be live and `text` a valid NUL-terminated string.
#[no_mangle]
pub unsafe extern "C" fn ofd_append_text_page_with_font(
    handle: *mut OfdDocument,
    document_index: usize,
    text: *const c_char,
    width_mm: f64,
    height_mm: f64,
    font_id: usize,
) -> i32 {
    run(|| {
        document_mut(handle)?
            .append_text_page_with_font(
                document_index,
                input_string(text)?,
                width_mm,
                height_mm,
                font_id,
            )
            .map_err(|e| e.to_string())
    })
}

/// Append a vector path page to a document body in an existing package.
///
/// # Safety
/// `handle` must be live and `commands` must be a valid NUL-terminated string.
#[no_mangle]
pub unsafe extern "C" fn ofd_append_path_page(
    handle: *mut OfdDocument,
    document_index: usize,
    commands: *const c_char,
    x: f64,
    y: f64,
    width: f64,
    height: f64,
    page_width: f64,
    page_height: f64,
    fill: i32,
    stroke: i32,
) -> i32 {
    run(|| {
        document_mut(handle)?
            .append_path_page(
                document_index,
                input_string(commands)?,
                [x, y, width, height],
                [page_width, page_height],
                fill != 0,
                stroke != 0,
            )
            .map_err(|error| error.to_string())
    })
}

/// Append an image page to a document body in an existing package.
///
/// # Safety
/// `handle` must be live, `format` NUL-terminated, and `data` valid for `len` bytes.
#[no_mangle]
pub unsafe extern "C" fn ofd_append_image_page(
    handle: *mut OfdDocument,
    document_index: usize,
    data: *const u8,
    len: usize,
    format: *const c_char,
    width_mm: f64,
    height_mm: f64,
) -> i32 {
    run(|| {
        document_mut(handle)?
            .append_image_page(
                document_index,
                input_bytes(data, len)?,
                input_string(format)?,
                [width_mm, height_mm],
            )
            .map_err(|error| error.to_string())
    })
}

/// Add a PNG or JPEG image page to a newly created package.
///
/// # Safety
/// `handle` must be live, `format` NUL-terminated, and `data` valid for `len` bytes.
#[no_mangle]
pub unsafe extern "C" fn ofd_add_image_page(
    handle: *mut OfdDocument,
    data: *const u8,
    len: usize,
    format: *const c_char,
    width_mm: f64,
    height_mm: f64,
) -> i32 {
    run(|| {
        document_mut(handle)?
            .add_image_page(
                input_bytes(data, len)?,
                input_string(format)?,
                width_mm,
                height_mm,
            )
            .map_err(|e| e.to_string())
    })
}

/// Add a PNG or JPEG image object to a page.
///
/// # Safety
/// `handle` must be live, `format` NUL-terminated, `data` valid for `len`
/// bytes, and `output_id` writable.
#[no_mangle]
pub unsafe extern "C" fn ofd_add_image_object(
    handle: *mut OfdDocument,
    page_index: usize,
    data: *const u8,
    len: usize,
    format: *const c_char,
    x: f64,
    y: f64,
    width: f64,
    height: f64,
    output_id: *mut usize,
) -> i32 {
    run(|| {
        let output = output_id.as_mut().ok_or("null image object ID output")?;
        *output = 0;
        let id = document_mut(handle)?
            .add_image_object(
                page_index,
                input_bytes(data, len)?,
                input_string(format)?,
                [x, y, width, height],
            )
            .map_err(|error| error.to_string())?;
        *output = id.parse().map_err(|_| "invalid generated object ID")?;
        Ok(())
    })
}

/// Add a PNG, JPEG, or JBIG2 image as a composite graphic resource.
///
/// # Safety
/// `handle` must be live, `format` NUL-terminated, `data` valid for `len`
/// bytes, and `output_id` writable.
#[no_mangle]
pub unsafe extern "C" fn ofd_add_composite_image_object(
    handle: *mut OfdDocument,
    page_index: usize,
    data: *const u8,
    len: usize,
    format: *const c_char,
    x: f64,
    y: f64,
    width: f64,
    height: f64,
    output_id: *mut usize,
) -> i32 {
    run(|| {
        let output = output_id
            .as_mut()
            .ok_or("null composite image object ID output")?;
        *output = 0;
        let id = document_mut(handle)?
            .add_composite_image_object(
                page_index,
                input_bytes(data, len)?,
                input_string(format)?,
                [x, y, width, height],
            )
            .map_err(|error| error.to_string())?;
        *output = id.parse().map_err(|_| "invalid generated object ID")?;
        Ok(())
    })
}

/// Add one vector path page to a newly created package.
///
/// # Safety
/// `handle` must be live and `commands` a valid NUL-terminated string.
#[no_mangle]
pub unsafe extern "C" fn ofd_add_path_page(
    handle: *mut OfdDocument,
    commands: *const c_char,
    x: f64,
    y: f64,
    width: f64,
    height: f64,
    page_width: f64,
    page_height: f64,
    fill: i32,
    stroke: i32,
) -> i32 {
    run(|| {
        document_mut(handle)?
            .add_path_page(
                input_string(commands)?,
                [x, y, width, height],
                page_width,
                page_height,
                fill != 0,
                stroke != 0,
            )
            .map_err(|e| e.to_string())
    })
}

/// Add an embedded attachment to a newly created document.
///
/// # Safety
/// `handle` must be live, `name` NUL-terminated, and `data` valid for `len` bytes.
#[no_mangle]
pub unsafe extern "C" fn ofd_add_attachment(
    handle: *mut OfdDocument,
    name: *const c_char,
    data: *const u8,
    len: usize,
) -> i32 {
    run(|| {
        document_mut(handle)?
            .add_attachment(input_string(name)?, input_bytes(data, len)?)
            .map_err(|e| e.to_string())
    })
}

/// Add an Image, Audio or Video multimedia resource to a document body.
///
/// # Safety
/// `handle` must be live, strings NUL-terminated, `data` valid for `len`
/// bytes, and `output_id` writable.
#[no_mangle]
pub unsafe extern "C" fn ofd_add_multimedia_resource(
    handle: *mut OfdDocument,
    document_index: usize,
    media_type: *const c_char,
    format: *const c_char,
    data: *const u8,
    len: usize,
    output_id: *mut usize,
) -> i32 {
    run(|| {
        let output = output_id.as_mut().ok_or("null media resource ID output")?;
        *output = 0;
        let id = document_mut(handle)?
            .add_multimedia_resource(
                document_index,
                input_string(media_type)?,
                input_string(format)?,
                input_bytes(data, len)?,
            )
            .map_err(|error| error.to_string())?;
        *output = id.parse().map_err(|_| "invalid media resource ID")?;
        Ok(())
    })
}

/// Add an Image, Audio or Video resource scoped to one page.
///
/// # Safety
/// `handle` must be live, strings NUL-terminated, `data` valid for `len`
/// bytes, and `output_id` writable.
#[no_mangle]
pub unsafe extern "C" fn ofd_add_page_multimedia_resource(
    handle: *mut OfdDocument,
    page_index: usize,
    media_type: *const c_char,
    format: *const c_char,
    data: *const u8,
    len: usize,
    output_id: *mut usize,
) -> i32 {
    run(|| {
        let output = output_id.as_mut().ok_or("null media resource ID output")?;
        *output = 0;
        let id = document_mut(handle)?
            .add_page_multimedia_resource(
                page_index,
                input_string(media_type)?,
                input_string(format)?,
                input_bytes(data, len)?,
            )
            .map_err(|error| error.to_string())?;
        *output = id.parse().map_err(|_| "invalid media resource ID")?;
        Ok(())
    })
}

/// Replace the bytes of one document-level multimedia resource.
///
/// # Safety
/// `handle` must be live, `resource_id` NUL-terminated, and `data` valid for `len` bytes.
#[no_mangle]
pub unsafe extern "C" fn ofd_replace_document_multimedia_data(
    handle: *mut OfdDocument,
    document_index: usize,
    resource_id: *const c_char,
    data: *const u8,
    len: usize,
) -> i32 {
    run(|| {
        document_mut(handle)?
            .replace_document_multimedia_data(
                document_index,
                input_string(resource_id)?,
                input_bytes(data, len)?,
            )
            .map_err(|error| error.to_string())
    })
}

/// Replace the bytes of a multimedia resource visible to a page.
///
/// # Safety
/// `handle` must be live, `resource_id` NUL-terminated, and `data` valid for `len` bytes.
#[no_mangle]
pub unsafe extern "C" fn ofd_replace_page_multimedia_data(
    handle: *mut OfdDocument,
    page_index: usize,
    resource_id: *const c_char,
    data: *const u8,
    len: usize,
) -> i32 {
    run(|| {
        document_mut(handle)?
            .replace_page_multimedia_data(
                page_index,
                input_string(resource_id)?,
                input_bytes(data, len)?,
            )
            .map_err(|error| error.to_string())
    })
}

/// Remove an unreferenced document-level multimedia resource definition.
///
/// # Safety
/// `handle` must be live and `resource_id` NUL-terminated.
#[no_mangle]
pub unsafe extern "C" fn ofd_remove_document_multimedia_resource(
    handle: *mut OfdDocument,
    document_index: usize,
    resource_id: *const c_char,
) -> i32 {
    run(|| {
        document_mut(handle)?
            .remove_document_multimedia_resource(document_index, input_string(resource_id)?)
            .map_err(|error| error.to_string())
    })
}

/// Remove an unreferenced page-local multimedia resource definition.
///
/// # Safety
/// `handle` must be live and `resource_id` NUL-terminated.
#[no_mangle]
pub unsafe extern "C" fn ofd_remove_page_multimedia_resource(
    handle: *mut OfdDocument,
    page_index: usize,
    resource_id: *const c_char,
) -> i32 {
    run(|| {
        document_mut(handle)?
            .remove_page_multimedia_resource(page_index, input_string(resource_id)?)
            .map_err(|error| error.to_string())
    })
}

/// Add an embedded attachment to a selected document body.
///
/// # Safety
/// `handle` must be live, `name` NUL-terminated, and `data` valid for `len` bytes.
#[no_mangle]
pub unsafe extern "C" fn ofd_add_attachment_to_document(
    handle: *mut OfdDocument,
    document_index: usize,
    name: *const c_char,
    data: *const u8,
    len: usize,
) -> i32 {
    run(|| {
        document_mut(handle)?
            .add_attachment_to_document(
                document_index,
                input_string(name)?,
                input_bytes(data, len)?,
            )
            .map_err(|e| e.to_string())
    })
}

/// Remove one attachment from the package's attachment lists by global index.
///
/// # Safety
/// `handle` must be a live document handle.
#[no_mangle]
pub unsafe extern "C" fn ofd_remove_attachment(handle: *mut OfdDocument, index: usize) -> i32 {
    run(|| {
        document_mut(handle)?
            .remove_attachment(index)
            .map_err(|error| error.to_string())
    })
}

/// Rename an attachment by global index while retaining its ID and bytes.
///
/// # Safety
/// `handle` must be live and `name` NUL-terminated.
#[no_mangle]
pub unsafe extern "C" fn ofd_rename_attachment(
    handle: *mut OfdDocument,
    index: usize,
    name: *const c_char,
) -> i32 {
    run(|| {
        document_mut(handle)?
            .rename_attachment(index, input_string(name)?)
            .map_err(|error| error.to_string())
    })
}

/// Replace attachment bytes by global index while retaining its ID.
///
/// # Safety
/// `handle` must be live and `data` valid for `len` bytes.
#[no_mangle]
pub unsafe extern "C" fn ofd_replace_attachment_data(
    handle: *mut OfdDocument,
    index: usize,
    data: *const u8,
    len: usize,
) -> i32 {
    run(|| {
        document_mut(handle)?
            .replace_attachment_data(index, input_bytes(data, len)?)
            .map_err(|error| error.to_string())
    })
}

/// Replace standard attachment metadata by global index from JSON.
///
/// Optional string fields are `format`, `creation_date`, `mod_date`, and
/// `usage`; omitted or null values clear them. `visible` defaults to true.
///
/// # Safety
/// `handle` must be live and `metadata_json` NUL-terminated.
#[no_mangle]
pub unsafe extern "C" fn ofd_set_attachment_metadata_json(
    handle: *mut OfdDocument,
    index: usize,
    metadata_json: *const c_char,
) -> i32 {
    run(|| {
        let input: serde_json::Value = serde_json::from_str(input_string(metadata_json)?)
            .map_err(|error| error.to_string())?;
        let object = input
            .as_object()
            .ok_or("attachment metadata must be a JSON object")?;
        let optional = |name: &str| -> Result<Option<String>, String> {
            match object.get(name) {
                None | Some(serde_json::Value::Null) => Ok(None),
                Some(serde_json::Value::String(value)) => Ok(Some(value.clone())),
                _ => Err(format!("attachment {name} must be a string or null")),
            }
        };
        let visible = match object.get("visible") {
            None => true,
            Some(serde_json::Value::Bool(value)) => *value,
            _ => return Err("attachment visible must be a boolean".into()),
        };
        document_mut(handle)?
            .set_attachment_metadata(
                index,
                AttachmentMetadata {
                    format: optional("format")?,
                    creation_date: optional("creation_date")?,
                    mod_date: optional("mod_date")?,
                    visible,
                    usage: optional("usage")?,
                },
            )
            .map_err(|error| error.to_string())
    })
}

/// Save an OFD package to disk.
///
/// # Safety
/// `handle` must be live and `path` must be a valid NUL-terminated string.
#[no_mangle]
pub unsafe extern "C" fn ofd_save(handle: *const OfdDocument, path: *const c_char) -> i32 {
    run(|| {
        document_ref(handle)?
            .save(input_string(path)?)
            .map_err(|e| e.to_string())
    })
}

/// Serialize an OFD package to ZIP bytes.
///
/// # Safety
/// `handle` must be live and `output` writable. Release the returned buffer
/// with `ofd_buffer_free`.
#[no_mangle]
pub unsafe extern "C" fn ofd_to_bytes(handle: *const OfdDocument, output: *mut OfdBuffer) -> i32 {
    run(|| {
        let bytes = document_ref(handle)?
            .to_bytes()
            .map_err(|error| error.to_string())?;
        output_buffer(bytes, output)
    })
}

/// Release a buffer allocated by this library.
///
/// # Safety
/// `buffer` must have been returned by this library and released only once.
#[no_mangle]
pub unsafe extern "C" fn ofd_buffer_free(buffer: OfdBuffer) {
    if !buffer.data.is_null() {
        drop(Box::from_raw(ptr::slice_from_raw_parts_mut(
            buffer.data,
            buffer.len,
        )));
    }
}

/// Release a returned buffer without passing a C struct by value.
///
/// # Safety
/// `data` and `len` must be the unchanged fields of a buffer returned by this
/// library, released only once. A null data pointer is accepted.
#[no_mangle]
pub unsafe extern "C" fn ofd_buffer_free_parts(data: *mut u8, len: usize) {
    ofd_buffer_free(OfdBuffer { data, len });
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;
    use std::sync::{Arc, Barrier};
    use tempfile::tempdir;

    #[test]
    fn ffi_releases_buffers_through_scalar_arguments() {
        let mut handle = ptr::null_mut();
        assert_eq!(unsafe { ofd_new(&mut handle) }, 0);
        let mut output = OfdBuffer {
            data: ptr::null_mut(),
            len: 0,
        };
        assert_eq!(unsafe { ofd_to_bytes(handle, &mut output) }, 0);
        assert!(unsafe { std::slice::from_raw_parts(output.data, output.len) }.starts_with(b"PK"));
        unsafe {
            ofd_buffer_free_parts(output.data, output.len);
            ofd_buffer_free_parts(ptr::null_mut(), 0);
            ofd_close(handle);
        }
    }

    #[test]
    fn ffi_serializes_shared_handle_calls_and_rejects_closed_tokens() {
        let mut handle = ptr::null_mut();
        assert_eq!(unsafe { ofd_new(&mut handle) }, 0);
        let token = handle.addr();
        let start = Arc::new(Barrier::new(9));
        std::thread::scope(|scope| {
            for _ in 0..8 {
                let start = Arc::clone(&start);
                scope.spawn(move || {
                    start.wait();
                    assert_eq!(
                        unsafe {
                            ofd_add_blank_page(ptr::without_provenance_mut(token), 10.0, 10.0)
                        },
                        0
                    );
                });
            }
            start.wait();
        });
        let mut count = 0;
        assert_eq!(unsafe { ofd_page_count(handle, &mut count) }, 0);
        assert_eq!(count, 8);

        unsafe { ofd_close(handle) };
        assert_eq!(unsafe { ofd_page_count(handle, &mut count) }, 2);
        assert!(unsafe { CStr::from_ptr(ofd_last_error()) }
            .to_str()
            .unwrap()
            .contains("closed or invalid"));
        unsafe { ofd_close(handle) };

        let mut next = ptr::null_mut();
        assert_eq!(unsafe { ofd_new(&mut next) }, 0);
        assert_ne!(next, handle);
        assert_eq!(unsafe { ofd_page_count(handle, &mut count) }, 2);
        unsafe { ofd_close(next) };
    }

    #[test]
    fn ffi_close_can_race_with_readers() {
        let mut handle = ptr::null_mut();
        assert_eq!(unsafe { ofd_new(&mut handle) }, 0);
        let token = handle.addr();
        let start = Arc::new(Barrier::new(9));
        std::thread::scope(|scope| {
            for _ in 0..8 {
                let start = Arc::clone(&start);
                scope.spawn(move || {
                    start.wait();
                    let mut count = 0;
                    let status =
                        unsafe { ofd_page_count(ptr::without_provenance(token), &mut count) };
                    assert!(status == 0 || status == 2);
                    if status == 0 {
                        assert_eq!(count, 0);
                    }
                });
            }
            start.wait();
            unsafe { ofd_close(handle) };
        });
        let mut count = 0;
        assert_eq!(unsafe { ofd_page_count(handle, &mut count) }, 2);
    }

    #[test]
    fn ffi_all_text_preserves_page_order_and_empty_pages() {
        let mut handle = ptr::null_mut();
        assert_eq!(unsafe { ofd_new(&mut handle) }, 0);
        assert_eq!(
            unsafe { ofd_add_text_page(handle, c"首页".as_ptr(), 100.0, 100.0) },
            0
        );
        assert_eq!(unsafe { ofd_add_blank_page(handle, 100.0, 100.0) }, 0);
        assert_eq!(
            unsafe { ofd_add_text_page(handle, c"last".as_ptr(), 100.0, 100.0) },
            0
        );
        let mut output = OfdBuffer {
            data: ptr::null_mut(),
            len: 0,
        };
        assert_eq!(unsafe { ofd_all_text_json(handle, &mut output) }, 0);
        let actual: Vec<Vec<String>> =
            serde_json::from_slice(unsafe { std::slice::from_raw_parts(output.data, output.len) })
                .unwrap();
        assert_eq!(actual, vec![vec!["首页"], vec![], vec!["last"]]);
        unsafe {
            ofd_buffer_free(output);
            ofd_close(handle);
        }
    }

    #[test]
    fn ffi_reports_supported_archival_violations() {
        let mut handle = ptr::null_mut();
        assert_eq!(unsafe { ofd_new(&mut handle) }, 0);
        assert_eq!(
            unsafe { ofd_add_text_page(handle, c"archive".as_ptr(), 100.0, 100.0) },
            0
        );
        let mut output = OfdBuffer {
            data: ptr::null_mut(),
            len: 0,
        };
        assert_eq!(
            unsafe { ofd_archive_violations_json(handle, &mut output) },
            0
        );
        let values: serde_json::Value =
            serde_json::from_slice(unsafe { std::slice::from_raw_parts(output.data, output.len) })
                .unwrap();
        assert_eq!(values[0]["rule"], "DOC_TYPE");
        assert_eq!(values[0]["severity"], "ERROR");
        unsafe { ofd_buffer_free(output) };
        unsafe { ofd_close(handle) };
    }

    #[test]
    fn ffi_prepares_archive_as_separate_document() {
        let mut handle = ptr::null_mut();
        assert_eq!(unsafe { ofd_new(&mut handle) }, 0);
        assert_eq!(
            unsafe { ofd_add_text_page(handle, c"archive".as_ptr(), 100.0, 100.0) },
            0
        );
        let mut prepared = ptr::null_mut();
        assert_eq!(unsafe { ofd_prepare_archive(handle, &mut prepared) }, 0);
        assert!(!prepared.is_null());
        assert_ne!(prepared, handle);
        assert!(unsafe { document_ref(handle).unwrap() }
            .archive_violations()
            .unwrap()
            .iter()
            .any(|issue| issue.rule == "DOC_TYPE"));
        assert!(unsafe { document_ref(prepared).unwrap() }
            .archive_violations()
            .unwrap()
            .iter()
            .all(|issue| issue.rule != "DOC_TYPE"));
        unsafe { ofd_close(prepared) };
        unsafe { ofd_close(handle) };
    }

    #[test]
    fn json_text_writers_preserve_embedded_nul() {
        fn request(value: serde_json::Value) -> CString {
            CString::new(value.to_string()).unwrap()
        }
        let mut handle = ptr::null_mut();
        assert_eq!(unsafe { ofd_new(&mut handle) }, 0);
        let add = request(serde_json::json!({
            "operation": "add", "text": "A\0B", "width_mm": 100.0, "height_mm": 100.0
        }));
        assert_eq!(
            unsafe { ofd_write_text_page_json(handle, add.as_ptr(), ptr::null_mut()) },
            0
        );
        assert_eq!(
            unsafe { document_ref(handle).unwrap() }
                .page_text(0)
                .unwrap(),
            ["A\0B"]
        );
        let id = unsafe { document_ref(handle).unwrap() }
            .page_text_objects(0)
            .unwrap()[0]
            .id
            .clone()
            .unwrap();
        let set = request(serde_json::json!({
            "operation": "set", "page_index": 0, "object_id": id,
            "code_index": 0, "text": "C\0D"
        }));
        assert_eq!(unsafe { ofd_edit_text_code_json(handle, set.as_ptr()) }, 0);
        let append_code = request(serde_json::json!({
            "operation": "append", "page_index": 0, "object_id": id,
            "text": "E\0F", "layout": {}
        }));
        assert_eq!(
            unsafe { ofd_edit_text_code_json(handle, append_code.as_ptr()) },
            0
        );
        assert_eq!(
            unsafe { document_ref(handle).unwrap() }
                .page_text(0)
                .unwrap(),
            ["C\0D", "E\0F"]
        );

        let append_page = request(serde_json::json!({
            "operation": "append", "document_index": 0, "text": "G\0H",
            "width_mm": 100.0, "height_mm": 100.0
        }));
        assert_eq!(
            unsafe { ofd_write_text_page_json(handle, append_page.as_ptr(), ptr::null_mut()) },
            0
        );
        let insert = request(serde_json::json!({
            "operation": "insert", "document_index": 0, "position": 1,
            "text": "I\0J", "width_mm": 100.0, "height_mm": 100.0
        }));
        let mut index = usize::MAX;
        assert_eq!(
            unsafe { ofd_write_text_page_json(handle, insert.as_ptr(), &mut index) },
            0
        );
        assert_eq!(index, 1);
        assert_eq!(
            unsafe { document_ref(handle).unwrap() }
                .page_text(1)
                .unwrap(),
            ["I\0J"]
        );
        assert_eq!(
            unsafe { document_ref(handle).unwrap() }
                .page_text(2)
                .unwrap(),
            ["G\0H"]
        );

        let bad = request(serde_json::json!({"operation": "unknown"}));
        assert_ne!(
            unsafe { ofd_write_text_page_json(handle, bad.as_ptr(), ptr::null_mut()) },
            0
        );
        assert_eq!(
            unsafe { document_ref(handle).unwrap() }
                .pages()
                .unwrap()
                .len(),
            3
        );
        unsafe { ofd_close(handle) };
    }

    #[test]
    fn ffi_imports_type1_and_cid_tounicode_text() {
        for bytes in [
            include_bytes!("../../ofdrw/tests/fixtures/type1-tounicode.pdf").as_slice(),
            include_bytes!("../../ofdrw/tests/fixtures/type1-subset-tounicode.pdf").as_slice(),
            include_bytes!("../../ofdrw/tests/fixtures/rotated-tounicode.pdf").as_slice(),
            include_bytes!("../../ofdrw/tests/fixtures/metadata-tounicode.pdf").as_slice(),
            include_bytes!("../../ofdrw/tests/fixtures/outline-tounicode.pdf").as_slice(),
            include_bytes!("../../ofdrw/tests/fixtures/type0-identity-tounicode.pdf").as_slice(),
        ] {
            let mut handle = ptr::null_mut();
            assert_eq!(
                unsafe { ofd_from_pdf_bytes(bytes.as_ptr(), bytes.len(), &mut handle) },
                0
            );
            let mut output = OfdBuffer {
                data: ptr::null_mut(),
                len: 0,
            };
            assert_eq!(unsafe { ofd_page_text_json(handle, 0, &mut output) }, 0);
            let text: serde_json::Value = serde_json::from_slice(unsafe {
                std::slice::from_raw_parts(output.data, output.len)
            })
            .unwrap();
            assert_eq!(text, serde_json::json!(["中文"]));
            unsafe {
                ofd_buffer_free(output);
                ofd_close(handle);
            }
        }
    }

    #[test]
    fn ffi_imports_pdf_document_info() {
        let bytes = include_bytes!("../../ofdrw/tests/fixtures/metadata-tounicode.pdf");
        let mut handle = ptr::null_mut();
        assert_eq!(
            unsafe { ofd_from_pdf_bytes(bytes.as_ptr(), bytes.len(), &mut handle) },
            0
        );
        let mut output = OfdBuffer {
            data: ptr::null_mut(),
            len: 0,
        };
        assert_eq!(unsafe { ofd_documents_json(handle, &mut output) }, 0);
        let infos: serde_json::Value =
            serde_json::from_slice(unsafe { std::slice::from_raw_parts(output.data, output.len) })
                .unwrap();
        assert_eq!(infos[0]["title"], "示例文档");
        assert_eq!(infos[0]["creation_date"], "2024-02-29");
        unsafe {
            ofd_buffer_free(output);
            ofd_close(handle);
        }
    }

    #[test]
    fn ffi_imports_pdf_outline_tree() {
        let bytes = include_bytes!("../../ofdrw/tests/fixtures/outline-tounicode.pdf");
        let mut handle = ptr::null_mut();
        assert_eq!(
            unsafe { ofd_from_pdf_bytes(bytes.as_ptr(), bytes.len(), &mut handle) },
            0
        );
        let mut output = OfdBuffer {
            data: ptr::null_mut(),
            len: 0,
        };
        assert_eq!(unsafe { ofd_outlines_json(handle, &mut output) }, 0);
        let outlines: serde_json::Value =
            serde_json::from_slice(unsafe { std::slice::from_raw_parts(output.data, output.len) })
                .unwrap();
        assert_eq!(outlines[0]["title"], "章节一");
        assert_eq!(outlines[0]["expanded"], false);
        assert_eq!(outlines[1]["parent_index"], 0);
        assert_eq!(outlines[2]["title"], "Chapter 2");
        unsafe {
            ofd_buffer_free(output);
            ofd_close(handle);
        }
    }

    #[test]
    fn ffi_mixed_recipients_unlock_with_either_credential() {
        let plain = register_document(OfdDocument::new()).unwrap();
        let certificate = include_bytes!("../../ofdrw/tests/fixtures/java-sm2-cert.der");
        let recipients = serde_json::json!([
            {"type": "password", "username": "password", "password": "secret"},
            {"type": "certificate", "username": "certificate", "certificate_der_base64": STANDARD.encode(certificate)},
        ]);
        let recipients = CString::new(recipients.to_string()).unwrap();
        let mut encrypted = ptr::null_mut();
        assert_eq!(
            unsafe { ofd_encrypt_with_recipients_json(plain, recipients.as_ptr(), &mut encrypted) },
            0
        );
        let password = CString::new("secret").unwrap();
        let mut by_password = ptr::null_mut();
        assert_eq!(
            unsafe {
                ofd_decrypt_with_password(
                    encrypted,
                    password.as_ptr(),
                    ptr::null(),
                    &mut by_password,
                )
            },
            0
        );
        let key = include_bytes!("../../ofdrw/tests/fixtures/java-sm2-private-key.bin");
        let mut by_certificate = ptr::null_mut();
        assert_eq!(
            unsafe {
                ofd_decrypt_with_sm2_private_key(
                    encrypted,
                    key.as_ptr(),
                    key.len(),
                    ptr::null(),
                    &mut by_certificate,
                )
            },
            0
        );
        assert_eq!(
            unsafe { document_ref(by_password).unwrap() }
                .entry("OFD.xml")
                .unwrap(),
            unsafe { document_ref(by_certificate).unwrap() }
                .entry("OFD.xml")
                .unwrap()
        );
        let bad = CString::new(r#"[{"type":"unknown","username":"x"}]"#).unwrap();
        let mut rejected = ptr::null_mut();
        assert_ne!(
            unsafe { ofd_encrypt_with_recipients_json(plain, bad.as_ptr(), &mut rejected) },
            0
        );
        assert!(rejected.is_null());
        unsafe {
            ofd_close(plain);
            ofd_close(encrypted);
            ofd_close(by_password);
            ofd_close(by_certificate);
        }
    }

    #[test]
    fn ffi_certificate_encryption_interoperates_with_java() {
        let document = OfdDocument::from_bytes(include_bytes!(
            "../../ofdrw/tests/fixtures/java-cert-encrypted.ofd"
        ))
        .unwrap();
        let encrypted = register_document(document).unwrap();
        let key = include_bytes!("../../ofdrw/tests/fixtures/java-sm2-private-key.bin");
        let wrong = CString::new("other").unwrap();
        let mut plain = ptr::null_mut();
        assert_ne!(
            unsafe {
                ofd_decrypt_with_sm2_private_key(
                    encrypted,
                    key.as_ptr(),
                    key.len(),
                    wrong.as_ptr(),
                    &mut plain,
                )
            },
            0
        );
        assert!(plain.is_null());
        assert_eq!(
            unsafe {
                ofd_decrypt_with_sm2_private_key(
                    encrypted,
                    key.as_ptr(),
                    key.len(),
                    ptr::null(),
                    &mut plain,
                )
            },
            0
        );
        let certificate = include_bytes!("../../ofdrw/tests/fixtures/java-sm2-cert.der");
        let users = serde_json::json!([{
            "username": "ffi-user",
            "certificate_der_base64": STANDARD.encode(certificate),
        }]);
        let users = CString::new(users.to_string()).unwrap();
        let mut wrapped = ptr::null_mut();
        assert_eq!(
            unsafe { ofd_encrypt_with_certificates_json(plain, users.as_ptr(), &mut wrapped) },
            0
        );
        let mut restored = ptr::null_mut();
        assert_eq!(
            unsafe {
                ofd_decrypt_with_sm2_private_key(
                    wrapped,
                    key.as_ptr(),
                    key.len(),
                    ptr::null(),
                    &mut restored,
                )
            },
            0
        );
        assert_eq!(
            unsafe { document_ref(restored).unwrap() }
                .entry("OFD.xml")
                .unwrap(),
            unsafe { document_ref(plain).unwrap() }
                .entry("OFD.xml")
                .unwrap()
        );
        let invalid = CString::new(r#"[{"username":"x","certificate_der_base64":"!"}]"#).unwrap();
        let mut rejected = ptr::null_mut();
        assert_ne!(
            unsafe { ofd_encrypt_with_certificates_json(plain, invalid.as_ptr(), &mut rejected) },
            0
        );
        assert!(rejected.is_null());
        unsafe {
            ofd_close(encrypted);
            ofd_close(plain);
            ofd_close(wrapped);
            ofd_close(restored);
        }
    }

    #[test]
    fn ffi_decrypts_java_password_encrypted_document() {
        let temp = tempdir().unwrap();
        let path = temp.path().join("encrypted.ofd");
        std::fs::write(
            &path,
            include_bytes!("../../ofdrw/tests/fixtures/java-password-encrypted.ofd"),
        )
        .unwrap();
        let name = CString::new(path.to_string_lossy().as_bytes()).unwrap();
        let mut handle = ptr::null_mut();
        assert_eq!(unsafe { ofd_open(name.as_ptr(), &mut handle) }, 0);
        let mut encrypted = 0;
        assert_eq!(unsafe { ofd_is_encrypted(handle, &mut encrypted) }, 0);
        assert_eq!(encrypted, 1);
        let mut info = OfdBuffer {
            data: ptr::null_mut(),
            len: 0,
        };
        assert_eq!(unsafe { ofd_encryption_info_json(handle, &mut info) }, 0);
        let value: serde_json::Value =
            serde_json::from_slice(unsafe { std::slice::from_raw_parts(info.data, info.len) })
                .unwrap();
        unsafe { ofd_buffer_free(info) };
        assert_eq!(value[0]["usernames"], serde_json::json!(["777"]));
        let wrong = CString::new("wrong").unwrap();
        let mut decrypted = ptr::null_mut();
        assert_ne!(
            unsafe {
                ofd_decrypt_with_password(handle, wrong.as_ptr(), ptr::null(), &mut decrypted)
            },
            0
        );
        assert!(decrypted.is_null());
        let password = CString::new("12345678").unwrap();
        assert_eq!(
            unsafe {
                ofd_decrypt_with_password(handle, password.as_ptr(), ptr::null(), &mut decrypted)
            },
            0
        );
        assert!(!decrypted.is_null());
        let mut plaintext = 1;
        assert_eq!(unsafe { ofd_is_encrypted(decrypted, &mut plaintext) }, 0);
        assert_eq!(plaintext, 0);
        assert_eq!(
            unsafe { document_ref(decrypted).unwrap() }
                .pages()
                .unwrap()
                .len(),
            2
        );
        let users = CString::new(r#"[{"username":"alice","password":"first"},{"username":"bob","password":"second","user_type":"Owner"}]"#).unwrap();
        let mut encrypted_again = ptr::null_mut();
        assert_eq!(
            unsafe {
                ofd_encrypt_with_passwords_json(decrypted, users.as_ptr(), &mut encrypted_again)
            },
            0
        );
        let bob = CString::new("bob").unwrap();
        let second = CString::new("second").unwrap();
        let mut restored = ptr::null_mut();
        assert_eq!(
            unsafe {
                ofd_decrypt_with_password(
                    encrypted_again,
                    second.as_ptr(),
                    bob.as_ptr(),
                    &mut restored,
                )
            },
            0
        );
        assert_eq!(
            unsafe { document_ref(restored).unwrap() }
                .pages()
                .unwrap()
                .len(),
            2
        );
        let invalid = CString::new("[]").unwrap();
        let mut rejected = ptr::null_mut();
        assert_ne!(
            unsafe { ofd_encrypt_with_passwords_json(decrypted, invalid.as_ptr(), &mut rejected) },
            0
        );
        assert!(rejected.is_null());
        unsafe {
            ofd_close(restored);
            ofd_close(encrypted_again);
            ofd_close(decrypted);
            ofd_close(handle);
        }
    }

    #[test]
    fn ffi_color_spaces_round_trip_and_reject_invalid_palette() {
        let mut handle = ptr::null_mut();
        assert_eq!(unsafe { ofd_new(&mut handle) }, 0);
        assert_eq!(unsafe { ofd_add_blank_page(handle, 20.0, 20.0) }, 0);
        let spec = CString::new(r#"{"color_type":"RGB","palette":[[255,0,0],[0,0,255]]}"#).unwrap();
        let mut id = 0;
        assert_eq!(
            unsafe { ofd_add_document_color_space_json(handle, 0, spec.as_ptr(), &mut id) },
            0
        );
        assert!(id > 0);
        let mut output = OfdBuffer {
            data: ptr::null_mut(),
            len: 0,
        };
        assert_eq!(
            unsafe { ofd_document_color_spaces_json(handle, 0, &mut output) },
            0
        );
        let spaces: serde_json::Value =
            serde_json::from_slice(unsafe { std::slice::from_raw_parts(output.data, output.len) })
                .unwrap();
        unsafe { ofd_buffer_free(output) };
        assert_eq!(spaces[0]["id"], id.to_string());
        assert_eq!(spaces[0]["palette"][1], serde_json::json!([0, 0, 255]));
        let mut output = OfdBuffer {
            data: ptr::null_mut(),
            len: 0,
        };
        assert_eq!(
            unsafe { ofd_page_color_spaces_json(handle, 0, &mut output) },
            0
        );
        let page: serde_json::Value =
            serde_json::from_slice(unsafe { std::slice::from_raw_parts(output.data, output.len) })
                .unwrap();
        unsafe { ofd_buffer_free(output) };
        assert_eq!(page, spaces);
        let document_id = CString::new(id.to_string()).unwrap();
        assert_eq!(
            unsafe { ofd_set_document_default_color_space(handle, 0, document_id.as_ptr()) },
            0
        );
        let mut output = OfdBuffer {
            data: ptr::null_mut(),
            len: 0,
        };
        assert_eq!(
            unsafe { ofd_document_default_color_space_json(handle, 0, &mut output) },
            0
        );
        let default_id: serde_json::Value =
            serde_json::from_slice(unsafe { std::slice::from_raw_parts(output.data, output.len) })
                .unwrap();
        unsafe { ofd_buffer_free(output) };
        assert_eq!(default_id, id.to_string());
        let gray = CString::new(r#"{"color_type":"GRAY","bits_per_component":4}"#).unwrap();
        let mut page_id = 0;
        assert_eq!(
            unsafe { ofd_add_page_color_space_json(handle, 0, gray.as_ptr(), &mut page_id) },
            0
        );
        assert!(page_id > id);
        let mut output = OfdBuffer {
            data: ptr::null_mut(),
            len: 0,
        };
        assert_eq!(
            unsafe { ofd_page_color_spaces_json(handle, 0, &mut output) },
            0
        );
        let page: serde_json::Value =
            serde_json::from_slice(unsafe { std::slice::from_raw_parts(output.data, output.len) })
                .unwrap();
        unsafe { ofd_buffer_free(output) };
        assert_eq!(page.as_array().unwrap().len(), 2);
        assert_eq!(page[0]["id"], page_id.to_string());
        let updated = CString::new(r#"{"color_type":"RGB","palette":[[0,255,0]]}"#).unwrap();
        let id_string = CString::new(id.to_string()).unwrap();
        let page_id_string = CString::new(page_id.to_string()).unwrap();
        assert_eq!(
            unsafe {
                ofd_set_document_color_space_json(handle, 0, id_string.as_ptr(), updated.as_ptr())
            },
            0
        );
        assert_ne!(
            unsafe {
                ofd_set_document_color_space_json(
                    handle,
                    0,
                    page_id_string.as_ptr(),
                    updated.as_ptr(),
                )
            },
            0
        );
        let bad_update =
            CString::new(r#"{"color_type":"GRAY","bits_per_component":4,"palette":[[16]]}"#)
                .unwrap();
        assert_ne!(
            unsafe {
                ofd_set_page_color_space_json(
                    handle,
                    0,
                    page_id_string.as_ptr(),
                    bad_update.as_ptr(),
                )
            },
            0
        );
        let good_update =
            CString::new(r#"{"color_type":"GRAY","bits_per_component":4,"palette":[[15]]}"#)
                .unwrap();
        assert_eq!(
            unsafe {
                ofd_set_page_color_space_json(
                    handle,
                    0,
                    page_id_string.as_ptr(),
                    good_update.as_ptr(),
                )
            },
            0
        );
        assert_ne!(
            unsafe { ofd_remove_document_color_space(handle, 0, id_string.as_ptr()) },
            0
        );
        assert_eq!(
            unsafe { ofd_remove_page_color_space(handle, 0, page_id_string.as_ptr()) },
            0
        );
        assert_eq!(
            unsafe { ofd_set_document_default_color_space(handle, 0, ptr::null()) },
            0
        );
        let mut output = OfdBuffer {
            data: ptr::null_mut(),
            len: 0,
        };
        assert_eq!(
            unsafe { ofd_document_default_color_space_json(handle, 0, &mut output) },
            0
        );
        assert_eq!(
            unsafe { std::slice::from_raw_parts(output.data, output.len) },
            b"null"
        );
        unsafe { ofd_buffer_free(output) };
        assert_eq!(
            unsafe { ofd_remove_document_color_space(handle, 0, id_string.as_ptr()) },
            0
        );
        let bad = CString::new(r#"{"color_type":"RGB","palette":[[256,0,0]]}"#).unwrap();
        assert_ne!(
            unsafe { ofd_add_document_color_space_json(handle, 0, bad.as_ptr(), &mut id) },
            0
        );
        assert_eq!(id, 0);
        unsafe { ofd_close(handle) };
    }

    #[test]
    fn ffi_extracts_page_text_in_region() {
        let mut handle = ptr::null_mut();
        assert_eq!(unsafe { ofd_new(&mut handle) }, 0);
        let value = CString::new("区域").unwrap();
        assert_eq!(
            unsafe { ofd_add_text_page(handle, value.as_ptr(), 80.0, 50.0) },
            0
        );
        let mut output = OfdBuffer {
            data: ptr::null_mut(),
            len: 0,
        };
        assert_eq!(
            unsafe { ofd_page_text_in_region_json(handle, 0, 9.0, 9.0, 20.0, 10.0, &mut output) },
            0
        );
        let text: Vec<String> =
            serde_json::from_slice(unsafe { std::slice::from_raw_parts(output.data, output.len) })
                .unwrap();
        assert_eq!(text, ["区域"]);
        unsafe { ofd_buffer_free(output) };
        assert_ne!(
            unsafe {
                ofd_page_text_in_region_json(
                    handle,
                    0,
                    0.0,
                    0.0,
                    0.0,
                    10.0,
                    &mut OfdBuffer {
                        data: ptr::null_mut(),
                        len: 0,
                    },
                )
            },
            0
        );
        unsafe { ofd_close(handle) };
    }

    #[test]
    fn ffi_finds_keyword_positions_and_checks_query() {
        let mut handle = ptr::null_mut();
        assert_eq!(unsafe { ofd_new(&mut handle) }, 0);
        let value = CString::new("重复重复").unwrap();
        assert_eq!(
            unsafe { ofd_add_text_page(handle, value.as_ptr(), 80.0, 50.0) },
            0
        );
        let query = CString::new(r#"{"keywords":["重复"],"pages":[0]}"#).unwrap();
        let mut output = OfdBuffer {
            data: ptr::null_mut(),
            len: 0,
        };
        assert_eq!(
            unsafe { ofd_keyword_positions_json(handle, query.as_ptr(), &mut output) },
            0
        );
        let values: serde_json::Value =
            serde_json::from_slice(unsafe { std::slice::from_raw_parts(output.data, output.len) })
                .unwrap();
        assert_eq!(values.as_array().unwrap().len(), 2);
        assert_eq!(values[0]["page_index"], 0);
        assert_eq!(values[0]["keyword"], "重复");
        unsafe { ofd_buffer_free(output) };
        let invalid = CString::new(r#"{"keywords":[""],"pages":[0]}"#).unwrap();
        let mut output = OfdBuffer {
            data: ptr::null_mut(),
            len: 0,
        };
        assert_ne!(
            unsafe { ofd_keyword_positions_json(handle, invalid.as_ptr(), &mut output) },
            0
        );
        unsafe { ofd_close(handle) };
    }

    #[test]
    fn ffi_create_and_read_page() {
        let mut handle = ptr::null_mut();
        assert_eq!(unsafe { ofd_new(&mut handle) }, 0);
        let text = CString::new("你好").unwrap();
        assert_eq!(
            unsafe { ofd_add_text_page(handle, text.as_ptr(), 210.0, 297.0) },
            0
        );
        let mut count = 0;
        assert_eq!(unsafe { ofd_page_count(handle, &mut count) }, 0);
        assert_eq!(count, 1);
        let mut output = OfdBuffer {
            data: ptr::null_mut(),
            len: 0,
        };
        assert_eq!(unsafe { ofd_page_text_json(handle, 0, &mut output) }, 0);
        let bytes = unsafe { std::slice::from_raw_parts(output.data, output.len) };
        assert_eq!(bytes, "[\"你好\"]".as_bytes());
        unsafe {
            ofd_buffer_free(output);
            ofd_close(handle);
        }
    }

    #[test]
    fn ffi_inserts_pages_at_local_positions() {
        let mut handle = ptr::null_mut();
        assert_eq!(unsafe { ofd_new(&mut handle) }, 0);
        for value in ["A", "C"] {
            let value = CString::new(value).unwrap();
            assert_eq!(
                unsafe { ofd_add_text_page(handle, value.as_ptr(), 20.0, 20.0) },
                0
            );
        }
        let value = CString::new("B").unwrap();
        let mut index = usize::MAX;
        assert_eq!(
            unsafe { ofd_insert_text_page(handle, 0, 1, value.as_ptr(), 20.0, 20.0, &mut index) },
            0
        );
        assert_eq!(index, 1);
        assert_eq!(
            unsafe { ofd_insert_blank_page(handle, 0, 0, 20.0, 20.0, &mut index) },
            0
        );
        assert_eq!(index, 0);
        assert_eq!(
            unsafe { document_ref(handle).unwrap() }
                .page_text(2)
                .unwrap(),
            ["B"]
        );
        let before = unsafe { document_ref(handle).unwrap() }.to_bytes().unwrap();
        assert_ne!(
            unsafe { ofd_insert_blank_page(handle, 0, 5, 20.0, 20.0, &mut index) },
            0
        );
        assert_ne!(
            unsafe { ofd_insert_blank_page(handle, 0, 0, 20.0, 20.0, ptr::null_mut()) },
            0
        );
        assert_eq!(
            unsafe { document_ref(handle).unwrap() }.to_bytes().unwrap(),
            before
        );
        unsafe { ofd_close(handle) };
    }

    #[test]
    fn ffi_moves_pages_and_preserves_order_after_append() {
        let mut handle = ptr::null_mut();
        assert_eq!(unsafe { ofd_new(&mut handle) }, 0);
        for value in ["A", "B", "C"] {
            let text = CString::new(value).unwrap();
            assert_eq!(
                unsafe { ofd_add_text_page(handle, text.as_ptr(), 20.0, 20.0) },
                0
            );
        }
        assert_eq!(unsafe { ofd_move_page(handle, 0, 2) }, 0);
        let text = CString::new("D").unwrap();
        assert_eq!(
            unsafe { ofd_add_text_page(handle, text.as_ptr(), 20.0, 20.0) },
            0
        );
        for (index, expected) in ["B", "C", "A", "D"].into_iter().enumerate() {
            let mut output = OfdBuffer {
                data: ptr::null_mut(),
                len: 0,
            };
            assert_eq!(unsafe { ofd_page_text_json(handle, index, &mut output) }, 0);
            let bytes = unsafe { std::slice::from_raw_parts(output.data, output.len) };
            assert_eq!(
                serde_json::from_slice::<Vec<String>>(bytes).unwrap(),
                vec![expected]
            );
            unsafe { ofd_buffer_free(output) };
        }
        assert_ne!(unsafe { ofd_move_page(handle, 0, 4) }, 0);
        unsafe { ofd_close(handle) };
    }

    #[test]
    fn ffi_reorders_pages_atomically() {
        let mut handle = ptr::null_mut();
        assert_eq!(unsafe { ofd_new(&mut handle) }, 0);
        for value in ["A", "B", "C"] {
            let text = CString::new(value).unwrap();
            assert_eq!(
                unsafe { ofd_add_text_page(handle, text.as_ptr(), 20.0, 20.0) },
                0
            );
        }
        let before = unsafe { document_ref(handle).unwrap() }.to_bytes().unwrap();
        let invalid = CString::new("[0,0,2]").unwrap();
        assert_ne!(
            unsafe { ofd_reorder_document_pages_json(handle, 0, invalid.as_ptr()) },
            0
        );
        assert_eq!(
            unsafe { document_ref(handle).unwrap() }.to_bytes().unwrap(),
            before
        );
        let order = CString::new("[2,0,1]").unwrap();
        assert_eq!(
            unsafe { ofd_reorder_document_pages_json(handle, 0, order.as_ptr()) },
            0
        );
        assert_eq!(
            unsafe { document_ref(handle).unwrap() }.all_text().unwrap(),
            [
                vec!["C".to_owned()],
                vec!["A".to_owned()],
                vec!["B".to_owned()]
            ]
        );
        unsafe { ofd_close(handle) };
    }

    #[test]
    fn ffi_duplicates_page_and_returns_new_index() {
        let mut handle = ptr::null_mut();
        assert_eq!(unsafe { ofd_new(&mut handle) }, 0);
        let text = CString::new("copy me").unwrap();
        assert_eq!(
            unsafe { ofd_add_text_page(handle, text.as_ptr(), 40.0, 30.0) },
            0
        );
        let watermark =
            CString::new(r#"{"text":"copied mark","boundary":[1,1,12,6],"size":4}"#).unwrap();
        let mut annotation_id = 0;
        assert_eq!(
            unsafe {
                ofd_add_text_watermark_json(handle, 0, watermark.as_ptr(), &mut annotation_id)
            },
            0
        );
        let mut index = usize::MAX;
        assert_eq!(unsafe { ofd_duplicate_page(handle, 0, &mut index) }, 0);
        assert_eq!(index, 1);
        let annotations = unsafe { document_ref(handle).unwrap() }
            .annotations()
            .unwrap();
        assert_eq!(annotations.len(), 2);
        assert_ne!(annotations[0].id, annotations[1].id);
        assert_ne!(unsafe { ofd_duplicate_page(handle, 2, &mut index) }, 0);
        assert_ne!(unsafe { ofd_duplicate_page(handle, 0, ptr::null_mut()) }, 0);
        let mut output = OfdBuffer {
            data: ptr::null_mut(),
            len: 0,
        };
        assert_eq!(unsafe { ofd_page_text_json(handle, 1, &mut output) }, 0);
        let bytes = unsafe { std::slice::from_raw_parts(output.data, output.len) };
        assert_eq!(bytes, b"[\"copy me\"]");
        assert_eq!(
            unsafe { ofd_duplicate_page_at(handle, 0, 0, &mut index) },
            0
        );
        assert_eq!(index, 0);
        assert_eq!(
            unsafe { document_ref(handle).unwrap() }
                .annotations()
                .unwrap()
                .len(),
            3
        );
        let before = unsafe { document_ref(handle).unwrap() }.to_bytes().unwrap();
        assert_ne!(
            unsafe { ofd_duplicate_page_at(handle, 0, 4, &mut index) },
            0
        );
        assert_ne!(
            unsafe { ofd_duplicate_page_at(handle, 0, 0, ptr::null_mut()) },
            0
        );
        assert_eq!(
            unsafe { document_ref(handle).unwrap() }.to_bytes().unwrap(),
            before
        );
        unsafe {
            ofd_buffer_free(output);
            ofd_close(handle);
        }
    }

    #[test]
    fn ffi_removes_page_and_rejects_invalid_index() {
        let mut handle = ptr::null_mut();
        assert_eq!(unsafe { ofd_new(&mut handle) }, 0);
        for value in ["A", "B", "C"] {
            let text = CString::new(value).unwrap();
            assert_eq!(
                unsafe { ofd_add_text_page(handle, text.as_ptr(), 20.0, 20.0) },
                0
            );
        }
        assert_eq!(unsafe { ofd_remove_page(handle, 1) }, 0);
        assert_ne!(unsafe { ofd_remove_page(handle, 2) }, 0);
        let text = CString::new("D").unwrap();
        assert_eq!(
            unsafe { ofd_add_text_page(handle, text.as_ptr(), 20.0, 20.0) },
            0
        );
        for (index, expected) in ["A", "C", "D"].into_iter().enumerate() {
            let mut output = OfdBuffer {
                data: ptr::null_mut(),
                len: 0,
            };
            assert_eq!(unsafe { ofd_page_text_json(handle, index, &mut output) }, 0);
            let bytes = unsafe { std::slice::from_raw_parts(output.data, output.len) };
            assert_eq!(
                serde_json::from_slice::<Vec<String>>(bytes).unwrap(),
                vec![expected]
            );
            unsafe { ofd_buffer_free(output) };
        }
        unsafe { ofd_close(handle) };
    }

    #[test]
    fn ffi_entry_save_and_reopen() {
        let mut handle = ptr::null_mut();
        assert_eq!(unsafe { ofd_new(&mut handle) }, 0);
        let entry = CString::new("Doc_0/Custom.bin").unwrap();
        let bytes = [0_u8, 1, 2, 255];
        assert_eq!(
            unsafe { ofd_set_entry(handle, entry.as_ptr(), bytes.as_ptr(), bytes.len()) },
            0
        );
        let mut output = OfdBuffer {
            data: ptr::null_mut(),
            len: 0,
        };
        assert_eq!(
            unsafe { ofd_get_entry(handle, entry.as_ptr(), &mut output) },
            0
        );
        assert_eq!(
            unsafe { std::slice::from_raw_parts(output.data, output.len) },
            bytes
        );
        unsafe {
            ofd_buffer_free(output);
        }

        let mut removed = -1;
        assert_eq!(
            unsafe { ofd_remove_entry(handle, entry.as_ptr(), &mut removed) },
            0
        );
        assert_eq!(removed, 1);
        assert_eq!(
            unsafe { ofd_remove_entry(handle, entry.as_ptr(), &mut removed) },
            0
        );
        assert_eq!(removed, 0);
        assert_ne!(
            unsafe { ofd_remove_entry(handle, entry.as_ptr(), ptr::null_mut()) },
            0
        );

        let dir = tempdir().unwrap();
        let path = CString::new(dir.path().join("test.ofd").to_str().unwrap()).unwrap();
        assert_eq!(unsafe { ofd_save(handle, path.as_ptr()) }, 0);
        unsafe {
            ofd_close(handle);
        }
        let mut reopened = ptr::null_mut();
        assert_eq!(unsafe { ofd_open(path.as_ptr(), &mut reopened) }, 0);
        assert!(!reopened.is_null());
        assert!(unsafe { document_ref(reopened).unwrap() }
            .entry("Doc_0/Custom.bin")
            .unwrap()
            .is_none());
        unsafe {
            ofd_close(reopened);
        }
    }

    #[test]
    fn ffi_reads_utf16_xml_and_preserves_raw_entry_bytes() {
        let mut handle = ptr::null_mut();
        assert_eq!(unsafe { ofd_new(&mut handle) }, 0);
        let text = CString::new("中文 OFD").unwrap();
        assert_eq!(
            unsafe { ofd_add_text_page(handle, text.as_ptr(), 40.0, 30.0) },
            0
        );
        let page_path = unsafe { document_ref(handle).unwrap() }.pages().unwrap()[0]
            .path
            .clone();
        let original = unsafe { document_ref(handle).unwrap() }
            .entry(&page_path)
            .unwrap()
            .unwrap();
        let mut utf16 = vec![0xff, 0xfe];
        for unit in std::str::from_utf8(original).unwrap().encode_utf16() {
            utf16.extend(unit.to_le_bytes());
        }
        let path = CString::new(page_path).unwrap();
        assert_eq!(
            unsafe { ofd_set_entry(handle, path.as_ptr(), utf16.as_ptr(), utf16.len()) },
            0
        );
        let mut package = OfdBuffer {
            data: ptr::null_mut(),
            len: 0,
        };
        assert_eq!(unsafe { ofd_to_bytes(handle, &mut package) }, 0);
        let mut reopened = ptr::null_mut();
        assert_eq!(
            unsafe { ofd_from_bytes(package.data, package.len, &mut reopened) },
            0
        );
        let mut output = OfdBuffer {
            data: ptr::null_mut(),
            len: 0,
        };
        assert_eq!(unsafe { ofd_page_text_json(reopened, 0, &mut output) }, 0);
        let page_text: Vec<String> =
            serde_json::from_slice(unsafe { std::slice::from_raw_parts(output.data, output.len) })
                .unwrap();
        assert_eq!(page_text, ["中文 OFD"]);
        unsafe { ofd_buffer_free(output) };
        let mut raw = OfdBuffer {
            data: ptr::null_mut(),
            len: 0,
        };
        assert_eq!(
            unsafe { ofd_get_entry(reopened, path.as_ptr(), &mut raw) },
            0
        );
        assert_eq!(
            unsafe { std::slice::from_raw_parts(raw.data, raw.len) },
            utf16
        );
        unsafe {
            ofd_buffer_free(raw);
            ofd_buffer_free(package);
            ofd_close(reopened);
            ofd_close(handle);
        }
    }

    #[test]
    fn ffi_opens_and_serializes_ofd_bytes() {
        let mut document = ptr::null_mut();
        assert_eq!(unsafe { ofd_new(&mut document) }, 0);
        let text = CString::new("in memory").unwrap();
        assert_eq!(
            unsafe { ofd_add_text_page(document, text.as_ptr(), 40.0, 30.0) },
            0
        );
        let mut bytes = OfdBuffer {
            data: ptr::null_mut(),
            len: 0,
        };
        assert_ne!(unsafe { ofd_to_bytes(document, ptr::null_mut()) }, 0);
        assert_eq!(unsafe { ofd_to_bytes(document, &mut bytes) }, 0);
        let source = unsafe { std::slice::from_raw_parts(bytes.data, bytes.len) };
        assert!(source.starts_with(b"PK"));
        let mut reopened = ptr::null_mut();
        assert_ne!(
            unsafe { ofd_from_bytes(source.as_ptr(), source.len(), ptr::null_mut()) },
            0
        );
        assert_ne!(
            unsafe { ofd_from_bytes(ptr::null(), source.len(), &mut reopened) },
            0
        );
        assert!(reopened.is_null());
        assert_ne!(unsafe { ofd_from_bytes(ptr::null(), 0, &mut reopened) }, 0);
        assert!(reopened.is_null());
        assert_eq!(
            unsafe { ofd_from_bytes(source.as_ptr(), source.len(), &mut reopened) },
            0
        );
        assert_eq!(
            unsafe { document_ref(reopened).unwrap() }
                .page_text(0)
                .unwrap(),
            ["in memory"]
        );
        assert_ne!(unsafe { ofd_to_bytes(ptr::null(), &mut bytes) }, 0);
        unsafe {
            ofd_close(reopened);
            ofd_buffer_free(bytes);
            ofd_close(document);
        }
    }

    #[test]
    fn ffi_reports_invalid_arguments_and_document_errors() {
        let mut handle = ptr::null_mut();
        assert_ne!(unsafe { ofd_open(ptr::null(), &mut handle) }, 0);
        assert!(unsafe { CStr::from_ptr(ofd_last_error()) }
            .to_str()
            .unwrap()
            .contains("null"));
        assert_ne!(unsafe { ofd_new(ptr::null_mut()) }, 0);
        assert_eq!(unsafe { ofd_new(&mut handle) }, 0);
        assert_ne!(unsafe { ofd_page_count(handle, ptr::null_mut()) }, 0);
        let mut output = OfdBuffer {
            data: ptr::null_mut(),
            len: 0,
        };
        assert_ne!(unsafe { ofd_page_text_json(handle, 0, &mut output) }, 0);
        let missing = CString::new("missing.bin").unwrap();
        assert_ne!(
            unsafe { ofd_get_entry(handle, missing.as_ptr(), &mut output) },
            0
        );
        let text = CString::new("bad").unwrap();
        assert_ne!(
            unsafe { ofd_add_text_page(handle, text.as_ptr(), -1.0, 297.0) },
            0
        );
        let path = CString::new("OFD.xml").unwrap();
        assert_ne!(
            unsafe { ofd_set_entry(handle, path.as_ptr(), b"<broken>".as_ptr(), 8) },
            0
        );
        unsafe {
            ofd_close(handle);
            ofd_close(ptr::null_mut());
            ofd_buffer_free(OfdBuffer {
                data: ptr::null_mut(),
                len: 0,
            });
        }
    }

    #[test]
    fn ffi_reads_java_attachment() {
        let fixture = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("../../../ofdrw-reader/src/test/resources/AddAttachment.ofd");
        let path = CString::new(fixture.to_str().unwrap()).unwrap();
        let mut handle = ptr::null_mut();
        assert_eq!(unsafe { ofd_open(path.as_ptr(), &mut handle) }, 0);
        let mut output = OfdBuffer {
            data: ptr::null_mut(),
            len: 0,
        };
        assert_eq!(unsafe { ofd_attachments_json(handle, &mut output) }, 0);
        let json = unsafe { std::slice::from_raw_parts(output.data, output.len) };
        let values: serde_json::Value = serde_json::from_slice(json).unwrap();
        assert_eq!(values[0]["name"], "Gao");
        unsafe {
            ofd_buffer_free(output);
        }
        output = OfdBuffer {
            data: ptr::null_mut(),
            len: 0,
        };
        assert_eq!(unsafe { ofd_attachment_data(handle, 0, &mut output) }, 0);
        assert_eq!(output.len, 36_494);
        unsafe {
            ofd_buffer_free(output);
        }
        output = OfdBuffer {
            data: ptr::null_mut(),
            len: 0,
        };
        assert_ne!(unsafe { ofd_attachment_data(handle, 1, &mut output) }, 0);
        unsafe {
            ofd_close(handle);
        }
    }

    #[test]
    fn ffi_reads_java_nested_page_block_objects() {
        let fixture = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("../../../ofdrw-reader/src/test/resources/helloworld_with_pageblock.ofd");
        let path = CString::new(fixture.to_string_lossy().as_bytes()).unwrap();
        let mut handle = ptr::null_mut();
        assert_eq!(unsafe { ofd_open(path.as_ptr(), &mut handle) }, 0);
        let mut objects = OfdBuffer {
            data: ptr::null_mut(),
            len: 0,
        };
        assert_eq!(unsafe { ofd_page_objects_json(handle, 0, &mut objects) }, 0);
        let values: serde_json::Value = serde_json::from_slice(unsafe {
            std::slice::from_raw_parts(objects.data, objects.len)
        })
        .unwrap();
        assert_eq!(values.as_array().unwrap().len(), 1);
        assert_eq!(values[0]["kind"], "TextObject");
        let id = CString::new(values[0]["id"].as_str().unwrap()).unwrap();
        assert_eq!(
            unsafe { ofd_set_page_object_visible(handle, 0, id.as_ptr(), 0) },
            0
        );
        assert!(unsafe { document_ref(handle).unwrap() }
            .page_text(0)
            .unwrap()
            .is_empty());
        assert_eq!(
            unsafe { ofd_set_page_object_visible(handle, 0, id.as_ptr(), 1) },
            0
        );
        unsafe {
            ofd_buffer_free(objects);
            ofd_close(handle);
        }
    }

    #[test]
    fn ffi_writes_attachment() {
        let mut handle = ptr::null_mut();
        assert_eq!(unsafe { ofd_new(&mut handle) }, 0);
        let name = CString::new("note.txt").unwrap();
        let data = b"hello";
        assert_eq!(
            unsafe { ofd_add_attachment(handle, name.as_ptr(), data.as_ptr(), data.len()) },
            0
        );
        let metadata = CString::new(r#"{"format":"text/plain","creation_date":"2024-02-29T12:30:59","mod_date":"2024-03-01T08:00:00","visible":false,"usage":"source & <record>"}"#).unwrap();
        assert_eq!(
            unsafe { ofd_set_attachment_metadata_json(handle, 0, metadata.as_ptr()) },
            0
        );
        let invalid = CString::new(r#"{"creation_date":"2023-02-29T12:30:59"}"#).unwrap();
        assert_ne!(
            unsafe { ofd_set_attachment_metadata_json(handle, 0, invalid.as_ptr()) },
            0
        );
        assert_eq!(
            unsafe { document_ref(handle).unwrap() }
                .attachments()
                .unwrap()[0]
                .usage,
            "source & <record>"
        );
        let renamed = CString::new("renamed & <file>.txt").unwrap();
        assert_eq!(
            unsafe { ofd_rename_attachment(handle, 0, renamed.as_ptr()) },
            0
        );
        assert_eq!(
            unsafe { document_ref(handle).unwrap() }
                .attachments()
                .unwrap()[0]
                .name,
            "renamed & <file>.txt"
        );
        let mut output = OfdBuffer {
            data: ptr::null_mut(),
            len: 0,
        };
        assert_eq!(unsafe { ofd_attachment_data(handle, 0, &mut output) }, 0);
        assert_eq!(
            unsafe { std::slice::from_raw_parts(output.data, output.len) },
            data
        );
        unsafe {
            ofd_buffer_free(output);
            assert_eq!(
                ofd_replace_attachment_data(handle, 0, b"updated".as_ptr(), 7),
                0
            );
            assert_ne!(
                ofd_replace_attachment_data(handle, 1, b"bad".as_ptr(), 3),
                0
            );
            assert_eq!(
                document_ref(handle).unwrap().attachment_data(0).unwrap(),
                b"updated"
            );
            assert_eq!(ofd_remove_attachment(handle, 0), 0);
            assert_ne!(ofd_remove_attachment(handle, 0), 0);
            ofd_close(handle);
        }
    }

    #[test]
    fn ffi_writes_attachment_actions() {
        let mut handle = ptr::null_mut();
        assert_eq!(unsafe { ofd_new(&mut handle) }, 0);
        let text = CString::new("click").unwrap();
        assert_eq!(
            unsafe { ofd_add_text_page(handle, text.as_ptr(), 40.0, 30.0) },
            0
        );
        let name = CString::new("note.txt").unwrap();
        assert_eq!(
            unsafe { ofd_add_attachment(handle, name.as_ptr(), b"hi".as_ptr(), 2) },
            0
        );
        let document = unsafe { document_ref(handle).unwrap() };
        let object_id = CString::new(
            document.page_objects(0).unwrap()[0]
                .id
                .as_ref()
                .unwrap()
                .as_str(),
        )
        .unwrap();
        let attachment_id = CString::new(document.attachments().unwrap()[0].id.as_str()).unwrap();
        assert_eq!(
            unsafe {
                ofd_add_page_object_attachment_action(
                    handle,
                    0,
                    object_id.as_ptr(),
                    attachment_id.as_ptr(),
                    1,
                )
            },
            0
        );
        let payload = CString::new(format!(
            r#"{{"attachment_id":"{}","boundary":[1,1,5,5],"new_window":false}}"#,
            attachment_id.to_str().unwrap()
        ))
        .unwrap();
        let mut annotation_id = 0;
        assert_eq!(
            unsafe {
                ofd_add_attachment_link_annotation_json(
                    handle,
                    0,
                    payload.as_ptr(),
                    &mut annotation_id,
                )
            },
            0
        );
        assert!(annotation_id > 0);
        assert!(unsafe { document_ref(handle).unwrap() }
            .validate_references()
            .unwrap()
            .is_empty());
        assert_ne!(
            unsafe {
                ofd_add_page_object_attachment_action(
                    handle,
                    0,
                    object_id.as_ptr(),
                    attachment_id.as_ptr(),
                    2,
                )
            },
            0
        );
        unsafe { ofd_close(handle) };
    }

    #[test]
    fn ffi_writes_audio_video_actions() {
        let mut handle = ptr::null_mut();
        assert_eq!(unsafe { ofd_new(&mut handle) }, 0);
        let text = CString::new("click").unwrap();
        assert_eq!(
            unsafe { ofd_add_text_page(handle, text.as_ptr(), 50.0, 40.0) },
            0
        );
        let audio = CString::new("Audio").unwrap();
        let video = CString::new("Video").unwrap();
        let mp3 = CString::new("MP3").unwrap();
        let mp4 = CString::new("MP4").unwrap();
        let mut audio_id = 0;
        let mut video_id = 0;
        assert_eq!(
            unsafe {
                ofd_add_multimedia_resource(
                    handle,
                    0,
                    audio.as_ptr(),
                    mp3.as_ptr(),
                    b"audio".as_ptr(),
                    5,
                    &mut audio_id,
                )
            },
            0
        );
        assert_eq!(
            unsafe {
                ofd_add_multimedia_resource(
                    handle,
                    0,
                    video.as_ptr(),
                    mp4.as_ptr(),
                    b"video".as_ptr(),
                    5,
                    &mut video_id,
                )
            },
            0
        );
        let mut resources = OfdBuffer {
            data: ptr::null_mut(),
            len: 0,
        };
        assert_eq!(
            unsafe { ofd_page_multimedia_resources_json(handle, 0, &mut resources) },
            0
        );
        let json: serde_json::Value = serde_json::from_slice(unsafe {
            std::slice::from_raw_parts(resources.data, resources.len)
        })
        .unwrap();
        assert_eq!(json[0]["media_type"], "Audio");
        assert_eq!(json[0]["size"], 5);
        assert_eq!(json[1]["media_type"], "Video");
        unsafe { ofd_buffer_free(resources) };
        let mut document_resources = OfdBuffer {
            data: ptr::null_mut(),
            len: 0,
        };
        assert_eq!(
            unsafe { ofd_document_multimedia_resources_json(handle, 0, &mut document_resources) },
            0
        );
        let document_json: serde_json::Value = serde_json::from_slice(unsafe {
            std::slice::from_raw_parts(document_resources.data, document_resources.len)
        })
        .unwrap();
        assert_eq!(document_json, json);
        unsafe { ofd_buffer_free(document_resources) };
        let id = CString::new(audio_id.to_string()).unwrap();
        let mut data = OfdBuffer {
            data: ptr::null_mut(),
            len: 0,
        };
        assert_eq!(
            unsafe { ofd_page_multimedia_data(handle, 0, id.as_ptr(), &mut data) },
            0
        );
        assert_eq!(
            unsafe { std::slice::from_raw_parts(data.data, data.len) },
            b"audio"
        );
        unsafe { ofd_buffer_free(data) };
        let mut document_data = OfdBuffer {
            data: ptr::null_mut(),
            len: 0,
        };
        assert_eq!(
            unsafe { ofd_document_multimedia_data(handle, 0, id.as_ptr(), &mut document_data) },
            0
        );
        assert_eq!(
            unsafe { std::slice::from_raw_parts(document_data.data, document_data.len) },
            b"audio"
        );
        unsafe { ofd_buffer_free(document_data) };
        let missing = CString::new("missing").unwrap();
        let mut data = OfdBuffer {
            data: ptr::null_mut(),
            len: 0,
        };
        assert_ne!(
            unsafe { ofd_page_multimedia_data(handle, 0, missing.as_ptr(), &mut data) },
            0
        );
        let mut resources = OfdBuffer {
            data: ptr::null_mut(),
            len: 0,
        };
        assert_ne!(
            unsafe { ofd_page_multimedia_resources_json(handle, 1, &mut resources) },
            0
        );
        assert_ne!(
            unsafe { ofd_document_multimedia_resources_json(handle, 1, &mut resources) },
            0
        );
        let object_id = CString::new(
            unsafe { document_ref(handle).unwrap() }
                .page_objects(0)
                .unwrap()[0]
                .id
                .as_ref()
                .unwrap()
                .as_str(),
        )
        .unwrap();
        let sound = CString::new(format!(
            r#"{{"kind":"Sound","resource_id":"{audio_id}","volume":75}}"#
        ))
        .unwrap();
        let movie = CString::new(format!(
            r#"{{"kind":"Movie","resource_id":"{video_id}","operator":"Pause"}}"#
        ))
        .unwrap();
        assert_eq!(
            unsafe {
                ofd_add_page_object_media_action_json(handle, 0, object_id.as_ptr(), sound.as_ptr())
            },
            0
        );
        assert_eq!(
            unsafe {
                ofd_add_page_object_media_action_json(handle, 0, object_id.as_ptr(), movie.as_ptr())
            },
            0
        );
        let link = CString::new(format!(
            r#"{{"kind":"Sound","resource_id":"{audio_id}","boundary":[1,1,5,5]}}"#
        ))
        .unwrap();
        let mut annotation_id = 0;
        assert_eq!(
            unsafe {
                ofd_add_media_link_annotation_json(handle, 0, link.as_ptr(), &mut annotation_id)
            },
            0
        );
        assert!(annotation_id > 0);
        assert!(unsafe { document_ref(handle).unwrap() }
            .validate_references()
            .unwrap()
            .is_empty());
        let png = unsafe { document_ref(handle).unwrap() }
            .page_png(0, 25.4)
            .unwrap();
        let image = CString::new("Image").unwrap();
        let png_format = CString::new("pNg").unwrap();
        let mut image_id = 0;
        assert_eq!(
            unsafe {
                ofd_add_multimedia_resource(
                    handle,
                    0,
                    image.as_ptr(),
                    png_format.as_ptr(),
                    png.as_ptr(),
                    png.len(),
                    &mut image_id,
                )
            },
            0
        );
        let image_id = CString::new(image_id.to_string()).unwrap();
        let mut image_data = OfdBuffer {
            data: ptr::null_mut(),
            len: 0,
        };
        assert_eq!(
            unsafe { ofd_document_multimedia_data(handle, 0, image_id.as_ptr(), &mut image_data) },
            0
        );
        assert_eq!(
            unsafe { std::slice::from_raw_parts(image_data.data, image_data.len) },
            png
        );
        unsafe { ofd_buffer_free(image_data) };
        let mut page_media_id = 0;
        assert_eq!(
            unsafe {
                ofd_add_page_multimedia_resource(
                    handle,
                    0,
                    audio.as_ptr(),
                    mp3.as_ptr(),
                    b"page".as_ptr(),
                    4,
                    &mut page_media_id,
                )
            },
            0
        );
        let page_media_id = CString::new(page_media_id.to_string()).unwrap();
        let mut local_data = OfdBuffer {
            data: ptr::null_mut(),
            len: 0,
        };
        assert_eq!(
            unsafe { ofd_page_multimedia_data(handle, 0, page_media_id.as_ptr(), &mut local_data) },
            0
        );
        assert_eq!(
            unsafe { std::slice::from_raw_parts(local_data.data, local_data.len) },
            b"page"
        );
        unsafe { ofd_buffer_free(local_data) };
        let mut local_data = OfdBuffer {
            data: ptr::null_mut(),
            len: 0,
        };
        assert_ne!(
            unsafe {
                ofd_document_multimedia_data(handle, 0, page_media_id.as_ptr(), &mut local_data)
            },
            0
        );
        assert_ne!(
            unsafe { ofd_remove_page_multimedia_resource(handle, 0, id.as_ptr()) },
            0
        );
        assert_eq!(
            unsafe { ofd_remove_page_multimedia_resource(handle, 0, page_media_id.as_ptr()) },
            0
        );
        assert_ne!(
            unsafe { ofd_page_multimedia_data(handle, 0, page_media_id.as_ptr(), &mut local_data) },
            0
        );
        let replacement = b"new audio";
        assert_eq!(
            unsafe {
                ofd_replace_document_multimedia_data(
                    handle,
                    0,
                    id.as_ptr(),
                    replacement.as_ptr(),
                    replacement.len(),
                )
            },
            0
        );
        let mut replaced = OfdBuffer {
            data: ptr::null_mut(),
            len: 0,
        };
        assert_eq!(
            unsafe { ofd_document_multimedia_data(handle, 0, id.as_ptr(), &mut replaced) },
            0
        );
        assert_eq!(
            unsafe { std::slice::from_raw_parts(replaced.data, replaced.len) },
            replacement
        );
        unsafe { ofd_buffer_free(replaced) };
        let page_replacement = b"page audio";
        assert_eq!(
            unsafe {
                ofd_replace_page_multimedia_data(
                    handle,
                    0,
                    id.as_ptr(),
                    page_replacement.as_ptr(),
                    page_replacement.len(),
                )
            },
            0
        );
        let mut page_data = OfdBuffer {
            data: ptr::null_mut(),
            len: 0,
        };
        assert_eq!(
            unsafe { ofd_page_multimedia_data(handle, 0, id.as_ptr(), &mut page_data) },
            0
        );
        assert_eq!(
            unsafe { std::slice::from_raw_parts(page_data.data, page_data.len) },
            page_replacement
        );
        unsafe { ofd_buffer_free(page_data) };
        assert_ne!(
            unsafe { ofd_remove_document_multimedia_resource(handle, 0, id.as_ptr()) },
            0
        );
        assert_eq!(
            unsafe { ofd_remove_document_multimedia_resource(handle, 0, image_id.as_ptr()) },
            0
        );
        let mut removed_data = OfdBuffer {
            data: ptr::null_mut(),
            len: 0,
        };
        assert_ne!(
            unsafe {
                ofd_document_multimedia_data(handle, 0, image_id.as_ptr(), &mut removed_data)
            },
            0
        );
        unsafe { ofd_close(handle) };
    }

    #[test]
    fn ffi_adds_attachment_to_existing_document() {
        let fixture = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("../../../ofdrw-reader/src/test/resources/AddAttachment.ofd");
        let path = CString::new(fixture.to_str().unwrap()).unwrap();
        let mut handle = ptr::null_mut();
        assert_eq!(unsafe { ofd_open(path.as_ptr(), &mut handle) }, 0);
        let name = CString::new("Rust note").unwrap();
        let data = b"new data";
        assert_ne!(
            unsafe {
                ofd_add_attachment_to_document(handle, 1, name.as_ptr(), data.as_ptr(), data.len())
            },
            0
        );
        assert_eq!(
            unsafe {
                ofd_add_attachment_to_document(handle, 0, name.as_ptr(), data.as_ptr(), data.len())
            },
            0
        );
        let mut output = OfdBuffer {
            data: ptr::null_mut(),
            len: 0,
        };
        assert_eq!(unsafe { ofd_attachment_data(handle, 1, &mut output) }, 0);
        assert_eq!(
            unsafe { std::slice::from_raw_parts(output.data, output.len) },
            data
        );
        unsafe {
            ofd_buffer_free(output);
            assert_eq!(ofd_remove_attachment(handle, 1), 0);
            assert_ne!(ofd_remove_attachment(handle, 1), 0);
            let mut retained = OfdBuffer {
                data: ptr::null_mut(),
                len: 0,
            };
            assert_eq!(ofd_attachment_data(handle, 0, &mut retained), 0);
            ofd_buffer_free(retained);
            ofd_close(handle);
        }
    }

    #[test]
    fn ffi_validates_references() {
        let mut handle = ptr::null_mut();
        assert_eq!(unsafe { ofd_new(&mut handle) }, 0);
        let text = CString::new("check").unwrap();
        assert_eq!(
            unsafe { ofd_add_text_page(handle, text.as_ptr(), 210.0, 297.0) },
            0
        );
        let mut output = OfdBuffer {
            data: ptr::null_mut(),
            len: 0,
        };
        assert_eq!(
            unsafe { ofd_validate_references_json(handle, &mut output) },
            0
        );
        let json = unsafe { std::slice::from_raw_parts(output.data, output.len) };
        assert_eq!(
            serde_json::from_slice::<serde_json::Value>(json).unwrap(),
            serde_json::json!([])
        );
        unsafe {
            ofd_buffer_free(output);
        }
        let page_path = CString::new("Doc_0/Pages/Page_0/Content.xml").unwrap();
        output = OfdBuffer {
            data: ptr::null_mut(),
            len: 0,
        };
        assert_eq!(
            unsafe { ofd_get_entry(handle, page_path.as_ptr(), &mut output) },
            0
        );
        let page = unsafe { std::slice::from_raw_parts(output.data, output.len) };
        let changed = String::from_utf8(page.to_vec())
            .unwrap()
            .replace("Font=\"1\"", "Font=\"999\"");
        unsafe {
            ofd_buffer_free(output);
        }
        assert_eq!(
            unsafe { ofd_set_entry(handle, page_path.as_ptr(), changed.as_ptr(), changed.len()) },
            0
        );
        output = OfdBuffer {
            data: ptr::null_mut(),
            len: 0,
        };
        assert_eq!(
            unsafe { ofd_validate_references_json(handle, &mut output) },
            0
        );
        let json = unsafe { std::slice::from_raw_parts(output.data, output.len) };
        let issues: serde_json::Value = serde_json::from_slice(json).unwrap();
        assert!(issues
            .as_array()
            .unwrap()
            .iter()
            .any(|issue| issue["code"] == "missing_font"));
        unsafe {
            ofd_buffer_free(output);
            ofd_close(handle);
        }
    }

    #[test]
    fn ffi_adds_and_appends_blank_pages() {
        let mut handle = ptr::null_mut();
        assert_eq!(unsafe { ofd_new(&mut handle) }, 0);
        assert_eq!(unsafe { ofd_add_blank_page(handle, 40.0, 30.0) }, 0);
        assert_eq!(unsafe { ofd_append_blank_page(handle, 0, 20.0, 20.0) }, 0);
        assert_ne!(unsafe { ofd_add_blank_page(handle, 0.0, 20.0) }, 0);
        let mut count = 0;
        assert_eq!(unsafe { ofd_page_count(handle, &mut count) }, 0);
        assert_eq!(count, 2);
        let mut output = OfdBuffer {
            data: ptr::null_mut(),
            len: 0,
        };
        assert_eq!(unsafe { ofd_page_text_json(handle, 1, &mut output) }, 0);
        assert_eq!(
            unsafe { std::slice::from_raw_parts(output.data, output.len) },
            b"[]"
        );
        unsafe {
            ofd_buffer_free(output);
            ofd_close(handle)
        };

        let fixture = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("../../..")
            .join("ofdrw-reader/src/test/resources/helloworld.ofd");
        let path = CString::new(fixture.to_str().unwrap()).unwrap();
        assert_eq!(unsafe { ofd_open(path.as_ptr(), &mut handle) }, 0);
        assert_eq!(unsafe { ofd_append_blank_page(handle, 0, 40.0, 30.0) }, 0);
        assert_ne!(unsafe { ofd_add_blank_page(handle, 40.0, 30.0) }, 0);
        assert_eq!(unsafe { ofd_page_count(handle, &mut count) }, 0);
        assert_eq!(count, 2);
        unsafe { ofd_close(handle) };
    }

    #[test]
    fn ffi_imports_document_bodies_and_rejects_null_output_atomically() {
        let mut target = ptr::null_mut();
        let mut source = ptr::null_mut();
        assert_eq!(unsafe { ofd_new(&mut target) }, 0);
        assert_eq!(unsafe { ofd_new(&mut source) }, 0);
        assert_eq!(unsafe { ofd_add_blank_page(source, 40.0, 30.0) }, 0);
        assert_ne!(
            unsafe { ofd_append_document_bodies_from_json(target, source, ptr::null_mut()) },
            0
        );
        let mut count = 0;
        assert_eq!(unsafe { ofd_page_count(target, &mut count) }, 0);
        assert_eq!(count, 0);
        let mut output = OfdBuffer {
            data: ptr::null_mut(),
            len: 0,
        };
        assert_eq!(
            unsafe { ofd_append_document_bodies_from_json(target, source, &mut output) },
            0
        );
        assert_eq!(
            unsafe { std::slice::from_raw_parts(output.data, output.len) },
            b"[1]"
        );
        unsafe { ofd_buffer_free(output) };
        assert_eq!(unsafe { ofd_page_count(target, &mut count) }, 0);
        assert_eq!(count, 1);
        unsafe {
            ofd_close(source);
            ofd_close(target);
        }
    }

    #[test]
    fn ffi_imports_one_selected_document_body() {
        let mut target = ptr::null_mut();
        let mut source = ptr::null_mut();
        assert_eq!(unsafe { ofd_new(&mut target) }, 0);
        assert_eq!(unsafe { ofd_new(&mut source) }, 0);
        let first = CString::new("first").unwrap();
        assert_eq!(
            unsafe { ofd_add_text_page(source, first.as_ptr(), 40.0, 30.0) },
            0
        );
        let mut source_index = usize::MAX;
        assert_eq!(
            unsafe { ofd_append_document_body(source, &mut source_index) },
            0
        );
        let selected = CString::new("selected").unwrap();
        assert_eq!(
            unsafe { ofd_append_text_page(source, 1, selected.as_ptr(), 40.0, 30.0) },
            0
        );
        let before = unsafe { document_ref(target).unwrap() }.to_bytes().unwrap();
        let mut output = usize::MAX;
        assert_ne!(
            unsafe { ofd_append_document_body_from(target, source, 1, ptr::null_mut()) },
            0
        );
        assert_ne!(
            unsafe { ofd_append_document_body_from(target, source, 2, &mut output) },
            0
        );
        assert_eq!(
            unsafe { document_ref(target).unwrap() }.to_bytes().unwrap(),
            before
        );
        assert_eq!(
            unsafe { ofd_append_document_body_from(target, source, 1, &mut output) },
            0
        );
        assert_eq!(output, 1);
        assert_eq!(
            unsafe { document_ref(target).unwrap() }
                .page_text(0)
                .unwrap(),
            ["selected"]
        );
        assert_eq!(
            unsafe { ofd_append_document_body_from(source, source, 1, &mut output) },
            0
        );
        assert_eq!(output, 2);
        assert_eq!(
            unsafe { document_ref(source).unwrap() }
                .page_text(2)
                .unwrap(),
            ["selected"]
        );
        unsafe {
            ofd_close(source);
            ofd_close(target);
        }
    }

    #[test]
    fn ffi_appends_document_body_and_pages() {
        let mut handle = ptr::null_mut();
        assert_eq!(unsafe { ofd_new(&mut handle) }, 0);
        let mut index = usize::MAX;
        assert_ne!(
            unsafe { ofd_append_document_body(handle, ptr::null_mut()) },
            0
        );
        assert_eq!(unsafe { ofd_append_document_body(handle, &mut index) }, 0);
        assert_eq!(index, 1);
        assert_eq!(
            unsafe { ofd_append_blank_page(handle, index, 20.0, 20.0) },
            0
        );
        let mut count = 0;
        assert_eq!(unsafe { ofd_page_count(handle, &mut count) }, 0);
        assert_eq!(count, 1);
        unsafe { ofd_close(handle) };
    }

    #[test]
    fn ffi_moves_document_bodies() {
        let mut handle = ptr::null_mut();
        assert_eq!(unsafe { ofd_new(&mut handle) }, 0);
        let first = CString::new("first").unwrap();
        assert_eq!(
            unsafe { ofd_add_text_page(handle, first.as_ptr(), 40.0, 30.0) },
            0
        );
        let mut index = usize::MAX;
        assert_eq!(unsafe { ofd_append_document_body(handle, &mut index) }, 0);
        let second = CString::new("second").unwrap();
        assert_eq!(
            unsafe { ofd_append_text_page(handle, index, second.as_ptr(), 40.0, 30.0) },
            0
        );
        assert_eq!(unsafe { ofd_move_document_body(handle, 0, 1) }, 0);
        assert_eq!(
            unsafe { document_ref(handle).unwrap() }
                .page_text(0)
                .unwrap(),
            ["second"]
        );
        assert_ne!(unsafe { ofd_move_document_body(handle, 0, 2) }, 0);
        assert_ne!(unsafe { ofd_move_document_body(ptr::null_mut(), 0, 1) }, 0);
        assert_eq!(unsafe { ofd_remove_document_body(handle, 1) }, 0);
        assert_eq!(
            unsafe { document_ref(handle).unwrap() }
                .page_text(0)
                .unwrap(),
            ["second"]
        );
        assert_ne!(unsafe { ofd_remove_document_body(handle, 0) }, 0);
        assert_ne!(unsafe { ofd_remove_document_body(ptr::null_mut(), 0) }, 0);
        unsafe { ofd_close(handle) };
    }

    #[test]
    fn ffi_extracts_one_document_body() {
        let mut source = ptr::null_mut();
        assert_eq!(unsafe { ofd_new(&mut source) }, 0);
        let first = CString::new("first").unwrap();
        assert_eq!(
            unsafe { ofd_add_text_page(source, first.as_ptr(), 40.0, 30.0) },
            0
        );
        let mut index = usize::MAX;
        assert_eq!(unsafe { ofd_append_document_body(source, &mut index) }, 0);
        let selected = CString::new("selected").unwrap();
        assert_eq!(
            unsafe { ofd_append_text_page(source, 1, selected.as_ptr(), 40.0, 30.0) },
            0
        );
        let before = unsafe { document_ref(source).unwrap() }.to_bytes().unwrap();
        let mut extracted = ptr::null_mut();
        assert_ne!(
            unsafe { ofd_extract_document_body(source, 1, ptr::null_mut()) },
            0
        );
        assert_ne!(
            unsafe { ofd_extract_document_body(source, 2, &mut extracted) },
            0
        );
        assert!(extracted.is_null());
        assert_eq!(
            unsafe { ofd_extract_document_body(source, 1, &mut extracted) },
            0
        );
        assert_eq!(
            unsafe { document_ref(extracted).unwrap() }
                .page_text(0)
                .unwrap(),
            ["selected"]
        );
        assert_eq!(
            unsafe { document_ref(extracted).unwrap() }
                .document_info()
                .unwrap()
                .len(),
            1
        );
        assert_eq!(
            unsafe { document_ref(source).unwrap() }.to_bytes().unwrap(),
            before
        );
        unsafe {
            ofd_close(extracted);
            ofd_close(source);
        }
    }

    #[test]
    fn ffi_flows_text_across_pages() {
        let mut handle = ptr::null_mut();
        assert_eq!(unsafe { ofd_new(&mut handle) }, 0);
        let payload = CString::new(r#"{"text":"A\nB\nC","page_size":[40,22],"margins":[5,5,5,5],"font_size":5,"line_height":6}"#).unwrap();
        let mut output = OfdBuffer {
            data: ptr::null_mut(),
            len: 0,
        };
        assert_eq!(
            unsafe { ofd_add_flowed_text_json(handle, 0, payload.as_ptr(), &mut output) },
            0
        );
        assert_eq!(
            unsafe { std::slice::from_raw_parts(output.data, output.len) },
            b"[0,1]"
        );
        unsafe { ofd_buffer_free(output) };
        let bad = CString::new(r#"{"text":"bad","margins":[1,2]}"#).unwrap();
        let mut output = OfdBuffer {
            data: ptr::null_mut(),
            len: 0,
        };
        assert_ne!(
            unsafe { ofd_add_flowed_text_json(handle, 0, bad.as_ptr(), &mut output) },
            0
        );
        let aligned = CString::new(r#"{"text":"D","page_size":[40,22],"margins":[5,5,5,5],"font_size":5,"line_height":6,"align":"right","first_line_indent":4}"#).unwrap();
        assert_eq!(
            unsafe { ofd_add_flowed_text_json(handle, 0, aligned.as_ptr(), &mut output) },
            0
        );
        assert_eq!(
            unsafe { std::slice::from_raw_parts(output.data, output.len) },
            b"[2]"
        );
        unsafe { ofd_buffer_free(output) };
        let mut output = OfdBuffer {
            data: ptr::null_mut(),
            len: 0,
        };
        let invalid_align = CString::new(r#"{"text":"bad","align":"justify"}"#).unwrap();
        assert_ne!(
            unsafe { ofd_add_flowed_text_json(handle, 0, invalid_align.as_ptr(), &mut output) },
            0
        );
        let mut count = 0;
        assert_eq!(unsafe { ofd_page_count(handle, &mut count) }, 0);
        assert_eq!(count, 3);
        unsafe { ofd_close(handle) };
    }

    #[test]
    fn ffi_adds_paginated_table_and_rejects_bad_geometry() {
        let mut handle = ptr::null_mut();
        assert_eq!(unsafe { ofd_new(&mut handle) }, 0);
        let input = CString::new(r#"{"rows":[["H1","H2"],["A","B"],["C","D"]],"column_widths":[12,12],"page_size":[30,26],"margins":[2,2,2,2],"row_height":8,"font_size":3,"line_height":3,"padding":1,"repeat_header":true}"#).unwrap();
        let mut output = OfdBuffer {
            data: ptr::null_mut(),
            len: 0,
        };
        assert_eq!(
            unsafe { ofd_add_table_json(handle, 0, input.as_ptr(), &mut output) },
            0
        );
        let indexes: Vec<usize> =
            serde_json::from_slice(unsafe { std::slice::from_raw_parts(output.data, output.len) })
                .unwrap();
        assert_eq!(indexes, [0, 1]);
        unsafe { ofd_buffer_free(output) };
        assert_eq!(
            unsafe { document_ref(handle).unwrap() }
                .page_text(1)
                .unwrap(),
            ["H1", "H2", "C", "D"]
        );
        let before = unsafe { document_ref(handle).unwrap() }.to_bytes().unwrap();
        let invalid = CString::new(r#"{"rows":[["x"]],"column_widths":[0]}"#).unwrap();
        assert_ne!(
            unsafe { ofd_add_table_json(handle, 0, invalid.as_ptr(), ptr::null_mut()) },
            0
        );
        let mut output = OfdBuffer {
            data: ptr::null_mut(),
            len: 0,
        };
        assert_ne!(
            unsafe { ofd_add_table_json(handle, 0, invalid.as_ptr(), &mut output) },
            0
        );
        assert_eq!(
            unsafe { document_ref(handle).unwrap() }.to_bytes().unwrap(),
            before
        );
        let automatic = CString::new(r#"{"rows":[["H1","H2"],["A\nA2",""] ,["B\nB2\nB3",""]],"column_widths":[12,12],"page_size":[30,26],"margins":[2,2,2,2],"row_height":0,"font_size":3,"line_height":3,"padding":1,"repeat_header":true}"#).unwrap();
        assert_eq!(
            unsafe { ofd_add_table_json(handle, 0, automatic.as_ptr(), &mut output) },
            0
        );
        let indexes: Vec<usize> =
            serde_json::from_slice(unsafe { std::slice::from_raw_parts(output.data, output.len) })
                .unwrap();
        assert_eq!(indexes, [2, 3]);
        unsafe { ofd_buffer_free(output) };
        assert_eq!(
            unsafe { document_ref(handle).unwrap() }
                .page_paths(3)
                .unwrap()[2]
                .boundary,
            Some([2.0, 7.0, 12.0, 11.0])
        );
        unsafe { ofd_close(handle) };
    }

    #[test]
    fn ffi_adds_merged_table_cells_and_rejects_invalid_grid() {
        let mut handle = ptr::null_mut();
        assert_eq!(unsafe { ofd_new(&mut handle) }, 0);
        let input = CString::new(r#"{"row_count":4,"cells":[{"row":0,"column":0,"text":"H1"},{"row":0,"column":1,"text":"H2"},{"row":1,"column":0,"row_span":2,"text":"L1\nL2\nL3\nL4"},{"row":1,"column":1,"text":"Top"},{"row":2,"column":1,"text":"Low"},{"row":3,"column":0,"column_span":2,"text":"Wide"}],"column_widths":[12,12],"page_size":[30,23],"margins":[2,2,2,2],"row_height":0,"font_size":3,"line_height":3,"padding":1,"repeat_header":true}"#).unwrap();
        let mut output = OfdBuffer {
            data: ptr::null_mut(),
            len: 0,
        };
        assert_eq!(
            unsafe { ofd_add_table_json(handle, 0, input.as_ptr(), &mut output) },
            0
        );
        let indexes: Vec<usize> =
            serde_json::from_slice(unsafe { std::slice::from_raw_parts(output.data, output.len) })
                .unwrap();
        assert_eq!(indexes, [0, 1]);
        unsafe { ofd_buffer_free(output) };
        assert_eq!(
            unsafe { document_ref(handle).unwrap() }
                .page_paths(0)
                .unwrap()[2]
                .boundary,
            Some([2.0, 7.0, 12.0, 14.0])
        );
        let invalid = CString::new(r#"{"row_count":1,"cells":[{"row":0,"column":0,"row_span":2,"text":"bad"}],"column_widths":[12]}"#).unwrap();
        let before = unsafe { document_ref(handle).unwrap() }.to_bytes().unwrap();
        let mut output = OfdBuffer {
            data: ptr::null_mut(),
            len: 0,
        };
        assert_ne!(
            unsafe { ofd_add_table_json(handle, 0, invalid.as_ptr(), &mut output) },
            0
        );
        assert_eq!(
            unsafe { document_ref(handle).unwrap() }.to_bytes().unwrap(),
            before
        );
        unsafe { ofd_close(handle) };
    }

    #[test]
    fn ffi_styles_individual_table_cells_and_rejects_bad_style() {
        let mut handle = ptr::null_mut();
        assert_eq!(unsafe { ofd_new(&mut handle) }, 0);
        let input = CString::new(r#"{"row_count":1,"cells":[{"row":0,"column":0,"text":"AB","background_color":[20,30,40],"text_color":[1,2,3],"border_width":0.8,"border_color":[255,0,0],"padding":[1,2,1,3],"font_id":"1","letter_spacing":0.5,"bold":true,"italic":true,"underline":true,"strikethrough":true,"align":"right"}],"column_widths":[20],"page_size":[30,20],"margins":[2,2,2,2],"row_height":8,"font_size":3,"line_height":3}"#).unwrap();
        let mut output = OfdBuffer {
            data: ptr::null_mut(),
            len: 0,
        };
        assert_eq!(
            unsafe { ofd_add_table_json(handle, 0, input.as_ptr(), &mut output) },
            0
        );
        unsafe { ofd_buffer_free(output) };
        let document = unsafe { document_ref(handle).unwrap() };
        assert_eq!(document.page_paths(0).unwrap().len(), 4);
        assert!(document.page_paths(0).unwrap()[0].fill);
        let text = &document.page_text_objects(0).unwrap()[0];
        assert_eq!(text.font_id.as_deref(), Some("1"));
        assert_eq!(text.weight, 700);
        assert!(text.italic);
        assert!(text.codes[0].delta_x.is_some());
        let page = &document.pages().unwrap()[0];
        let xml = std::str::from_utf8(document.entry(&page.path).unwrap().unwrap()).unwrap();
        assert!(xml.contains("FillColor Value=\"20 30 40\""));
        assert!(xml.contains("FillColor Value=\"1 2 3\""));
        assert!(xml.contains("StrokeColor Value=\"255 0 0\""));
        assert!(xml.contains("LineWidth=\"0.8\""));
        let before = document.to_bytes().unwrap();
        let mut output = OfdBuffer {
            data: ptr::null_mut(),
            len: 0,
        };
        for bad in [
            r#"{"row_count":1,"cells":[{"row":0,"column":0,"text":"A","align":"justify"}],"column_widths":[20]}"#,
            r#"{"row_count":1,"cells":[{"row":0,"column":0,"text":"A","background_color":[256,0,0]}],"column_widths":[20]}"#,
            r#"{"row_count":1,"cells":[{"row":0,"column":0,"text":"A","bold":"yes"}],"column_widths":[20]}"#,
            r#"{"row_count":1,"cells":[{"row":0,"column":0,"text":"A","italic":1}],"column_widths":[20]}"#,
            r#"{"row_count":1,"cells":[{"row":0,"column":0,"text":"A","font_id":42}],"column_widths":[20]}"#,
            r#"{"row_count":1,"cells":[{"row":0,"column":0,"text":"A","font_id":"999999"}],"column_widths":[20]}"#,
            r#"{"row_count":1,"cells":[{"row":0,"column":0,"text":"A","padding":[1,2,3]}],"column_widths":[20]}"#,
            r#"{"row_count":1,"cells":[{"row":0,"column":0,"text":"A","padding":[-1,1,1,1]}],"column_widths":[20]}"#,
            r#"{"row_count":1,"cells":[{"row":0,"column":0,"text":"A","border_width":-1}],"column_widths":[20]}"#,
            r#"{"row_count":1,"cells":[{"row":0,"column":0,"text":"A","border_color":[256,0,0]}],"column_widths":[20]}"#,
            r#"{"row_count":1,"cells":[{"row":0,"column":0,"text":"A","underline":"yes"}],"column_widths":[20]}"#,
            r#"{"row_count":1,"cells":[{"row":0,"column":0,"text":"A","strikethrough":1}],"column_widths":[20]}"#,
            r#"{"row_count":1,"cells":[{"row":0,"column":0,"text":"A","letter_spacing":"wide"}],"column_widths":[20]}"#,
        ] {
            let input = CString::new(bad).unwrap();
            assert_ne!(
                unsafe { ofd_add_table_json(handle, 0, input.as_ptr(), &mut output) },
                0
            );
            assert_eq!(
                unsafe { document_ref(handle).unwrap() }.to_bytes().unwrap(),
                before
            );
        }
        unsafe { ofd_close(handle) };
    }

    #[test]
    fn ffi_embeds_table_cell_image_and_rejects_bad_base64() {
        let mut source = OfdDocument::new();
        source.add_blank_page(10.0, 10.0).unwrap();
        let png = source.page_png(0, 25.4).unwrap();
        let mut handle = ptr::null_mut();
        assert_eq!(unsafe { ofd_new(&mut handle) }, 0);
        let input = serde_json::json!({
            "row_count": 1,
            "cells": [{"row": 0, "column": 0, "text": "C", "image": {
                "data_base64": STANDARD.encode(&png), "format": "png", "width": 8, "height": 6
            }}],
            "column_widths": [12], "page_size": [20, 20], "margins": [2, 2, 2, 2],
            "row_height": 0, "font_size": 3, "line_height": 3, "padding": 1
        });
        let input = CString::new(input.to_string()).unwrap();
        let mut output = OfdBuffer {
            data: ptr::null_mut(),
            len: 0,
        };
        assert_eq!(
            unsafe { ofd_add_table_json(handle, 0, input.as_ptr(), &mut output) },
            0
        );
        unsafe { ofd_buffer_free(output) };
        let document = unsafe { document_ref(handle).unwrap() };
        assert_eq!(document.page_text(0).unwrap(), ["C"]);
        assert_eq!(
            document.page_images(0).unwrap()[0].boundary,
            Some([3.0, 7.0, 8.0, 6.0])
        );
        assert!(document.validate_references().unwrap().is_empty());
        let before = document.to_bytes().unwrap();
        let bad = CString::new(r#"{"row_count":1,"cells":[{"row":0,"column":0,"image":{"data_base64":"!","format":"png","width":8,"height":6}}],"column_widths":[12]}"#).unwrap();
        let mut output = OfdBuffer {
            data: ptr::null_mut(),
            len: 0,
        };
        assert_ne!(
            unsafe { ofd_add_table_json(handle, 0, bad.as_ptr(), &mut output) },
            0
        );
        assert_eq!(
            unsafe { document_ref(handle).unwrap() }.to_bytes().unwrap(),
            before
        );
        unsafe { ofd_close(handle) };
    }

    #[test]
    fn ffi_adds_ordered_table_cell_blocks() {
        let mut source = OfdDocument::new();
        source.add_blank_page(10.0, 10.0).unwrap();
        let png = source.page_png(0, 25.4).unwrap();
        let mut handle = ptr::null_mut();
        assert_eq!(unsafe { ofd_new(&mut handle) }, 0);
        let input = serde_json::json!({
            "row_count": 1,
            "cells": [{"row": 0, "column": 0, "blocks": [
                {"type": "text", "text": "A", "align": "left"},
                {"type": "image", "data_base64": STANDARD.encode(&png), "format": "png", "width": 6, "height": 4},
                {"type": "text", "text": "B", "text_color": [4, 5, 6], "align": "right"}
            ]}],
            "column_widths": [20], "page_size": [40, 40], "margins": [2, 2, 2, 2],
            "row_height": 0, "font_size": 3, "line_height": 3, "padding": 1
        });
        let input = CString::new(input.to_string()).unwrap();
        let mut output = OfdBuffer {
            data: ptr::null_mut(),
            len: 0,
        };
        assert_eq!(
            unsafe { ofd_add_table_json(handle, 0, input.as_ptr(), &mut output) },
            0
        );
        unsafe { ofd_buffer_free(output) };
        let document = unsafe { document_ref(handle).unwrap() };
        assert_eq!(document.page_text(0).unwrap(), ["A", "B"]);
        assert_eq!(
            document.page_images(0).unwrap()[0].boundary,
            Some([3.0, 7.0, 6.0, 4.0])
        );
        let before = document.to_bytes().unwrap();
        let bad = CString::new(r#"{"row_count":1,"cells":[{"row":0,"column":0,"blocks":[{"type":"video"}]}],"column_widths":[20]}"#).unwrap();
        let mut output = OfdBuffer {
            data: ptr::null_mut(),
            len: 0,
        };
        assert_ne!(
            unsafe { ofd_add_table_json(handle, 0, bad.as_ptr(), &mut output) },
            0
        );
        assert_eq!(
            unsafe { document_ref(handle).unwrap() }.to_bytes().unwrap(),
            before
        );
        unsafe { ofd_close(handle) };
    }

    #[test]
    fn ffi_adds_table_spacer_and_rule_blocks() {
        let mut handle = ptr::null_mut();
        assert_eq!(unsafe { ofd_new(&mut handle) }, 0);
        let input = CString::new(
            serde_json::json!({
                "row_count": 1,
                "cells": [{"row": 0, "column": 0, "blocks": [
                    {"type": "spacer", "height": 3},
                    {"type": "rule", "thickness": 1, "color": [255, 0, 0]},
                    {"type": "spacer", "height": 2}
                ]}],
                "column_widths": [20], "page_size": [40, 40], "margins": [2, 2, 2, 2],
                "row_height": 0, "font_size": 3, "line_height": 3, "padding": 1
            })
            .to_string(),
        )
        .unwrap();
        let mut output = OfdBuffer {
            data: ptr::null_mut(),
            len: 0,
        };
        assert_eq!(
            unsafe { ofd_add_table_json(handle, 0, input.as_ptr(), &mut output) },
            0
        );
        unsafe { ofd_buffer_free(output) };
        let document = unsafe { document_ref(handle).unwrap() };
        assert_eq!(
            document.page_paths(0).unwrap()[1].boundary,
            Some([3.0, 7.0, 18.0, 1.0])
        );
        unsafe { ofd_close(handle) };
    }

    #[test]
    fn ffi_allows_image_only_table_without_font() {
        let mut source = OfdDocument::new();
        source.add_blank_page(10.0, 10.0).unwrap();
        let png = source.page_png(0, 25.4).unwrap();
        let mut fontless =
            OfdDocument::from_bytes(&OfdDocument::new().to_bytes().unwrap()).unwrap();
        let root = "Doc_0/Document.xml";
        let xml = std::str::from_utf8(fontless.entry(root).unwrap().unwrap()).unwrap();
        fontless
            .set_entry(
                root,
                xml.replace("<ofd:PublicRes>PublicRes.xml</ofd:PublicRes>", "")
                    .into_bytes(),
            )
            .unwrap();
        assert!(fontless.fonts().unwrap().is_empty());
        let dir = tempdir().unwrap();
        let path = dir.path().join("fontless.ofd");
        fontless.save(&path).unwrap();
        let path = CString::new(path.to_str().unwrap()).unwrap();
        let mut handle = ptr::null_mut();
        assert_eq!(unsafe { ofd_open(path.as_ptr(), &mut handle) }, 0);
        let input = serde_json::json!({
            "row_count": 1,
            "cells": [{"row": 0, "column": 0, "blocks": [{"type": "image",
                "data_base64": STANDARD.encode(&png), "format": "png", "width": 8, "height": 6}]}],
            "column_widths": [12], "page_size": [20, 20], "margins": [2, 2, 2, 2],
            "row_height": 0, "font_size": 3, "line_height": 3, "padding": 1
        });
        let input = CString::new(input.to_string()).unwrap();
        let mut output = OfdBuffer {
            data: ptr::null_mut(),
            len: 0,
        };
        assert_eq!(
            unsafe { ofd_add_table_json(handle, 0, input.as_ptr(), &mut output) },
            0
        );
        unsafe { ofd_buffer_free(output) };
        assert_eq!(
            unsafe { document_ref(handle).unwrap() }
                .page_images(0)
                .unwrap()
                .len(),
            1
        );
        let before = unsafe { document_ref(handle).unwrap() }.to_bytes().unwrap();
        let text = CString::new(
            r#"{"row_count":1,"cells":[{"row":0,"column":0,"text":"font"}],"column_widths":[12]}"#,
        )
        .unwrap();
        let mut output = OfdBuffer {
            data: ptr::null_mut(),
            len: 0,
        };
        assert_ne!(
            unsafe { ofd_add_table_json(handle, 0, text.as_ptr(), &mut output) },
            0
        );
        assert_eq!(
            unsafe { document_ref(handle).unwrap() }.to_bytes().unwrap(),
            before
        );
        unsafe { ofd_close(handle) };
    }

    #[test]
    fn ffi_flows_styled_spans_and_rejects_invalid_color() {
        let mut handle = ptr::null_mut();
        assert_eq!(unsafe { ofd_new(&mut handle) }, 0);
        let payload = CString::new(r#"{"spans":[{"text":"AA","bold":true,"fill_color":[255,0,0],"letter_spacing":0.5,"underline":true,"underline_offset":0.5,"underline_width":0.2,"line_break_after":true},{"text":"B\nC","italic":true,"font_size":3,"keep_together":true}],"page_size":[40,16],"margins":[5,5,5,5],"font_size":5,"line_height":6}"#).unwrap();
        let mut output = OfdBuffer {
            data: ptr::null_mut(),
            len: 0,
        };
        assert_eq!(
            unsafe { ofd_add_flowed_spans_json(handle, 0, payload.as_ptr(), &mut output) },
            0
        );
        assert_eq!(
            unsafe { std::slice::from_raw_parts(output.data, output.len) },
            b"[0,1,2]"
        );
        unsafe { ofd_buffer_free(output) };
        let mut paths = OfdBuffer {
            data: ptr::null_mut(),
            len: 0,
        };
        assert_eq!(unsafe { ofd_page_paths_json(handle, 0, &mut paths) }, 0);
        let parsed: serde_json::Value =
            serde_json::from_slice(unsafe { std::slice::from_raw_parts(paths.data, paths.len) })
                .unwrap();
        assert_eq!(parsed.as_array().unwrap().len(), 1);
        unsafe { ofd_buffer_free(paths) };
        let bad = CString::new(r#"{"spans":[{"text":"bad","fill_color":[256,0,0]}]}"#).unwrap();
        let mut output = OfdBuffer {
            data: ptr::null_mut(),
            len: 0,
        };
        assert_ne!(
            unsafe { ofd_add_flowed_spans_json(handle, 0, bad.as_ptr(), &mut output) },
            0
        );
        let mut count = 0;
        assert_eq!(unsafe { ofd_page_count(handle, &mut count) }, 0);
        assert_eq!(count, 3);
        unsafe { ofd_close(handle) };
    }

    #[test]
    fn ffi_flowed_text_and_spans_keep_literal_spaces() {
        let mut handle = ptr::null_mut();
        assert_eq!(unsafe { ofd_new(&mut handle) }, 0);
        for (page_index, payload) in [
            r#"{"text":" A \n B ","page_size":[40,22],"margins":[5,5,5,5]}"#,
            r#"{"spans":[{"text":" A \n B "}],"page_size":[40,22],"margins":[5,5,5,5]}"#,
        ]
        .into_iter()
        .enumerate()
        {
            let input = CString::new(payload).unwrap();
            let mut output = OfdBuffer {
                data: ptr::null_mut(),
                len: 0,
            };
            let status = if page_index == 0 {
                unsafe { ofd_add_flowed_text_json(handle, 0, input.as_ptr(), &mut output) }
            } else {
                unsafe { ofd_add_flowed_spans_json(handle, 0, input.as_ptr(), &mut output) }
            };
            assert_eq!(status, 0);
            unsafe { ofd_buffer_free(output) };
            let mut text = OfdBuffer {
                data: ptr::null_mut(),
                len: 0,
            };
            assert_eq!(
                unsafe { ofd_page_text_json(handle, page_index, &mut text) },
                0
            );
            assert_eq!(
                unsafe { std::slice::from_raw_parts(text.data, text.len) },
                br#"[" A "," B "]"#
            );
            unsafe { ofd_buffer_free(text) };
        }
        unsafe { ofd_close(handle) };
    }

    #[test]
    fn ffi_replaces_unshaped_text_code_atomically() {
        let mut handle = ptr::null_mut();
        assert_eq!(unsafe { ofd_new(&mut handle) }, 0);
        let original = CString::new("AB").unwrap();
        assert_eq!(
            unsafe { ofd_add_text_page(handle, original.as_ptr(), 20.0, 20.0) },
            0
        );
        let id = unsafe { document_ref(handle).unwrap() }
            .page_text_objects(0)
            .unwrap()[0]
            .id
            .clone()
            .unwrap();
        let id = CString::new(id).unwrap();
        let replacement = CString::new("A & <B>").unwrap();
        assert_eq!(
            unsafe { ofd_set_page_text_code(handle, 0, id.as_ptr(), 0, replacement.as_ptr()) },
            0
        );
        assert_eq!(
            unsafe { document_ref(handle).unwrap() }
                .page_text(0)
                .unwrap(),
            ["A & <B>"]
        );
        let before = unsafe { document_ref(handle).unwrap() }.to_bytes().unwrap();
        assert_ne!(
            unsafe { ofd_set_page_text_code(handle, 0, id.as_ptr(), 1, replacement.as_ptr()) },
            0
        );
        assert_eq!(
            unsafe { document_ref(handle).unwrap() }.to_bytes().unwrap(),
            before
        );
        unsafe { ofd_close(handle) };
    }

    #[test]
    fn ffi_sets_text_code_layout_atomically() {
        let mut handle = ptr::null_mut();
        assert_eq!(unsafe { ofd_new(&mut handle) }, 0);
        let text = CString::new("AB").unwrap();
        assert_eq!(
            unsafe { ofd_add_text_page(handle, text.as_ptr(), 30.0, 20.0) },
            0
        );
        let id = unsafe { document_ref(handle).unwrap() }
            .page_text_objects(0)
            .unwrap()[0]
            .id
            .clone()
            .unwrap();
        let id = CString::new(id).unwrap();
        let layout = CString::new(r#"{"x":2,"y":7,"delta_x":[3.5]}"#).unwrap();
        assert_eq!(
            unsafe {
                ofd_set_page_text_code_layout_json(handle, 0, id.as_ptr(), 0, layout.as_ptr())
            },
            0
        );
        assert_eq!(
            unsafe { document_ref(handle).unwrap() }
                .page_text_objects(0)
                .unwrap()[0]
                .codes[0]
                .delta_x
                .as_deref(),
            Some("3.5")
        );
        let before = unsafe { document_ref(handle).unwrap() }.to_bytes().unwrap();
        let invalid = CString::new(r#"{"x":2,"y":7,"delta_x":[1,2]}"#).unwrap();
        assert_ne!(
            unsafe {
                ofd_set_page_text_code_layout_json(handle, 0, id.as_ptr(), 0, invalid.as_ptr())
            },
            0
        );
        assert_eq!(
            unsafe { document_ref(handle).unwrap() }.to_bytes().unwrap(),
            before
        );
        unsafe { ofd_close(handle) };
    }

    #[test]
    fn ffi_appends_positioned_text_code_atomically() {
        let mut handle = ptr::null_mut();
        assert_eq!(unsafe { ofd_new(&mut handle) }, 0);
        let initial = CString::new("AB").unwrap();
        assert_eq!(
            unsafe { ofd_add_text_page(handle, initial.as_ptr(), 30.0, 20.0) },
            0
        );
        let id = unsafe { document_ref(handle).unwrap() }
            .page_text_objects(0)
            .unwrap()[0]
            .id
            .clone()
            .unwrap();
        let id = CString::new(id).unwrap();
        let text = CString::new("C&D").unwrap();
        let layout = CString::new(r#"{"delta_x":[2,2.5]}"#).unwrap();
        assert_eq!(
            unsafe {
                ofd_append_page_text_code_json(
                    handle,
                    0,
                    id.as_ptr(),
                    text.as_ptr(),
                    layout.as_ptr(),
                )
            },
            0
        );
        assert_eq!(
            unsafe { document_ref(handle).unwrap() }
                .page_text(0)
                .unwrap(),
            ["AB", "C&D"]
        );
        let before = unsafe { document_ref(handle).unwrap() }.to_bytes().unwrap();
        let invalid = CString::new(r#"{"delta_x":[1]}"#).unwrap();
        assert_ne!(
            unsafe {
                ofd_append_page_text_code_json(
                    handle,
                    0,
                    id.as_ptr(),
                    text.as_ptr(),
                    invalid.as_ptr(),
                )
            },
            0
        );
        assert_eq!(
            unsafe { document_ref(handle).unwrap() }.to_bytes().unwrap(),
            before
        );
        unsafe { ofd_close(handle) };
    }

    #[test]
    fn ffi_removes_text_code_without_moving_successor() {
        let mut handle = ptr::null_mut();
        assert_eq!(unsafe { ofd_new(&mut handle) }, 0);
        let initial = CString::new("A").unwrap();
        assert_eq!(
            unsafe { ofd_add_text_page(handle, initial.as_ptr(), 30.0, 20.0) },
            0
        );
        let id = unsafe { document_ref(handle).unwrap() }
            .page_text_objects(0)
            .unwrap()[0]
            .id
            .clone()
            .unwrap();
        let id = CString::new(id).unwrap();
        let second = CString::new("B").unwrap();
        let positioned = CString::new(r#"{"x":5,"y":6}"#).unwrap();
        assert_eq!(
            unsafe {
                ofd_append_page_text_code_json(
                    handle,
                    0,
                    id.as_ptr(),
                    second.as_ptr(),
                    positioned.as_ptr(),
                )
            },
            0
        );
        let third = CString::new("C").unwrap();
        let inherited = CString::new("{}").unwrap();
        assert_eq!(
            unsafe {
                ofd_append_page_text_code_json(
                    handle,
                    0,
                    id.as_ptr(),
                    third.as_ptr(),
                    inherited.as_ptr(),
                )
            },
            0
        );
        assert_eq!(
            unsafe { ofd_remove_page_text_code(handle, 0, id.as_ptr(), 1) },
            0
        );
        let codes = unsafe { document_ref(handle).unwrap() }
            .page_text_objects(0)
            .unwrap()[0]
            .codes
            .clone();
        assert_eq!((codes[1].x, codes[1].y), (Some(5.0), Some(6.0)));
        let before = unsafe { document_ref(handle).unwrap() }.to_bytes().unwrap();
        assert_ne!(
            unsafe { ofd_remove_page_text_code(handle, 0, id.as_ptr(), 9) },
            0
        );
        assert_eq!(
            unsafe { document_ref(handle).unwrap() }.to_bytes().unwrap(),
            before
        );
        unsafe { ofd_close(handle) };
    }

    #[test]
    fn ffi_sets_text_font_and_rejects_invalid_reference() {
        let font = std::fs::read(
            PathBuf::from(env!("CARGO_MANIFEST_DIR"))
                .join("../ofdrw/tests/fixtures/dejavu-serif.ttf"),
        )
        .unwrap();
        let mut handle = ptr::null_mut();
        assert_eq!(unsafe { ofd_new(&mut handle) }, 0);
        let text = CString::new("AB").unwrap();
        assert_eq!(
            unsafe { ofd_add_text_page(handle, text.as_ptr(), 30.0, 20.0) },
            0
        );
        let id = unsafe { document_ref(handle).unwrap() }
            .page_text_objects(0)
            .unwrap()[0]
            .id
            .clone()
            .unwrap();
        let id = CString::new(id).unwrap();
        let name = CString::new("DejaVu Serif").unwrap();
        let mut font_id = 0;
        assert_eq!(
            unsafe {
                ofd_add_embedded_font(
                    handle,
                    name.as_ptr(),
                    font.as_ptr(),
                    font.len(),
                    &mut font_id,
                )
            },
            0
        );
        let font_id = CString::new(font_id.to_string()).unwrap();
        assert_eq!(
            unsafe { ofd_set_page_text_font(handle, 0, id.as_ptr(), font_id.as_ptr()) },
            0
        );
        assert_eq!(
            unsafe { document_ref(handle).unwrap() }
                .page_text_objects(0)
                .unwrap()[0]
                .font_id
                .as_deref(),
            Some(font_id.to_str().unwrap())
        );
        let before = unsafe { document_ref(handle).unwrap() }.to_bytes().unwrap();
        let missing = CString::new("999999").unwrap();
        assert_ne!(
            unsafe { ofd_set_page_text_font(handle, 0, id.as_ptr(), missing.as_ptr()) },
            0
        );
        assert_eq!(
            unsafe { document_ref(handle).unwrap() }.to_bytes().unwrap(),
            before
        );
        unsafe { ofd_close(handle) };
    }

    #[test]
    fn ffi_sets_text_paint_flags_atomically() {
        let mut handle = ptr::null_mut();
        assert_eq!(unsafe { ofd_new(&mut handle) }, 0);
        let text = CString::new("AB").unwrap();
        assert_eq!(
            unsafe { ofd_add_text_page(handle, text.as_ptr(), 30.0, 20.0) },
            0
        );
        let id = unsafe { document_ref(handle).unwrap() }
            .page_text_objects(0)
            .unwrap()[0]
            .id
            .clone()
            .unwrap();
        let id = CString::new(id).unwrap();
        let flags = CString::new(r#"{"fill":false,"stroke":true}"#).unwrap();
        assert_eq!(
            unsafe { ofd_set_page_text_paint_flags_json(handle, 0, id.as_ptr(), flags.as_ptr()) },
            0
        );
        let paint = unsafe { document_ref(handle).unwrap() }
            .page_object_paints(0)
            .unwrap();
        assert!(!paint[0].fill_enabled);
        assert!(paint[0].stroke_enabled);
        let before = unsafe { document_ref(handle).unwrap() }.to_bytes().unwrap();
        let invalid = CString::new(r#"{"fill":"false"}"#).unwrap();
        assert_ne!(
            unsafe { ofd_set_page_text_paint_flags_json(handle, 0, id.as_ptr(), invalid.as_ptr()) },
            0
        );
        assert_eq!(
            unsafe { document_ref(handle).unwrap() }.to_bytes().unwrap(),
            before
        );
        let reset = CString::new("{}").unwrap();
        assert_eq!(
            unsafe { ofd_set_page_text_paint_flags_json(handle, 0, id.as_ptr(), reset.as_ptr()) },
            0
        );
        let paint = unsafe { document_ref(handle).unwrap() }
            .page_object_paints(0)
            .unwrap();
        assert!(paint[0].fill_enabled);
        assert!(!paint[0].stroke_enabled);
        unsafe { ofd_close(handle) };
    }

    #[test]
    fn ffi_styled_flow_uses_variable_line_heights() {
        let mut handle = ptr::null_mut();
        assert_eq!(unsafe { ofd_new(&mut handle) }, 0);
        let input = CString::new(r#"{"spans":[{"text":"A"},{"text":"x\n","font_size":3},{"text":"B\n","font_size":3},{"text":"C","font_size":3}],"page_size":[40,20],"margins":[5,5,5,5]}"#).unwrap();
        let mut output = OfdBuffer {
            data: ptr::null_mut(),
            len: 0,
        };
        assert_eq!(
            unsafe { ofd_add_flowed_spans_json(handle, 0, input.as_ptr(), &mut output) },
            0
        );
        assert_eq!(
            unsafe { std::slice::from_raw_parts(output.data, output.len) },
            b"[0,1]"
        );
        unsafe { ofd_buffer_free(output) };
        let mut objects_output = OfdBuffer {
            data: ptr::null_mut(),
            len: 0,
        };
        assert_eq!(
            unsafe { ofd_page_text_objects_json(handle, 0, &mut objects_output) },
            0
        );
        let parsed: serde_json::Value = serde_json::from_slice(unsafe {
            std::slice::from_raw_parts(objects_output.data, objects_output.len)
        })
        .unwrap();
        let objects = parsed.as_array().unwrap();
        assert_eq!(objects.len(), 3);
        assert_eq!(objects[0]["read_direction"], 0);
        assert_eq!(objects[0]["char_direction"], 0);
        assert_eq!(objects[0]["weight"], 400);
        assert_eq!(objects[0]["italic"], false);
        assert_eq!(objects[0]["boundary"][3], 6.0);
        assert_eq!(objects[1]["boundary"][3], 6.0);
        assert_eq!(objects[2]["boundary"][1], 11.0);
        assert_eq!(objects[2]["boundary"][3], 4.0);
        let id = CString::new(objects[0]["id"].as_str().unwrap()).unwrap();
        let style = CString::new(r#"{"size":4,"h_scale":0.8,"read_direction":180,"char_direction":90,"weight":700,"italic":true}"#).unwrap();
        assert_eq!(
            unsafe { ofd_set_page_text_typography_json(handle, 0, id.as_ptr(), style.as_ptr()) },
            0
        );
        let text = unsafe { document_ref(handle).unwrap() }
            .page_text_objects(0)
            .unwrap();
        assert_eq!(text[0].size, Some(4.0));
        assert_eq!(text[0].read_direction, 180);
        assert_eq!(text[0].weight, 700);
        let before = unsafe { document_ref(handle).unwrap() }.to_bytes().unwrap();
        let invalid = CString::new(r#"{"size":4,"weight":750}"#).unwrap();
        assert_ne!(
            unsafe { ofd_set_page_text_typography_json(handle, 0, id.as_ptr(), invalid.as_ptr()) },
            0
        );
        assert_eq!(
            unsafe { document_ref(handle).unwrap() }.to_bytes().unwrap(),
            before
        );
        unsafe { ofd_buffer_free(objects_output) };
        unsafe { ofd_close(handle) };
    }

    #[test]
    fn ffi_flowed_text_rejects_overwide_glyph_without_mutation() {
        let mut handle = ptr::null_mut();
        assert_eq!(unsafe { ofd_new(&mut handle) }, 0);
        for (is_styled, payload) in [
            (
                false,
                r#"{"text":"中","page_size":[11,20],"margins":[5,5,5,5]}"#,
            ),
            (
                true,
                r#"{"spans":[{"text":"中"}],"page_size":[11,20],"margins":[5,5,5,5]}"#,
            ),
        ] {
            let input = CString::new(payload).unwrap();
            let mut output = OfdBuffer {
                data: ptr::null_mut(),
                len: 0,
            };
            let status = if is_styled {
                unsafe { ofd_add_flowed_spans_json(handle, 0, input.as_ptr(), &mut output) }
            } else {
                unsafe { ofd_add_flowed_text_json(handle, 0, input.as_ptr(), &mut output) }
            };
            assert_ne!(status, 0);
            let mut count = 0;
            assert_eq!(unsafe { ofd_page_count(handle, &mut count) }, 0);
            assert_eq!(count, 0);
        }
        unsafe { ofd_close(handle) };
    }

    #[test]
    fn ffi_flowed_spans_accept_weight_fill_and_stroke() {
        let mut handle = ptr::null_mut();
        assert_eq!(unsafe { ofd_new(&mut handle) }, 0);
        let input = CString::new(r#"{"spans":[{"text":"描边","weight":600,"bold":true,"fill":false,"stroke":true,"stroke_color":[20,40,60],"stroke_width":0.2}],"page_size":[40,20],"margins":[5,5,5,5]}"#).unwrap();
        let mut output = OfdBuffer {
            data: ptr::null_mut(),
            len: 0,
        };
        assert_eq!(
            unsafe { ofd_add_flowed_spans_json(handle, 0, input.as_ptr(), &mut output) },
            0
        );
        unsafe { ofd_buffer_free(output) };
        let mut svg = OfdBuffer {
            data: ptr::null_mut(),
            len: 0,
        };
        assert_eq!(unsafe { ofd_page_svg(handle, 0, &mut svg) }, 0);
        let content =
            String::from_utf8(unsafe { std::slice::from_raw_parts(svg.data, svg.len) }.to_vec())
                .unwrap();
        assert!(content.contains("font-weight=\"600\""));
        assert!(content.contains("fill=\"none\""));
        assert!(content.contains("stroke=\"#14283c\""));
        unsafe { ofd_buffer_free(svg) };
        let invalid = CString::new(r#"{"spans":[{"text":"bad","weight":650}]}"#).unwrap();
        let mut output = OfdBuffer {
            data: ptr::null_mut(),
            len: 0,
        };
        assert_ne!(
            unsafe { ofd_add_flowed_spans_json(handle, 0, invalid.as_ptr(), &mut output) },
            0
        );
        let mut count = 0;
        assert_eq!(unsafe { ofd_page_count(handle, &mut count) }, 0);
        assert_eq!(count, 1);
        unsafe { ofd_close(handle) };
    }

    #[test]
    fn ffi_flowed_text_uses_embedded_font_pair_kerning() {
        let font = std::fs::read(
            PathBuf::from(env!("CARGO_MANIFEST_DIR"))
                .join("../ofdrw/tests/fixtures/kerning-simhei.ttf"),
        )
        .unwrap();
        let mut handle = ptr::null_mut();
        assert_eq!(unsafe { ofd_new(&mut handle) }, 0);
        let name = CString::new("Kerning Test").unwrap();
        let mut font_id = 0;
        assert_eq!(
            unsafe {
                ofd_add_embedded_font(
                    handle,
                    name.as_ptr(),
                    font.as_ptr(),
                    font.len(),
                    &mut font_id,
                )
            },
            0
        );
        for (page, payload) in [
            format!(r#"{{"text":"HW","font_id":"{font_id}","page_size":[14.5,20],"margins":[5,5,5,5]}}"#),
            format!(r#"{{"spans":[{{"text":"HW","keep_together":true}}],"font_id":"{font_id}","page_size":[14.5,20],"margins":[5,5,5,5]}}"#),
        ]
        .into_iter()
        .enumerate()
        {
            let input = CString::new(payload).unwrap();
            let mut output = OfdBuffer { data: ptr::null_mut(), len: 0 };
            let status = if page == 0 {
                unsafe { ofd_add_flowed_text_json(handle, 0, input.as_ptr(), &mut output) }
            } else {
                unsafe { ofd_add_flowed_spans_json(handle, 0, input.as_ptr(), &mut output) }
            };
            assert_eq!(status, 0);
            unsafe { ofd_buffer_free(output) };
            let mut objects = OfdBuffer { data: ptr::null_mut(), len: 0 };
            assert_eq!(unsafe { ofd_page_text_objects_json(handle, page, &mut objects) }, 0);
            let parsed: serde_json::Value = serde_json::from_slice(
                unsafe { std::slice::from_raw_parts(objects.data, objects.len) },
            ).unwrap();
            assert_eq!(parsed[0]["boundary"][2], 3.75);
            assert_eq!(parsed[0]["codes"][0]["delta_x"], "1.25");
            unsafe { ofd_buffer_free(objects) };
        }
        unsafe { ofd_close(handle) };
    }

    #[test]
    fn ffi_flows_shaped_ligatures_without_splitting_them() {
        let font = std::fs::read(
            PathBuf::from(env!("CARGO_MANIFEST_DIR"))
                .join("../ofdrw/tests/fixtures/dejavu-serif.ttf"),
        )
        .unwrap();
        let mut handle = ptr::null_mut();
        assert_eq!(unsafe { ofd_new(&mut handle) }, 0);
        let mut font_id = 0;
        let name = CString::new("DejaVu Serif").unwrap();
        assert_eq!(
            unsafe {
                ofd_add_embedded_font(
                    handle,
                    name.as_ptr(),
                    font.as_ptr(),
                    font.len(),
                    &mut font_id,
                )
            },
            0
        );
        let input = CString::new(format!(r#"{{"text":"fifi","font_id":"{font_id}","page_size":[14.5,22],"margins":[5,5,5,5],"shape":true}}"#)).unwrap();
        let mut output = OfdBuffer {
            data: ptr::null_mut(),
            len: 0,
        };
        assert_eq!(
            unsafe { ofd_add_flowed_text_json(handle, 0, input.as_ptr(), &mut output) },
            0
        );
        assert_eq!(
            unsafe { std::slice::from_raw_parts(output.data, output.len) },
            b"[0]"
        );
        unsafe { ofd_buffer_free(output) };
        let mut objects = OfdBuffer {
            data: ptr::null_mut(),
            len: 0,
        };
        assert_eq!(
            unsafe { ofd_page_text_objects_json(handle, 0, &mut objects) },
            0
        );
        let parsed: serde_json::Value = serde_json::from_slice(unsafe {
            std::slice::from_raw_parts(objects.data, objects.len)
        })
        .unwrap();
        assert_eq!(parsed.as_array().unwrap().len(), 2);
        assert_eq!(parsed[0]["codes"][0]["text"], "fi");
        assert_eq!(parsed[1]["codes"][0]["text"], "fi");
        assert_eq!(parsed[0]["cg_transforms"][0]["code_count"], 2);
        assert_eq!(parsed[0]["cg_transforms"][0]["glyph_count"], 1);
        unsafe { ofd_buffer_free(objects) };
        let invalid = CString::new(format!(
            r#"{{"text":"fi","font_id":"{font_id}","shape":"true"}}"#
        ))
        .unwrap();
        let mut output = OfdBuffer {
            data: ptr::null_mut(),
            len: 0,
        };
        assert_ne!(
            unsafe { ofd_add_flowed_text_json(handle, 0, invalid.as_ptr(), &mut output) },
            0
        );
        let mut count = 0;
        assert_eq!(unsafe { ofd_page_count(handle, &mut count) }, 0);
        assert_eq!(count, 1);
        unsafe { ofd_close(handle) };
    }

    #[test]
    fn ffi_writes_shaped_ligature_and_keeps_original_text() {
        let font = std::fs::read(
            PathBuf::from(env!("CARGO_MANIFEST_DIR"))
                .join("../ofdrw/tests/fixtures/dejavu-serif.ttf"),
        )
        .unwrap();
        let mut handle = ptr::null_mut();
        assert_eq!(unsafe { ofd_new(&mut handle) }, 0);
        let name = CString::new("DejaVu Serif").unwrap();
        let mut font_id = 0;
        assert_eq!(
            unsafe {
                ofd_add_embedded_font(
                    handle,
                    name.as_ptr(),
                    font.as_ptr(),
                    font.len(),
                    &mut font_id,
                )
            },
            0
        );
        assert_eq!(unsafe { ofd_add_blank_page(handle, 40.0, 20.0) }, 0);
        let input = CString::new(format!(r#"{{"text":"AfiB","boundary":[5,5,30,10],"size":6,"font_id":"{font_id}","shape":true,"fill_color":[12,34,56]}}"#)).unwrap();
        let mut id = 0;
        assert_eq!(
            unsafe { ofd_add_text_object_json(handle, 0, input.as_ptr(), &mut id) },
            0
        );
        assert!(id > 0);
        let mut text = OfdBuffer {
            data: ptr::null_mut(),
            len: 0,
        };
        assert_eq!(unsafe { ofd_page_text_json(handle, 0, &mut text) }, 0);
        assert_eq!(
            unsafe { std::slice::from_raw_parts(text.data, text.len) },
            br#"["AfiB"]"#
        );
        unsafe { ofd_buffer_free(text) };
        let mut svg_output = OfdBuffer {
            data: ptr::null_mut(),
            len: 0,
        };
        assert_eq!(unsafe { ofd_page_svg(handle, 0, &mut svg_output) }, 0);
        let svg = String::from_utf8(
            unsafe { std::slice::from_raw_parts(svg_output.data, svg_output.len) }.to_vec(),
        )
        .unwrap();
        assert!(svg.contains("data-ofd-glyph="));
        unsafe { ofd_buffer_free(svg_output) };
        let mut paints = OfdBuffer {
            data: ptr::null_mut(),
            len: 0,
        };
        assert_eq!(
            unsafe { ofd_page_object_paints_json(handle, 0, &mut paints) },
            0
        );
        let paint_values: serde_json::Value =
            serde_json::from_slice(unsafe { std::slice::from_raw_parts(paints.data, paints.len) })
                .unwrap();
        assert_eq!(paint_values[0]["fill"]["value"], "12 34 56");
        unsafe { ofd_buffer_free(paints) };
        let mut objects = OfdBuffer {
            data: ptr::null_mut(),
            len: 0,
        };
        assert_eq!(
            unsafe { ofd_page_text_objects_json(handle, 0, &mut objects) },
            0
        );
        let values: serde_json::Value = serde_json::from_slice(unsafe {
            std::slice::from_raw_parts(objects.data, objects.len)
        })
        .unwrap();
        assert_eq!(values[0]["cg_transforms"][1]["code_position"], 1);
        assert_eq!(values[0]["cg_transforms"][1]["code_count"], 2);
        assert_eq!(values[0]["cg_transforms"][1]["glyph_count"], 1);
        assert_eq!(
            values[0]["cg_transforms"][1]["glyphs"]
                .as_array()
                .unwrap()
                .len(),
            1
        );
        unsafe { ofd_buffer_free(objects) };
        let invalid = CString::new(format!(r#"{{"text":"A\nB","boundary":[5,5,30,10],"size":6,"font_id":"{font_id}","shape":true}}"#)).unwrap();
        assert_ne!(
            unsafe { ofd_add_text_object_json(handle, 0, invalid.as_ptr(), &mut id) },
            0
        );
        let invalid_color = CString::new(format!(r#"{{"text":"AfiB","boundary":[5,5,30,10],"size":6,"font_id":"{font_id}","shape":true,"fill_color":[256,0,0]}}"#)).unwrap();
        assert_ne!(
            unsafe { ofd_add_text_object_json(handle, 0, invalid_color.as_ptr(), &mut id) },
            0
        );
        unsafe { ofd_close(handle) };
    }

    #[test]
    fn ffi_lists_page_metadata() {
        let mut handle = ptr::null_mut();
        assert_eq!(unsafe { ofd_new(&mut handle) }, 0);
        let text = CString::new("page").unwrap();
        assert_eq!(
            unsafe { ofd_add_text_page(handle, text.as_ptr(), 180.0, 250.0) },
            0
        );
        let mut output = OfdBuffer {
            data: ptr::null_mut(),
            len: 0,
        };
        assert_eq!(unsafe { ofd_pages_json(handle, &mut output) }, 0);
        let json = unsafe { std::slice::from_raw_parts(output.data, output.len) };
        let values: serde_json::Value = serde_json::from_slice(json).unwrap();
        assert_eq!(
            values[0]["physical_box"],
            serde_json::json!([0.0, 0.0, 180.0, 250.0])
        );
        unsafe {
            ofd_buffer_free(output);
        }
        output = OfdBuffer {
            data: ptr::null_mut(),
            len: 0,
        };
        assert_eq!(unsafe { ofd_entries_json(handle, &mut output) }, 0);
        let json = unsafe { std::slice::from_raw_parts(output.data, output.len) };
        let values: serde_json::Value = serde_json::from_slice(json).unwrap();
        assert!(values
            .as_array()
            .unwrap()
            .iter()
            .any(|name| name == "OFD.xml"));
        unsafe {
            ofd_buffer_free(output);
        }
        output = OfdBuffer {
            data: ptr::null_mut(),
            len: 0,
        };
        assert_eq!(
            unsafe { ofd_page_text_objects_json(handle, 0, &mut output) },
            0
        );
        let json = unsafe { std::slice::from_raw_parts(output.data, output.len) };
        let values: serde_json::Value = serde_json::from_slice(json).unwrap();
        assert_eq!(
            values[0]["boundary"],
            serde_json::json!([10.0, 10.0, 160.0, 8.0])
        );
        assert_eq!(values[0]["codes"][0]["text"], "page");
        unsafe {
            ofd_buffer_free(output);
            ofd_close(handle);
        }
    }

    #[test]
    fn ffi_sets_page_area_and_rejects_invalid_json() {
        let mut handle = ptr::null_mut();
        assert_eq!(unsafe { ofd_new(&mut handle) }, 0);
        let text = CString::new("area").unwrap();
        assert_eq!(
            unsafe { ofd_add_text_page(handle, text.as_ptr(), 100.0, 100.0) },
            0
        );
        let area = CString::new(
            r#"{"physical_box":[0,0,100,100],"application_box":[25,25,50,50],"content_box":[30,30,40,40]}"#,
        )
        .unwrap();
        assert_eq!(
            unsafe { ofd_set_page_area_json(handle, 0, area.as_ptr()) },
            0
        );
        let mut output = OfdBuffer {
            data: ptr::null_mut(),
            len: 0,
        };
        assert_eq!(unsafe { ofd_pages_json(handle, &mut output) }, 0);
        let pages: serde_json::Value =
            serde_json::from_slice(unsafe { std::slice::from_raw_parts(output.data, output.len) })
                .unwrap();
        assert_eq!(
            pages[0]["application_box"],
            serde_json::json!([25.0, 25.0, 50.0, 50.0])
        );
        assert_eq!(
            pages[0]["content_box"],
            serde_json::json!([30.0, 30.0, 40.0, 40.0])
        );
        unsafe { ofd_buffer_free(output) };
        for invalid in [
            r#"{"application_box":[25,25,50,50]}"#,
            r#"{"physical_box":[0,0,100]}"#,
            r#"{"physical_box":[0,0,0,100]}"#,
            "[]",
        ] {
            let value = CString::new(invalid).unwrap();
            assert_ne!(
                unsafe { ofd_set_page_area_json(handle, 0, value.as_ptr()) },
                0
            );
        }
        assert_ne!(
            unsafe { ofd_set_page_area_json(handle, 1, area.as_ptr()) },
            0
        );
        unsafe { ofd_close(handle) };
    }

    #[test]
    fn ffi_document_page_area_round_trips() {
        let mut handle = ptr::null_mut();
        assert_eq!(unsafe { ofd_new(&mut handle) }, 0);
        let mut output = OfdBuffer {
            data: ptr::null_mut(),
            len: 0,
        };
        assert_eq!(
            unsafe { ofd_document_page_area_json(handle, 0, &mut output) },
            0
        );
        let initial: serde_json::Value =
            serde_json::from_slice(unsafe { std::slice::from_raw_parts(output.data, output.len) })
                .unwrap();
        unsafe { ofd_buffer_free(output) };
        assert_eq!(
            initial["physical_box"],
            serde_json::json!([0.0, 0.0, 210.0, 297.0])
        );
        let area = CString::new(r#"{"physical_box":[0,0,150,60],"application_box":[10,5,130,50]}"#)
            .unwrap();
        assert_eq!(
            unsafe { ofd_set_document_page_area_json(handle, 0, area.as_ptr()) },
            0
        );
        let mut output = OfdBuffer {
            data: ptr::null_mut(),
            len: 0,
        };
        assert_eq!(
            unsafe { ofd_document_page_area_json(handle, 0, &mut output) },
            0
        );
        let changed: serde_json::Value =
            serde_json::from_slice(unsafe { std::slice::from_raw_parts(output.data, output.len) })
                .unwrap();
        unsafe { ofd_buffer_free(output) };
        assert_eq!(
            changed["physical_box"],
            serde_json::json!([0.0, 0.0, 150.0, 60.0])
        );
        assert_eq!(
            changed["application_box"],
            serde_json::json!([10.0, 5.0, 130.0, 50.0])
        );
        let invalid = CString::new(r#"{"physical_box":[0,0,-1,60]}"#).unwrap();
        assert_ne!(
            unsafe { ofd_set_document_page_area_json(handle, 0, invalid.as_ptr()) },
            0
        );
        unsafe { ofd_close(handle) };
    }

    #[test]
    fn ffi_integrity_manifest_reports_package_files() {
        let mut handle = ptr::null_mut();
        assert_eq!(unsafe { ofd_new(&mut handle) }, 0);
        let mut output = OfdBuffer {
            data: ptr::null_mut(),
            len: 0,
        };
        assert_eq!(
            unsafe { ofd_integrity_manifest_json(handle, &mut output) },
            0
        );
        assert_eq!(
            unsafe { std::slice::from_raw_parts(output.data, output.len) },
            b"null"
        );
        unsafe { ofd_buffer_free(output) };
        let xml = b"<DocEntries ID=\"m\"><SignedValueLoc>/signedvalue.dat</SignedValueLoc><FileList><File ID=\"1\" FileLoc=\"/OFD.xml\"/><File ID=\"2\" FileLoc=\"/Doc_0/Document.xml\"/><File ID=\"3\" FileLoc=\"/Doc_0/PublicRes.xml\"/></FileList></DocEntries>";
        let path = CString::new("OFDEntries.xml").unwrap();
        assert_eq!(
            unsafe { ofd_set_entry(handle, path.as_ptr(), xml.as_ptr(), xml.len()) },
            0
        );
        let path = CString::new("signedvalue.dat").unwrap();
        assert_eq!(
            unsafe { ofd_set_entry(handle, path.as_ptr(), b"sig".as_ptr(), 3) },
            0
        );
        let mut output = OfdBuffer {
            data: ptr::null_mut(),
            len: 0,
        };
        assert_eq!(
            unsafe { ofd_integrity_manifest_json(handle, &mut output) },
            0
        );
        let manifest: serde_json::Value =
            serde_json::from_slice(unsafe { std::slice::from_raw_parts(output.data, output.len) })
                .unwrap();
        unsafe { ofd_buffer_free(output) };
        assert_eq!(manifest["files"].as_array().unwrap().len(), 3);
        let mut output = OfdBuffer {
            data: ptr::null_mut(),
            len: 0,
        };
        assert_eq!(
            unsafe { ofd_check_integrity_manifest_json(handle, &mut output) },
            0
        );
        let report: serde_json::Value =
            serde_json::from_slice(unsafe { std::slice::from_raw_parts(output.data, output.len) })
                .unwrap();
        unsafe { ofd_buffer_free(output) };
        assert_eq!(report["complete"], true);
        unsafe { ofd_close(handle) };
    }

    #[test]
    fn ffi_verifies_java_sm2_integrity_signature() {
        let fixture = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("../../../ofdrw-crypto/src/test/resources/hello-integrity.ofd");
        let path = CString::new(fixture.to_str().unwrap()).unwrap();
        let mut handle = ptr::null_mut();
        assert_eq!(unsafe { ofd_open(path.as_ptr(), &mut handle) }, 0);
        for (distid, expected) in [(None, true), (Some("wrong-id"), false)] {
            let distid = distid.map(|value| CString::new(value).unwrap());
            let mut output = OfdBuffer {
                data: ptr::null_mut(),
                len: 0,
            };
            assert_eq!(
                unsafe {
                    ofd_verify_integrity_manifest_sm2_json(
                        handle,
                        distid.as_ref().map_or(ptr::null(), |value| value.as_ptr()),
                        &mut output,
                    )
                },
                0
            );
            let report: serde_json::Value = serde_json::from_slice(unsafe {
                std::slice::from_raw_parts(output.data, output.len)
            })
            .unwrap();
            unsafe { ofd_buffer_free(output) };
            assert_eq!(report["manifest_complete"], true);
            assert_eq!(report["signature_valid"], expected);
            assert_eq!(report["all_valid"], expected);
        }
        unsafe { ofd_close(handle) };
    }

    #[test]
    fn ffi_creates_sm2_integrity_protection() {
        let key = include_bytes!("../../ofdrw/tests/fixtures/user-private-key.bin");
        let certificate = include_bytes!("../../ofdrw/tests/fixtures/user-cert.der");
        let mut handle = ptr::null_mut();
        assert_eq!(unsafe { ofd_new(&mut handle) }, 0);
        assert_eq!(
            unsafe {
                ofd_protect_integrity_sm2(
                    handle,
                    key.as_ptr(),
                    key.len(),
                    certificate.as_ptr(),
                    certificate.len(),
                    ptr::null(),
                )
            },
            0
        );
        let mut output = OfdBuffer {
            data: ptr::null_mut(),
            len: 0,
        };
        assert_eq!(
            unsafe { ofd_verify_integrity_manifest_sm2_json(handle, ptr::null(), &mut output) },
            0
        );
        let report: serde_json::Value =
            serde_json::from_slice(unsafe { std::slice::from_raw_parts(output.data, output.len) })
                .unwrap();
        unsafe { ofd_buffer_free(output) };
        assert_eq!(report["all_valid"], true);
        assert_ne!(
            unsafe {
                ofd_protect_integrity_sm2(
                    handle,
                    key.as_ptr(),
                    key.len(),
                    certificate.as_ptr(),
                    certificate.len(),
                    ptr::null(),
                )
            },
            0
        );
        unsafe { ofd_close(handle) };
    }

    #[test]
    fn ffi_verifies_integrity_with_ordered_sm2_chain() {
        let key = include_bytes!("../../ofdrw/tests/fixtures/trust-leaf-private-key.bin");
        let signer = include_bytes!("../../ofdrw/tests/fixtures/trust-leaf-cert.der");
        let anchor = include_bytes!("../../ofdrw/tests/fixtures/trust-root-cert.der");
        let mut handle = ptr::null_mut();
        assert_eq!(unsafe { ofd_new(&mut handle) }, 0);
        assert_eq!(
            unsafe {
                ofd_protect_integrity_sm2(
                    handle,
                    key.as_ptr(),
                    key.len(),
                    signer.as_ptr(),
                    signer.len(),
                    ptr::null(),
                )
            },
            0
        );
        let options = CString::new(
            serde_json::json!({
                "signer_certificate_der_base64": STANDARD.encode(signer),
                "intermediates_der_base64": [],
                "anchor_certificate_der_base64": STANDARD.encode(anchor),
                "at_unix_seconds": 1_893_456_000_u64,
            })
            .to_string(),
        )
        .unwrap();
        let mut output = OfdBuffer {
            data: ptr::null_mut(),
            len: 0,
        };
        assert_eq!(
            unsafe {
                ofd_verify_integrity_manifest_sm2_with_ordered_chain_json(
                    handle,
                    options.as_ptr(),
                    &mut output,
                )
            },
            0
        );
        let report: serde_json::Value =
            serde_json::from_slice(unsafe { std::slice::from_raw_parts(output.data, output.len) })
                .unwrap();
        unsafe { ofd_buffer_free(output) };
        assert_eq!(report["certificate_chain_valid"], true);
        assert_eq!(report["all_valid"], true);
        unsafe { ofd_close(handle) };
    }

    #[test]
    fn ffi_builds_integrity_chain_from_unordered_candidates() {
        let key = include_bytes!("../../ofdrw/tests/fixtures/chain-leaf-private-key.bin");
        let signer = include_bytes!("../../ofdrw/tests/fixtures/chain-leaf-cert.der");
        let intermediate = include_bytes!("../../ofdrw/tests/fixtures/chain-intermediate-cert.der");
        let unrelated = include_bytes!("../../ofdrw/tests/fixtures/root-cert.der");
        let anchor = include_bytes!("../../ofdrw/tests/fixtures/chain-root-cert.der");
        let mut handle = ptr::null_mut();
        assert_eq!(unsafe { ofd_new(&mut handle) }, 0);
        assert_eq!(
            unsafe {
                ofd_protect_integrity_sm2(
                    handle,
                    key.as_ptr(),
                    key.len(),
                    signer.as_ptr(),
                    signer.len(),
                    ptr::null(),
                )
            },
            0
        );
        let options = CString::new(serde_json::json!({
            "signer_certificate_der_base64": STANDARD.encode(signer),
            "intermediates_der_base64": [STANDARD.encode(unrelated), STANDARD.encode(intermediate)],
            "anchor_certificate_der_base64": STANDARD.encode(anchor),
            "at_unix_seconds": 1_893_456_000_u64,
        }).to_string()).unwrap();
        let mut output = OfdBuffer {
            data: ptr::null_mut(),
            len: 0,
        };
        assert_eq!(
            unsafe {
                ofd_verify_integrity_manifest_sm2_with_certificate_pool_json(
                    handle,
                    options.as_ptr(),
                    &mut output,
                )
            },
            0
        );
        let report: serde_json::Value =
            serde_json::from_slice(unsafe { std::slice::from_raw_parts(output.data, output.len) })
                .unwrap();
        unsafe { ofd_buffer_free(output) };
        assert_eq!(report["all_valid"], true);
        unsafe { ofd_close(handle) };
    }

    #[test]
    fn ffi_reads_invoice_graphics() {
        let fixture = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("../../../ofdrw-reader/src/test/resources/发票示例.ofd");
        let path = CString::new(fixture.to_str().unwrap()).unwrap();
        let mut handle = ptr::null_mut();
        assert_eq!(unsafe { ofd_open(path.as_ptr(), &mut handle) }, 0);
        let mut output = OfdBuffer {
            data: ptr::null_mut(),
            len: 0,
        };
        assert_eq!(unsafe { ofd_page_paths_json(handle, 0, &mut output) }, 0);
        let values: serde_json::Value =
            serde_json::from_slice(unsafe { std::slice::from_raw_parts(output.data, output.len) })
                .unwrap();
        assert!(!values.as_array().unwrap().is_empty());
        unsafe {
            ofd_buffer_free(output);
        }

        output = OfdBuffer {
            data: ptr::null_mut(),
            len: 0,
        };
        assert_eq!(unsafe { ofd_page_images_json(handle, 0, &mut output) }, 0);
        let values: serde_json::Value =
            serde_json::from_slice(unsafe { std::slice::from_raw_parts(output.data, output.len) })
                .unwrap();
        assert_eq!(values[0]["media_path"], "Doc_0/Res/image_6920.png");
        unsafe {
            ofd_buffer_free(output);
        }

        output = OfdBuffer {
            data: ptr::null_mut(),
            len: 0,
        };
        assert_eq!(unsafe { ofd_page_image_data(handle, 0, 0, &mut output) }, 0);
        assert_eq!(output.len, 2024);
        unsafe {
            ofd_buffer_free(output);
        }
        output = OfdBuffer {
            data: ptr::null_mut(),
            len: 0,
        };
        assert_ne!(unsafe { ofd_page_image_data(handle, 0, 1, &mut output) }, 0);
        unsafe {
            ofd_close(handle);
        }
    }

    #[test]
    fn ffi_reads_java_composite_graphics() {
        let fixture = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("../../../ofdrw-converter/src/test/resources/y.ofd");
        let path = CString::new(fixture.to_str().unwrap()).unwrap();
        let mut handle = ptr::null_mut();
        assert_eq!(unsafe { ofd_open(path.as_ptr(), &mut handle) }, 0);
        let mut output = OfdBuffer {
            data: ptr::null_mut(),
            len: 0,
        };
        assert_eq!(
            unsafe { ofd_page_composite_objects_json(handle, 0, &mut output) },
            0
        );
        let values: serde_json::Value =
            serde_json::from_slice(unsafe { std::slice::from_raw_parts(output.data, output.len) })
                .unwrap();
        assert!(!values.as_array().unwrap().is_empty());
        assert!(values[0]["resource_id"].as_str().is_some());
        let resource_id = values[0]["resource_id"].as_str().unwrap();
        let count = values.as_array().unwrap().len();
        unsafe { ofd_buffer_free(output) };
        let payload = CString::new(format!(
            "{{\"resource_id\":\"{resource_id}\",\"boundary\":[2,3,10,12],\"alpha\":180}}"
        ))
        .unwrap();
        let mut id = 0;
        assert_eq!(
            unsafe { ofd_add_composite_object_json(handle, 0, payload.as_ptr(), &mut id) },
            0
        );
        assert!(id > 0);
        output = OfdBuffer {
            data: ptr::null_mut(),
            len: 0,
        };
        assert_eq!(
            unsafe { ofd_page_composite_objects_json(handle, 0, &mut output) },
            0
        );
        let values: serde_json::Value =
            serde_json::from_slice(unsafe { std::slice::from_raw_parts(output.data, output.len) })
                .unwrap();
        assert_eq!(values.as_array().unwrap().len(), count + 1);
        assert_eq!(values[count]["alpha"], 180);
        unsafe { ofd_buffer_free(output) };
        let id = CString::new(id.to_string()).unwrap();
        assert_eq!(unsafe { ofd_remove_page_object(handle, 0, id.as_ptr()) }, 0);
        output = OfdBuffer {
            data: ptr::null_mut(),
            len: 0,
        };
        assert_ne!(
            unsafe { ofd_page_composite_objects_json(handle, usize::MAX, &mut output) },
            0
        );
        unsafe { ofd_close(handle) };
    }

    #[test]
    fn ffi_creates_composite_path_resource() {
        let mut handle = ptr::null_mut();
        assert_eq!(unsafe { ofd_new(&mut handle) }, 0);
        let text = CString::new("base").unwrap();
        assert_eq!(
            unsafe { ofd_add_text_page(handle, text.as_ptr(), 40.0, 40.0) },
            0
        );
        let path = CString::new(r#"{"commands":"M 0 0 L 10 0 L 10 10 C","boundary":[2,3,10,10],"fill":true,"stroke":false,"fill_color":[255,0,0]}"#).unwrap();
        let mut id = 0;
        assert_eq!(
            unsafe { ofd_add_composite_path_object_json(handle, 0, path.as_ptr(), &mut id) },
            0
        );
        assert!(id > 0);
        let mut output = OfdBuffer {
            data: ptr::null_mut(),
            len: 0,
        };
        assert_eq!(
            unsafe { ofd_page_composite_objects_json(handle, 0, &mut output) },
            0
        );
        let objects: serde_json::Value =
            serde_json::from_slice(unsafe { std::slice::from_raw_parts(output.data, output.len) })
                .unwrap();
        assert_eq!(objects[0]["id"], id.to_string());
        unsafe { ofd_buffer_free(output) };
        output = OfdBuffer {
            data: ptr::null_mut(),
            len: 0,
        };
        assert_eq!(unsafe { ofd_page_svg(handle, 0, &mut output) }, 0);
        let svg =
            std::str::from_utf8(unsafe { std::slice::from_raw_parts(output.data, output.len) })
                .unwrap();
        assert!(svg.contains("fill=\"#ff0000\""));
        unsafe { ofd_buffer_free(output) };
        let invalid = CString::new(r#"{"commands":"M 0","boundary":[2,3,10,10]}"#).unwrap();
        assert_ne!(
            unsafe { ofd_add_composite_path_object_json(handle, 0, invalid.as_ptr(), &mut id) },
            0
        );
        unsafe { ofd_close(handle) };
    }

    #[test]
    fn ffi_creates_composite_text_resource() {
        let mut handle = ptr::null_mut();
        assert_eq!(unsafe { ofd_new(&mut handle) }, 0);
        let base = CString::new("base").unwrap();
        assert_eq!(
            unsafe { ofd_add_text_page(handle, base.as_ptr(), 40.0, 40.0) },
            0
        );
        let payload =
            CString::new(r#"{"text":"Rust & OFD","boundary":[2,3,20,8],"size":5,"font_id":"1"}"#)
                .unwrap();
        let mut id = 0;
        assert_eq!(
            unsafe { ofd_add_composite_text_object_json(handle, 0, payload.as_ptr(), &mut id) },
            0
        );
        assert!(id > 0);
        let mut output = OfdBuffer {
            data: ptr::null_mut(),
            len: 0,
        };
        assert_eq!(unsafe { ofd_page_text_json(handle, 0, &mut output) }, 0);
        let values: serde_json::Value =
            serde_json::from_slice(unsafe { std::slice::from_raw_parts(output.data, output.len) })
                .unwrap();
        assert_eq!(values[1], "Rust & OFD");
        unsafe { ofd_buffer_free(output) };
        let invalid =
            CString::new(r#"{"text":"bad","boundary":[2,3,20,8],"size":5,"font_id":"missing"}"#)
                .unwrap();
        assert_ne!(
            unsafe { ofd_add_composite_text_object_json(handle, 0, invalid.as_ptr(), &mut id) },
            0
        );
        unsafe { ofd_close(handle) };
    }

    #[test]
    fn ffi_creates_image_page() {
        let fixture = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("../../../ofdrw-reader/src/test/resources/发票示例.ofd");
        let source = OfdDocument::open(fixture).unwrap();
        let png = source.page_image_data(0, 0).unwrap();
        let mut handle = ptr::null_mut();
        assert_eq!(unsafe { ofd_new(&mut handle) }, 0);
        let format = CString::new("png").unwrap();
        let truncated = b"\x89PNG\r\n\x1a\n";
        assert_ne!(
            unsafe {
                ofd_add_image_page(
                    handle,
                    truncated.as_ptr(),
                    truncated.len(),
                    format.as_ptr(),
                    210.0,
                    140.0,
                )
            },
            0
        );
        assert!(unsafe { document_ref(handle).unwrap() }
            .pages()
            .unwrap()
            .is_empty());
        assert_eq!(
            unsafe {
                ofd_add_image_page(
                    handle,
                    png.as_ptr(),
                    png.len(),
                    format.as_ptr(),
                    210.0,
                    140.0,
                )
            },
            0
        );
        let mut output = OfdBuffer {
            data: ptr::null_mut(),
            len: 0,
        };
        assert_eq!(unsafe { ofd_page_image_data(handle, 0, 0, &mut output) }, 0);
        assert_eq!(output.len, png.len());
        assert_eq!(
            unsafe { document_ref(handle).unwrap() }
                .page_images(0)
                .unwrap()[0]
                .ctm,
            Some([210.0, 0.0, 0.0, 140.0, 0.0, 0.0])
        );
        unsafe {
            ofd_buffer_free(output);
            ofd_close(handle);
        }
    }

    #[test]
    fn ffi_creates_path_page() {
        let mut handle = ptr::null_mut();
        assert_eq!(unsafe { ofd_new(&mut handle) }, 0);
        let commands = CString::new("M 0 0 L 30 20").unwrap();
        assert_eq!(
            unsafe {
                ofd_add_path_page(
                    handle,
                    commands.as_ptr(),
                    10.0,
                    10.0,
                    30.0,
                    20.0,
                    210.0,
                    297.0,
                    0,
                    1,
                )
            },
            0
        );
        let mut output = OfdBuffer {
            data: ptr::null_mut(),
            len: 0,
        };
        assert_eq!(unsafe { ofd_page_paths_json(handle, 0, &mut output) }, 0);
        let values: serde_json::Value =
            serde_json::from_slice(unsafe { std::slice::from_raw_parts(output.data, output.len) })
                .unwrap();
        assert_eq!(values[0]["abbreviated_data"], "M 0 0 L 30 20");
        unsafe {
            ofd_buffer_free(output);
            ofd_close(handle);
        }
    }

    #[test]
    fn ffi_appends_to_existing_document() {
        let fixture = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("../../../ofdrw-reader/src/test/resources/helloworld.ofd");
        let path = CString::new(fixture.to_str().unwrap()).unwrap();
        let mut handle = ptr::null_mut();
        assert_eq!(unsafe { ofd_open(path.as_ptr(), &mut handle) }, 0);
        let text = CString::new("appended").unwrap();
        assert_eq!(
            unsafe { ofd_append_text_page(handle, 0, text.as_ptr(), 180.0, 250.0) },
            0
        );
        let mut count = 0;
        assert_eq!(unsafe { ofd_page_count(handle, &mut count) }, 0);
        assert_eq!(count, 2);
        unsafe {
            ofd_close(handle);
        }
    }

    #[test]
    fn ffi_appends_path_page_to_existing_document() {
        let fixture = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("../../../ofdrw-reader/src/test/resources/helloworld.ofd");
        let path = CString::new(fixture.to_str().unwrap()).unwrap();
        let mut handle = ptr::null_mut();
        assert_eq!(unsafe { ofd_open(path.as_ptr(), &mut handle) }, 0);
        let commands = CString::new("M 0 0 L 10 10").unwrap();
        assert_eq!(
            unsafe {
                ofd_append_path_page(
                    handle,
                    0,
                    commands.as_ptr(),
                    5.0,
                    5.0,
                    20.0,
                    20.0,
                    80.0,
                    60.0,
                    0,
                    1,
                )
            },
            0
        );
        let mut output = OfdBuffer {
            data: ptr::null_mut(),
            len: 0,
        };
        assert_eq!(unsafe { ofd_page_paths_json(handle, 1, &mut output) }, 0);
        let values: serde_json::Value =
            serde_json::from_slice(unsafe { std::slice::from_raw_parts(output.data, output.len) })
                .unwrap();
        assert_eq!(values[0]["abbreviated_data"], "M 0 0 L 10 10");
        unsafe { ofd_buffer_free(output) };
        assert_ne!(
            unsafe {
                ofd_append_path_page(
                    handle,
                    1,
                    commands.as_ptr(),
                    5.0,
                    5.0,
                    20.0,
                    20.0,
                    80.0,
                    60.0,
                    0,
                    1,
                )
            },
            0
        );
        unsafe { ofd_close(handle) };
    }

    #[test]
    fn ffi_appends_image_page_to_existing_document() {
        let invoice = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("../../../ofdrw-reader/src/test/resources/发票示例.ofd");
        let source = OfdDocument::open(invoice).unwrap();
        let png = source.page_image_data(0, 0).unwrap();
        let fixture = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("../../../ofdrw-reader/src/test/resources/helloworld.ofd");
        let path = CString::new(fixture.to_str().unwrap()).unwrap();
        let format = CString::new("png").unwrap();
        let mut handle = ptr::null_mut();
        assert_eq!(unsafe { ofd_open(path.as_ptr(), &mut handle) }, 0);
        assert_eq!(
            unsafe {
                ofd_append_image_page(
                    handle,
                    0,
                    png.as_ptr(),
                    png.len(),
                    format.as_ptr(),
                    50.0,
                    40.0,
                )
            },
            0
        );
        let mut output = OfdBuffer {
            data: ptr::null_mut(),
            len: 0,
        };
        assert_eq!(unsafe { ofd_page_image_data(handle, 1, 0, &mut output) }, 0);
        let written = unsafe { std::slice::from_raw_parts(output.data, output.len) };
        assert_eq!(written, png);
        unsafe { ofd_buffer_free(output) };
        assert_ne!(
            unsafe {
                ofd_append_image_page(
                    handle,
                    1,
                    png.as_ptr(),
                    png.len(),
                    format.as_ptr(),
                    50.0,
                    40.0,
                )
            },
            0
        );
        unsafe { ofd_close(handle) };
    }

    #[test]
    fn ffi_lists_template_sources_and_text() {
        let fixture = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("../../../ofdrw-reader/src/test/resources/发票示例.ofd");
        let path = CString::new(fixture.to_str().unwrap()).unwrap();
        let mut handle = ptr::null_mut();
        assert_eq!(unsafe { ofd_open(path.as_ptr(), &mut handle) }, 0);
        let mut output = OfdBuffer {
            data: ptr::null_mut(),
            len: 0,
        };
        assert_eq!(unsafe { ofd_page_sources_json(handle, 0, &mut output) }, 0);
        let values: serde_json::Value =
            serde_json::from_slice(unsafe { std::slice::from_raw_parts(output.data, output.len) })
                .unwrap();
        assert_eq!(values[0]["template_id"], "1");
        assert_eq!(values[1]["template_id"], serde_json::Value::Null);
        unsafe {
            ofd_buffer_free(output);
        }
        output = OfdBuffer {
            data: ptr::null_mut(),
            len: 0,
        };
        assert_eq!(unsafe { ofd_page_layers_json(handle, 0, &mut output) }, 0);
        let layers: serde_json::Value =
            serde_json::from_slice(unsafe { std::slice::from_raw_parts(output.data, output.len) })
                .unwrap();
        assert!(layers.as_array().unwrap().len() >= 2);
        assert!(layers[0]["layer_type"].is_string());
        unsafe {
            ofd_buffer_free(output);
        }
        output = OfdBuffer {
            data: ptr::null_mut(),
            len: 0,
        };
        assert_eq!(
            unsafe { ofd_page_text_with_templates_json(handle, 0, &mut output) },
            0
        );
        let values: serde_json::Value =
            serde_json::from_slice(unsafe { std::slice::from_raw_parts(output.data, output.len) })
                .unwrap();
        assert!(values.as_array().unwrap().len() > 24);
        unsafe {
            ofd_buffer_free(output);
            ofd_close(handle);
        }
    }

    #[test]
    fn ffi_promotes_and_applies_page_template() {
        let mut handle = ptr::null_mut();
        assert_eq!(unsafe { ofd_new(&mut handle) }, 0);
        let text = CString::new("template text").unwrap();
        assert_eq!(
            unsafe { ofd_add_text_page(handle, text.as_ptr(), 30.0, 20.0) },
            0
        );
        assert_eq!(unsafe { ofd_add_blank_page(handle, 30.0, 20.0) }, 0);
        let object_id = unsafe { document_ref(handle).unwrap() }
            .page_objects(0)
            .unwrap()[0]
            .id
            .clone()
            .unwrap();
        let object = CString::new(object_id.clone()).unwrap();
        let uri = CString::new("https://example.org/template").unwrap();
        assert_eq!(
            unsafe { ofd_add_page_object_uri_action(handle, 0, object.as_ptr(), uri.as_ptr()) },
            0
        );
        let targets = CString::new("[1]").unwrap();
        let background = CString::new("Background").unwrap();
        let mut template_id = 0;
        assert_eq!(
            unsafe {
                ofd_promote_page_to_template(
                    handle,
                    0,
                    targets.as_ptr(),
                    background.as_ptr(),
                    &mut template_id,
                )
            },
            0
        );
        assert_eq!(
            unsafe { document_ref(handle).unwrap() }
                .page_text_with_templates(1)
                .unwrap(),
            ["template text"]
        );
        let source_path = unsafe { document_ref(handle).unwrap() }
            .page_sources(1)
            .unwrap()[0]
            .path
            .clone();
        let source = CString::new(source_path).unwrap();
        let mut action_output = OfdBuffer {
            data: ptr::null_mut(),
            len: 0,
        };
        assert_eq!(
            unsafe {
                ofd_page_source_object_actions_json(
                    handle,
                    1,
                    source.as_ptr(),
                    object.as_ptr(),
                    &mut action_output,
                )
            },
            0
        );
        let actions: serde_json::Value = serde_json::from_slice(unsafe {
            std::slice::from_raw_parts(action_output.data, action_output.len)
        })
        .unwrap();
        assert_eq!(
            actions[0]["attributes"]["URI"],
            "https://example.org/template"
        );
        unsafe { ofd_buffer_free(action_output) };
        let bad_source = CString::new("OFD.xml").unwrap();
        let mut rejected_output = OfdBuffer {
            data: ptr::null_mut(),
            len: 0,
        };
        assert_ne!(
            unsafe {
                ofd_page_source_object_actions_json(
                    handle,
                    1,
                    bad_source.as_ptr(),
                    object.as_ptr(),
                    &mut rejected_output,
                )
            },
            0
        );
        assert_eq!(unsafe { ofd_add_blank_page(handle, 30.0, 20.0) }, 0);
        let id = CString::new(template_id.to_string()).unwrap();
        assert_eq!(
            unsafe { ofd_apply_page_template(handle, 2, id.as_ptr(), background.as_ptr()) },
            0
        );
        assert_eq!(
            unsafe { document_ref(handle).unwrap() }
                .page_text_with_templates(2)
                .unwrap(),
            ["template text"]
        );
        assert_ne!(
            unsafe { ofd_apply_page_template(handle, 2, id.as_ptr(), background.as_ptr()) },
            0
        );
        assert_ne!(
            unsafe {
                ofd_promote_page_to_template(
                    handle,
                    0,
                    targets.as_ptr(),
                    background.as_ptr(),
                    ptr::null_mut(),
                )
            },
            0
        );
        let malformed = CString::new("not json").unwrap();
        assert_ne!(
            unsafe {
                ofd_promote_page_to_template(
                    handle,
                    0,
                    malformed.as_ptr(),
                    background.as_ptr(),
                    &mut template_id,
                )
            },
            0
        );
        unsafe { ofd_close(handle) };
    }

    #[test]
    fn ffi_removes_template_reference_and_definition() {
        let mut handle = ptr::null_mut();
        assert_eq!(unsafe { ofd_new(&mut handle) }, 0);
        let text = CString::new("shared").unwrap();
        assert_eq!(
            unsafe { ofd_add_text_page(handle, text.as_ptr(), 30.0, 20.0) },
            0
        );
        assert_eq!(unsafe { ofd_add_blank_page(handle, 30.0, 20.0) }, 0);
        let targets = CString::new("[1]").unwrap();
        let background = CString::new("Background").unwrap();
        let mut id = 0;
        assert_eq!(
            unsafe {
                ofd_promote_page_to_template(
                    handle,
                    0,
                    targets.as_ptr(),
                    background.as_ptr(),
                    &mut id,
                )
            },
            0
        );
        let id = CString::new(id.to_string()).unwrap();
        assert_ne!(
            unsafe { ofd_remove_document_template(handle, 0, id.as_ptr()) },
            0
        );
        assert_eq!(
            unsafe { ofd_remove_page_template(handle, 1, id.as_ptr()) },
            0
        );
        assert_eq!(
            unsafe { ofd_remove_page_template(handle, 0, id.as_ptr()) },
            0
        );
        assert_eq!(
            unsafe { ofd_remove_document_template(handle, 0, id.as_ptr()) },
            0
        );
        assert_ne!(
            unsafe { ofd_remove_document_template(handle, 0, id.as_ptr()) },
            0
        );
        assert!(unsafe { document_ref(handle).unwrap() }
            .page_text_with_templates(0)
            .unwrap()
            .is_empty());
        unsafe { ofd_close(handle) };
    }

    #[test]
    fn ffi_reads_and_edits_document_metadata() {
        let fixture = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("../../../ofdrw-reader/src/test/resources/发票示例.ofd");
        let path = CString::new(fixture.to_str().unwrap()).unwrap();
        let mut handle = ptr::null_mut();
        assert_eq!(unsafe { ofd_open(path.as_ptr(), &mut handle) }, 0);
        let field = CString::new("Title").unwrap();
        let title = CString::new("Rust metadata").unwrap();
        assert_eq!(
            unsafe { ofd_set_document_info_field(handle, 0, field.as_ptr(), title.as_ptr()) },
            0
        );
        let mut output = OfdBuffer {
            data: ptr::null_mut(),
            len: 0,
        };
        assert_eq!(unsafe { ofd_documents_json(handle, &mut output) }, 0);
        let values: serde_json::Value =
            serde_json::from_slice(unsafe { std::slice::from_raw_parts(output.data, output.len) })
                .unwrap();
        assert_eq!(values[0]["title"], "Rust metadata");
        assert!(values[0]["custom_data"].as_array().unwrap().len() > 5);
        unsafe {
            ofd_buffer_free(output);
            ofd_close(handle);
        }
    }

    #[test]
    fn ffi_sets_and_removes_custom_document_metadata() {
        let mut handle = ptr::null_mut();
        assert_eq!(unsafe { ofd_new(&mut handle) }, 0);
        let name = CString::new("客户&类型").unwrap();
        let value = CString::new("<VIP>").unwrap();
        assert_eq!(
            unsafe { ofd_set_document_custom_data(handle, 0, name.as_ptr(), value.as_ptr()) },
            0
        );
        assert_eq!(
            unsafe { document_ref(handle).unwrap() }
                .document_info()
                .unwrap()[0]
                .custom_data[0],
            ("客户&类型".into(), "<VIP>".into())
        );
        assert_eq!(
            unsafe { ofd_remove_document_custom_data(handle, 0, name.as_ptr()) },
            0
        );
        assert!(unsafe { document_ref(handle).unwrap() }
            .document_info()
            .unwrap()[0]
            .custom_data
            .is_empty());
        assert_ne!(
            unsafe { ofd_remove_document_custom_data(handle, 0, name.as_ptr()) },
            0
        );
        unsafe { ofd_close(handle) };
    }

    #[test]
    fn ffi_edits_keywords_and_removes_standard_metadata() {
        let mut handle = ptr::null_mut();
        assert_eq!(unsafe { ofd_new(&mut handle) }, 0);
        let title = CString::new("Title").unwrap();
        let value = CString::new("Rust & OFD").unwrap();
        let keyword = CString::new("票据 & 发票").unwrap();
        let doc_id = CString::new("DocID").unwrap();
        let valid_id = CString::new("0123456789abcdef0123456789abcdef").unwrap();
        let invalid_id = CString::new("invalid").unwrap();
        assert_ne!(
            unsafe { ofd_set_document_info_field(handle, 0, doc_id.as_ptr(), invalid_id.as_ptr()) },
            0
        );
        assert_ne!(
            unsafe { ofd_remove_document_info_field(handle, 0, doc_id.as_ptr()) },
            0
        );
        assert_eq!(
            unsafe { ofd_set_document_info_field(handle, 0, doc_id.as_ptr(), valid_id.as_ptr()) },
            0
        );
        assert_eq!(
            unsafe { document_ref(handle).unwrap() }
                .document_info()
                .unwrap()[0]
                .doc_id,
            "0123456789abcdef0123456789abcdef"
        );
        assert_ne!(
            unsafe { ofd_remove_document_info_field(handle, 0, title.as_ptr()) },
            0
        );
        assert_eq!(
            unsafe { ofd_set_document_info_field(handle, 0, title.as_ptr(), value.as_ptr()) },
            0
        );
        assert_eq!(
            unsafe { ofd_add_document_keyword(handle, 0, keyword.as_ptr()) },
            0
        );
        assert_ne!(unsafe { ofd_remove_document_keyword(handle, 0, 1) }, 0);
        assert_eq!(
            unsafe { ofd_remove_document_info_field(handle, 0, title.as_ptr()) },
            0
        );
        assert_eq!(
            unsafe { document_ref(handle).unwrap() }
                .document_info()
                .unwrap()[0]
                .keywords,
            ["票据 & 发票"]
        );
        assert_eq!(unsafe { ofd_remove_document_keyword(handle, 0, 0) }, 0);
        assert!(unsafe { document_ref(handle).unwrap() }
            .document_info()
            .unwrap()[0]
            .keywords
            .is_empty());
        unsafe { ofd_close(handle) };
    }

    #[test]
    fn ffi_reads_invoice_annotations() {
        let fixture = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("../../../ofdrw-reader/src/test/resources/发票示例.ofd");
        let path = CString::new(fixture.to_str().unwrap()).unwrap();
        let mut handle = ptr::null_mut();
        assert_eq!(unsafe { ofd_open(path.as_ptr(), &mut handle) }, 0);
        let mut output = OfdBuffer {
            data: ptr::null_mut(),
            len: 0,
        };
        assert_eq!(unsafe { ofd_annotations_json(handle, &mut output) }, 0);
        let values: serde_json::Value =
            serde_json::from_slice(unsafe { std::slice::from_raw_parts(output.data, output.len) })
                .unwrap();
        assert_eq!(values.as_array().unwrap().len(), 2);
        assert_eq!(values[0]["type"], "Watermark");
        assert_eq!(values[1]["text"][0], "下载次数：1   ");
        unsafe {
            ofd_buffer_free(output);
            ofd_close(handle);
        }
    }

    #[test]
    fn ffi_edits_font_attribute_and_clears_optional_value() {
        let mut handle = ptr::null_mut();
        assert_eq!(unsafe { ofd_new(&mut handle) }, 0);
        let family = CString::new("FamilyName").unwrap();
        let value = CString::new("Family & More").unwrap();
        assert_eq!(
            unsafe { ofd_set_font_attribute(handle, 0, family.as_ptr(), value.as_ptr()) },
            0
        );
        assert_eq!(
            unsafe { document_ref(handle).unwrap() }.fonts().unwrap()[0]
                .family_name
                .as_deref(),
            Some("Family & More")
        );
        let charset = CString::new("Charset").unwrap();
        let invalid = CString::new("invalid").unwrap();
        let before = unsafe { document_ref(handle).unwrap() }.to_bytes().unwrap();
        assert_ne!(
            unsafe { ofd_set_font_attribute(handle, 0, charset.as_ptr(), invalid.as_ptr()) },
            0
        );
        assert_eq!(
            unsafe { document_ref(handle).unwrap() }.to_bytes().unwrap(),
            before
        );
        assert_eq!(
            unsafe { ofd_set_font_attribute(handle, 0, family.as_ptr(), ptr::null()) },
            0
        );
        assert_eq!(
            unsafe { document_ref(handle).unwrap() }.fonts().unwrap()[0].family_name,
            None
        );
        unsafe { ofd_close(handle) };
    }

    #[test]
    fn ffi_edits_page_local_font_without_touching_public_font() {
        let mut handle = ptr::null_mut();
        assert_eq!(unsafe { ofd_new(&mut handle) }, 0);
        assert_eq!(unsafe { ofd_add_blank_page(handle, 30.0, 20.0) }, 0);
        let page_path = unsafe { document_ref(handle).unwrap() }.pages().unwrap()[0]
            .path
            .clone();
        let directory = page_path.rsplit_once('/').unwrap().0;
        let resource = b"<ofd:Res xmlns:ofd=\"http://www.ofdspec.org/2016\"><ofd:Fonts><ofd:Font ID=\"1\" FontName=\"Local\"/></ofd:Fonts></ofd:Res>";
        unsafe { document_mut(handle).unwrap() }
            .set_entry(&format!("{directory}/PageRes.xml"), resource.to_vec())
            .unwrap();
        let page = unsafe { document_ref(handle).unwrap() }
            .entry(&page_path)
            .unwrap()
            .unwrap();
        let updated = String::from_utf8(page.to_vec()).unwrap().replacen(
            "<ofd:Content>",
            "<ofd:PageRes>PageRes.xml</ofd:PageRes><ofd:Content>",
            1,
        );
        unsafe { document_mut(handle).unwrap() }
            .set_entry(&page_path, updated.into_bytes())
            .unwrap();
        let name = CString::new("FontName").unwrap();
        let value = CString::new("Changed").unwrap();
        assert_eq!(
            unsafe { ofd_set_page_font_attribute(handle, 0, 0, name.as_ptr(), value.as_ptr()) },
            0
        );
        assert_eq!(
            unsafe { document_ref(handle).unwrap() }
                .page_fonts(0)
                .unwrap()[0]
                .font_name,
            "Changed"
        );
        assert_eq!(
            unsafe { document_ref(handle).unwrap() }.fonts().unwrap()[0].font_name,
            "SimSun"
        );
        assert_ne!(
            unsafe { ofd_set_page_font_attribute(handle, 9, 0, name.as_ptr(), value.as_ptr()) },
            0
        );
        unsafe { ofd_close(handle) };
    }

    #[test]
    fn ffi_replaces_embedded_font_data_atomically() {
        let sans = include_bytes!("../../ofdrw/tests/fixtures/dejavu-sans.ttf");
        let serif = include_bytes!("../../ofdrw/tests/fixtures/dejavu-serif.ttf");
        let mut handle = ptr::null_mut();
        assert_eq!(unsafe { ofd_new(&mut handle) }, 0);
        let name = CString::new("Embedded").unwrap();
        let mut id = 0;
        assert_eq!(
            unsafe {
                ofd_add_embedded_font(handle, name.as_ptr(), sans.as_ptr(), sans.len(), &mut id)
            },
            0
        );
        assert_eq!(unsafe { ofd_add_blank_page(handle, 30.0, 20.0) }, 0);
        let index = unsafe { document_ref(handle).unwrap() }
            .fonts()
            .unwrap()
            .iter()
            .position(|font| font.id == id.to_string())
            .unwrap();
        assert_eq!(
            unsafe { ofd_replace_font_data(handle, index, serif.as_ptr(), serif.len()) },
            0
        );
        assert_eq!(
            unsafe { document_ref(handle).unwrap() }
                .font_data(index)
                .unwrap(),
            serif
        );
        let before = unsafe { document_ref(handle).unwrap() }.to_bytes().unwrap();
        assert_ne!(
            unsafe { ofd_replace_font_data(handle, index, ptr::null(), 1) },
            0
        );
        assert_ne!(
            unsafe { ofd_replace_font_data(handle, index, b"bad".as_ptr(), 3) },
            0
        );
        assert_eq!(
            unsafe { document_ref(handle).unwrap() }.to_bytes().unwrap(),
            before
        );
        assert_eq!(
            unsafe { ofd_replace_page_font_data(handle, 0, index, sans.as_ptr(), sans.len()) },
            0
        );
        assert_eq!(
            unsafe { document_ref(handle).unwrap() }
                .font_data(index)
                .unwrap(),
            sans
        );
        unsafe { ofd_close(handle) };
    }

    #[test]
    fn ffi_embeds_font_in_one_page() {
        let font = include_bytes!("../../ofdrw/tests/fixtures/dejavu-sans.ttf");
        let mut handle = ptr::null_mut();
        assert_eq!(unsafe { ofd_new(&mut handle) }, 0);
        assert_eq!(unsafe { ofd_add_blank_page(handle, 40.0, 30.0) }, 0);
        assert_eq!(unsafe { ofd_add_blank_page(handle, 40.0, 30.0) }, 0);
        let name = CString::new("Page Font").unwrap();
        let mut id = 0;
        assert_eq!(
            unsafe {
                ofd_add_embedded_font_to_page(
                    handle,
                    0,
                    name.as_ptr(),
                    font.as_ptr(),
                    font.len(),
                    &mut id,
                )
            },
            0
        );
        assert_eq!(
            unsafe { document_ref(handle).unwrap() }
                .page_fonts(0)
                .unwrap()[0]
                .id,
            id.to_string()
        );
        assert_eq!(
            unsafe { document_ref(handle).unwrap() }
                .page_font_data(0, 0)
                .unwrap(),
            font
        );
        assert!(unsafe { document_ref(handle).unwrap() }
            .page_fonts(1)
            .unwrap()
            .iter()
            .all(|item| item.id != id.to_string()));
        let before = unsafe { document_ref(handle).unwrap() }.to_bytes().unwrap();
        assert_ne!(
            unsafe {
                ofd_add_embedded_font_to_page(handle, 0, name.as_ptr(), b"bad".as_ptr(), 3, &mut id)
            },
            0
        );
        assert_eq!(
            unsafe { document_ref(handle).unwrap() }.to_bytes().unwrap(),
            before
        );
        unsafe { ofd_close(handle) };
    }

    #[test]
    fn ffi_reads_embedded_font() {
        let fixture = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("../../../ofdrw-reader/src/test/resources/keyword.ofd");
        let path = CString::new(fixture.to_str().unwrap()).unwrap();
        let mut handle = ptr::null_mut();
        assert_eq!(unsafe { ofd_open(path.as_ptr(), &mut handle) }, 0);
        let mut output = OfdBuffer {
            data: ptr::null_mut(),
            len: 0,
        };
        assert_eq!(unsafe { ofd_fonts_json(handle, &mut output) }, 0);
        let values: serde_json::Value =
            serde_json::from_slice(unsafe { std::slice::from_raw_parts(output.data, output.len) })
                .unwrap();
        assert_eq!(values[0]["embedded_path"], "Doc_0/Res/Font7.ttf");
        unsafe {
            ofd_buffer_free(output);
        }
        output = OfdBuffer {
            data: ptr::null_mut(),
            len: 0,
        };
        assert_eq!(unsafe { ofd_font_data(handle, 0, &mut output) }, 0);
        assert_eq!(output.len, 244_104);
        unsafe {
            ofd_buffer_free(output);
        }
        output = OfdBuffer {
            data: ptr::null_mut(),
            len: 0,
        };
        assert_eq!(unsafe { ofd_page_fonts_json(handle, 0, &mut output) }, 0);
        let page_fonts: serde_json::Value =
            serde_json::from_slice(unsafe { std::slice::from_raw_parts(output.data, output.len) })
                .unwrap();
        assert_eq!(page_fonts[0]["embedded_path"], "Doc_0/Res/Font7.ttf");
        unsafe { ofd_buffer_free(output) };
        output = OfdBuffer {
            data: ptr::null_mut(),
            len: 0,
        };
        assert_eq!(unsafe { ofd_page_font_data(handle, 0, 0, &mut output) }, 0);
        assert_eq!(output.len, 244_104);
        unsafe {
            ofd_buffer_free(output);
            ofd_close(handle);
        }
    }

    #[test]
    fn ffi_writes_embedded_font_and_references_it() {
        let fixture = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("../../../ofdrw-reader/src/test/resources/keyword.ofd");
        let source = OfdDocument::open(fixture).unwrap();
        let data = source.font_data(0).unwrap();
        let mut handle = ptr::null_mut();
        assert_eq!(unsafe { ofd_new(&mut handle) }, 0);
        let name = CString::new("宋体").unwrap();
        let mut id = 0;
        assert_eq!(
            unsafe {
                ofd_add_embedded_font(handle, name.as_ptr(), data.as_ptr(), data.len(), &mut id)
            },
            0
        );
        let text = CString::new("字体测试").unwrap();
        assert_eq!(
            unsafe { ofd_add_text_page_with_font(handle, text.as_ptr(), 210.0, 297.0, id) },
            0
        );
        let mut output = OfdBuffer {
            data: ptr::null_mut(),
            len: 0,
        };
        assert_eq!(
            unsafe { ofd_page_text_objects_json(handle, 0, &mut output) },
            0
        );
        let values: serde_json::Value =
            serde_json::from_slice(unsafe { std::slice::from_raw_parts(output.data, output.len) })
                .unwrap();
        assert_eq!(values[0]["font_id"], id.to_string());
        unsafe {
            ofd_buffer_free(output);
            ofd_close(handle);
        }
    }

    #[test]
    fn ffi_embeds_font_in_existing_document() {
        let base = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("../../../ofdrw-reader/src/test/resources");
        let font = OfdDocument::open(base.join("keyword.ofd"))
            .unwrap()
            .font_data(0)
            .unwrap()
            .to_vec();
        let path = CString::new(base.join("helloworld.ofd").to_str().unwrap()).unwrap();
        let mut handle = ptr::null_mut();
        assert_eq!(unsafe { ofd_open(path.as_ptr(), &mut handle) }, 0);
        let name = CString::new("Rust 字体").unwrap();
        let mut id = 0;
        assert_ne!(
            unsafe {
                ofd_add_embedded_font_to_document(
                    handle,
                    1,
                    name.as_ptr(),
                    font.as_ptr(),
                    font.len(),
                    &mut id,
                )
            },
            0
        );
        assert_eq!(
            unsafe {
                ofd_add_embedded_font_to_document(
                    handle,
                    0,
                    name.as_ptr(),
                    font.as_ptr(),
                    font.len(),
                    &mut id,
                )
            },
            0
        );
        assert!(id > 0);
        let text = CString::new("selected font page").unwrap();
        assert_ne!(
            unsafe { ofd_append_text_page_with_font(handle, 0, text.as_ptr(), 80.0, 60.0, 999) },
            0
        );
        assert_eq!(
            unsafe { ofd_append_text_page_with_font(handle, 0, text.as_ptr(), 80.0, 60.0, id) },
            0
        );
        let document = unsafe { document_ref(handle).unwrap() };
        assert_eq!(document.fonts().unwrap().last().unwrap().id, id.to_string());
        assert_eq!(document.page_text(1).unwrap(), vec!["selected font page"]);
        assert!(document.validate_references().unwrap().is_empty());
        unsafe { ofd_close(handle) };
    }

    #[test]
    fn ffi_reads_signature_description_and_data() {
        let fixture = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("../../../ofdrw-reader/src/test/resources/SESV4SignDoc.ofd");
        let path = CString::new(fixture.to_str().unwrap()).unwrap();
        let mut handle = ptr::null_mut();
        assert_eq!(unsafe { ofd_open(path.as_ptr(), &mut handle) }, 0);
        let mut output = OfdBuffer {
            data: ptr::null_mut(),
            len: 0,
        };
        assert_eq!(unsafe { ofd_signatures_json(handle, &mut output) }, 0);
        let values: serde_json::Value =
            serde_json::from_slice(unsafe { std::slice::from_raw_parts(output.data, output.len) })
                .unwrap();
        assert_eq!(values[0]["check_method"], "SM3");
        assert_eq!(values[0]["references"].as_array().unwrap().len(), 5);
        unsafe { ofd_buffer_free(output) };
        output = OfdBuffer {
            data: ptr::null_mut(),
            len: 0,
        };
        assert_eq!(
            unsafe { ofd_check_signature_description_hash_json(handle, 0, &mut output) },
            0
        );
        let check: serde_json::Value =
            serde_json::from_slice(unsafe { std::slice::from_raw_parts(output.data, output.len) })
                .unwrap();
        assert_eq!(check["valid"], true);
        assert_eq!(check["expected"], check["actual"]);
        unsafe { ofd_buffer_free(output) };
        let mut seal_match = -1;
        assert_eq!(
            unsafe { ofd_check_signature_seal_match(handle, 0, &mut seal_match) },
            0
        );
        assert_eq!(seal_match, 1);
        assert_ne!(
            unsafe { ofd_check_signature_seal_match(handle, 0, ptr::null_mut()) },
            0
        );
        let mut seal_signature_valid = 0;
        assert_eq!(
            unsafe { ofd_check_ses_seal_signature(handle, 0, &mut seal_signature_valid) },
            0
        );
        assert_eq!(seal_signature_valid, 1);
        let mut seal_certificate_valid = 0;
        assert_eq!(
            unsafe {
                ofd_check_ses_seal_certificate_validity(
                    handle,
                    0,
                    1_600_000_000,
                    &mut seal_certificate_valid,
                )
            },
            0
        );
        assert_eq!(seal_certificate_valid, 1);
        assert_eq!(
            unsafe {
                ofd_check_ses_seal_certificate_validity(handle, 0, 0, &mut seal_certificate_valid)
            },
            0
        );
        assert_eq!(seal_certificate_valid, 0);
        let mut signer_authorized = 0;
        assert_eq!(
            unsafe { ofd_check_ses_seal_signer_authorization(handle, 0, &mut signer_authorized) },
            0
        );
        assert_eq!(signer_authorized, 1);
        let mut seal_valid = 0;
        assert_eq!(
            unsafe { ofd_check_ses_seal_validity(handle, 0, 1_600_000_000, &mut seal_valid) },
            0
        );
        assert_eq!(seal_valid, 1);
        assert_eq!(
            unsafe { ofd_check_ses_seal_validity(handle, 0, 0, &mut seal_valid) },
            0
        );
        assert_eq!(seal_valid, 0);
        let anchor = include_bytes!("../../ofdrw/tests/fixtures/root-cert.der");
        let mut seal_chain_valid = -1;
        assert_eq!(
            unsafe {
                ofd_check_ses_seal_issuer_ordered_sm2_chain(
                    handle,
                    0,
                    b"[]".as_ptr(),
                    2,
                    anchor.as_ptr(),
                    anchor.len(),
                    1_600_000_000,
                    c"".as_ptr(),
                    &mut seal_chain_valid,
                )
            },
            0
        );
        assert_eq!(seal_chain_valid, 0);
        assert_ne!(
            unsafe {
                ofd_check_ses_seal_issuer_ordered_sm2_chain(
                    handle,
                    0,
                    b"bad".as_ptr(),
                    3,
                    anchor.as_ptr(),
                    anchor.len(),
                    1_600_000_000,
                    c"".as_ptr(),
                    &mut seal_chain_valid,
                )
            },
            0
        );
        assert_eq!(
            unsafe {
                ofd_check_ses_seal_signature_with_distid(
                    handle,
                    0,
                    c"different-id".as_ptr(),
                    &mut seal_signature_valid,
                )
            },
            0
        );
        assert_eq!(seal_signature_valid, 0);
        let mut signature_valid = 0;
        assert_eq!(
            unsafe { ofd_check_ses_signature_value(handle, 0, &mut signature_valid) },
            0
        );
        assert_eq!(signature_valid, 1);
        assert_ne!(
            unsafe { ofd_check_ses_signature_value(handle, 1, &mut signature_valid) },
            0
        );
        output = OfdBuffer {
            data: ptr::null_mut(),
            len: 0,
        };
        assert_eq!(
            unsafe { ofd_check_signature_references_json(handle, 0, &mut output) },
            0
        );
        let checks: serde_json::Value =
            serde_json::from_slice(unsafe { std::slice::from_raw_parts(output.data, output.len) })
                .unwrap();
        assert_eq!(checks.as_array().unwrap().len(), 5);
        assert!(checks
            .as_array()
            .unwrap()
            .iter()
            .all(|item| item["valid"] == true));
        unsafe { ofd_buffer_free(output) };
        output = OfdBuffer {
            data: ptr::null_mut(),
            len: 0,
        };
        assert_eq!(unsafe { ofd_signed_value_data(handle, 0, &mut output) }, 0);
        assert_eq!(output.len, 17_726);
        unsafe { ofd_buffer_free(output) };
        output = OfdBuffer {
            data: ptr::null_mut(),
            len: 0,
        };
        assert_eq!(unsafe { ofd_seal_data(handle, 0, &mut output) }, 0);
        assert_eq!(output.len, 17_011);
        unsafe { ofd_buffer_free(output) };
        output = OfdBuffer {
            data: ptr::null_mut(),
            len: 0,
        };
        assert_ne!(unsafe { ofd_signed_value_data(handle, 1, &mut output) }, 0);
        unsafe { ofd_close(handle) };
    }

    #[test]
    fn ffi_checks_raw_digital_signature_with_certificate() {
        let fixture = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("../../../ofdrw-sign/src/test/resources/DigitalSign.ofd");
        let path = CString::new(fixture.to_str().unwrap()).unwrap();
        let mut handle = ptr::null_mut();
        assert_eq!(unsafe { ofd_open(path.as_ptr(), &mut handle) }, 0);
        let certificate = include_bytes!("../../ofdrw/tests/fixtures/user-cert.der");
        let mut valid = 0;
        assert_eq!(
            unsafe {
                ofd_check_digital_signature_with_certificate(
                    handle,
                    0,
                    certificate.as_ptr(),
                    certificate.len(),
                    &mut valid,
                )
            },
            0
        );
        assert_eq!(valid, 1);
        assert_ne!(
            unsafe {
                ofd_check_digital_signature_with_certificate(handle, 0, ptr::null(), 0, &mut valid)
            },
            0
        );
        unsafe { ofd_close(handle) };
    }

    #[test]
    fn ffi_checks_signer_certificate_validity() {
        let fixture = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("../../../ofdrw-sign/src/test/resources/DigitalSign.ofd");
        let path = CString::new(fixture.to_str().unwrap()).unwrap();
        let mut handle = ptr::null_mut();
        assert_eq!(unsafe { ofd_open(path.as_ptr(), &mut handle) }, 0);
        let certificate = include_bytes!("../../ofdrw/tests/fixtures/user-cert.der");
        let mut valid = 0;
        assert_eq!(
            unsafe {
                ofd_check_signature_certificate_validity(
                    handle,
                    0,
                    certificate.as_ptr(),
                    certificate.len(),
                    1_600_000_000,
                    &mut valid,
                )
            },
            0
        );
        assert_eq!(valid, 1);
        assert_eq!(
            unsafe {
                ofd_check_signature_certificate_validity(
                    handle,
                    0,
                    certificate.as_ptr(),
                    certificate.len(),
                    1_700_000_000,
                    &mut valid,
                )
            },
            0
        );
        assert_eq!(valid, 0);
        assert_ne!(
            unsafe {
                ofd_check_signature_certificate_validity(
                    handle,
                    0,
                    ptr::null(),
                    0,
                    1_600_000_000,
                    &mut valid,
                )
            },
            0
        );
        unsafe { ofd_close(handle) };
    }

    #[test]
    fn ffi_creates_raw_digital_signature() {
        let mut handle = ptr::null_mut();
        assert_eq!(unsafe { ofd_new(&mut handle) }, 0);
        let key = include_bytes!("../../ofdrw/tests/fixtures/user-private-key.bin");
        let certificate = include_bytes!("../../ofdrw/tests/fixtures/user-cert.der");
        let date = CString::new("2020-04-22 16:22:44").unwrap();
        let mut index = usize::MAX;
        assert_eq!(
            unsafe {
                ofd_add_digital_signature(
                    handle,
                    key.as_ptr(),
                    key.len(),
                    date.as_ptr(),
                    &mut index,
                )
            },
            0
        );
        assert_eq!(index, 0);
        let mut valid = 0;
        assert_eq!(
            unsafe {
                ofd_check_digital_signature_with_certificate(
                    handle,
                    index,
                    certificate.as_ptr(),
                    certificate.len(),
                    &mut valid,
                )
            },
            0
        );
        assert_eq!(valid, 1);
        assert_eq!(
            unsafe {
                ofd_add_digital_signature(
                    handle,
                    key.as_ptr(),
                    key.len(),
                    date.as_ptr(),
                    &mut index,
                )
            },
            0
        );
        assert_eq!(index, 1);
        assert_eq!(
            unsafe {
                ofd_check_digital_signature_with_certificate(
                    handle,
                    index,
                    certificate.as_ptr(),
                    certificate.len(),
                    &mut valid,
                )
            },
            0
        );
        assert_eq!(valid, 1);
        assert_ne!(
            unsafe { ofd_add_digital_signature(handle, ptr::null(), 0, date.as_ptr(), &mut index) },
            0
        );
        unsafe { ofd_close(handle) };
    }

    #[test]
    fn ffi_creates_positioned_ses_signature_through_json_options() {
        let mut handle = ptr::null_mut();
        assert_eq!(unsafe { ofd_new(&mut handle) }, 0);
        assert_eq!(
            unsafe { ofd_add_text_page(handle, c"stamp".as_ptr(), 210.0, 297.0) },
            0
        );
        let key = include_bytes!("../../ofdrw/tests/fixtures/user-private-key.bin");
        let certificate = include_bytes!("../../ofdrw/tests/fixtures/user-cert.der");
        let seal = include_bytes!("../../../../ofdrw-gm/src/test/resources/UserV4.esl");
        let options = CString::new(
            serde_json::json!({
                "signature_datetime": "2020-09-13 12:26:40",
                "page_index": 0,
                "boundary": [10.0, 20.0, 40.0, 40.0],
                "signature_distid": "1234567812345678",
                "seal_distid": "1234567812345678",
            })
            .to_string(),
        )
        .unwrap();
        let mut index = usize::MAX;
        for invalid in [ptr::null(), c"{}".as_ptr(), c"not json".as_ptr()] {
            assert_ne!(
                unsafe {
                    ofd_add_ses_signature_with_stamp_json(
                        handle,
                        key.as_ptr(),
                        key.len(),
                        certificate.as_ptr(),
                        certificate.len(),
                        seal.as_ptr(),
                        seal.len(),
                        invalid,
                        &mut index,
                    )
                },
                0
            );
            assert_eq!(index, usize::MAX);
        }
        assert_ne!(
            unsafe {
                ofd_add_ses_signature_with_stamp_json(
                    handle,
                    key.as_ptr(),
                    key.len(),
                    certificate.as_ptr(),
                    certificate.len(),
                    seal.as_ptr(),
                    seal.len(),
                    options.as_ptr(),
                    ptr::null_mut(),
                )
            },
            0
        );
        assert_eq!(
            unsafe {
                ofd_add_ses_signature_with_stamp_json(
                    handle,
                    key.as_ptr(),
                    key.len(),
                    certificate.as_ptr(),
                    certificate.len(),
                    seal.as_ptr(),
                    seal.len(),
                    options.as_ptr(),
                    &mut index,
                )
            },
            0
        );
        assert_eq!(index, 0);
        let mut valid = 0;
        assert_eq!(
            unsafe { ofd_check_ses_signature_value(handle, index, &mut valid) },
            0
        );
        assert_eq!(valid, 1);
        assert_eq!(
            unsafe { ofd_check_signature_seal_match(handle, index, &mut valid) },
            0
        );
        assert_eq!(valid, 1);
        unsafe { ofd_close(handle) };
    }

    #[test]
    fn ffi_creates_ses_signature_with_embedded_and_detached_seal() {
        let mut handle = ptr::null_mut();
        assert_eq!(unsafe { ofd_new(&mut handle) }, 0);
        assert_eq!(
            unsafe { ofd_add_text_page(handle, c"stamped".as_ptr(), 210.0, 297.0) },
            0
        );
        let key = include_bytes!("../../ofdrw/tests/fixtures/user-private-key.bin");
        let certificate = include_bytes!("../../ofdrw/tests/fixtures/user-cert.der");
        let seal = include_bytes!("../../../../ofdrw-gm/src/test/resources/UserV4.esl");
        let date = c"2020-09-13 12:26:40";
        let mut index = usize::MAX;
        assert_eq!(
            unsafe {
                ofd_add_ses_signature_with_stamp_and_distids(
                    handle,
                    key.as_ptr(),
                    key.len(),
                    certificate.as_ptr(),
                    certificate.len(),
                    seal.as_ptr(),
                    seal.len(),
                    date.as_ptr(),
                    0,
                    10.0,
                    20.0,
                    40.0,
                    40.0,
                    c"1234567812345678".as_ptr(),
                    c"1234567812345678".as_ptr(),
                    &mut index,
                )
            },
            0
        );
        assert_eq!(index, 0);
        let mut valid = 0;
        assert_eq!(
            unsafe { ofd_check_ses_signature_value(handle, index, &mut valid) },
            0
        );
        assert_eq!(valid, 1);
        assert_eq!(
            unsafe { ofd_check_signature_seal_match(handle, index, &mut valid) },
            0
        );
        assert_eq!(valid, 1);
        assert_ne!(
            unsafe {
                ofd_add_ses_signature_with_distids(
                    handle,
                    key.as_ptr(),
                    key.len(),
                    certificate.as_ptr(),
                    certificate.len(),
                    seal.as_ptr(),
                    seal.len(),
                    date.as_ptr(),
                    c"1234567812345678".as_ptr(),
                    c"bad-seal-id".as_ptr(),
                    &mut index,
                )
            },
            0
        );
        let picture = include_bytes!("../../../../ofdrw-layout/src/test/resources/testimg.png");
        let spec = serde_json::json!({
            "version": 4,
            "vendor_id": "ofdrw-rs",
            "seal_id": "ffi-seal",
            "name": "FFI seal",
            "seal_type": 3,
            "authorized_signer_certificates_base64": [STANDARD.encode(certificate)],
            "created_at": 1_600_000_000_u64,
            "valid_from": 1_600_000_000_u64,
            "valid_until": 1_900_000_000_u64,
            "picture_type": "PNG",
            "picture_width_mm": 40,
            "picture_height_mm": 40,
        })
        .to_string();
        let mut created = OfdBuffer {
            data: ptr::null_mut(),
            len: 0,
        };
        assert_eq!(
            unsafe {
                ofd_create_ses_seal_json(
                    key.as_ptr(),
                    key.len(),
                    certificate.as_ptr(),
                    certificate.len(),
                    picture.as_ptr(),
                    picture.len(),
                    spec.as_ptr(),
                    spec.len(),
                    c"1234567812345678".as_ptr(),
                    &mut created,
                )
            },
            0
        );
        let generated = unsafe { std::slice::from_raw_parts(created.data, created.len) };
        assert_eq!(generated[0], 0x30);
        assert_eq!(
            unsafe {
                ofd_add_ses_signature(
                    handle,
                    key.as_ptr(),
                    key.len(),
                    certificate.as_ptr(),
                    certificate.len(),
                    generated.as_ptr(),
                    generated.len(),
                    date.as_ptr(),
                    &mut index,
                )
            },
            0
        );
        assert_eq!(index, 1);
        assert_eq!(
            unsafe { ofd_check_ses_seal_signature(handle, index, &mut valid) },
            0
        );
        assert_eq!(valid, 1);
        unsafe { ofd_buffer_free(created) };
        unsafe { ofd_close(handle) };
    }

    #[test]
    fn ffi_checks_direct_sm2_trust_anchor() {
        let mut handle = ptr::null_mut();
        assert_eq!(unsafe { ofd_new(&mut handle) }, 0);
        let key = include_bytes!("../../ofdrw/tests/fixtures/trust-leaf-private-key.bin");
        let signer = include_bytes!("../../ofdrw/tests/fixtures/trust-leaf-cert.der");
        let anchor = include_bytes!("../../ofdrw/tests/fixtures/trust-root-cert.der");
        let date = CString::new("2030-01-01 00:00:00").unwrap();
        let empty_id = CString::new("").unwrap();
        let default_id = CString::new("1234567812345678").unwrap();
        let mut index = usize::MAX;
        assert_eq!(
            unsafe {
                ofd_add_digital_signature(
                    handle,
                    key.as_ptr(),
                    key.len(),
                    date.as_ptr(),
                    &mut index,
                )
            },
            0
        );
        let mut valid = 0;
        assert_eq!(
            unsafe {
                ofd_check_signature_direct_trust_anchor(
                    handle,
                    0,
                    signer.as_ptr(),
                    signer.len(),
                    anchor.as_ptr(),
                    anchor.len(),
                    1_893_456_000,
                    empty_id.as_ptr(),
                    &mut valid,
                )
            },
            0
        );
        assert_eq!(valid, 1);
        assert_eq!(
            unsafe {
                ofd_check_signature_direct_trust_anchor(
                    handle,
                    0,
                    signer.as_ptr(),
                    signer.len(),
                    anchor.as_ptr(),
                    anchor.len(),
                    1_893_456_000,
                    default_id.as_ptr(),
                    &mut valid,
                )
            },
            0
        );
        assert_eq!(valid, 0);
        assert_ne!(
            unsafe {
                ofd_check_signature_direct_trust_anchor(
                    handle,
                    0,
                    signer.as_ptr(),
                    signer.len(),
                    ptr::null(),
                    0,
                    1_893_456_000,
                    empty_id.as_ptr(),
                    &mut valid,
                )
            },
            0
        );
        unsafe { ofd_close(handle) };
    }

    #[test]
    fn ffi_reports_complete_signature_verification() {
        let mut handle = ptr::null_mut();
        assert_eq!(unsafe { ofd_new(&mut handle) }, 0);
        let key = include_bytes!("../../ofdrw/tests/fixtures/trust-leaf-private-key.bin");
        let signer = include_bytes!("../../ofdrw/tests/fixtures/trust-leaf-cert.der");
        let anchor = include_bytes!("../../ofdrw/tests/fixtures/trust-root-cert.der");
        let date = CString::new("2030-01-01 00:00:00").unwrap();
        let empty_id = CString::new("").unwrap();
        let mut index = usize::MAX;
        assert_eq!(
            unsafe {
                ofd_add_digital_signature(
                    handle,
                    key.as_ptr(),
                    key.len(),
                    date.as_ptr(),
                    &mut index,
                )
            },
            0
        );
        let mut output = OfdBuffer {
            data: ptr::null_mut(),
            len: 0,
        };
        assert_eq!(
            unsafe {
                ofd_verify_signature_with_direct_trust_anchor_json(
                    handle,
                    index,
                    signer.as_ptr(),
                    signer.len(),
                    anchor.as_ptr(),
                    anchor.len(),
                    1_893_456_000,
                    empty_id.as_ptr(),
                    &mut output,
                )
            },
            0
        );
        let value: serde_json::Value =
            serde_json::from_slice(unsafe { std::slice::from_raw_parts(output.data, output.len) })
                .unwrap();
        assert_eq!(value["all_valid"], true);
        assert_eq!(value["references_valid"], true);
        assert_eq!(value["description_hash"], serde_json::Value::Null);
        unsafe { ofd_buffer_free(output) };
        let mut output = OfdBuffer {
            data: ptr::null_mut(),
            len: 0,
        };
        assert_ne!(
            unsafe {
                ofd_verify_signature_with_direct_trust_anchor_json(
                    handle,
                    index,
                    signer.as_ptr(),
                    signer.len(),
                    ptr::null(),
                    0,
                    1_893_456_000,
                    empty_id.as_ptr(),
                    &mut output,
                )
            },
            0
        );
        unsafe { ofd_close(handle) };
    }

    #[test]
    fn ffi_reports_ordered_sm2_chain_verification() {
        let mut handle = ptr::null_mut();
        assert_eq!(unsafe { ofd_new(&mut handle) }, 0);
        let key = include_bytes!("../../ofdrw/tests/fixtures/chain-leaf-private-key.bin");
        let signer = include_bytes!("../../ofdrw/tests/fixtures/chain-leaf-cert.der");
        let intermediate = include_bytes!("../../ofdrw/tests/fixtures/chain-intermediate-cert.der");
        let anchor = include_bytes!("../../ofdrw/tests/fixtures/chain-root-cert.der");
        let date = CString::new("2030-01-01 00:00:00").unwrap();
        let empty_id = CString::new("").unwrap();
        let mut index = usize::MAX;
        assert_eq!(
            unsafe {
                ofd_add_digital_signature(
                    handle,
                    key.as_ptr(),
                    key.len(),
                    date.as_ptr(),
                    &mut index,
                )
            },
            0
        );
        let certificates = serde_json::to_vec(&vec![STANDARD.encode(intermediate)]).unwrap();
        let mut output = OfdBuffer {
            data: ptr::null_mut(),
            len: 0,
        };
        assert_eq!(
            unsafe {
                ofd_verify_signature_with_ordered_sm2_chain_json(
                    handle,
                    index,
                    signer.as_ptr(),
                    signer.len(),
                    certificates.as_ptr(),
                    certificates.len(),
                    anchor.as_ptr(),
                    anchor.len(),
                    1_893_456_000,
                    empty_id.as_ptr(),
                    &mut output,
                )
            },
            0
        );
        let value: serde_json::Value =
            serde_json::from_slice(unsafe { std::slice::from_raw_parts(output.data, output.len) })
                .unwrap();
        assert_eq!(value["all_valid"], true);
        assert_eq!(value["certificate_chain_valid"], true);
        unsafe { ofd_buffer_free(output) };
        let unrelated = include_bytes!("../../ofdrw/tests/fixtures/root-cert.der");
        let candidates = serde_json::to_vec(&vec![
            STANDARD.encode(unrelated),
            STANDARD.encode(intermediate),
        ])
        .unwrap();
        let default_id = CString::new("1234567812345678").unwrap();
        let mut output = OfdBuffer {
            data: ptr::null_mut(),
            len: 0,
        };
        assert_eq!(
            unsafe {
                ofd_verify_signature_with_certificate_pool_json(
                    handle,
                    index,
                    signer.as_ptr(),
                    signer.len(),
                    candidates.as_ptr(),
                    candidates.len(),
                    anchor.as_ptr(),
                    anchor.len(),
                    1_893_456_000,
                    empty_id.as_ptr(),
                    default_id.as_ptr(),
                    &mut output,
                )
            },
            0
        );
        let value: serde_json::Value =
            serde_json::from_slice(unsafe { std::slice::from_raw_parts(output.data, output.len) })
                .unwrap();
        assert_eq!(value["all_valid"], true);
        unsafe { ofd_buffer_free(output) };
        let options = serde_json::to_vec(&serde_json::json!({
            "signer_certificate_der_base64": STANDARD.encode(signer),
            "signer_intermediates_der_base64": [STANDARD.encode(intermediate)],
            "signer_anchor_der_base64": STANDARD.encode(anchor),
            "seal_intermediates_der_base64": [],
            "seal_anchor_der_base64": "",
            "at_unix_seconds": 1_893_456_000_u64,
            "signer_certificate_distid": "",
            "signer_signature_distid": "1234567812345678",
            "seal_certificate_distid": "",
            "seal_signature_distid": "1234567812345678",
        }))
        .unwrap();
        let mut output = OfdBuffer {
            data: ptr::null_mut(),
            len: 0,
        };
        assert_eq!(
            unsafe {
                ofd_verify_signature_with_ordered_sm2_chains_json(
                    handle,
                    index,
                    options.as_ptr(),
                    options.len(),
                    &mut output,
                )
            },
            0
        );
        let value: serde_json::Value =
            serde_json::from_slice(unsafe { std::slice::from_raw_parts(output.data, output.len) })
                .unwrap();
        assert_eq!(value["all_valid"], true);
        assert_eq!(value["seal_issuer_chain_valid"], serde_json::Value::Null);
        assert_eq!(value["signature"]["all_valid"], true);
        unsafe { ofd_buffer_free(output) };
        let mut output = OfdBuffer {
            data: ptr::null_mut(),
            len: 0,
        };
        assert_eq!(
            unsafe {
                ofd_verify_signature_with_certificate_pools_json(
                    handle,
                    index,
                    options.as_ptr(),
                    options.len(),
                    &mut output,
                )
            },
            0
        );
        let value: serde_json::Value =
            serde_json::from_slice(unsafe { std::slice::from_raw_parts(output.data, output.len) })
                .unwrap();
        assert_eq!(value["all_valid"], true);
        unsafe { ofd_buffer_free(output) };
        let mut output = OfdBuffer {
            data: ptr::null_mut(),
            len: 0,
        };
        let invalid = b"[\"%%%\"]";
        assert_ne!(
            unsafe {
                ofd_verify_signature_with_ordered_sm2_chain_json(
                    handle,
                    index,
                    signer.as_ptr(),
                    signer.len(),
                    invalid.as_ptr(),
                    invalid.len(),
                    anchor.as_ptr(),
                    anchor.len(),
                    1_893_456_000,
                    empty_id.as_ptr(),
                    &mut output,
                )
            },
            0
        );
        unsafe { ofd_close(handle) };
    }

    #[test]
    fn ffi_uses_custom_ofd_signature_distid() {
        let mut handle = ptr::null_mut();
        assert_eq!(unsafe { ofd_new(&mut handle) }, 0);
        let key = include_bytes!("../../ofdrw/tests/fixtures/chain-leaf-private-key.bin");
        let signer = include_bytes!("../../ofdrw/tests/fixtures/chain-leaf-cert.der");
        let intermediate = include_bytes!("../../ofdrw/tests/fixtures/chain-intermediate-cert.der");
        let anchor = include_bytes!("../../ofdrw/tests/fixtures/chain-root-cert.der");
        let date = CString::new("2030-01-01 00:00:00").unwrap();
        let signature_id = CString::new("OFD-Rust-Interop").unwrap();
        let certificate_id = CString::new("").unwrap();
        let mut index = usize::MAX;
        assert_eq!(
            unsafe {
                ofd_add_digital_signature_with_distid(
                    handle,
                    key.as_ptr(),
                    key.len(),
                    date.as_ptr(),
                    signature_id.as_ptr(),
                    &mut index,
                )
            },
            0
        );
        let mut valid = -1;
        assert_eq!(
            unsafe {
                ofd_check_digital_signature_with_certificate(
                    handle,
                    index,
                    signer.as_ptr(),
                    signer.len(),
                    &mut valid,
                )
            },
            0
        );
        assert_eq!(valid, 0);
        assert_eq!(
            unsafe {
                ofd_check_digital_signature_with_certificate_and_distid(
                    handle,
                    index,
                    signer.as_ptr(),
                    signer.len(),
                    signature_id.as_ptr(),
                    &mut valid,
                )
            },
            0
        );
        assert_eq!(valid, 1);
        let intermediates = serde_json::to_vec(&vec![STANDARD.encode(intermediate)]).unwrap();
        let mut output = OfdBuffer {
            data: ptr::null_mut(),
            len: 0,
        };
        assert_eq!(
            unsafe {
                ofd_verify_signature_with_ordered_sm2_chain_and_distids_json(
                    handle,
                    index,
                    signer.as_ptr(),
                    signer.len(),
                    intermediates.as_ptr(),
                    intermediates.len(),
                    anchor.as_ptr(),
                    anchor.len(),
                    1_893_456_000,
                    certificate_id.as_ptr(),
                    signature_id.as_ptr(),
                    &mut output,
                )
            },
            0
        );
        let value: serde_json::Value =
            serde_json::from_slice(unsafe { std::slice::from_raw_parts(output.data, output.len) })
                .unwrap();
        assert_eq!(value["all_valid"], true);
        unsafe { ofd_buffer_free(output) };
        unsafe { ofd_close(handle) };
    }

    #[test]
    fn ffi_reads_bookmarks_and_outlines() {
        for (fixture_name, function, key, expected) in [
            (
                "ofdrw-layout/src/test/resources/Page5.ofd",
                ofd_bookmarks_json
                    as unsafe extern "C" fn(*const OfdDocument, *mut OfdBuffer) -> i32,
                "name",
                "新建书签",
            ),
            (
                "ofdrw-converter/src/test/resources/z.ofd",
                ofd_outlines_json,
                "title",
                "11.4字型变换 ",
            ),
        ] {
            let fixture = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
                .join("../../..")
                .join(fixture_name);
            let path = CString::new(fixture.to_str().unwrap()).unwrap();
            let mut handle = ptr::null_mut();
            assert_eq!(unsafe { ofd_open(path.as_ptr(), &mut handle) }, 0);
            let mut output = OfdBuffer {
                data: ptr::null_mut(),
                len: 0,
            };
            assert_eq!(unsafe { function(handle, &mut output) }, 0);
            let values: serde_json::Value = serde_json::from_slice(unsafe {
                std::slice::from_raw_parts(output.data, output.len)
            })
            .unwrap();
            assert_eq!(values[0][key], expected);
            unsafe {
                ofd_buffer_free(output);
                ofd_close(handle);
            }
        }
    }

    #[test]
    fn ffi_reads_document_versions() {
        let fixture =
            PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../ofdrw/tests/fixtures/versioned.ofd");
        let path = CString::new(fixture.to_str().unwrap()).unwrap();
        let mut handle = ptr::null_mut();
        assert_eq!(unsafe { ofd_open(path.as_ptr(), &mut handle) }, 0);
        let mut output = OfdBuffer {
            data: ptr::null_mut(),
            len: 0,
        };
        assert_eq!(unsafe { ofd_versions_json(handle, &mut output) }, 0);
        let values: serde_json::Value =
            serde_json::from_slice(unsafe { std::slice::from_raw_parts(output.data, output.len) })
                .unwrap();
        assert_eq!(values[0]["id"], "v1");
        assert_eq!(values[0]["current"], true);
        assert_eq!(values[0]["files"].as_array().unwrap().len(), 3);
        unsafe {
            ofd_buffer_free(output);
            ofd_close(handle);
        }
    }

    #[test]
    fn ffi_adds_version_snapshot_and_rejects_bad_requests() {
        let mut handle = ptr::null_mut();
        assert_eq!(unsafe { ofd_new(&mut handle) }, 0);
        let text = CString::new("versioned content").unwrap();
        assert_eq!(
            unsafe { ofd_add_text_page(handle, text.as_ptr(), 80.0, 50.0) },
            0
        );
        let request = CString::new(r#"{"id":"v1","name":"First"}"#).unwrap();
        let mut output = OfdBuffer {
            data: ptr::null_mut(),
            len: 0,
        };
        assert_eq!(
            unsafe { ofd_add_version_snapshot_json(handle, 0, request.as_ptr(), &mut output) },
            0
        );
        let value: serde_json::Value =
            serde_json::from_slice(unsafe { std::slice::from_raw_parts(output.data, output.len) })
                .unwrap();
        assert_eq!(value["id"], "v1");
        assert_eq!(value["files"].as_array().unwrap().len(), 4);
        unsafe { ofd_buffer_free(output) };
        let mut extracted = ptr::null_mut();
        assert_ne!(
            unsafe { ofd_extract_version_snapshot(handle, 0, ptr::null_mut()) },
            0
        );
        assert_ne!(
            unsafe { ofd_extract_version_snapshot(handle, 1, &mut extracted) },
            0
        );
        assert!(extracted.is_null());
        assert_eq!(
            unsafe { ofd_extract_version_snapshot(handle, 0, &mut extracted) },
            0
        );
        assert_eq!(
            unsafe { document_ref(extracted).unwrap() }
                .page_text(0)
                .unwrap(),
            ["versioned content"]
        );
        assert!(unsafe { document_ref(extracted).unwrap() }
            .versions()
            .unwrap()
            .is_empty());
        unsafe { ofd_close(extracted) };
        let mut output = OfdBuffer {
            data: ptr::null_mut(),
            len: 0,
        };
        let mut versions = OfdBuffer {
            data: ptr::null_mut(),
            len: 0,
        };
        assert_eq!(unsafe { ofd_versions_json(handle, &mut versions) }, 0);
        let values: serde_json::Value = serde_json::from_slice(unsafe {
            std::slice::from_raw_parts(versions.data, versions.len)
        })
        .unwrap();
        assert_eq!(values[0]["id"], "v1");
        unsafe { ofd_buffer_free(versions) };
        assert_ne!(
            unsafe { ofd_add_version_snapshot_json(handle, 0, request.as_ptr(), &mut output) },
            0
        );
        let invalid = CString::new(r#"{"id":"../escape"}"#).unwrap();
        assert_ne!(
            unsafe { ofd_add_version_snapshot_json(handle, 0, invalid.as_ptr(), &mut output) },
            0
        );
        let invalid_name = CString::new(r#"{"id":"v2","name":3}"#).unwrap();
        assert_ne!(
            unsafe { ofd_add_version_snapshot_json(handle, 0, invalid_name.as_ptr(), &mut output) },
            0
        );
        assert_ne!(
            unsafe { ofd_add_version_snapshot_json(handle, 0, request.as_ptr(), ptr::null_mut()) },
            0
        );
        unsafe { ofd_close(handle) };
    }

    #[test]
    fn ffi_writes_bookmark_and_reports_bad_input() {
        let mut handle = ptr::null_mut();
        assert_eq!(unsafe { ofd_new(&mut handle) }, 0);
        let text = CString::new("page").unwrap();
        assert_eq!(
            unsafe { ofd_add_text_page(handle, text.as_ptr(), 210.0, 297.0) },
            0
        );
        let bookmark =
            CString::new(r#"{"name":"Rust & Python","page_id":"2","kind":"XYZ","left":12.5}"#)
                .unwrap();
        assert_eq!(
            unsafe { ofd_add_bookmark_json(handle, 0, bookmark.as_ptr()) },
            0
        );
        let mut output = OfdBuffer {
            data: ptr::null_mut(),
            len: 0,
        };
        assert_eq!(unsafe { ofd_bookmarks_json(handle, &mut output) }, 0);
        let values: serde_json::Value =
            serde_json::from_slice(unsafe { std::slice::from_raw_parts(output.data, output.len) })
                .unwrap();
        assert_eq!(values[0]["destination"]["left"], 12.5);
        unsafe { ofd_buffer_free(output) };
        assert_ne!(
            unsafe { ofd_add_bookmark_json(handle, 0, bookmark.as_ptr()) },
            0
        );
        let malformed = CString::new("{broken}").unwrap();
        assert_ne!(
            unsafe { ofd_add_bookmark_json(handle, 0, malformed.as_ptr()) },
            0
        );
        let bad_type =
            CString::new(r#"{"name":"bad","page_id":"2","kind":"XYZ","left":"no"}"#).unwrap();
        assert_ne!(
            unsafe { ofd_add_bookmark_json(handle, 0, bad_type.as_ptr()) },
            0
        );
        let name = CString::new("Rust & Python").unwrap();
        assert_ne!(unsafe { ofd_remove_bookmark(handle, 0, ptr::null()) }, 0);
        assert_eq!(unsafe { ofd_remove_bookmark(handle, 0, name.as_ptr()) }, 0);
        assert_ne!(unsafe { ofd_remove_bookmark(handle, 0, name.as_ptr()) }, 0);
        assert!(unsafe { document_ref(handle).unwrap() }
            .bookmarks()
            .unwrap()
            .is_empty());
        unsafe { ofd_close(handle) };
    }

    #[test]
    fn ffi_exports_svg_preview() {
        let mut handle = ptr::null_mut();
        assert_eq!(unsafe { ofd_new(&mut handle) }, 0);
        let text = CString::new("SVG & Rust").unwrap();
        assert_eq!(
            unsafe { ofd_add_text_page(handle, text.as_ptr(), 210.0, 297.0) },
            0
        );
        let mut output = OfdBuffer {
            data: ptr::null_mut(),
            len: 0,
        };
        assert_eq!(unsafe { ofd_page_svg(handle, 0, &mut output) }, 0);
        let svg =
            std::str::from_utf8(unsafe { std::slice::from_raw_parts(output.data, output.len) })
                .unwrap();
        assert!(svg.contains("SVG &amp; Rust"));
        unsafe { ofd_buffer_free(output) };
        output = OfdBuffer {
            data: ptr::null_mut(),
            len: 0,
        };
        assert_ne!(unsafe { ofd_page_svg(handle, 1, &mut output) }, 0);
        unsafe { ofd_close(handle) };
    }

    #[test]
    fn ffi_renders_png_preview() {
        let mut handle = ptr::null_mut();
        assert_eq!(unsafe { ofd_new(&mut handle) }, 0);
        let text = CString::new("PNG").unwrap();
        assert_eq!(
            unsafe { ofd_add_text_page(handle, text.as_ptr(), 20.0, 20.0) },
            0
        );
        let mut output = OfdBuffer {
            data: ptr::null_mut(),
            len: 0,
        };
        assert_eq!(unsafe { ofd_page_png(handle, 0, 25.4, &mut output) }, 0);
        let png = unsafe { std::slice::from_raw_parts(output.data, output.len) };
        assert!(png.starts_with(b"\x89PNG\r\n\x1a\n"));
        assert_eq!(u32::from_be_bytes(png[16..20].try_into().unwrap()), 20);
        unsafe { ofd_buffer_free(output) };
        output = OfdBuffer {
            data: ptr::null_mut(),
            len: 0,
        };
        assert_ne!(unsafe { ofd_page_png(handle, 0, 0.0, &mut output) }, 0);
        unsafe { ofd_close(handle) };
    }

    #[test]
    fn ffi_exports_multi_page_pdf() {
        let mut handle = ptr::null_mut();
        assert_eq!(unsafe { ofd_new(&mut handle) }, 0);
        let mut output = OfdBuffer {
            data: ptr::null_mut(),
            len: 0,
        };
        assert_ne!(unsafe { ofd_to_pdf(handle, &mut output) }, 0);
        let text = CString::new("PDF").unwrap();
        assert_eq!(
            unsafe { ofd_add_text_page(handle, text.as_ptr(), 20.0, 30.0) },
            0
        );
        assert_eq!(
            unsafe { ofd_add_text_page(handle, text.as_ptr(), 40.0, 50.0) },
            0
        );
        assert_eq!(unsafe { ofd_to_pdf(handle, &mut output) }, 0);
        let pdf = unsafe { std::slice::from_raw_parts(output.data, output.len) };
        assert!(pdf.starts_with(b"%PDF-"));
        assert!(pdf
            .windows(b"/Count 2".len())
            .any(|window| window == b"/Count 2"));
        unsafe {
            ofd_buffer_free(output);
            ofd_close(handle);
        }
    }

    #[test]
    fn ffi_adds_positioned_text_object() {
        let mut handle = ptr::null_mut();
        assert_eq!(unsafe { ofd_new(&mut handle) }, 0);
        let first = CString::new("first").unwrap();
        assert_eq!(
            unsafe { ofd_add_text_page(handle, first.as_ptr(), 210.0, 297.0) },
            0
        );
        let payload =
            CString::new(r#"{"text":"positioned","boundary":[30,40,70,8],"size":5}"#).unwrap();
        let mut id = 0;
        assert_eq!(
            unsafe { ofd_add_text_object_json(handle, 0, payload.as_ptr(), &mut id) },
            0
        );
        assert!(id > 0);
        let mut output = OfdBuffer {
            data: ptr::null_mut(),
            len: 0,
        };
        assert_eq!(
            unsafe { ofd_page_text_objects_json(handle, 0, &mut output) },
            0
        );
        let values: serde_json::Value =
            serde_json::from_slice(unsafe { std::slice::from_raw_parts(output.data, output.len) })
                .unwrap();
        assert_eq!(values[1]["id"], id.to_string());
        assert_eq!(values[1]["codes"][0]["text"], "positioned");
        unsafe { ofd_buffer_free(output) };
        let bad = CString::new(r#"{"text":"bad","boundary":[1,2],"size":5}"#).unwrap();
        assert_ne!(
            unsafe { ofd_add_text_object_json(handle, 0, bad.as_ptr(), &mut id) },
            0
        );
        unsafe { ofd_close(handle) };
    }

    #[test]
    fn ffi_adds_positioned_path_object() {
        let mut handle = ptr::null_mut();
        assert_eq!(unsafe { ofd_new(&mut handle) }, 0);
        let first = CString::new("first").unwrap();
        assert_eq!(
            unsafe { ofd_add_text_page(handle, first.as_ptr(), 210.0, 297.0) },
            0
        );
        let payload = CString::new(
            r#"{"commands":"M 0 0 L 10 10","boundary":[20,30,10,10],"stroke_color":[255,0,0]}"#,
        )
        .unwrap();
        let mut id = 0;
        assert_eq!(
            unsafe { ofd_add_path_object_json(handle, 0, payload.as_ptr(), &mut id) },
            0
        );
        assert!(id > 0);
        let mut output = OfdBuffer {
            data: ptr::null_mut(),
            len: 0,
        };
        assert_eq!(unsafe { ofd_page_paths_json(handle, 0, &mut output) }, 0);
        let values: serde_json::Value =
            serde_json::from_slice(unsafe { std::slice::from_raw_parts(output.data, output.len) })
                .unwrap();
        assert_eq!(values[0]["id"], id.to_string());
        unsafe { ofd_buffer_free(output) };
        let even_odd = CString::new(
            r#"{"commands":"M 0 0 L 10 0 L 10 10 L 0 10 C M 2 2 L 8 2 L 8 8 L 2 8 C","boundary":[0,0,10,10],"fill":true,"stroke":false,"even_odd":true,"alpha":128}"#,
        )
        .unwrap();
        assert_eq!(
            unsafe { ofd_add_path_object_json(handle, 0, even_odd.as_ptr(), &mut id) },
            0
        );
        assert!(unsafe { document_ref(handle).unwrap() }
            .page_svg(0)
            .unwrap()
            .contains("fill-rule=\"evenodd\""));
        assert!(unsafe { document_ref(handle).unwrap() }
            .page_svg(0)
            .unwrap()
            .contains("opacity=\"0.5019607843137255\""));
        let stroke = CString::new(
            r#"{"commands":"M 0 0 L 10 10","boundary":[0,0,10,10],"fill":true,"cap":"round","join":"bevel","miter_limit":5,"dash_pattern":[3,2],"dash_offset":1,"fill_alpha":128,"stroke_alpha":64}"#,
        )
        .unwrap();
        assert_eq!(
            unsafe { ofd_add_path_object_json(handle, 0, stroke.as_ptr(), &mut id) },
            0
        );
        let svg = unsafe { document_ref(handle).unwrap() }
            .page_svg(0)
            .unwrap();
        assert!(svg.contains("stroke-linecap=\"round\""));
        assert!(svg.contains("stroke-linejoin=\"bevel\""));
        assert!(svg.contains("stroke-dasharray=\"3 2\""));
        assert!(svg.contains("fill-opacity=\"0.5019607843137255\""));
        assert!(svg.contains("stroke-opacity=\"0.25098039215686274\""));
        for invalid in [
            r#"{"commands":"M 0 0","boundary":[1,2]}"#,
            r#"{"commands":"M 0 0","boundary":[1,2,3,4],"stroke_color":[256,0,0]}"#,
            r#"{"commands":"M 0 0","boundary":[1,2,3,4],"fill":"yes"}"#,
            r#"{"commands":"M 0 0","boundary":[1,2,3,4],"even_odd":"yes"}"#,
            r#"{"commands":"M 0 0","boundary":[1,2,3,4],"alpha":256}"#,
            r#"{"commands":"M 0 0","boundary":[1,2,3,4],"cap":"flat"}"#,
            r#"{"commands":"M 0 0","boundary":[1,2,3,4],"join":"sharp"}"#,
            r#"{"commands":"M 0 0","boundary":[1,2,3,4],"miter_limit":0}"#,
            r#"{"commands":"M 0 0","boundary":[1,2,3,4],"dash_pattern":[3]}"#,
            r#"{"commands":"M 0 0","boundary":[1,2,3,4],"dash_pattern":[3,0]}"#,
            r#"{"commands":"M 0 0","boundary":[1,2,3,4],"fill_alpha":256}"#,
            r#"{"commands":"M 0 0","boundary":[1,2,3,4],"stroke_alpha":"half"}"#,
        ] {
            let payload = CString::new(invalid).unwrap();
            assert_ne!(
                unsafe { ofd_add_path_object_json(handle, 0, payload.as_ptr(), &mut id) },
                0
            );
        }
        unsafe { ofd_close(handle) };
    }

    #[test]
    fn ffi_adds_composite_image_object() {
        let source = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("../../..")
            .join("ofdrw-reader/src/test/resources/发票示例.ofd");
        let source_document = OfdDocument::open(source).unwrap();
        let image = source_document.page_image_data(0, 0).unwrap();
        let mut handle = ptr::null_mut();
        assert_eq!(unsafe { ofd_new(&mut handle) }, 0);
        let first = CString::new("base").unwrap();
        assert_eq!(
            unsafe { ofd_add_text_page(handle, first.as_ptr(), 40.0, 40.0) },
            0
        );
        let format = CString::new("png").unwrap();
        let mut id = 0;
        assert_eq!(
            unsafe {
                ofd_add_composite_image_object(
                    handle,
                    0,
                    image.as_ptr(),
                    image.len(),
                    format.as_ptr(),
                    2.0,
                    3.0,
                    20.0,
                    20.0,
                    &mut id,
                )
            },
            0
        );
        assert!(id > 0);
        let mut output = OfdBuffer {
            data: ptr::null_mut(),
            len: 0,
        };
        assert_eq!(
            unsafe { ofd_page_composite_objects_json(handle, 0, &mut output) },
            0
        );
        let values: serde_json::Value =
            serde_json::from_slice(unsafe { std::slice::from_raw_parts(output.data, output.len) })
                .unwrap();
        assert_eq!(values[0]["id"], id.to_string());
        assert_eq!(values.as_array().unwrap().len(), 1);
        unsafe { ofd_buffer_free(output) };
        let mut output = OfdBuffer {
            data: ptr::null_mut(),
            len: 0,
        };
        assert_eq!(
            unsafe { ofd_page_composite_images_json(handle, 0, &mut output) },
            0
        );
        let images: serde_json::Value =
            serde_json::from_slice(unsafe { std::slice::from_raw_parts(output.data, output.len) })
                .unwrap();
        assert_eq!(images[0]["format"], "PNG");
        unsafe { ofd_buffer_free(output) };
        let mut output = OfdBuffer {
            data: ptr::null_mut(),
            len: 0,
        };
        assert_eq!(
            unsafe { ofd_page_composite_image_data(handle, 0, 0, &mut output) },
            0
        );
        assert_eq!(
            unsafe { std::slice::from_raw_parts(output.data, output.len) },
            image
        );
        unsafe { ofd_buffer_free(output) };
        let mut output = OfdBuffer {
            data: ptr::null_mut(),
            len: 0,
        };
        assert_ne!(
            unsafe { ofd_page_composite_image_data(handle, 0, 1, &mut output) },
            0
        );
        assert_ne!(
            unsafe {
                ofd_add_composite_image_object(
                    handle,
                    0,
                    ptr::null(),
                    1,
                    format.as_ptr(),
                    2.0,
                    3.0,
                    20.0,
                    20.0,
                    &mut id,
                )
            },
            0
        );
        assert_eq!(id, 0);
        unsafe { ofd_close(handle) };
    }

    #[test]
    fn ffi_adds_positioned_image_object() {
        let mut handle = ptr::null_mut();
        assert_eq!(unsafe { ofd_new(&mut handle) }, 0);
        let first = CString::new("first").unwrap();
        assert_eq!(
            unsafe { ofd_add_text_page(handle, first.as_ptr(), 210.0, 297.0) },
            0
        );
        let format = CString::new("png").unwrap();
        let invoice = OfdDocument::open(
            PathBuf::from(env!("CARGO_MANIFEST_DIR"))
                .join("../../../ofdrw-reader/src/test/resources/发票示例.ofd"),
        )
        .unwrap();
        let image = invoice.page_image_data(0, 0).unwrap();
        let mut id = 0;
        assert_eq!(
            unsafe {
                ofd_add_image_object(
                    handle,
                    0,
                    image.as_ptr(),
                    image.len(),
                    format.as_ptr(),
                    10.0,
                    20.0,
                    30.0,
                    40.0,
                    &mut id,
                )
            },
            0
        );
        assert!(id > 0);
        let mut output = OfdBuffer {
            data: ptr::null_mut(),
            len: 0,
        };
        assert_eq!(unsafe { ofd_page_images_json(handle, 0, &mut output) }, 0);
        let values: serde_json::Value =
            serde_json::from_slice(unsafe { std::slice::from_raw_parts(output.data, output.len) })
                .unwrap();
        assert_eq!(values[0]["id"], id.to_string());
        assert_eq!(
            values[0]["ctm"],
            serde_json::json!([30.0, 0.0, 0.0, 40.0, 0.0, 0.0])
        );
        unsafe { ofd_buffer_free(output) };
        assert_ne!(
            unsafe {
                ofd_add_image_object(
                    handle,
                    0,
                    ptr::null(),
                    1,
                    format.as_ptr(),
                    10.0,
                    20.0,
                    30.0,
                    40.0,
                    &mut id,
                )
            },
            0
        );
        unsafe { ofd_close(handle) };
    }

    #[test]
    fn ffi_adds_text_watermark_annotation() {
        let mut handle = ptr::null_mut();
        assert_eq!(unsafe { ofd_new(&mut handle) }, 0);
        let first = CString::new("first").unwrap();
        assert_eq!(
            unsafe { ofd_add_text_page(handle, first.as_ptr(), 210.0, 297.0) },
            0
        );
        let payload =
            CString::new(r#"{"text":"watermark","boundary":[10,20,60,10],"size":5}"#).unwrap();
        let mut id = 0;
        assert_eq!(
            unsafe { ofd_add_text_watermark_json(handle, 0, payload.as_ptr(), &mut id) },
            0
        );
        assert!(id > 0);
        let mut output = OfdBuffer {
            data: ptr::null_mut(),
            len: 0,
        };
        assert_eq!(unsafe { ofd_annotations_json(handle, &mut output) }, 0);
        let values: serde_json::Value =
            serde_json::from_slice(unsafe { std::slice::from_raw_parts(output.data, output.len) })
                .unwrap();
        assert_eq!(values[0]["id"], id.to_string());
        assert_eq!(values[0]["text"][0], "watermark");
        unsafe { ofd_buffer_free(output) };
        let stamp =
            CString::new(r#"{"text":"批准 & <同意>","boundary":[10,40,60,10],"size":5}"#).unwrap();
        assert_eq!(
            unsafe { ofd_add_text_stamp_annotation_json(handle, 0, stamp.as_ptr(), &mut id) },
            0
        );
        let annotations = unsafe { document_ref(handle).unwrap() }
            .annotations()
            .unwrap();
        assert_eq!(annotations.len(), 2);
        assert_eq!(annotations[1].annotation_type, "Stamp");
        assert_eq!(annotations[1].text, ["批准 & <同意>"]);
        let invalid = CString::new(r#"{"text":"bad","boundary":[1,2],"size":5}"#).unwrap();
        assert_ne!(
            unsafe { ofd_add_text_watermark_json(handle, 0, invalid.as_ptr(), &mut id) },
            0
        );
        assert_ne!(
            unsafe { ofd_add_text_stamp_annotation_json(handle, 0, invalid.as_ptr(), &mut id) },
            0
        );
        assert_ne!(
            unsafe { ofd_add_text_watermark_json(handle, 1, payload.as_ptr(), &mut id) },
            0
        );
        unsafe { ofd_close(handle) };
    }

    #[test]
    fn ffi_adds_styled_text_watermark_and_rejects_invalid_options() {
        let mut handle = ptr::null_mut();
        assert_eq!(unsafe { ofd_new(&mut handle) }, 0);
        let first = CString::new("first").unwrap();
        assert_eq!(
            unsafe { ofd_add_text_page(handle, first.as_ptr(), 80.0, 50.0) },
            0
        );
        let payload = CString::new(r#"{"text":"DRAFT","boundary":[10,10,50,20],"size":5,"color":[255,0,0],"opacity":96,"angle_degrees":-30,"bold":true,"italic":true}"#).unwrap();
        let mut id = 0;
        assert_eq!(
            unsafe { ofd_add_text_watermark_json(handle, 0, payload.as_ptr(), &mut id) },
            0
        );
        assert!(unsafe { document_ref(handle).unwrap() }
            .page_svg(0)
            .unwrap()
            .contains("#ff0000"));
        let before = unsafe { document_ref(handle).unwrap() }.to_bytes().unwrap();
        let invalid =
            CString::new(r#"{"text":"bad","boundary":[10,10,50,20],"size":5,"opacity":256}"#)
                .unwrap();
        assert_ne!(
            unsafe { ofd_add_text_watermark_json(handle, 0, invalid.as_ptr(), &mut id) },
            0
        );
        assert_eq!(id, 0);
        assert_eq!(
            unsafe { document_ref(handle).unwrap() }.to_bytes().unwrap(),
            before
        );
        unsafe { ofd_close(handle) };
    }

    #[test]
    fn ffi_adds_watermarks_to_all_pages_and_checks_output_before_mutation() {
        let mut handle = ptr::null_mut();
        assert_eq!(unsafe { ofd_new(&mut handle) }, 0);
        assert_eq!(unsafe { ofd_add_blank_page(handle, 40.0, 30.0) }, 0);
        assert_eq!(unsafe { ofd_add_blank_page(handle, 60.0, 50.0) }, 0);
        let payload =
            CString::new(r#"{"text":"COPY","size":5,"color":[0,0,255],"tile_spacing":[20,15]}"#)
                .unwrap();
        assert_ne!(
            unsafe {
                ofd_add_text_watermark_all_pages_json(handle, payload.as_ptr(), ptr::null_mut())
            },
            0
        );
        assert!(unsafe { document_ref(handle).unwrap() }
            .annotations()
            .unwrap()
            .is_empty());
        let mut output = OfdBuffer {
            data: ptr::null_mut(),
            len: 0,
        };
        assert_eq!(
            unsafe { ofd_add_text_watermark_all_pages_json(handle, payload.as_ptr(), &mut output) },
            0
        );
        let ids: Vec<usize> =
            serde_json::from_slice(unsafe { std::slice::from_raw_parts(output.data, output.len) })
                .unwrap();
        assert_eq!(ids.len(), 2);
        unsafe { ofd_buffer_free(output) };
        let marks = unsafe { document_ref(handle).unwrap() }
            .annotations()
            .unwrap();
        assert_eq!(marks[0].text.len(), 4);
        assert_eq!(marks[1].text.len(), 12);
        assert!(unsafe { document_ref(handle).unwrap() }
            .page_svg(1)
            .unwrap()
            .contains("#0000ff"));
        let invalid = CString::new(r#"{"text":"bad","size":5,"tile_spacing":[0,15]}"#).unwrap();
        let mut rejected = OfdBuffer {
            data: ptr::null_mut(),
            len: 0,
        };
        assert_ne!(
            unsafe {
                ofd_add_text_watermark_all_pages_json(handle, invalid.as_ptr(), &mut rejected)
            },
            0
        );
        assert_eq!(
            unsafe { document_ref(handle).unwrap() }
                .annotations()
                .unwrap()
                .len(),
            2
        );
        unsafe { ofd_close(handle) };
    }

    #[test]
    fn ffi_adds_image_watermark_from_bytes_and_existing_resource() {
        let invoice = OfdDocument::open(
            PathBuf::from(env!("CARGO_MANIFEST_DIR"))
                .join("../../../ofdrw-reader/src/test/resources/发票示例.ofd"),
        )
        .unwrap();
        let image = invoice.page_image_data(0, 0).unwrap();
        let mut handle = ptr::null_mut();
        assert_eq!(unsafe { ofd_new(&mut handle) }, 0);
        assert_eq!(unsafe { ofd_add_blank_page(handle, 40.0, 30.0) }, 0);
        assert_eq!(unsafe { ofd_add_blank_page(handle, 40.0, 30.0) }, 0);
        let payload = CString::new(format!(
            r#"{{"boundary":[5,5,20,15],"opacity":128,"data_base64":"{}","format":"png"}}"#,
            STANDARD.encode(image)
        ))
        .unwrap();
        let mut id = 0;
        assert_eq!(
            unsafe { ofd_add_image_watermark_json(handle, 0, payload.as_ptr(), &mut id) },
            0
        );
        assert!(id > 0);
        let resource = unsafe { document_ref(handle).unwrap() }
            .document_multimedia_resources(0)
            .unwrap();
        let reuse = CString::new(format!(
            r#"{{"boundary":[5,5,20,15],"resource_id":"{}"}}"#,
            resource[0].id
        ))
        .unwrap();
        assert_eq!(
            unsafe { ofd_add_image_watermark_json(handle, 1, reuse.as_ptr(), &mut id) },
            0
        );
        assert!(unsafe { document_ref(handle).unwrap() }
            .page_svg(1)
            .unwrap()
            .contains("<image"));
        let before = unsafe { document_ref(handle).unwrap() }.to_bytes().unwrap();
        let invalid =
            CString::new(r#"{"boundary":[5,5,20,15],"data_base64":"bad","format":"png"}"#).unwrap();
        assert_ne!(
            unsafe { ofd_add_image_watermark_json(handle, 0, invalid.as_ptr(), &mut id) },
            0
        );
        assert_eq!(id, 0);
        assert_eq!(
            unsafe { document_ref(handle).unwrap() }.to_bytes().unwrap(),
            before
        );
        unsafe { ofd_close(handle) };
    }

    #[test]
    fn ffi_adds_image_stamp_annotation_and_rejects_bad_image() {
        let invoice = OfdDocument::open(
            PathBuf::from(env!("CARGO_MANIFEST_DIR"))
                .join("../../../ofdrw-reader/src/test/resources/发票示例.ofd"),
        )
        .unwrap();
        let image = invoice.page_image_data(0, 0).unwrap();
        let mut handle = ptr::null_mut();
        assert_eq!(unsafe { ofd_new(&mut handle) }, 0);
        assert_eq!(unsafe { ofd_add_blank_page(handle, 40.0, 30.0) }, 0);
        let payload = CString::new(format!(
            r#"{{"boundary":[5,5,20,15],"data_base64":"{}","format":"png"}}"#,
            STANDARD.encode(image)
        ))
        .unwrap();
        let mut id = 0;
        assert_eq!(
            unsafe { ofd_add_image_stamp_annotation_json(handle, 0, payload.as_ptr(), &mut id) },
            0
        );
        assert_eq!(
            unsafe { document_ref(handle).unwrap() }
                .annotations()
                .unwrap()[0]
                .annotation_type,
            "Stamp"
        );
        assert!(unsafe { document_ref(handle).unwrap() }
            .page_svg(0)
            .unwrap()
            .contains("<image"));
        let invalid =
            CString::new(r#"{"boundary":[5,5,20,15],"data_base64":"???","format":"png"}"#).unwrap();
        assert_ne!(
            unsafe { ofd_add_image_stamp_annotation_json(handle, 0, invalid.as_ptr(), &mut id) },
            0
        );
        assert_eq!(
            unsafe { document_ref(handle).unwrap() }
                .annotations()
                .unwrap()
                .len(),
            1
        );
        unsafe { ofd_close(handle) };
    }

    #[test]
    fn ffi_adds_image_watermark_to_all_pages_atomically() {
        let invoice = OfdDocument::open(
            PathBuf::from(env!("CARGO_MANIFEST_DIR"))
                .join("../../../ofdrw-reader/src/test/resources/发票示例.ofd"),
        )
        .unwrap();
        let image = invoice.page_image_data(0, 0).unwrap();
        let mut handle = ptr::null_mut();
        assert_eq!(unsafe { ofd_new(&mut handle) }, 0);
        assert_eq!(unsafe { ofd_add_blank_page(handle, 40.0, 30.0) }, 0);
        assert_eq!(unsafe { ofd_add_blank_page(handle, 40.0, 30.0) }, 0);
        let payload = CString::new(format!(
            r#"{{"boundary":[5,5,20,15],"data_base64":"{}","format":"png"}}"#,
            STANDARD.encode(image)
        ))
        .unwrap();
        assert_ne!(
            unsafe {
                ofd_add_image_watermark_all_pages_json(handle, payload.as_ptr(), ptr::null_mut())
            },
            0
        );
        assert!(unsafe { document_ref(handle).unwrap() }
            .annotations()
            .unwrap()
            .is_empty());
        let mut output = OfdBuffer {
            data: ptr::null_mut(),
            len: 0,
        };
        assert_eq!(
            unsafe {
                ofd_add_image_watermark_all_pages_json(handle, payload.as_ptr(), &mut output)
            },
            0
        );
        let ids: Vec<usize> =
            serde_json::from_slice(unsafe { std::slice::from_raw_parts(output.data, output.len) })
                .unwrap();
        assert_eq!(ids.len(), 2);
        unsafe { ofd_buffer_free(output) };
        assert_eq!(
            unsafe { document_ref(handle).unwrap() }
                .document_multimedia_resources(0)
                .unwrap()
                .len(),
            1
        );
        let before = unsafe { document_ref(handle).unwrap() }.to_bytes().unwrap();
        let invalid =
            CString::new(r#"{"boundary":[5,5,20,15],"data_base64":"bad","format":"png"}"#).unwrap();
        let mut rejected = OfdBuffer {
            data: ptr::null_mut(),
            len: 0,
        };
        assert_ne!(
            unsafe {
                ofd_add_image_watermark_all_pages_json(handle, invalid.as_ptr(), &mut rejected)
            },
            0
        );
        assert_eq!(
            unsafe { document_ref(handle).unwrap() }.to_bytes().unwrap(),
            before
        );
        unsafe { ofd_close(handle) };
    }

    #[test]
    fn ffi_adds_path_annotation() {
        let mut handle = ptr::null_mut();
        assert_eq!(unsafe { ofd_new(&mut handle) }, 0);
        assert_eq!(unsafe { ofd_add_blank_page(handle, 30.0, 20.0) }, 0);
        let payload = CString::new(r#"{"commands":"M 0 0 L 10 5","boundary":[2,3,10,5],"stroke":true,"stroke_color":[255,0,0]}"#).unwrap();
        let mut id = 0;
        assert_eq!(
            unsafe { ofd_add_path_annotation_json(handle, 0, payload.as_ptr(), &mut id) },
            0
        );
        assert!(id > 0);
        let document = unsafe { document_ref(handle).unwrap() };
        assert_eq!(document.annotations().unwrap()[0].annotation_type, "Path");
        assert!(document.page_svg(0).unwrap().contains("#ff0000"));
        assert!(document.validate_references().unwrap().is_empty());
        let id_text = CString::new(id.to_string()).unwrap();
        assert_eq!(
            unsafe { ofd_set_annotation_print(handle, 0, id_text.as_ptr(), 0) },
            0
        );
        assert!(unsafe { document_ref(handle).unwrap() }
            .page_svg(0)
            .unwrap()
            .contains("#ff0000"));
        assert_eq!(
            unsafe { ofd_set_annotation_visible(handle, 0, id_text.as_ptr(), 0) },
            0
        );
        assert!(!unsafe { document_ref(handle).unwrap() }
            .page_svg(0)
            .unwrap()
            .contains("data-ofd-annotation"));
        assert_ne!(
            unsafe { ofd_set_annotation_visible(handle, 0, id_text.as_ptr(), 2) },
            0
        );
        assert_ne!(
            unsafe { ofd_set_annotation_print(handle, 0, id_text.as_ptr(), -1) },
            0
        );
        for flag in ["NoZoom", "NoRotate", "ReadOnly"] {
            let flag = CString::new(flag).unwrap();
            assert_eq!(
                unsafe { ofd_set_annotation_flag(handle, 0, id_text.as_ptr(), flag.as_ptr(), 1) },
                0
            );
        }
        assert_eq!(
            unsafe { document_ref(handle).unwrap() }
                .annotations()
                .unwrap()[0]
                .attributes["NoRotate"],
            "true"
        );
        let unknown = CString::new("Unknown").unwrap();
        assert_ne!(
            unsafe { ofd_set_annotation_flag(handle, 0, id_text.as_ptr(), unknown.as_ptr(), 1) },
            0
        );
        assert_ne!(
            unsafe { ofd_set_annotation_flag(handle, 0, id_text.as_ptr(), unknown.as_ptr(), 2) },
            0
        );
        let invalid = CString::new(r#"{"commands":"","boundary":[2,3,10,5]}"#).unwrap();
        assert_ne!(
            unsafe { ofd_add_path_annotation_json(handle, 0, invalid.as_ptr(), &mut id) },
            0
        );
        assert_eq!(id, 0);
        unsafe { ofd_close(handle) };
    }

    #[test]
    fn ffi_adds_multi_path_annotation_atomically() {
        let mut handle = ptr::null_mut();
        assert_eq!(unsafe { ofd_new(&mut handle) }, 0);
        assert_eq!(unsafe { ofd_add_blank_page(handle, 40.0, 30.0) }, 0);
        let payload = CString::new(r#"{"boundary":[3,4,25,15],"paths":[{"commands":"M 0 0 L 10 10","boundary":[0,0,10,10],"stroke_color":[255,0,0]},{"commands":"M 0 10 L 10 0","boundary":[10,0,10,10],"stroke_color":[0,0,255]}]}"#).unwrap();
        let mut id = 0;
        assert_eq!(
            unsafe { ofd_add_multi_path_annotation_json(handle, 0, payload.as_ptr(), &mut id) },
            0
        );
        assert!(id > 0);
        let doc = unsafe { document_ref(handle).unwrap() };
        assert_eq!(doc.annotations().unwrap().len(), 1);
        assert!(doc.page_svg(0).unwrap().contains("#ff0000"));
        assert!(doc.page_svg(0).unwrap().contains("#0000ff"));
        let before = doc.to_bytes().unwrap();
        let invalid = CString::new(r#"{"boundary":[3,4,25,15],"paths":[{"commands":"M 0 0 L 10 10","boundary":[0,0,10,10]},{"commands":"bad","boundary":[10,0,10,10]}]}"#).unwrap();
        assert_ne!(
            unsafe { ofd_add_multi_path_annotation_json(handle, 0, invalid.as_ptr(), &mut id) },
            0
        );
        assert_eq!(id, 0);
        assert_eq!(
            unsafe { document_ref(handle).unwrap() }.to_bytes().unwrap(),
            before
        );
        unsafe { ofd_close(handle) };
    }

    #[test]
    fn ffi_adds_mixed_annotation_and_rejects_invalid_objects() {
        let mut handle = ptr::null_mut();
        assert_eq!(unsafe { ofd_new(&mut handle) }, 0);
        assert_eq!(unsafe { ofd_add_blank_page(handle, 40.0, 30.0) }, 0);
        let payload = CString::new(r#"{"boundary":[2,3,30,12],"objects":[{"kind":"path","commands":"M 0 0 L 9 9","boundary":[0,0,10,10],"stroke_color":[255,0,0]},{"kind":"text","text":"Mixed","boundary":[10,0,12,10],"size":4,"color":[0,0,255]}]}"#).unwrap();
        let mut id = 0;
        assert_eq!(
            unsafe { ofd_add_mixed_annotation_json(handle, 0, payload.as_ptr(), &mut id) },
            0
        );
        assert!(id > 0);
        let svg = unsafe { document_ref(handle).unwrap() }
            .page_svg(0)
            .unwrap();
        assert!(svg.contains("#ff0000") && svg.contains("Mixed"));
        let annotation_id = CString::new(id.to_string()).unwrap();
        let mut output = OfdBuffer {
            data: ptr::null_mut(),
            len: 0,
        };
        assert_eq!(
            unsafe { ofd_annotation_objects_json(handle, 0, annotation_id.as_ptr(), &mut output) },
            0
        );
        let objects: serde_json::Value =
            serde_json::from_slice(unsafe { std::slice::from_raw_parts(output.data, output.len) })
                .unwrap();
        assert_eq!(objects[0]["kind"], "PathObject");
        assert_eq!(objects[1]["text"][0], "Mixed");
        let object_id = CString::new(objects[0]["id"].as_str().unwrap()).unwrap();
        unsafe { ofd_buffer_free(output) };
        let update = CString::new(
            r#"{"boundary":[1,1,9,9],"ctm":[1,0,0,1,2,3],"visible":false,"alpha":128}"#,
        )
        .unwrap();
        assert_eq!(
            unsafe {
                ofd_update_annotation_object_json(
                    handle,
                    0,
                    annotation_id.as_ptr(),
                    object_id.as_ptr(),
                    update.as_ptr(),
                )
            },
            0
        );
        assert_eq!(
            unsafe { document_ref(handle).unwrap() }
                .annotation_objects(0, annotation_id.to_str().unwrap())
                .unwrap()[0]
                .alpha,
            128
        );
        let invalid_update = CString::new(r#"{"boundary":[0,0,0,9]}"#).unwrap();
        let before_update = unsafe { document_ref(handle).unwrap() }.to_bytes().unwrap();
        assert_ne!(
            unsafe {
                ofd_update_annotation_object_json(
                    handle,
                    0,
                    annotation_id.as_ptr(),
                    object_id.as_ptr(),
                    invalid_update.as_ptr(),
                )
            },
            0
        );
        assert_eq!(
            unsafe { document_ref(handle).unwrap() }.to_bytes().unwrap(),
            before_update
        );
        let mut output = OfdBuffer {
            data: ptr::null_mut(),
            len: 0,
        };
        let missing = CString::new("missing").unwrap();
        assert_ne!(
            unsafe { ofd_annotation_objects_json(handle, 0, missing.as_ptr(), &mut output) },
            0
        );
        let stamp = CString::new(r#"{"annotation_type":"Stamp","boundary":[2,15,30,12],"objects":[{"kind":"path","commands":"M 0 0 L 9 9","boundary":[0,0,10,10]},{"kind":"text","text":"Stamp","boundary":[10,0,12,10],"size":4}]}"#).unwrap();
        assert_eq!(
            unsafe { ofd_add_mixed_annotation_json(handle, 0, stamp.as_ptr(), &mut id) },
            0
        );
        assert_eq!(
            unsafe { document_ref(handle).unwrap() }
                .annotations()
                .unwrap()[1]
                .annotation_type,
            "Stamp"
        );
        let before = unsafe { document_ref(handle).unwrap() }.to_bytes().unwrap();
        let invalid = CString::new(r#"{"boundary":[2,3,30,12],"objects":[{"kind":"path","commands":"M 0 0 L 9 9","boundary":[0,0,10,10]},{"kind":"image","resource_id":"999999","boundary":[20,0,8,8]}]}"#).unwrap();
        assert_ne!(
            unsafe { ofd_add_mixed_annotation_json(handle, 0, invalid.as_ptr(), &mut id) },
            0
        );
        assert_eq!(id, 0);
        assert_eq!(
            unsafe { document_ref(handle).unwrap() }.to_bytes().unwrap(),
            before
        );
        let unsupported = CString::new(r#"{"annotation_type":"Widget","boundary":[2,3,30,12],"objects":[{"kind":"text","text":"bad","boundary":[0,0,10,10],"size":4}]}"#).unwrap();
        assert_ne!(
            unsafe { ofd_add_mixed_annotation_json(handle, 0, unsupported.as_ptr(), &mut id) },
            0
        );
        assert_eq!(id, 0);
        assert_eq!(
            unsafe { document_ref(handle).unwrap() }.to_bytes().unwrap(),
            before
        );
        assert_eq!(
            unsafe {
                ofd_remove_annotation_object(handle, 0, annotation_id.as_ptr(), object_id.as_ptr())
            },
            0
        );
        assert_eq!(
            unsafe { document_ref(handle).unwrap() }
                .annotation_objects(0, annotation_id.to_str().unwrap())
                .unwrap()
                .len(),
            1
        );
        assert_ne!(
            unsafe {
                ofd_remove_annotation_object(handle, 0, annotation_id.as_ptr(), object_id.as_ptr())
            },
            0
        );
        unsafe { ofd_close(handle) };
    }

    #[test]
    fn ffi_appends_annotation_objects_and_preserves_failed_edits() {
        let mut handle = ptr::null_mut();
        assert_eq!(unsafe { ofd_new(&mut handle) }, 0);
        assert_eq!(unsafe { ofd_add_blank_page(handle, 40.0, 30.0) }, 0);
        let initial = CString::new(r#"{"boundary":[1,1,20,10],"objects":[{"kind":"text","text":"first","boundary":[0,0,8,5],"size":3}]}"#).unwrap();
        let mut annotation_id = 0;
        assert_eq!(
            unsafe {
                ofd_add_mixed_annotation_json(handle, 0, initial.as_ptr(), &mut annotation_id)
            },
            0
        );
        let id = CString::new(annotation_id.to_string()).unwrap();
        let request = CString::new(r#"{"objects":[{"kind":"path","commands":"M 0 0 L 5 5","boundary":[0,0,5,5]},{"kind":"text","text":"second","boundary":[5,0,8,5],"size":3}]}"#).unwrap();
        let mut output = OfdBuffer {
            data: ptr::null_mut(),
            len: 0,
        };
        assert_eq!(
            unsafe {
                ofd_append_annotation_objects_json(
                    handle,
                    0,
                    id.as_ptr(),
                    request.as_ptr(),
                    &mut output,
                )
            },
            0
        );
        let ids: Vec<String> =
            serde_json::from_slice(unsafe { std::slice::from_raw_parts(output.data, output.len) })
                .unwrap();
        unsafe { ofd_buffer_free(output) };
        assert_eq!(ids.len(), 2);
        let bytes = unsafe { document_ref(handle).unwrap() }.to_bytes().unwrap();
        let reopened = OfdDocument::from_bytes(&bytes).unwrap();
        let graphics = reopened
            .annotation_objects(0, id.to_str().unwrap())
            .unwrap();
        assert_eq!(graphics[1].id.as_deref(), Some(ids[0].as_str()));
        assert_eq!(graphics[2].id.as_deref(), Some(ids[1].as_str()));
        let invalid = CString::new(
            r#"{"objects":[{"kind":"image","resource_id":"999999","boundary":[0,0,5,5]}]}"#,
        )
        .unwrap();
        let mut output = OfdBuffer {
            data: ptr::null_mut(),
            len: 0,
        };
        assert_ne!(
            unsafe {
                ofd_append_annotation_objects_json(
                    handle,
                    0,
                    id.as_ptr(),
                    invalid.as_ptr(),
                    &mut output,
                )
            },
            0
        );
        assert_eq!(
            unsafe { document_ref(handle).unwrap() }.to_bytes().unwrap(),
            bytes
        );
        unsafe { ofd_close(handle) };
    }

    #[test]
    fn ffi_writes_composite_annotation_graphics() {
        let mut handle = ptr::null_mut();
        assert_eq!(unsafe { ofd_new(&mut handle) }, 0);
        assert_eq!(unsafe { ofd_add_blank_page(handle, 40.0, 30.0) }, 0);
        unsafe { document_mut(handle).unwrap() }
            .add_composite_path_object(
                0,
                "M 0 0 L 8 0 L 8 8 C",
                [0.0, 0.0, 8.0, 8.0],
                PathStyle::default(),
            )
            .unwrap();
        let resource_id = unsafe { document_ref(handle).unwrap() }
            .page_composite_objects(0)
            .unwrap()[0]
            .resource_id
            .clone();
        let payload = CString::new(format!(
            r#"{{"boundary":[2,2,20,12],"objects":[{{"kind":"composite","resource_id":"{resource_id}","boundary":[0,0,8,8],"ctm":[1,0,0,1,1,2],"alpha":180}}]}}"#,
        ))
        .unwrap();
        let mut id = 0;
        assert_eq!(
            unsafe { ofd_add_mixed_annotation_json(handle, 0, payload.as_ptr(), &mut id) },
            0
        );
        let annotation_id = CString::new(id.to_string()).unwrap();
        let objects = unsafe { document_ref(handle).unwrap() }
            .annotation_objects(0, annotation_id.to_str().unwrap())
            .unwrap();
        assert_eq!(objects[0].kind, "CompositeObject");
        assert_eq!(
            objects[0].resource_id.as_deref(),
            Some(resource_id.as_str())
        );
        assert_eq!(objects[0].alpha, 180);
        let mut output = OfdBuffer {
            data: ptr::null_mut(),
            len: 0,
        };
        assert_eq!(
            unsafe {
                ofd_append_annotation_objects_json(
                    handle,
                    0,
                    annotation_id.as_ptr(),
                    payload.as_ptr(),
                    &mut output,
                )
            },
            0
        );
        let ids: Vec<String> =
            serde_json::from_slice(unsafe { std::slice::from_raw_parts(output.data, output.len) })
                .unwrap();
        unsafe { ofd_buffer_free(output) };
        assert_eq!(ids.len(), 1);
        assert_eq!(
            unsafe { document_ref(handle).unwrap() }
                .annotation_objects(0, annotation_id.to_str().unwrap())
                .unwrap()[1]
                .id
                .as_deref(),
            Some(ids[0].as_str())
        );
        let before = unsafe { document_ref(handle).unwrap() }.to_bytes().unwrap();
        let missing = CString::new(
            r#"{"objects":[{"kind":"composite","resource_id":"999999","boundary":[0,0,8,8]}]}"#,
        )
        .unwrap();
        let mut output = OfdBuffer {
            data: ptr::null_mut(),
            len: 0,
        };
        assert_ne!(
            unsafe {
                ofd_append_annotation_objects_json(
                    handle,
                    0,
                    annotation_id.as_ptr(),
                    missing.as_ptr(),
                    &mut output,
                )
            },
            0
        );
        assert_eq!(
            unsafe { document_ref(handle).unwrap() }.to_bytes().unwrap(),
            before
        );
        unsafe { ofd_close(handle) };
    }

    #[test]
    fn ffi_updates_annotation_path_commands_atomically() {
        let mut handle = ptr::null_mut();
        assert_eq!(unsafe { ofd_new(&mut handle) }, 0);
        assert_eq!(unsafe { ofd_add_blank_page(handle, 40.0, 30.0) }, 0);
        let request = CString::new(r#"{"boundary":[2,2,20,12],"objects":[{"kind":"path","commands":"M 0 0 L 8 8","boundary":[0,0,8,8]},{"kind":"text","text":"keep","boundary":[8,0,8,8],"size":3}]}"#).unwrap();
        let mut id = 0;
        assert_eq!(
            unsafe { ofd_add_mixed_annotation_json(handle, 0, request.as_ptr(), &mut id) },
            0
        );
        let annotation_id = CString::new(id.to_string()).unwrap();
        let objects = unsafe { document_ref(handle).unwrap() }
            .annotation_objects(0, annotation_id.to_str().unwrap())
            .unwrap();
        let path_id = CString::new(objects[0].id.as_deref().unwrap()).unwrap();
        let text_id = CString::new(objects[1].id.as_deref().unwrap()).unwrap();
        let commands = CString::new("M 0 8 L 8 0").unwrap();
        assert_eq!(
            unsafe {
                ofd_set_annotation_path_data(
                    handle,
                    0,
                    annotation_id.as_ptr(),
                    path_id.as_ptr(),
                    commands.as_ptr(),
                )
            },
            0
        );
        let edited = unsafe { document_ref(handle).unwrap() }
            .annotation_objects(0, annotation_id.to_str().unwrap())
            .unwrap();
        assert_eq!(edited[0].commands.as_deref(), Some("M 0 8 L 8 0"));
        let before = unsafe { document_ref(handle).unwrap() }.to_bytes().unwrap();
        let invalid = CString::new("invalid").unwrap();
        assert_ne!(
            unsafe {
                ofd_set_annotation_path_data(
                    handle,
                    0,
                    annotation_id.as_ptr(),
                    path_id.as_ptr(),
                    invalid.as_ptr(),
                )
            },
            0
        );
        assert_ne!(
            unsafe {
                ofd_set_annotation_path_data(
                    handle,
                    0,
                    annotation_id.as_ptr(),
                    text_id.as_ptr(),
                    commands.as_ptr(),
                )
            },
            0
        );
        assert_eq!(
            unsafe { document_ref(handle).unwrap() }.to_bytes().unwrap(),
            before
        );
        unsafe { ofd_close(handle) };
    }

    #[test]
    fn ffi_updates_annotation_path_stroke_style_atomically() {
        let mut handle = ptr::null_mut();
        assert_eq!(unsafe { ofd_new(&mut handle) }, 0);
        assert_eq!(unsafe { ofd_add_blank_page(handle, 40.0, 30.0) }, 0);
        let request = CString::new(r#"{"boundary":[2,2,20,12],"objects":[{"kind":"path","commands":"M 0 0 L 8 8","boundary":[0,0,8,8]},{"kind":"text","text":"keep","boundary":[8,0,8,8],"size":3}]}"#).unwrap();
        let mut id = 0;
        assert_eq!(
            unsafe { ofd_add_mixed_annotation_json(handle, 0, request.as_ptr(), &mut id) },
            0
        );
        let annotation_id = CString::new(id.to_string()).unwrap();
        let objects = unsafe { document_ref(handle).unwrap() }
            .annotation_objects(0, annotation_id.to_str().unwrap())
            .unwrap();
        let path_id = CString::new(objects[0].id.as_deref().unwrap()).unwrap();
        let text_id = CString::new(objects[1].id.as_deref().unwrap()).unwrap();
        let style = CString::new(
            r#"{"line_width":1.25,"cap":"round","dash_sequence":[1,2,3],"dash_offset":0.5}"#,
        )
        .unwrap();
        assert_eq!(
            unsafe {
                ofd_set_annotation_path_stroke_style_json(
                    handle,
                    0,
                    annotation_id.as_ptr(),
                    path_id.as_ptr(),
                    style.as_ptr(),
                )
            },
            0
        );
        let source_path = unsafe { document_ref(handle).unwrap() }
            .annotations()
            .unwrap()[0]
            .source_path
            .clone();
        let xml = unsafe { document_ref(handle).unwrap() }
            .entry(&source_path)
            .unwrap()
            .unwrap();
        let xml = std::str::from_utf8(xml).unwrap();
        assert!(xml.contains("LineWidth=\"1.25\""));
        assert!(xml.contains("DashPattern=\"1 2 3\""));
        let before = unsafe { document_ref(handle).unwrap() }.to_bytes().unwrap();
        let invalid = CString::new(r#"{"line_width":-1}"#).unwrap();
        assert_ne!(
            unsafe {
                ofd_set_annotation_path_stroke_style_json(
                    handle,
                    0,
                    annotation_id.as_ptr(),
                    path_id.as_ptr(),
                    invalid.as_ptr(),
                )
            },
            0
        );
        assert_ne!(
            unsafe {
                ofd_set_annotation_path_stroke_style_json(
                    handle,
                    0,
                    annotation_id.as_ptr(),
                    text_id.as_ptr(),
                    style.as_ptr(),
                )
            },
            0
        );
        assert_eq!(
            unsafe { document_ref(handle).unwrap() }.to_bytes().unwrap(),
            before
        );
        unsafe { ofd_close(handle) };
    }

    #[test]
    fn ffi_updates_annotation_text_code_and_rejects_wrong_object() {
        let mut handle = ptr::null_mut();
        assert_eq!(unsafe { ofd_new(&mut handle) }, 0);
        assert_eq!(unsafe { ofd_add_blank_page(handle, 40.0, 30.0) }, 0);
        let request = CString::new(r#"{"boundary":[2,2,25,12],"objects":[{"kind":"text","text":"AB","boundary":[0,0,15,8],"size":3},{"kind":"path","commands":"M 0 0 L 8 8","boundary":[15,0,8,8]}]}"#).unwrap();
        let mut id = 0;
        assert_eq!(
            unsafe { ofd_add_mixed_annotation_json(handle, 0, request.as_ptr(), &mut id) },
            0
        );
        let annotation_id = CString::new(id.to_string()).unwrap();
        let objects = unsafe { document_ref(handle).unwrap() }
            .annotation_objects(0, annotation_id.to_str().unwrap())
            .unwrap();
        let text_id = CString::new(objects[0].id.as_deref().unwrap()).unwrap();
        let path_id = CString::new(objects[1].id.as_deref().unwrap()).unwrap();
        let text = CString::new("A & <B>").unwrap();
        assert_eq!(
            unsafe {
                ofd_set_annotation_text_code(
                    handle,
                    0,
                    annotation_id.as_ptr(),
                    text_id.as_ptr(),
                    0,
                    text.as_ptr(),
                )
            },
            0
        );
        let edited = unsafe { document_ref(handle).unwrap() }
            .annotation_objects(0, annotation_id.to_str().unwrap())
            .unwrap();
        assert_eq!(edited[0].text, ["A & <B>"]);
        let before = unsafe { document_ref(handle).unwrap() }.to_bytes().unwrap();
        assert_ne!(
            unsafe {
                ofd_set_annotation_text_code(
                    handle,
                    0,
                    annotation_id.as_ptr(),
                    path_id.as_ptr(),
                    0,
                    text.as_ptr(),
                )
            },
            0
        );
        assert_ne!(
            unsafe {
                ofd_set_annotation_text_code(
                    handle,
                    0,
                    annotation_id.as_ptr(),
                    text_id.as_ptr(),
                    1,
                    text.as_ptr(),
                )
            },
            0
        );
        assert_eq!(
            unsafe { document_ref(handle).unwrap() }.to_bytes().unwrap(),
            before
        );
        unsafe { ofd_close(handle) };
    }

    #[test]
    fn ffi_edits_annotation_text_layout_paint_and_object_metadata() {
        let mut handle = ptr::null_mut();
        assert_eq!(unsafe { ofd_new(&mut handle) }, 0);
        assert_eq!(unsafe { ofd_add_blank_page(handle, 40.0, 30.0) }, 0);
        let request = CString::new(r#"{"boundary":[2,2,25,12],"objects":[{"kind":"text","text":"AB","boundary":[0,0,15,8],"size":3}]}"#).unwrap();
        let mut annotation_id = 0;
        assert_eq!(
            unsafe {
                ofd_add_mixed_annotation_json(handle, 0, request.as_ptr(), &mut annotation_id)
            },
            0
        );
        let annotation_id = CString::new(annotation_id.to_string()).unwrap();
        let objects = unsafe { document_ref(handle).unwrap() }
            .annotation_objects(0, annotation_id.to_str().unwrap())
            .unwrap();
        let object_id = CString::new(objects[0].id.as_deref().unwrap()).unwrap();
        let name = CString::new("review-text").unwrap();
        assert_eq!(
            unsafe {
                ofd_set_annotation_object_name(
                    handle,
                    0,
                    annotation_id.as_ptr(),
                    object_id.as_ptr(),
                    name.as_ptr(),
                )
            },
            0
        );
        let blend = CString::new("Multiply").unwrap();
        assert_eq!(
            unsafe {
                ofd_set_annotation_object_blend_mode(
                    handle,
                    0,
                    annotation_id.as_ptr(),
                    object_id.as_ptr(),
                    blend.as_ptr(),
                )
            },
            0
        );
        let missing_draw_param = CString::new("999").unwrap();
        let before = unsafe { document_ref(handle).unwrap() }.to_bytes().unwrap();
        assert_ne!(
            unsafe {
                ofd_set_annotation_object_draw_param(
                    handle,
                    0,
                    annotation_id.as_ptr(),
                    object_id.as_ptr(),
                    missing_draw_param.as_ptr(),
                )
            },
            0
        );
        assert_eq!(
            unsafe { document_ref(handle).unwrap() }.to_bytes().unwrap(),
            before
        );
        let typography =
            CString::new(r#"{"size":4,"weight":600,"italic":true,"h_scale":0.8}"#).unwrap();
        assert_eq!(
            unsafe {
                ofd_set_annotation_text_typography_json(
                    handle,
                    0,
                    annotation_id.as_ptr(),
                    object_id.as_ptr(),
                    typography.as_ptr(),
                )
            },
            0
        );
        let paint = CString::new(r#"{"fill":false,"stroke":true}"#).unwrap();
        assert_eq!(
            unsafe {
                ofd_set_annotation_text_paint_flags_json(
                    handle,
                    0,
                    annotation_id.as_ptr(),
                    object_id.as_ptr(),
                    paint.as_ptr(),
                )
            },
            0
        );
        let layout = CString::new(r#"{"x":1,"y":5,"delta_x":[1]}"#).unwrap();
        assert_eq!(
            unsafe {
                ofd_set_annotation_text_code_layout_json(
                    handle,
                    0,
                    annotation_id.as_ptr(),
                    object_id.as_ptr(),
                    0,
                    layout.as_ptr(),
                )
            },
            0
        );
        let appended = CString::new("CD").unwrap();
        let append_layout = CString::new(r#"{"x":7,"y":5}"#).unwrap();
        assert_eq!(
            unsafe {
                ofd_append_annotation_text_code_json(
                    handle,
                    0,
                    annotation_id.as_ptr(),
                    object_id.as_ptr(),
                    appended.as_ptr(),
                    append_layout.as_ptr(),
                )
            },
            0
        );
        let font_id = CString::new("1").unwrap();
        assert_eq!(
            unsafe {
                ofd_set_annotation_text_font(
                    handle,
                    0,
                    annotation_id.as_ptr(),
                    object_id.as_ptr(),
                    font_id.as_ptr(),
                )
            },
            0
        );
        let edited = unsafe { document_ref(handle).unwrap() }
            .annotation_objects(0, annotation_id.to_str().unwrap())
            .unwrap();
        assert_eq!(edited[0].text, ["AB", "CD"]);
        assert_eq!(edited[0].name.as_deref(), Some("review-text"));
        assert_eq!(edited[0].blend_mode.as_deref(), Some("Multiply"));
        assert_eq!(
            unsafe {
                ofd_remove_annotation_text_code(
                    handle,
                    0,
                    annotation_id.as_ptr(),
                    object_id.as_ptr(),
                    0,
                )
            },
            0
        );
        let edited = unsafe { document_ref(handle).unwrap() }
            .annotation_objects(0, annotation_id.to_str().unwrap())
            .unwrap();
        assert_eq!(edited[0].text, ["CD"]);
        assert_eq!(
            unsafe {
                ofd_set_annotation_object_name(
                    handle,
                    0,
                    annotation_id.as_ptr(),
                    object_id.as_ptr(),
                    ptr::null(),
                )
            },
            0
        );
        assert_eq!(
            unsafe {
                ofd_set_annotation_object_blend_mode(
                    handle,
                    0,
                    annotation_id.as_ptr(),
                    object_id.as_ptr(),
                    ptr::null(),
                )
            },
            0
        );
        assert!(unsafe { document_ref(handle).unwrap() }
            .validate_references()
            .unwrap()
            .is_empty());
        unsafe { ofd_close(handle) };
    }

    #[test]
    fn ffi_annotation_text_json_preserves_embedded_nul() {
        let mut handle = ptr::null_mut();
        assert_eq!(unsafe { ofd_new(&mut handle) }, 0);
        assert_eq!(unsafe { ofd_add_blank_page(handle, 40.0, 30.0) }, 0);
        let request = CString::new(r#"{"boundary":[2,2,20,10],"objects":[{"kind":"text","text":"AB","boundary":[0,0,15,8],"size":3}]}"#).unwrap();
        let mut id = 0;
        assert_eq!(
            unsafe { ofd_add_mixed_annotation_json(handle, 0, request.as_ptr(), &mut id) },
            0
        );
        let annotation_id = id.to_string();
        let object_id = unsafe { document_ref(handle).unwrap() }
            .annotation_objects(0, &annotation_id)
            .unwrap()[0]
            .id
            .clone()
            .unwrap();
        let set = CString::new(
            serde_json::json!({
                "operation": "set", "page_index": 0, "annotation_id": annotation_id,
                "object_id": object_id, "code_index": 0, "text": "A\0B",
            })
            .to_string(),
        )
        .unwrap();
        assert_eq!(
            unsafe { ofd_edit_annotation_text_code_json(handle, set.as_ptr()) },
            0
        );
        let append = CString::new(
            serde_json::json!({
                "operation": "append", "page_index": 0, "annotation_id": annotation_id,
                "object_id": object_id, "text": "C\0D", "layout": {},
            })
            .to_string(),
        )
        .unwrap();
        assert_eq!(
            unsafe { ofd_edit_annotation_text_code_json(handle, append.as_ptr()) },
            0
        );
        let edited = unsafe { document_ref(handle).unwrap() }
            .annotation_objects(0, &annotation_id)
            .unwrap();
        assert_eq!(edited[0].text, ["A\0B", "C\0D"]);
        let before = unsafe { document_ref(handle).unwrap() }.to_bytes().unwrap();
        let invalid = CString::new(
            serde_json::json!({
                "operation": "append", "page_index": 0, "annotation_id": annotation_id,
                "object_id": object_id, "text": "XY", "layout": {"delta_x": [1, 2]},
            })
            .to_string(),
        )
        .unwrap();
        assert_ne!(
            unsafe { ofd_edit_annotation_text_code_json(handle, invalid.as_ptr()) },
            0
        );
        assert_eq!(
            unsafe { document_ref(handle).unwrap() }.to_bytes().unwrap(),
            before
        );
        unsafe { ofd_close(handle) };
    }

    #[test]
    fn ffi_updates_annotation_composite_resource_atomically() {
        let mut handle = ptr::null_mut();
        assert_eq!(unsafe { ofd_new(&mut handle) }, 0);
        assert_eq!(unsafe { ofd_add_blank_page(handle, 40.0, 30.0) }, 0);
        for _ in 0..2 {
            unsafe { document_mut(handle).unwrap() }
                .add_composite_path_object(
                    0,
                    "M 0 0 L 8 0 L 8 8 C",
                    [0.0, 0.0, 8.0, 8.0],
                    PathStyle::default(),
                )
                .unwrap();
        }
        let resources = unsafe { document_ref(handle).unwrap() }
            .page_composite_objects(0)
            .unwrap();
        let first = resources[0].resource_id.clone();
        let second = resources[1].resource_id.clone();
        let payload = CString::new(format!(
            r#"{{"boundary":[2,2,20,12],"objects":[{{"kind":"composite","resource_id":"{first}","boundary":[0,0,8,8]}}]}}"#,
        ))
        .unwrap();
        let mut id = 0;
        assert_eq!(
            unsafe { ofd_add_mixed_annotation_json(handle, 0, payload.as_ptr(), &mut id) },
            0
        );
        let annotation_id = CString::new(id.to_string()).unwrap();
        let objects = unsafe { document_ref(handle).unwrap() }
            .annotation_objects(0, annotation_id.to_str().unwrap())
            .unwrap();
        let object_id = CString::new(objects[0].id.as_deref().unwrap()).unwrap();
        let second_id = CString::new(second.as_str()).unwrap();
        assert_eq!(
            unsafe {
                ofd_set_annotation_graphic_resource(
                    handle,
                    0,
                    annotation_id.as_ptr(),
                    object_id.as_ptr(),
                    second_id.as_ptr(),
                )
            },
            0
        );
        let edited = unsafe { document_ref(handle).unwrap() }
            .annotation_objects(0, annotation_id.to_str().unwrap())
            .unwrap();
        assert_eq!(edited[0].resource_id.as_deref(), Some(second.as_str()));
        let before = unsafe { document_ref(handle).unwrap() }.to_bytes().unwrap();
        let missing = CString::new("999999").unwrap();
        assert_ne!(
            unsafe {
                ofd_set_annotation_graphic_resource(
                    handle,
                    0,
                    annotation_id.as_ptr(),
                    object_id.as_ptr(),
                    missing.as_ptr(),
                )
            },
            0
        );
        assert_eq!(
            unsafe { document_ref(handle).unwrap() }.to_bytes().unwrap(),
            before
        );
        unsafe { ofd_close(handle) };
    }

    #[test]
    fn ffi_adds_highlight_annotation() {
        let mut handle = ptr::null_mut();
        assert_eq!(unsafe { ofd_new(&mut handle) }, 0);
        assert_eq!(unsafe { ofd_add_blank_page(handle, 30.0, 20.0) }, 0);
        let payload =
            CString::new(r#"{"boundary":[2,3,10,5],"color":[255,220,0],"opacity":96}"#).unwrap();
        let mut id = 0;
        assert_eq!(
            unsafe { ofd_add_highlight_annotation_json(handle, 0, payload.as_ptr(), &mut id) },
            0
        );
        assert!(id > 0);
        assert_eq!(
            unsafe { document_ref(handle).unwrap() }
                .annotations()
                .unwrap()[0]
                .annotation_type,
            "Highlight"
        );
        assert!(unsafe { document_ref(handle).unwrap() }
            .annotations()
            .unwrap()[0]
            .last_mod_date
            .is_some());
        let annotation_id = CString::new(id.to_string()).unwrap();
        let creator = CString::new("张三 & Team").unwrap();
        let subtype = CString::new("审阅 & <校对>").unwrap();
        let date = CString::new("2024-02-29").unwrap();
        let invalid_date = CString::new("2023-02-29").unwrap();
        let remark = CString::new("审核 & 通过").unwrap();
        assert_eq!(
            unsafe {
                ofd_set_annotation_creator(handle, 0, annotation_id.as_ptr(), creator.as_ptr())
            },
            0
        );
        assert_eq!(
            unsafe {
                ofd_set_annotation_subtype(handle, 0, annotation_id.as_ptr(), subtype.as_ptr())
            },
            0
        );
        assert_eq!(
            unsafe {
                ofd_set_annotation_last_mod_date(handle, 0, annotation_id.as_ptr(), date.as_ptr())
            },
            0
        );
        assert_ne!(
            unsafe {
                ofd_set_annotation_last_mod_date(
                    handle,
                    0,
                    annotation_id.as_ptr(),
                    invalid_date.as_ptr(),
                )
            },
            0
        );
        assert_eq!(
            unsafe {
                ofd_set_annotation_appearance_boundary(
                    handle,
                    0,
                    annotation_id.as_ptr(),
                    12.0,
                    8.0,
                    10.0,
                    5.0,
                )
            },
            0
        );
        assert_ne!(
            unsafe {
                ofd_set_annotation_appearance_boundary(
                    handle,
                    0,
                    annotation_id.as_ptr(),
                    12.0,
                    8.0,
                    0.0,
                    5.0,
                )
            },
            0
        );
        assert_eq!(
            unsafe {
                ofd_set_annotation_remark(handle, 0, annotation_id.as_ptr(), remark.as_ptr())
            },
            0
        );
        let mut output = OfdBuffer {
            data: ptr::null_mut(),
            len: 0,
        };
        assert_eq!(unsafe { ofd_annotations_json(handle, &mut output) }, 0);
        let values: serde_json::Value =
            serde_json::from_slice(unsafe { std::slice::from_raw_parts(output.data, output.len) })
                .unwrap();
        assert_eq!(values[0]["attributes"]["Creator"], "张三 & Team");
        assert_eq!(values[0]["subtype"], "审阅 & <校对>");
        assert_eq!(values[0]["last_mod_date"], "2024-02-29");
        assert_eq!(
            values[0]["appearance_box"],
            serde_json::json!([12.0, 8.0, 10.0, 5.0])
        );
        assert_eq!(values[0]["remark"], "审核 & 通过");
        unsafe { ofd_buffer_free(output) };
        let key = CString::new("分类 & <键>").unwrap();
        let value = CString::new("甲 & <乙>").unwrap();
        assert_eq!(
            unsafe {
                ofd_set_annotation_parameter(
                    handle,
                    0,
                    annotation_id.as_ptr(),
                    key.as_ptr(),
                    value.as_ptr(),
                )
            },
            0
        );
        assert_eq!(
            unsafe { document_ref(handle).unwrap() }
                .annotations()
                .unwrap()[0]
                .parameters["分类 & <键>"],
            "甲 & <乙>"
        );
        assert_eq!(
            unsafe {
                ofd_remove_annotation_parameter(handle, 0, annotation_id.as_ptr(), key.as_ptr())
            },
            0
        );
        let annotations = unsafe { document_ref(handle).unwrap() }
            .annotations()
            .unwrap();
        let parameters = &annotations[0].parameters;
        assert!(!parameters.contains_key("分类 & <键>"));
        assert_eq!(parameters["PDFMarkupQuads"], "0 0 1 0 1 1 0 1");
        assert_eq!(
            unsafe { ofd_set_annotation_remark(handle, 0, annotation_id.as_ptr(), ptr::null()) },
            0
        );
        assert_eq!(
            unsafe { document_ref(handle).unwrap() }
                .annotations()
                .unwrap()[0]
                .remark,
            None
        );
        assert_eq!(
            unsafe { ofd_set_annotation_subtype(handle, 0, annotation_id.as_ptr(), ptr::null()) },
            0
        );
        assert_eq!(
            unsafe { document_ref(handle).unwrap() }
                .annotations()
                .unwrap()[0]
                .subtype,
            None
        );
        let invalid = CString::new(r#"{"boundary":[2,3,0,5],"color":[255,220,0]}"#).unwrap();
        assert_ne!(
            unsafe { ofd_add_highlight_annotation_json(handle, 0, invalid.as_ptr(), &mut id) },
            0
        );
        assert_eq!(id, 0);
        assert_eq!(
            unsafe { document_ref(handle).unwrap() }
                .annotations()
                .unwrap()
                .len(),
            1
        );
        unsafe { ofd_close(handle) };
    }

    #[test]
    fn ffi_adds_shape_annotation_and_rejects_bad_style_atomically() {
        let mut handle = ptr::null_mut();
        assert_eq!(unsafe { ofd_new(&mut handle) }, 0);
        assert_eq!(unsafe { ofd_add_blank_page(handle, 30.0, 20.0) }, 0);
        let payload = CString::new(
            r#"{"kind":"Circle","boundary":[2,3,10,5],"stroke":false,"fill_color":[255,0,0]}"#,
        )
        .unwrap();
        let mut id = 0;
        assert_eq!(
            unsafe { ofd_add_shape_annotation_json(handle, 0, payload.as_ptr(), &mut id) },
            0
        );
        assert!(id > 0);
        let annotations = unsafe { document_ref(handle).unwrap() }
            .annotations()
            .unwrap();
        assert_eq!(annotations[0].annotation_type, "Path");
        assert_eq!(annotations[0].subtype.as_deref(), Some("Circle"));
        assert!(!annotations[0].parameters.contains_key("PDFShapeStroke"));
        let before = unsafe { document_ref(handle).unwrap() }.to_bytes().unwrap();
        let invalid =
            CString::new(r#"{"kind":"Circle","boundary":[2,3,10,5],"dash_sequence":[0]}"#).unwrap();
        assert_ne!(
            unsafe { ofd_add_shape_annotation_json(handle, 0, invalid.as_ptr(), &mut id) },
            0
        );
        assert_eq!(id, 0);
        assert_eq!(
            unsafe { document_ref(handle).unwrap() }.to_bytes().unwrap(),
            before
        );
        unsafe { ofd_close(handle) };
    }

    #[test]
    fn ffi_adds_line_annotation_and_rejects_bad_endpoints_atomically() {
        let mut handle = ptr::null_mut();
        assert_eq!(unsafe { ofd_new(&mut handle) }, 0);
        assert_eq!(unsafe { ofd_add_blank_page(handle, 30.0, 20.0) }, 0);
        let payload = CString::new(r#"{"boundary":[2,3,10,5],"endpoints":[0,5,10,0],"stroke_color":[255,0,0],"line_width":0.5}"#).unwrap();
        let mut id = 0;
        assert_eq!(
            unsafe { ofd_add_line_annotation_json(handle, 0, payload.as_ptr(), &mut id) },
            0
        );
        assert!(id > 0);
        let annotations = unsafe { document_ref(handle).unwrap() }
            .annotations()
            .unwrap();
        let line = &annotations[0];
        assert_eq!(line.subtype.as_deref(), Some("Line"));
        assert_eq!(line.parameters["PDFLine"], "0 1 1 0");
        assert!(unsafe { document_ref(handle).unwrap() }.to_pdf().is_ok());
        let before = unsafe { document_ref(handle).unwrap() }.to_bytes().unwrap();
        let invalid = CString::new(r#"{"boundary":[2,3,10,5],"endpoints":[0,5,11,0]}"#).unwrap();
        assert_ne!(
            unsafe { ofd_add_line_annotation_json(handle, 0, invalid.as_ptr(), &mut id) },
            0
        );
        assert_eq!(id, 0);
        assert_eq!(
            unsafe { document_ref(handle).unwrap() }.to_bytes().unwrap(),
            before
        );
        unsafe { ofd_close(handle) };
    }

    #[test]
    fn ffi_adds_text_note_annotation() {
        let mut handle = ptr::null_mut();
        assert_eq!(unsafe { ofd_new(&mut handle) }, 0);
        assert_eq!(unsafe { ofd_add_blank_page(handle, 30.0, 20.0) }, 0);
        let payload = CString::new(r#"{"text":"审核 & <通过>","boundary":[2,3,5,5],"color":[0,100,200],"opacity":128,"icon":"Comment","open":true}"#).unwrap();
        let mut id = 0;
        assert_eq!(
            unsafe { ofd_add_text_note_annotation_json(handle, 0, payload.as_ptr(), &mut id) },
            0
        );
        let annotations = unsafe { document_ref(handle).unwrap() }
            .annotations()
            .unwrap();
        assert_eq!(annotations[0].id, id.to_string());
        assert_eq!(annotations[0].annotation_type, "Path");
        assert_eq!(annotations[0].subtype.as_deref(), Some("Text"));
        assert_eq!(annotations[0].remark.as_deref(), Some("审核 & <通过>"));
        assert_eq!(annotations[0].parameters["PDFTextIcon"], "Comment");
        let before = unsafe { document_ref(handle).unwrap() }.to_bytes().unwrap();
        let invalid =
            CString::new(r#"{"text":"bad","boundary":[2,3,5,5],"icon":"Unknown"}"#).unwrap();
        assert_ne!(
            unsafe { ofd_add_text_note_annotation_json(handle, 0, invalid.as_ptr(), &mut id) },
            0
        );
        assert_eq!(id, 0);
        assert_eq!(
            unsafe { document_ref(handle).unwrap() }.to_bytes().unwrap(),
            before
        );
        unsafe { ofd_close(handle) };
    }

    #[test]
    fn ffi_adds_uri_link_annotation_and_exposes_action() {
        let mut handle = ptr::null_mut();
        assert_eq!(unsafe { ofd_new(&mut handle) }, 0);
        assert_eq!(unsafe { ofd_add_blank_page(handle, 30.0, 20.0) }, 0);
        let payload =
            CString::new(r#"{"uri":"https://example.com/?a=1&b=2","boundary":[2,3,10,5]}"#)
                .unwrap();
        let mut id = 0;
        assert_eq!(
            unsafe { ofd_add_uri_link_annotation_json(handle, 0, payload.as_ptr(), &mut id) },
            0
        );
        let mut output = OfdBuffer {
            data: ptr::null_mut(),
            len: 0,
        };
        assert_eq!(unsafe { ofd_annotations_json(handle, &mut output) }, 0);
        let values: serde_json::Value =
            serde_json::from_slice(unsafe { std::slice::from_raw_parts(output.data, output.len) })
                .unwrap();
        assert_eq!(values[0]["id"], id.to_string());
        assert_eq!(values[0]["type"], "Link");
        assert_eq!(values[0]["actions"][0]["kind"], "URI");
        assert_eq!(
            values[0]["actions"][0]["attributes"]["URI"],
            "https://example.com/?a=1&b=2"
        );
        unsafe { ofd_buffer_free(output) };
        let invalid = CString::new(r#"{"uri":"","boundary":[2,3,10,5]}"#).unwrap();
        assert_ne!(
            unsafe { ofd_add_uri_link_annotation_json(handle, 0, invalid.as_ptr(), &mut id) },
            0
        );
        assert_eq!(id, 0);
        unsafe { ofd_close(handle) };
    }

    #[test]
    fn ffi_adds_goto_and_bookmark_link_annotations() {
        let mut handle = ptr::null_mut();
        assert_eq!(unsafe { ofd_new(&mut handle) }, 0);
        assert_eq!(unsafe { ofd_add_blank_page(handle, 30.0, 20.0) }, 0);
        assert_eq!(unsafe { ofd_add_blank_page(handle, 30.0, 20.0) }, 0);
        let target = unsafe { document_ref(handle).unwrap() }.pages().unwrap()[1]
            .id
            .clone();
        let bookmark = CString::new(format!(
            r#"{{"name":"target","page_id":"{target}","kind":"Fit"}}"#
        ))
        .unwrap();
        assert_eq!(
            unsafe { ofd_add_bookmark_json(handle, 0, bookmark.as_ptr()) },
            0
        );
        let goto = CString::new(format!(r#"{{"destination":{{"page_id":"{target}","kind":"XYZ","left":4}},"boundary":[2,3,10,5]}}"#)).unwrap();
        let mut goto_id = 0;
        assert_eq!(
            unsafe { ofd_add_goto_link_annotation_json(handle, 0, goto.as_ptr(), &mut goto_id) },
            0
        );
        assert!(goto_id > 0);
        let named = CString::new(r#"{"bookmark_name":"target","boundary":[13,3,10,5]}"#).unwrap();
        let mut named_id = 0;
        assert_eq!(
            unsafe {
                ofd_add_bookmark_link_annotation_json(handle, 0, named.as_ptr(), &mut named_id)
            },
            0
        );
        assert_ne!(goto_id, named_id);
        let mut output = OfdBuffer {
            data: ptr::null_mut(),
            len: 0,
        };
        assert_eq!(unsafe { ofd_annotations_json(handle, &mut output) }, 0);
        let values: serde_json::Value =
            serde_json::from_slice(unsafe { std::slice::from_raw_parts(output.data, output.len) })
                .unwrap();
        assert_eq!(values[0]["actions"][0]["destination"]["page_id"], target);
        assert_eq!(values[1]["actions"][0]["bookmark"], "target");
        unsafe { ofd_buffer_free(output) };
        assert!(unsafe { document_ref(handle).unwrap() }
            .to_pdf()
            .unwrap()
            .windows(b"/Subtype /Link".len())
            .any(|window| window == b"/Subtype /Link"));
        let invalid = CString::new(r#"{"bookmark_name":"missing","boundary":[2,3,10,5]}"#).unwrap();
        assert_ne!(
            unsafe {
                ofd_add_bookmark_link_annotation_json(handle, 0, invalid.as_ptr(), &mut named_id)
            },
            0
        );
        assert_eq!(named_id, 0);
        unsafe { ofd_close(handle) };
    }

    #[test]
    fn ffi_writes_quadrilateral_link_regions() {
        let mut handle = ptr::null_mut();
        assert_eq!(unsafe { ofd_new(&mut handle) }, 0);
        assert_eq!(unsafe { ofd_add_blank_page(handle, 30.0, 20.0) }, 0);
        let payload = CString::new(r#"{"uri":"https://example.org","boundary":[2,3,10,5],"quad_points":[[[3,4],[6,4],[7,6],[4,6]]]}"#).unwrap();
        let mut id = 0;
        assert_eq!(
            unsafe { ofd_add_uri_link_annotation_json(handle, 0, payload.as_ptr(), &mut id) },
            0
        );
        let annotations = unsafe { document_ref(handle).unwrap() }
            .annotations()
            .unwrap();
        assert_eq!(
            annotations[0].actions[0].region.as_ref().unwrap().areas[0].start,
            [3.0, 4.0]
        );
        let invalid = CString::new(r#"{"uri":"https://example.org","boundary":[2,3,10,5],"quad_points":[[[3,4],[7,6],[6,4],[4,6]]]}"#).unwrap();
        let before = unsafe { document_ref(handle).unwrap() }.to_bytes().unwrap();
        assert_ne!(
            unsafe { ofd_add_uri_link_annotation_json(handle, 0, invalid.as_ptr(), &mut id) },
            0
        );
        assert_eq!(
            unsafe { document_ref(handle).unwrap() }.to_bytes().unwrap(),
            before
        );
        let pdf = unsafe { document_ref(handle).unwrap() }.to_pdf().unwrap();
        assert!(pdf
            .windows(b"/QuadPoints".len())
            .any(|window| window == b"/QuadPoints"));
        unsafe { ofd_close(handle) };
    }

    #[test]
    fn ffi_removes_annotation() {
        let mut handle = ptr::null_mut();
        assert_eq!(unsafe { ofd_new(&mut handle) }, 0);
        let text = CString::new("body").unwrap();
        assert_eq!(
            unsafe { ofd_add_text_page(handle, text.as_ptr(), 210.0, 297.0) },
            0
        );
        let payload = CString::new(r#"{"text":"mark","boundary":[10,20,40,8],"size":4}"#).unwrap();
        let mut id = 0;
        assert_eq!(
            unsafe { ofd_add_text_watermark_json(handle, 0, payload.as_ptr(), &mut id) },
            0
        );
        let name = CString::new(id.to_string()).unwrap();
        assert_ne!(
            unsafe { ofd_remove_annotation(handle, 1, name.as_ptr()) },
            0
        );
        assert_eq!(
            unsafe { ofd_remove_annotation(handle, 0, name.as_ptr()) },
            0
        );
        assert!(unsafe { document_ref(handle).unwrap() }
            .annotations()
            .unwrap()
            .is_empty());
        assert_ne!(
            unsafe { ofd_remove_annotation(handle, 0, name.as_ptr()) },
            0
        );
        unsafe { ofd_close(handle) };
    }

    #[test]
    fn ffi_adds_and_reads_page_object_uri_action() {
        let mut handle = ptr::null_mut();
        assert_eq!(unsafe { ofd_new(&mut handle) }, 0);
        let text = CString::new("link").unwrap();
        assert_eq!(
            unsafe { ofd_add_text_page(handle, text.as_ptr(), 100.0, 60.0) },
            0
        );
        let object_id = unsafe { document_ref(handle).unwrap() }
            .page_objects(0)
            .unwrap()[0]
            .id
            .clone()
            .unwrap();
        let id = CString::new(object_id).unwrap();
        let uri = CString::new("https://example.org/?a=1&b=2").unwrap();
        assert_eq!(
            unsafe { ofd_add_page_object_uri_action(handle, 0, id.as_ptr(), uri.as_ptr()) },
            0
        );
        let mut output = OfdBuffer {
            data: ptr::null_mut(),
            len: 0,
        };
        assert_eq!(
            unsafe { ofd_page_object_actions_json(handle, 0, id.as_ptr(), &mut output) },
            0
        );
        let values: serde_json::Value =
            serde_json::from_slice(unsafe { std::slice::from_raw_parts(output.data, output.len) })
                .unwrap();
        assert_eq!(values[0]["event"], "CLICK");
        assert_eq!(values[0]["kind"], "URI");
        assert_eq!(
            values[0]["attributes"]["URI"],
            "https://example.org/?a=1&b=2"
        );
        unsafe { ofd_buffer_free(output) };
        let mut output = OfdBuffer {
            data: ptr::null_mut(),
            len: 0,
        };
        assert_eq!(unsafe { ofd_to_pdf(handle, &mut output) }, 0);
        let pdf = unsafe { std::slice::from_raw_parts(output.data, output.len) };
        let imported = OfdDocument::from_pdf_bytes(pdf).unwrap();
        let annotations = imported.annotations().unwrap();
        assert_eq!(annotations.len(), 1);
        assert_eq!(annotations[0].annotation_type, "Link");
        assert_eq!(annotations[0].actions[0].kind, "URI");
        assert_eq!(
            annotations[0].actions[0].attributes["URI"],
            "https://example.org/?a=1&b=2"
        );
        unsafe { ofd_buffer_free(output) };
        assert_ne!(
            unsafe { ofd_add_page_object_uri_action(handle, 0, id.as_ptr(), ptr::null()) },
            0
        );
        unsafe { ofd_close(handle) };
    }

    #[test]
    fn ffi_adds_page_object_goto_action() {
        let mut handle = ptr::null_mut();
        assert_eq!(unsafe { ofd_new(&mut handle) }, 0);
        let text = CString::new("jump").unwrap();
        assert_eq!(
            unsafe { ofd_add_text_page(handle, text.as_ptr(), 100.0, 60.0) },
            0
        );
        assert_eq!(unsafe { ofd_add_blank_page(handle, 100.0, 60.0) }, 0);
        let object_id = unsafe { document_ref(handle).unwrap() }
            .page_objects(0)
            .unwrap()[0]
            .id
            .clone()
            .unwrap();
        let page_id = unsafe { document_ref(handle).unwrap() }.pages().unwrap()[1]
            .id
            .clone();
        let id = CString::new(object_id).unwrap();
        let payload = CString::new(format!(
            r#"{{"page_id":"{page_id}","kind":"XYZ","left":12.5,"top":21}}"#
        ))
        .unwrap();
        assert_eq!(
            unsafe {
                ofd_add_page_object_goto_action_json(handle, 0, id.as_ptr(), payload.as_ptr())
            },
            0
        );
        let actions = unsafe { document_ref(handle).unwrap() }
            .page_object_actions(0, id.to_str().unwrap())
            .unwrap();
        assert_eq!(actions[0].kind, "Goto");
        assert_eq!(actions[0].destination.as_ref().unwrap().left, Some(12.5));
        let invalid = CString::new("{}").unwrap();
        assert_ne!(
            unsafe {
                ofd_add_page_object_goto_action_json(handle, 0, id.as_ptr(), invalid.as_ptr())
            },
            0
        );
        unsafe { ofd_close(handle) };
    }

    #[test]
    fn ffi_sets_and_reads_page_object_action_regions() {
        let mut handle = ptr::null_mut();
        assert_eq!(unsafe { ofd_new(&mut handle) }, 0);
        let text = CString::new("link").unwrap();
        assert_eq!(
            unsafe { ofd_add_text_page(handle, text.as_ptr(), 100.0, 60.0) },
            0
        );
        let id = CString::new(
            unsafe { document_ref(handle).unwrap() }
                .page_objects(0)
                .unwrap()[0]
                .id
                .as_ref()
                .unwrap()
                .as_str(),
        )
        .unwrap();
        let uri = CString::new("https://example.org").unwrap();
        assert_eq!(
            unsafe { ofd_add_page_object_uri_action(handle, 0, id.as_ptr(), uri.as_ptr()) },
            0
        );
        let rectangles = CString::new("[[2,3,4,5],[10,12,3,2]]").unwrap();
        assert_eq!(
            unsafe {
                ofd_set_page_object_action_regions_json(
                    handle,
                    0,
                    id.as_ptr(),
                    0,
                    rectangles.as_ptr(),
                )
            },
            0
        );
        let mut output = OfdBuffer {
            data: ptr::null_mut(),
            len: 0,
        };
        assert_eq!(
            unsafe { ofd_page_object_actions_json(handle, 0, id.as_ptr(), &mut output) },
            0
        );
        let values: serde_json::Value =
            serde_json::from_slice(unsafe { std::slice::from_raw_parts(output.data, output.len) })
                .unwrap();
        assert_eq!(
            values[0]["region"]["areas"][0]["start"],
            serde_json::json!([2.0, 3.0])
        );
        assert_eq!(
            values[0]["region"]["areas"][1]["commands"][3]["kind"],
            "Close"
        );
        unsafe { ofd_buffer_free(output) };
        let invalid = CString::new("[[2,3,-1,5]]").unwrap();
        assert_ne!(
            unsafe {
                ofd_set_page_object_action_regions_json(handle, 0, id.as_ptr(), 0, invalid.as_ptr())
            },
            0
        );
        assert_eq!(
            unsafe { document_ref(handle).unwrap() }
                .page_object_actions(0, id.to_str().unwrap())
                .unwrap()[0]
                .region
                .as_ref()
                .unwrap()
                .areas
                .len(),
            2
        );
        let empty = CString::new("[]").unwrap();
        assert_eq!(
            unsafe {
                ofd_set_page_object_action_regions_json(handle, 0, id.as_ptr(), 0, empty.as_ptr())
            },
            0
        );
        assert!(unsafe { document_ref(handle).unwrap() }
            .page_object_actions(0, id.to_str().unwrap())
            .unwrap()[0]
            .region
            .is_none());
        unsafe { ofd_close(handle) };
    }

    #[test]
    fn ffi_adds_bookmark_action_and_removes_action() {
        let mut handle = ptr::null_mut();
        assert_eq!(unsafe { ofd_new(&mut handle) }, 0);
        let text = CString::new("click").unwrap();
        assert_eq!(
            unsafe { ofd_add_text_page(handle, text.as_ptr(), 100.0, 60.0) },
            0
        );
        let object_id = unsafe { document_ref(handle).unwrap() }
            .page_objects(0)
            .unwrap()[0]
            .id
            .clone()
            .unwrap();
        let page_id = unsafe { document_ref(handle).unwrap() }.pages().unwrap()[0]
            .id
            .clone();
        let id = CString::new(object_id).unwrap();
        let bookmark = CString::new("Section & One").unwrap();
        let payload = CString::new(format!(
            r#"{{"name":"Section & One","page_id":"{page_id}","kind":"XYZ"}}"#
        ))
        .unwrap();
        assert_eq!(
            unsafe { ofd_add_bookmark_json(handle, 0, payload.as_ptr()) },
            0
        );
        assert_eq!(
            unsafe {
                ofd_add_page_object_bookmark_action(handle, 0, id.as_ptr(), bookmark.as_ptr())
            },
            0
        );
        assert_eq!(
            unsafe { document_ref(handle).unwrap() }
                .page_object_actions(0, id.to_str().unwrap())
                .unwrap()[0]
                .bookmark
                .as_deref(),
            Some("Section & One")
        );
        assert_ne!(
            unsafe { ofd_remove_bookmark(handle, 0, bookmark.as_ptr()) },
            0
        );
        assert_ne!(
            unsafe { ofd_remove_page_object_action(handle, 0, id.as_ptr(), 1) },
            0
        );
        assert_eq!(
            unsafe { ofd_remove_page_object_action(handle, 0, id.as_ptr(), 0) },
            0
        );
        assert!(unsafe { document_ref(handle).unwrap() }
            .page_object_actions(0, id.to_str().unwrap())
            .unwrap()
            .is_empty());
        assert_eq!(
            unsafe { ofd_remove_bookmark(handle, 0, bookmark.as_ptr()) },
            0
        );
        unsafe { ofd_close(handle) };
    }

    #[test]
    fn ffi_adds_nested_outlines() {
        let mut handle = ptr::null_mut();
        assert_eq!(unsafe { ofd_new(&mut handle) }, 0);
        let first = CString::new("first").unwrap();
        assert_eq!(
            unsafe { ofd_add_text_page(handle, first.as_ptr(), 210.0, 297.0) },
            0
        );
        let root = CString::new(r#"{"title":"Root","destination":null}"#).unwrap();
        let mut index = usize::MAX;
        assert_eq!(
            unsafe { ofd_add_outline_json(handle, 0, root.as_ptr(), &mut index) },
            0
        );
        assert_eq!(index, 0);
        let child = CString::new(r#"{"title":"Child","parent_index":0,"expanded":false}"#).unwrap();
        assert_eq!(
            unsafe { ofd_add_outline_json(handle, 0, child.as_ptr(), &mut index) },
            0
        );
        assert_eq!(index, 1);
        let edited = CString::new("章节 & C").unwrap();
        assert_eq!(
            unsafe { ofd_set_outline_title(handle, 0, edited.as_ptr()) },
            0
        );
        assert_eq!(unsafe { ofd_set_outline_expanded(handle, 0, 0) }, 0);
        assert_ne!(unsafe { ofd_set_outline_expanded(handle, 0, 2) }, 0);
        assert_eq!(unsafe { ofd_add_blank_page(handle, 210.0, 297.0) }, 0);
        let second_page = unsafe { document_ref(handle).unwrap() }.pages().unwrap()[1]
            .id
            .clone();
        let destination =
            CString::new(format!(r#"{{"page_id":"{second_page}","kind":"Fit"}}"#)).unwrap();
        assert_eq!(
            unsafe { ofd_set_outline_destination_json(handle, 0, destination.as_ptr()) },
            0
        );
        let mut output = OfdBuffer {
            data: ptr::null_mut(),
            len: 0,
        };
        assert_eq!(unsafe { ofd_outlines_json(handle, &mut output) }, 0);
        let values: serde_json::Value =
            serde_json::from_slice(unsafe { std::slice::from_raw_parts(output.data, output.len) })
                .unwrap();
        assert_eq!(values[1]["parent_index"], 0);
        assert_eq!(values[1]["title"], "Child");
        assert_eq!(values[0]["title"], "章节 & C");
        assert_eq!(values[0]["expanded"], false);
        assert_eq!(values[0]["destination"]["page_id"], second_page);
        unsafe { ofd_buffer_free(output) };
        unsafe { document_mut(handle).unwrap() }
            .add_bookmark(
                0,
                "索引 & C",
                DestinationInfo {
                    page_id: second_page.clone(),
                    kind: "Fit".into(),
                    left: None,
                    top: None,
                    right: None,
                    bottom: None,
                    zoom: None,
                },
            )
            .unwrap();
        let bookmark = CString::new("索引 & C").unwrap();
        assert_eq!(
            unsafe { ofd_set_outline_bookmark(handle, 0, bookmark.as_ptr()) },
            0
        );
        assert_eq!(
            unsafe { document_ref(handle).unwrap() }.outlines().unwrap()[0]
                .bookmark_name
                .as_deref(),
            Some("索引 & C")
        );
        let bad = CString::new(r#"{"title":"bad","parent_index":-1}"#).unwrap();
        assert_ne!(
            unsafe { ofd_add_outline_json(handle, 0, bad.as_ptr(), &mut index) },
            0
        );
        assert_ne!(unsafe { ofd_remove_outline(handle, 2) }, 0);
        assert_eq!(unsafe { ofd_remove_outline(handle, 0) }, 0);
        let mut output = OfdBuffer {
            data: ptr::null_mut(),
            len: 0,
        };
        assert_eq!(unsafe { ofd_outlines_json(handle, &mut output) }, 0);
        assert_eq!(
            unsafe { std::slice::from_raw_parts(output.data, output.len) },
            b"[]"
        );
        unsafe { ofd_buffer_free(output) };
        unsafe { ofd_close(handle) };
    }

    #[test]
    fn ffi_imports_pdf_vector_pages_and_rejects_bad_input() {
        let pdf = include_bytes!("../../ofdrw/tests/fixtures/vector.pdf");
        let mut handle = ptr::null_mut();
        assert_eq!(
            unsafe { ofd_from_pdf_bytes(pdf.as_ptr(), pdf.len(), &mut handle) },
            0
        );
        let mut count = 0;
        assert_eq!(unsafe { ofd_page_count(handle, &mut count) }, 0);
        assert_eq!(count, 1);
        let mut output = OfdBuffer {
            data: ptr::null_mut(),
            len: 0,
        };
        assert_eq!(unsafe { ofd_page_paths_json(handle, 0, &mut output) }, 0);
        let paths: serde_json::Value =
            serde_json::from_slice(unsafe { std::slice::from_raw_parts(output.data, output.len) })
                .unwrap();
        assert_eq!(paths.as_array().unwrap().len(), 2);
        unsafe { ofd_buffer_free(output) };
        unsafe { ofd_close(handle) };
        let image_pdf = include_bytes!("../../ofdrw/tests/fixtures/image.pdf");
        handle = ptr::null_mut();
        assert_eq!(
            unsafe { ofd_from_pdf_bytes(image_pdf.as_ptr(), image_pdf.len(), &mut handle) },
            0
        );
        let mut images = OfdBuffer {
            data: ptr::null_mut(),
            len: 0,
        };
        assert_eq!(unsafe { ofd_page_images_json(handle, 0, &mut images) }, 0);
        let image_values: serde_json::Value =
            serde_json::from_slice(unsafe { std::slice::from_raw_parts(images.data, images.len) })
                .unwrap();
        assert_eq!(image_values.as_array().unwrap().len(), 1);
        unsafe {
            ofd_buffer_free(images);
            ofd_close(handle)
        };
        let text_pdf = include_bytes!("../../ofdrw/tests/fixtures/text.pdf");
        handle = ptr::null_mut();
        assert_eq!(
            unsafe { ofd_from_pdf_bytes(text_pdf.as_ptr(), text_pdf.len(), &mut handle) },
            0
        );
        let mut text = OfdBuffer {
            data: ptr::null_mut(),
            len: 0,
        };
        assert_eq!(unsafe { ofd_page_text_json(handle, 0, &mut text) }, 0);
        let text_values: serde_json::Value =
            serde_json::from_slice(unsafe { std::slice::from_raw_parts(text.data, text.len) })
                .unwrap();
        assert_eq!(text_values[0], "Hello PDF");
        unsafe {
            ofd_buffer_free(text);
            ofd_close(handle)
        };
        let spaced_pdf = include_bytes!("../../ofdrw/tests/fixtures/spacing.pdf");
        handle = ptr::null_mut();
        assert_eq!(
            unsafe { ofd_from_pdf_bytes(spaced_pdf.as_ptr(), spaced_pdf.len(), &mut handle) },
            0
        );
        let mut spaced_text = OfdBuffer {
            data: ptr::null_mut(),
            len: 0,
        };
        assert_eq!(
            unsafe { ofd_page_text_json(handle, 0, &mut spaced_text) },
            0
        );
        let spaced_values: serde_json::Value = serde_json::from_slice(unsafe {
            std::slice::from_raw_parts(spaced_text.data, spaced_text.len)
        })
        .unwrap();
        assert_eq!(spaced_values, serde_json::json!(["AB", "CD"]));
        unsafe {
            ofd_buffer_free(spaced_text);
            ofd_close(handle)
        };
        let scaled_pdf = include_bytes!("../../ofdrw/tests/fixtures/horizontal-scale.pdf");
        handle = ptr::null_mut();
        assert_eq!(
            unsafe { ofd_from_pdf_bytes(scaled_pdf.as_ptr(), scaled_pdf.len(), &mut handle) },
            0
        );
        let mut scaled_text = OfdBuffer {
            data: ptr::null_mut(),
            len: 0,
        };
        assert_eq!(
            unsafe { ofd_page_text_objects_json(handle, 0, &mut scaled_text) },
            0
        );
        let scaled_values: serde_json::Value = serde_json::from_slice(unsafe {
            std::slice::from_raw_parts(scaled_text.data, scaled_text.len)
        })
        .unwrap();
        assert_eq!(scaled_values[0]["h_scale"], 0.5);
        assert_eq!(scaled_values[2]["h_scale"], 1.5);
        unsafe {
            ofd_buffer_free(scaled_text);
            ofd_close(handle)
        };
        let appearance_pdf = include_bytes!("../../ofdrw/tests/fixtures/text-appearance.pdf");
        handle = ptr::null_mut();
        assert_eq!(
            unsafe {
                ofd_from_pdf_bytes(appearance_pdf.as_ptr(), appearance_pdf.len(), &mut handle)
            },
            0
        );
        let mut appearance_text = OfdBuffer {
            data: ptr::null_mut(),
            len: 0,
        };
        assert_eq!(
            unsafe { ofd_page_text_json(handle, 0, &mut appearance_text) },
            0
        );
        let appearance_values: serde_json::Value = serde_json::from_slice(unsafe {
            std::slice::from_raw_parts(appearance_text.data, appearance_text.len)
        })
        .unwrap();
        assert_eq!(
            appearance_values,
            serde_json::json!(["A", "B", "C", "D", "E"])
        );
        unsafe {
            ofd_buffer_free(appearance_text);
            ofd_close(handle)
        };
        handle = ptr::null_mut();
        assert_ne!(
            unsafe { ofd_from_pdf_bytes(ptr::null(), 1, &mut handle) },
            0
        );
        assert!(handle.is_null());
        assert_ne!(
            unsafe { ofd_from_pdf_bytes(b"bad".as_ptr(), 3, &mut handle) },
            0
        );
        assert!(handle.is_null());
        assert_ne!(
            unsafe { ofd_from_pdf_bytes(pdf.as_ptr(), pdf.len(), ptr::null_mut()) },
            0
        );
    }

    #[test]
    fn ffi_imports_pdf_link_annotations() {
        let pdf = include_bytes!("../../ofdrw/tests/fixtures/links.pdf");
        let mut handle = ptr::null_mut();
        assert_eq!(
            unsafe { ofd_from_pdf_bytes(pdf.as_ptr(), pdf.len(), &mut handle) },
            0
        );
        let mut output = OfdBuffer {
            data: ptr::null_mut(),
            len: 0,
        };
        assert_eq!(unsafe { ofd_annotations_json(handle, &mut output) }, 0);
        let annotations: serde_json::Value =
            serde_json::from_slice(unsafe { std::slice::from_raw_parts(output.data, output.len) })
                .unwrap();
        assert_eq!(annotations.as_array().unwrap().len(), 2);
        assert_eq!(
            annotations[0]["actions"][0]["attributes"]["URI"],
            "https://example.com/?a=1&b=2"
        );
        assert_eq!(annotations[1]["actions"][0]["destination"]["kind"], "Fit");
        unsafe { ofd_buffer_free(output) };
        unsafe { ofd_close(handle) };
    }

    #[test]
    fn ffi_imports_pdf_selected_color_spaces() {
        let pdf = include_bytes!("../../ofdrw/tests/fixtures/color-spaces.pdf");
        let mut handle = ptr::null_mut();
        assert_eq!(
            unsafe { ofd_from_pdf_bytes(pdf.as_ptr(), pdf.len(), &mut handle) },
            0
        );
        let mut output = OfdBuffer {
            data: ptr::null_mut(),
            len: 0,
        };
        assert_eq!(unsafe { ofd_page_paths_json(handle, 0, &mut output) }, 0);
        let paths: serde_json::Value =
            serde_json::from_slice(unsafe { std::slice::from_raw_parts(output.data, output.len) })
                .unwrap();
        assert_eq!(paths.as_array().unwrap().len(), 4);
        unsafe { ofd_buffer_free(output) };
        unsafe { ofd_close(handle) };
    }

    #[test]
    fn ffi_imports_pdf_device_cmyk() {
        let pdf = include_bytes!("../../ofdrw/tests/fixtures/cmyk.pdf");
        let mut handle = ptr::null_mut();
        assert_eq!(
            unsafe { ofd_from_pdf_bytes(pdf.as_ptr(), pdf.len(), &mut handle) },
            0
        );
        let mut output = OfdBuffer {
            data: ptr::null_mut(),
            len: 0,
        };
        assert_eq!(unsafe { ofd_page_images_json(handle, 0, &mut output) }, 0);
        let images: serde_json::Value =
            serde_json::from_slice(unsafe { std::slice::from_raw_parts(output.data, output.len) })
                .unwrap();
        assert_eq!(images.as_array().unwrap().len(), 1);
        unsafe { ofd_buffer_free(output) };
        unsafe { ofd_close(handle) };
    }

    #[test]
    fn ffi_imports_pdf_cmyk_icc_jpeg() {
        for pdf in [
            include_bytes!("../../ofdrw/tests/fixtures/cmyk-icc-jpeg.pdf").as_slice(),
            include_bytes!("../../ofdrw/tests/fixtures/ycck-icc-jpeg.pdf").as_slice(),
        ] {
            let mut handle = ptr::null_mut();
            assert_eq!(
                unsafe { ofd_from_pdf_bytes(pdf.as_ptr(), pdf.len(), &mut handle) },
                0
            );
            let mut output = OfdBuffer {
                data: ptr::null_mut(),
                len: 0,
            };
            assert_eq!(unsafe { ofd_page_image_data(handle, 0, 0, &mut output) }, 0);
            let bytes = unsafe { std::slice::from_raw_parts(output.data, output.len) };
            assert!(bytes.starts_with(b"\x89PNG\r\n\x1a\n"));
            unsafe { ofd_buffer_free(output) };
            unsafe { ofd_close(handle) };
        }
    }

    #[test]
    fn ffi_imports_nested_pdf_forms() {
        let pdf = include_bytes!("../../ofdrw/tests/fixtures/forms.pdf");
        let mut handle = ptr::null_mut();
        assert_eq!(
            unsafe { ofd_from_pdf_bytes(pdf.as_ptr(), pdf.len(), &mut handle) },
            0
        );
        let mut output = OfdBuffer {
            data: ptr::null_mut(),
            len: 0,
        };
        assert_eq!(unsafe { ofd_page_paths_json(handle, 0, &mut output) }, 0);
        let paths: serde_json::Value =
            serde_json::from_slice(unsafe { std::slice::from_raw_parts(output.data, output.len) })
                .unwrap();
        assert_eq!(paths.as_array().unwrap().len(), 2);
        unsafe { ofd_buffer_free(output) };
        unsafe { ofd_close(handle) };
    }

    #[test]
    fn ffi_imports_named_pdf_link_destinations() {
        let pdf = include_bytes!("../../ofdrw/tests/fixtures/named-links.pdf");
        let mut handle = ptr::null_mut();
        assert_eq!(
            unsafe { ofd_from_pdf_bytes(pdf.as_ptr(), pdf.len(), &mut handle) },
            0
        );
        let mut output = OfdBuffer {
            data: ptr::null_mut(),
            len: 0,
        };
        assert_eq!(unsafe { ofd_annotations_json(handle, &mut output) }, 0);
        let annotations: serde_json::Value =
            serde_json::from_slice(unsafe { std::slice::from_raw_parts(output.data, output.len) })
                .unwrap();
        assert_eq!(annotations[0]["actions"][0]["destination"]["kind"], "XYZ");
        assert_eq!(annotations[0]["actions"][0]["destination"]["zoom"], 1.25);
        assert_eq!(annotations[1]["actions"][0]["destination"]["kind"], "Fit");
        unsafe { ofd_buffer_free(output) };
        unsafe { ofd_close(handle) };
    }

    #[test]
    fn ffi_imports_pdf_link_quadrilaterals() {
        let pdf = include_bytes!("../../ofdrw/tests/fixtures/quad-links.pdf");
        let mut handle = ptr::null_mut();
        assert_eq!(
            unsafe { ofd_from_pdf_bytes(pdf.as_ptr(), pdf.len(), &mut handle) },
            0
        );
        let mut output = OfdBuffer {
            data: ptr::null_mut(),
            len: 0,
        };
        assert_eq!(unsafe { ofd_annotations_json(handle, &mut output) }, 0);
        let annotations: serde_json::Value =
            serde_json::from_slice(unsafe { std::slice::from_raw_parts(output.data, output.len) })
                .unwrap();
        assert_eq!(
            annotations[0]["actions"][0]["region"]["areas"]
                .as_array()
                .unwrap()
                .len(),
            2
        );
        assert_eq!(
            annotations[0]["actions"][0]["region"]["areas"][0]["commands"][3]["kind"],
            "Close"
        );
        unsafe { ofd_buffer_free(output) };
        unsafe { ofd_close(handle) };
    }

    #[test]
    fn ffi_imports_pdf_user_unit_geometry() {
        let pdf = include_bytes!("../../ofdrw/tests/fixtures/user-unit.pdf");
        let mut handle = ptr::null_mut();
        assert_eq!(
            unsafe { ofd_from_pdf_bytes(pdf.as_ptr(), pdf.len(), &mut handle) },
            0
        );
        let mut output = OfdBuffer {
            data: ptr::null_mut(),
            len: 0,
        };
        assert_eq!(unsafe { ofd_pages_json(handle, &mut output) }, 0);
        let pages: serde_json::Value =
            serde_json::from_slice(unsafe { std::slice::from_raw_parts(output.data, output.len) })
                .unwrap();
        assert_eq!(
            pages[0]["physical_box"],
            serde_json::json!([0.0, 0.0, 50.8, 50.8])
        );
        unsafe { ofd_buffer_free(output) };
        assert_eq!(
            unsafe { document_ref(handle).unwrap() }
                .page_text(0)
                .unwrap(),
            ["Hi"]
        );
        assert_eq!(
            unsafe { document_ref(handle).unwrap() }
                .page_images(0)
                .unwrap()
                .len(),
            1
        );
        assert_eq!(
            unsafe { document_ref(handle).unwrap() }
                .page_paths(0)
                .unwrap()
                .len(),
            2
        );
        unsafe { ofd_close(handle) };
    }

    #[test]
    fn ffi_imports_pdf_rotated_page_geometry() {
        let pdf = include_bytes!("../../ofdrw/tests/fixtures/rotate-90.pdf");
        let mut handle = ptr::null_mut();
        assert_eq!(
            unsafe { ofd_from_pdf_bytes(pdf.as_ptr(), pdf.len(), &mut handle) },
            0
        );
        let pages = unsafe { document_ref(handle).unwrap() }.pages().unwrap();
        let physical = pages[0].physical_box.unwrap();
        assert!((physical[2] - 76.2).abs() < 1e-8);
        assert!((physical[3] - 50.8).abs() < 1e-8);
        assert_eq!(
            unsafe { document_ref(handle).unwrap() }
                .page_text(0)
                .unwrap(),
            ["Hi"]
        );
        assert_eq!(
            unsafe { document_ref(handle).unwrap() }
                .page_paths(0)
                .unwrap()
                .len(),
            2
        );
        assert_eq!(
            unsafe { document_ref(handle).unwrap() }
                .page_images(0)
                .unwrap()
                .len(),
            1
        );
        unsafe { ofd_close(handle) };
    }

    #[test]
    fn ffi_imports_pdf_affine_image() {
        let pdf = include_bytes!("../../ofdrw/tests/fixtures/affine-image.pdf");
        let mut handle = ptr::null_mut();
        assert_eq!(
            unsafe { ofd_from_pdf_bytes(pdf.as_ptr(), pdf.len(), &mut handle) },
            0
        );
        let images = unsafe { document_ref(handle).unwrap() }
            .page_images(0)
            .unwrap();
        assert_eq!(images.len(), 1);
        assert!(images[0].ctm.is_some());
        unsafe { ofd_close(handle) };
    }

    #[test]
    fn ffi_imports_pdf_packed_gray_image() {
        let pdf = include_bytes!("../../ofdrw/tests/fixtures/packed-gray.pdf");
        let mut handle = ptr::null_mut();
        assert_eq!(
            unsafe { ofd_from_pdf_bytes(pdf.as_ptr(), pdf.len(), &mut handle) },
            0
        );
        assert_eq!(
            unsafe { document_ref(handle).unwrap() }
                .page_images(0)
                .unwrap()
                .len(),
            1
        );
        assert!(unsafe { document_ref(handle).unwrap() }
            .validate_references()
            .unwrap()
            .is_empty());
        unsafe { ofd_close(handle) };
    }

    #[test]
    fn ffi_imports_sixteen_bit_pdf_image() {
        let pdf = include_bytes!("../../ofdrw/tests/fixtures/gray-16-bit.pdf");
        let mut handle = ptr::null_mut();
        assert_eq!(
            unsafe { ofd_from_pdf_bytes(pdf.as_ptr(), pdf.len(), &mut handle) },
            0
        );
        let mut output = OfdBuffer {
            data: ptr::null_mut(),
            len: 0,
        };
        assert_eq!(unsafe { ofd_page_image_data(handle, 0, 0, &mut output) }, 0);
        let bytes = unsafe { std::slice::from_raw_parts(output.data, output.len) };
        assert!(bytes.starts_with(b"\x89PNG\r\n\x1a\n"));
        unsafe { ofd_buffer_free(output) };
        assert!(unsafe { document_ref(handle).unwrap() }
            .validate_references()
            .unwrap()
            .is_empty());
        unsafe { ofd_close(handle) };
    }

    #[test]
    fn ffi_creates_layers_and_moves_objects() {
        let mut handle = ptr::null_mut();
        assert_eq!(unsafe { ofd_new(&mut handle) }, 0);
        assert_eq!(unsafe { ofd_add_blank_page(handle, 20.0, 20.0) }, 0);
        let foreground = CString::new("Foreground").unwrap();
        let mut layer_id = 0;
        assert_eq!(
            unsafe {
                ofd_add_page_layer(handle, 0, foreground.as_ptr(), ptr::null(), &mut layer_id)
            },
            0
        );
        assert!(layer_id > 0);
        let payload = CString::new(r#"{"commands":"M 0 0 L 20 0 L 20 20 L 0 20 C","boundary":[0,0,20,20],"fill":true,"stroke":false,"fill_color":[255,0,0]}"#).unwrap();
        let mut object_id = 0;
        assert_eq!(
            unsafe { ofd_add_path_object_json(handle, 0, payload.as_ptr(), &mut object_id) },
            0
        );
        let object = CString::new(object_id.to_string()).unwrap();
        let layer = CString::new(layer_id.to_string()).unwrap();
        assert_eq!(
            unsafe { ofd_move_page_object_to_layer(handle, 0, object.as_ptr(), layer.as_ptr()) },
            0
        );
        let mut output = OfdBuffer {
            data: ptr::null_mut(),
            len: 0,
        };
        assert_eq!(unsafe { ofd_page_layers_json(handle, 0, &mut output) }, 0);
        let layers: serde_json::Value =
            serde_json::from_slice(unsafe { std::slice::from_raw_parts(output.data, output.len) })
                .unwrap();
        assert_eq!(layers[1]["id"], layer_id.to_string());
        assert_eq!(layers[1]["object_count"], 1);
        unsafe { ofd_buffer_free(output) };
        assert_ne!(
            unsafe { ofd_remove_page_layer(handle, 0, layer.as_ptr()) },
            0
        );
        let background = CString::new("Background").unwrap();
        let mut empty_id = 0;
        assert_eq!(
            unsafe {
                ofd_add_page_layer(handle, 0, background.as_ptr(), ptr::null(), &mut empty_id)
            },
            0
        );
        let empty = CString::new(empty_id.to_string()).unwrap();
        assert_eq!(
            unsafe { ofd_remove_page_layer(handle, 0, empty.as_ptr()) },
            0
        );
        assert_ne!(
            unsafe { ofd_remove_page_layer(handle, 0, empty.as_ptr()) },
            0
        );
        assert_ne!(unsafe { ofd_remove_page_layer(handle, 0, ptr::null()) }, 0);
        assert_ne!(
            unsafe { ofd_add_page_layer(handle, 0, ptr::null(), ptr::null(), &mut layer_id) },
            0
        );
        assert_ne!(
            unsafe { ofd_move_page_object_to_layer(handle, 0, object.as_ptr(), ptr::null()) },
            0
        );
        unsafe { ofd_close(handle) };
    }

    #[test]
    fn ffi_edits_page_layer_type_and_draw_param() {
        let mut handle = ptr::null_mut();
        assert_eq!(unsafe { ofd_new(&mut handle) }, 0);
        assert_eq!(unsafe { ofd_add_blank_page(handle, 20.0, 20.0) }, 0);
        let foreground = CString::new("Foreground").unwrap();
        let mut layer_id = 0;
        assert_eq!(
            unsafe {
                ofd_add_page_layer(handle, 0, foreground.as_ptr(), ptr::null(), &mut layer_id)
            },
            0
        );
        let param = unsafe { document_mut(handle).unwrap() }
            .add_document_draw_param(0, &ofdrw::DrawParamStyle::default())
            .unwrap();
        let layer = CString::new(layer_id.to_string()).unwrap();
        let reference = CString::new(param).unwrap();
        let background = CString::new("Background").unwrap();
        assert_eq!(
            unsafe { ofd_set_page_layer_draw_param(handle, 0, layer.as_ptr(), reference.as_ptr()) },
            0
        );
        assert_eq!(
            unsafe { ofd_set_page_layer_type(handle, 0, layer.as_ptr(), background.as_ptr()) },
            0
        );
        let name = CString::new("图层 & A").unwrap();
        assert_eq!(
            unsafe { ofd_set_page_layer_name(handle, 0, layer.as_ptr(), name.as_ptr()) },
            0
        );
        let layers = unsafe { document_ref(handle).unwrap() }
            .page_layers(0)
            .unwrap();
        assert_eq!(layers[0].id.as_deref(), Some(layer.to_str().unwrap()));
        assert_eq!(layers[0].layer_type, "Background");
        assert_eq!(layers[0].name.as_deref(), Some("图层 & A"));
        assert_eq!(
            layers[0].draw_param.as_deref(),
            Some(reference.to_str().unwrap())
        );
        assert_ne!(
            unsafe { ofd_set_page_layer_type(handle, 0, layer.as_ptr(), ptr::null()) },
            0
        );
        assert_eq!(
            unsafe { ofd_set_page_layer_draw_param(handle, 0, layer.as_ptr(), ptr::null()) },
            0
        );
        assert_eq!(
            unsafe { ofd_set_page_layer_name(handle, 0, layer.as_ptr(), ptr::null()) },
            0
        );
        assert_eq!(
            unsafe { document_ref(handle).unwrap() }
                .page_layers(0)
                .unwrap()[0]
                .draw_param,
            None
        );
        assert_eq!(
            unsafe { document_ref(handle).unwrap() }
                .page_layers(0)
                .unwrap()[0]
                .name,
            None
        );
        unsafe { ofd_close(handle) };
    }

    #[test]
    fn ffi_reorders_page_objects() {
        let mut handle = ptr::null_mut();
        assert_eq!(unsafe { ofd_new(&mut handle) }, 0);
        assert_eq!(unsafe { ofd_add_blank_page(handle, 20.0, 20.0) }, 0);
        let payload =
            CString::new(r#"{"commands":"M 0 0 L 20 20","boundary":[0,0,20,20]}"#).unwrap();
        let mut first = 0;
        let mut second = 0;
        assert_eq!(
            unsafe { ofd_add_path_object_json(handle, 0, payload.as_ptr(), &mut first) },
            0
        );
        assert_eq!(
            unsafe { ofd_add_path_object_json(handle, 0, payload.as_ptr(), &mut second) },
            0
        );
        let first_id = CString::new(first.to_string()).unwrap();
        let second_id = CString::new(second.to_string()).unwrap();
        assert_eq!(
            unsafe { ofd_move_page_object(handle, 0, second_id.as_ptr(), first_id.as_ptr()) },
            0
        );
        let paths = unsafe { document_ref(handle).unwrap() }
            .page_paths(0)
            .unwrap();
        assert_eq!(paths[0].id.as_deref(), Some(second_id.to_str().unwrap()));
        assert_eq!(
            unsafe { ofd_move_page_object(handle, 0, second_id.as_ptr(), ptr::null()) },
            0
        );
        let paths = unsafe { document_ref(handle).unwrap() }
            .page_paths(0)
            .unwrap();
        assert_eq!(paths[1].id.as_deref(), Some(second_id.to_str().unwrap()));
        assert_eq!(
            unsafe { ofd_group_page_objects(handle, 0, first_id.as_ptr(), second_id.as_ptr()) },
            0
        );
        assert_eq!(
            unsafe { ofd_group_page_objects(handle, 0, first_id.as_ptr(), first_id.as_ptr()) },
            0
        );
        let page = unsafe { document_ref(handle).unwrap() }.pages().unwrap()[0]
            .path
            .clone();
        let xml = std::str::from_utf8(
            unsafe { document_ref(handle).unwrap() }
                .entry(&page)
                .unwrap()
                .unwrap(),
        )
        .unwrap();
        assert_eq!(xml.matches("<ofd:PageBlock>").count(), 2);
        let missing = CString::new("missing").unwrap();
        assert_ne!(
            unsafe { ofd_group_page_objects(handle, 0, first_id.as_ptr(), missing.as_ptr()) },
            0
        );
        assert_eq!(
            unsafe { ofd_ungroup_page_objects(handle, 0, first_id.as_ptr()) },
            0
        );
        assert_eq!(
            unsafe { ofd_ungroup_page_objects(handle, 0, second_id.as_ptr()) },
            0
        );
        let xml = std::str::from_utf8(
            unsafe { document_ref(handle).unwrap() }
                .entry(&page)
                .unwrap()
                .unwrap(),
        )
        .unwrap();
        assert!(!xml.contains("PageBlock"));
        assert_ne!(
            unsafe { ofd_ungroup_page_objects(handle, 0, first_id.as_ptr()) },
            0
        );
        assert_ne!(
            unsafe { ofd_move_page_object(handle, 0, first_id.as_ptr(), missing.as_ptr()) },
            0
        );
        assert_ne!(
            unsafe { ofd_move_page_object(handle, 0, ptr::null(), ptr::null()) },
            0
        );
        unsafe { ofd_close(handle) };
    }

    #[test]
    fn ffi_sets_and_clears_page_object_name() {
        let mut handle = ptr::null_mut();
        assert_eq!(unsafe { ofd_new(&mut handle) }, 0);
        let text = CString::new("named").unwrap();
        assert_eq!(
            unsafe { ofd_add_text_page(handle, text.as_ptr(), 40.0, 30.0) },
            0
        );
        let id = unsafe { document_ref(handle).unwrap() }
            .page_objects(0)
            .unwrap()[0]
            .id
            .clone()
            .unwrap();
        let id = CString::new(id).unwrap();
        let name = CString::new("发票 & \"A\"").unwrap();
        assert_eq!(
            unsafe { ofd_set_page_object_name(handle, 0, id.as_ptr(), name.as_ptr()) },
            0
        );
        let mut output = OfdBuffer {
            data: ptr::null_mut(),
            len: 0,
        };
        assert_eq!(unsafe { ofd_page_objects_json(handle, 0, &mut output) }, 0);
        let bytes = unsafe { std::slice::from_raw_parts(output.data, output.len) };
        let value: serde_json::Value = serde_json::from_slice(bytes).unwrap();
        assert_eq!(value[0]["name"], "发票 & \"A\"");
        unsafe { ofd_buffer_free(output) };
        assert_eq!(
            unsafe { ofd_set_page_object_name(handle, 0, id.as_ptr(), ptr::null()) },
            0
        );
        assert_eq!(
            unsafe { document_ref(handle).unwrap() }
                .page_objects(0)
                .unwrap()[0]
                .name,
            None
        );
        unsafe { ofd_close(handle) };
    }

    #[test]
    fn ffi_sets_and_clears_page_object_blend_mode() {
        let mut handle = ptr::null_mut();
        assert_eq!(unsafe { ofd_new(&mut handle) }, 0);
        assert_eq!(unsafe { ofd_add_blank_page(handle, 20.0, 20.0) }, 0);
        let payload =
            CString::new(r#"{"commands":"M 0 0 L 20 20","boundary":[0,0,20,20]}"#).unwrap();
        let mut object_id = 0;
        assert_eq!(
            unsafe { ofd_add_path_object_json(handle, 0, payload.as_ptr(), &mut object_id) },
            0
        );
        let id = CString::new(object_id.to_string()).unwrap();
        let mode = CString::new("darken").unwrap();
        assert_eq!(
            unsafe { ofd_set_page_object_blend_mode(handle, 0, id.as_ptr(), mode.as_ptr()) },
            0
        );
        assert_eq!(
            unsafe { document_ref(handle).unwrap() }
                .page_objects(0)
                .unwrap()[0]
                .blend_mode
                .as_deref(),
            Some("Darken")
        );
        let invalid = CString::new("invalid").unwrap();
        assert_ne!(
            unsafe { ofd_set_page_object_blend_mode(handle, 0, id.as_ptr(), invalid.as_ptr()) },
            0
        );
        assert_eq!(
            unsafe { ofd_set_page_object_blend_mode(handle, 0, id.as_ptr(), ptr::null()) },
            0
        );
        assert_eq!(
            unsafe { document_ref(handle).unwrap() }
                .page_objects(0)
                .unwrap()[0]
                .blend_mode,
            None
        );
        unsafe { ofd_close(handle) };
    }

    #[test]
    fn ffi_sets_and_clears_page_object_clip_rectangles() {
        let mut handle = ptr::null_mut();
        assert_eq!(unsafe { ofd_new(&mut handle) }, 0);
        assert_eq!(unsafe { ofd_add_blank_page(handle, 20.0, 20.0) }, 0);
        let payload =
            CString::new(r#"{"commands":"M 0 0 L 20 20","boundary":[0,0,20,20]}"#).unwrap();
        let mut object_id = 0;
        assert_eq!(
            unsafe { ofd_add_path_object_json(handle, 0, payload.as_ptr(), &mut object_id) },
            0
        );
        let id = CString::new(object_id.to_string()).unwrap();
        let clips = CString::new(r#"{"rects":[[0,0,10,20]],"follows_ctm":false}"#).unwrap();
        assert_eq!(
            unsafe { ofd_set_page_object_clip_rects_json(handle, 0, id.as_ptr(), clips.as_ptr()) },
            0
        );
        assert!(unsafe { document_ref(handle).unwrap() }
            .page_svg(0)
            .unwrap()
            .contains("ofd-clip-"));
        let invalid = CString::new(r#"{"rects":[[0,0,-1,20]]}"#).unwrap();
        assert_ne!(
            unsafe {
                ofd_set_page_object_clip_rects_json(handle, 0, id.as_ptr(), invalid.as_ptr())
            },
            0
        );
        let clear = CString::new(r#"{"rects":[]}"#).unwrap();
        assert_eq!(
            unsafe { ofd_set_page_object_clip_rects_json(handle, 0, id.as_ptr(), clear.as_ptr()) },
            0
        );
        assert!(!unsafe { document_ref(handle).unwrap() }
            .page_svg(0)
            .unwrap()
            .contains("ofd-clip-"));
        unsafe { ofd_close(handle) };
    }

    #[test]
    fn ffi_sets_and_clears_page_object_path_clips() {
        let mut handle = ptr::null_mut();
        assert_eq!(unsafe { ofd_new(&mut handle) }, 0);
        assert_eq!(unsafe { ofd_add_blank_page(handle, 20.0, 20.0) }, 0);
        let payload =
            CString::new(r#"{"commands":"M 0 0 L 20 20","boundary":[0,0,20,20]}"#).unwrap();
        let mut object_id = 0;
        assert_eq!(
            unsafe { ofd_add_path_object_json(handle, 0, payload.as_ptr(), &mut object_id) },
            0
        );
        let id = CString::new(object_id.to_string()).unwrap();
        let clips = CString::new(r#"{"clips":[[{"boundary":[0,0,10,20],"commands":"M 0 0 L 10 0 L 10 20 L 0 20 C","even_odd":true}]]}"#).unwrap();
        assert_eq!(
            unsafe { ofd_set_page_object_path_clips_json(handle, 0, id.as_ptr(), clips.as_ptr()) },
            0
        );
        let page = unsafe { document_ref(handle).unwrap() }.pages().unwrap()[0]
            .path
            .clone();
        assert!(std::str::from_utf8(
            unsafe { document_ref(handle).unwrap() }
                .entry(&page)
                .unwrap()
                .unwrap()
        )
        .unwrap()
        .contains("Rule=\"Even-Odd\""));
        let invalid = CString::new(r#"{"clips":[[]]}"#).unwrap();
        assert_ne!(
            unsafe {
                ofd_set_page_object_path_clips_json(handle, 0, id.as_ptr(), invalid.as_ptr())
            },
            0
        );
        let draw_param = unsafe { document_mut(handle).unwrap() }
            .add_document_draw_param(
                0,
                &DrawParamStyle {
                    line_width: Some(2.0),
                    ..DrawParamStyle::default()
                },
            )
            .unwrap();
        let stroke = serde_json::json!({"clips":[[{
            "boundary":[0,0,20,20], "commands":"M 2 10 L 18 10",
            "fill":false,"stroke":true,"draw_param":draw_param
        }]]})
        .to_string();
        let stroke = CString::new(stroke).unwrap();
        assert_eq!(
            unsafe { ofd_set_page_object_path_clips_json(handle, 0, id.as_ptr(), stroke.as_ptr()) },
            0
        );
        assert!(unsafe { document_ref(handle).unwrap() }
            .page_svg(0)
            .unwrap()
            .contains("stroke-width=\"2\""));
        let clip = &unsafe { document_ref(handle).unwrap() }
            .page_object_clips(0)
            .unwrap()[0]
            .clips[0][0];
        assert_eq!(clip.fill, Some(false));
        assert_eq!(clip.stroke, Some(true));
        let clear = CString::new(r#"{"clips":[]}"#).unwrap();
        assert_eq!(
            unsafe { ofd_set_page_object_path_clips_json(handle, 0, id.as_ptr(), clear.as_ptr()) },
            0
        );
        assert!(!unsafe { document_ref(handle).unwrap() }
            .page_svg(0)
            .unwrap()
            .contains("ofd-clip-"));
        unsafe { ofd_close(handle) };
    }

    #[test]
    fn ffi_sets_mixed_text_and_path_clips() {
        let mut handle = ptr::null_mut();
        assert_eq!(unsafe { ofd_new(&mut handle) }, 0);
        let text = CString::new("font").unwrap();
        assert_eq!(
            unsafe { ofd_add_text_page(handle, text.as_ptr(), 30.0, 30.0) },
            0
        );
        let font_id = unsafe { document_ref(handle).unwrap() }.fonts().unwrap()[0]
            .id
            .clone();
        let path = CString::new(r#"{"commands":"M 0 0 L 20 20","boundary":[0,0,20,20]}"#).unwrap();
        let mut object_id = 0;
        assert_eq!(
            unsafe { ofd_add_path_object_json(handle, 0, path.as_ptr(), &mut object_id) },
            0
        );
        let id = CString::new(object_id.to_string()).unwrap();
        let clips = serde_json::json!({"clips":[[
            {"kind":"Path","boundary":[0,0,5,20],"commands":"M 0 0 L 5 0 L 5 20 L 0 20 C"},
            {"kind":"Text","boundary":[0,0,20,20],"font_id":font_id,"size":14,"text":"O","x":10,"y":15}
        ]]}).to_string();
        let clips = CString::new(clips).unwrap();
        assert_eq!(
            unsafe { ofd_set_page_object_clips_json(handle, 0, id.as_ptr(), clips.as_ptr()) },
            0
        );
        let mut output = OfdBuffer {
            data: ptr::null_mut(),
            len: 0,
        };
        assert_eq!(
            unsafe { ofd_page_object_clips_json(handle, 0, &mut output) },
            0
        );
        let value: serde_json::Value =
            serde_json::from_slice(unsafe { std::slice::from_raw_parts(output.data, output.len) })
                .unwrap();
        assert_eq!(value[1]["clips"][0][0]["kind"], "Path");
        assert_eq!(value[1]["clips"][0][0]["fill"], true);
        assert_eq!(value[1]["clips"][0][0]["stroke"], false);
        assert_eq!(value[1]["clips"][0][1]["kind"], "Text");
        assert_eq!(value[1]["clips"][0][1]["text"], "O");
        assert_eq!(value[1]["clips"][0][1]["text_codes"][0]["x"], 10.0);
        assert_eq!(value[1]["clips"][0][1]["text_codes"][0]["y"], 15.0);
        assert!(value[1]["clips"][0][1]["xml"]
            .as_str()
            .unwrap()
            .contains("TextCode"));
        unsafe { ofd_buffer_free(output) };
        assert!(unsafe { document_ref(handle).unwrap() }
            .page_svg(0)
            .unwrap()
            .contains("ofd-clip-"));
        let invalid = CString::new(r#"{"clips":[[{"kind":"Text","boundary":[0,0,20,20],"font_id":"99999","size":14,"text":"O","x":10,"y":15}]]}"#).unwrap();
        assert_ne!(
            unsafe { ofd_set_page_object_clips_json(handle, 0, id.as_ptr(), invalid.as_ptr()) },
            0
        );
        let rich = serde_json::json!({"clips":[[{
            "kind":"Text","boundary":[0,0,20,20],"font_id":font_id,
            "size":14,"text":"IX","x":2,"y":15,
            "text_codes":[{"x":2,"y":15,"text":"I"},{"x":8,"y":15,"text":"X"}],
            "cg_transforms":[{"code_position":0,"code_count":2,"glyph_count":1,"glyphs":[1]}]
        }]]})
        .to_string();
        let rich = CString::new(rich).unwrap();
        assert_eq!(
            unsafe { ofd_set_page_object_clips_json(handle, 0, id.as_ptr(), rich.as_ptr()) },
            0
        );
        let area = &unsafe { document_ref(handle).unwrap() }
            .page_object_clips(0)
            .unwrap()[1]
            .clips[0][0];
        assert_eq!(area.text.as_deref(), Some("IX"));
        assert_eq!(area.text_codes[1].x, Some(8.0));
        assert_eq!(area.cg_transforms[0].glyphs, vec![1]);
        let draw_param = unsafe { document_mut(handle).unwrap() }
            .add_document_draw_param(
                0,
                &DrawParamStyle {
                    line_width: Some(2.0),
                    ..DrawParamStyle::default()
                },
            )
            .unwrap();
        let stroke = serde_json::json!({"clips":[[{
            "kind":"Text","boundary":[0,0,20,20],"font_id":font_id,
            "size":14,"text":"O","x":3,"y":16,
            "fill":false,"stroke":true,"draw_param":draw_param
        }]]})
        .to_string();
        let stroke = CString::new(stroke).unwrap();
        assert_eq!(
            unsafe { ofd_set_page_object_clips_json(handle, 0, id.as_ptr(), stroke.as_ptr()) },
            0
        );
        let area = &unsafe { document_ref(handle).unwrap() }
            .page_object_clips(0)
            .unwrap()[1]
            .clips[0][0];
        assert_eq!(area.fill, Some(false));
        assert_eq!(area.stroke, Some(true));
        assert!(unsafe { document_ref(handle).unwrap() }
            .page_svg(0)
            .unwrap()
            .contains("stroke-width=\"2\""));
        let clear = CString::new(r#"{"clips":[]}"#).unwrap();
        assert_eq!(
            unsafe { ofd_set_page_object_clips_json(handle, 0, id.as_ptr(), clear.as_ptr()) },
            0
        );
        assert!(!unsafe { document_ref(handle).unwrap() }
            .page_svg(0)
            .unwrap()
            .contains("ofd-clip-"));
        unsafe { ofd_close(handle) };
    }

    #[test]
    fn ffi_updates_page_object_visibility_and_alpha() {
        let mut handle = ptr::null_mut();
        assert_eq!(unsafe { ofd_new(&mut handle) }, 0);
        assert_eq!(unsafe { ofd_add_blank_page(handle, 20.0, 20.0) }, 0);
        let payload =
            CString::new(r#"{"commands":"M 0 0 L 20 20","boundary":[0,0,20,20]}"#).unwrap();
        let mut id = 0;
        assert_eq!(
            unsafe { ofd_add_path_object_json(handle, 0, payload.as_ptr(), &mut id) },
            0
        );
        let id = CString::new(id.to_string()).unwrap();
        assert_eq!(
            unsafe { ofd_set_page_object_visible(handle, 0, id.as_ptr(), 0) },
            0
        );
        assert_eq!(
            unsafe { ofd_set_page_object_alpha(handle, 0, id.as_ptr(), 128) },
            0
        );
        let commands = CString::new("M 0 0 Q 2 4 5 5").unwrap();
        assert_eq!(
            unsafe { ofd_set_page_path_data(handle, 0, id.as_ptr(), commands.as_ptr()) },
            0
        );
        assert_eq!(
            unsafe { document_ref(handle).unwrap() }
                .page_paths(0)
                .unwrap()[0]
                .abbreviated_data
                .as_deref(),
            Some("M 0 0 Q 2 4 5 5")
        );
        let before = unsafe { document_ref(handle).unwrap() }.to_bytes().unwrap();
        let invalid = CString::new("M 0 0 X 5 5").unwrap();
        assert_ne!(
            unsafe { ofd_set_page_path_data(handle, 0, id.as_ptr(), invalid.as_ptr()) },
            0
        );
        assert_eq!(
            unsafe { document_ref(handle).unwrap() }.to_bytes().unwrap(),
            before
        );
        let page_path = unsafe { document_ref(handle).unwrap() }.pages().unwrap()[0]
            .path
            .clone();
        let page = std::str::from_utf8(
            unsafe { document_ref(handle).unwrap() }
                .entry(&page_path)
                .unwrap()
                .unwrap(),
        )
        .unwrap();
        assert!(page.contains("Visible=\"false\""));
        assert!(page.contains("Alpha=\"128\""));
        assert_ne!(
            unsafe { ofd_set_page_object_visible(handle, 0, id.as_ptr(), 2) },
            0
        );
        assert_ne!(
            unsafe { ofd_set_page_object_alpha(handle, 0, id.as_ptr(), 256) },
            0
        );
        assert_ne!(
            unsafe { ofd_set_page_object_alpha(handle, 0, ptr::null(), 0) },
            0
        );
        unsafe { ofd_close(handle) };
    }

    #[test]
    fn ffi_sets_and_clears_page_image_mask() {
        let fixture = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("../ofdrw/tests/fixtures/image-mask.ofd");
        let path = CString::new(fixture.to_str().unwrap()).unwrap();
        let mut handle = ptr::null_mut();
        assert_eq!(unsafe { ofd_open(path.as_ptr(), &mut handle) }, 0);
        let id = CString::new("5").unwrap();
        let mask = CString::new("4").unwrap();
        assert_eq!(
            unsafe { ofd_set_page_image_mask(handle, 0, id.as_ptr(), ptr::null()) },
            0
        );
        assert!(unsafe { document_ref(handle).unwrap() }
            .page_images(0)
            .unwrap()[0]
            .image_mask
            .is_none());
        assert_eq!(
            unsafe { ofd_set_page_image_mask(handle, 0, id.as_ptr(), mask.as_ptr()) },
            0
        );
        let mut output = OfdBuffer {
            data: ptr::null_mut(),
            len: 0,
        };
        assert_eq!(unsafe { ofd_page_images_json(handle, 0, &mut output) }, 0);
        let values: serde_json::Value =
            serde_json::from_slice(unsafe { std::slice::from_raw_parts(output.data, output.len) })
                .unwrap();
        assert_eq!(values[0]["image_mask"], "4");
        unsafe { ofd_buffer_free(output) };
        let missing = CString::new("999").unwrap();
        assert_ne!(
            unsafe { ofd_set_page_image_mask(handle, 0, id.as_ptr(), missing.as_ptr()) },
            0
        );
        assert_ne!(
            unsafe { ofd_set_page_image_mask(handle, 0, ptr::null(), mask.as_ptr()) },
            0
        );
        assert_eq!(
            unsafe { document_ref(handle).unwrap() }
                .page_images(0)
                .unwrap()[0]
                .image_mask
                .as_deref(),
            Some("4")
        );
        unsafe { ofd_close(handle) };
    }

    #[test]
    fn ffi_sets_and_clears_page_image_substitution() {
        let fixture = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("../ofdrw/tests/fixtures/image-mask.ofd");
        let path = CString::new(fixture.to_str().unwrap()).unwrap();
        let mut handle = ptr::null_mut();
        assert_eq!(unsafe { ofd_open(path.as_ptr(), &mut handle) }, 0);
        let id = CString::new("5").unwrap();
        let alternative = CString::new("4").unwrap();
        assert_eq!(
            unsafe {
                ofd_set_page_image_substitution(handle, 0, id.as_ptr(), alternative.as_ptr())
            },
            0
        );
        let mut output = OfdBuffer {
            data: ptr::null_mut(),
            len: 0,
        };
        assert_eq!(unsafe { ofd_page_images_json(handle, 0, &mut output) }, 0);
        let values: serde_json::Value =
            serde_json::from_slice(unsafe { std::slice::from_raw_parts(output.data, output.len) })
                .unwrap();
        assert_eq!(values[0]["substitution"], "4");
        unsafe { ofd_buffer_free(output) };
        let missing = CString::new("999").unwrap();
        assert_ne!(
            unsafe { ofd_set_page_image_substitution(handle, 0, id.as_ptr(), missing.as_ptr()) },
            0
        );
        assert_eq!(
            unsafe { ofd_set_page_image_substitution(handle, 0, id.as_ptr(), ptr::null()) },
            0
        );
        assert!(unsafe { document_ref(handle).unwrap() }
            .page_images(0)
            .unwrap()[0]
            .substitution
            .is_none());
        unsafe { ofd_close(handle) };
    }

    #[test]
    fn ffi_selects_image_substitution_for_exports() {
        let fixture = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("../ofdrw/tests/fixtures/image-mask.ofd");
        let path = CString::new(fixture.to_str().unwrap()).unwrap();
        let mut handle = ptr::null_mut();
        assert_eq!(unsafe { ofd_open(path.as_ptr(), &mut handle) }, 0);
        let id = CString::new("5").unwrap();
        let alternative = CString::new("4").unwrap();
        assert_eq!(
            unsafe {
                ofd_set_page_image_substitution(handle, 0, id.as_ptr(), alternative.as_ptr())
            },
            0
        );
        let mut primary = OfdBuffer {
            data: ptr::null_mut(),
            len: 0,
        };
        let mut selected = OfdBuffer {
            data: ptr::null_mut(),
            len: 0,
        };
        assert_eq!(unsafe { ofd_page_svg(handle, 0, &mut primary) }, 0);
        assert_eq!(
            unsafe { ofd_page_svg_with_options(handle, 0, 1, &mut selected) },
            0
        );
        assert_ne!(
            unsafe { std::slice::from_raw_parts(primary.data, primary.len) },
            unsafe { std::slice::from_raw_parts(selected.data, selected.len) }
        );
        unsafe {
            ofd_buffer_free(primary);
            ofd_buffer_free(selected)
        };
        selected = OfdBuffer {
            data: ptr::null_mut(),
            len: 0,
        };
        assert_ne!(
            unsafe { ofd_page_svg_with_options(handle, 0, 2, &mut selected) },
            0
        );
        assert_eq!(
            unsafe { ofd_page_png_with_options(handle, 0, 254.0, 1, &mut selected) },
            0
        );
        assert!(
            unsafe { std::slice::from_raw_parts(selected.data, selected.len) }
                .starts_with(b"\x89PNG")
        );
        unsafe { ofd_buffer_free(selected) };
        selected = OfdBuffer {
            data: ptr::null_mut(),
            len: 0,
        };
        assert_eq!(
            unsafe { ofd_to_pdf_with_options(handle, 1, &mut selected) },
            0
        );
        assert!(
            unsafe { std::slice::from_raw_parts(selected.data, selected.len) }
                .starts_with(b"%PDF-")
        );
        unsafe {
            ofd_buffer_free(selected);
            ofd_close(handle)
        };
    }

    #[test]
    fn ffi_sets_and_clears_page_image_border() {
        let fixture = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("../ofdrw/tests/fixtures/image-mask.ofd");
        let path = CString::new(fixture.to_str().unwrap()).unwrap();
        let mut handle = ptr::null_mut();
        assert_eq!(unsafe { ofd_open(path.as_ptr(), &mut handle) }, 0);
        let id = CString::new("5").unwrap();
        let style =
            CString::new(r#"{"line_width":0.4,"color":[0,255,0],"dash_pattern":[1,2,3]}"#).unwrap();
        assert_eq!(
            unsafe { ofd_set_page_image_border_json(handle, 0, id.as_ptr(), style.as_ptr()) },
            0
        );
        let mut images = OfdBuffer {
            data: ptr::null_mut(),
            len: 0,
        };
        assert_eq!(unsafe { ofd_page_images_json(handle, 0, &mut images) }, 0);
        let values: serde_json::Value =
            serde_json::from_slice(unsafe { std::slice::from_raw_parts(images.data, images.len) })
                .unwrap();
        assert_eq!(values[0]["border"]["line_width"], 0.4);
        assert_eq!(values[0]["border"]["color_value"], "0 255 0");
        unsafe { ofd_buffer_free(images) };
        assert!(unsafe { document_ref(handle).unwrap() }
            .page_svg(0)
            .unwrap()
            .contains("stroke=\"#00ff00\""));
        let before = unsafe { document_ref(handle).unwrap() }.to_bytes().unwrap();
        let invalid = CString::new(r#"{"dash_pattern":[1,0]}"#).unwrap();
        assert_ne!(
            unsafe { ofd_set_page_image_border_json(handle, 0, id.as_ptr(), invalid.as_ptr()) },
            0
        );
        assert_eq!(
            unsafe { document_ref(handle).unwrap() }.to_bytes().unwrap(),
            before
        );
        let clear = CString::new("null").unwrap();
        assert_eq!(
            unsafe { ofd_set_page_image_border_json(handle, 0, id.as_ptr(), clear.as_ptr()) },
            0
        );
        assert!(!unsafe { document_ref(handle).unwrap() }
            .page_svg(0)
            .unwrap()
            .contains("stroke=\"#00ff00\""));
        unsafe { ofd_close(handle) };
    }

    #[test]
    fn ffi_lists_page_objects_in_painting_order() {
        let mut handle = ptr::null_mut();
        assert_eq!(unsafe { ofd_new(&mut handle) }, 0);
        assert_eq!(unsafe { ofd_add_blank_page(handle, 20.0, 20.0) }, 0);
        let payload = CString::new(r#"{"commands":"M 0 0 L 5 5","boundary":[0,0,5,5]}"#).unwrap();
        let mut id = 0;
        assert_eq!(
            unsafe { ofd_add_path_object_json(handle, 0, payload.as_ptr(), &mut id) },
            0
        );
        let id = CString::new(id.to_string()).unwrap();
        assert_eq!(
            unsafe { ofd_set_page_object_alpha(handle, 0, id.as_ptr(), 128) },
            0
        );
        let mut output = OfdBuffer {
            data: ptr::null_mut(),
            len: 0,
        };
        assert_eq!(unsafe { ofd_page_objects_json(handle, 0, &mut output) }, 0);
        let objects: serde_json::Value =
            serde_json::from_slice(unsafe { std::slice::from_raw_parts(output.data, output.len) })
                .unwrap();
        assert_eq!(objects[0]["kind"], "PathObject");
        assert_eq!(objects[0]["alpha"], 128);
        assert_eq!(objects[0]["visible"], true);
        assert_eq!(objects[0]["id"], id.to_str().unwrap());
        unsafe { ofd_buffer_free(output) };
        let mut output = OfdBuffer {
            data: ptr::null_mut(),
            len: 0,
        };
        assert_eq!(
            unsafe { ofd_page_object_paints_json(handle, 0, &mut output) },
            0
        );
        let paints: serde_json::Value =
            serde_json::from_slice(unsafe { std::slice::from_raw_parts(output.data, output.len) })
                .unwrap();
        assert_eq!(paints[0]["id"], id.to_str().unwrap());
        assert_eq!(paints[0]["kind"], "PathObject");
        assert_eq!(paints[0]["fill_enabled"], false);
        assert_eq!(paints[0]["stroke_enabled"], true);
        assert!(paints[0]["fill"].is_null());
        assert!(paints[0]["effective_fill"].is_null());
        assert!(paints[0]["effective_stroke_draw_param"].is_null());
        unsafe { ofd_buffer_free(output) };
        let mut output = OfdBuffer {
            data: ptr::null_mut(),
            len: 0,
        };
        assert_eq!(
            unsafe { ofd_page_object_stroke_styles_json(handle, 0, &mut output) },
            0
        );
        let styles: serde_json::Value =
            serde_json::from_slice(unsafe { std::slice::from_raw_parts(output.data, output.len) })
                .unwrap();
        assert_eq!(styles[0]["id"], id.to_str().unwrap());
        assert_eq!(styles[0]["style"]["line_width"], 0.4);
        assert_eq!(styles[0]["style"]["cap"], "butt");
        assert_eq!(styles[0]["style"]["join"], "miter");
        assert!(styles[0]["style"]["dash_pattern"].is_null());
        unsafe { ofd_buffer_free(output) };
        let style = CString::new(r#"{"line_width":1.25,"cap":"Round","join":"Bevel","dash_pattern":[2,1],"dash_offset":0.5}"#).unwrap();
        assert_eq!(
            unsafe { ofd_set_page_path_stroke_style_json(handle, 0, id.as_ptr(), style.as_ptr()) },
            0
        );
        let styles = unsafe { document_ref(handle).unwrap() }
            .page_object_stroke_styles(0)
            .unwrap();
        assert_eq!(styles[0].style.line_width, 1.25);
        assert_eq!(styles[0].style.cap.as_deref(), Some("round"));
        assert_eq!(styles[0].style.dash_pattern, Some([2.0, 1.0]));
        let before = unsafe { document_ref(handle).unwrap() }.to_bytes().unwrap();
        let invalid = CString::new(r#"{"line_width":1,"dash_pattern":[0,1]}"#).unwrap();
        assert_ne!(
            unsafe {
                ofd_set_page_path_stroke_style_json(handle, 0, id.as_ptr(), invalid.as_ptr())
            },
            0
        );
        assert_eq!(
            unsafe { document_ref(handle).unwrap() }.to_bytes().unwrap(),
            before
        );
        let mut output = OfdBuffer {
            data: ptr::null_mut(),
            len: 0,
        };
        assert_ne!(unsafe { ofd_page_objects_json(handle, 1, &mut output) }, 0);
        assert_ne!(
            unsafe { ofd_page_object_paints_json(handle, 1, &mut output) },
            0
        );
        assert_ne!(
            unsafe { ofd_page_object_stroke_styles_json(handle, 1, &mut output) },
            0
        );
        assert_ne!(
            unsafe { ofd_page_objects_json(handle, 0, ptr::null_mut()) },
            0
        );
        unsafe { ofd_close(handle) };
    }

    #[test]
    fn ffi_creates_and_assigns_reusable_draw_param() {
        let mut handle = ptr::null_mut();
        assert_eq!(unsafe { ofd_new(&mut handle) }, 0);
        assert_eq!(unsafe { ofd_add_blank_page(handle, 20.0, 20.0) }, 0);
        let payload =
            CString::new(r#"{"commands":"M 0 0 L 20 0","boundary":[0,0,20,20]}"#).unwrap();
        let mut object_id = 0;
        assert_eq!(
            unsafe { ofd_add_path_object_json(handle, 0, payload.as_ptr(), &mut object_id) },
            0
        );
        let style = CString::new(r#"{"stroke":{"color":[255,0,0],"alpha":200}}"#).unwrap();
        let mut draw_id = 0;
        assert_eq!(
            unsafe { ofd_add_document_draw_param_json(handle, 0, style.as_ptr(), &mut draw_id) },
            0
        );
        let object_id = CString::new(object_id.to_string()).unwrap();
        let draw_id = CString::new(draw_id.to_string()).unwrap();
        assert_eq!(
            unsafe {
                ofd_set_page_object_draw_param(handle, 0, object_id.as_ptr(), draw_id.as_ptr())
            },
            0
        );
        let mut output = OfdBuffer {
            data: ptr::null_mut(),
            len: 0,
        };
        assert_eq!(
            unsafe { ofd_page_object_paints_json(handle, 0, &mut output) },
            0
        );
        let paints: serde_json::Value =
            serde_json::from_slice(unsafe { std::slice::from_raw_parts(output.data, output.len) })
                .unwrap();
        assert_eq!(
            paints[0]["effective_stroke_draw_param"],
            draw_id.to_str().unwrap()
        );
        assert_eq!(paints[0]["effective_stroke"]["alpha"], 200);
        unsafe { ofd_buffer_free(output) };
        let gradient = CString::new(r#"{"fill_gradient":{"kind":"axial","start_point":[0,0],"end_point":[20,0],"segments":[{"color":[255,0,0]},{"color":[0,0,255]}]}}"#).unwrap();
        let mut gradient_id = 0;
        assert_eq!(
            unsafe {
                ofd_add_document_draw_param_json(handle, 0, gradient.as_ptr(), &mut gradient_id)
            },
            0
        );
        let gradient_id = CString::new(gradient_id.to_string()).unwrap();
        assert_eq!(
            unsafe {
                ofd_set_page_object_draw_param(handle, 0, object_id.as_ptr(), gradient_id.as_ptr())
            },
            0
        );
        let mut output = OfdBuffer {
            data: ptr::null_mut(),
            len: 0,
        };
        assert_eq!(
            unsafe { ofd_page_object_paints_json(handle, 0, &mut output) },
            0
        );
        let paints: serde_json::Value =
            serde_json::from_slice(unsafe { std::slice::from_raw_parts(output.data, output.len) })
                .unwrap();
        assert_eq!(paints[0]["effective_fill"]["kind"], "Axial");
        unsafe { ofd_buffer_free(output) };
        let pattern = CString::new(r#"{"fill_pattern":{"width":4,"height":4,"cells":[{"kind":"path","commands":"M 0 0 L 4 0 L 4 4 C","boundary":[0,0,4,4],"fill":true,"stroke":false,"fill_color":[255,0,0]}]}}"#).unwrap();
        let mut pattern_id = 0;
        assert_eq!(
            unsafe {
                ofd_add_document_draw_param_json(handle, 0, pattern.as_ptr(), &mut pattern_id)
            },
            0
        );
        let pattern_id = CString::new(pattern_id.to_string()).unwrap();
        assert_eq!(
            unsafe {
                ofd_set_page_object_draw_param(handle, 0, object_id.as_ptr(), pattern_id.as_ptr())
            },
            0
        );
        let mut output = OfdBuffer {
            data: ptr::null_mut(),
            len: 0,
        };
        assert_eq!(
            unsafe { ofd_page_object_paints_json(handle, 0, &mut output) },
            0
        );
        let paints: serde_json::Value =
            serde_json::from_slice(unsafe { std::slice::from_raw_parts(output.data, output.len) })
                .unwrap();
        assert_eq!(paints[0]["effective_fill"]["kind"], "Pattern");
        unsafe { ofd_buffer_free(output) };
        let invalid = CString::new(r#"{"relative":"999999"}"#).unwrap();
        let mut invalid_id = 0;
        assert_ne!(
            unsafe {
                ofd_add_document_draw_param_json(handle, 0, invalid.as_ptr(), &mut invalid_id)
            },
            0
        );
        assert_eq!(invalid_id, 0);
        assert_eq!(
            unsafe { ofd_set_page_object_draw_param(handle, 0, object_id.as_ptr(), ptr::null()) },
            0
        );
        unsafe { ofd_close(handle) };
    }

    #[test]
    fn ffi_page_draw_param_stays_in_its_page_scope() {
        let mut handle = ptr::null_mut();
        assert_eq!(unsafe { ofd_new(&mut handle) }, 0);
        assert_eq!(unsafe { ofd_add_blank_page(handle, 20.0, 20.0) }, 0);
        assert_eq!(unsafe { ofd_append_blank_page(handle, 0, 20.0, 20.0) }, 0);
        let style = CString::new(r#"{"fill":{"color":[0,255,0]}}"#).unwrap();
        let mut id = 0;
        assert_eq!(
            unsafe { ofd_add_page_draw_param_json(handle, 0, style.as_ptr(), &mut id) },
            0
        );
        assert!(id > 0);
        let document = unsafe { document_ref(handle).unwrap() };
        let pages = document.pages().unwrap();
        let resource_name = format!("RustPageDrawParams_{id}.xml");
        let first = std::str::from_utf8(document.entry(&pages[0].path).unwrap().unwrap()).unwrap();
        let second = std::str::from_utf8(document.entry(&pages[1].path).unwrap().unwrap()).unwrap();
        assert!(first.contains(&resource_name));
        assert!(!second.contains(&resource_name));
        let directory = pages[0].path.rsplit_once('/').unwrap().0;
        assert!(document
            .entry(&format!("{directory}/{resource_name}"))
            .unwrap()
            .is_some());
        let calls: [(
            unsafe extern "C" fn(*const OfdDocument, usize, *mut OfdBuffer) -> i32,
            usize,
        ); 2] = [
            (ofd_document_draw_params_json, 0),
            (ofd_page_draw_params_json, 1),
        ];
        for (call, expected) in calls {
            let mut output = OfdBuffer {
                data: ptr::null_mut(),
                len: 0,
            };
            assert_eq!(unsafe { call(handle, 0, &mut output) }, 0);
            let params: serde_json::Value = serde_json::from_slice(unsafe {
                std::slice::from_raw_parts(output.data, output.len)
            })
            .unwrap();
            unsafe { ofd_buffer_free(output) };
            assert_eq!(params.as_array().unwrap().len(), expected);
            if expected == 1 {
                assert_eq!(params[0]["id"], id.to_string());
                assert!(params[0]["resource_path"]
                    .as_str()
                    .unwrap()
                    .ends_with(&resource_name));
            }
        }
        let local_id = CString::new(id.to_string()).unwrap();
        let updated = CString::new(r#"{"fill":{"color":[0,0,255]}}"#).unwrap();
        assert_eq!(
            unsafe { ofd_set_page_draw_param_json(handle, 0, local_id.as_ptr(), updated.as_ptr()) },
            0
        );
        assert!(unsafe { document_ref(handle).unwrap() }
            .page_draw_params(0)
            .unwrap()[0]
            .xml
            .contains("0 0 255"));
        let before = unsafe { document_ref(handle).unwrap() }.to_bytes().unwrap();
        assert_ne!(
            unsafe { ofd_set_page_draw_param_json(handle, 1, local_id.as_ptr(), updated.as_ptr()) },
            0
        );
        assert_ne!(
            unsafe {
                ofd_set_document_draw_param_json(handle, 0, local_id.as_ptr(), updated.as_ptr())
            },
            0
        );
        let mut invalid_id = 0;
        assert_ne!(
            unsafe { ofd_add_page_draw_param_json(handle, 1, ptr::null(), &mut invalid_id) },
            0
        );
        assert_eq!(invalid_id, 0);
        assert_eq!(
            unsafe { document_ref(handle).unwrap() }.to_bytes().unwrap(),
            before
        );
        assert_eq!(
            unsafe { ofd_remove_page_draw_param(handle, 0, local_id.as_ptr()) },
            0
        );
        assert!(unsafe { document_ref(handle).unwrap() }
            .page_draw_params(0)
            .unwrap()
            .is_empty());
        let mut public_id = 0;
        assert_eq!(
            unsafe { ofd_add_document_draw_param_json(handle, 0, style.as_ptr(), &mut public_id) },
            0
        );
        let public_id = CString::new(public_id.to_string()).unwrap();
        assert_eq!(
            unsafe { ofd_remove_document_draw_param(handle, 0, public_id.as_ptr()) },
            0
        );
        unsafe { ofd_close(handle) };
    }

    #[test]
    fn ffi_edits_page_object_geometry() {
        let mut handle = ptr::null_mut();
        assert_eq!(unsafe { ofd_new(&mut handle) }, 0);
        assert_eq!(unsafe { ofd_add_blank_page(handle, 20.0, 20.0) }, 0);
        let payload = CString::new(r#"{"commands":"M 0 0 L 5 5","boundary":[0,0,5,5]}"#).unwrap();
        let mut id = 0;
        assert_eq!(
            unsafe { ofd_add_path_object_json(handle, 0, payload.as_ptr(), &mut id) },
            0
        );
        let id = CString::new(id.to_string()).unwrap();
        assert_eq!(
            unsafe { ofd_set_page_object_boundary(handle, 0, id.as_ptr(), 5.0, 6.0, 5.0, 5.0) },
            0
        );
        let ctm = CString::new("[1,0,0,1,2,3]").unwrap();
        assert_eq!(
            unsafe { ofd_set_page_object_ctm_json(handle, 0, id.as_ptr(), ctm.as_ptr()) },
            0
        );
        let path = &unsafe { document_ref(handle).unwrap() }
            .page_paths(0)
            .unwrap()[0];
        assert_eq!(path.boundary, Some([5.0, 6.0, 5.0, 5.0]));
        assert_eq!(path.ctm, Some([1.0, 0.0, 0.0, 1.0, 2.0, 3.0]));
        assert_eq!(
            unsafe { ofd_set_page_object_ctm_json(handle, 0, id.as_ptr(), ptr::null()) },
            0
        );
        assert_eq!(
            unsafe { document_ref(handle).unwrap() }
                .page_paths(0)
                .unwrap()[0]
                .ctm,
            None
        );
        let bad = CString::new("[1,2]").unwrap();
        assert_ne!(
            unsafe { ofd_set_page_object_ctm_json(handle, 0, id.as_ptr(), bad.as_ptr()) },
            0
        );
        assert_ne!(
            unsafe { ofd_set_page_object_boundary(handle, 0, id.as_ptr(), 0.0, 0.0, -1.0, 5.0) },
            0
        );
        assert_ne!(
            unsafe { ofd_set_page_object_boundary(handle, 0, ptr::null(), 0.0, 0.0, 5.0, 5.0) },
            0
        );
        unsafe { ofd_close(handle) };
    }

    #[test]
    fn ffi_duplicates_page_object_and_rejects_invalid_requests() {
        let mut handle = ptr::null_mut();
        assert_eq!(unsafe { ofd_new(&mut handle) }, 0);
        assert_eq!(unsafe { ofd_add_blank_page(handle, 20.0, 20.0) }, 0);
        let payload = CString::new(
            r#"{"commands":"M 0 0 L 10 10","boundary":[0,0,10,10],"dash_sequence":[1,2,3]}"#,
        )
        .unwrap();
        let mut id = 0;
        assert_eq!(
            unsafe { ofd_add_path_object_json(handle, 0, payload.as_ptr(), &mut id) },
            0
        );
        let source_id = CString::new(id.to_string()).unwrap();
        let mut copy_id = 0;
        assert_eq!(
            unsafe { ofd_duplicate_page_object(handle, 0, source_id.as_ptr(), &mut copy_id) },
            0
        );
        assert!(copy_id > id);
        let document = unsafe { document_ref(handle).unwrap() };
        assert_eq!(document.page_paths(0).unwrap().len(), 2);
        assert_eq!(
            document.page_object_stroke_styles(0).unwrap()[1]
                .style
                .dash_sequence
                .as_deref(),
            Some(&[1.0, 2.0, 3.0][..])
        );
        let before = document.to_bytes().unwrap();
        let missing = CString::new("missing").unwrap();
        copy_id = 999;
        assert_ne!(
            unsafe { ofd_duplicate_page_object(handle, 0, missing.as_ptr(), &mut copy_id) },
            0
        );
        assert_eq!(copy_id, 0);
        assert_ne!(
            unsafe { ofd_duplicate_page_object(handle, 0, source_id.as_ptr(), ptr::null_mut()) },
            0
        );
        assert_eq!(
            unsafe { document_ref(handle).unwrap() }.to_bytes().unwrap(),
            before
        );
        unsafe { ofd_close(handle) };
    }

    #[test]
    fn ffi_removes_page_object() {
        let mut handle = ptr::null_mut();
        assert_eq!(unsafe { ofd_new(&mut handle) }, 0);
        let first = CString::new("first").unwrap();
        assert_eq!(
            unsafe { ofd_add_text_page(handle, first.as_ptr(), 210.0, 297.0) },
            0
        );
        let payload =
            CString::new(r#"{"text":"second","boundary":[20,30,50,8],"size":5}"#).unwrap();
        let mut id = 0;
        assert_eq!(
            unsafe { ofd_add_text_object_json(handle, 0, payload.as_ptr(), &mut id) },
            0
        );
        let object_id = CString::new(id.to_string()).unwrap();
        assert_eq!(
            unsafe { ofd_remove_page_object(handle, 0, object_id.as_ptr()) },
            0
        );
        let mut output = OfdBuffer {
            data: ptr::null_mut(),
            len: 0,
        };
        assert_eq!(
            unsafe { ofd_page_text_objects_json(handle, 0, &mut output) },
            0
        );
        let values: serde_json::Value =
            serde_json::from_slice(unsafe { std::slice::from_raw_parts(output.data, output.len) })
                .unwrap();
        assert_eq!(values.as_array().unwrap().len(), 1);
        unsafe { ofd_buffer_free(output) };
        assert_ne!(
            unsafe { ofd_remove_page_object(handle, 0, object_id.as_ptr()) },
            0
        );
        assert_ne!(
            unsafe { ofd_remove_page_object(handle, 1, object_id.as_ptr()) },
            0
        );
        unsafe { ofd_close(handle) };
    }

    #[test]
    fn ffi_document_permissions_round_trip() {
        let mut handle = ptr::null_mut();
        assert_eq!(unsafe { ofd_new(&mut handle) }, 0);
        assert_eq!(unsafe { ofd_add_blank_page(handle, 80.0, 40.0) }, 0);
        let mut output = OfdBuffer {
            data: ptr::null_mut(),
            len: 0,
        };
        assert_eq!(
            unsafe { ofd_document_permissions_json(handle, 0, &mut output) },
            0
        );
        let mut permissions: serde_json::Value =
            serde_json::from_slice(unsafe { std::slice::from_raw_parts(output.data, output.len) })
                .unwrap();
        unsafe { ofd_buffer_free(output) };
        assert_eq!(permissions["print"]["copies"], -1);
        assert_eq!(permissions["edit"], true);
        permissions["edit"] = serde_json::json!(false);
        permissions["print"]["copies"] = serde_json::json!(2);
        permissions["valid_period"] = serde_json::json!({
            "start_date": "2024-02-29T12:00:00", "end_date": null,
        });
        let payload = CString::new(permissions.to_string()).unwrap();
        assert_eq!(
            unsafe { ofd_set_document_permissions_json(handle, 0, payload.as_ptr()) },
            0
        );
        let mut output = OfdBuffer {
            data: ptr::null_mut(),
            len: 0,
        };
        assert_eq!(
            unsafe { ofd_document_permissions_json(handle, 0, &mut output) },
            0
        );
        let got: serde_json::Value =
            serde_json::from_slice(unsafe { std::slice::from_raw_parts(output.data, output.len) })
                .unwrap();
        unsafe { ofd_buffer_free(output) };
        assert_eq!(got, permissions);
        let bad = CString::new(r#"{"edit":false}"#).unwrap();
        assert_ne!(
            unsafe { ofd_set_document_permissions_json(handle, 0, bad.as_ptr()) },
            0
        );
        assert_ne!(
            unsafe { ofd_document_permissions_json(handle, 1, ptr::null_mut()) },
            0
        );
        unsafe { ofd_close(handle) };
    }

    #[test]
    fn ffi_view_preferences_round_trip() {
        let mut handle = ptr::null_mut();
        assert_eq!(unsafe { ofd_new(&mut handle) }, 0);
        let mut output = OfdBuffer {
            data: ptr::null_mut(),
            len: 0,
        };
        assert_eq!(
            unsafe { ofd_view_preferences_json(handle, 0, &mut output) },
            0
        );
        let mut preferences: serde_json::Value =
            serde_json::from_slice(unsafe { std::slice::from_raw_parts(output.data, output.len) })
                .unwrap();
        unsafe { ofd_buffer_free(output) };
        assert_eq!(preferences["page_layout"], "OneColumn");
        preferences["page_mode"] = serde_json::json!("UseOutlines");
        preferences["hide_toolbar"] = serde_json::json!(true);
        preferences["zoom_mode"] = serde_json::json!("FitWidth");
        let payload = CString::new(preferences.to_string()).unwrap();
        assert_eq!(
            unsafe { ofd_set_view_preferences_json(handle, 0, payload.as_ptr()) },
            0
        );
        let mut output = OfdBuffer {
            data: ptr::null_mut(),
            len: 0,
        };
        assert_eq!(
            unsafe { ofd_view_preferences_json(handle, 0, &mut output) },
            0
        );
        let got: serde_json::Value =
            serde_json::from_slice(unsafe { std::slice::from_raw_parts(output.data, output.len) })
                .unwrap();
        unsafe { ofd_buffer_free(output) };
        assert_eq!(got, preferences);
        preferences["zoom"] = serde_json::json!(1.25);
        let invalid = CString::new(preferences.to_string()).unwrap();
        assert_ne!(
            unsafe { ofd_set_view_preferences_json(handle, 0, invalid.as_ptr()) },
            0
        );
        let incomplete = CString::new(r#"{"page_mode":"None"}"#).unwrap();
        assert_ne!(
            unsafe { ofd_set_view_preferences_json(handle, 0, incomplete.as_ptr()) },
            0
        );
        unsafe { ofd_close(handle) };
    }

    #[test]
    fn ffi_document_actions_round_trip() {
        let mut handle = ptr::null_mut();
        assert_eq!(unsafe { ofd_new(&mut handle) }, 0);
        let action =
            CString::new(r#"{"kind":"uri","event":"DO","uri":"https://example.org/?x=1&y=2"}"#)
                .unwrap();
        assert_eq!(
            unsafe { ofd_add_document_action_json(handle, 0, action.as_ptr()) },
            0
        );
        let mut output = OfdBuffer {
            data: ptr::null_mut(),
            len: 0,
        };
        assert_eq!(
            unsafe { ofd_document_actions_json(handle, 0, &mut output) },
            0
        );
        let actions: serde_json::Value =
            serde_json::from_slice(unsafe { std::slice::from_raw_parts(output.data, output.len) })
                .unwrap();
        unsafe { ofd_buffer_free(output) };
        assert_eq!(actions[0]["event"], "DO");
        assert_eq!(
            actions[0]["attributes"]["URI"],
            "https://example.org/?x=1&y=2"
        );
        let invalid = CString::new(r#"{"kind":"bookmark","event":"DO","name":"missing"}"#).unwrap();
        assert_ne!(
            unsafe { ofd_add_document_action_json(handle, 0, invalid.as_ptr()) },
            0
        );
        assert_ne!(unsafe { ofd_remove_document_action(handle, 0, 1) }, 0);
        assert_eq!(unsafe { ofd_remove_document_action(handle, 0, 0) }, 0);
        let mut output = OfdBuffer {
            data: ptr::null_mut(),
            len: 0,
        };
        assert_eq!(
            unsafe { ofd_document_actions_json(handle, 0, &mut output) },
            0
        );
        assert_eq!(
            unsafe { std::slice::from_raw_parts(output.data, output.len) },
            b"[]"
        );
        unsafe { ofd_buffer_free(output) };
        unsafe { ofd_close(handle) };
    }

    #[test]
    fn ffi_custom_tags_round_trip() {
        let mut handle = ptr::null_mut();
        assert_eq!(unsafe { ofd_new(&mut handle) }, 0);
        let metadata =
            CString::new(r#"{"type_id":"invoice","namespace":"urn:test","schema_path":null}"#)
                .unwrap();
        let payload = b"<Invoice Ref=\"1\"/>";
        let mut output = OfdBuffer {
            data: ptr::null_mut(),
            len: 0,
        };
        assert_eq!(
            unsafe {
                ofd_add_custom_tag_json(
                    handle,
                    0,
                    metadata.as_ptr(),
                    payload.as_ptr(),
                    payload.len(),
                    &mut output,
                )
            },
            0
        );
        let tag: serde_json::Value =
            serde_json::from_slice(unsafe { std::slice::from_raw_parts(output.data, output.len) })
                .unwrap();
        unsafe { ofd_buffer_free(output) };
        assert_eq!(tag["type_id"], "invoice");
        let mut output = OfdBuffer {
            data: ptr::null_mut(),
            len: 0,
        };
        assert_eq!(unsafe { ofd_custom_tags_json(handle, &mut output) }, 0);
        let tags: serde_json::Value =
            serde_json::from_slice(unsafe { std::slice::from_raw_parts(output.data, output.len) })
                .unwrap();
        unsafe { ofd_buffer_free(output) };
        assert_eq!(tags[0], tag);
        let mut output = OfdBuffer {
            data: ptr::null_mut(),
            len: 0,
        };
        assert_eq!(unsafe { ofd_custom_tag_data(handle, 0, &mut output) }, 0);
        assert_eq!(
            unsafe { std::slice::from_raw_parts(output.data, output.len) },
            payload
        );
        unsafe { ofd_buffer_free(output) };
        assert_ne!(
            unsafe { ofd_custom_tag_data(handle, 1, ptr::null_mut()) },
            0
        );
        let replacement = b"<Replacement/>";
        let mut output = OfdBuffer {
            data: ptr::null_mut(),
            len: 0,
        };
        assert_ne!(
            unsafe { ofd_replace_custom_tag_data(handle, 0, b"<broken>".as_ptr(), 8, &mut output) },
            0
        );
        assert_eq!(
            unsafe { document_ref(handle).unwrap() }
                .custom_tag_data(0)
                .unwrap(),
            payload
        );
        assert_ne!(
            unsafe {
                ofd_replace_custom_tag_data(
                    handle,
                    0,
                    replacement.as_ptr(),
                    replacement.len(),
                    ptr::null_mut(),
                )
            },
            0
        );
        assert_eq!(
            unsafe {
                ofd_replace_custom_tag_data(
                    handle,
                    0,
                    replacement.as_ptr(),
                    replacement.len(),
                    &mut output,
                )
            },
            0
        );
        let replaced: serde_json::Value =
            serde_json::from_slice(unsafe { std::slice::from_raw_parts(output.data, output.len) })
                .unwrap();
        unsafe { ofd_buffer_free(output) };
        assert_ne!(replaced["path"], tag["path"]);
        assert_eq!(replaced["type_id"], tag["type_id"]);
        assert_eq!(
            unsafe { document_ref(handle).unwrap() }
                .custom_tag_data(0)
                .unwrap(),
            replacement
        );
        assert_eq!(
            unsafe { document_ref(handle).unwrap() }
                .entry(tag["path"].as_str().unwrap())
                .unwrap(),
            Some(payload.as_slice())
        );
        assert_ne!(
            unsafe {
                ofd_add_custom_tag_json(
                    handle,
                    0,
                    metadata.as_ptr(),
                    payload.as_ptr(),
                    payload.len(),
                    ptr::null_mut(),
                )
            },
            0
        );
        assert_eq!(
            unsafe { document_ref(handle).unwrap() }
                .custom_tags()
                .unwrap()
                .len(),
            1
        );
        unsafe { ofd_close(handle) };
    }

    #[test]
    fn ffi_extensions_round_trip() {
        let mut handle = ptr::null_mut();
        assert_eq!(unsafe { ofd_new(&mut handle) }, 0);
        let extension = serde_json::json!({
            "document_index": 0, "app_name": "Example & Co", "company": "Acme",
            "app_version": "1.2", "date": "2024-02-29", "ref_id": 1,
            "properties": [{"name": "status", "type_name": "string", "value": "ready & checked"}],
            "extend_data_paths": [],
            "data_xml": ["<ofd:Data xmlns:ofd=\"http://www.ofdspec.org/2016\"><Payload/></ofd:Data>"],
        });
        let payload = CString::new(extension.to_string()).unwrap();
        assert_eq!(
            unsafe { ofd_add_extension_json(handle, payload.as_ptr()) },
            0
        );
        let mut output = OfdBuffer {
            data: ptr::null_mut(),
            len: 0,
        };
        assert_eq!(unsafe { ofd_extensions_json(handle, &mut output) }, 0);
        let extensions: serde_json::Value =
            serde_json::from_slice(unsafe { std::slice::from_raw_parts(output.data, output.len) })
                .unwrap();
        unsafe { ofd_buffer_free(output) };
        assert_eq!(extensions[0], extension);
        let mut invalid = extension;
        invalid["date"] = serde_json::json!("2025-02-29");
        let payload = CString::new(invalid.to_string()).unwrap();
        assert_ne!(
            unsafe { ofd_add_extension_json(handle, payload.as_ptr()) },
            0
        );
        assert_eq!(
            unsafe { document_ref(handle).unwrap() }
                .extensions()
                .unwrap()
                .len(),
            1
        );
        unsafe { ofd_close(handle) };
    }
}
