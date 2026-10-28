//! Password and SM2 certificate encryption for OFD 2.0 packages.

use std::collections::HashSet;

use base64::{engine::general_purpose::STANDARD, Engine as _};
use cbc::cipher::{block_padding::Pkcs7, BlockModeDecrypt, BlockModeEncrypt, KeyIvInit};
use der::Decode;
use sm2::{
    elliptic_curve::common::getrandom::SysRng,
    pke::{DecryptingKey, EncryptingKey},
    PublicKey, SecretKey,
};
use sm3::{Digest, Sm3};
use sm4::Sm4;
use x509_cert::Certificate;

use super::{
    child_text, escape_xml, is_element, parent_path, parse_xml, resolve_path, utf16_xml_view,
    OfdDocument, OfdError,
};

/// One user who can unlock a password-encrypted OFD package.
#[derive(Clone, PartialEq, Eq)]
pub struct PasswordRecipient {
    pub username: String,
    pub password: String,
    /// `User` or `Owner`; defaults to `User` when omitted.
    pub user_type: Option<String>,
}

impl std::fmt::Debug for PasswordRecipient {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("PasswordRecipient")
            .field("username", &self.username)
            .field("password", &"[REDACTED]")
            .field("user_type", &self.user_type)
            .finish()
    }
}

/// One user who can unlock an OFD package with an SM2 certificate.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CertificateRecipient {
    pub username: String,
    pub certificate_der: Vec<u8>,
    /// `User` or `Owner`; defaults to `User` when omitted.
    pub user_type: Option<String>,
}

/// One password or SM2 certificate user of an encrypted OFD package.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum EncryptionRecipient {
    Password(PasswordRecipient),
    Certificate(CertificateRecipient),
}

/// Public metadata for one operation in `Encryptions.xml`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EncryptionInfo {
    pub id: String,
    pub scope: String,
    pub decrypt_seed_path: String,
    pub entries_map_path: String,
    pub usernames: Vec<String>,
    pub protection_case_id: Option<String>,
}

impl OfdDocument {
    /// Whether this package contains OFD 2.0 encryption metadata.
    pub fn is_encrypted(&self) -> bool {
        self.entries.contains_key("Encryptions.xml")
    }

    /// Read encryption metadata without using a password.
    pub fn encryption_info(&self) -> Result<Vec<EncryptionInfo>, OfdError> {
        if !self.entries.contains_key("Encryptions.xml") {
            return Ok(Vec::new());
        }
        let bytes = self.required_entry("Encryptions.xml")?;
        let xml = parse_xml("Encryptions.xml", bytes)?;
        if !is_element(xml.root_element(), "Encryptions") {
            return Err(OfdError::Invalid(
                "Encryptions.xml root is not Encryptions".into(),
            ));
        }
        let mut infos = Vec::new();
        let mut seen = HashSet::new();
        for node in xml
            .root_element()
            .children()
            .filter(|node| is_element(*node, "EncryptInfo"))
        {
            let id = node
                .attribute("ID")
                .filter(|id| !id.is_empty())
                .ok_or_else(|| OfdError::Invalid("EncryptInfo has no ID".into()))?;
            if !seen.insert(id.to_owned()) {
                return Err(OfdError::Invalid(format!("duplicate EncryptInfo ID {id}")));
            }
            let decrypt_seed_path = child_text(node, "DecryptSeedLoc").ok_or_else(|| {
                OfdError::Invalid(format!("EncryptInfo {id} has no DecryptSeedLoc"))
            })?;
            let entries_map_path = child_text(node, "EntriesMapLoc").ok_or_else(|| {
                OfdError::Invalid(format!("EncryptInfo {id} has no EntriesMapLoc"))
            })?;
            let decrypt_seed_path = resolve_path("", &decrypt_seed_path)?;
            let entries_map_path = resolve_path("", &entries_map_path)?;
            let seed = parse_xml(&decrypt_seed_path, self.required_entry(&decrypt_seed_path)?)?;
            if !is_element(seed.root_element(), "DecyptSeed") {
                return Err(OfdError::Invalid(format!(
                    "{decrypt_seed_path} root is not DecyptSeed"
                )));
            }
            if seed.root_element().attribute("ID") != Some(id) {
                return Err(OfdError::Invalid(format!(
                    "{decrypt_seed_path} ID does not match EncryptInfo {id}"
                )));
            }
            self.required_entry(&entries_map_path)?;
            let usernames = seed
                .root_element()
                .children()
                .filter(|node| is_element(*node, "UserInfo"))
                .filter_map(|node| node.attribute("UserName").map(str::to_owned))
                .collect();
            infos.push(EncryptionInfo {
                id: id.to_owned(),
                scope: child_text(node, "EncryptScope").unwrap_or_default(),
                decrypt_seed_path,
                entries_map_path,
                usernames,
                protection_case_id: seed
                    .root_element()
                    .attribute("EncryptCaseId")
                    .map(str::to_owned),
            });
        }
        if infos.is_empty() {
            return Err(OfdError::Invalid(
                "Encryptions.xml has no EncryptInfo".into(),
            ));
        }
        Ok(infos)
    }

