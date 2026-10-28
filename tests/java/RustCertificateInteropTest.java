package org.ofdrw.crypto;

import org.apache.commons.io.IOUtils;
import org.bouncycastle.crypto.params.ECPrivateKeyParameters;
import org.bouncycastle.jcajce.provider.asymmetric.util.ECUtil;
import org.junit.jupiter.api.Test;
import org.junit.jupiter.api.io.TempDir;
import org.ofdrw.crypto.decryptor.UserCertDecryptor;
import org.ofdrw.gm.cert.PEMLoader;

import java.nio.file.Path;
import java.nio.file.Paths;
import java.security.PrivateKey;
import java.util.Enumeration;
import java.util.zip.ZipEntry;
import java.util.zip.ZipFile;

import static org.junit.jupiter.api.Assertions.assertArrayEquals;
import static org.junit.jupiter.api.Assertions.assertNotNull;

class RustCertificateInteropTest {

    @Test
    void decryptsRustCertificatePackage(@TempDir Path tempDir) throws Exception {
        Path encrypted = Path.of(System.getenv().getOrDefault("OFDRW_RUST_INTEROP_DIR",
                "../ofdrw-rs/crates/ofdrw/tests/fixtures"), "rust-cert-encrypted.ofd");
        Path original = Paths.get("src/test/resources/hello.ofd");
        Path output = tempDir.resolve("decrypted.ofd");
        PrivateKey privateKey = PEMLoader.loadPrivateKey(
                Paths.get("src/test/resources/sign_key.pem"));
        ECPrivateKeyParameters parameters =
                (ECPrivateKeyParameters) ECUtil.generatePrivateKeyParameter(privateKey);

        try (OFDDecryptor decryptor = new OFDDecryptor(encrypted, output)) {
            decryptor.addUser(new UserCertDecryptor("rust-cert-user", parameters));
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
}
