#!/usr/bin/env bash
set -euo pipefail

ofdrw_root="$(cd "$(dirname "$0")/../.." && pwd)"
ofdrw_rust_root="$ofdrw_root/ofdrw-rs"
ofdrw_tmp="$(mktemp -d)"
trap 'rm -rf "$ofdrw_tmp"' EXIT

mvn -q -f "$ofdrw_root/pom.xml" -pl ofdrw-sign -am \
  -DskipTests -Dmaven.javadoc.skip=true -Dmaven.source.skip=true package
cargo run --quiet --manifest-path "$ofdrw_rust_root/Cargo.toml" \
  -p ofdrw --example create_simple -- "$ofdrw_tmp/rust.ofd"
cargo run --quiet --manifest-path "$ofdrw_rust_root/Cargo.toml" \
  -p ofdrw --example create_custom_tag -- "$ofdrw_tmp/rust-custom-tag.ofd"
cargo run --quiet --manifest-path "$ofdrw_rust_root/Cargo.toml" \
  -p ofdrw --example duplicate_page -- \
  "$ofdrw_root/ofdrw-reader/src/test/resources/helloworld.ofd" \
  "$ofdrw_tmp/rust-duplicate-page.ofd"
cargo run --quiet --manifest-path "$ofdrw_rust_root/Cargo.toml" \
  -p ofdrw --example duplicate_dashed_path -- \
  "$ofdrw_tmp/rust-duplicate-dashed-path.ofd"
cargo run --quiet --manifest-path "$ofdrw_rust_root/Cargo.toml" \
  -p ofdrw --example create_text_stamp -- "$ofdrw_tmp/rust-text-stamp.ofd"
cargo run --quiet --manifest-path "$ofdrw_rust_root/Cargo.toml" \
  -p ofdrw --example create_pdf_annotations -- "$ofdrw_tmp/rust-pdf-annotations.ofd"
cargo run --quiet --manifest-path "$ofdrw_rust_root/Cargo.toml" \
  -p ofdrw --example create_image -- \
  "$ofdrw_root/ofdrw-reader/src/test/resources/发票示例.ofd" \
  "$ofdrw_tmp/rust-image.ofd"
cargo run --quiet --manifest-path "$ofdrw_rust_root/Cargo.toml" \
  -p ofdrw --example create_path -- "$ofdrw_tmp/rust-path.ofd"
cargo run --quiet --manifest-path "$ofdrw_rust_root/Cargo.toml" \
  -p ofdrw --example create_composite -- "$ofdrw_tmp/rust-composite.ofd" \
  "$ofdrw_root/ofdrw-reader/src/test/resources/发票示例.ofd"
cargo run --quiet --manifest-path "$ofdrw_rust_root/Cargo.toml" \
  -p ofdrw --example create_blank -- "$ofdrw_tmp/rust-blank.ofd" \
  "$ofdrw_root/ofdrw-reader/src/test/resources/helloworld.ofd" \
  "$ofdrw_tmp/rust-appended-blank.ofd"
cargo run --quiet --manifest-path "$ofdrw_rust_root/Cargo.toml" \
  -p ofdrw --example create_flow -- "$ofdrw_tmp/rust-flow.ofd"
cargo run --quiet --manifest-path "$ofdrw_rust_root/Cargo.toml" \
  -p ofdrw --example create_styled_flow -- "$ofdrw_tmp/rust-styled-flow.ofd"
cargo run --quiet --manifest-path "$ofdrw_rust_root/Cargo.toml" \
  -p ofdrw --example create_shaped_text -- "$ofdrw_tmp/rust-shaped-text.ofd"
cargo run --quiet --manifest-path "$ofdrw_rust_root/Cargo.toml" \
  -p ofdrw --example create_shaped_text -- "$ofdrw_tmp/rust-rtl-text.ofd" rtl
cargo run --quiet --manifest-path "$ofdrw_rust_root/Cargo.toml" \
  -p ofdrw --example create_shaped_text -- "$ofdrw_tmp/rust-mixed-text.ofd" mixed
cargo run --quiet --manifest-path "$ofdrw_rust_root/Cargo.toml" \
  -p ofdrw --example create_shaped_text -- "$ofdrw_tmp/rust-shaped-flow.ofd" flow
cargo run --quiet --manifest-path "$ofdrw_rust_root/Cargo.toml" \
  -p ofdrw --example create_version -- "$ofdrw_tmp/rust-version.ofd"
cargo run --quiet --manifest-path "$ofdrw_rust_root/Cargo.toml" \
  -p ofdrw --example create_table -- "$ofdrw_tmp/rust-table.ofd"
cargo run --quiet --manifest-path "$ofdrw_rust_root/Cargo.toml" \
  -p ofdrw --example create_merged_table -- "$ofdrw_tmp/rust-merged-table.ofd"
cargo run --quiet --manifest-path "$ofdrw_rust_root/Cargo.toml" \
  -p ofdrw --example create_block_table -- "$ofdrw_tmp/rust-block-table.ofd"
cargo run --quiet --manifest-path "$ofdrw_rust_root/Cargo.toml" \
  -p ofdrw --example create_template -- "$ofdrw_tmp/rust-template.ofd"
cargo run --quiet --manifest-path "$ofdrw_rust_root/Cargo.toml" \
  -p ofdrw --example import_pdf -- \
  "$ofdrw_rust_root/crates/ofdrw/tests/fixtures/horizontal-scale.pdf" \
  "$ofdrw_tmp/rust-scaled-pdf.ofd"
cargo run --quiet --manifest-path "$ofdrw_rust_root/Cargo.toml" \
  -p ofdrw --example import_pdf -- \
  "$ofdrw_rust_root/crates/ofdrw/tests/fixtures/negative-horizontal-scale.pdf" \
  "$ofdrw_tmp/rust-mirrored-pdf.ofd"
cargo run --quiet --manifest-path "$ofdrw_rust_root/Cargo.toml" \
  -p ofdrw --example import_pdf -- \
  "$ofdrw_rust_root/crates/ofdrw/tests/fixtures/text-appearance.pdf" \
  "$ofdrw_tmp/rust-text-appearance.ofd"
cargo run --quiet --manifest-path "$ofdrw_rust_root/Cargo.toml" \
  -p ofdrw --example import_pdf -- \
  "$ofdrw_rust_root/crates/ofdrw/tests/fixtures/hairline-text.pdf" \
  "$ofdrw_tmp/rust-hairline-text.ofd"