    /// Decrypt a Java-compatible SM4-CBC password-protected package.
    ///
    /// Returns a separate plaintext document; the encrypted package is unchanged.
    /// `username` may be omitted to try each password user in the seed file.
    pub fn decrypt_with_password(
        &self,
        password: &str,
        username: Option<&str>,
    ) -> Result<Self, OfdError> {
        if password.is_empty() {
            return Err(OfdError::Invalid("password is empty".into()));
        }
        let infos = self.encryption_info()?;
        if infos.is_empty() {
            return Ok(self.clone());
        }
        let mut staged = self.clone();
        for info in &infos {
            staged.decrypt_password_layer(info, password, username)?;
        }
        staged.entries.remove("Encryptions.xml");
        staged.xml_views.remove("Encryptions.xml");
        staged.doc_roots()?;
        Ok(staged)
    }

    /// Decrypt an SM2 certificate-protected package with a 32-byte raw SM2 private key.
    /// The source package is unchanged.
    pub fn decrypt_with_sm2_private_key(
        &self,
        private_key: &[u8],
        username: Option<&str>,
    ) -> Result<Self, OfdError> {
        if private_key.len() != 32 {
            return Err(OfdError::Invalid("SM2 private key must be 32 bytes".into()));
        }
        let secret = SecretKey::from_slice(private_key)
            .map_err(|_| OfdError::Invalid("invalid SM2 private key".into()))?;
        let key = DecryptingKey::new(secret);
        let infos = self.encryption_info()?;
        if infos.is_empty() {
            return Ok(self.clone());
        }
        let mut staged = self.clone();
        for info in &infos {
            if info
                .protection_case_id
                .as_deref()
                .is_some_and(|id| id != "1.1.2")
            {
                return Err(OfdError::Invalid(format!(
                    "unsupported certificate encryption protection case for {}",
                    info.id
                )));
            }
            let seed = parse_xml(
                &info.decrypt_seed_path,
                staged.required_entry(&info.decrypt_seed_path)?,
            )?;
            let mut recovered = None;
            for user in seed
                .root_element()
                .children()
                .filter(|node| is_element(*node, "UserInfo"))
            {
                if username.is_some_and(|name| user.attribute("UserName") != Some(name)) {
                    continue;
                }
                let wrapped = child_text(user, "EncryptedWK")
                    .ok_or_else(|| OfdError::Invalid("UserInfo has no EncryptedWK".into()))?;
                let wrapped = STANDARD
                    .decode(wrapped.trim())
                    .map_err(|_| OfdError::Invalid("UserInfo EncryptedWK is not base64".into()))?;
                let iv = child_text(user, "IVValue")
                    .ok_or_else(|| OfdError::Invalid("UserInfo has no IVValue".into()))?;
                let iv = STANDARD
                    .decode(iv.trim())
                    .map_err(|_| OfdError::Invalid("UserInfo IVValue is not base64".into()))?;
                let Ok(iv) = <[u8; 16]>::try_from(iv.as_slice()) else {
                    continue;
                };
                if let Ok(fek) = key.decrypt_der(&wrapped) {
                    if let Ok(fek) = <[u8; 16]>::try_from(fek.as_slice()) {
                        recovered = Some((fek, iv));
                        break;
                    }
                }
            }
            let (fek, iv) = recovered.ok_or_else(|| {
                OfdError::Invalid("SM2 private key does not match an encryption user".into())
            })?;
            staged.decrypt_layer_with_key(info, &fek, &iv)?;
        }
        staged.entries.remove("Encryptions.xml");
        staged.xml_views.remove("Encryptions.xml");
        staged.doc_roots()?;
        Ok(staged)
    }

