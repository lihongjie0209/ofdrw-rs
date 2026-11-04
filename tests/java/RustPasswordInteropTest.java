package org.ofdrw.crypto;

import org.apache.commons.io.IOUtils;
import org.bouncycastle.crypto.CryptoException;
import org.bouncycastle.crypto.engines.SM4Engine;
import org.bouncycastle.crypto.modes.CBCBlockCipher;
import org.bouncycastle.crypto.paddings.PKCS7Padding;
import org.bouncycastle.crypto.paddings.PaddedBufferedBlockCipher;
import org.bouncycastle.crypto.params.KeyParameter;
import org.bouncycastle.crypto.params.ParametersWithIV;
import org.bouncycastle.jcajce.provider.digest.SM3;
import org.ofdrw.core.crypto.encryt.UserInfo;
import org.ofdrw.gm.support.KDF;
import org.junit.jupiter.api.Test;
import org.junit.jupiter.api.io.TempDir;
import org.ofdrw.crypto.decryptor.UserPasswordDecryptor;

import java.nio.charset.StandardCharsets;
import java.nio.file.Files;
import java.nio.file.Path;
import java.nio.file.Paths;
import java.util.Arrays;
import java.util.Enumeration;
import java.util.zip.ZipEntry;
import java.util.zip.ZipFile;

import static org.junit.jupiter.api.Assertions.assertArrayEquals;
import static org.junit.jupiter.api.Assertions.assertEquals;
import static org.junit.jupiter.api.Assertions.assertFalse;
import static org.junit.jupiter.api.Assertions.assertNotNull;
import static org.junit.jupiter.api.Assertions.assertThrows;

class RustPasswordInteropTest {

    @Test
    void rejectsWrongPasswordForRustPackage(@TempDir Path tempDir) throws Exception {
        Path encrypted = rustFixture();
        Path output = tempDir.resolve("wrong-password.ofd");
        try (OFDDecryptor decryptor = new OFDDecryptor(encrypted, output)) {
            decryptor.addUser(new UserPasswordDecryptor("bob", "wrong"));
            Exception failure = assertThrows(Exception.class, decryptor::decrypt);
            if (!(failure instanceof CryptoException)) {
                // Wrong-key CBC can have valid padding but recover a non-16-byte FEK.
                // Pinned Java rejects that in DecryptResult instead of wrapping it.
                assertEquals(IllegalArgumentException.class, failure.getClass());
                assertEquals("文件加密密钥(fek)必须为16字节", failure.getMessage());
                assertEquals("org.ofdrw.crypto.decryptor.DecryptResult",
                        failure.getStackTrace()[0].getClassName());
            }
        }
        assertFalse(Files.exists(output), "wrong password must not produce plaintext output");
    }

    @Test
    void rejectsValidPaddingWithInvalidRecoveredKeyLength() throws Exception {
        String password = "test-key-length";
        byte[] iv = new byte[16];
        byte[] kek = KDF.extend(new SM3.Digest(), password.getBytes(StandardCharsets.UTF_8), 16);
        PaddedBufferedBlockCipher cipher = new PaddedBufferedBlockCipher(
                new CBCBlockCipher(new SM4Engine()), new PKCS7Padding());
        cipher.init(true, new ParametersWithIV(new KeyParameter(kek), iv));
        byte[] invalidFek = new byte[31];
        byte[] encrypted = new byte[cipher.getOutputSize(invalidFek.length)];
        int length = cipher.processBytes(invalidFek, 0, invalidFek.length, encrypted, 0);
        length += cipher.doFinal(encrypted, length);
        UserInfo user = new UserInfo().setUserName("bob")
                .setEncryptedWK(Arrays.copyOf(encrypted, length)).setIVValue(iv);
        IllegalArgumentException failure = assertThrows(IllegalArgumentException.class,
                () -> new UserPasswordDecryptor("bob", password).decrypt(user));
        assertEquals("文件加密密钥(fek)必须为16字节", failure.getMessage());
    }

    @Test
    void decryptsRustPasswordPackage(@TempDir Path tempDir) throws Exception {
        Path encrypted = rustFixture();
        Path original = Paths.get("src/test/resources/hello.ofd");
        Path output = tempDir.resolve("decrypted.ofd");

        try (OFDDecryptor decryptor = new OFDDecryptor(encrypted, output)) {
            decryptor.addUser(new UserPasswordDecryptor("bob", "second"));
            decryptor.decrypt();
        }

        try (ZipFile expected = new ZipFile(original.toFile());
             ZipFile actual = new ZipFile(output.toFile())) {
            Enumeration<? extends ZipEntry> entries = expected.entries();
            while (entries.hasMoreElements()) {
                ZipEntry entry = entries.nextElement();
                if (entry.isDirectory()) {
                    continue;
                }
                ZipEntry restored = actual.getEntry(entry.getName());
                assertNotNull(restored, entry.getName());
                assertArrayEquals(IOUtils.toByteArray(expected.getInputStream(entry)),
                        IOUtils.toByteArray(actual.getInputStream(restored)), entry.getName());
            }
        }
    }

    private static Path rustFixture() {
        return Path.of(System.getenv().getOrDefault("OFDRW_RUST_INTEROP_DIR",
                "../ofdrw-rs/crates/ofdrw/tests/fixtures"), "rust-password-encrypted.ofd");
    }
}