cargo run --quiet --manifest-path "$ofdrw_rust_root/Cargo.toml" \
  -p ofdrw --example import_pdf -- \
  "$ofdrw_rust_root/crates/ofdrw/tests/fixtures/direct-ocmd.pdf" \
  "$ofdrw_tmp/rust-direct-ocmd.ofd"
cargo run --quiet --manifest-path "$ofdrw_rust_root/Cargo.toml" \
  -p ofdrw --example import_pdf -- \
  "$ofdrw_rust_root/crates/ofdrw/tests/fixtures/user-unit.pdf" \
  "$ofdrw_tmp/rust-user-unit.ofd"
cargo run --quiet --manifest-path "$ofdrw_rust_root/Cargo.toml" \
  -p ofdrw --example import_pdf -- \
  "$ofdrw_rust_root/crates/ofdrw/tests/fixtures/rotate-90.pdf" \
  "$ofdrw_tmp/rust-rotated-pdf.ofd"
cargo run --quiet --manifest-path "$ofdrw_rust_root/Cargo.toml" \
  -p ofdrw --example import_pdf -- \
  "$ofdrw_rust_root/crates/ofdrw/tests/fixtures/affine-image.pdf" \
  "$ofdrw_tmp/rust-affine-image.ofd"
cargo run --quiet --manifest-path "$ofdrw_rust_root/Cargo.toml" \
  -p ofdrw --example import_pdf -- \
  "$ofdrw_rust_root/crates/ofdrw/tests/fixtures/packed-gray.pdf" \
  "$ofdrw_tmp/rust-packed-gray.ofd"
cargo run --quiet --manifest-path "$ofdrw_rust_root/Cargo.toml" \
  -p ofdrw --example import_pdf -- \
  "$ofdrw_rust_root/crates/ofdrw/tests/fixtures/text-clip.pdf" \
  "$ofdrw_tmp/rust-text-clip.ofd"
cargo run --quiet --manifest-path "$ofdrw_rust_root/Cargo.toml" \
  -p ofdrw --example import_pdf -- \
  "$ofdrw_rust_root/crates/ofdrw/tests/fixtures/soft-mask.pdf" \
  "$ofdrw_tmp/rust-soft-mask.ofd"
cargo run --quiet --manifest-path "$ofdrw_rust_root/Cargo.toml" \
  -p ofdrw --example append_existing -- \
  "$ofdrw_root/ofdrw-reader/src/test/resources/helloworld.ofd" \
  "$ofdrw_tmp/rust-appended.ofd" \
  "$ofdrw_root/ofdrw-reader/src/test/resources/发票示例.ofd" \
  "$ofdrw_root/ofdrw-reader/src/test/resources/keyword.ofd"
cargo run --quiet --manifest-path "$ofdrw_rust_root/Cargo.toml" \
  -p ofdrw --example create_font -- \
  "$ofdrw_root/ofdrw-reader/src/test/resources/keyword.ofd" \
  "$ofdrw_tmp/rust-font.ofd"
cargo run --quiet --manifest-path "$ofdrw_rust_root/Cargo.toml" \
  -p ofdrw --example create_signed -- \
  "$ofdrw_root/ofdrw-reader/src/test/resources/helloworld.ofd" \
  "$ofdrw_rust_root/crates/ofdrw/tests/fixtures/user-private-key.bin" \
  "$ofdrw_tmp/rust-signed.ofd" twice
for ofdrw_version in 4 5; do
  cargo run --quiet --manifest-path "$ofdrw_rust_root/Cargo.toml" \
    -p ofdrw --example create_ses_signed -- \
    "$ofdrw_tmp/rust-ses-v$ofdrw_version.ofd" \
    "$ofdrw_rust_root/crates/ofdrw/tests/fixtures/user-private-key.bin" \
    "$ofdrw_rust_root/crates/ofdrw/tests/fixtures/user-cert.der" \
    "$ofdrw_root/ofdrw-gm/src/test/resources/UserV$ofdrw_version.esl"
  cargo run --quiet --manifest-path "$ofdrw_rust_root/Cargo.toml" \
    -p ofdrw --example create_ses_seal -- \
    "$ofdrw_tmp/rust-generated-v$ofdrw_version.esl" \
    "$ofdrw_rust_root/crates/ofdrw/tests/fixtures/user-private-key.bin" \
    "$ofdrw_rust_root/crates/ofdrw/tests/fixtures/user-cert.der" \
    "$ofdrw_root/ofdrw-layout/src/test/resources/testimg.png" \
    "$ofdrw_version"
  cargo run --quiet --manifest-path "$ofdrw_rust_root/Cargo.toml" \
    -p ofdrw --example create_ses_signed -- \
    "$ofdrw_tmp/rust-generated-v$ofdrw_version.ofd" \
    "$ofdrw_rust_root/crates/ofdrw/tests/fixtures/user-private-key.bin" \
    "$ofdrw_rust_root/crates/ofdrw/tests/fixtures/user-cert.der" \
    "$ofdrw_tmp/rust-generated-v$ofdrw_version.esl"
done