    /// Encrypt all package entries using Java-compatible SM4-CBC password users.
    ///
    /// Returns a separate encrypted package. The source remains unchanged.
    pub fn encrypt_with_passwords(
        &self,
        recipients: &[PasswordRecipient],
    ) -> Result<Self, OfdError> {
        if recipients.is_empty() {
            return Err(OfdError::Invalid("no password recipients".into()));
        }
        let recipients = recipients
            .iter()
            .cloned()
            .map(EncryptionRecipient::Password)
            .collect::<Vec<_>>();
        self.encrypt_with_recipients(&recipients)
    }

    /// Encrypt every package entry for one or more SM2 X.509 certificate users.
    /// The source package is unchanged.
    pub fn encrypt_with_certificates(
        &self,
        recipients: &[CertificateRecipient],
    ) -> Result<Self, OfdError> {
        if recipients.is_empty() {
            return Err(OfdError::Invalid("no certificate recipients".into()));
        }
        let recipients = recipients
            .iter()
            .cloned()
            .map(EncryptionRecipient::Certificate)
            .collect::<Vec<_>>();
        self.encrypt_with_recipients(&recipients)
    }

    /// Encrypt one package for any combination of password and SM2 certificate users.
    /// Each user unlocks the same file encryption key independently.
    pub fn encrypt_with_recipients(
        &self,
        recipients: &[EncryptionRecipient],
    ) -> Result<Self, OfdError> {
        self.doc_roots()?;
        if self.is_encrypted() {
            return Err(OfdError::Invalid("package is already encrypted".into()));
        }
        if recipients.is_empty() {
            return Err(OfdError::Invalid("no encryption recipients".into()));
        }
        let mut users = HashSet::new();
        let mut has_password = false;
        let mut has_certificate = false;
        let mut public_keys = Vec::with_capacity(recipients.len());
        for recipient in recipients {
            let (username, user_type) = match recipient {
                EncryptionRecipient::Password(user) => {
                    has_password = true;
                    if user.password.is_empty() {
                        return Err(OfdError::Invalid("encryption password is empty".into()));
                    }
                    public_keys.push(None);
                    (&user.username, user.user_type.as_deref())
                }
                EncryptionRecipient::Certificate(user) => {
                    has_certificate = true;
                    let certificate = Certificate::from_der(&user.certificate_der)
                        .map_err(|_| OfdError::Invalid("invalid X.509 certificate".into()))?;
                    let key_bytes = certificate
                        .tbs_certificate()
                        .subject_public_key_info()
                        .subject_public_key
                        .raw_bytes();
                    let key = PublicKey::from_sec1_bytes(key_bytes).map_err(|_| {
                        OfdError::Invalid("certificate has no SM2 public key".into())
                    })?;
                    public_keys.push(Some(key));
                    (&user.username, user.user_type.as_deref())
                }
            };
            if username.is_empty() || !users.insert(username) {
                return Err(OfdError::Invalid(
                    "encryption usernames must be nonempty and unique".into(),
                ));
            }
            if user_type.is_some_and(|kind| kind != "Owner" && kind != "User") {
                return Err(OfdError::Invalid(
                    "encryption user_type must be Owner or User".into(),
                ));
            }
        }
        let case_id = match (has_password, has_certificate) {
            (true, false) => Some("1.1.1"),
            (false, true) => Some("1.1.2"),
            _ => None,
        };
        self.encrypt_with_seed(case_id, |fek, iv| {
            let mut users_xml = String::new();
            for (recipient, public_key) in recipients.iter().zip(&public_keys) {
                match recipient {
                    EncryptionRecipient::Password(user) => {
                        let mut hash = Sm3::new();
                        hash.update(user.password.as_bytes());
                        hash.update([0, 0, 0, 1]);
                        let digest = hash.finalize();
                        let kek: [u8; 16] = digest[..16]
                            .try_into()
                            .expect("SM3 digest is at least 16 bytes");
                        let wrapped = sm4_cbc_encrypt(&kek, iv, fek);
                        let user_type = user.user_type.as_deref().unwrap_or("User");
                        users_xml.push_str(&format!("<ofd:UserInfo UserName=\"{}\" UserType=\"{}\"><ofd:EncryptedWK>{}</ofd:EncryptedWK><ofd:IVValue>{}</ofd:IVValue></ofd:UserInfo>",
                            escape_xml(&user.username), user_type, STANDARD.encode(wrapped), STANDARD.encode(iv)));
                    }
                    EncryptionRecipient::Certificate(user) => {
                        let public_key = public_key.ok_or_else(|| {
                            OfdError::Invalid("missing validated certificate key".into())
                        })?;
                        let wrapped = EncryptingKey::new(public_key)
                            .encrypt_der(&mut SysRng, fek)
                            .map_err(|_| OfdError::Invalid("SM2 key encryption failed".into()))?;
                        let user_type = user.user_type.as_deref().unwrap_or("User");
                        users_xml.push_str(&format!("<ofd:UserInfo UserName=\"{}\" UserType=\"{}\"><ofd:UserCert>{}</ofd:UserCert><ofd:EncryptedWK>{}</ofd:EncryptedWK><ofd:IVValue>{}</ofd:IVValue></ofd:UserInfo>",
                            escape_xml(&user.username), user_type, STANDARD.encode(&user.certificate_der), STANDARD.encode(wrapped), STANDARD.encode(iv)));
                    }
                }
            }
            Ok(users_xml)
        })
    }

