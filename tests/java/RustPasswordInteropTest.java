package org.ofdrw.crypto;

import org.apache.commons.io.IOUtils;
import org.bouncycastle.crypto.CryptoException;
import org.junit.jupiter.api.Test;
import org.junit.jupiter.api.io.TempDir;
import org.ofdrw.crypto.decryptor.UserPasswordDecryptor;

import java.nio.file.Path;
import java.nio.file.Paths;
import java.util.Enumeration;
import java.util.zip.ZipEntry;
import java.util.zip.ZipFile;

import static org.junit.jupiter.api.Assertions.assertArrayEquals;
import static org.junit.jupiter.api.Assertions.assertNotNull;
import static org.junit.jupiter.api.Assertions.assertThrows;

class RustPasswordInteropTest {

    @Test
    void rejectsWrongPasswordForRustPackage(@TempDir Path tempDir) throws Exception {
        Path encrypted = rustFixture();
        Path output = tempDir.resolve("wrong-password.ofd");
        try (OFDDecryptor decryptor = new OFDDecryptor(encrypted, output)) {
            decryptor.addUser(new UserPasswordDecryptor("bob", "wrong"));
            assertThrows(CryptoException.class, decryptor::decrypt);
        }
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