cat > "$ofdrw_tmp/Interop.java" <<'JAVA'
import org.ofdrw.reader.OFDReader;
import org.ofdrw.reader.ContentExtractor;
import org.ofdrw.core.basicStructure.pageObj.layer.block.ImageObject;
import org.ofdrw.core.basicStructure.pageObj.layer.block.PathObject;
import org.ofdrw.core.basicStructure.pageObj.layer.block.CompositeObject;
import org.ofdrw.core.basicStructure.pageObj.layer.block.TextObject;
import org.ofdrw.core.action.EventType;
import org.ofdrw.core.action.actionType.URI;
import org.ofdrw.core.action.actionType.actionGoto.Goto;
import org.ofdrw.core.action.actionType.actionGoto.CT_Dest;
import org.ofdrw.core.basicStructure.pageObj.layer.CT_Layer;
import org.ofdrw.core.compositeObj.CT_VectorG;
import org.ofdrw.sign.verify.OFDValidator;
import org.ofdrw.sign.verify.container.DigitalValidateContainer;
import org.ofdrw.sign.verify.container.SESV4ValidateContainer;
import org.ofdrw.sign.verify.container.SESV5ValidateContainer;
import org.ofdrw.gm.cert.PKCS12Tools;
import org.bouncycastle.jce.provider.BouncyCastleProvider;
import org.dom4j.io.SAXReader;
import org.ofdrw.core.customTags.CustomTags;
import java.io.ByteArrayInputStream;
import java.nio.file.Files;
import java.nio.file.Paths;
import java.nio.charset.StandardCharsets;
import java.util.zip.ZipFile;
public class Interop {
    public static void main(String[] args) throws Exception {
        try (OFDReader reader = new OFDReader(Paths.get(args[0]))) {
            if (reader.getNumberOfPages() != 1) {
                throw new AssertionError("unexpected page count");
            }
            String text = new ContentExtractor(reader).getPageContent(1).get(0);
            if (!"你好，Rust OFD!".equals(text)) {
                throw new AssertionError("unexpected text: " + text);
            }
            if (!"positioned by Rust".equals(new ContentExtractor(reader).getPageContent(1).get(1))) {
                throw new AssertionError("Rust positioned text was not recognized");
            }
            if (!(reader.getPage(1).getContent().getLayers().get(0).getPageBlocks().get(2) instanceof PathObject)) {
                throw new AssertionError("Rust positioned path was not recognized");
            }
            TextObject link = (TextObject) reader.getPage(1).getContent().getLayers().get(0).getPageBlocks().get(0);
            if (link.getActions() == null || link.getActions().getActions().size() != 3 ||
                    link.getActions().getActions().get(0).getEvent() != EventType.CLICK ||
                    !(link.getActions().getActions().get(0).getAction() instanceof URI) ||
                    !"https://example.org/rust?x=1&y=2".equals(
                            ((URI) link.getActions().getActions().get(0).getAction()).getURI())) {
                throw new AssertionError("Rust URI click action was not recognized");
            }
            if (link.getActions().getActions().get(1).getEvent() != EventType.CLICK ||
                    !(link.getActions().getActions().get(1).getAction() instanceof Goto) ||
                    !(((Goto) link.getActions().getActions().get(1).getAction()).getTarget() instanceof CT_Dest)) {
                throw new AssertionError("Rust Goto click action was not recognized");
            }
            CT_Dest linkDest = (CT_Dest) ((Goto) link.getActions().getActions().get(1).getAction()).getTarget();
            if (linkDest.getLeft() != 12.5 || linkDest.getTop() != 21.0 || linkDest.getZoom() != 1.5) {
                throw new AssertionError("Rust Goto destination coordinates were not recognized");
            }
            if (!(link.getActions().getActions().get(2).getAction() instanceof Goto) ||
                    !(((Goto) link.getActions().getActions().get(2).getAction()).getTarget()
                            instanceof org.ofdrw.core.action.actionType.actionGoto.Bookmark) ||
                    !"Rust bookmark".equals(((org.ofdrw.core.action.actionType.actionGoto.Bookmark)
                            ((Goto) link.getActions().getActions().get(2).getAction()).getTarget()).getBookmarkName())) {
                throw new AssertionError("Rust bookmark click action was not recognized");
            }
            if (reader.getAnnotations() == null || reader.getAnnotations().getPages().size() != 1) {
                throw new AssertionError("Rust watermark annotation index was not recognized");
            }
            if (reader.getAnnotationEntities().size() != 1 ||
                    reader.getAnnotationEntities().get(0).getAnnots().size() != 1 ||
                    reader.getAnnotationEntities().get(0).getAnnots().get(0).getAppearance() == null) {
                throw new AssertionError("Rust watermark annotation appearance was not recognized");
            }
            if (reader.getAnnotationEntities().get(0).getAnnots().get(0).getLastModDate() == null) {
                throw new AssertionError("Rust watermark annotation modification date was not recognized");
            }
            if (reader.getDoc(0).getOutlines() == null ||
                    reader.getDoc(0).getOutlines().getOutlineElems().size() != 1 ||
                    reader.getDoc(0).getOutlines().getOutlineElems().get(0).getOutlineElems().size() != 1) {
                throw new AssertionError("Rust nested outlines were not recognized");
            }
            if (reader.getAttachmentList().size() != 1 ||
                    !"renamed note.txt".equals(reader.getAttachmentList().get(0).getAttachmentName())) {
                throw new AssertionError("unexpected attachment metadata");
            }
            var attachmentRecord = reader.getAttachmentList().get(0);
            if (!"text/plain".equals(attachmentRecord.getFormat()) ||
                    !java.time.LocalDateTime.of(2024, 2, 29, 12, 30, 59).equals(attachmentRecord.getCreationDateTime()) ||
                    !java.time.LocalDateTime.of(2024, 3, 1, 8, 0).equals(attachmentRecord.getModDateTime()) ||
                    attachmentRecord.getVisible() || !"source".equals(attachmentRecord.getUsage())) {
                throw new AssertionError("Rust attachment metadata was not recognized");
            }
            byte[] attachment = Files.readAllBytes(reader.getAttachmentFile("renamed note.txt"));
            if (!"hello updated".equals(new String(attachment, "UTF-8"))) {
                throw new AssertionError("unexpected attachment content");
            }
            if (reader.getDoc(0).getBookmarks() == null ||
                    reader.getDoc(0).getBookmarks().getBookmarks().size() != 1 ||
                    !"Rust bookmark".equals(reader.getDoc(0).getBookmarks().getBookmarks().get(0).getBookmarkName())) {
                throw new AssertionError("Rust bookmark was not recognized");
            }
            if (reader.getDoc(0).getBookmarks().getBookmarks().get(0).getDest().getLeft() != 10.0 ||
                    reader.getDoc(0).getBookmarks().getBookmarks().get(0).getDest().getTop() != 10.0) {
                throw new AssertionError("Rust bookmark coordinates were not recognized");
            }
            if (!"value & <OFD>".equals(reader.getOFDDir().getOfd().getDocBody()
                    .getDocInfo().getCustomDatas().getCustomDataValue("Rust key"))) {
                throw new AssertionError("Rust custom DocInfo data was not recognized");
            }
            if (!"Rust & OFD".equals(reader.getOFDDir().getOfd().getDocBody()
                    .getDocInfo().getKeywords().elements("Keyword").get(0).getText())) {
                throw new AssertionError("Rust DocInfo keyword was not recognized");
            }
        }
        try (OFDReader reader = new OFDReader(Paths.get(args[1]))) {
            if (reader.getNumberOfPages() != 1 ||
                    !(reader.getPage(1).getContent().getLayers().get(0).getPageBlocks().get(0) instanceof ImageObject)) {
                throw new AssertionError("Rust image page was not recognized");
            }
            ImageObject image = (ImageObject) reader.getPage(1).getContent().getLayers().get(0).getPageBlocks().get(0);
            if (image.getCTM() == null || image.getCTM().toDouble()[0] != 210.0 ||
                    image.getCTM().toDouble()[3] != 140.0) {
                throw new AssertionError("Rust image placement matrix was not recognized");
            }
        }
        try (OFDReader reader = new OFDReader(Paths.get(args[2]))) {
            if (reader.getNumberOfPages() != 1 ||
                    !(reader.getPage(1).getContent().getLayers().get(0).getPageBlocks().get(0) instanceof PathObject)) {
                throw new AssertionError("Rust vector path page was not recognized");
            }
        }
        try (OFDReader reader = new OFDReader(Paths.get(args[3]))) {
            if (reader.getNumberOfPages() != 4 ||
                    !"Rust added page".equals(new ContentExtractor(reader).getPageContent(2).get(0))) {
                throw new AssertionError("Rust appended page was not recognized");
            }
            if (!"Rust font on existing document".equals(
                    new ContentExtractor(reader).getPageContent(2).get(1))) {
                throw new AssertionError("Rust font on an existing document was not recognized");
            }
            if (!(reader.getPage(3).getContent().getLayers().get(0).getPageBlocks().get(0) instanceof PathObject)) {
                throw new AssertionError("Rust path page appended to an existing document was not recognized");
            }
            if (!(reader.getPage(4).getContent().getLayers().get(0).getPageBlocks().get(0) instanceof ImageObject)) {
                throw new AssertionError("Rust image page appended to an existing document was not recognized");
            }
            if (!new ContentExtractor(reader).getPageContent(1).isEmpty()) {
                throw new AssertionError("Rust page object removal was not recognized");
            }
            if (!(reader.getPage(1).getContent().getLayers().get(0).getPageBlocks().get(0) instanceof ImageObject)) {
                throw new AssertionError("Rust image object in existing document was not recognized");
            }
            if (reader.getAttachmentList().size() != 1 ||
                    !"rust-note.txt".equals(reader.getAttachmentList().get(0).getAttachmentName()) ||
                    !"Rust existing attachment".equals(
                        Files.readString(reader.getAttachmentFile("rust-note.txt")))) {
                throw new AssertionError("Rust attachment in existing document was not recognized");
            }
        }
        try (OFDReader reader = new OFDReader(Paths.get(args[4]))) {
            if (reader.getNumberOfPages() != 1 ||
                    !"Rust embedded font".equals(new ContentExtractor(reader).getPageContent(1).get(0))) {
                throw new AssertionError("Rust embedded-font page was not recognized");
            }
        }
        try (OFDReader reader = new OFDReader(Paths.get(args[5]));
             OFDValidator validator = new OFDValidator(reader)) {
            validator.setValidator(new DigitalValidateContainer(
                PKCS12Tools.ReadUserCert(Paths.get(args[6]), "private", "777777")));
            validator.exeValidate();
        }
        for (int version = 4; version <= 5; version++) {
            for (int source = 0; source < 2; source++) {
            try (OFDReader reader = new OFDReader(Paths.get(args[version + 25 + source * 2]));
                 OFDValidator validator = new OFDValidator(reader)) {
                if (reader.getNumberOfPages() != 1) {
                    throw new AssertionError("Rust SES page was not recognized");
                }
                var stamps = reader.getStampAnnots();
                if (stamps.size() != 1 || stamps.get(0).getStampAnnots().size() != 1 ||
                        !"PNG".equalsIgnoreCase(stamps.get(0).getImgType()) ||
                        stamps.get(0).getImageByte().length == 0 ||
                        stamps.get(0).getStampAnnots().get(0).getBoundary().getWidth() != 40.0) {
                    throw new AssertionError("Rust SES stamp appearance was not recognized");
                }
                validator.setValidator(version == 4
                    ? new SESV4ValidateContainer() : new SESV5ValidateContainer());
                validator.exeValidate();
            }
            }
        }
        for (int version = 4; version <= 5; version++) {
            byte[] info, issuerCert, signatureValue;
            String algorithm;
            byte[] sealBytes = Files.readAllBytes(Paths.get(args[version + 29]));
            if (version == 4) {
                var seal = org.ofdrw.gm.ses.v4.SESeal.getInstance(sealBytes);
                info = seal.geteSealInfo().getEncoded("DER");
                issuerCert = seal.getCert().getOctets();
                signatureValue = seal.getSignedValue().getOctets();
                algorithm = seal.getSignAlgID().getId();
            } else {
                var seal = org.ofdrw.gm.ses.v5.SESeal.getInstance(sealBytes);
                info = seal.geteSealInfo().getEncoded("DER");
                issuerCert = seal.getCert().getOctets();
                signatureValue = seal.getSignatureValue().getOctets();
                algorithm = seal.getSignatureAlgID().getId();
            }
            var provider = new BouncyCastleProvider();
            var certificate = java.security.cert.CertificateFactory.getInstance("X.509", provider)
                .generateCertificate(new ByteArrayInputStream(issuerCert));
            var verifier = java.security.Signature.getInstance(algorithm, provider);
            verifier.initVerify(certificate);
            verifier.update(info);
            if (!verifier.verify(signatureValue)) {
                throw new AssertionError("Rust SES seal issuer signature did not verify");
            }
        }
        try (OFDReader reader = new OFDReader(Paths.get(args[7]))) {
            CompositeObject composite = (CompositeObject) reader.getPage(1).getContent().getLayers().get(0).getPageBlocks().get(1);
            CT_VectorG unit = reader.getResMgt().getCompositeGraphicUnit(composite.getResourceID().toString());
            CompositeObject textComposite = (CompositeObject) reader.getPage(1).getContent().getLayers().get(0).getPageBlocks().get(2);
            CT_VectorG textUnit = reader.getResMgt().getCompositeGraphicUnit(textComposite.getResourceID().toString());
            CompositeObject imageComposite = (CompositeObject) reader.getPage(1).getContent().getLayers().get(0).getPageBlocks().get(3);
            CT_VectorG imageUnit = reader.getResMgt().getCompositeGraphicUnit(imageComposite.getResourceID().toString());
            if (reader.getNumberOfPages() != 1 ||
                    composite.getResourceID() == null ||
                    unit == null ||
                    unit.getContent().getPageBlocks().size() != 1 ||
                    !(unit.getContent().getPageBlocks().get(0) instanceof CT_Layer) ||
                    !(((CT_Layer) unit.getContent().getPageBlocks().get(0)).getPageBlocks().get(0) instanceof PathObject) ||
                    textUnit == null ||
                    !(((CT_Layer) textUnit.getContent().getPageBlocks().get(0)).getPageBlocks().get(0) instanceof TextObject) ||
                    imageUnit == null ||
                    !(((CT_Layer) imageUnit.getContent().getPageBlocks().get(0)).getPageBlocks().get(0) instanceof ImageObject)) {
                throw new AssertionError("Rust composite graphics were not recognized");
            }
        }
        try (OFDReader reader = new OFDReader(Paths.get(args[8]))) {
            if (reader.getNumberOfPages() != 1 ||
                    !reader.getPage(1).getContent().getLayers().get(0).getPageBlocks().isEmpty()) {
                throw new AssertionError("Rust blank page was not recognized");
            }
        }
        try (OFDReader reader = new OFDReader(Paths.get(args[9]))) {
            if (reader.getNumberOfPages() != 2 ||
                    !reader.getPage(2).getContent().getLayers().get(0).getPageBlocks().isEmpty()) {
                throw new AssertionError("Rust appended blank page was not recognized");
            }
        }
        try (OFDReader reader = new OFDReader(Paths.get(args[10]))) {
            if (reader.getNumberOfPages() != 2 ||
                    !new ContentExtractor(reader).getPageContent(1).equals(java.util.List.of(" A ", " B ")) ||
                    !new ContentExtractor(reader).getPageContent(2).equals(java.util.List.of(" C "))) {
                throw new AssertionError("Rust flowed text was not recognized");
            }
            var blocks = reader.getPage(1).getContent().getLayers().get(0).getPageBlocks();
            if (((TextObject) blocks.get(0)).getBoundary().getTopLeftX() != 10.0 ||
                    ((TextObject) blocks.get(1)).getBoundary().getTopLeftX() != 5.0) {
                throw new AssertionError("Rust first-line indent differs from Java paragraph layout");
            }
        }
        try (OFDReader reader = new OFDReader(Paths.get(args[11]))) {
            if (reader.getNumberOfPages() != 3 ||
                    !new ContentExtractor(reader).getPageContent(1).equals(java.util.List.of("AA")) ||
                    !new ContentExtractor(reader).getPageContent(2).equals(java.util.List.of("B")) ||
                    !new ContentExtractor(reader).getPageContent(3).equals(java.util.List.of("C"))) {
                throw new AssertionError("Rust styled flowed spans were not recognized");
            }
            TextObject first = (TextObject) reader.getPage(1).getContent().getLayers().get(0).getPageBlocks().get(0);
            TextObject outlined = (TextObject) reader.getPage(2).getContent().getLayers().get(0).getPageBlocks().get(0);
            if (first.getWeight().getWeight() != 600 || outlined.getFill() || !outlined.getStroke()) {
                throw new AssertionError("Rust styled text weight or outline was not recognized");
            }
        }
        try (OFDReader reader = new OFDReader(Paths.get(args[12]))) {
            if (reader.getNumberOfPages() != 1 ||
                    !new ContentExtractor(reader).getPageContent(1).equals(java.util.List.of("AfiB"))) {
                throw new AssertionError("Rust shaped text was not recognized");
            }
            TextObject shaped = (TextObject) reader.getPage(1).getContent().getLayers().get(0).getPageBlocks().get(0);
            if (shaped.getCGTransforms().stream().noneMatch(transform ->
                    transform.getCodePosition() == 1 && transform.getCodeCount() == 2 && transform.getGlyphCount() == 1)) {
                throw new AssertionError("Rust ligature mapping was not recognized");
            }
        }
        try (OFDReader reader = new OFDReader(Paths.get(args[13]))) {
            if (reader.getNumberOfPages() != 1 ||
                    !new ContentExtractor(reader).getPageContent(1).equals(java.util.List.of("سلام"))) {
                throw new AssertionError("Rust Arabic text was not recognized");
            }
            TextObject shaped = (TextObject) reader.getPage(1).getContent().getLayers().get(0).getPageBlocks().get(0);
            if (shaped.getCGTransforms().size() != 1 ||
                    shaped.getCGTransforms().get(0).getCodePosition() != 0 ||
                    shaped.getCGTransforms().get(0).getCodeCount() != 4 ||
                    shaped.getCGTransforms().get(0).getGlyphCount() >= 4) {
                throw new AssertionError("Rust right-to-left glyph mapping was not recognized");
            }
        }
        try (OFDReader reader = new OFDReader(Paths.get(args[14]))) {
            if (reader.getNumberOfPages() != 1 ||
                    !new ContentExtractor(reader).getPageContent(1).equals(java.util.List.of("AسلامB"))) {
                throw new AssertionError("Rust mixed-direction text was not recognized");
            }
            TextObject shaped = (TextObject) reader.getPage(1).getContent().getLayers().get(0).getPageBlocks().get(0);
            if (shaped.getCGTransforms().size() != 1 ||
                    shaped.getCGTransforms().get(0).getCodeCount() != 6 ||
                    shaped.getCGTransforms().get(0).getGlyphCount() != 5) {
                throw new AssertionError("Rust mixed-direction glyph mapping was not recognized");
            }
        }
        try (OFDReader reader = new OFDReader(Paths.get(args[15]))) {
            if (reader.getNumberOfPages() != 1 ||
                    !new ContentExtractor(reader).getPageContent(1).equals(java.util.List.of("fi", "fi"))) {
                throw new AssertionError("Rust shaped flow text was not recognized");
            }
            for (Object block : reader.getPage(1).getContent().getLayers().get(0).getPageBlocks()) {
                TextObject shaped = (TextObject) block;
                if (shaped.getCGTransforms().size() != 1 ||
                        shaped.getCGTransforms().get(0).getCodeCount() != 2 ||
                        shaped.getCGTransforms().get(0).getGlyphCount() != 1) {
                    throw new AssertionError("Rust shaped flow ligature was not recognized");
                }
            }
        }
        try (OFDReader reader = new OFDReader(Paths.get(args[16]))) {
            var versions = reader.getOFDDir().getOfd().getDocBody().getVersions();
            if (reader.getNumberOfPages() != 2 ||
                    versions == null || versions.getVersions().size() != 1 ||
                    !"rust-v1".equals(versions.getVersions().get(0).getID()) ||
                    !versions.getVersions().get(0).getCurrent()) {
                throw new AssertionError("Rust document version was not recognized");
            }
        }
        try (OFDReader reader = new OFDReader(Paths.get(args[17]))) {
            if (reader.getNumberOfPages() != 4 ||
                    !new ContentExtractor(reader).getPageContent(1).equals(java.util.List.of("H1", "H2", "A", "B")) ||
                    !new ContentExtractor(reader).getPageContent(2).equals(java.util.List.of("H1", "H2", "C", "D")) ||
                    !new ContentExtractor(reader).getPageContent(3).equals(java.util.List.of("H1", "H2", "A", "A2")) ||
                    !new ContentExtractor(reader).getPageContent(4).equals(java.util.List.of("H1", "H2", "B", "B2", "B3"))) {
                throw new AssertionError("Rust paginated table was not recognized");
            }
        }
        try (OFDReader reader = new OFDReader(Paths.get(args[18]))) {
            if (reader.getNumberOfPages() != 2 ||
                    reader.getPage(1).getTemplates().size() != 1 ||
                    reader.getPage(2).getTemplates().size() != 1) {
                throw new AssertionError("Rust template references were not recognized");
            }
            String templateId = reader.getPage(1).getTemplates().get(0).getTemplateID().toString();
            var template = reader.getTemplate(templateId);
            if (template == null || template.getPage().getContent() == null ||
                    template.getPage().getContent().getLayers().isEmpty()) {
                throw new AssertionError("Rust template definition was not recognized");
            }
        }
        try (OFDReader reader = new OFDReader(Paths.get(args[19]))) {
            if (reader.getNumberOfPages() != 1 ||
                    !new ContentExtractor(reader).getPageContent(1).equals(java.util.List.of("AB", "C", "D"))) {
                throw new AssertionError("Rust PDF scaled text was not recognized");
            }
            var blocks = reader.getPage(1).getContent().getLayers().get(0).getPageBlocks();
            if (blocks.size() != 3 ||
                    Math.abs(((TextObject) blocks.get(0)).getHScale() - 0.5) > 0.000001 ||
                    Math.abs(((TextObject) blocks.get(2)).getHScale() - 1.5) > 0.000001) {
                throw new AssertionError("Rust PDF horizontal scaling was not recognized");
            }
        }
        try (OFDReader reader = new OFDReader(Paths.get(args[42]))) {
            if (reader.getNumberOfPages() != 1 ||
                    !new ContentExtractor(reader).getPageContent(1).equals(java.util.List.of("AB", "C", "D"))) {
                throw new AssertionError("Rust PDF mirrored text was not recognized");
            }
            var blocks = reader.getPage(1).getContent().getLayers().get(0).getPageBlocks();
            if (blocks.size() != 3 ||
                    Math.abs(((TextObject) blocks.get(0)).getHScale() - 0.5) > 0.000001 ||
                    Math.abs(((TextObject) blocks.get(0)).getCTM().expectArr(6)[0] + 1.0) > 0.000001) {
                throw new AssertionError("Rust PDF mirrored glyph transform was not recognized");
            }
        }
        try (OFDReader reader = new OFDReader(Paths.get(args[20]))) {
            if (reader.getNumberOfPages() != 1 ||
                    !new ContentExtractor(reader).getPageContent(1).equals(java.util.List.of("A", "B", "C", "D", "E"))) {
                throw new AssertionError("Rust PDF text appearance was not recognized");
            }
            var blocks = reader.getPage(1).getContent().getLayers().get(0).getPageBlocks();
            TextObject fill = (TextObject) blocks.get(0);
            TextObject stroke = (TextObject) blocks.get(1);
            TextObject invisible = (TextObject) blocks.get(3);
            if (fill.getFillColor() == null || !fill.getFill() ||
                    stroke.getFill() || !stroke.getStroke() || stroke.getStrokeColor() == null ||
                    invisible.getFill() || invisible.getStroke()) {
                throw new AssertionError("Rust PDF text paint modes were not recognized");
            }
        }
        try (OFDReader reader = new OFDReader(Paths.get(args[21]))) {
            if (reader.getNumberOfPages() != 2 ||
                    !new ContentExtractor(reader).getPageContent(1).equals(java.util.List.of("H1", "H2", "L1", "L2", "L3", "L4", "Top", "Low")) ||
                    !new ContentExtractor(reader).getPageContent(2).equals(java.util.List.of("H1", "H2", "Wide"))) {
                throw new AssertionError("Rust merged table was not recognized");
            }
            if (reader.getPage(1).getContent().getLayers().get(0).getPageBlocks().size() < 5) {
                throw new AssertionError("Rust merged table borders were not recognized");
            }
        }
        try (OFDReader reader = new OFDReader(Paths.get(args[22]))) {
            if (reader.getNumberOfPages() != 1 ||
                    !new ContentExtractor(reader).getPageContent(1).equals(java.util.List.of("A", "B"))) {
                throw new AssertionError("Rust table content blocks were not recognized");
            }
            var blocks = reader.getPage(1).getContent().getLayers().get(0).getPageBlocks();
            if (blocks.size() != 5 || !(blocks.get(0) instanceof PathObject) ||
                    !(blocks.get(1) instanceof TextObject) || !(blocks.get(2) instanceof ImageObject) ||
                    !(blocks.get(3) instanceof TextObject) || !(blocks.get(4) instanceof ImageObject)) {
                throw new AssertionError("Rust table block order was not recognized");
            }
        }
        try (OFDReader reader = new OFDReader(Paths.get(args[23]))) {
            if (reader.getNumberOfPages() != 1 ||
                    !new ContentExtractor(reader).getPageContent(1).equals(java.util.List.of("Hi"))) {
                throw new AssertionError("Rust PDF UserUnit text was not recognized");
            }
            var blocks = reader.getPage(1).getContent().getLayers().get(0).getPageBlocks();
            if (blocks.size() != 4 || !(blocks.get(0) instanceof PathObject) ||
                    !(blocks.get(1) instanceof PathObject) || !(blocks.get(2) instanceof TextObject) ||
                    !(blocks.get(3) instanceof ImageObject)) {
                throw new AssertionError("Rust PDF UserUnit graphics were not recognized");
            }
        }
        try (OFDReader reader = new OFDReader(Paths.get(args[24]))) {
            if (reader.getNumberOfPages() != 1 ||
                    !new ContentExtractor(reader).getPageContent(1).equals(java.util.List.of("Hi"))) {
                throw new AssertionError("Rust rotated PDF text was not recognized");
            }
            var blocks = reader.getPage(1).getContent().getLayers().get(0).getPageBlocks();
            if (blocks.size() != 4 || !(blocks.get(0) instanceof PathObject) ||
                    !(blocks.get(1) instanceof PathObject) || !(blocks.get(2) instanceof TextObject) ||
                    !(blocks.get(3) instanceof ImageObject)) {
                throw new AssertionError("Rust rotated PDF graphics were not recognized");
            }
        }
        try (OFDReader reader = new OFDReader(Paths.get(args[25]))) {
            if (reader.getNumberOfPages() != 1) {
                throw new AssertionError("Rust affine PDF page was not recognized");
            }
            var blocks = reader.getPage(1).getContent().getLayers().get(0).getPageBlocks();
            if (blocks.size() != 1 || !(blocks.get(0) instanceof ImageObject)) {
                throw new AssertionError("Rust affine PDF image was not recognized");
            }
        }
        try (OFDReader reader = new OFDReader(Paths.get(args[26]))) {
            if (reader.getNumberOfPages() != 1) {
                throw new AssertionError("Rust packed-gray PDF page was not recognized");
            }
            var blocks = reader.getPage(1).getContent().getLayers().get(0).getPageBlocks();
            if (blocks.size() != 1 || !(blocks.get(0) instanceof ImageObject)) {
                throw new AssertionError("Rust packed-gray PDF image was not recognized");
            }
        }
        try (OFDReader reader = new OFDReader(Paths.get(args[27]))) {
            if (!new ContentExtractor(reader).getPageContent(1).equals(java.util.List.of("A", "B"))) {
                throw new AssertionError("Rust text-clipped PDF text was not recognized");
            }
            var blocks = reader.getPage(1).getContent().getLayers().get(0).getPageBlocks();
            if (blocks.size() != 4 || !(blocks.get(2) instanceof PathObject)) {
                throw new AssertionError("Rust text-clipped PDF graphics were not recognized");
            }
            var clips = ((PathObject) blocks.get(2)).getClips();
            if (clips == null || clips.getClips().size() != 1 ||
                    clips.getClips().get(0).getAreas().size() != 2) {
                throw new AssertionError("Rust text clip union was not recognized");
            }
        }
        try (OFDReader reader = new OFDReader(Paths.get(args[28]))) {
            var blocks = reader.getPage(1).getContent().getLayers().get(0).getPageBlocks();
            if (blocks.size() != 1 || !(blocks.get(0) instanceof ImageObject)) {
                throw new AssertionError("Rust soft-mask PDF image was not recognized");
            }
        }
        try (OFDReader reader = new OFDReader(Paths.get(args[35]))) {
            var annots = reader.getAnnotationEntities().get(0).getAnnots();
            if (annots.size() != 1 ||
                    annots.get(0).getType() != org.ofdrw.core.annotation.pageannot.AnnotType.Stamp ||
                    annots.get(0).getAppearance() == null ||
                    annots.get(0).getLastModDate() == null) {
                throw new AssertionError("Rust text Stamp annotation was not recognized");
            }
        }
        try (OFDReader reader = new OFDReader(Paths.get(args[36]))) {
            if (reader.getNumberOfPages() != 1 ||
                    !new ContentExtractor(reader).getPageContent(1).equals(java.util.List.of("Hairline"))) {
                throw new AssertionError("Rust hairline PDF text was not recognized");
            }
            var block = reader.getPage(1).getContent().getLayers().get(0).getPageBlocks().get(0);
            if (!(block instanceof TextObject) ||
                    !Double.valueOf(0.1).equals(((TextObject) block).getLineWidth())) {
                throw new AssertionError("Rust hairline PDF text stroke was not recognized");
            }
        }
        try (OFDReader reader = new OFDReader(Paths.get(args[37]))) {
            if (reader.getNumberOfPages() != 1 ||
                    !new ContentExtractor(reader).getPageContent(1).equals(java.util.List.of("Shown"))) {
                throw new AssertionError("Rust direct OCMD PDF visibility was not recognized");
            }
        }
        try (OFDReader reader = new OFDReader(Paths.get(args[38]))) {
            if (reader.getNumberOfPages() != 2 ||
                    !new ContentExtractor(reader).getPageContent(1).equals(
                            new ContentExtractor(reader).getPageContent(2))) {
                throw new AssertionError("Rust duplicated OFD page was not recognized");
            }
            var first = reader.getPage(1).getContent().getLayers().get(0).getPageBlocks().get(0);
            var second = reader.getPage(2).getContent().getLayers().get(0).getPageBlocks().get(0);
            if (!(first instanceof TextObject) || !(second instanceof TextObject) ||
                    ((TextObject) first).getID().equals(((TextObject) second).getID())) {
                throw new AssertionError("Rust duplicated page reused a graphic object ID");
            }
        }
        try (OFDReader reader = new OFDReader(Paths.get(args[39]))) {
            var blocks = reader.getPage(1).getContent().getLayers().get(0).getPageBlocks();
            if (blocks.size() != 2 || !(blocks.get(0) instanceof PathObject) ||
                    !(blocks.get(1) instanceof PathObject)) {
                throw new AssertionError("Rust duplicated dashed paths were not recognized");
            }
            PathObject first = (PathObject) blocks.get(0);
            PathObject second = (PathObject) blocks.get(1);
            if (first.getID().equals(second.getID())) {
                throw new AssertionError("Rust duplicated path reused its object ID");
            }
            for (PathObject path : new PathObject[]{first, second}) {
                var dash = path.getDashPattern();
                if (dash == null || dash.size() != 4) {
                    throw new AssertionError("Rust full dash sequence was not recognized");
                }
                for (int index = 0; index < 4; index++) {
                    if (Double.compare(dash.get(index), index + 1.0) != 0) {
                        throw new AssertionError("Rust dash sequence value changed");
                    }
                }
            }
        }
        try (OFDReader reader = new OFDReader(Paths.get(args[40]))) {
            var pages = reader.getAnnotationEntities();
            if (pages.size() != 1 || pages.get(0).getAnnots().size() != 8) {
                throw new AssertionError("Rust PDF annotation count was not recognized");
            }
            var annots = pages.get(0).getAnnots();
            String[] subtypes = {"Square", "Circle", "Text", null, null, null, null, null};
            for (int index = 0; index < annots.size(); index++) {
                var annotation = annots.get(index);
                var expectedType = index == 3
                        ? org.ofdrw.core.annotation.pageannot.AnnotType.Highlight
                        : index >= 6
                        ? org.ofdrw.core.annotation.pageannot.AnnotType.Stamp
                        : org.ofdrw.core.annotation.pageannot.AnnotType.Path;
                if (annotation.getType() != expectedType ||
                        !java.util.Objects.equals(annotation.getSubtype(), subtypes[index]) ||
                        annotation.getAppearance() == null) {
                    throw new AssertionError("Rust PDF annotation is not Java-readable: " + index);
                }
            }
            var multi = annots.get(4).getAppearance().getPageBlocks();
            if (multi.size() != 2 || !(multi.get(0) instanceof PathObject) ||
                    !(multi.get(1) instanceof PathObject)) {
                throw new AssertionError("Rust multi-path annotation was not recognized");
            }
            if (!"M 0 10 L 10 0".equals(((PathObject) multi.get(0)).getAbbreviatedData())) {
                throw new AssertionError("Rust edited annotation path was not recognized");
            }
            var mixed = annots.get(5).getAppearance().getPageBlocks();
            if (mixed.size() != 3 || !(mixed.get(0) instanceof PathObject) ||
                    !(mixed.get(1) instanceof TextObject) ||
                    !(mixed.get(2) instanceof ImageObject)) {
                throw new AssertionError("Rust mixed annotation was not recognized");
            }
            if (!"Updated & <Text>".equals(((TextObject) mixed.get(1)).getTextCodes().get(0).getContent())) {
                throw new AssertionError("Rust edited annotation text was not recognized");
            }
            var mixedStamp = annots.get(6).getAppearance().getPageBlocks();
            if (mixedStamp.size() != 3 || !(mixedStamp.get(0) instanceof PathObject) ||
                    !(mixedStamp.get(1) instanceof TextObject) ||
                    !(mixedStamp.get(2) instanceof ImageObject)) {
                throw new AssertionError("Rust mixed Stamp annotation was not recognized");
            }
            var composites = annots.get(7).getAppearance().getPageBlocks();
            if (composites.size() != 2 || !(composites.get(0) instanceof CompositeObject) ||
                    !(composites.get(1) instanceof CompositeObject)) {
                throw new AssertionError("Rust composite Stamp annotation was not recognized");
            }
        }
        try (OFDReader reader = new OFDReader(Paths.get(args[41]));
             ZipFile zip = new ZipFile(Paths.get(args[41]).toFile())) {
            if (!"CustomTags_1.xml".equals(reader.getDoc(0).getCustomTags().toString())) {
                throw new AssertionError("Rust custom tag index link was not recognized");
            }
            var listEntry = zip.getEntry("Doc_0/CustomTags_1.xml");
            if (listEntry == null) {
                throw new AssertionError("Rust custom tag index is missing");
            }
            var list = new CustomTags(new SAXReader().read(zip.getInputStream(listEntry)).getRootElement());
            if (list.getCustomTags().size() != 1) {
                throw new AssertionError("Rust custom tag count was not recognized");
            }
            var tag = list.getCustomTags().get(0);
            if (!"invoice-v2".equals(tag.getTypeID()) ||
                    !"urn:example:invoice".equals(tag.getNameSpace()) ||
                    !"/Doc_0/Tags/CustomTagReplacement_0_1.xml".equals(tag.getFileLoc().toString())) {
                throw new AssertionError("Rust custom tag metadata was not recognized");
            }
            var payload = zip.getEntry("Doc_0/Tags/CustomTagReplacement_0_1.xml");
            var former = zip.getEntry("Doc_0/Tags/CustomTag_1.xml");
            if (payload == null || former == null ||
                    !"<Invoice Version=\"2\"/>".equals(new String(zip.getInputStream(payload).readAllBytes(), StandardCharsets.UTF_8)) ||
                    !"<Invoice Version=\"1\"/>".equals(new String(zip.getInputStream(former).readAllBytes(), StandardCharsets.UTF_8))) {
                throw new AssertionError("Rust custom tag payload replacement was not recognized");
            }
        }
    }
}
JAVA