    fn encrypt_with_seed(
        &self,
        protection_case_id: Option<&str>,
        wrap_users: impl FnOnce(&[u8; 16], &[u8; 16]) -> Result<String, OfdError>,
    ) -> Result<Self, OfdError> {
        for path in ["Encryptions.xml", "decryptseed.dat", "entriesmap.dat"] {
            if self.entries.contains_key(path) {
                return Err(OfdError::Invalid(format!(
                    "encryption metadata entry already exists: {path}"
                )));
            }
        }
        let mut fek = [0_u8; 16];
        let mut iv = [0_u8; 16];
        getrandom::fill(&mut fek)
            .map_err(|error| OfdError::Invalid(format!("random key generation failed: {error}")))?;
        getrandom::fill(&mut iv)
            .map_err(|error| OfdError::Invalid(format!("random IV generation failed: {error}")))?;

        let mut encrypted_entries = std::collections::BTreeMap::new();
        let mut mapping = String::from("<?xml version=\"1.0\" encoding=\"UTF-8\"?>");
        mapping.push_str(&format!(
            "<ofd:EncryptEntries xmlns:ofd=\"{}\" ID=\"1\">",
            super::OFD_NS
        ));
        for (path, bytes) in &self.entries {
            let name = path.rsplit('/').next().unwrap_or(path);
            let stem = name.rsplit_once('.').map_or(name, |(stem, _)| stem);
            let stem = stem.to_lowercase();
            let parent = parent_path(path);
            let mut suffix = 0_usize;
            let encrypted_path = loop {
                let name = if suffix == 0 {
                    format!("{stem}.dat")
                } else {
                    format!("{stem}_{suffix}.dat")
                };
                let candidate = resolve_path(&parent, &name)?;
                if !self.entries.contains_key(&candidate)
                    && !encrypted_entries.contains_key(&candidate)
                    && !matches!(
                        candidate.as_str(),
                        "decryptseed.dat" | "entriesmap.dat" | "Encryptions.xml"
                    )
                {
                    break candidate;
                }
                suffix = suffix
                    .checked_add(1)
                    .ok_or_else(|| OfdError::Limit("encrypted file suffix overflow".into()))?;
            };
            let ciphertext = sm4_cbc_encrypt(&fek, &iv, bytes);
            if ciphertext.len() as u64 > super::MAX_ENTRY_BYTES {
                return Err(OfdError::Limit(format!(
                    "encrypted entry {path} is too large"
                )));
            }
            encrypted_entries.insert(encrypted_path.clone(), ciphertext);
            mapping.push_str(&format!(
                "<ofd:EncryptEntry Path=\"/{}\" EPath=\"/{}\"/>",
                escape_xml(path),
                escape_xml(&encrypted_path)
            ));
        }
        mapping.push_str("</ofd:EncryptEntries>");
        let entries_map = sm4_cbc_encrypt(&fek, &iv, mapping.as_bytes());

        let case_attribute = protection_case_id
            .map(|id| format!(" EncryptCaseId=\"{id}\""))
            .unwrap_or_default();
        let mut seed = format!("<?xml version=\"1.0\" encoding=\"UTF-8\"?><ofd:DecyptSeed xmlns:ofd=\"{}\" ID=\"1\"{}>", super::OFD_NS, case_attribute);
        seed.push_str(&wrap_users(&fek, &iv)?);
        seed.push_str("<ofd:ExtendParams/></ofd:DecyptSeed>");
        let header = format!("<?xml version=\"1.0\" encoding=\"UTF-8\"?><ofd:Encryptions xmlns:ofd=\"{}\"><ofd:EncryptInfo ID=\"1\"><ofd:Provider Company=\"ofdrw.org\" Name=\"ofdrw-rs\" Version=\"{}\"/><ofd:EncryptScope>All</ofd:EncryptScope><ofd:DecryptSeedLoc>/decryptseed.dat</ofd:DecryptSeedLoc><ofd:EntriesMapLoc>/entriesmap.dat</ofd:EntriesMapLoc></ofd:EncryptInfo></ofd:Encryptions>", super::OFD_NS, env!("CARGO_PKG_VERSION"));
        encrypted_entries.insert("Encryptions.xml".into(), header.into_bytes());
        encrypted_entries.insert("decryptseed.dat".into(), seed.into_bytes());
        encrypted_entries.insert("entriesmap.dat".into(), entries_map);
        let total: usize = encrypted_entries.values().map(Vec::len).sum();
        if encrypted_entries.len() > super::MAX_ENTRIES || total as u64 > super::MAX_TOTAL_BYTES {
            return Err(OfdError::Limit("encrypted package is too large".into()));
        }
        let mut result = self.clone();
        result.entries = encrypted_entries;
        result.xml_views = super::xml_views_for_entries(&result.entries)?;
        result.newly_created = false;
        result.created_pages.clear();
        result.created_attachments.clear();
        result.created_media.clear();
        result.created_fonts.clear();
        result.created_bookmarks.clear();
        result.created_annotations = false;
        Ok(result)
    }

