package org.ofdrw.crypto;

import org.apache.commons.io.IOUtils;
import org.bouncycastle.crypto.params.ECPrivateKeyParameters;
import org.bouncycastle.jcajce.provider.asymmetric.util.ECUtil;
import org.junit.jupiter.api.Test;
import org.junit.jupiter.api.io.TempDir;
import org.ofdrw.crypto.decryptor.UserCertDecryptor;
import org.ofdrw.crypto.decryptor.UserPasswordDecryptor;
import org.ofdrw.crypto.enryptor.UserCertEncryptor;
import org.ofdrw.crypto.enryptor.UserPasswordEncryptor;
import org.ofdrw.gm.cert.PEMLoader;

import java.nio.file.Path;
import java.nio.file.Paths;
import java.util.Enumeration;
import java.util.zip.ZipEntry;
import java.util.zip.ZipFile;

import static org.junit.jupiter.api.Assertions.assertArrayEquals;
import static org.junit.jupiter.api.Assertions.assertNotNull;

class RustMixedInteropTest {

    @Test
    void javaWritesMixedPackage() throws Exception {
        Path source = Paths.get("src/test/resources/hello.ofd");
        Path output = Paths.get("target/java-mixed-encrypted.ofd");
        try (OFDEncryptor encryptor = new OFDEncryptor(source, output)) {
            encryptor.addUser(new UserPasswordEncryptor("java-password-user", "java-mixed-pass"));
            encryptor.addUser(new UserCertEncryptor("java-cert-user",
                    PEMLoader.loadCert(Paths.get("src/test/resources/sign_cert.pem"))));
            encryptor.encrypt();
        }
    }

    @Test
    void javaDecryptsRustMixedPackageWithEitherCredential(@TempDir Path tempDir) throws Exception {
        Path encrypted = Path.of(System.getenv().getOrDefault("OFDRW_RUST_INTEROP_DIR",
                "../ofdrw-rs/crates/ofdrw/tests/fixtures"), "rust-mixed-encrypted.ofd");
        Path original = Paths.get("src/test/resources/hello.ofd");
        Path passwordOutput = tempDir.resolve("password.ofd");
        try (OFDDecryptor decryptor = new OFDDecryptor(encrypted, passwordOutput)) {
            decryptor.addUser(new UserPasswordDecryptor("rust-password-user", "rust-mixed-pass"));
            decryptor.decrypt();
        }
        assertSameEntries(original, passwordOutput);

        Path certificateOutput = tempDir.resolve("certificate.ofd");
        ECPrivateKeyParameters key = (ECPrivateKeyParameters) ECUtil.generatePrivateKeyParameter(
                PEMLoader.loadPrivateKey(Paths.get("src/test/resources/sign_key.pem")));
        try (OFDDecryptor decryptor = new OFDDecryptor(encrypted, certificateOutput)) {
            decryptor.addUser(new UserCertDecryptor("rust-cert-user", key));
            decryptor.decrypt();
        }
        assertSameEntries(original, certificateOutput);
    }

    private static void assertSameEntries(Path original, Path restored) throws Exception {
        try (ZipFile expected = new ZipFile(original.toFile());
             ZipFile actual = new ZipFile(restored.toFile())) {
            Enumeration<? extends ZipEntry> entries = expected.entries();
            while (entries.hasMoreElements()) {
                ZipEntry entry = entries.nextElement();
                if (entry.isDirectory()) {
                    continue;
                }
                ZipEntry restoredEntry = actual.getEntry(entry.getName());
                assertNotNull(restoredEntry, entry.getName());
                assertArrayEquals(IOUtils.toByteArray(expected.getInputStream(entry)),
                        IOUtils.toByteArray(actual.getInputStream(restoredEntry)), entry.getName());
            }
        }
    }
}