ofdrw_cp="$ofdrw_root/ofdrw-sign/target/classes:$ofdrw_root/ofdrw-reader/target/classes:$ofdrw_root/ofdrw-pkg/target/classes:$ofdrw_root/ofdrw-core/target/classes:$ofdrw_root/ofdrw-gm/target/classes:$ofdrw_root/ofdrw-gv/target/classes"
for ofdrw_dep in dom4j jaxen commons-io commons-compress zip4j bcprov-jdk18on bcpkix-jdk18on bcutil-jdk18on jbig2-imageio; do
  ofdrw_jar="$(find "$HOME/.m2/repository" -name "$ofdrw_dep-*.jar" | sort -V | tail -1)"
  if [[ -z "$ofdrw_jar" ]]; then
    echo "missing Maven dependency: $ofdrw_dep" >&2
    exit 1
  fi
  ofdrw_cp="$ofdrw_cp:$ofdrw_jar"
done

javac -encoding UTF-8 -cp "$ofdrw_cp" "$ofdrw_tmp/Interop.java"
java -cp "$ofdrw_cp:$ofdrw_tmp" Interop "$ofdrw_tmp/rust.ofd" "$ofdrw_tmp/rust-image.ofd" "$ofdrw_tmp/rust-path.ofd" "$ofdrw_tmp/rust-appended.ofd" "$ofdrw_tmp/rust-font.ofd" "$ofdrw_tmp/rust-signed.ofd" "$ofdrw_root/ofdrw-sign/src/test/resources/USER.p12" "$ofdrw_tmp/rust-composite.ofd" "$ofdrw_tmp/rust-blank.ofd" "$ofdrw_tmp/rust-appended-blank.ofd" "$ofdrw_tmp/rust-flow.ofd" "$ofdrw_tmp/rust-styled-flow.ofd" "$ofdrw_tmp/rust-shaped-text.ofd" "$ofdrw_tmp/rust-rtl-text.ofd" "$ofdrw_tmp/rust-mixed-text.ofd" "$ofdrw_tmp/rust-shaped-flow.ofd" "$ofdrw_tmp/rust-version.ofd" "$ofdrw_tmp/rust-table.ofd" "$ofdrw_tmp/rust-template.ofd" "$ofdrw_tmp/rust-scaled-pdf.ofd" "$ofdrw_tmp/rust-text-appearance.ofd" "$ofdrw_tmp/rust-merged-table.ofd" "$ofdrw_tmp/rust-block-table.ofd" "$ofdrw_tmp/rust-user-unit.ofd" "$ofdrw_tmp/rust-rotated-pdf.ofd" "$ofdrw_tmp/rust-affine-image.ofd" "$ofdrw_tmp/rust-packed-gray.ofd" "$ofdrw_tmp/rust-text-clip.ofd" "$ofdrw_tmp/rust-soft-mask.ofd" "$ofdrw_tmp/rust-ses-v4.ofd" "$ofdrw_tmp/rust-ses-v5.ofd" "$ofdrw_tmp/rust-generated-v4.ofd" "$ofdrw_tmp/rust-generated-v5.ofd" "$ofdrw_tmp/rust-generated-v4.esl" "$ofdrw_tmp/rust-generated-v5.esl" "$ofdrw_tmp/rust-text-stamp.ofd" "$ofdrw_tmp/rust-hairline-text.ofd" "$ofdrw_tmp/rust-direct-ocmd.ofd" "$ofdrw_tmp/rust-duplicate-page.ofd" "$ofdrw_tmp/rust-duplicate-dashed-path.ofd" "$ofdrw_tmp/rust-pdf-annotations.ofd" "$ofdrw_tmp/rust-custom-tag.ofd" "$ofdrw_tmp/rust-mirrored-pdf.ofd"