    fn decrypt_password_layer(
        &mut self,
        info: &EncryptionInfo,
        password: &str,
        username: Option<&str>,
    ) -> Result<(), OfdError> {
        if info
            .protection_case_id
            .as_deref()
            .is_some_and(|id| id != "1.1.1")
        {
            return Err(OfdError::Invalid(format!(
                "unsupported encryption protection case for {}",
                info.id
            )));
        }
        let seed = parse_xml(
            &info.decrypt_seed_path,
            self.required_entry(&info.decrypt_seed_path)?,
        )?;
        let mut hash = Sm3::new();
        hash.update(password.as_bytes());
        hash.update([0, 0, 0, 1]);
        let digest = hash.finalize();
        let kek: [u8; 16] = digest[..16]
            .try_into()
            .expect("SM3 digest is at least 16 bytes");
        let mut recovered = None;
        for user in seed
            .root_element()
            .children()
            .filter(|node| is_element(*node, "UserInfo"))
        {
            if username.is_some_and(|name| user.attribute("UserName") != Some(name)) {
                continue;
            }
            let wrapped = child_text(user, "EncryptedWK")
                .ok_or_else(|| OfdError::Invalid("UserInfo has no EncryptedWK".into()))?;
            let wrapped = STANDARD
                .decode(wrapped.trim())
                .map_err(|_| OfdError::Invalid("UserInfo EncryptedWK is not base64".into()))?;
            let iv = child_text(user, "IVValue")
                .map(|value| STANDARD.decode(value.trim()))
                .transpose()
                .map_err(|_| OfdError::Invalid("UserInfo IVValue is not base64".into()))?
                .unwrap_or_else(|| vec![0; 16]);
            let Ok(iv) = <[u8; 16]>::try_from(iv.as_slice()) else {
                continue;
            };
            if let Ok(fek) = sm4_cbc_decrypt(&kek, &iv, &wrapped) {
                if let Ok(fek) = <[u8; 16]>::try_from(fek.as_slice()) {
                    recovered = Some((fek, iv));
                    break;
                }
            }
        }
        let (fek, iv) = recovered.ok_or_else(|| {
            OfdError::Invalid(
                "password does not match an encryption user or key data is invalid".into(),
            )
        })?;
        self.decrypt_layer_with_key(info, &fek, &iv)
    }

