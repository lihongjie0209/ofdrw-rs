package org.ofdrw.crypto;

import org.junit.jupiter.api.Test;
import org.junit.jupiter.api.io.TempDir;
import org.ofdrw.crypto.integrity.GMProtectVerifier;
import org.ofdrw.crypto.integrity.OFDIntegrityVerifier;

import java.io.InputStream;
import java.nio.charset.StandardCharsets;
import java.nio.file.Files;
import java.nio.file.Path;
import java.util.Enumeration;
import java.util.zip.ZipEntry;
import java.util.zip.ZipFile;
import java.util.zip.ZipOutputStream;

import static org.junit.jupiter.api.Assertions.assertFalse;
import static org.junit.jupiter.api.Assertions.assertTrue;

class RustIntegrityInteropTest {

    @Test
    void verifiesRustIntegrityPackage() throws Exception {
        Path packagePath = rustFixture();
        assertTrue(new OFDIntegrityVerifier().integrity(packagePath, new GMProtectVerifier()));
    }

    @Test
    void rejectsExtraFileInRustIntegrityPackage(@TempDir Path tempDir) throws Exception {
        Path packagePath = rustFixture();
        Path modified = tempDir.resolve("extra-file.ofd");
        try (ZipFile original = new ZipFile(packagePath.toFile());
             ZipOutputStream output = new ZipOutputStream(Files.newOutputStream(modified))) {
            Enumeration<? extends ZipEntry> entries = original.entries();
            while (entries.hasMoreElements()) {
                ZipEntry entry = entries.nextElement();
                output.putNextEntry(new ZipEntry(entry.getName()));
                try (InputStream input = original.getInputStream(entry)) {
                    input.transferTo(output);
                }
                output.closeEntry();
            }
            output.putNextEntry(new ZipEntry("Injected.txt"));
            output.write("extra".getBytes(StandardCharsets.UTF_8));
            output.closeEntry();
        }
        assertFalse(new OFDIntegrityVerifier().integrity(modified, new GMProtectVerifier()));
    }

    private static Path rustFixture() {
        return Path.of(System.getenv().getOrDefault("OFDRW_RUST_INTEROP_DIR",
                "../ofdrw-rs/crates/ofdrw/tests/fixtures"), "rust-integrity.ofd");
    }
}