    fn decrypt_layer_with_key(
        &mut self,
        info: &EncryptionInfo,
        fek: &[u8; 16],
        iv: &[u8; 16],
    ) -> Result<(), OfdError> {
        let map_bytes = self.required_entry(&info.entries_map_path)?;
        let decrypted_map = sm4_cbc_decrypt(fek, iv, map_bytes).ok();
        let (map, map_path) = if let Some(bytes) = decrypted_map.as_deref() {
            if parse_xml(&info.entries_map_path, bytes).is_ok() {
                (bytes, info.entries_map_path.as_str())
            } else {
                (map_bytes, info.entries_map_path.as_str())
            }
        } else {
            (map_bytes, info.entries_map_path.as_str())
        };
        let xml = parse_xml(map_path, map)?;
        if !is_element(xml.root_element(), "EncryptEntries")
            || xml.root_element().attribute("ID") != Some(info.id.as_str())
        {
            return Err(OfdError::Invalid(format!(
                "invalid EncryptEntries for {}",
                info.id
            )));
        }
        let mut changes = Vec::new();
        let mut targets = HashSet::new();
        for entry in xml
            .root_element()
            .children()
            .filter(|node| is_element(*node, "EncryptEntry"))
        {
            let plain = entry
                .attribute("Path")
                .ok_or_else(|| OfdError::Invalid("EncryptEntry has no Path".into()))?;
            let cipher = entry
                .attribute("EPath")
                .ok_or_else(|| OfdError::Invalid("EncryptEntry has no EPath".into()))?;
            let plain = resolve_path("", plain)?;
            let cipher = resolve_path("", cipher)?;
            if plain == cipher || !targets.insert(plain.clone()) || plain == "Encryptions.xml" {
                return Err(OfdError::Invalid(format!(
                    "invalid encrypted entry target {plain}"
                )));
            }
            if self.entries.contains_key(&plain) {
                return Err(OfdError::Invalid(format!(
                    "plaintext entry already exists: {plain}"
                )));
            }
            let clear = sm4_cbc_decrypt(fek, iv, self.required_raw_entry(&cipher)?)?;
            if clear.len() as u64 > super::MAX_ENTRY_BYTES {
                return Err(OfdError::Limit(format!(
                    "decrypted entry {plain} is too large"
                )));
            }
            changes.push((plain, cipher, clear));
        }
        if changes.is_empty() {
            return Err(OfdError::Invalid(format!(
                "EncryptEntries {} is empty",
                info.id
            )));
        }
        for (plain, cipher, clear) in changes {
            self.entries.remove(&cipher);
            self.xml_views.remove(&cipher);
            if let Some(view) = utf16_xml_view(&plain, &clear)? {
                self.xml_views.insert(plain.clone(), view);
            }
            self.entries.insert(plain, clear);
        }
        self.entries.remove(&info.decrypt_seed_path);
        self.xml_views.remove(&info.decrypt_seed_path);
        self.entries.remove(&info.entries_map_path);
        self.xml_views.remove(&info.entries_map_path);
        let total: usize = self.entries.values().map(Vec::len).sum();
        if total as u64 > super::MAX_TOTAL_BYTES {
            return Err(OfdError::Limit("decrypted package is too large".into()));
        }
        Ok(())
    }
}

fn sm4_cbc_decrypt(key: &[u8; 16], iv: &[u8; 16], bytes: &[u8]) -> Result<Vec<u8>, OfdError> {
    if bytes.is_empty() || !bytes.len().is_multiple_of(16) {
        return Err(OfdError::Invalid(
            "SM4 ciphertext has invalid length".into(),
        ));
    }
    cbc::Decryptor::<Sm4>::new(key.into(), iv.into())
        .decrypt_padded_vec::<Pkcs7>(bytes)
        .map_err(|_| OfdError::Invalid("SM4-CBC decryption failed".into()))
}

fn sm4_cbc_encrypt(key: &[u8; 16], iv: &[u8; 16], bytes: &[u8]) -> Vec<u8> {
    cbc::Encryptor::<Sm4>::new(key.into(), iv.into()).encrypt_padded_vec::<Pkcs7>(bytes)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn recipient() -> PasswordRecipient {
        PasswordRecipient {
            username: "alice".into(),
            password: "secret".into(),
            user_type: None,
        }
    }

    #[test]
    fn password_encryption_rejects_invalid_inputs_without_mutating_source() {
        let plain = OfdDocument::new();
        assert!(!format!("{:?}", recipient()).contains("secret"));
        assert!(!plain.is_encrypted());
        assert!(plain.encryption_info().unwrap().is_empty());
        assert_eq!(
            plain.decrypt_with_password("secret", None).unwrap().entries,
            plain.entries
        );
        assert!(plain.decrypt_with_password("", None).is_err());
        assert!(plain.encrypt_with_passwords(&[]).is_err());
        let mut bad = recipient();
        bad.password.clear();
        assert!(plain.encrypt_with_passwords(&[bad]).is_err());
        let mut bad = recipient();
        bad.user_type = Some("Admin".into());
        assert!(plain.encrypt_with_passwords(&[bad]).is_err());
        assert!(plain
            .encrypt_with_passwords(&[recipient(), recipient()])
            .is_err());
        let mut reserved = plain.clone();
        reserved.set_entry("entriesmap.dat", vec![1]).unwrap();
        assert!(reserved.encrypt_with_passwords(&[recipient()]).is_err());
        assert!(!plain.is_encrypted());
    }

    #[test]
    fn encrypted_metadata_and_ciphertext_fail_closed() {
        let encrypted = OfdDocument::new()
            .encrypt_with_passwords(&[recipient()])
            .unwrap();
        let mut bad = encrypted.clone();
        bad.entries
            .insert("Encryptions.xml".into(), b"<wrong/>".to_vec());
        assert!(bad.encryption_info().is_err());
        let mut bad = encrypted.clone();
        let header = String::from_utf8(bad.entries["Encryptions.xml"].clone()).unwrap();
        bad.entries.insert(
            "Encryptions.xml".into(),
            header
                .replace("/decryptseed.dat", "/../escape")
                .into_bytes(),
        );
        assert!(bad.encryption_info().is_err());
        let mut bad = encrypted.clone();
        let seed = String::from_utf8(bad.entries["decryptseed.dat"].clone()).unwrap();
        bad.entries.insert(
            "decryptseed.dat".into(),
            seed.replace("ID=\"1\"", "ID=\"2\"").into_bytes(),
        );
        assert!(bad.encryption_info().is_err());
        let mut bad = encrypted.clone();
        bad.entries.insert(
            "decryptseed.dat".into(),
            seed.replace("EncryptCaseId=\"1.1.1\"", "EncryptCaseId=\"1.1.2\"")
                .into_bytes(),
        );
        assert!(bad.decrypt_with_password("secret", None).is_err());
        let mut bad = encrypted.clone();
        bad.entries.insert(
            "decryptseed.dat".into(),
            seed.replace("<ofd:EncryptedWK>", "<ofd:EncryptedWK>!")
                .into_bytes(),
        );
        assert!(bad.decrypt_with_password("secret", None).is_err());
        let mut bad = encrypted.clone();
        bad.entries.insert(
            "decryptseed.dat".into(),
            seed.replace("<ofd:IVValue>", "<ofd:IVValue>!").into_bytes(),
        );
        assert!(bad.decrypt_with_password("secret", None).is_err());
        let mut bad = encrypted.clone();
        bad.entries.insert("entriesmap.dat".into(), vec![0]);
        assert!(bad.decrypt_with_password("secret", None).is_err());
        let mut bad = encrypted.clone();
        let ofd_cipher = bad
            .entries
            .keys()
            .find(|path| path.starts_with("ofd") && path.ends_with(".dat"))
            .unwrap()
            .clone();
        bad.entries.insert(ofd_cipher, vec![0]);
        assert!(bad.decrypt_with_password("secret", None).is_err());
        assert!(encrypted.decrypt_with_password("wrong", None).is_err());
        assert!(encrypted
            .decrypt_with_password("secret", Some("bob"))
            .is_err());
        assert!(encrypted.decrypt_with_password("secret", None).is_ok());

        // Java also accepts a plaintext EncryptEntries map with encrypted files.
        let parsed = parse_xml("decryptseed.dat", seed.as_bytes()).unwrap();
        let user = parsed
            .root_element()
            .children()
            .find(|node| is_element(*node, "UserInfo"))
            .unwrap();
        let wrapped = STANDARD
            .decode(child_text(user, "EncryptedWK").unwrap())
            .unwrap();
        let iv: [u8; 16] = STANDARD
            .decode(child_text(user, "IVValue").unwrap())
            .unwrap()
            .try_into()
            .unwrap();
        let mut hash = Sm3::new();
        hash.update(b"secret");
        hash.update([0, 0, 0, 1]);
        let kek: [u8; 16] = hash.finalize()[..16].try_into().unwrap();
        let fek: [u8; 16] = sm4_cbc_decrypt(&kek, &iv, &wrapped)
            .unwrap()
            .try_into()
            .unwrap();
        let map = sm4_cbc_decrypt(&fek, &iv, &encrypted.entries["entriesmap.dat"]).unwrap();
        let mut plaintext_map = encrypted.clone();
        plaintext_map.entries.insert("entriesmap.dat".into(), map);
        assert!(plaintext_map.decrypt_with_password("secret", None).is_ok());
    }

    #[test]
    fn encrypted_names_avoid_plaintext_and_ciphertext_collisions() {
        let mut plain = OfdDocument::new();
        plain
            .set_entry("OFD.dat", b"existing dat file".to_vec())
            .unwrap();
        plain
            .set_entry("extra", b"extensionless file".to_vec())
            .unwrap();
        plain
            .set_entry("Doc_0/document.dat", b"nested collision".to_vec())
            .unwrap();
        let encrypted = plain.encrypt_with_passwords(&[recipient()]).unwrap();
        assert!(encrypted.entries.contains_key("ofd_1.dat"));
        assert!(encrypted.entries.contains_key("extra.dat"));
        assert!(encrypted.entries.contains_key("Doc_0/document_1.dat"));
        let restored = encrypted.decrypt_with_password("secret", None).unwrap();
        assert_eq!(restored.entries, plain.entries);
    }
}
