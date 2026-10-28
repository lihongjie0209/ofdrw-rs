// Package ofdrw calls the Rust OFD library through its versioned C ABI.
package ofdrw

import (
	"encoding/base64"
	"encoding/json"
	"errors"
	"fmt"
	"os"
	"path/filepath"
	"runtime"
	"strings"
	"sync"
	"unsafe"

	"github.com/ebitengine/purego"
)

const defaultSM2DistID = "1234567812345678"

type nativeBuffer struct {
	data *byte
	len  uintptr
}

type nativeAPI struct {
	version                         func() uint32
	lastError                       func() *byte
	open                            func(*byte, *uintptr) int32
	fromBytes                       func(*byte, uintptr, *uintptr) int32
	newDocument                     func(*uintptr) int32
	isEncrypted                     func(uintptr, *int32) int32
	encryptionInfo                  func(uintptr, *nativeBuffer) int32
	decryptWithPassword             func(uintptr, *byte, *byte, *uintptr) int32
	decryptWithSM2PrivateKey        func(uintptr, *byte, uintptr, *byte, *uintptr) int32
	encryptWithPasswords            func(uintptr, *byte, *uintptr) int32
	encryptWithCertificates         func(uintptr, *byte, *uintptr) int32
	encryptWithRecipients           func(uintptr, *byte, *uintptr) int32
	fromPDF                         func(*byte, uintptr, *uintptr) int32
	close                           func(uintptr)
	pageCount                       func(uintptr, *uintptr) int32
	appendDocumentBody              func(uintptr, *uintptr) int32
	moveDocumentBody                func(uintptr, uintptr, uintptr) int32
	removeDocumentBody              func(uintptr, uintptr) int32
	extractDocumentBody             func(uintptr, uintptr, *uintptr) int32
	appendDocumentBodiesFrom        func(uintptr, uintptr, *nativeBuffer) int32
	appendDocumentBodyFrom          func(uintptr, uintptr, uintptr, *uintptr) int32
	movePage                        func(uintptr, uintptr, uintptr) int32
	reorderDocumentPages            func(uintptr, uintptr, *byte) int32
	duplicatePage                   func(uintptr, uintptr, *uintptr) int32
	duplicatePageAt                 func(uintptr, uintptr, uintptr, *uintptr) int32
	removePage                      func(uintptr, uintptr) int32
	pages                           func(uintptr, *nativeBuffer) int32
	setPageArea                     func(uintptr, uintptr, *byte) int32
	documentPageArea                func(uintptr, uintptr, *nativeBuffer) int32
	setDocumentPageArea             func(uintptr, uintptr, *byte) int32
	validateReferences              func(uintptr, *nativeBuffer) int32
	archiveViolations               func(uintptr, *nativeBuffer) int32
	prepareArchive                  func(uintptr, *uintptr) int32
	entries                         func(uintptr, *nativeBuffer) int32
	documents                       func(uintptr, *nativeBuffer) int32
	packageInfo                     func(uintptr, *nativeBuffer) int32
	setPackageHeader                func(uintptr, *byte, *byte) int32
	documentPermissions             func(uintptr, uintptr, *nativeBuffer) int32
	setDocumentPermissions          func(uintptr, uintptr, *byte) int32
	viewPreferences                 func(uintptr, uintptr, *nativeBuffer) int32
	setViewPreferences              func(uintptr, uintptr, *byte) int32
	documentActions                 func(uintptr, uintptr, *nativeBuffer) int32
	addDocumentAction               func(uintptr, uintptr, *byte) int32
	removeDocumentAction            func(uintptr, uintptr, uintptr) int32
	pageActions                     func(uintptr, uintptr, *nativeBuffer) int32
	addPageAction                   func(uintptr, uintptr, *byte) int32
	removePageAction                func(uintptr, uintptr, uintptr) int32
	integrityManifest               func(uintptr, *nativeBuffer) int32
	checkIntegrityManifest          func(uintptr, *nativeBuffer) int32
	verifyIntegrityManifestSM2      func(uintptr, *byte, *nativeBuffer) int32
	protectIntegritySM2             func(uintptr, *byte, uintptr, *byte, uintptr, *byte) int32
	verifyIntegrityOrderedChain     func(uintptr, *byte, *nativeBuffer) int32
	verifyIntegrityPool             func(uintptr, *byte, *nativeBuffer) int32
	versions                        func(uintptr, *nativeBuffer) int32
	extractVersionSnapshot          func(uintptr, uintptr, *uintptr) int32
	addVersionSnapshot              func(uintptr, uintptr, *byte, *nativeBuffer) int32
	bookmarks                       func(uintptr, *nativeBuffer) int32
	addBookmark                     func(uintptr, uintptr, *byte) int32
	removeBookmark                  func(uintptr, uintptr, *byte) int32
	addOutline                      func(uintptr, uintptr, *byte, *uintptr) int32
	removeOutline                   func(uintptr, uintptr) int32
	setOutlineTitle                 func(uintptr, uintptr, *byte) int32
	setOutlineExpanded              func(uintptr, uintptr, int32) int32
	setOutlineDestination           func(uintptr, uintptr, *byte) int32
	setOutlineBookmark              func(uintptr, uintptr, *byte) int32
	outlines                        func(uintptr, *nativeBuffer) int32
	setInfoField                    func(uintptr, uintptr, *byte, *byte) int32
	removeInfoField                 func(uintptr, uintptr, *byte) int32
	addKeyword                      func(uintptr, uintptr, *byte) int32
	removeKeyword                   func(uintptr, uintptr, uintptr) int32
	setCustomData                   func(uintptr, uintptr, *byte, *byte) int32
	removeCustomData                func(uintptr, uintptr, *byte) int32
	pageText                        func(uintptr, uintptr, *nativeBuffer) int32
	allText                         func(uintptr, *nativeBuffer) int32
	regionText                      func(uintptr, uintptr, float64, float64, float64, float64, *nativeBuffer) int32
	keywordPositions                func(uintptr, *byte, *nativeBuffer) int32
	pageSources                     func(uintptr, uintptr, *nativeBuffer) int32
	pageLayers                      func(uintptr, uintptr, *nativeBuffer) int32
	promotePageToTemplate           func(uintptr, uintptr, *byte, *byte, *uintptr) int32
	applyPageTemplate               func(uintptr, uintptr, *byte, *byte) int32
	removePageTemplate              func(uintptr, uintptr, *byte) int32
	removeDocumentTemplate          func(uintptr, uintptr, *byte) int32
	pageObjects                     func(uintptr, uintptr, *nativeBuffer) int32
	pageObjectClips                 func(uintptr, uintptr, *nativeBuffer) int32
	pageObjectPaints                func(uintptr, uintptr, *nativeBuffer) int32
	pageObjectStrokeStyles          func(uintptr, uintptr, *nativeBuffer) int32
	pageObjectActions               func(uintptr, uintptr, *byte, *nativeBuffer) int32
	pageSourceObjectActions         func(uintptr, uintptr, *byte, *byte, *nativeBuffer) int32
	addPageObjectURIAction          func(uintptr, uintptr, *byte, *byte) int32
	addPageObjectGotoAction         func(uintptr, uintptr, *byte, *byte) int32
	addPageObjectBookmarkAction     func(uintptr, uintptr, *byte, *byte) int32
	addPageObjectAttachmentAction   func(uintptr, uintptr, *byte, *byte, int32) int32
	addPageObjectMediaAction        func(uintptr, uintptr, *byte, *byte) int32
	removePageObjectAction          func(uintptr, uintptr, *byte, uintptr) int32
	setPageObjectActionRegions      func(uintptr, uintptr, *byte, uintptr, *byte) int32
	templateText                    func(uintptr, uintptr, *nativeBuffer) int32
	textObjects                     func(uintptr, uintptr, *nativeBuffer) int32
	pagePaths                       func(uintptr, uintptr, *nativeBuffer) int32
	pageImages                      func(uintptr, uintptr, *nativeBuffer) int32
	pageMultimediaResources         func(uintptr, uintptr, *nativeBuffer) int32
	pageMultimediaData              func(uintptr, uintptr, *byte, *nativeBuffer) int32
	documentMultimediaResources     func(uintptr, uintptr, *nativeBuffer) int32
	documentMultimediaData          func(uintptr, uintptr, *byte, *nativeBuffer) int32
	compositeImages                 func(uintptr, uintptr, *nativeBuffer) int32
	compositeObjects                func(uintptr, uintptr, *nativeBuffer) int32
	compositeUnits                  func(uintptr, uintptr, *nativeBuffer) int32
	setPageCompositeUnitImages      func(uintptr, uintptr, *byte, *byte, *byte) int32
	imageData                       func(uintptr, uintptr, uintptr, *nativeBuffer) int32
	compositeImageData              func(uintptr, uintptr, uintptr, *nativeBuffer) int32
	pageSVG                         func(uintptr, uintptr, *nativeBuffer) int32
	pageSVGWithOptions              func(uintptr, uintptr, int32, *nativeBuffer) int32
	pagePNG                         func(uintptr, uintptr, float64, *nativeBuffer) int32
	pagePNGWithOptions              func(uintptr, uintptr, float64, int32, *nativeBuffer) int32
	toPDF                           func(uintptr, *nativeBuffer) int32
	toPDFWithOptions                func(uintptr, int32, *nativeBuffer) int32
	attachments                     func(uintptr, *nativeBuffer) int32
	customTags                      func(uintptr, *nativeBuffer) int32
	customTagData                   func(uintptr, uintptr, *nativeBuffer) int32
	replaceCustomTagData            func(uintptr, uintptr, *byte, uintptr, *nativeBuffer) int32
	addCustomTag                    func(uintptr, uintptr, *byte, *byte, uintptr, *nativeBuffer) int32
	updateCustomTag                 func(uintptr, uintptr, *byte, *nativeBuffer) int32
	removeCustomTag                 func(uintptr, uintptr) int32
	extensions                      func(uintptr, *nativeBuffer) int32
	addExtension                    func(uintptr, *byte) int32
	updateExtension                 func(uintptr, uintptr, *byte) int32
	removeExtension                 func(uintptr, uintptr) int32
	annotations                     func(uintptr, *nativeBuffer) int32
	annotationObjects               func(uintptr, uintptr, *byte, *nativeBuffer) int32
	appendAnnotationObjects         func(uintptr, uintptr, *byte, *byte, *nativeBuffer) int32
	updateAnnotationObject          func(uintptr, uintptr, *byte, *byte, *byte) int32
	setAnnotationObjectName         func(uintptr, uintptr, *byte, *byte, *byte) int32
	setAnnotationObjectBlendMode    func(uintptr, uintptr, *byte, *byte, *byte) int32
	setAnnotationObjectDrawParam    func(uintptr, uintptr, *byte, *byte, *byte) int32
	removeAnnotationObject          func(uintptr, uintptr, *byte, *byte) int32
	setAnnotationPathData           func(uintptr, uintptr, *byte, *byte, *byte) int32
	setAnnotationPathStrokeStyle    func(uintptr, uintptr, *byte, *byte, *byte) int32
	setAnnotationTextCode           func(uintptr, uintptr, *byte, *byte, uintptr, *byte) int32
	editAnnotationTextCode          func(uintptr, *byte) int32
	setAnnotationTextCodeLayout     func(uintptr, uintptr, *byte, *byte, uintptr, *byte) int32
	setAnnotationTextTypography     func(uintptr, uintptr, *byte, *byte, *byte) int32
	setAnnotationTextPaintFlags     func(uintptr, uintptr, *byte, *byte, *byte) int32
	appendAnnotationTextCode        func(uintptr, uintptr, *byte, *byte, *byte, *byte) int32
	removeAnnotationTextCode        func(uintptr, uintptr, *byte, *byte, uintptr) int32
	setAnnotationTextFont           func(uintptr, uintptr, *byte, *byte, *byte) int32
	setAnnotationGraphicResource    func(uintptr, uintptr, *byte, *byte, *byte) int32
	removeAnnotation                func(uintptr, uintptr, *byte) int32
	setAnnotationCreator            func(uintptr, uintptr, *byte, *byte) int32
	setAnnotationSubtype            func(uintptr, uintptr, *byte, *byte) int32
	setAnnotationLastModDate        func(uintptr, uintptr, *byte, *byte) int32
	setAnnotationAppearanceBoundary func(uintptr, uintptr, *byte, float64, float64, float64, float64) int32
	setAnnotationRemark             func(uintptr, uintptr, *byte, *byte) int32
	setAnnotationParameter          func(uintptr, uintptr, *byte, *byte, *byte) int32
	removeAnnotationParameter       func(uintptr, uintptr, *byte, *byte) int32
	setAnnotationVisible            func(uintptr, uintptr, *byte, int32) int32
	setAnnotationPrint              func(uintptr, uintptr, *byte, int32) int32
	setAnnotationFlag               func(uintptr, uintptr, *byte, *byte, int32) int32
	fonts                           func(uintptr, *nativeBuffer) int32
	fontData                        func(uintptr, uintptr, *nativeBuffer) int32
	pageFonts                       func(uintptr, uintptr, *nativeBuffer) int32
	pageFontData                    func(uintptr, uintptr, uintptr, *nativeBuffer) int32
	signatures                      func(uintptr, *nativeBuffer) int32
	signedValueData                 func(uintptr, uintptr, *nativeBuffer) int32
	sealData                        func(uintptr, uintptr, *nativeBuffer) int32
	checkReferences                 func(uintptr, uintptr, *nativeBuffer) int32
	checkDescription                func(uintptr, uintptr, *nativeBuffer) int32
	checkSealMatch                  func(uintptr, uintptr, *int32) int32
	checkSealSignature              func(uintptr, uintptr, *int32) int32
	checkSealWithID                 func(uintptr, uintptr, *byte, *int32) int32
	checkSealCertValidity           func(uintptr, uintptr, uint64, *int32) int32
	checkSealSignerAuthorization    func(uintptr, uintptr, *int32) int32
	checkSealValidity               func(uintptr, uintptr, uint64, *int32) int32
	checkSealIssuerChain            func(uintptr, uintptr, *byte, uintptr, *byte, uintptr, uint64, *byte, *int32) int32
	checkSESValue                   func(uintptr, uintptr, *int32) int32
	checkSESWithID                  func(uintptr, uintptr, *byte, *int32) int32
	checkDigitalValue               func(uintptr, uintptr, *byte, uintptr, *int32) int32
	checkDigitalWithID              func(uintptr, uintptr, *byte, uintptr, *byte, *int32) int32
	checkCertValidity               func(uintptr, uintptr, *byte, uintptr, uint64, *int32) int32
	checkDirectAnchor               func(uintptr, uintptr, *byte, uintptr, *byte, uintptr, uint64, *byte, *int32) int32
	verifyDirectAnchor              func(uintptr, uintptr, *byte, uintptr, *byte, uintptr, uint64, *byte, *nativeBuffer) int32
	verifyDirectWithID              func(uintptr, uintptr, *byte, uintptr, *byte, uintptr, uint64, *byte, *byte, *nativeBuffer) int32
	verifyOrderedChain              func(uintptr, uintptr, *byte, uintptr, *byte, uintptr, *byte, uintptr, uint64, *byte, *nativeBuffer) int32
	verifyChainWithID               func(uintptr, uintptr, *byte, uintptr, *byte, uintptr, *byte, uintptr, uint64, *byte, *byte, *nativeBuffer) int32
	verifySignaturePool             func(uintptr, uintptr, *byte, uintptr, *byte, uintptr, *byte, uintptr, uint64, *byte, *byte, *nativeBuffer) int32
	verifyTwoChains                 func(uintptr, uintptr, *byte, uintptr, *nativeBuffer) int32
	verifyTwoPools                  func(uintptr, uintptr, *byte, uintptr, *nativeBuffer) int32
	addDigitalSign                  func(uintptr, *byte, uintptr, *byte, *uintptr) int32
	addDigitalWithID                func(uintptr, *byte, uintptr, *byte, *byte, *uintptr) int32
	addSESWithIDs                   func(uintptr, *byte, uintptr, *byte, uintptr, *byte, uintptr, *byte, *byte, *byte, *uintptr) int32
	addSESWithStamp                 func(uintptr, *byte, uintptr, *byte, uintptr, *byte, uintptr, *byte, uintptr, float64, float64, float64, float64, *byte, *byte, *uintptr) int32
	createSESSeal                   func(*byte, uintptr, *byte, uintptr, *byte, uintptr, *byte, uintptr, *byte, *nativeBuffer) int32
	attachData                      func(uintptr, uintptr, *nativeBuffer) int32
	getEntry                        func(uintptr, *byte, *nativeBuffer) int32
	setEntry                        func(uintptr, *byte, *byte, uintptr) int32
	removeEntry                     func(uintptr, *byte, *int32) int32
	addTextPage                     func(uintptr, *byte, float64, float64) int32
	writeTextPage                   func(uintptr, *byte, *uintptr) int32
	addBlankPage                    func(uintptr, float64, float64) int32
	addTextObject                   func(uintptr, uintptr, *byte, *uintptr) int32
	addCompositeTextObject          func(uintptr, uintptr, *byte, *uintptr) int32
	removePageObject                func(uintptr, uintptr, *byte) int32
	duplicatePageObject             func(uintptr, uintptr, *byte, *uintptr) int32
	addPageLayer                    func(uintptr, uintptr, *byte, *byte, *uintptr) int32
	setPageLayerType                func(uintptr, uintptr, *byte, *byte) int32
	setPageLayerName                func(uintptr, uintptr, *byte, *byte) int32
	setPageLayerDrawParam           func(uintptr, uintptr, *byte, *byte) int32
	addDocumentDrawParam            func(uintptr, uintptr, *byte, *uintptr) int32
	addPageDrawParam                func(uintptr, uintptr, *byte, *uintptr) int32
	setDocumentDrawParam            func(uintptr, uintptr, *byte, *byte) int32
	setPageDrawParam                func(uintptr, uintptr, *byte, *byte) int32
	removeDocumentDrawParam         func(uintptr, uintptr, *byte) int32
	removePageDrawParam             func(uintptr, uintptr, *byte) int32
	pageColorSpaces                 func(uintptr, uintptr, *nativeBuffer) int32
	documentColorSpaces             func(uintptr, uintptr, *nativeBuffer) int32
	pageDrawParams                  func(uintptr, uintptr, *nativeBuffer) int32
	documentDrawParams              func(uintptr, uintptr, *nativeBuffer) int32
	addDocumentColorSpace           func(uintptr, uintptr, *byte, *uintptr) int32
	addPageColorSpace               func(uintptr, uintptr, *byte, *uintptr) int32
	setDocumentColorSpace           func(uintptr, uintptr, *byte, *byte) int32
	setPageColorSpace               func(uintptr, uintptr, *byte, *byte) int32
	removeDocumentColorSpace        func(uintptr, uintptr, *byte) int32
	removePageColorSpace            func(uintptr, uintptr, *byte) int32
	documentDefaultColorSpace       func(uintptr, uintptr, *nativeBuffer) int32
	setDocumentDefaultColorSpace    func(uintptr, uintptr, *byte) int32
	setPageObjectDrawParam          func(uintptr, uintptr, *byte, *byte) int32
	movePageObjectToLayer           func(uintptr, uintptr, *byte, *byte) int32
	groupPageObjects                func(uintptr, uintptr, *byte, *byte) int32
	ungroupPageObjects              func(uintptr, uintptr, *byte) int32
	movePageObject                  func(uintptr, uintptr, *byte, *byte) int32
	setPageObjectVisible            func(uintptr, uintptr, *byte, int32) int32
	setPageObjectName               func(uintptr, uintptr, *byte, *byte) int32
	setPageObjectBlendMode          func(uintptr, uintptr, *byte, *byte) int32
	setPageObjectClipRects          func(uintptr, uintptr, *byte, *byte) int32
	setPageObjectPathClips          func(uintptr, uintptr, *byte, *byte) int32
	setPageObjectClips              func(uintptr, uintptr, *byte, *byte) int32
	setPageObjectAlpha              func(uintptr, uintptr, *byte, int32) int32
	setPagePathStrokeStyle          func(uintptr, uintptr, *byte, *byte) int32
	setPagePathDashSequence         func(uintptr, uintptr, *byte, *byte) int32
	setPagePathData                 func(uintptr, uintptr, *byte, *byte) int32
	setPageTextTypography           func(uintptr, uintptr, *byte, *byte) int32
	setPageTextCode                 func(uintptr, uintptr, *byte, uintptr, *byte) int32
	editTextCode                    func(uintptr, *byte) int32
	setPageTextCodeLayout           func(uintptr, uintptr, *byte, uintptr, *byte) int32
	appendPageTextCode              func(uintptr, uintptr, *byte, *byte, *byte) int32
	removePageTextCode              func(uintptr, uintptr, *byte, uintptr) int32
	setPageTextFont                 func(uintptr, uintptr, *byte, *byte) int32
	setPageTextPaintFlags           func(uintptr, uintptr, *byte, *byte) int32
	setPageImageMask                func(uintptr, uintptr, *byte, *byte) int32
	setPageImageSubstitution        func(uintptr, uintptr, *byte, *byte) int32
	setPageImageBorder              func(uintptr, uintptr, *byte, *byte) int32
	setPageObjectGouraudFill        func(uintptr, uintptr, *byte, *byte) int32
	setPageObjectGouraudStroke      func(uintptr, uintptr, *byte, *byte) int32
	setPageObjectGradientFill       func(uintptr, uintptr, *byte, *byte) int32
	setPageObjectGradientStroke     func(uintptr, uintptr, *byte, *byte) int32
	setPageObjectPatternFill        func(uintptr, uintptr, *byte, *byte) int32
	setPageObjectPatternStroke      func(uintptr, uintptr, *byte, *byte) int32
	setPageObjectSolidFill          func(uintptr, uintptr, *byte, *byte) int32
	setPageObjectSolidStroke        func(uintptr, uintptr, *byte, *byte) int32
	setPageObjectBoundary           func(uintptr, uintptr, *byte, float64, float64, float64, float64) int32
	setPageObjectCTM                func(uintptr, uintptr, *byte, *byte) int32
	removePageLayer                 func(uintptr, uintptr, *byte) int32
	addPathObject                   func(uintptr, uintptr, *byte, *uintptr) int32
	addCompositePathObject          func(uintptr, uintptr, *byte, *uintptr) int32
	addCompositeObject              func(uintptr, uintptr, *byte, *uintptr) int32
	addTextWatermark                func(uintptr, uintptr, *byte, *uintptr) int32
	addTextStampAnnotation          func(uintptr, uintptr, *byte, *uintptr) int32
	addTextWatermarkAllPages        func(uintptr, *byte, *nativeBuffer) int32
	addImageWatermark               func(uintptr, uintptr, *byte, *uintptr) int32
	addImageStampAnnotation         func(uintptr, uintptr, *byte, *uintptr) int32
	addImageWatermarkAllPages       func(uintptr, *byte, *nativeBuffer) int32
	addPathAnnotation               func(uintptr, uintptr, *byte, *uintptr) int32
	addMultiPathAnnotation          func(uintptr, uintptr, *byte, *uintptr) int32
	addMixedAnnotation              func(uintptr, uintptr, *byte, *uintptr) int32
	addHighlightAnnotation          func(uintptr, uintptr, *byte, *uintptr) int32
	addTextNoteAnnotation           func(uintptr, uintptr, *byte, *uintptr) int32
	addShapeAnnotation              func(uintptr, uintptr, *byte, *uintptr) int32
	addLineAnnotation               func(uintptr, uintptr, *byte, *uintptr) int32
	addURILinkAnnotation            func(uintptr, uintptr, *byte, *uintptr) int32
	addGotoLinkAnnotation           func(uintptr, uintptr, *byte, *uintptr) int32
	addBookmarkLinkAnnotation       func(uintptr, uintptr, *byte, *uintptr) int32
	addAttachmentLinkAnnotation     func(uintptr, uintptr, *byte, *uintptr) int32
	addMediaLinkAnnotation          func(uintptr, uintptr, *byte, *uintptr) int32
	addTextWithFont                 func(uintptr, *byte, float64, float64, uintptr) int32
	addFont                         func(uintptr, *byte, *byte, uintptr, *uintptr) int32
	addFontToDocument               func(uintptr, uintptr, *byte, *byte, uintptr, *uintptr) int32
	addFontToPage                   func(uintptr, uintptr, *byte, *byte, uintptr, *uintptr) int32
	setFontAttribute                func(uintptr, uintptr, *byte, *byte) int32
	setPageFontAttribute            func(uintptr, uintptr, uintptr, *byte, *byte) int32
	replaceFontData                 func(uintptr, uintptr, *byte, uintptr) int32
	replacePageFontData             func(uintptr, uintptr, uintptr, *byte, uintptr) int32
	appendTextPage                  func(uintptr, uintptr, *byte, float64, float64) int32
	appendBlankPage                 func(uintptr, uintptr, float64, float64) int32
	insertTextPage                  func(uintptr, uintptr, uintptr, *byte, float64, float64, *uintptr) int32
	insertBlankPage                 func(uintptr, uintptr, uintptr, float64, float64, *uintptr) int32
	addFlowedText                   func(uintptr, uintptr, *byte, *nativeBuffer) int32
	addTable                        func(uintptr, uintptr, *byte, *nativeBuffer) int32
	addFlowedSpans                  func(uintptr, uintptr, *byte, *nativeBuffer) int32
	appendTextWithFont              func(uintptr, uintptr, *byte, float64, float64, uintptr) int32
	appendPathPage                  func(uintptr, uintptr, *byte, float64, float64, float64, float64, float64, float64, int32, int32) int32
	appendImagePage                 func(uintptr, uintptr, *byte, uintptr, *byte, float64, float64) int32
	addImagePage                    func(uintptr, *byte, uintptr, *byte, float64, float64) int32
	addImageObject                  func(uintptr, uintptr, *byte, uintptr, *byte, float64, float64, float64, float64, *uintptr) int32
	addCompositeImageObject         func(uintptr, uintptr, *byte, uintptr, *byte, float64, float64, float64, float64, *uintptr) int32
	addPathPage                     func(uintptr, *byte, float64, float64, float64, float64, float64, float64, int32, int32) int32
	addAttachment                   func(uintptr, *byte, *byte, uintptr) int32
	addMultimediaResource           func(uintptr, uintptr, *byte, *byte, *byte, uintptr, *uintptr) int32
	addPageMultimediaResource       func(uintptr, uintptr, *byte, *byte, *byte, uintptr, *uintptr) int32
	replaceDocumentMultimediaData   func(uintptr, uintptr, *byte, *byte, uintptr) int32
	replacePageMultimediaData       func(uintptr, uintptr, *byte, *byte, uintptr) int32
	removeDocumentMultimedia        func(uintptr, uintptr, *byte) int32
	removePageMultimedia            func(uintptr, uintptr, *byte) int32
	addAttachmentToDocument         func(uintptr, uintptr, *byte, *byte, uintptr) int32
	removeAttachment                func(uintptr, uintptr) int32
	renameAttachment                func(uintptr, uintptr, *byte) int32
	replaceAttachmentData           func(uintptr, uintptr, *byte, uintptr) int32
	setAttachmentMetadata           func(uintptr, uintptr, *byte) int32
	save                            func(uintptr, *byte) int32
	toBytes                         func(uintptr, *nativeBuffer) int32
	bufferFree                      func(nativeBuffer)
}

var (
	apiOnce sync.Once
	api     nativeAPI
	apiErr  error
)

func libraryName() string {
	switch runtime.GOOS {
	case "windows":
		return "ofdrw_ffi.dll"
	case "darwin":
		return "libofdrw_ffi.dylib"
	default:
		return "libofdrw_ffi.so"
	}
}

func loadAPI() error {
	apiOnce.Do(func() {
		name := libraryName()
		candidates := []string{}
		if configured := os.Getenv("OFDRW_LIB_PATH"); configured != "" {
			candidates = append(candidates, configured)
		} else {
			if _, source, _, ok := runtime.Caller(0); ok {
				root := filepath.Clean(filepath.Join(filepath.Dir(source), "..", "..", ".."))
				candidates = append(candidates, filepath.Join(root, "target", "debug", name), filepath.Join(root, "target", "release", name))
			}
			candidates = append(candidates, name)
		}
		var library uintptr
		var failures []string
		for _, candidate := range candidates {
			var err error
			library, err = openLibrary(candidate)
			if err == nil {
				break
			}
			failures = append(failures, err.Error())
		}
		if library == 0 {
			apiErr = fmt.Errorf("load ofdrw native library: %s", strings.Join(failures, "; "))
			return
		}
		bindings := []struct {
			name string
			dest any
		}{
			{"ofd_abi_version", &api.version}, {"ofd_last_error", &api.lastError},
			{"ofd_open", &api.open}, {"ofd_from_bytes", &api.fromBytes},
			{"ofd_new", &api.newDocument}, {"ofd_close", &api.close},
			{"ofd_is_encrypted", &api.isEncrypted},
			{"ofd_encryption_info_json", &api.encryptionInfo},
			{"ofd_decrypt_with_password", &api.decryptWithPassword},
			{"ofd_decrypt_with_sm2_private_key", &api.decryptWithSM2PrivateKey},
			{"ofd_encrypt_with_passwords_json", &api.encryptWithPasswords},
			{"ofd_encrypt_with_certificates_json", &api.encryptWithCertificates},
			{"ofd_encrypt_with_recipients_json", &api.encryptWithRecipients},
			{"ofd_from_pdf_bytes", &api.fromPDF},
			{"ofd_page_count", &api.pageCount}, {"ofd_page_text_json", &api.pageText},
			{"ofd_all_text_json", &api.allText},
			{"ofd_append_document_body", &api.appendDocumentBody},
			{"ofd_move_document_body", &api.moveDocumentBody},
			{"ofd_remove_document_body", &api.removeDocumentBody},
			{"ofd_extract_document_body", &api.extractDocumentBody},
			{"ofd_append_document_bodies_from_json", &api.appendDocumentBodiesFrom},
			{"ofd_append_document_body_from", &api.appendDocumentBodyFrom},
			{"ofd_page_text_in_region_json", &api.regionText},
			{"ofd_keyword_positions_json", &api.keywordPositions},
			{"ofd_move_page", &api.movePage},
			{"ofd_reorder_document_pages_json", &api.reorderDocumentPages},
			{"ofd_duplicate_page", &api.duplicatePage},
			{"ofd_duplicate_page_at", &api.duplicatePageAt},
			{"ofd_remove_page", &api.removePage},
			{"ofd_page_sources_json", &api.pageSources},
			{"ofd_page_layers_json", &api.pageLayers},
			{"ofd_promote_page_to_template", &api.promotePageToTemplate},
			{"ofd_apply_page_template", &api.applyPageTemplate},
			{"ofd_remove_page_template", &api.removePageTemplate},
			{"ofd_remove_document_template", &api.removeDocumentTemplate},
			{"ofd_page_objects_json", &api.pageObjects},
			{"ofd_page_object_clips_json", &api.pageObjectClips},
			{"ofd_page_object_paints_json", &api.pageObjectPaints},
			{"ofd_page_object_stroke_styles_json", &api.pageObjectStrokeStyles},
			{"ofd_page_object_actions_json", &api.pageObjectActions},
			{"ofd_page_source_object_actions_json", &api.pageSourceObjectActions},
			{"ofd_add_page_object_uri_action", &api.addPageObjectURIAction},
			{"ofd_add_page_object_goto_action_json", &api.addPageObjectGotoAction},
			{"ofd_add_page_object_bookmark_action", &api.addPageObjectBookmarkAction},
			{"ofd_add_page_object_attachment_action", &api.addPageObjectAttachmentAction},
			{"ofd_add_page_object_media_action_json", &api.addPageObjectMediaAction},
			{"ofd_remove_page_object_action", &api.removePageObjectAction},
			{"ofd_set_page_object_action_regions_json", &api.setPageObjectActionRegions},
			{"ofd_page_text_with_templates_json", &api.templateText},
			{"ofd_pages_json", &api.pages},
			{"ofd_set_page_area_json", &api.setPageArea},
			{"ofd_document_page_area_json", &api.documentPageArea},
			{"ofd_set_document_page_area_json", &api.setDocumentPageArea},
			{"ofd_validate_references_json", &api.validateReferences},
			{"ofd_archive_violations_json", &api.archiveViolations},
			{"ofd_prepare_archive", &api.prepareArchive},
			{"ofd_entries_json", &api.entries},
			{"ofd_documents_json", &api.documents},
			{"ofd_package_info_json", &api.packageInfo},
			{"ofd_set_package_header", &api.setPackageHeader},
			{"ofd_document_permissions_json", &api.documentPermissions},
			{"ofd_set_document_permissions_json", &api.setDocumentPermissions},
			{"ofd_view_preferences_json", &api.viewPreferences},
			{"ofd_set_view_preferences_json", &api.setViewPreferences},
			{"ofd_document_actions_json", &api.documentActions},
			{"ofd_add_document_action_json", &api.addDocumentAction},
			{"ofd_remove_document_action", &api.removeDocumentAction},
			{"ofd_page_actions_json", &api.pageActions},
			{"ofd_add_page_action_json", &api.addPageAction},
			{"ofd_remove_page_action", &api.removePageAction},
			{"ofd_integrity_manifest_json", &api.integrityManifest},
			{"ofd_check_integrity_manifest_json", &api.checkIntegrityManifest},
			{"ofd_verify_integrity_manifest_sm2_json", &api.verifyIntegrityManifestSM2},
			{"ofd_protect_integrity_sm2", &api.protectIntegritySM2},
			{"ofd_verify_integrity_manifest_sm2_with_ordered_chain_json", &api.verifyIntegrityOrderedChain},
			{"ofd_verify_integrity_manifest_sm2_with_certificate_pool_json", &api.verifyIntegrityPool},
			{"ofd_versions_json", &api.versions},
			{"ofd_extract_version_snapshot", &api.extractVersionSnapshot},
			{"ofd_add_version_snapshot_json", &api.addVersionSnapshot},
			{"ofd_bookmarks_json", &api.bookmarks},
			{"ofd_add_bookmark_json", &api.addBookmark},
			{"ofd_remove_bookmark", &api.removeBookmark},
			{"ofd_add_outline_json", &api.addOutline},
			{"ofd_remove_outline", &api.removeOutline},
			{"ofd_set_outline_title", &api.setOutlineTitle},
			{"ofd_set_outline_expanded", &api.setOutlineExpanded},
			{"ofd_set_outline_destination_json", &api.setOutlineDestination},
			{"ofd_set_outline_bookmark", &api.setOutlineBookmark},
			{"ofd_outlines_json", &api.outlines},
			{"ofd_set_document_info_field", &api.setInfoField},
			{"ofd_remove_document_info_field", &api.removeInfoField},
			{"ofd_add_document_keyword", &api.addKeyword},
			{"ofd_remove_document_keyword", &api.removeKeyword},
			{"ofd_set_document_custom_data", &api.setCustomData},
			{"ofd_remove_document_custom_data", &api.removeCustomData},
			{"ofd_page_text_objects_json", &api.textObjects},
			{"ofd_page_paths_json", &api.pagePaths},
			{"ofd_page_images_json", &api.pageImages},
			{"ofd_page_multimedia_resources_json", &api.pageMultimediaResources},
			{"ofd_page_multimedia_data", &api.pageMultimediaData},
			{"ofd_document_multimedia_resources_json", &api.documentMultimediaResources},
			{"ofd_document_multimedia_data", &api.documentMultimediaData},
			{"ofd_page_composite_images_json", &api.compositeImages},
			{"ofd_page_composite_objects_json", &api.compositeObjects},
			{"ofd_page_composite_units_json", &api.compositeUnits},
			{"ofd_set_page_composite_unit_images", &api.setPageCompositeUnitImages},
			{"ofd_page_image_data", &api.imageData},
			{"ofd_page_composite_image_data", &api.compositeImageData},
			{"ofd_page_svg", &api.pageSVG},
			{"ofd_page_svg_with_options", &api.pageSVGWithOptions},
			{"ofd_page_png", &api.pagePNG},
			{"ofd_page_png_with_options", &api.pagePNGWithOptions},
			{"ofd_to_pdf", &api.toPDF},
			{"ofd_to_pdf_with_options", &api.toPDFWithOptions},
			{"ofd_attachments_json", &api.attachments}, {"ofd_attachment_data", &api.attachData},
			{"ofd_custom_tags_json", &api.customTags},
			{"ofd_custom_tag_data", &api.customTagData},
			{"ofd_replace_custom_tag_data", &api.replaceCustomTagData},
			{"ofd_add_custom_tag_json", &api.addCustomTag},
			{"ofd_update_custom_tag_json", &api.updateCustomTag},
			{"ofd_remove_custom_tag", &api.removeCustomTag},
			{"ofd_extensions_json", &api.extensions},
			{"ofd_add_extension_json", &api.addExtension},
			{"ofd_update_extension_json", &api.updateExtension},
			{"ofd_remove_extension", &api.removeExtension},
			{"ofd_annotations_json", &api.annotations},
			{"ofd_annotation_objects_json", &api.annotationObjects},
			{"ofd_append_annotation_objects_json", &api.appendAnnotationObjects},
			{"ofd_update_annotation_object_json", &api.updateAnnotationObject},
			{"ofd_set_annotation_object_name", &api.setAnnotationObjectName},
			{"ofd_set_annotation_object_blend_mode", &api.setAnnotationObjectBlendMode},
			{"ofd_set_annotation_object_draw_param", &api.setAnnotationObjectDrawParam},
			{"ofd_remove_annotation_object", &api.removeAnnotationObject},
			{"ofd_set_annotation_path_data", &api.setAnnotationPathData},
			{"ofd_set_annotation_path_stroke_style_json", &api.setAnnotationPathStrokeStyle},
			{"ofd_set_annotation_text_code", &api.setAnnotationTextCode},
			{"ofd_edit_annotation_text_code_json", &api.editAnnotationTextCode},
			{"ofd_set_annotation_text_code_layout_json", &api.setAnnotationTextCodeLayout},
			{"ofd_set_annotation_text_typography_json", &api.setAnnotationTextTypography},
			{"ofd_set_annotation_text_paint_flags_json", &api.setAnnotationTextPaintFlags},
			{"ofd_append_annotation_text_code_json", &api.appendAnnotationTextCode},
			{"ofd_remove_annotation_text_code", &api.removeAnnotationTextCode},
			{"ofd_set_annotation_text_font", &api.setAnnotationTextFont},
			{"ofd_set_annotation_graphic_resource", &api.setAnnotationGraphicResource},
			{"ofd_remove_annotation", &api.removeAnnotation},
			{"ofd_set_annotation_creator", &api.setAnnotationCreator},
			{"ofd_set_annotation_subtype", &api.setAnnotationSubtype},
			{"ofd_set_annotation_last_mod_date", &api.setAnnotationLastModDate},
			{"ofd_set_annotation_appearance_boundary", &api.setAnnotationAppearanceBoundary},
			{"ofd_set_annotation_remark", &api.setAnnotationRemark},
			{"ofd_set_annotation_parameter", &api.setAnnotationParameter},
			{"ofd_remove_annotation_parameter", &api.removeAnnotationParameter},
			{"ofd_set_annotation_visible", &api.setAnnotationVisible},
			{"ofd_set_annotation_print", &api.setAnnotationPrint},
			{"ofd_set_annotation_flag", &api.setAnnotationFlag},
			{"ofd_fonts_json", &api.fonts}, {"ofd_font_data", &api.fontData},
			{"ofd_page_fonts_json", &api.pageFonts}, {"ofd_page_font_data", &api.pageFontData},
			{"ofd_signatures_json", &api.signatures},
			{"ofd_signed_value_data", &api.signedValueData},
			{"ofd_seal_data", &api.sealData},
			{"ofd_check_signature_references_json", &api.checkReferences},
			{"ofd_check_signature_description_hash_json", &api.checkDescription},
			{"ofd_check_signature_seal_match", &api.checkSealMatch},
			{"ofd_check_ses_seal_signature", &api.checkSealSignature},
			{"ofd_check_ses_seal_signature_with_distid", &api.checkSealWithID},
			{"ofd_check_ses_seal_certificate_validity", &api.checkSealCertValidity},
			{"ofd_check_ses_seal_signer_authorization", &api.checkSealSignerAuthorization},
			{"ofd_check_ses_seal_validity", &api.checkSealValidity},
			{"ofd_check_ses_seal_issuer_ordered_sm2_chain", &api.checkSealIssuerChain},
			{"ofd_check_ses_signature_value", &api.checkSESValue},
			{"ofd_check_ses_signature_value_with_distid", &api.checkSESWithID},
			{"ofd_check_digital_signature_with_certificate", &api.checkDigitalValue},
			{"ofd_check_digital_signature_with_certificate_and_distid", &api.checkDigitalWithID},
			{"ofd_check_signature_certificate_validity", &api.checkCertValidity},
			{"ofd_check_signature_direct_trust_anchor", &api.checkDirectAnchor},
			{"ofd_verify_signature_with_direct_trust_anchor_json", &api.verifyDirectAnchor},
			{"ofd_verify_signature_with_direct_trust_anchor_and_distids_json", &api.verifyDirectWithID},
			{"ofd_verify_signature_with_ordered_sm2_chain_json", &api.verifyOrderedChain},
			{"ofd_verify_signature_with_ordered_sm2_chain_and_distids_json", &api.verifyChainWithID},
			{"ofd_verify_signature_with_certificate_pool_json", &api.verifySignaturePool},
			{"ofd_verify_signature_with_ordered_sm2_chains_json", &api.verifyTwoChains},
			{"ofd_verify_signature_with_certificate_pools_json", &api.verifyTwoPools},
			{"ofd_add_digital_signature", &api.addDigitalSign},
			{"ofd_add_digital_signature_with_distid", &api.addDigitalWithID},
			{"ofd_add_ses_signature_with_distids", &api.addSESWithIDs},
			{"ofd_add_ses_signature_with_stamp_and_distids", &api.addSESWithStamp},
			{"ofd_create_ses_seal_json", &api.createSESSeal},
			{"ofd_get_entry", &api.getEntry}, {"ofd_set_entry", &api.setEntry}, {"ofd_remove_entry", &api.removeEntry},
			{"ofd_add_text_page", &api.addTextPage}, {"ofd_save", &api.save},
			{"ofd_write_text_page_json", &api.writeTextPage},
			{"ofd_to_bytes", &api.toBytes},
			{"ofd_add_blank_page", &api.addBlankPage},
			{"ofd_add_text_object_json", &api.addTextObject},
			{"ofd_add_composite_text_object_json", &api.addCompositeTextObject},
			{"ofd_remove_page_object", &api.removePageObject},
			{"ofd_duplicate_page_object", &api.duplicatePageObject},
			{"ofd_add_page_layer", &api.addPageLayer},
			{"ofd_set_page_layer_type", &api.setPageLayerType},
			{"ofd_set_page_layer_name", &api.setPageLayerName},
			{"ofd_set_page_layer_draw_param", &api.setPageLayerDrawParam},
			{"ofd_add_document_draw_param_json", &api.addDocumentDrawParam},
			{"ofd_add_page_draw_param_json", &api.addPageDrawParam},
			{"ofd_set_document_draw_param_json", &api.setDocumentDrawParam},
			{"ofd_set_page_draw_param_json", &api.setPageDrawParam},
			{"ofd_remove_document_draw_param", &api.removeDocumentDrawParam},
			{"ofd_remove_page_draw_param", &api.removePageDrawParam},
			{"ofd_page_color_spaces_json", &api.pageColorSpaces},
			{"ofd_document_color_spaces_json", &api.documentColorSpaces},
			{"ofd_page_draw_params_json", &api.pageDrawParams},
			{"ofd_document_draw_params_json", &api.documentDrawParams},
			{"ofd_add_document_color_space_json", &api.addDocumentColorSpace},
			{"ofd_add_page_color_space_json", &api.addPageColorSpace},
			{"ofd_set_document_color_space_json", &api.setDocumentColorSpace},
			{"ofd_set_page_color_space_json", &api.setPageColorSpace},
			{"ofd_remove_document_color_space", &api.removeDocumentColorSpace},
			{"ofd_remove_page_color_space", &api.removePageColorSpace},
			{"ofd_document_default_color_space_json", &api.documentDefaultColorSpace},
			{"ofd_set_document_default_color_space", &api.setDocumentDefaultColorSpace},
			{"ofd_set_page_object_draw_param", &api.setPageObjectDrawParam},
			{"ofd_move_page_object_to_layer", &api.movePageObjectToLayer},
			{"ofd_group_page_objects", &api.groupPageObjects},
			{"ofd_ungroup_page_objects", &api.ungroupPageObjects},
			{"ofd_move_page_object", &api.movePageObject},
			{"ofd_set_page_object_visible", &api.setPageObjectVisible},
			{"ofd_set_page_object_name", &api.setPageObjectName},
			{"ofd_set_page_object_blend_mode", &api.setPageObjectBlendMode},
			{"ofd_set_page_object_clip_rects_json", &api.setPageObjectClipRects},
			{"ofd_set_page_object_path_clips_json", &api.setPageObjectPathClips},
			{"ofd_set_page_object_clips_json", &api.setPageObjectClips},
			{"ofd_set_page_object_alpha", &api.setPageObjectAlpha},
			{"ofd_set_page_path_stroke_style_json", &api.setPagePathStrokeStyle},
			{"ofd_set_page_path_dash_sequence_json", &api.setPagePathDashSequence},
			{"ofd_set_page_path_data", &api.setPagePathData},
			{"ofd_set_page_text_typography_json", &api.setPageTextTypography},
			{"ofd_set_page_text_code", &api.setPageTextCode},
			{"ofd_edit_text_code_json", &api.editTextCode},
			{"ofd_set_page_text_code_layout_json", &api.setPageTextCodeLayout},
			{"ofd_append_page_text_code_json", &api.appendPageTextCode},
			{"ofd_remove_page_text_code", &api.removePageTextCode},
			{"ofd_set_page_text_font", &api.setPageTextFont},
			{"ofd_set_page_text_paint_flags_json", &api.setPageTextPaintFlags},
			{"ofd_set_page_image_mask", &api.setPageImageMask},
			{"ofd_set_page_image_substitution", &api.setPageImageSubstitution},
			{"ofd_set_page_image_border_json", &api.setPageImageBorder},
			{"ofd_set_page_object_gouraud_fill_json", &api.setPageObjectGouraudFill},
			{"ofd_set_page_object_gouraud_stroke_json", &api.setPageObjectGouraudStroke},
			{"ofd_set_page_object_gradient_fill_json", &api.setPageObjectGradientFill},
			{"ofd_set_page_object_gradient_stroke_json", &api.setPageObjectGradientStroke},
			{"ofd_set_page_object_pattern_fill_json", &api.setPageObjectPatternFill},
			{"ofd_set_page_object_pattern_stroke_json", &api.setPageObjectPatternStroke},
			{"ofd_set_page_object_solid_fill_json", &api.setPageObjectSolidFill},
			{"ofd_set_page_object_solid_stroke_json", &api.setPageObjectSolidStroke},
			{"ofd_set_page_object_boundary", &api.setPageObjectBoundary},
			{"ofd_set_page_object_ctm_json", &api.setPageObjectCTM},
			{"ofd_remove_page_layer", &api.removePageLayer},
			{"ofd_add_path_object_json", &api.addPathObject},
			{"ofd_add_composite_path_object_json", &api.addCompositePathObject},
			{"ofd_add_composite_object_json", &api.addCompositeObject},
			{"ofd_add_text_watermark_json", &api.addTextWatermark},
			{"ofd_add_text_stamp_annotation_json", &api.addTextStampAnnotation},
			{"ofd_add_text_watermark_all_pages_json", &api.addTextWatermarkAllPages},
			{"ofd_add_image_watermark_json", &api.addImageWatermark},
			{"ofd_add_image_stamp_annotation_json", &api.addImageStampAnnotation},
			{"ofd_add_image_watermark_all_pages_json", &api.addImageWatermarkAllPages},
			{"ofd_add_path_annotation_json", &api.addPathAnnotation},
			{"ofd_add_multi_path_annotation_json", &api.addMultiPathAnnotation},
			{"ofd_add_mixed_annotation_json", &api.addMixedAnnotation},
			{"ofd_add_highlight_annotation_json", &api.addHighlightAnnotation},
			{"ofd_add_text_note_annotation_json", &api.addTextNoteAnnotation},
			{"ofd_add_shape_annotation_json", &api.addShapeAnnotation},
			{"ofd_add_line_annotation_json", &api.addLineAnnotation},
			{"ofd_add_uri_link_annotation_json", &api.addURILinkAnnotation},
			{"ofd_add_goto_link_annotation_json", &api.addGotoLinkAnnotation},
			{"ofd_add_bookmark_link_annotation_json", &api.addBookmarkLinkAnnotation},
			{"ofd_add_attachment_link_annotation_json", &api.addAttachmentLinkAnnotation},
			{"ofd_add_media_link_annotation_json", &api.addMediaLinkAnnotation},
			{"ofd_add_text_page_with_font", &api.addTextWithFont},
			{"ofd_add_embedded_font", &api.addFont},
			{"ofd_add_embedded_font_to_document", &api.addFontToDocument},
			{"ofd_add_embedded_font_to_page", &api.addFontToPage},
			{"ofd_set_font_attribute", &api.setFontAttribute},
			{"ofd_set_page_font_attribute", &api.setPageFontAttribute},
			{"ofd_replace_font_data", &api.replaceFontData},
			{"ofd_replace_page_font_data", &api.replacePageFontData},
			{"ofd_append_text_page", &api.appendTextPage},
			{"ofd_append_blank_page", &api.appendBlankPage},
			{"ofd_insert_text_page", &api.insertTextPage},
			{"ofd_insert_blank_page", &api.insertBlankPage},
			{"ofd_add_flowed_text_json", &api.addFlowedText},
			{"ofd_add_table_json", &api.addTable},
			{"ofd_add_flowed_spans_json", &api.addFlowedSpans},
			{"ofd_append_text_page_with_font", &api.appendTextWithFont},
			{"ofd_append_path_page", &api.appendPathPage},
			{"ofd_append_image_page", &api.appendImagePage},
			{"ofd_add_image_page", &api.addImagePage},
			{"ofd_add_image_object", &api.addImageObject},
			{"ofd_add_composite_image_object", &api.addCompositeImageObject},
			{"ofd_add_path_page", &api.addPathPage},
			{"ofd_add_attachment", &api.addAttachment},
			{"ofd_add_multimedia_resource", &api.addMultimediaResource},
			{"ofd_add_page_multimedia_resource", &api.addPageMultimediaResource},
			{"ofd_replace_document_multimedia_data", &api.replaceDocumentMultimediaData},
			{"ofd_replace_page_multimedia_data", &api.replacePageMultimediaData},
			{"ofd_remove_document_multimedia_resource", &api.removeDocumentMultimedia},
			{"ofd_remove_page_multimedia_resource", &api.removePageMultimedia},
			{"ofd_add_attachment_to_document", &api.addAttachmentToDocument},
			{"ofd_remove_attachment", &api.removeAttachment},
			{"ofd_rename_attachment", &api.renameAttachment},
			{"ofd_replace_attachment_data", &api.replaceAttachmentData},
			{"ofd_set_attachment_metadata_json", &api.setAttachmentMetadata},
			{"ofd_buffer_free", &api.bufferFree},
		}
		for _, binding := range bindings {
			address, err := lookupSymbol(library, binding.name)
			if err != nil {
				apiErr = fmt.Errorf("load %s: %w", binding.name, err)
				return
			}
			purego.RegisterFunc(binding.dest, address)
		}
		if version := api.version(); version != 1 {
			apiErr = fmt.Errorf("unsupported ofdrw C ABI version %d", version)
		}
	})
	return apiErr
}

func nativeError(status int32) error {
	if status == 0 {
		return nil
	}
	ptr := api.lastError()
	if ptr == nil {
		return errors.New("ofdrw native call failed")
	}
	bytes := make([]byte, 0, 128)
	for i := uintptr(0); i < 4096; i++ {
		ch := *(*byte)(unsafe.Add(unsafe.Pointer(ptr), i))
		if ch == 0 {
			break
		}
		bytes = append(bytes, ch)
	}
	return errors.New(string(bytes))
}

func cString(value string) ([]byte, error) {
	if strings.IndexByte(value, 0) >= 0 {
		return nil, errors.New("string contains NUL byte")
	}
	return append([]byte(value), 0), nil
}

func textRequest(value any) ([]byte, error) {
	data, err := json.Marshal(value)
	if err != nil {
		return nil, fmt.Errorf("encode text operation: %w", err)
	}
	return append(data, 0), nil
}

func bytesAt(buffer nativeBuffer) ([]byte, error) {
	if buffer.len == 0 {
		return []byte{}, nil
	}
	if buffer.data == nil || buffer.len > uintptr(int(^uint(0)>>1)) {
		return nil, errors.New("invalid native buffer")
	}
	return append([]byte(nil), unsafe.Slice(buffer.data, int(buffer.len))...), nil
}

// Document owns an OFD document in the Rust library.
type Document struct {
	mu sync.Mutex
	// The native ABI returns an opaque registry ID, not an address Go may scan.
	handle uintptr
}

// Attachment describes one embedded file.
type Attachment struct {
	DocumentIndex int     `json:"document_index"`
	ID            string  `json:"id"`
	Name          string  `json:"name"`
	Path          string  `json:"path"`
	Size          int     `json:"size"`
	Format        *string `json:"format"`
	CreationDate  *string `json:"creation_date"`
	ModDate       *string `json:"mod_date"`
	Visible       bool    `json:"visible"`
	Usage         string  `json:"usage"`
}

// AttachmentMetadata replaces an attachment's optional standard fields.
// A nil Visible uses the OFD default of true.
type AttachmentMetadata struct {
	Format       *string `json:"format"`
	CreationDate *string `json:"creation_date"`
	ModDate      *string `json:"mod_date"`
	Visible      *bool   `json:"visible,omitempty"`
	Usage        *string `json:"usage"`
}

// CustomTag describes one entry in a document's custom tag index.
type CustomTag struct {
	DocumentIndex int     `json:"document_index"`
	TypeID        string  `json:"type_id"`
	Namespace     *string `json:"namespace"`
	SchemaPath    *string `json:"schema_path"`
	Path          string  `json:"path"`
}

// CustomTagOptions provides optional namespace and schema references.
type CustomTagOptions struct {
	Namespace  *string `json:"namespace"`
	SchemaPath *string `json:"schema_path"`
}

// ExtensionProperty is one simple property in an OFD extension.
type ExtensionProperty struct {
	Name     string  `json:"name"`
	TypeName *string `json:"type_name"`
	Value    string  `json:"value"`
}

// Extension is one document extension entry.
type Extension struct {
	DocumentIndex   int                 `json:"document_index"`
	AppName         string              `json:"app_name"`
	Company         *string             `json:"company"`
	AppVersion      *string             `json:"app_version"`
	Date            *string             `json:"date"`
	RefID           *uint64             `json:"ref_id"`
	Properties      []ExtensionProperty `json:"properties"`
	ExtendDataPaths []string            `json:"extend_data_paths"`
	DataXML         []string            `json:"data_xml"`
}

// SoundActionOptions controls playback of an Audio resource.
type SoundActionOptions struct {
	Volume      *uint8 `json:"volume,omitempty"`
	Repeat      *bool  `json:"repeat,omitempty"`
	Synchronous *bool  `json:"synchronous,omitempty"`
}

// Annotation contains metadata and appearance text from a page annotation.
type Annotation struct {
	DocumentIndex int                    `json:"document_index"`
	PageID        string                 `json:"page_id"`
	ID            string                 `json:"id"`
	Type          string                 `json:"type"`
	Subtype       *string                `json:"subtype"`
	LastModDate   *string                `json:"last_mod_date"`
	Remark        *string                `json:"remark"`
	Parameters    map[string]string      `json:"parameters"`
	SourcePath    string                 `json:"source_path"`
	AppearanceBox []float64              `json:"appearance_box"`
	Attributes    map[string]string      `json:"attributes"`
	Text          []string               `json:"text"`
	Actions       []PageObjectActionInfo `json:"actions"`
}

// AnnotationObject is one direct graphic object in an annotation appearance.
type AnnotationObject struct {
	ID            *string       `json:"id"`
	Name          *string       `json:"name"`
	BlendMode     *string       `json:"blend_mode"`
	Kind          string        `json:"kind"`
	Boundary      []float64     `json:"boundary"`
	CTM           []float64     `json:"ctm"`
	Visible       bool          `json:"visible"`
	Alpha         uint8         `json:"alpha"`
	DrawParam     *string       `json:"draw_param"`
	ResourceID    *string       `json:"resource_id"`
	FontID        *string       `json:"font_id"`
	Size          *float64      `json:"size"`
	HScale        *float64      `json:"h_scale"`
	Fill          *bool         `json:"fill"`
	Stroke        *bool         `json:"stroke"`
	ReadDirection *uint16       `json:"read_direction"`
	CharDirection *uint16       `json:"char_direction"`
	Weight        *uint16       `json:"weight"`
	Italic        *bool         `json:"italic"`
	Text          []string      `json:"text"`
	Codes         []TextCode    `json:"codes"`
	CGTransforms  []CGTransform `json:"cg_transforms"`
	Commands      *string       `json:"commands"`
}

// AnnotationObjectUpdate changes selected direct attributes atomically.
// ClearCTM removes CTM; it cannot be combined with CTM.
type AnnotationObjectUpdate struct {
	Boundary *[4]float64
	CTM      *[6]float64
	ClearCTM bool
	Visible  *bool
	Alpha    *uint8
}

// Font describes a document font resource.
type Font struct {
	DocumentIndex int               `json:"document_index"`
	ID            string            `json:"id"`
	FontName      string            `json:"font_name"`
	FamilyName    *string           `json:"family_name"`
	ResourcePath  string            `json:"resource_path"`
	EmbeddedPath  *string           `json:"embedded_path"`
	Attributes    map[string]string `json:"attributes"`
}

// SignatureReference describes one protected package entry.
type SignatureReference struct {
	Path       string `json:"path"`
	CheckValue string `json:"check_value"`
}

// SignatureStamp describes an appearance on one page.
type SignatureStamp struct {
	ID       string    `json:"id"`
	PageRef  string    `json:"page_ref"`
	Boundary []float64 `json:"boundary"`
	Clip     []float64 `json:"clip"`
}

// SESSealSpec describes a v4/v5 electronic seal to be signed by its issuer.
// Timestamps are UTC Unix seconds. Certificate trust remains caller supplied.
type SESSealSpec struct {
	Version                      uint8    `json:"version"`
	VendorID                     string   `json:"vendor_id"`
	SealID                       string   `json:"seal_id"`
	Name                         string   `json:"name"`
	SealType                     uint8    `json:"seal_type"`
	AuthorizedSignerCertificates [][]byte `json:"authorized_signer_certificates_base64"`
	CreatedAt                    uint64   `json:"created_at"`
	ValidFrom                    uint64   `json:"valid_from"`
	ValidUntil                   uint64   `json:"valid_until"`
	PictureType                  string   `json:"picture_type"`
	PictureWidthMM               uint32   `json:"picture_width_mm"`
	PictureHeightMM              uint32   `json:"picture_height_mm"`
}

// SignatureReferenceCheck compares one protected file with its recorded digest.
type SignatureReferenceCheck struct {
	Path     string `json:"path"`
	Valid    bool   `json:"valid"`
	Expected string `json:"expected"`
	Actual   string `json:"actual"`
}

// SignatureVerificationReport combines all checks for a directly trusted issuer.
// The time and trust anchor are supplied by the caller; revocation is not checked.
type SignatureVerificationReport struct {
	References                   []SignatureReferenceCheck `json:"references"`
	ReferencesValid              bool                      `json:"references_valid"`
	DescriptionHash              *SignatureReferenceCheck  `json:"description_hash"`
	SealMatch                    *bool                     `json:"seal_match"`
	SealSignatureValid           *bool                     `json:"seal_signature_valid"`
	SealCertificateValidityValid *bool                     `json:"seal_certificate_validity_valid"`
	SealSignerAuthorized         *bool                     `json:"seal_signer_authorized"`
	SealValidityValid            *bool                     `json:"seal_validity_valid"`
	SignatureValueValid          bool                      `json:"signature_value_valid"`
	CertificateValidityValid     bool                      `json:"certificate_validity_valid"`
	DirectTrustAnchorValid       bool                      `json:"direct_trust_anchor_valid"`
	AllValid                     bool                      `json:"all_valid"`
}

// SignatureChainVerificationReport combines digest, signature, and ordered
// SM2 certificate chain checks. The caller supplies the trusted anchor and time.
type SignatureChainVerificationReport struct {
	References                   []SignatureReferenceCheck `json:"references"`
	ReferencesValid              bool                      `json:"references_valid"`
	DescriptionHash              *SignatureReferenceCheck  `json:"description_hash"`
	SealMatch                    *bool                     `json:"seal_match"`
	SealSignatureValid           *bool                     `json:"seal_signature_valid"`
	SealCertificateValidityValid *bool                     `json:"seal_certificate_validity_valid"`
	SealSignerAuthorized         *bool                     `json:"seal_signer_authorized"`
	SealValidityValid            *bool                     `json:"seal_validity_valid"`
	SignatureValueValid          bool                      `json:"signature_value_valid"`
	CertificateValidityValid     bool                      `json:"certificate_validity_valid"`
	CertificateChainValid        bool                      `json:"certificate_chain_valid"`
	AllValid                     bool                      `json:"all_valid"`
}

// SignatureTrustOptions supplies separate trust chains and SM2 IDs for the
// signer and the electronic seal issuer. For raw signatures, leave seal fields empty.
type SignatureTrustOptions struct {
	SignerCertificateDER    []byte   `json:"signer_certificate_der_base64"`
	SignerIntermediatesDER  [][]byte `json:"signer_intermediates_der_base64"`
	SignerAnchorDER         []byte   `json:"signer_anchor_der_base64"`
	SealIntermediatesDER    [][]byte `json:"seal_intermediates_der_base64"`
	SealAnchorDER           []byte   `json:"seal_anchor_der_base64"`
	AtUnixSeconds           uint64   `json:"at_unix_seconds"`
	SignerCertificateDistID string   `json:"signer_certificate_distid"`
	SignerSignatureDistID   string   `json:"signer_signature_distid"`
	SealCertificateDistID   string   `json:"seal_certificate_distid"`
	SealSignatureDistID     string   `json:"seal_signature_distid"`
}

// SignatureTrustVerificationReport combines the signer report with the seal
// issuer chain result. Revocation and trusted signing time are not checked.
type SignatureTrustVerificationReport struct {
	Signature            SignatureChainVerificationReport `json:"signature"`
	SealIssuerChainValid *bool                            `json:"seal_issuer_chain_valid"`
	AllValid             bool                             `json:"all_valid"`
}

// Signature describes an OFD signature without verifying its authenticity.
type Signature struct {
	DocumentIndex     int                  `json:"document_index"`
	ID                string               `json:"id"`
	Type              *string              `json:"type"`
	Path              string               `json:"path"`
	SignedValuePath   string               `json:"signed_value_path"`
	SealPath          *string              `json:"seal_path"`
	ProviderName      *string              `json:"provider_name"`
	SignatureMethod   *string              `json:"signature_method"`
	SignatureDateTime *string              `json:"signature_datetime"`
	CheckMethod       *string              `json:"check_method"`
	References        []SignatureReference `json:"references"`
	Stamps            []SignatureStamp     `json:"stamps"`
}

// PageInfo contains page location and effective area boxes in millimeters.
type PageInfo struct {
	Index          int       `json:"index"`
	DocumentIndex  int       `json:"document_index"`
	ID             string    `json:"id"`
	Path           string    `json:"path"`
	PhysicalBox    []float64 `json:"physical_box"`
	ApplicationBox []float64 `json:"application_box"`
	ContentBox     []float64 `json:"content_box"`
	BleedBox       []float64 `json:"bleed_box"`
}

// PageArea contains explicit OFD page boxes in millimeters. Nil optional boxes
// are removed from the selected page's Area element.
type PageArea struct {
	PhysicalBox    [4]float64  `json:"physical_box"`
	ApplicationBox *[4]float64 `json:"application_box,omitempty"`
	ContentBox     *[4]float64 `json:"content_box,omitempty"`
	BleedBox       *[4]float64 `json:"bleed_box,omitempty"`
}

// FlowTextStyle controls wrapping and pagination in millimeters. Styled lines
// use their largest font size plus LineHeight minus FontSize as their height.
type FlowTextStyle struct {
	PageSize               [2]float64 `json:"page_size"`
	Margins                [4]float64 `json:"margins"`
	FontSize               float64    `json:"font_size"`
	LineHeight             float64    `json:"line_height"`
	ParagraphGap           float64    `json:"paragraph_gap,omitempty"`
	KeepParagraphsTogether bool       `json:"keep_paragraphs_together,omitempty"`
	FontID                 string     `json:"font_id,omitempty"`
	Align                  string     `json:"align,omitempty"`
	FirstLineIndent        float64    `json:"first_line_indent,omitempty"`
	RestLineIndent         float64    `json:"rest_line_indent,omitempty"`
	FirstLineIndentChars   int        `json:"first_line_indent_chars,omitempty"`
	IndentEachParagraph    bool       `json:"indent_each_paragraph,omitempty"`
	Shape                  bool       `json:"shape,omitempty"`
	Language               string     `json:"language,omitempty"`
}

// TableStyle controls table layout and row pagination. RowHeight zero sizes
// each row from its wrapped cell content.
type TableStyle struct {
	PageSize     [2]float64 `json:"page_size"`
	Margins      [4]float64 `json:"margins"`
	ColumnWidths []float64  `json:"column_widths"`
	RowHeight    float64    `json:"row_height"`
	FontSize     float64    `json:"font_size"`
	LineHeight   float64    `json:"line_height"`
	Padding      float64    `json:"padding"`
	BorderWidth  float64    `json:"border_width"`
	BorderColor  [3]uint8   `json:"border_color"`
	FontID       string     `json:"font_id,omitempty"`
	RepeatHeader bool       `json:"repeat_header"`
}

// TableCell occupies a one-based span on a zero-based table grid.
// FontID overrides the table font and is inherited by nested cells.
// Padding is top, right, bottom, left inner spacing in millimeters.
// BorderWidth and BorderColor override the table border; zero width omits it.
// Underline and Strikethrough decorate each rendered line of cell text.
// LetterSpacing adds millimeter spacing between adjacent characters.
type TableCell struct {
	Row             int              `json:"row"`
	Column          int              `json:"column"`
	RowSpan         int              `json:"row_span"`
	ColumnSpan      int              `json:"column_span"`
	Text            string           `json:"text"`
	BackgroundColor *[3]uint8        `json:"background_color,omitempty"`
	TextColor       *[3]uint8        `json:"text_color,omitempty"`
	BorderWidth     *float64         `json:"border_width,omitempty"`
	BorderColor     *[3]uint8        `json:"border_color,omitempty"`
	Padding         *[4]float64      `json:"padding,omitempty"`
	FontID          string           `json:"font_id,omitempty"`
	FontSize        *float64         `json:"font_size,omitempty"`
	LineHeight      *float64         `json:"line_height,omitempty"`
	LetterSpacing   *float64         `json:"letter_spacing,omitempty"`
	Bold            bool             `json:"bold,omitempty"`
	Italic          bool             `json:"italic,omitempty"`
	Underline       bool             `json:"underline,omitempty"`
	Strikethrough   bool             `json:"strikethrough,omitempty"`
	Align           string           `json:"align,omitempty"`
	VerticalAlign   string           `json:"vertical_align,omitempty"`
	Image           *TableCellImage  `json:"image,omitempty"`
	Blocks          []TableCellBlock `json:"blocks,omitempty"`
}

// TableCellBlock is ordered text, image, spacer, rule, or table content in a cell.
// TextColor and Align override the cell style; Color overrides a rule's border color.
type TableCellBlock struct {
	Type      string       `json:"type"`
	Text      string       `json:"text,omitempty"`
	TextColor *[3]uint8    `json:"text_color,omitempty"`
	Align     string       `json:"align,omitempty"`
	Data      []byte       `json:"data_base64,omitempty"`
	Format    string       `json:"format,omitempty"`
	Width     float64      `json:"width,omitempty"`
	Height    float64      `json:"height,omitempty"`
	Thickness float64      `json:"thickness,omitempty"`
	Color     *[3]uint8    `json:"color,omitempty"`
	Table     *NestedTable `json:"table,omitempty"`
}

// NestedTable is a complete grid inside a table cell block.
type NestedTable struct {
	RowCount     int         `json:"row_count"`
	ColumnWidths []float64   `json:"column_widths"`
	Cells        []TableCell `json:"cells"`
	RowHeight    float64     `json:"row_height,omitempty"`
	Padding      float64     `json:"padding,omitempty"`
	BorderWidth  float64     `json:"border_width,omitempty"`
	BorderColor  [3]uint8    `json:"border_color,omitempty"`
}

// TableCellImage places an encoded image after a cell's text.
// Width and height are measured in millimeters.
type TableCellImage struct {
	Data   []byte  `json:"data_base64"`
	Format string  `json:"format"`
	Width  float64 `json:"width"`
	Height float64 `json:"height"`
}

// DefaultTableStyle returns A4 geometry; callers must set ColumnWidths.
func DefaultTableStyle() TableStyle {
	return TableStyle{
		PageSize:  [2]float64{210, 297},
		Margins:   [4]float64{25.4, 31.7, 25.4, 31.7},
		RowHeight: 8, FontSize: 5, LineHeight: 6,
		Padding: 1, BorderWidth: 0.2,
	}
}

// FlowSpan is one styled segment in a flowing paragraph.
type FlowSpan struct {
	Text            string    `json:"text"`
	FontID          string    `json:"font_id,omitempty"`
	FontSize        *float64  `json:"font_size,omitempty"`
	Language        string    `json:"language,omitempty"`
	FillColor       *[3]uint8 `json:"fill_color,omitempty"`
	Fill            *bool     `json:"fill,omitempty"`
	Stroke          bool      `json:"stroke,omitempty"`
	StrokeColor     *[3]uint8 `json:"stroke_color,omitempty"`
	StrokeWidth     *float64  `json:"stroke_width,omitempty"`
	Bold            bool      `json:"bold,omitempty"`
	Weight          *uint16   `json:"weight,omitempty"`
	Italic          bool      `json:"italic,omitempty"`
	LetterSpacing   float64   `json:"letter_spacing,omitempty"`
	Underline       bool      `json:"underline,omitempty"`
	UnderlineOffset *float64  `json:"underline_offset,omitempty"`
	UnderlineWidth  *float64  `json:"underline_width,omitempty"`
	LineBreakAfter  bool      `json:"line_break_after,omitempty"`
	KeepTogether    bool      `json:"keep_together,omitempty"`
}

// DefaultFlowTextStyle returns A4 page and margin settings.
func DefaultFlowTextStyle() FlowTextStyle {
	return FlowTextStyle{
		PageSize:   [2]float64{210, 297},
		Margins:    [4]float64{25.4, 31.7, 25.4, 31.7},
		FontSize:   5,
		LineHeight: 6,
	}
}

// ValidationIssue identifies a broken package reference or page object.
type ValidationIssue struct {
	Path    string `json:"path"`
	Code    string `json:"code"`
	Message string `json:"message"`
}

// ArchiveViolation reports one supported OFD-A archival rule failure.
type ArchiveViolation struct {
	Rule     string  `json:"rule"`
	Severity string  `json:"severity"`
	Message  string  `json:"message"`
	Path     *string `json:"path"`
	Actual   *string `json:"actual"`
	Expected *string `json:"expected"`
}

// PageSource is a referenced template or the page's own XML source.
type PageSource struct {
	Path       string  `json:"path"`
	TemplateID *string `json:"template_id"`
	ZOrder     *string `json:"z_order"`
}

// LayerInfo describes one page or template layer in painting order.
type LayerInfo struct {
	ID               *string `json:"id"`
	Name             *string `json:"name"`
	LayerType        string  `json:"layer_type"`
	DrawParam        *string `json:"draw_param"`
	SourcePath       string  `json:"source_path"`
	TemplateID       *string `json:"template_id"`
	SourceLayerIndex int     `json:"source_layer_index"`
	ObjectCount      int     `json:"object_count"`
}

// PageObjectInfo describes a direct page or template graphic object in painting order.
type PageObjectInfo struct {
	ID               *string   `json:"id"`
	Name             *string   `json:"name"`
	BlendMode        *string   `json:"blend_mode"`
	Kind             string    `json:"kind"`
	LayerID          *string   `json:"layer_id"`
	LayerType        string    `json:"layer_type"`
	SourceLayerIndex int       `json:"source_layer_index"`
	SourcePath       string    `json:"source_path"`
	TemplateID       *string   `json:"template_id"`
	Boundary         []float64 `json:"boundary"`
	CTM              []float64 `json:"ctm"`
	Visible          bool      `json:"visible"`
	Alpha            uint8     `json:"alpha"`
	DrawParam        *string   `json:"draw_param"`
	ResourceID       *string   `json:"resource_id"`
}

// ClipTextCodeInfo describes one positioned text run in a clip area.
type ClipTextCodeInfo struct {
	X      *float64 `json:"x"`
	Y      *float64 `json:"y"`
	DeltaX *string  `json:"delta_x"`
	DeltaY *string  `json:"delta_y"`
	Text   string   `json:"text"`
}

// ClipCGTransformInfo maps UTF-16 text positions to font glyph IDs.
type ClipCGTransformInfo struct {
	CodePosition int      `json:"code_position"`
	CodeCount    int      `json:"code_count"`
	GlyphCount   int      `json:"glyph_count"`
	Glyphs       []uint32 `json:"glyphs"`
}

// ClipAreaInfo describes one path or text area and retains its original XML.
type ClipAreaInfo struct {
	Kind          string                `json:"kind"`
	Fill          *bool                 `json:"fill"`
	Stroke        *bool                 `json:"stroke"`
	Boundary      *[4]float64           `json:"boundary"`
	AreaCTM       *[6]float64           `json:"area_ctm"`
	ShapeCTM      *[6]float64           `json:"shape_ctm"`
	DrawParam     *string               `json:"draw_param"`
	Rule          *string               `json:"rule"`
	Commands      *string               `json:"commands"`
	FontID        *string               `json:"font_id"`
	Size          *float64              `json:"size"`
	Text          *string               `json:"text"`
	HScale        *float64              `json:"h_scale"`
	CharDirection *uint16               `json:"char_direction"`
	TextCodes     []ClipTextCodeInfo    `json:"text_codes"`
	CGTransforms  []ClipCGTransformInfo `json:"cg_transforms"`
	XML           string                `json:"xml"`
}

// PageObjectClipsInfo describes clips on a direct page or template object.
type PageObjectClipsInfo struct {
	ID         *string          `json:"id"`
	Kind       string           `json:"kind"`
	SourcePath string           `json:"source_path"`
	TemplateID *string          `json:"template_id"`
	FollowsCTM bool             `json:"follows_ctm"`
	Clips      [][]ClipAreaInfo `json:"clips"`
}

// PathClipArea is a path area in an OFD Clip.
type PathClipArea struct {
	Boundary  [4]float64  `json:"boundary"`
	Commands  string      `json:"commands"`
	AreaCTM   *[6]float64 `json:"area_ctm,omitempty"`
	PathCTM   *[6]float64 `json:"path_ctm,omitempty"`
	EvenOdd   bool        `json:"even_odd,omitempty"`
	Fill      *bool       `json:"fill,omitempty"`
	Stroke    *bool       `json:"stroke,omitempty"`
	DrawParam string      `json:"draw_param,omitempty"`
}

// ClipAreaSpec is a Path or Text component in an OFD Clip.
type ClipAreaSpec struct {
	Kind          string                `json:"kind"`
	Boundary      [4]float64            `json:"boundary"`
	Commands      string                `json:"commands,omitempty"`
	FontID        string                `json:"font_id,omitempty"`
	Size          float64               `json:"size,omitempty"`
	Text          string                `json:"text,omitempty"`
	X             float64               `json:"x,omitempty"`
	Y             float64               `json:"y,omitempty"`
	AreaCTM       *[6]float64           `json:"area_ctm,omitempty"`
	PathCTM       *[6]float64           `json:"path_ctm,omitempty"`
	TextCTM       *[6]float64           `json:"text_ctm,omitempty"`
	EvenOdd       bool                  `json:"even_odd,omitempty"`
	Fill          *bool                 `json:"fill,omitempty"`
	Stroke        *bool                 `json:"stroke,omitempty"`
	DrawParam     string                `json:"draw_param,omitempty"`
	HScale        float64               `json:"h_scale,omitempty"`
	CharDirection uint16                `json:"char_direction,omitempty"`
	TextCodes     []ClipTextCodeInfo    `json:"text_codes,omitempty"`
	CGTransforms  []ClipCGTransformInfo `json:"cg_transforms,omitempty"`
}

// PaintInfo describes paint declared directly on a graphic object.
type PaintInfo struct {
	Kind       string  `json:"kind"`
	Alpha      uint8   `json:"alpha"`
	Value      *string `json:"value"`
	ColorSpace *string `json:"color_space"`
	ColorIndex *string `json:"color_index"`
	XML        string  `json:"xml"`
}

// PageObjectPaintInfo describes direct path/text paints, including template objects.
type PageObjectPaintInfo struct {
	ID                       *string    `json:"id"`
	Kind                     string     `json:"kind"`
	SourcePath               string     `json:"source_path"`
	TemplateID               *string    `json:"template_id"`
	FillEnabled              bool       `json:"fill_enabled"`
	StrokeEnabled            bool       `json:"stroke_enabled"`
	Fill                     *PaintInfo `json:"fill"`
	Stroke                   *PaintInfo `json:"stroke"`
	EffectiveFill            *PaintInfo `json:"effective_fill"`
	EffectiveStroke          *PaintInfo `json:"effective_stroke"`
	EffectiveFillDrawParam   *string    `json:"effective_fill_draw_param"`
	EffectiveStrokeDrawParam *string    `json:"effective_stroke_draw_param"`
}

// StrokeStyleInfo contains effective stroke geometry after DrawParam inheritance.
type StrokeStyleInfo struct {
	LineWidth    float64     `json:"line_width"`
	Cap          *string     `json:"cap"`
	Join         *string     `json:"join"`
	MiterLimit   *float64    `json:"miter_limit"`
	DashPattern  *[2]float64 `json:"dash_pattern"`
	DashSequence []float64   `json:"dash_sequence"`
	DashOffset   float64     `json:"dash_offset"`
}

// PageObjectStrokeStyleInfo describes one direct page or template path/text object.
type PageObjectStrokeStyleInfo struct {
	ID            *string         `json:"id"`
	Kind          string          `json:"kind"`
	SourcePath    string          `json:"source_path"`
	TemplateID    *string         `json:"template_id"`
	StrokeEnabled bool            `json:"stroke_enabled"`
	Style         StrokeStyleInfo `json:"style"`
}

// PageObjectActionInfo describes an action attached to a direct page object.
type PageObjectActionInfo struct {
	Event       string            `json:"event"`
	Kind        string            `json:"kind"`
	Attributes  map[string]string `json:"attributes"`
	Destination *Destination      `json:"destination"`
	Bookmark    *string           `json:"bookmark"`
	Region      *ActionRegionInfo `json:"region"`
}

// DocumentActionInfo describes one action declared directly on a document body.
type DocumentActionInfo = PageObjectActionInfo

// DocumentActionSpec appends a URI, page, bookmark, or attachment action.
// Kind is one of "uri", "page", "bookmark", or "attachment".
type DocumentActionSpec struct {
	Kind        string       `json:"kind"`
	Event       string       `json:"event"`
	URI         string       `json:"uri,omitempty"`
	Destination *Destination `json:"destination,omitempty"`
	Name        string       `json:"name,omitempty"`
	ID          string       `json:"id,omitempty"`
	NewWindow   *bool        `json:"new_window,omitempty"`
}

// IntegrityFileInfo is one package file named by OFDEntries.xml.
type IntegrityFileInfo struct {
	ID   string `json:"id"`
	Path string `json:"path"`
}

// IntegrityManifestInfo describes the optional OFDEntries.xml file list.
type IntegrityManifestInfo struct {
	ID              string              `json:"id"`
	CreatorName     *string             `json:"creator_name"`
	Version         *string             `json:"version"`
	CreationDate    *string             `json:"creation_date"`
	SignedValuePath string              `json:"signed_value_path"`
	Files           []IntegrityFileInfo `json:"files"`
}

// IntegrityManifestCheck compares manifest file names with package entries.
// It does not authenticate the manifest signature.
type IntegrityManifestCheck struct {
	MissingFiles []string `json:"missing_files"`
	ExtraFiles   []string `json:"extra_files"`
	Complete     bool     `json:"complete"`
}

// IntegrityVerificationInfo combines the file-list and SM2 signature results.
// The embedded certificate's trust and validity are not established.
type IntegrityVerificationInfo struct {
	ManifestComplete bool `json:"manifest_complete"`
	SignatureValid   bool `json:"signature_valid"`
	AllValid         bool `json:"all_valid"`
}

// IntegrityTrustedVerificationInfo includes the caller-supplied SM2 chain check.
type IntegrityTrustedVerificationInfo struct {
	ManifestComplete      bool `json:"manifest_complete"`
	SignatureValid        bool `json:"signature_valid"`
	CertificateChainValid bool `json:"certificate_chain_valid"`
	AllValid              bool `json:"all_valid"`
}

// ActionRegionInfo contains explicit trigger areas for an OFD action.
type ActionRegionInfo struct {
	Areas []ActionRegionAreaInfo `json:"areas"`
}

// ActionRegionAreaInfo is a path within an action trigger region.
type ActionRegionAreaInfo struct {
	Start    [2]float64                `json:"start"`
	Commands []ActionRegionCommandInfo `json:"commands"`
}

// ActionRegionCommandInfo preserves one OFD path command.
type ActionRegionCommandInfo struct {
	Kind       string            `json:"kind"`
	Attributes map[string]string `json:"attributes"`
}

// CustomData is one user-defined document metadata value.
type CustomData struct {
	Name  string `json:"name"`
	Value string `json:"value"`
}

// DocumentInfo contains metadata from a DocBody/DocInfo entry.
type DocumentInfo struct {
	Index          int          `json:"index"`
	Root           string       `json:"root"`
	DocID          string       `json:"doc_id"`
	Title          *string      `json:"title"`
	Author         *string      `json:"author"`
	Subject        *string      `json:"subject"`
	Abstract       *string      `json:"abstract"`
	CreationDate   *string      `json:"creation_date"`
	ModDate        *string      `json:"mod_date"`
	Creator        *string      `json:"creator"`
	CreatorVersion *string      `json:"creator_version"`
	DocUsage       *string      `json:"doc_usage"`
	Cover          *string      `json:"cover"`
	Keywords       []string     `json:"keywords"`
	CustomData     []CustomData `json:"custom_data"`
}

// PrintPermission declares whether printing is allowed and how many copies are allowed.
type PrintPermission struct {
	Printable bool  `json:"printable"`
	Copies    int32 `json:"copies"`
}

// ValidPeriod bounds a document's validity using local ISO date-time strings.
type ValidPeriod struct {
	StartDate *string `json:"start_date"`
	EndDate   *string `json:"end_date"`
}

// DocumentPermissions is the effective permission declaration of one document body.
type DocumentPermissions struct {
	Edit        bool            `json:"edit"`
	Annot       bool            `json:"annot"`
	Export      bool            `json:"export"`
	Signature   bool            `json:"signature"`
	Watermark   bool            `json:"watermark"`
	PrintScreen bool            `json:"print_screen"`
	Print       PrintPermission `json:"print"`
	ValidPeriod *ValidPeriod    `json:"valid_period"`
}

// DefaultDocumentPermissions returns the defaults used by the OFD permission model.
func DefaultDocumentPermissions() DocumentPermissions {
	return DocumentPermissions{
		Edit: true, Annot: true, Export: true, Signature: true,
		Watermark: true, PrintScreen: true,
		Print: PrintPermission{Printable: true, Copies: -1},
	}
}

// ViewPreferences controls the initial presentation of a document body.
type ViewPreferences struct {
	PageMode     string   `json:"page_mode"`
	PageLayout   string   `json:"page_layout"`
	TabDisplay   string   `json:"tab_display"`
	HideToolbar  bool     `json:"hide_toolbar"`
	HideMenubar  bool     `json:"hide_menubar"`
	HideWindowUI bool     `json:"hide_window_ui"`
	ZoomMode     *string  `json:"zoom_mode"`
	Zoom         *float64 `json:"zoom"`
}

// DefaultViewPreferences returns the OFD viewer preference defaults.
func DefaultViewPreferences() ViewPreferences {
	return ViewPreferences{PageMode: "None", PageLayout: "OneColumn", TabDisplay: "FileName"}
}

// VersionFileInfo is one package entry listed by a document version.
type VersionFileInfo struct {
	ID   string `json:"id"`
	Path string `json:"path"`
}

// VersionInfo describes one OFD DocVersion and its package file list.
type VersionInfo struct {
	DocumentIndex   int               `json:"document_index"`
	ID              string            `json:"id"`
	Index           uint32            `json:"index"`
	Current         bool              `json:"current"`
	DescriptionPath string            `json:"description_path"`
	DocRoot         string            `json:"doc_root"`
	FormatVersion   *string           `json:"format_version"`
	Name            *string           `json:"name"`
	CreationDate    *string           `json:"creation_date"`
	Files           []VersionFileInfo `json:"files"`
}

// KeywordPosition describes a keyword match in page millimeters.
type KeywordPosition struct {
	PageIndex int        `json:"page_index"`
	Keyword   string     `json:"keyword"`
	BoxMM     [4]float64 `json:"box_mm"`
}

// Destination identifies a page and optional location within it.
type Destination struct {
	PageID string   `json:"page_id"`
	Kind   string   `json:"kind"`
	Left   *float64 `json:"left"`
	Top    *float64 `json:"top"`
	Right  *float64 `json:"right"`
	Bottom *float64 `json:"bottom"`
	Zoom   *float64 `json:"zoom"`
}

// Bookmark is a named document location.
type Bookmark struct {
	DocumentIndex int         `json:"document_index"`
	Name          string      `json:"name"`
	Destination   Destination `json:"destination"`
}

// Outline is one node in the document navigation tree.
type Outline struct {
	DocumentIndex int          `json:"document_index"`
	Title         string       `json:"title"`
	Depth         int          `json:"depth"`
	ParentIndex   *int         `json:"parent_index"`
	Expanded      bool         `json:"expanded"`
	Count         *int64       `json:"count"`
	Destination   *Destination `json:"destination"`
	BookmarkName  *string      `json:"bookmark_name"`
}

// PathObject describes an OFD path and its abbreviated drawing commands.
type PathObject struct {
	ID              *string   `json:"id"`
	Boundary        []float64 `json:"boundary"`
	CTM             []float64 `json:"ctm"`
	Stroke          bool      `json:"stroke"`
	Fill            bool      `json:"fill"`
	AbbreviatedData *string   `json:"abbreviated_data"`
}

// ImageObject describes an image and its resolved multimedia resource.
type ImageObject struct {
	ID           *string          `json:"id"`
	Boundary     []float64        `json:"boundary"`
	CTM          []float64        `json:"ctm"`
	ResourceID   string           `json:"resource_id"`
	Substitution *string          `json:"substitution"`
	ImageMask    *string          `json:"image_mask"`
	Border       *ImageBorderInfo `json:"border"`
	MediaPath    *string          `json:"media_path"`
	Format       *string          `json:"format"`
}

// MultimediaResource describes an image, audio or video resource visible to a page.
type MultimediaResource struct {
	ID        string  `json:"id"`
	MediaType *string `json:"media_type"`
	Format    *string `json:"format"`
	Path      string  `json:"path"`
	Size      int     `json:"size"`
}

// ColorSpace describes an OFD color space and its defining resource.
type ColorSpace struct {
	ID               string     `json:"id"`
	ColorType        string     `json:"color_type"`
	BitsPerComponent uint8      `json:"bits_per_component"`
	Profile          *string    `json:"profile"`
	Palette          [][]uint16 `json:"palette"`
	ResourcePath     string     `json:"resource_path"`
}

// DrawParam describes a reusable drawing parameter and its defining resource.
type DrawParam struct {
	ID           string  `json:"id"`
	Relative     *string `json:"relative"`
	XML          string  `json:"xml"`
	ResourcePath string  `json:"resource_path"`
}

// ColorSpaceSpec configures a document-level color space.
type ColorSpaceSpec struct {
	ColorType        string     `json:"color_type"`
	BitsPerComponent uint8      `json:"bits_per_component,omitempty"`
	Profile          *string    `json:"profile,omitempty"`
	Palette          [][]uint16 `json:"palette,omitempty"`
}

// EncryptionInfo describes one OFD encryption operation and its users.
type EncryptionInfo struct {
	ID               string   `json:"id"`
	Scope            string   `json:"scope"`
	DecryptSeedPath  string   `json:"decrypt_seed_path"`
	EntriesMapPath   string   `json:"entries_map_path"`
	Usernames        []string `json:"usernames"`
	ProtectionCaseID *string  `json:"protection_case_id"`
}

// PasswordRecipient is a user who can unlock a password-encrypted OFD.
type PasswordRecipient struct {
	Username string `json:"username"`
	Password string `json:"password"`
	UserType string `json:"user_type,omitempty"`
}

// CertificateRecipient is a user who can unlock an SM2 certificate-encrypted OFD.
type CertificateRecipient struct {
	Username       string `json:"username"`
	CertificateDER []byte `json:"certificate_der_base64"`
	UserType       string `json:"user_type,omitempty"`
}

// EncryptionRecipient selects password or certificate protection for one user.
// Type is "password" or "certificate"; set the corresponding credential field.
type EncryptionRecipient struct {
	Type           string `json:"type"`
	Username       string `json:"username"`
	Password       string `json:"password,omitempty"`
	CertificateDER []byte `json:"certificate_der_base64,omitempty"`
	UserType       string `json:"user_type,omitempty"`
}

// ImageBorderInfo describes border attributes read from an OFD image.
type ImageBorderInfo struct {
	LineWidth              float64   `json:"line_width"`
	HorizontalCornerRadius float64   `json:"horizontal_corner_radius"`
	VerticalCornerRadius   float64   `json:"vertical_corner_radius"`
	DashPattern            []float64 `json:"dash_pattern"`
	DashOffset             float64   `json:"dash_offset"`
	ColorValue             *string   `json:"color_value"`
	ColorSpace             *string   `json:"color_space"`
	ColorIndex             *string   `json:"color_index"`
	ColorAlpha             uint8     `json:"color_alpha"`
	ColorXML               *string   `json:"color_xml"`
}

// ImageBorderStyle controls the stroke surrounding an image object.
type ImageBorderStyle struct {
	LineWidth              float64   `json:"line_width"`
	HorizontalCornerRadius float64   `json:"horizontal_corner_radius"`
	VerticalCornerRadius   float64   `json:"vertical_corner_radius"`
	DashPattern            []float64 `json:"dash_pattern"`
	DashOffset             float64   `json:"dash_offset"`
	Color                  [3]uint8  `json:"color"`
	ColorAlpha             uint8     `json:"color_alpha"`
}

// GouraudControlPoint is a mesh vertex in object-local millimeters.
type GouraudControlPoint struct {
	X        float64  `json:"x"`
	Y        float64  `json:"y"`
	Color    [3]uint8 `json:"color"`
	Alpha    *uint8   `json:"alpha,omitempty"`
	EdgeFlag uint8    `json:"edge_flag,omitempty"`
}

// GouraudFillStyle describes free-form triangles or a regular point grid.
type GouraudFillStyle struct {
	Points         []GouraudControlPoint `json:"points"`
	VerticesPerRow *int                  `json:"vertices_per_row,omitempty"`
	Extend         bool                  `json:"extend,omitempty"`
	BackColor      *[3]uint8             `json:"back_color,omitempty"`
	BackAlpha      *uint8                `json:"back_alpha,omitempty"`
}

// GradientSegment is a color stop in an axial or radial gradient.
type GradientSegment struct {
	Position *float64 `json:"position,omitempty"`
	Color    [3]uint8 `json:"color"`
	Alpha    *uint8   `json:"alpha,omitempty"`
}

// GradientFillStyle describes an axial or radial shading fill.
// Kind is "axial" or "radial"; radial shading requires EndRadius.
type GradientFillStyle struct {
	Kind         string            `json:"kind"`
	StartPoint   [2]float64        `json:"start_point"`
	EndPoint     [2]float64        `json:"end_point"`
	Segments     []GradientSegment `json:"segments"`
	MapType      string            `json:"map_type,omitempty"`
	MapUnit      *float64          `json:"map_unit,omitempty"`
	Extend       *uint8            `json:"extend,omitempty"`
	FillAlpha    *uint8            `json:"fill_alpha,omitempty"`
	StartRadius  *float64          `json:"start_radius,omitempty"`
	EndRadius    *float64          `json:"end_radius,omitempty"`
	Eccentricity *float64          `json:"eccentricity,omitempty"`
	Angle        *float64          `json:"angle,omitempty"`
}

// PatternCell is one path, text, image, or composite in a pattern tile.
// Set PathStyle for path cells; its fields are encoded alongside the cell fields.
type PatternCell struct {
	Kind     string     `json:"kind"`
	Commands string     `json:"commands,omitempty"`
	Boundary [4]float64 `json:"boundary"`
	*PathStyle
	Text       string  `json:"text,omitempty"`
	Size       float64 `json:"size,omitempty"`
	FontID     *string `json:"font_id,omitempty"`
	ResourceID string  `json:"resource_id,omitempty"`
}

// PatternStyle describes a repeated cell pattern for a path or text object.
type PatternStyle struct {
	Width      float64       `json:"width"`
	Height     float64       `json:"height"`
	XStep      *float64      `json:"x_step,omitempty"`
	YStep      *float64      `json:"y_step,omitempty"`
	Reflect    string        `json:"reflect,omitempty"`
	RelativeTo string        `json:"relative_to,omitempty"`
	CTM        *[6]float64   `json:"ctm,omitempty"`
	Alpha      *uint8        `json:"alpha,omitempty"`
	Thumbnail  *string       `json:"thumbnail,omitempty"`
	Cells      []PatternCell `json:"cells,omitempty"`
}

// SolidPaintStyle is an RGB color with optional opacity; nil Alpha means 255.
type SolidPaintStyle struct {
	Color [3]uint8 `json:"color"`
	Alpha *uint8   `json:"alpha,omitempty"`
}

// DrawParamStyle describes a reusable drawing parameter with solid, shading, or pattern paints.
type DrawParamStyle struct {
	Relative       string             `json:"relative,omitempty"`
	LineWidth      *float64           `json:"line_width,omitempty"`
	Cap            string             `json:"cap,omitempty"`
	Join           string             `json:"join,omitempty"`
	MiterLimit     *float64           `json:"miter_limit,omitempty"`
	DashPattern    *[2]float64        `json:"dash_pattern,omitempty"`
	DashSequence   []float64          `json:"dash_sequence"`
	DashOffset     *float64           `json:"dash_offset,omitempty"`
	Fill           *SolidPaintStyle   `json:"fill,omitempty"`
	Stroke         *SolidPaintStyle   `json:"stroke,omitempty"`
	FillGradient   *GradientFillStyle `json:"fill_gradient,omitempty"`
	StrokeGradient *GradientFillStyle `json:"stroke_gradient,omitempty"`
	FillGouraud    *GouraudFillStyle  `json:"fill_gouraud,omitempty"`
	StrokeGouraud  *GouraudFillStyle  `json:"stroke_gouraud,omitempty"`
	FillPattern    *PatternStyle      `json:"fill_pattern,omitempty"`
	StrokePattern  *PatternStyle      `json:"stroke_pattern,omitempty"`
}

// RenderOptions controls image resource selection during SVG, PNG, and PDF export.
type RenderOptions struct {
	PreferImageSubstitution bool
}

// DefaultImageBorderStyle returns the OFD defaults: a solid black border.
func DefaultImageBorderStyle() ImageBorderStyle {
	return ImageBorderStyle{LineWidth: 0.353, ColorAlpha: 255}
}

// CompositeObject describes a composite graphic resource placed on a page.
type CompositeObject struct {
	ID         *string   `json:"id"`
	Boundary   []float64 `json:"boundary"`
	CTM        []float64 `json:"ctm"`
	ResourceID string    `json:"resource_id"`
	Alpha      *uint8    `json:"alpha"`
	DrawParam  *string   `json:"draw_param"`
}

// CompositeUnit describes a graphic unit available through a page's resources.
type CompositeUnit struct {
	ID           string  `json:"id"`
	Width        float64 `json:"width"`
	Height       float64 `json:"height"`
	Thumbnail    *string `json:"thumbnail"`
	Substitution *string `json:"substitution"`
}

// CompositeStyle controls the appearance of a placed composite graphic unit.
type CompositeStyle struct {
	CTM       *[6]float64 `json:"ctm,omitempty"`
	Alpha     *uint8      `json:"alpha,omitempty"`
	DrawParam *string     `json:"draw_param,omitempty"`
}

// TextCode is one positioned run within a text object.
type TextCode struct {
	X      *float64 `json:"x"`
	Y      *float64 `json:"y"`
	DeltaX *string  `json:"delta_x"`
	DeltaY *string  `json:"delta_y"`
	Text   string   `json:"text"`
}

// CGTransform maps UTF-16 text positions to font glyph IDs in drawing order.
type CGTransform struct {
	CodePosition int      `json:"code_position"`
	CodeCount    int      `json:"code_count"`
	GlyphCount   int      `json:"glyph_count"`
	Glyphs       []uint32 `json:"glyphs"`
}

// TextObject includes position, font reference, and text runs.
type TextObject struct {
	ID            *string       `json:"id"`
	Boundary      []float64     `json:"boundary"`
	FontID        *string       `json:"font_id"`
	Size          *float64      `json:"size"`
	HScale        *float64      `json:"h_scale"`
	ReadDirection uint16        `json:"read_direction"`
	CharDirection uint16        `json:"char_direction"`
	Weight        uint16        `json:"weight"`
	Italic        bool          `json:"italic"`
	Codes         []TextCode    `json:"codes"`
	CGTransforms  []CGTransform `json:"cg_transforms"`
}

// Open reads an OFD package from a file.
func Open(path string) (*Document, error) {
	if err := loadAPI(); err != nil {
		return nil, err
	}
	pathBytes, err := cString(path)
	if err != nil {
		return nil, err
	}
	runtime.LockOSThread()
	defer runtime.UnlockOSThread()
	var handle uintptr
	if err := nativeError(api.open(&pathBytes[0], &handle)); err != nil {
		return nil, fmt.Errorf("open ofd: %w", err)
	}
	doc := &Document{handle: handle}
	runtime.SetFinalizer(doc, (*Document).Close)
	return doc, nil
}

// FromBytes opens an OFD package from ZIP bytes.
func FromBytes(data []byte) (*Document, error) {
	if err := loadAPI(); err != nil {
		return nil, err
	}
	var pointer *byte
	if len(data) > 0 {
		pointer = &data[0]
	}
	runtime.LockOSThread()
	defer runtime.UnlockOSThread()
	var handle uintptr
	status := api.fromBytes(pointer, uintptr(len(data)), &handle)
	runtime.KeepAlive(data)
	if err := nativeError(status); err != nil {
		return nil, fmt.Errorf("open ofd bytes: %w", err)
	}
	doc := &Document{handle: handle}
	runtime.SetFinalizer(doc, (*Document).Close)
	return doc, nil
}

// New creates an empty OFD package.
func New() (*Document, error) {
	if err := loadAPI(); err != nil {
		return nil, err
	}
	runtime.LockOSThread()
	defer runtime.UnlockOSThread()
	var handle uintptr
	if err := nativeError(api.newDocument(&handle)); err != nil {
		return nil, fmt.Errorf("create ofd: %w", err)
	}
	doc := &Document{handle: handle}
	runtime.SetFinalizer(doc, (*Document).Close)
	return doc, nil
}

// FromPDF imports a PDF containing supported vector drawing operations.
func FromPDF(data []byte) (*Document, error) {
	if err := loadAPI(); err != nil {
		return nil, err
	}
	var pointer *byte
	if len(data) > 0 {
		pointer = &data[0]
	}
	runtime.LockOSThread()
	defer runtime.UnlockOSThread()
	var handle uintptr
	status := api.fromPDF(pointer, uintptr(len(data)), &handle)
	runtime.KeepAlive(data)
	if err := nativeError(status); err != nil {
		return nil, fmt.Errorf("import pdf: %w", err)
	}
	doc := &Document{handle: handle}
	runtime.SetFinalizer(doc, (*Document).Close)
	return doc, nil
}

// CreateSESSeal returns a signed SES v4/v5 electronic seal in DER form.
func CreateSESSeal(issuerPrivateKey, issuerCertificateDER, picture []byte, spec SESSealSpec) ([]byte, error) {
	return CreateSESSealWithDistID(issuerPrivateKey, issuerCertificateDER, picture, spec, defaultSM2DistID)
}

// CreateSESSealWithDistID uses a caller-supplied SM2 issuer distinguishing ID.
func CreateSESSealWithDistID(issuerPrivateKey, issuerCertificateDER, picture []byte, spec SESSealSpec, issuerDistID string) ([]byte, error) {
	if err := loadAPI(); err != nil {
		return nil, err
	}
	options, err := json.Marshal(spec)
	if err != nil {
		return nil, fmt.Errorf("encode SES seal options: %w", err)
	}
	id, err := cString(issuerDistID)
	if err != nil {
		return nil, err
	}
	var keyPtr, certificatePtr, picturePtr *byte
	if len(issuerPrivateKey) > 0 {
		keyPtr = &issuerPrivateKey[0]
	}
	if len(issuerCertificateDER) > 0 {
		certificatePtr = &issuerCertificateDER[0]
	}
	if len(picture) > 0 {
		picturePtr = &picture[0]
	}
	runtime.LockOSThread()
	defer runtime.UnlockOSThread()
	var output nativeBuffer
	status := api.createSESSeal(keyPtr, uintptr(len(issuerPrivateKey)), certificatePtr, uintptr(len(issuerCertificateDER)), picturePtr, uintptr(len(picture)), &options[0], uintptr(len(options)), &id[0], &output)
	runtime.KeepAlive(issuerPrivateKey)
	runtime.KeepAlive(issuerCertificateDER)
	runtime.KeepAlive(picture)
	runtime.KeepAlive(options)
	if err := nativeError(status); err != nil {
		return nil, fmt.Errorf("create SES seal: %w", err)
	}
	defer api.bufferFree(output)
	return bytesAt(output)
}

// IsEncrypted reports whether the package contains OFD encryption metadata.
func (doc *Document) IsEncrypted() (bool, error) {
	doc.mu.Lock()
	defer doc.mu.Unlock()
	if err := doc.requireOpen(); err != nil {
		return false, err
	}
	runtime.LockOSThread()
	defer runtime.UnlockOSThread()
	var value int32
	if err := nativeError(api.isEncrypted(doc.handle, &value)); err != nil {
		return false, err
	}
	return value != 0, nil
}

// EncryptionInfo lists encryption operations and their user names.
func (doc *Document) EncryptionInfo() ([]EncryptionInfo, error) {
	doc.mu.Lock()
	defer doc.mu.Unlock()
	if err := doc.requireOpen(); err != nil {
		return nil, err
	}
	runtime.LockOSThread()
	defer runtime.UnlockOSThread()
	var buffer nativeBuffer
	if err := nativeError(api.encryptionInfo(doc.handle, &buffer)); err != nil {
		return nil, err
	}
	defer api.bufferFree(buffer)
	data, err := bytesAt(buffer)
	if err != nil {
		return nil, err
	}
	var infos []EncryptionInfo
	if err := json.Unmarshal(data, &infos); err != nil {
		return nil, fmt.Errorf("decode encryption info: %w", err)
	}
	return infos, nil
}

// DecryptWithPassword returns a separate plaintext document.
// An empty username tries all password users in the package.
func (doc *Document) DecryptWithPassword(password, username string) (*Document, error) {
	doc.mu.Lock()
	defer doc.mu.Unlock()
	if err := doc.requireOpen(); err != nil {
		return nil, err
	}
	passwordBytes, err := cString(password)
	if err != nil {
		return nil, err
	}
	var namePtr *byte
	if username != "" {
		name, err := cString(username)
		if err != nil {
			return nil, err
		}
		namePtr = &name[0]
	}
	runtime.LockOSThread()
	defer runtime.UnlockOSThread()
	var handle uintptr
	if err := nativeError(api.decryptWithPassword(doc.handle, &passwordBytes[0], namePtr, &handle)); err != nil {
		return nil, fmt.Errorf("decrypt OFD: %w", err)
	}
	result := &Document{handle: handle}
	runtime.SetFinalizer(result, (*Document).Close)
	return result, nil
}

// EncryptWithPasswords returns a separate package encrypted for all recipients.
func (doc *Document) EncryptWithPasswords(recipients []PasswordRecipient) (*Document, error) {
	doc.mu.Lock()
	defer doc.mu.Unlock()
	if err := doc.requireOpen(); err != nil {
		return nil, err
	}
	payload, err := json.Marshal(recipients)
	if err != nil {
		return nil, fmt.Errorf("encode recipients: %w", err)
	}
	encoded, err := cString(string(payload))
	if err != nil {
		return nil, err
	}
	runtime.LockOSThread()
	defer runtime.UnlockOSThread()
	var handle uintptr
	if err := nativeError(api.encryptWithPasswords(doc.handle, &encoded[0], &handle)); err != nil {
		return nil, fmt.Errorf("encrypt OFD: %w", err)
	}
	result := &Document{handle: handle}
	runtime.SetFinalizer(result, (*Document).Close)
	return result, nil
}

// DecryptWithSM2PrivateKey decrypts with a raw 32-byte SM2 private key.
// An empty username tries all certificate users.
func (doc *Document) DecryptWithSM2PrivateKey(privateKey []byte, username string) (*Document, error) {
	doc.mu.Lock()
	defer doc.mu.Unlock()
	if err := doc.requireOpen(); err != nil {
		return nil, err
	}
	var keyPtr *byte
	if len(privateKey) > 0 {
		keyPtr = &privateKey[0]
	}
	var namePtr *byte
	var nameBytes []byte
	if username != "" {
		var err error
		nameBytes, err = cString(username)
		if err != nil {
			return nil, err
		}
		namePtr = &nameBytes[0]
	}
	runtime.LockOSThread()
	defer runtime.UnlockOSThread()
	var handle uintptr
	if err := nativeError(api.decryptWithSM2PrivateKey(doc.handle, keyPtr, uintptr(len(privateKey)), namePtr, &handle)); err != nil {
		return nil, fmt.Errorf("decrypt OFD with SM2: %w", err)
	}
	runtime.KeepAlive(privateKey)
	runtime.KeepAlive(nameBytes)
	result := &Document{handle: handle}
	runtime.SetFinalizer(result, (*Document).Close)
	return result, nil
}

// EncryptWithCertificates returns a separate package encrypted for SM2 certificate users.
func (doc *Document) EncryptWithCertificates(recipients []CertificateRecipient) (*Document, error) {
	doc.mu.Lock()
	defer doc.mu.Unlock()
	if err := doc.requireOpen(); err != nil {
		return nil, err
	}
	payload, err := json.Marshal(recipients)
	if err != nil {
		return nil, fmt.Errorf("encode recipients: %w", err)
	}
	encoded, err := cString(string(payload))
	if err != nil {
		return nil, err
	}
	runtime.LockOSThread()
	defer runtime.UnlockOSThread()
	var handle uintptr
	if err := nativeError(api.encryptWithCertificates(doc.handle, &encoded[0], &handle)); err != nil {
		return nil, fmt.Errorf("encrypt OFD with certificates: %w", err)
	}
	result := &Document{handle: handle}
	runtime.SetFinalizer(result, (*Document).Close)
	return result, nil
}

// EncryptWithRecipients encrypts one package for password and SM2 certificate users.
func (doc *Document) EncryptWithRecipients(recipients []EncryptionRecipient) (*Document, error) {
	doc.mu.Lock()
	defer doc.mu.Unlock()
	if err := doc.requireOpen(); err != nil {
		return nil, err
	}
	payload, err := json.Marshal(recipients)
	if err != nil {
		return nil, fmt.Errorf("encode recipients: %w", err)
	}
	encoded, err := cString(string(payload))
	if err != nil {
		return nil, err
	}
	runtime.LockOSThread()
	defer runtime.UnlockOSThread()
	var handle uintptr
	if err := nativeError(api.encryptWithRecipients(doc.handle, &encoded[0], &handle)); err != nil {
		return nil, fmt.Errorf("encrypt OFD for recipients: %w", err)
	}
	result := &Document{handle: handle}
	runtime.SetFinalizer(result, (*Document).Close)
	return result, nil
}

// Close releases the native handle. It is safe to call multiple times.
func (doc *Document) Close() {
	if doc == nil {
		return
	}
	doc.mu.Lock()
	defer doc.mu.Unlock()
	if doc.handle != 0 {
		api.close(doc.handle)
		doc.handle = 0
	}
	runtime.SetFinalizer(doc, nil)
}

func (doc *Document) requireOpen() error {
	if doc.handle == 0 {
		return errors.New("document is closed")
	}
	return nil
}

// PageCount returns the number of pages.
func (doc *Document) PageCount() (int, error) {
	doc.mu.Lock()
	defer doc.mu.Unlock()
	if err := doc.requireOpen(); err != nil {
		return 0, err
	}
	runtime.LockOSThread()
	defer runtime.UnlockOSThread()
	var count uintptr
	if err := nativeError(api.pageCount(doc.handle, &count)); err != nil {
		return 0, fmt.Errorf("count pages: %w", err)
	}
	if count > uintptr(int(^uint(0)>>1)) {
		return 0, errors.New("page count exceeds Go int range")
	}
	return int(count), nil
}

// AllText returns TextCode strings for every page in package order.
func (doc *Document) AllText() ([][]string, error) {
	doc.mu.Lock()
	defer doc.mu.Unlock()
	if err := doc.requireOpen(); err != nil {
		return nil, err
	}
	runtime.LockOSThread()
	defer runtime.UnlockOSThread()
	var buffer nativeBuffer
	if err := nativeError(api.allText(doc.handle, &buffer)); err != nil {
		return nil, fmt.Errorf("read all text: %w", err)
	}
	defer api.bufferFree(buffer)
	data, err := bytesAt(buffer)
	if err != nil {
		return nil, err
	}
	var result [][]string
	if err := json.Unmarshal(data, &result); err != nil {
		return nil, fmt.Errorf("decode all text: %w", err)
	}
	return result, nil
}

// PageText returns TextCode strings from a zero-based page.
func (doc *Document) PageText(index int) ([]string, error) {
	doc.mu.Lock()
	defer doc.mu.Unlock()
	if err := doc.requireOpen(); err != nil {
		return nil, err
	}
	if index < 0 {
		return nil, errors.New("negative page index")
	}
	runtime.LockOSThread()
	defer runtime.UnlockOSThread()
	var buffer nativeBuffer
	if err := nativeError(api.pageText(doc.handle, uintptr(index), &buffer)); err != nil {
		return nil, fmt.Errorf("read page text: %w", err)
	}
	defer api.bufferFree(buffer)
	data, err := bytesAt(buffer)
	if err != nil {
		return nil, err
	}
	var result []string
	if err := json.Unmarshal(data, &result); err != nil {
		return nil, fmt.Errorf("decode page text: %w", err)
	}
	return result, nil
}

// PageSources returns template sources followed by the page's own XML source.
func (doc *Document) PageSources(index int) ([]PageSource, error) {
	doc.mu.Lock()
	defer doc.mu.Unlock()
	if err := doc.requireOpen(); err != nil {
		return nil, err
	}
	if index < 0 {
		return nil, errors.New("negative page index")
	}
	runtime.LockOSThread()
	defer runtime.UnlockOSThread()
	var buffer nativeBuffer
	if err := nativeError(api.pageSources(doc.handle, uintptr(index), &buffer)); err != nil {
		return nil, fmt.Errorf("list page sources: %w", err)
	}
	defer api.bufferFree(buffer)
	data, err := bytesAt(buffer)
	if err != nil {
		return nil, err
	}
	var sources []PageSource
	if err := json.Unmarshal(data, &sources); err != nil {
		return nil, fmt.Errorf("decode page sources: %w", err)
	}
	return sources, nil
}

// PageLayers returns page and template layers in painting order.
func (doc *Document) PageLayers(index int) ([]LayerInfo, error) {
	doc.mu.Lock()
	defer doc.mu.Unlock()
	if err := doc.requireOpen(); err != nil {
		return nil, err
	}
	if index < 0 {
		return nil, errors.New("negative page index")
	}
	runtime.LockOSThread()
	defer runtime.UnlockOSThread()
	var buffer nativeBuffer
	if err := nativeError(api.pageLayers(doc.handle, uintptr(index), &buffer)); err != nil {
		return nil, fmt.Errorf("list page layers: %w", err)
	}
	defer api.bufferFree(buffer)
	data, err := bytesAt(buffer)
	if err != nil {
		return nil, err
	}
	var layers []LayerInfo
	if err := json.Unmarshal(data, &layers); err != nil {
		return nil, fmt.Errorf("decode page layers: %w", err)
	}
	return layers, nil
}

// PromotePageToTemplate moves one page's content to a reusable template and
// applies it to that page and the additional zero-based pages.
func (doc *Document) PromotePageToTemplate(sourcePageIndex int, additionalPages []int, zOrder string) (int, error) {
	doc.mu.Lock()
	defer doc.mu.Unlock()
	if err := doc.requireOpen(); err != nil {
		return 0, err
	}
	if sourcePageIndex < 0 {
		return 0, errors.New("negative page index")
	}
	for _, index := range additionalPages {
		if index < 0 {
			return 0, errors.New("negative page index")
		}
	}
	if additionalPages == nil {
		additionalPages = []int{}
	}
	data, err := json.Marshal(additionalPages)
	if err != nil {
		return 0, err
	}
	targets, err := cString(string(data))
	if err != nil {
		return 0, err
	}
	zOrderBytes, err := cString(zOrder)
	if err != nil {
		return 0, err
	}
	runtime.LockOSThread()
	defer runtime.UnlockOSThread()
	var id uintptr
	if err := nativeError(api.promotePageToTemplate(doc.handle, uintptr(sourcePageIndex), &targets[0], &zOrderBytes[0], &id)); err != nil {
		return 0, fmt.Errorf("promote page to template: %w", err)
	}
	return int(id), nil
}

// ApplyPageTemplate attaches a document-defined template to a page.
func (doc *Document) ApplyPageTemplate(pageIndex int, templateID, zOrder string) error {
	doc.mu.Lock()
	defer doc.mu.Unlock()
	if err := doc.requireOpen(); err != nil {
		return err
	}
	if pageIndex < 0 {
		return errors.New("negative page index")
	}
	id, err := cString(templateID)
	if err != nil {
		return err
	}
	zOrderBytes, err := cString(zOrder)
	if err != nil {
		return err
	}
	runtime.LockOSThread()
	defer runtime.UnlockOSThread()
	if err := nativeError(api.applyPageTemplate(doc.handle, uintptr(pageIndex), &id[0], &zOrderBytes[0])); err != nil {
		return fmt.Errorf("apply page template: %w", err)
	}
	return nil
}

// RemovePageTemplate removes one template reference from a page.
func (doc *Document) RemovePageTemplate(pageIndex int, templateID string) error {
	doc.mu.Lock()
	defer doc.mu.Unlock()
	if err := doc.requireOpen(); err != nil {
		return err
	}
	if pageIndex < 0 {
		return errors.New("negative page index")
	}
	id, err := cString(templateID)
	if err != nil {
		return err
	}
	runtime.LockOSThread()
	defer runtime.UnlockOSThread()
	if err := nativeError(api.removePageTemplate(doc.handle, uintptr(pageIndex), &id[0])); err != nil {
		return fmt.Errorf("remove page template: %w", err)
	}
	return nil
}

// RemoveDocumentTemplate removes an unused template definition from a document body.
func (doc *Document) RemoveDocumentTemplate(documentIndex int, templateID string) error {
	doc.mu.Lock()
	defer doc.mu.Unlock()
	if err := doc.requireOpen(); err != nil {
		return err
	}
	if documentIndex < 0 {
		return errors.New("negative document index")
	}
	id, err := cString(templateID)
	if err != nil {
		return err
	}
	runtime.LockOSThread()
	defer runtime.UnlockOSThread()
	if err := nativeError(api.removeDocumentTemplate(doc.handle, uintptr(documentIndex), &id[0])); err != nil {
		return fmt.Errorf("remove document template: %w", err)
	}
	return nil
}

// PageObjects returns direct page and template graphic objects in painting order.
func (doc *Document) PageObjects(index int) ([]PageObjectInfo, error) {
	doc.mu.Lock()
	defer doc.mu.Unlock()
	if err := doc.requireOpen(); err != nil {
		return nil, err
	}
	if index < 0 {
		return nil, errors.New("negative page index")
	}
	runtime.LockOSThread()
	defer runtime.UnlockOSThread()
	var buffer nativeBuffer
	if err := nativeError(api.pageObjects(doc.handle, uintptr(index), &buffer)); err != nil {
		return nil, fmt.Errorf("list page objects: %w", err)
	}
	defer api.bufferFree(buffer)
	data, err := bytesAt(buffer)
	if err != nil {
		return nil, err
	}
	var objects []PageObjectInfo
	if err := json.Unmarshal(data, &objects); err != nil {
		return nil, fmt.Errorf("decode page objects: %w", err)
	}
	return objects, nil
}

// PageObjectClips returns object clips in painting order, including template objects.
func (doc *Document) PageObjectClips(index int) ([]PageObjectClipsInfo, error) {
	doc.mu.Lock()
	defer doc.mu.Unlock()
	if err := doc.requireOpen(); err != nil {
		return nil, err
	}
	if index < 0 {
		return nil, errors.New("negative page index")
	}
	runtime.LockOSThread()
	defer runtime.UnlockOSThread()
	var buffer nativeBuffer
	if err := nativeError(api.pageObjectClips(doc.handle, uintptr(index), &buffer)); err != nil {
		return nil, fmt.Errorf("list page object clips: %w", err)
	}
	defer api.bufferFree(buffer)
	data, err := bytesAt(buffer)
	if err != nil {
		return nil, err
	}
	var objects []PageObjectClipsInfo
	if err := json.Unmarshal(data, &objects); err != nil {
		return nil, fmt.Errorf("decode page object clips: %w", err)
	}
	return objects, nil
}

// PageObjectPaints returns paints declared directly on page and template paths/text.
// Direct and DrawParam-resolved paints are returned separately.
func (doc *Document) PageObjectPaints(index int) ([]PageObjectPaintInfo, error) {
	doc.mu.Lock()
	defer doc.mu.Unlock()
	if err := doc.requireOpen(); err != nil {
		return nil, err
	}
	if index < 0 {
		return nil, errors.New("negative page index")
	}
	runtime.LockOSThread()
	defer runtime.UnlockOSThread()
	var buffer nativeBuffer
	if err := nativeError(api.pageObjectPaints(doc.handle, uintptr(index), &buffer)); err != nil {
		return nil, fmt.Errorf("list page object paints: %w", err)
	}
	defer api.bufferFree(buffer)
	data, err := bytesAt(buffer)
	if err != nil {
		return nil, err
	}
	var paints []PageObjectPaintInfo
	if err := json.Unmarshal(data, &paints); err != nil {
		return nil, fmt.Errorf("decode page object paints: %w", err)
	}
	return paints, nil
}

// PageObjectStrokeStyles returns effective path/text stroke geometry in painting order.
func (doc *Document) PageObjectStrokeStyles(index int) ([]PageObjectStrokeStyleInfo, error) {
	doc.mu.Lock()
	defer doc.mu.Unlock()
	if err := doc.requireOpen(); err != nil {
		return nil, err
	}
	if index < 0 {
		return nil, errors.New("negative page index")
	}
	runtime.LockOSThread()
	defer runtime.UnlockOSThread()
	var buffer nativeBuffer
	if err := nativeError(api.pageObjectStrokeStyles(doc.handle, uintptr(index), &buffer)); err != nil {
		return nil, fmt.Errorf("list page object stroke styles: %w", err)
	}
	defer api.bufferFree(buffer)
	data, err := bytesAt(buffer)
	if err != nil {
		return nil, err
	}
	var styles []PageObjectStrokeStyleInfo
	if err := json.Unmarshal(data, &styles); err != nil {
		return nil, fmt.Errorf("decode page object stroke styles: %w", err)
	}
	return styles, nil
}

// PageObjectActions returns actions attached to a direct page graphic object.
func (doc *Document) PageObjectActions(pageIndex int, objectID string) ([]PageObjectActionInfo, error) {
	doc.mu.Lock()
	defer doc.mu.Unlock()
	if err := doc.requireOpen(); err != nil {
		return nil, err
	}
	if pageIndex < 0 {
		return nil, errors.New("negative page index")
	}
	id, err := cString(objectID)
	if err != nil {
		return nil, err
	}
	runtime.LockOSThread()
	defer runtime.UnlockOSThread()
	var buffer nativeBuffer
	if err := nativeError(api.pageObjectActions(doc.handle, uintptr(pageIndex), &id[0], &buffer)); err != nil {
		return nil, fmt.Errorf("list page object actions: %w", err)
	}
	defer api.bufferFree(buffer)
	data, err := bytesAt(buffer)
	if err != nil {
		return nil, err
	}
	var actions []PageObjectActionInfo
	if err := json.Unmarshal(data, &actions); err != nil {
		return nil, fmt.Errorf("decode page object actions: %w", err)
	}
	return actions, nil
}

// PageSourceObjectActions returns actions on an object in a page or referenced template source.
// sourcePath is the SourcePath returned by PageObjects.
func (doc *Document) PageSourceObjectActions(pageIndex int, sourcePath, objectID string) ([]PageObjectActionInfo, error) {
	doc.mu.Lock()
	defer doc.mu.Unlock()
	if err := doc.requireOpen(); err != nil {
		return nil, err
	}
	if pageIndex < 0 {
		return nil, errors.New("negative page index")
	}
	path, err := cString(sourcePath)
	if err != nil {
		return nil, err
	}
	id, err := cString(objectID)
	if err != nil {
		return nil, err
	}
	runtime.LockOSThread()
	defer runtime.UnlockOSThread()
	var buffer nativeBuffer
	if err := nativeError(api.pageSourceObjectActions(doc.handle, uintptr(pageIndex), &path[0], &id[0], &buffer)); err != nil {
		return nil, fmt.Errorf("list page source object actions: %w", err)
	}
	defer api.bufferFree(buffer)
	data, err := bytesAt(buffer)
	if err != nil {
		return nil, err
	}
	var actions []PageObjectActionInfo
	if err := json.Unmarshal(data, &actions); err != nil {
		return nil, fmt.Errorf("decode page source object actions: %w", err)
	}
	return actions, nil
}

// AddPageObjectURIAction appends a click action that opens a URI.
func (doc *Document) AddPageObjectURIAction(pageIndex int, objectID, uri string) error {
	doc.mu.Lock()
	defer doc.mu.Unlock()
	if err := doc.requireOpen(); err != nil {
		return err
	}
	if pageIndex < 0 {
		return errors.New("negative page index")
	}
	id, err := cString(objectID)
	if err != nil {
		return err
	}
	target, err := cString(uri)
	if err != nil {
		return err
	}
	runtime.LockOSThread()
	defer runtime.UnlockOSThread()
	if err := nativeError(api.addPageObjectURIAction(doc.handle, uintptr(pageIndex), &id[0], &target[0])); err != nil {
		return fmt.Errorf("add page object URI action: %w", err)
	}
	return nil
}

// AddPageObjectGotoAction appends a click action to a page in the same document body.
func (doc *Document) AddPageObjectGotoAction(pageIndex int, objectID string, destination Destination) error {
	doc.mu.Lock()
	defer doc.mu.Unlock()
	if err := doc.requireOpen(); err != nil {
		return err
	}
	if pageIndex < 0 {
		return errors.New("negative page index")
	}
	id, err := cString(objectID)
	if err != nil {
		return err
	}
	data, err := json.Marshal(destination)
	if err != nil {
		return fmt.Errorf("encode destination: %w", err)
	}
	payload, err := cString(string(data))
	if err != nil {
		return err
	}
	runtime.LockOSThread()
	defer runtime.UnlockOSThread()
	if err := nativeError(api.addPageObjectGotoAction(doc.handle, uintptr(pageIndex), &id[0], &payload[0])); err != nil {
		return fmt.Errorf("add page object Goto action: %w", err)
	}
	return nil
}

// AddPageObjectBookmarkAction appends a click action targeting a named bookmark.
func (doc *Document) AddPageObjectBookmarkAction(pageIndex int, objectID, bookmarkName string) error {
	doc.mu.Lock()
	defer doc.mu.Unlock()
	if err := doc.requireOpen(); err != nil {
		return err
	}
	if pageIndex < 0 {
		return errors.New("negative page index")
	}
	id, err := cString(objectID)
	if err != nil {
		return err
	}
	name, err := cString(bookmarkName)
	if err != nil {
		return err
	}
	runtime.LockOSThread()
	defer runtime.UnlockOSThread()
	if err := nativeError(api.addPageObjectBookmarkAction(doc.handle, uintptr(pageIndex), &id[0], &name[0])); err != nil {
		return fmt.Errorf("add page object bookmark action: %w", err)
	}
	return nil
}

// AddPageObjectAttachmentAction appends a click action that opens an attachment.
// A nil newWindow omits the OFD NewWindow attribute.
func (doc *Document) AddPageObjectAttachmentAction(pageIndex int, objectID, attachmentID string, newWindow *bool) error {
	doc.mu.Lock()
	defer doc.mu.Unlock()
	if err := doc.requireOpen(); err != nil {
		return err
	}
	if pageIndex < 0 {
		return errors.New("negative page index")
	}
	id, err := cString(objectID)
	if err != nil {
		return err
	}
	attachment, err := cString(attachmentID)
	if err != nil {
		return err
	}
	flag := int32(-1)
	if newWindow != nil {
		flag = 0
		if *newWindow {
			flag = 1
		}
	}
	runtime.LockOSThread()
	defer runtime.UnlockOSThread()
	if err := nativeError(api.addPageObjectAttachmentAction(doc.handle, uintptr(pageIndex), &id[0], &attachment[0], flag)); err != nil {
		return fmt.Errorf("add page object attachment action: %w", err)
	}
	return nil
}

func (doc *Document) addPageObjectMediaAction(pageIndex int, objectID string, payload map[string]any) error {
	doc.mu.Lock()
	defer doc.mu.Unlock()
	if err := doc.requireOpen(); err != nil {
		return err
	}
	if pageIndex < 0 {
		return errors.New("negative page index")
	}
	id, err := cString(objectID)
	if err != nil {
		return err
	}
	encoded, err := json.Marshal(payload)
	if err != nil {
		return fmt.Errorf("encode media action: %w", err)
	}
	data, err := cString(string(encoded))
	if err != nil {
		return err
	}
	runtime.LockOSThread()
	defer runtime.UnlockOSThread()
	if err := nativeError(api.addPageObjectMediaAction(doc.handle, uintptr(pageIndex), &id[0], &data[0])); err != nil {
		return fmt.Errorf("add page object media action: %w", err)
	}
	return nil
}

// AddPageObjectSoundAction appends a click Sound action using an Audio resource.
func (doc *Document) AddPageObjectSoundAction(pageIndex int, objectID, resourceID string, options SoundActionOptions) error {
	return doc.addPageObjectMediaAction(pageIndex, objectID, map[string]any{
		"kind": "Sound", "resource_id": resourceID,
		"volume": options.Volume, "repeat": options.Repeat, "synchronous": options.Synchronous,
	})
}

// AddPageObjectMovieAction appends a click Movie action using a Video resource.
// An empty operator uses the OFD default Play behavior.
func (doc *Document) AddPageObjectMovieAction(pageIndex int, objectID, resourceID, operator string) error {
	payload := map[string]any{"kind": "Movie", "resource_id": resourceID}
	if operator != "" {
		payload["operator"] = operator
	}
	return doc.addPageObjectMediaAction(pageIndex, objectID, payload)
}

// RemovePageObjectAction removes one action by its zero-based index on a page object.
func (doc *Document) RemovePageObjectAction(pageIndex int, objectID string, actionIndex int) error {
	doc.mu.Lock()
	defer doc.mu.Unlock()
	if err := doc.requireOpen(); err != nil {
		return err
	}
	if pageIndex < 0 || actionIndex < 0 {
		return errors.New("negative page or action index")
	}
	id, err := cString(objectID)
	if err != nil {
		return err
	}
	runtime.LockOSThread()
	defer runtime.UnlockOSThread()
	if err := nativeError(api.removePageObjectAction(doc.handle, uintptr(pageIndex), &id[0], uintptr(actionIndex))); err != nil {
		return fmt.Errorf("remove page object action: %w", err)
	}
	return nil
}

// SetPageObjectActionRegions sets rectangular trigger areas. An empty slice clears the explicit region.
func (doc *Document) SetPageObjectActionRegions(pageIndex int, objectID string, actionIndex int, rectangles [][4]float64) error {
	doc.mu.Lock()
	defer doc.mu.Unlock()
	if err := doc.requireOpen(); err != nil {
		return err
	}
	if pageIndex < 0 || actionIndex < 0 {
		return errors.New("negative page or action index")
	}
	id, err := cString(objectID)
	if err != nil {
		return err
	}
	if rectangles == nil {
		rectangles = [][4]float64{}
	}
	data, err := json.Marshal(rectangles)
	if err != nil {
		return fmt.Errorf("encode action regions: %w", err)
	}
	payload, err := cString(string(data))
	if err != nil {
		return err
	}
	runtime.LockOSThread()
	defer runtime.UnlockOSThread()
	if err := nativeError(api.setPageObjectActionRegions(doc.handle, uintptr(pageIndex), &id[0], uintptr(actionIndex), &payload[0])); err != nil {
		return fmt.Errorf("set page object action regions: %w", err)
	}
	return nil
}

// PageTextInRegion returns characters whose text origins fall inside a page
// rectangle in millimeters. The page index is zero-based.
func (doc *Document) PageTextInRegion(index int, x, y, width, height float64) ([]string, error) {
	doc.mu.Lock()
	defer doc.mu.Unlock()
	if err := doc.requireOpen(); err != nil {
		return nil, err
	}
	if index < 0 {
		return nil, errors.New("negative page index")
	}
	runtime.LockOSThread()
	defer runtime.UnlockOSThread()
	var buffer nativeBuffer
	if err := nativeError(api.regionText(doc.handle, uintptr(index), x, y, width, height, &buffer)); err != nil {
		return nil, fmt.Errorf("read page text in region: %w", err)
	}
	defer api.bufferFree(buffer)
	data, err := bytesAt(buffer)
	if err != nil {
		return nil, err
	}
	var text []string
	if err := json.Unmarshal(data, &text); err != nil {
		return nil, fmt.Errorf("decode page text in region: %w", err)
	}
	return text, nil
}

// KeywordPositions finds keyword matches and approximate page-space boxes.
// Page indexes start at zero; nil pages searches all pages.
func (doc *Document) KeywordPositions(keywords []string, pages []int) ([]KeywordPosition, error) {
	doc.mu.Lock()
	defer doc.mu.Unlock()
	if err := doc.requireOpen(); err != nil {
		return nil, err
	}
	for _, page := range pages {
		if page < 0 {
			return nil, errors.New("negative page index")
		}
	}
	// This fixed value contains only string and integer slices, which json.Marshal supports.
	query, _ := json.Marshal(struct {
		Keywords []string `json:"keywords"`
		Pages    []int    `json:"pages"`
	}{Keywords: keywords, Pages: pages})
	input := append(query, 0)
	runtime.LockOSThread()
	defer runtime.UnlockOSThread()
	var buffer nativeBuffer
	if err := nativeError(api.keywordPositions(doc.handle, &input[0], &buffer)); err != nil {
		return nil, fmt.Errorf("find keyword positions: %w", err)
	}
	defer api.bufferFree(buffer)
	data, err := bytesAt(buffer)
	if err != nil {
		return nil, err
	}
	var positions []KeywordPosition
	if err := json.Unmarshal(data, &positions); err != nil {
		return nil, fmt.Errorf("decode keyword positions: %w", err)
	}
	return positions, nil
}

// PageTextWithTemplates returns TextCode strings from templates and the page.
func (doc *Document) PageTextWithTemplates(index int) ([]string, error) {
	doc.mu.Lock()
	defer doc.mu.Unlock()
	if err := doc.requireOpen(); err != nil {
		return nil, err
	}
	if index < 0 {
		return nil, errors.New("negative page index")
	}
	runtime.LockOSThread()
	defer runtime.UnlockOSThread()
	var buffer nativeBuffer
	if err := nativeError(api.templateText(doc.handle, uintptr(index), &buffer)); err != nil {
		return nil, fmt.Errorf("read text with templates: %w", err)
	}
	defer api.bufferFree(buffer)
	data, err := bytesAt(buffer)
	if err != nil {
		return nil, err
	}
	var text []string
	if err := json.Unmarshal(data, &text); err != nil {
		return nil, fmt.Errorf("decode text with templates: %w", err)
	}
	return text, nil
}

// Pages returns page metadata in document order.
func (doc *Document) Pages() ([]PageInfo, error) {
	doc.mu.Lock()
	defer doc.mu.Unlock()
	if err := doc.requireOpen(); err != nil {
		return nil, err
	}
	runtime.LockOSThread()
	defer runtime.UnlockOSThread()
	var buffer nativeBuffer
	if err := nativeError(api.pages(doc.handle, &buffer)); err != nil {
		return nil, fmt.Errorf("list pages: %w", err)
	}
	defer api.bufferFree(buffer)
	data, err := bytesAt(buffer)
	if err != nil {
		return nil, err
	}
	var pages []PageInfo
	if err := json.Unmarshal(data, &pages); err != nil {
		return nil, fmt.Errorf("decode pages: %w", err)
	}
	return pages, nil
}

// AppendDocumentBody adds an empty document body and returns its zero-based index.
func (doc *Document) AppendDocumentBody() (int, error) {
	doc.mu.Lock()
	defer doc.mu.Unlock()
	if err := doc.requireOpen(); err != nil {
		return 0, err
	}
	runtime.LockOSThread()
	defer runtime.UnlockOSThread()
	var index uintptr
	if err := nativeError(api.appendDocumentBody(doc.handle, &index)); err != nil {
		return 0, fmt.Errorf("append document body: %w", err)
	}
	return int(index), nil
}

// MoveDocumentBody moves a document body to another zero-based position.
func (doc *Document) MoveDocumentBody(fromIndex, toIndex int) error {
	doc.mu.Lock()
	defer doc.mu.Unlock()
	if err := doc.requireOpen(); err != nil {
		return err
	}
	if fromIndex < 0 || toIndex < 0 {
		return fmt.Errorf("document body index must be nonnegative")
	}
	runtime.LockOSThread()
	defer runtime.UnlockOSThread()
	if err := nativeError(api.moveDocumentBody(doc.handle, uintptr(fromIndex), uintptr(toIndex))); err != nil {
		return fmt.Errorf("move document body: %w", err)
	}
	return nil
}

// RemoveDocumentBody removes a body from the package order and retains its files.
func (doc *Document) RemoveDocumentBody(index int) error {
	doc.mu.Lock()
	defer doc.mu.Unlock()
	if err := doc.requireOpen(); err != nil {
		return err
	}
	if index < 0 {
		return fmt.Errorf("document body index must be nonnegative")
	}
	runtime.LockOSThread()
	defer runtime.UnlockOSThread()
	if err := nativeError(api.removeDocumentBody(doc.handle, uintptr(index))); err != nil {
		return fmt.Errorf("remove document body: %w", err)
	}
	return nil
}

// ExtractDocumentBody returns a separate package with one active body.
func (doc *Document) ExtractDocumentBody(index int) (*Document, error) {
	doc.mu.Lock()
	defer doc.mu.Unlock()
	if err := doc.requireOpen(); err != nil {
		return nil, err
	}
	if index < 0 {
		return nil, fmt.Errorf("document body index must be nonnegative")
	}
	runtime.LockOSThread()
	defer runtime.UnlockOSThread()
	var handle uintptr
	if err := nativeError(api.extractDocumentBody(doc.handle, uintptr(index), &handle)); err != nil {
		return nil, fmt.Errorf("extract document body: %w", err)
	}
	extracted := &Document{handle: handle}
	runtime.SetFinalizer(extracted, (*Document).Close)
	return extracted, nil
}

// AppendDocumentBodiesFrom imports all bodies from an unsigned source package.
// The bodies remain separate and retain their page order and resources.
func (doc *Document) AppendDocumentBodiesFrom(source *Document) ([]int, error) {
	if doc == nil || source == nil {
		return nil, errors.New("document is nil")
	}
	first, second := doc, source
	if uintptr(unsafe.Pointer(first)) > uintptr(unsafe.Pointer(second)) {
		first, second = second, first
	}
	first.mu.Lock()
	defer first.mu.Unlock()
	if second != first {
		second.mu.Lock()
		defer second.mu.Unlock()
	}
	if err := doc.requireOpen(); err != nil {
		return nil, err
	}
	if err := source.requireOpen(); err != nil {
		return nil, err
	}
	runtime.LockOSThread()
	defer runtime.UnlockOSThread()
	var buffer nativeBuffer
	if err := nativeError(api.appendDocumentBodiesFrom(doc.handle, source.handle, &buffer)); err != nil {
		return nil, fmt.Errorf("append document bodies: %w", err)
	}
	defer api.bufferFree(buffer)
	data, err := bytesAt(buffer)
	if err != nil {
		return nil, err
	}
	var indexes []int
	if err := json.Unmarshal(data, &indexes); err != nil {
		return nil, fmt.Errorf("decode imported document indexes: %w", err)
	}
	return indexes, nil
}

// AppendDocumentBodyFrom imports one selected body from an unsigned source.
func (doc *Document) AppendDocumentBodyFrom(source *Document, sourceIndex int) (int, error) {
	if doc == nil || source == nil {
		return 0, errors.New("document is nil")
	}
	if sourceIndex < 0 {
		return 0, errors.New("document body index must be nonnegative")
	}
	first, second := doc, source
	if uintptr(unsafe.Pointer(first)) > uintptr(unsafe.Pointer(second)) {
		first, second = second, first
	}
	first.mu.Lock()
	defer first.mu.Unlock()
	if second != first {
		second.mu.Lock()
		defer second.mu.Unlock()
	}
	if err := doc.requireOpen(); err != nil {
		return 0, err
	}
	if err := source.requireOpen(); err != nil {
		return 0, err
	}
	runtime.LockOSThread()
	defer runtime.UnlockOSThread()
	var index uintptr
	if err := nativeError(api.appendDocumentBodyFrom(doc.handle, source.handle, uintptr(sourceIndex), &index)); err != nil {
		return 0, fmt.Errorf("append document body: %w", err)
	}
	return int(index), nil
}

// SetPageArea writes explicit page boxes while preserving unrelated page XML.
func (doc *Document) SetPageArea(index int, area PageArea) error {
	doc.mu.Lock()
	defer doc.mu.Unlock()
	if err := doc.requireOpen(); err != nil {
		return err
	}
	if index < 0 {
		return errors.New("negative page index")
	}
	payload, err := json.Marshal(area)
	if err != nil {
		return fmt.Errorf("encode page area: %w", err)
	}
	bytes, err := cString(string(payload))
	if err != nil {
		return err
	}
	runtime.LockOSThread()
	defer runtime.UnlockOSThread()
	if err := nativeError(api.setPageArea(doc.handle, uintptr(index), &bytes[0])); err != nil {
		return fmt.Errorf("set page area: %w", err)
	}
	return nil
}

// DocumentPageArea returns the explicit default page area of one document body.
func (doc *Document) DocumentPageArea(documentIndex int) (*PageArea, error) {
	doc.mu.Lock()
	defer doc.mu.Unlock()
	if err := doc.requireOpen(); err != nil {
		return nil, err
	}
	if documentIndex < 0 {
		return nil, errors.New("negative document index")
	}
	runtime.LockOSThread()
	defer runtime.UnlockOSThread()
	var buffer nativeBuffer
	if err := nativeError(api.documentPageArea(doc.handle, uintptr(documentIndex), &buffer)); err != nil {
		return nil, fmt.Errorf("read document page area: %w", err)
	}
	defer api.bufferFree(buffer)
	data, err := bytesAt(buffer)
	if err != nil {
		return nil, err
	}
	var area *PageArea
	if err := json.Unmarshal(data, &area); err != nil {
		return nil, fmt.Errorf("decode document page area: %w", err)
	}
	return area, nil
}

// SetDocumentPageArea writes default page boxes for one document body.
func (doc *Document) SetDocumentPageArea(documentIndex int, area PageArea) error {
	doc.mu.Lock()
	defer doc.mu.Unlock()
	if err := doc.requireOpen(); err != nil {
		return err
	}
	if documentIndex < 0 {
		return errors.New("negative document index")
	}
	payload, err := json.Marshal(area)
	if err != nil {
		return fmt.Errorf("encode document page area: %w", err)
	}
	bytes, err := cString(string(payload))
	if err != nil {
		return err
	}
	runtime.LockOSThread()
	defer runtime.UnlockOSThread()
	if err := nativeError(api.setDocumentPageArea(doc.handle, uintptr(documentIndex), &bytes[0])); err != nil {
		return fmt.Errorf("set document page area: %w", err)
	}
	return nil
}

// MovePage moves a page to another zero-based position in the same document body.
func (doc *Document) MovePage(fromIndex, toIndex int) error {
	doc.mu.Lock()
	defer doc.mu.Unlock()
	if err := doc.requireOpen(); err != nil {
		return err
	}
	if fromIndex < 0 || toIndex < 0 {
		return fmt.Errorf("page index must be nonnegative")
	}
	runtime.LockOSThread()
	defer runtime.UnlockOSThread()
	if err := nativeError(api.movePage(doc.handle, uintptr(fromIndex), uintptr(toIndex))); err != nil {
		return fmt.Errorf("move page: %w", err)
	}
	return nil
}

// ReorderDocumentPages atomically reorders one body's pages by local index.
func (doc *Document) ReorderDocumentPages(documentIndex int, order []int) error {
	doc.mu.Lock()
	defer doc.mu.Unlock()
	if err := doc.requireOpen(); err != nil {
		return err
	}
	if documentIndex < 0 {
		return errors.New("negative document index")
	}
	for _, index := range order {
		if index < 0 {
			return errors.New("negative page index")
		}
	}
	if order == nil {
		order = []int{}
	}
	payload, err := textRequest(order)
	if err != nil {
		return err
	}
	runtime.LockOSThread()
	defer runtime.UnlockOSThread()
	if err := nativeError(api.reorderDocumentPages(doc.handle, uintptr(documentIndex), &payload[0])); err != nil {
		return fmt.Errorf("reorder document pages: %w", err)
	}
	return nil
}

// DuplicatePage appends a copy of a page to its document body and returns its index.
func (doc *Document) DuplicatePage(index int) (int, error) {
	doc.mu.Lock()
	defer doc.mu.Unlock()
	if err := doc.requireOpen(); err != nil {
		return 0, err
	}
	if index < 0 {
		return 0, fmt.Errorf("page index must be nonnegative")
	}
	var output uintptr
	runtime.LockOSThread()
	defer runtime.UnlockOSThread()
	if err := nativeError(api.duplicatePage(doc.handle, uintptr(index), &output)); err != nil {
		return 0, fmt.Errorf("duplicate page: %w", err)
	}
	if output > uintptr(int(^uint(0)>>1)) {
		return 0, errors.New("page index exceeds Go int range")
	}
	return int(output), nil
}

// DuplicatePageAt copies a page into a body-local position and returns its global index.
func (doc *Document) DuplicatePageAt(index, position int) (int, error) {
	doc.mu.Lock()
	defer doc.mu.Unlock()
	if err := doc.requireOpen(); err != nil {
		return 0, err
	}
	if index < 0 || position < 0 {
		return 0, fmt.Errorf("page index and position must be nonnegative")
	}
	var output uintptr
	runtime.LockOSThread()
	defer runtime.UnlockOSThread()
	if err := nativeError(api.duplicatePageAt(doc.handle, uintptr(index), uintptr(position), &output)); err != nil {
		return 0, fmt.Errorf("duplicate page at position: %w", err)
	}
	if output > uintptr(int(^uint(0)>>1)) {
		return 0, errors.New("page index exceeds Go int range")
	}
	return int(output), nil
}

// RemovePage removes a zero-based page from document order if it has no dependencies.
func (doc *Document) RemovePage(index int) error {
	doc.mu.Lock()
	defer doc.mu.Unlock()
	if err := doc.requireOpen(); err != nil {
		return err
	}
	if index < 0 {
		return fmt.Errorf("page index must be nonnegative")
	}
	runtime.LockOSThread()
	defer runtime.UnlockOSThread()
	if err := nativeError(api.removePage(doc.handle, uintptr(index))); err != nil {
		return fmt.Errorf("remove page: %w", err)
	}
	return nil
}

// ValidateReferences checks IDs, geometry, and supported resource references.
func (doc *Document) ValidateReferences() ([]ValidationIssue, error) {
	doc.mu.Lock()
	defer doc.mu.Unlock()
	if err := doc.requireOpen(); err != nil {
		return nil, err
	}
	runtime.LockOSThread()
	defer runtime.UnlockOSThread()
	var buffer nativeBuffer
	if err := nativeError(api.validateReferences(doc.handle, &buffer)); err != nil {
		return nil, fmt.Errorf("validate references: %w", err)
	}
	defer api.bufferFree(buffer)
	data, err := bytesAt(buffer)
	if err != nil {
		return nil, err
	}
	var issues []ValidationIssue
	if err := json.Unmarshal(data, &issues); err != nil {
		return nil, fmt.Errorf("decode validation issues: %w", err)
	}
	return issues, nil
}

// ArchiveViolations checks the currently supported OFD-A archival rules.
// An empty result does not establish full OFD-A conformance.
func (doc *Document) ArchiveViolations() ([]ArchiveViolation, error) {
	doc.mu.Lock()
	defer doc.mu.Unlock()
	if err := doc.requireOpen(); err != nil {
		return nil, err
	}
	runtime.LockOSThread()
	defer runtime.UnlockOSThread()
	var buffer nativeBuffer
	if err := nativeError(api.archiveViolations(doc.handle, &buffer)); err != nil {
		return nil, fmt.Errorf("check OFD-A rules: %w", err)
	}
	defer api.bufferFree(buffer)
	data, err := bytesAt(buffer)
	if err != nil {
		return nil, err
	}
	var issues []ArchiveViolation
	if err := json.Unmarshal(data, &issues); err != nil {
		return nil, fmt.Errorf("decode OFD-A violations: %w", err)
	}
	return issues, nil
}

// PrepareArchive returns a separate document with supported OFD-A normalizations.
// Call ArchiveViolations on the result to inspect remaining archival issues.
func (doc *Document) PrepareArchive() (*Document, error) {
	doc.mu.Lock()
	defer doc.mu.Unlock()
	if err := doc.requireOpen(); err != nil {
		return nil, err
	}
	runtime.LockOSThread()
	defer runtime.UnlockOSThread()
	var handle uintptr
	if err := nativeError(api.prepareArchive(doc.handle, &handle)); err != nil {
		return nil, fmt.Errorf("prepare OFD archive: %w", err)
	}
	result := &Document{handle: handle}
	runtime.SetFinalizer(result, (*Document).Close)
	return result, nil
}

// Entries returns sorted, normalized package entry names.
func (doc *Document) Entries() ([]string, error) {
	doc.mu.Lock()
	defer doc.mu.Unlock()
	if err := doc.requireOpen(); err != nil {
		return nil, err
	}
	runtime.LockOSThread()
	defer runtime.UnlockOSThread()
	var buffer nativeBuffer
	if err := nativeError(api.entries(doc.handle, &buffer)); err != nil {
		return nil, fmt.Errorf("list entries: %w", err)
	}
	defer api.bufferFree(buffer)
	data, err := bytesAt(buffer)
	if err != nil {
		return nil, err
	}
	var entries []string
	if err := json.Unmarshal(data, &entries); err != nil {
		return nil, fmt.Errorf("decode entries: %w", err)
	}
	return entries, nil
}

// Documents returns metadata for every document body.
func (doc *Document) Documents() ([]DocumentInfo, error) {
	doc.mu.Lock()
	defer doc.mu.Unlock()
	if err := doc.requireOpen(); err != nil {
		return nil, err
	}
	runtime.LockOSThread()
	defer runtime.UnlockOSThread()
	var buffer nativeBuffer
	if err := nativeError(api.documents(doc.handle, &buffer)); err != nil {
		return nil, fmt.Errorf("read document metadata: %w", err)
	}
	defer api.bufferFree(buffer)
	data, err := bytesAt(buffer)
	if err != nil {
		return nil, err
	}
	var infos []DocumentInfo
	if err := json.Unmarshal(data, &infos); err != nil {
		return nil, fmt.Errorf("decode document metadata: %w", err)
	}
	return infos, nil
}

// PackageInfo contains the root OFD.xml attributes.
type PackageInfo struct {
	Version *string `json:"version"`
	DocType *string `json:"doc_type"`
}

// PackageInfo reads the package version and document type.
func (doc *Document) PackageInfo() (PackageInfo, error) {
	doc.mu.Lock()
	defer doc.mu.Unlock()
	if err := doc.requireOpen(); err != nil {
		return PackageInfo{}, err
	}
	runtime.LockOSThread()
	defer runtime.UnlockOSThread()
	var buffer nativeBuffer
	if err := nativeError(api.packageInfo(doc.handle, &buffer)); err != nil {
		return PackageInfo{}, fmt.Errorf("read package info: %w", err)
	}
	defer api.bufferFree(buffer)
	data, err := bytesAt(buffer)
	if err != nil {
		return PackageInfo{}, err
	}
	var info PackageInfo
	if err := json.Unmarshal(data, &info); err != nil {
		return PackageInfo{}, fmt.Errorf("decode package info: %w", err)
	}
	return info, nil
}

// SetPackageHeader updates root attributes in an unsigned package.
func (doc *Document) SetPackageHeader(version, docType string) error {
	doc.mu.Lock()
	defer doc.mu.Unlock()
	if err := doc.requireOpen(); err != nil {
		return err
	}
	versionBytes, err := cString(version)
	if err != nil {
		return err
	}
	typeBytes, err := cString(docType)
	if err != nil {
		return err
	}
	runtime.LockOSThread()
	defer runtime.UnlockOSThread()
	if err := nativeError(api.setPackageHeader(doc.handle, &versionBytes[0], &typeBytes[0])); err != nil {
		return fmt.Errorf("set package header: %w", err)
	}
	return nil
}

// DocumentPermissions returns the effective permissions for one document body.
func (doc *Document) DocumentPermissions(documentIndex int) (DocumentPermissions, error) {
	doc.mu.Lock()
	defer doc.mu.Unlock()
	var permissions DocumentPermissions
	if err := doc.requireOpen(); err != nil {
		return permissions, err
	}
	if documentIndex < 0 {
		return permissions, errors.New("negative document index")
	}
	runtime.LockOSThread()
	defer runtime.UnlockOSThread()
	var buffer nativeBuffer
	if err := nativeError(api.documentPermissions(doc.handle, uintptr(documentIndex), &buffer)); err != nil {
		return permissions, fmt.Errorf("read document permissions: %w", err)
	}
	defer api.bufferFree(buffer)
	data, err := bytesAt(buffer)
	if err != nil {
		return permissions, err
	}
	if err := json.Unmarshal(data, &permissions); err != nil {
		return permissions, fmt.Errorf("decode document permissions: %w", err)
	}
	return permissions, nil
}

// SetDocumentPermissions writes a complete permission declaration.
func (doc *Document) SetDocumentPermissions(documentIndex int, permissions DocumentPermissions) error {
	doc.mu.Lock()
	defer doc.mu.Unlock()
	if err := doc.requireOpen(); err != nil {
		return err
	}
	if documentIndex < 0 {
		return errors.New("negative document index")
	}
	data, err := json.Marshal(permissions)
	if err != nil {
		return err
	}
	bytes, err := cString(string(data))
	if err != nil {
		return err
	}
	runtime.LockOSThread()
	defer runtime.UnlockOSThread()
	if err := nativeError(api.setDocumentPermissions(doc.handle, uintptr(documentIndex), &bytes[0])); err != nil {
		return fmt.Errorf("set document permissions: %w", err)
	}
	return nil
}

// ViewPreferences returns effective initial viewer preferences for one document body.
func (doc *Document) ViewPreferences(documentIndex int) (ViewPreferences, error) {
	doc.mu.Lock()
	defer doc.mu.Unlock()
	var preferences ViewPreferences
	if err := doc.requireOpen(); err != nil {
		return preferences, err
	}
	if documentIndex < 0 {
		return preferences, errors.New("negative document index")
	}
	runtime.LockOSThread()
	defer runtime.UnlockOSThread()
	var buffer nativeBuffer
	if err := nativeError(api.viewPreferences(doc.handle, uintptr(documentIndex), &buffer)); err != nil {
		return preferences, fmt.Errorf("read view preferences: %w", err)
	}
	defer api.bufferFree(buffer)
	data, err := bytesAt(buffer)
	if err != nil {
		return preferences, err
	}
	if err := json.Unmarshal(data, &preferences); err != nil {
		return preferences, fmt.Errorf("decode view preferences: %w", err)
	}
	return preferences, nil
}

// SetViewPreferences writes complete initial viewer preferences.
func (doc *Document) SetViewPreferences(documentIndex int, preferences ViewPreferences) error {
	doc.mu.Lock()
	defer doc.mu.Unlock()
	if err := doc.requireOpen(); err != nil {
		return err
	}
	if documentIndex < 0 {
		return errors.New("negative document index")
	}
	data, err := json.Marshal(preferences)
	if err != nil {
		return err
	}
	bytes, err := cString(string(data))
	if err != nil {
		return err
	}
	runtime.LockOSThread()
	defer runtime.UnlockOSThread()
	if err := nativeError(api.setViewPreferences(doc.handle, uintptr(documentIndex), &bytes[0])); err != nil {
		return fmt.Errorf("set view preferences: %w", err)
	}
	return nil
}

// DocumentActions returns actions declared directly on one document body.
func (doc *Document) DocumentActions(documentIndex int) ([]DocumentActionInfo, error) {
	doc.mu.Lock()
	defer doc.mu.Unlock()
	if err := doc.requireOpen(); err != nil {
		return nil, err
	}
	if documentIndex < 0 {
		return nil, errors.New("negative document index")
	}
	runtime.LockOSThread()
	defer runtime.UnlockOSThread()
	var buffer nativeBuffer
	if err := nativeError(api.documentActions(doc.handle, uintptr(documentIndex), &buffer)); err != nil {
		return nil, fmt.Errorf("read document actions: %w", err)
	}
	defer api.bufferFree(buffer)
	data, err := bytesAt(buffer)
	if err != nil {
		return nil, err
	}
	var actions []DocumentActionInfo
	if err := json.Unmarshal(data, &actions); err != nil {
		return nil, fmt.Errorf("decode document actions: %w", err)
	}
	return actions, nil
}

// AddDocumentAction appends one document action.
func (doc *Document) AddDocumentAction(documentIndex int, action DocumentActionSpec) error {
	doc.mu.Lock()
	defer doc.mu.Unlock()
	if err := doc.requireOpen(); err != nil {
		return err
	}
	if documentIndex < 0 {
		return errors.New("negative document index")
	}
	data, err := json.Marshal(action)
	if err != nil {
		return err
	}
	bytes, err := cString(string(data))
	if err != nil {
		return err
	}
	runtime.LockOSThread()
	defer runtime.UnlockOSThread()
	if err := nativeError(api.addDocumentAction(doc.handle, uintptr(documentIndex), &bytes[0])); err != nil {
		return fmt.Errorf("add document action: %w", err)
	}
	return nil
}

// RemoveDocumentAction removes one direct document action by zero-based index.
func (doc *Document) RemoveDocumentAction(documentIndex, actionIndex int) error {
	doc.mu.Lock()
	defer doc.mu.Unlock()
	if err := doc.requireOpen(); err != nil {
		return err
	}
	if documentIndex < 0 || actionIndex < 0 {
		return errors.New("negative document or action index")
	}
	runtime.LockOSThread()
	defer runtime.UnlockOSThread()
	if err := nativeError(api.removeDocumentAction(doc.handle, uintptr(documentIndex), uintptr(actionIndex))); err != nil {
		return fmt.Errorf("remove document action: %w", err)
	}
	return nil
}

// PageActions returns actions attached directly to a page.
func (doc *Document) PageActions(pageIndex int) ([]DocumentActionInfo, error) {
	doc.mu.Lock()
	defer doc.mu.Unlock()
	if err := doc.requireOpen(); err != nil {
		return nil, err
	}
	if pageIndex < 0 {
		return nil, errors.New("negative page index")
	}
	runtime.LockOSThread()
	defer runtime.UnlockOSThread()
	var buffer nativeBuffer
	if err := nativeError(api.pageActions(doc.handle, uintptr(pageIndex), &buffer)); err != nil {
		return nil, fmt.Errorf("read page actions: %w", err)
	}
	defer api.bufferFree(buffer)
	data, err := bytesAt(buffer)
	if err != nil {
		return nil, err
	}
	var actions []DocumentActionInfo
	if err := json.Unmarshal(data, &actions); err != nil {
		return nil, fmt.Errorf("decode page actions: %w", err)
	}
	return actions, nil
}

// AddPageAction appends a page-open action. Event must be "PO".
func (doc *Document) AddPageAction(pageIndex int, action DocumentActionSpec) error {
	doc.mu.Lock()
	defer doc.mu.Unlock()
	if err := doc.requireOpen(); err != nil {
		return err
	}
	if pageIndex < 0 {
		return errors.New("negative page index")
	}
	data, err := json.Marshal(action)
	if err != nil {
		return err
	}
	bytes, err := cString(string(data))
	if err != nil {
		return err
	}
	runtime.LockOSThread()
	defer runtime.UnlockOSThread()
	if err := nativeError(api.addPageAction(doc.handle, uintptr(pageIndex), &bytes[0])); err != nil {
		return fmt.Errorf("add page action: %w", err)
	}
	return nil
}

// RemovePageAction removes a direct page action by zero-based index.
func (doc *Document) RemovePageAction(pageIndex, actionIndex int) error {
	doc.mu.Lock()
	defer doc.mu.Unlock()
	if err := doc.requireOpen(); err != nil {
		return err
	}
	if pageIndex < 0 || actionIndex < 0 {
		return errors.New("negative page or action index")
	}
	runtime.LockOSThread()
	defer runtime.UnlockOSThread()
	if err := nativeError(api.removePageAction(doc.handle, uintptr(pageIndex), uintptr(actionIndex))); err != nil {
		return fmt.Errorf("remove page action: %w", err)
	}
	return nil
}

// IntegrityManifest reads the optional OFDEntries.xml file manifest.
func (doc *Document) IntegrityManifest() (*IntegrityManifestInfo, error) {
	doc.mu.Lock()
	defer doc.mu.Unlock()
	if err := doc.requireOpen(); err != nil {
		return nil, err
	}
	runtime.LockOSThread()
	defer runtime.UnlockOSThread()
	var buffer nativeBuffer
	if err := nativeError(api.integrityManifest(doc.handle, &buffer)); err != nil {
		return nil, fmt.Errorf("read integrity manifest: %w", err)
	}
	defer api.bufferFree(buffer)
	data, err := bytesAt(buffer)
	if err != nil {
		return nil, err
	}
	var manifest *IntegrityManifestInfo
	if err := json.Unmarshal(data, &manifest); err != nil {
		return nil, fmt.Errorf("decode integrity manifest: %w", err)
	}
	return manifest, nil
}

// CheckIntegrityManifest reports missing and extra package files.
// Signature validity must be checked separately.
func (doc *Document) CheckIntegrityManifest() (*IntegrityManifestCheck, error) {
	doc.mu.Lock()
	defer doc.mu.Unlock()
	if err := doc.requireOpen(); err != nil {
		return nil, err
	}
	runtime.LockOSThread()
	defer runtime.UnlockOSThread()
	var buffer nativeBuffer
	if err := nativeError(api.checkIntegrityManifest(doc.handle, &buffer)); err != nil {
		return nil, fmt.Errorf("check integrity manifest: %w", err)
	}
	defer api.bufferFree(buffer)
	data, err := bytesAt(buffer)
	if err != nil {
		return nil, err
	}
	var report *IntegrityManifestCheck
	if err := json.Unmarshal(data, &report); err != nil {
		return nil, fmt.Errorf("decode integrity manifest check: %w", err)
	}
	return report, nil
}

// VerifyIntegrityManifestSM2 checks the manifest signature with the default SM2 ID.
func (doc *Document) VerifyIntegrityManifestSM2() (*IntegrityVerificationInfo, error) {
	return doc.verifyIntegrityManifestSM2(nil)
}

// VerifyIntegrityManifestSM2WithDistID uses an explicit SM2 distinguishing ID.
func (doc *Document) VerifyIntegrityManifestSM2WithDistID(distid string) (*IntegrityVerificationInfo, error) {
	if distid == "" {
		return nil, errors.New("empty SM2 distinguishing ID")
	}
	return doc.verifyIntegrityManifestSM2(&distid)
}

func (doc *Document) verifyIntegrityManifestSM2(distid *string) (*IntegrityVerificationInfo, error) {
	doc.mu.Lock()
	defer doc.mu.Unlock()
	if err := doc.requireOpen(); err != nil {
		return nil, err
	}
	var distidBytes []byte
	var distidPtr *byte
	if distid != nil {
		var err error
		distidBytes, err = cString(*distid)
		if err != nil {
			return nil, err
		}
		distidPtr = &distidBytes[0]
	}
	runtime.LockOSThread()
	defer runtime.UnlockOSThread()
	var buffer nativeBuffer
	if err := nativeError(api.verifyIntegrityManifestSM2(doc.handle, distidPtr, &buffer)); err != nil {
		return nil, fmt.Errorf("verify integrity manifest SM2: %w", err)
	}
	defer api.bufferFree(buffer)
	data, err := bytesAt(buffer)
	if err != nil {
		return nil, err
	}
	var report *IntegrityVerificationInfo
	if err := json.Unmarshal(data, &report); err != nil {
		return nil, fmt.Errorf("decode integrity signature report: %w", err)
	}
	return report, nil
}

// ProtectIntegritySM2 adds a signed root integrity manifest.
func (doc *Document) ProtectIntegritySM2(privateKey, certificateDER []byte) error {
	return doc.protectIntegritySM2(privateKey, certificateDER, nil)
}

// ProtectIntegritySM2WithDistID signs using an explicit SM2 distinguishing ID.
func (doc *Document) ProtectIntegritySM2WithDistID(privateKey, certificateDER []byte, distid string) error {
	return doc.protectIntegritySM2(privateKey, certificateDER, &distid)
}

func (doc *Document) protectIntegritySM2(privateKey, certificateDER []byte, distid *string) error {
	doc.mu.Lock()
	defer doc.mu.Unlock()
	if err := doc.requireOpen(); err != nil {
		return err
	}
	var keyPtr, certPtr, distidPtr *byte
	var distidBytes []byte
	if len(privateKey) > 0 {
		keyPtr = &privateKey[0]
	}
	if len(certificateDER) > 0 {
		certPtr = &certificateDER[0]
	}
	if distid != nil {
		var err error
		distidBytes, err = cString(*distid)
		if err != nil {
			return err
		}
		distidPtr = &distidBytes[0]
	}
	runtime.LockOSThread()
	defer runtime.UnlockOSThread()
	if err := nativeError(api.protectIntegritySM2(doc.handle, keyPtr, uintptr(len(privateKey)), certPtr, uintptr(len(certificateDER)), distidPtr)); err != nil {
		return fmt.Errorf("protect integrity: %w", err)
	}
	return nil
}

// VerifyIntegrityManifestSM2WithOrderedChain checks integrity and a caller-trusted
// ordered SM2 certificate chain. The supplied time is a caller claim.
func (doc *Document) VerifyIntegrityManifestSM2WithOrderedChain(
	signerCertificateDER []byte, intermediatesDER [][]byte, anchorCertificateDER []byte,
	atUnixSeconds uint64, certificateDistID, signatureDistID string,
) (*IntegrityTrustedVerificationInfo, error) {
	return doc.verifyIntegrityManifestSM2Chain(signerCertificateDER, intermediatesDER, anchorCertificateDER, atUnixSeconds, certificateDistID, signatureDistID, false)
}

// VerifyIntegrityManifestSM2WithCertificatePool builds an SM2 chain from an
// unordered pool of intermediate certificates before verifying integrity.
func (doc *Document) VerifyIntegrityManifestSM2WithCertificatePool(
	signerCertificateDER []byte, candidateCertificatesDER [][]byte, anchorCertificateDER []byte,
	atUnixSeconds uint64, certificateDistID, signatureDistID string,
) (*IntegrityTrustedVerificationInfo, error) {
	return doc.verifyIntegrityManifestSM2Chain(signerCertificateDER, candidateCertificatesDER, anchorCertificateDER, atUnixSeconds, certificateDistID, signatureDistID, true)
}

func (doc *Document) verifyIntegrityManifestSM2Chain(
	signerCertificateDER []byte, intermediatesDER [][]byte, anchorCertificateDER []byte,
	atUnixSeconds uint64, certificateDistID, signatureDistID string, unordered bool,
) (*IntegrityTrustedVerificationInfo, error) {
	doc.mu.Lock()
	defer doc.mu.Unlock()
	if err := doc.requireOpen(); err != nil {
		return nil, err
	}
	intermediates := make([]string, len(intermediatesDER))
	for index, certificate := range intermediatesDER {
		intermediates[index] = base64.StdEncoding.EncodeToString(certificate)
	}
	options, err := json.Marshal(map[string]any{
		"signer_certificate_der_base64": base64.StdEncoding.EncodeToString(signerCertificateDER),
		"intermediates_der_base64":      intermediates,
		"anchor_certificate_der_base64": base64.StdEncoding.EncodeToString(anchorCertificateDER),
		"at_unix_seconds":               atUnixSeconds,
		"certificate_distid":            certificateDistID,
		"signature_distid":              signatureDistID,
	})
	if err != nil {
		return nil, err
	}
	runtime.LockOSThread()
	defer runtime.UnlockOSThread()
	var buffer nativeBuffer
	verify := api.verifyIntegrityOrderedChain
	if unordered {
		verify = api.verifyIntegrityPool
	}
	if err := nativeError(verify(doc.handle, &append(options, 0)[0], &buffer)); err != nil {
		return nil, fmt.Errorf("verify integrity chain: %w", err)
	}
	defer api.bufferFree(buffer)
	data, err := bytesAt(buffer)
	if err != nil {
		return nil, err
	}
	var report *IntegrityTrustedVerificationInfo
	if err := json.Unmarshal(data, &report); err != nil {
		return nil, fmt.Errorf("decode integrity chain report: %w", err)
	}
	return report, nil
}

// Versions returns version descriptions from every document body.
func (doc *Document) Versions() ([]VersionInfo, error) {
	doc.mu.Lock()
	defer doc.mu.Unlock()
	if err := doc.requireOpen(); err != nil {
		return nil, err
	}
	runtime.LockOSThread()
	defer runtime.UnlockOSThread()
	var buffer nativeBuffer
	if err := nativeError(api.versions(doc.handle, &buffer)); err != nil {
		return nil, fmt.Errorf("read document versions: %w", err)
	}
	defer api.bufferFree(buffer)
	data, err := bytesAt(buffer)
	if err != nil {
		return nil, err
	}
	var versions []VersionInfo
	if err := json.Unmarshal(data, &versions); err != nil {
		return nil, fmt.Errorf("decode document versions: %w", err)
	}
	return versions, nil
}

// ExtractVersionSnapshot opens an archived version as an independent package.
func (doc *Document) ExtractVersionSnapshot(index int) (*Document, error) {
	doc.mu.Lock()
	defer doc.mu.Unlock()
	if err := doc.requireOpen(); err != nil {
		return nil, err
	}
	if index < 0 {
		return nil, errors.New("version index must be nonnegative")
	}
	runtime.LockOSThread()
	defer runtime.UnlockOSThread()
	var handle uintptr
	if err := nativeError(api.extractVersionSnapshot(doc.handle, uintptr(index), &handle)); err != nil {
		return nil, fmt.Errorf("extract version snapshot: %w", err)
	}
	extracted := &Document{handle: handle}
	runtime.SetFinalizer(extracted, (*Document).Close)
	return extracted, nil
}

// AddVersionSnapshot archives current package files under a new version ID.
func (doc *Document) AddVersionSnapshot(documentIndex int, id, name string) (VersionInfo, error) {
	doc.mu.Lock()
	defer doc.mu.Unlock()
	if err := doc.requireOpen(); err != nil {
		return VersionInfo{}, err
	}
	if documentIndex < 0 {
		return VersionInfo{}, errors.New("negative document index")
	}
	value := map[string]string{"id": id}
	if name != "" {
		value["name"] = name
	}
	payload, err := json.Marshal(value)
	if err != nil {
		return VersionInfo{}, fmt.Errorf("encode version snapshot: %w", err)
	}
	input, err := cString(string(payload))
	if err != nil {
		return VersionInfo{}, err
	}
	runtime.LockOSThread()
	defer runtime.UnlockOSThread()
	var buffer nativeBuffer
	if err := nativeError(api.addVersionSnapshot(doc.handle, uintptr(documentIndex), &input[0], &buffer)); err != nil {
		return VersionInfo{}, fmt.Errorf("add version snapshot: %w", err)
	}
	defer api.bufferFree(buffer)
	data, err := bytesAt(buffer)
	if err != nil {
		return VersionInfo{}, err
	}
	var version VersionInfo
	if err := json.Unmarshal(data, &version); err != nil {
		return VersionInfo{}, fmt.Errorf("decode version snapshot: %w", err)
	}
	return version, nil
}

// Bookmarks returns named destinations from all document bodies.
func (doc *Document) Bookmarks() ([]Bookmark, error) {
	doc.mu.Lock()
	defer doc.mu.Unlock()
	if err := doc.requireOpen(); err != nil {
		return nil, err
	}
	runtime.LockOSThread()
	defer runtime.UnlockOSThread()
	var buffer nativeBuffer
	if err := nativeError(api.bookmarks(doc.handle, &buffer)); err != nil {
		return nil, fmt.Errorf("list bookmarks: %w", err)
	}
	defer api.bufferFree(buffer)
	data, err := bytesAt(buffer)
	if err != nil {
		return nil, err
	}
	var items []Bookmark
	if err := json.Unmarshal(data, &items); err != nil {
		return nil, fmt.Errorf("decode bookmarks: %w", err)
	}
	return items, nil
}

// AddBookmark adds a named destination to an existing page.
func (doc *Document) AddBookmark(documentIndex int, name string, destination Destination) error {
	doc.mu.Lock()
	defer doc.mu.Unlock()
	if err := doc.requireOpen(); err != nil {
		return err
	}
	if documentIndex < 0 {
		return errors.New("negative document index")
	}
	if destination.Kind == "" {
		destination.Kind = "XYZ"
	}
	payload, err := json.Marshal(struct {
		Name string `json:"name"`
		Destination
	}{Name: name, Destination: destination})
	if err != nil {
		return fmt.Errorf("encode bookmark: %w", err)
	}
	bytes, err := cString(string(payload))
	if err != nil {
		return err
	}
	runtime.LockOSThread()
	defer runtime.UnlockOSThread()
	if err := nativeError(api.addBookmark(doc.handle, uintptr(documentIndex), &bytes[0])); err != nil {
		return fmt.Errorf("add bookmark: %w", err)
	}
	return nil
}

// RemoveBookmark removes a named destination from a document body.
func (doc *Document) RemoveBookmark(documentIndex int, name string) error {
	doc.mu.Lock()
	defer doc.mu.Unlock()
	if err := doc.requireOpen(); err != nil {
		return err
	}
	if documentIndex < 0 {
		return errors.New("negative document index")
	}
	value, err := cString(name)
	if err != nil {
		return err
	}
	runtime.LockOSThread()
	defer runtime.UnlockOSThread()
	if err := nativeError(api.removeBookmark(doc.handle, uintptr(documentIndex), &value[0])); err != nil {
		return fmt.Errorf("remove bookmark: %w", err)
	}
	return nil
}

// AddOutline adds a navigation node and returns its index in Outlines.
func (doc *Document) AddOutline(documentIndex int, title string, parentIndex *int, destination *Destination, expanded bool) (int, error) {
	doc.mu.Lock()
	defer doc.mu.Unlock()
	if err := doc.requireOpen(); err != nil {
		return 0, err
	}
	if documentIndex < 0 {
		return 0, errors.New("negative document index")
	}
	if parentIndex != nil && *parentIndex < 0 {
		return 0, errors.New("negative outline parent index")
	}
	if destination != nil && destination.Kind == "" {
		copy := *destination
		copy.Kind = "XYZ"
		destination = &copy
	}
	payload, err := json.Marshal(struct {
		Title       string       `json:"title"`
		ParentIndex *int         `json:"parent_index"`
		Destination *Destination `json:"destination"`
		Expanded    bool         `json:"expanded"`
	}{title, parentIndex, destination, expanded})
	if err != nil {
		return 0, fmt.Errorf("encode outline: %w", err)
	}
	bytes, err := cString(string(payload))
	if err != nil {
		return 0, err
	}
	runtime.LockOSThread()
	defer runtime.UnlockOSThread()
	var index uintptr
	if err := nativeError(api.addOutline(doc.handle, uintptr(documentIndex), &bytes[0], &index)); err != nil {
		return 0, fmt.Errorf("add outline: %w", err)
	}
	return int(index), nil
}

// RemoveOutline removes a navigation node and all descendants by its current index.
func (doc *Document) RemoveOutline(index int) error {
	doc.mu.Lock()
	defer doc.mu.Unlock()
	if err := doc.requireOpen(); err != nil {
		return err
	}
	if index < 0 {
		return errors.New("negative outline index")
	}
	runtime.LockOSThread()
	defer runtime.UnlockOSThread()
	if err := nativeError(api.removeOutline(doc.handle, uintptr(index))); err != nil {
		return fmt.Errorf("remove outline: %w", err)
	}
	return nil
}

// SetOutlineTitle changes a navigation node's title without changing its children or actions.
func (doc *Document) SetOutlineTitle(index int, title string) error {
	doc.mu.Lock()
	defer doc.mu.Unlock()
	if err := doc.requireOpen(); err != nil {
		return err
	}
	if index < 0 {
		return errors.New("negative outline index")
	}
	encoded, err := cString(title)
	if err != nil {
		return err
	}
	runtime.LockOSThread()
	defer runtime.UnlockOSThread()
	if err := nativeError(api.setOutlineTitle(doc.handle, uintptr(index), &encoded[0])); err != nil {
		return fmt.Errorf("set outline title: %w", err)
	}
	return nil
}

// SetOutlineExpanded changes a navigation node's initial expanded state.
func (doc *Document) SetOutlineExpanded(index int, expanded bool) error {
	doc.mu.Lock()
	defer doc.mu.Unlock()
	if err := doc.requireOpen(); err != nil {
		return err
	}
	if index < 0 {
		return errors.New("negative outline index")
	}
	var value int32
	if expanded {
		value = 1
	}
	runtime.LockOSThread()
	defer runtime.UnlockOSThread()
	if err := nativeError(api.setOutlineExpanded(doc.handle, uintptr(index), value)); err != nil {
		return fmt.Errorf("set outline expanded: %w", err)
	}
	return nil
}

// SetOutlineDestination retargets an outline's direct GoTo action.
func (doc *Document) SetOutlineDestination(index int, destination Destination) error {
	doc.mu.Lock()
	defer doc.mu.Unlock()
	if err := doc.requireOpen(); err != nil {
		return err
	}
	if index < 0 {
		return errors.New("negative outline index")
	}
	if destination.Kind == "" {
		destination.Kind = "XYZ"
	}
	payload, err := json.Marshal(destination)
	if err != nil {
		return fmt.Errorf("encode outline destination: %w", err)
	}
	encoded, err := cString(string(payload))
	if err != nil {
		return err
	}
	runtime.LockOSThread()
	defer runtime.UnlockOSThread()
	if err := nativeError(api.setOutlineDestination(doc.handle, uintptr(index), &encoded[0])); err != nil {
		return fmt.Errorf("set outline destination: %w", err)
	}
	return nil
}

// SetOutlineBookmark points an outline's direct GoTo action at a named bookmark.
func (doc *Document) SetOutlineBookmark(index int, name string) error {
	doc.mu.Lock()
	defer doc.mu.Unlock()
	if err := doc.requireOpen(); err != nil {
		return err
	}
	if index < 0 {
		return errors.New("negative outline index")
	}
	encoded, err := cString(name)
	if err != nil {
		return err
	}
	runtime.LockOSThread()
	defer runtime.UnlockOSThread()
	if err := nativeError(api.setOutlineBookmark(doc.handle, uintptr(index), &encoded[0])); err != nil {
		return fmt.Errorf("set outline bookmark: %w", err)
	}
	return nil
}

// Outlines returns navigation nodes in depth-first order.
func (doc *Document) Outlines() ([]Outline, error) {
	doc.mu.Lock()
	defer doc.mu.Unlock()
	if err := doc.requireOpen(); err != nil {
		return nil, err
	}
	runtime.LockOSThread()
	defer runtime.UnlockOSThread()
	var buffer nativeBuffer
	if err := nativeError(api.outlines(doc.handle, &buffer)); err != nil {
		return nil, fmt.Errorf("list outlines: %w", err)
	}
	defer api.bufferFree(buffer)
	data, err := bytesAt(buffer)
	if err != nil {
		return nil, err
	}
	var items []Outline
	if err := json.Unmarshal(data, &items); err != nil {
		return nil, fmt.Errorf("decode outlines: %w", err)
	}
	return items, nil
}

// SetDocumentInfoField updates one standard DocInfo field. DocID requires a
// unique 32-digit hexadecimal UUID and cannot be removed.
func (doc *Document) SetDocumentInfoField(documentIndex int, field, value string) error {
	doc.mu.Lock()
	defer doc.mu.Unlock()
	if err := doc.requireOpen(); err != nil {
		return err
	}
	if documentIndex < 0 {
		return errors.New("negative document index")
	}
	fieldBytes, err := cString(field)
	if err != nil {
		return err
	}
	valueBytes, err := cString(value)
	if err != nil {
		return err
	}
	runtime.LockOSThread()
	defer runtime.UnlockOSThread()
	if err := nativeError(api.setInfoField(doc.handle, uintptr(documentIndex), &fieldBytes[0], &valueBytes[0])); err != nil {
		return fmt.Errorf("set document metadata: %w", err)
	}
	return nil
}

// RemoveDocumentInfoField removes an optional standard plain-text DocInfo field.
func (doc *Document) RemoveDocumentInfoField(documentIndex int, field string) error {
	doc.mu.Lock()
	defer doc.mu.Unlock()
	if err := doc.requireOpen(); err != nil {
		return err
	}
	if documentIndex < 0 {
		return errors.New("negative document index")
	}
	fieldBytes, err := cString(field)
	if err != nil {
		return err
	}
	runtime.LockOSThread()
	defer runtime.UnlockOSThread()
	if err := nativeError(api.removeInfoField(doc.handle, uintptr(documentIndex), &fieldBytes[0])); err != nil {
		return fmt.Errorf("remove document metadata: %w", err)
	}
	return nil
}

// AddDocumentKeyword appends a keyword to a document's DocInfo.
func (doc *Document) AddDocumentKeyword(documentIndex int, keyword string) error {
	doc.mu.Lock()
	defer doc.mu.Unlock()
	if err := doc.requireOpen(); err != nil {
		return err
	}
	if documentIndex < 0 {
		return errors.New("negative document index")
	}
	keywordBytes, err := cString(keyword)
	if err != nil {
		return err
	}
	runtime.LockOSThread()
	defer runtime.UnlockOSThread()
	if err := nativeError(api.addKeyword(doc.handle, uintptr(documentIndex), &keywordBytes[0])); err != nil {
		return fmt.Errorf("add document keyword: %w", err)
	}
	return nil
}

// RemoveDocumentKeyword removes a keyword by its zero-based index.
func (doc *Document) RemoveDocumentKeyword(documentIndex, keywordIndex int) error {
	doc.mu.Lock()
	defer doc.mu.Unlock()
	if err := doc.requireOpen(); err != nil {
		return err
	}
	if documentIndex < 0 {
		return errors.New("negative document index")
	}
	if keywordIndex < 0 {
		return errors.New("negative keyword index")
	}
	runtime.LockOSThread()
	defer runtime.UnlockOSThread()
	if err := nativeError(api.removeKeyword(doc.handle, uintptr(documentIndex), uintptr(keywordIndex))); err != nil {
		return fmt.Errorf("remove document keyword: %w", err)
	}
	return nil
}

// SetDocumentCustomData creates or updates one named custom DocInfo value.
func (doc *Document) SetDocumentCustomData(documentIndex int, name, value string) error {
	doc.mu.Lock()
	defer doc.mu.Unlock()
	if err := doc.requireOpen(); err != nil {
		return err
	}
	if documentIndex < 0 {
		return errors.New("negative document index")
	}
	nameBytes, err := cString(name)
	if err != nil {
		return err
	}
	valueBytes, err := cString(value)
	if err != nil {
		return err
	}
	runtime.LockOSThread()
	defer runtime.UnlockOSThread()
	if err := nativeError(api.setCustomData(doc.handle, uintptr(documentIndex), &nameBytes[0], &valueBytes[0])); err != nil {
		return fmt.Errorf("set custom document metadata: %w", err)
	}
	return nil
}

// RemoveDocumentCustomData removes one named custom DocInfo value.
func (doc *Document) RemoveDocumentCustomData(documentIndex int, name string) error {
	doc.mu.Lock()
	defer doc.mu.Unlock()
	if err := doc.requireOpen(); err != nil {
		return err
	}
	if documentIndex < 0 {
		return errors.New("negative document index")
	}
	nameBytes, err := cString(name)
	if err != nil {
		return err
	}
	runtime.LockOSThread()
	defer runtime.UnlockOSThread()
	if err := nativeError(api.removeCustomData(doc.handle, uintptr(documentIndex), &nameBytes[0])); err != nil {
		return fmt.Errorf("remove custom document metadata: %w", err)
	}
	return nil
}

// PageTextObjects returns positioned text objects from a zero-based page.
func (doc *Document) PageTextObjects(index int) ([]TextObject, error) {
	doc.mu.Lock()
	defer doc.mu.Unlock()
	if err := doc.requireOpen(); err != nil {
		return nil, err
	}
	if index < 0 {
		return nil, errors.New("negative page index")
	}
	runtime.LockOSThread()
	defer runtime.UnlockOSThread()
	var buffer nativeBuffer
	if err := nativeError(api.textObjects(doc.handle, uintptr(index), &buffer)); err != nil {
		return nil, fmt.Errorf("read page text objects: %w", err)
	}
	defer api.bufferFree(buffer)
	data, err := bytesAt(buffer)
	if err != nil {
		return nil, err
	}
	var objects []TextObject
	if err := json.Unmarshal(data, &objects); err != nil {
		return nil, fmt.Errorf("decode page text objects: %w", err)
	}
	return objects, nil
}

// PagePaths returns path objects from a zero-based page.
func (doc *Document) PagePaths(index int) ([]PathObject, error) {
	doc.mu.Lock()
	defer doc.mu.Unlock()
	if err := doc.requireOpen(); err != nil {
		return nil, err
	}
	if index < 0 {
		return nil, errors.New("negative page index")
	}
	runtime.LockOSThread()
	defer runtime.UnlockOSThread()
	var buffer nativeBuffer
	if err := nativeError(api.pagePaths(doc.handle, uintptr(index), &buffer)); err != nil {
		return nil, fmt.Errorf("read page paths: %w", err)
	}
	defer api.bufferFree(buffer)
	data, err := bytesAt(buffer)
	if err != nil {
		return nil, err
	}
	var paths []PathObject
	if err := json.Unmarshal(data, &paths); err != nil {
		return nil, fmt.Errorf("decode page paths: %w", err)
	}
	return paths, nil
}

// PageImages returns image objects and resolved resource paths.
func (doc *Document) PageImages(index int) ([]ImageObject, error) {
	doc.mu.Lock()
	defer doc.mu.Unlock()
	if err := doc.requireOpen(); err != nil {
		return nil, err
	}
	if index < 0 {
		return nil, errors.New("negative page index")
	}
	runtime.LockOSThread()
	defer runtime.UnlockOSThread()
	var buffer nativeBuffer
	if err := nativeError(api.pageImages(doc.handle, uintptr(index), &buffer)); err != nil {
		return nil, fmt.Errorf("read page images: %w", err)
	}
	defer api.bufferFree(buffer)
	data, err := bytesAt(buffer)
	if err != nil {
		return nil, err
	}
	var images []ImageObject
	if err := json.Unmarshal(data, &images); err != nil {
		return nil, fmt.Errorf("decode page images: %w", err)
	}
	return images, nil
}

// PageMultimediaResources lists multimedia resources visible to a page.
func (doc *Document) PageMultimediaResources(index int) ([]MultimediaResource, error) {
	doc.mu.Lock()
	defer doc.mu.Unlock()
	if err := doc.requireOpen(); err != nil {
		return nil, err
	}
	if index < 0 {
		return nil, errors.New("negative page index")
	}
	runtime.LockOSThread()
	defer runtime.UnlockOSThread()
	var buffer nativeBuffer
	if err := nativeError(api.pageMultimediaResources(doc.handle, uintptr(index), &buffer)); err != nil {
		return nil, fmt.Errorf("list page multimedia resources: %w", err)
	}
	defer api.bufferFree(buffer)
	data, err := bytesAt(buffer)
	if err != nil {
		return nil, err
	}
	var resources []MultimediaResource
	if err := json.Unmarshal(data, &resources); err != nil {
		return nil, fmt.Errorf("decode page multimedia resources: %w", err)
	}
	return resources, nil
}

// PageMultimediaData returns raw bytes of a resource visible to a page.
func (doc *Document) PageMultimediaData(index int, resourceID string) ([]byte, error) {
	doc.mu.Lock()
	defer doc.mu.Unlock()
	if err := doc.requireOpen(); err != nil {
		return nil, err
	}
	if index < 0 {
		return nil, errors.New("negative page index")
	}
	id, err := cString(resourceID)
	if err != nil {
		return nil, err
	}
	runtime.LockOSThread()
	defer runtime.UnlockOSThread()
	var buffer nativeBuffer
	if err := nativeError(api.pageMultimediaData(doc.handle, uintptr(index), &id[0], &buffer)); err != nil {
		return nil, fmt.Errorf("read page multimedia resource: %w", err)
	}
	defer api.bufferFree(buffer)
	return bytesAt(buffer)
}

// DocumentMultimediaResources lists resources declared by a document body.
func (doc *Document) DocumentMultimediaResources(index int) ([]MultimediaResource, error) {
	doc.mu.Lock()
	defer doc.mu.Unlock()
	if err := doc.requireOpen(); err != nil {
		return nil, err
	}
	if index < 0 {
		return nil, errors.New("negative document index")
	}
	runtime.LockOSThread()
	defer runtime.UnlockOSThread()
	var buffer nativeBuffer
	if err := nativeError(api.documentMultimediaResources(doc.handle, uintptr(index), &buffer)); err != nil {
		return nil, fmt.Errorf("list document multimedia resources: %w", err)
	}
	defer api.bufferFree(buffer)
	data, err := bytesAt(buffer)
	if err != nil {
		return nil, err
	}
	var resources []MultimediaResource
	if err := json.Unmarshal(data, &resources); err != nil {
		return nil, fmt.Errorf("decode document multimedia resources: %w", err)
	}
	return resources, nil
}

// DocumentMultimediaData returns raw bytes of a document-level resource.
func (doc *Document) DocumentMultimediaData(index int, resourceID string) ([]byte, error) {
	doc.mu.Lock()
	defer doc.mu.Unlock()
	if err := doc.requireOpen(); err != nil {
		return nil, err
	}
	if index < 0 {
		return nil, errors.New("negative document index")
	}
	id, err := cString(resourceID)
	if err != nil {
		return nil, err
	}
	runtime.LockOSThread()
	defer runtime.UnlockOSThread()
	var buffer nativeBuffer
	if err := nativeError(api.documentMultimediaData(doc.handle, uintptr(index), &id[0], &buffer)); err != nil {
		return nil, fmt.Errorf("read document multimedia resource: %w", err)
	}
	defer api.bufferFree(buffer)
	return bytesAt(buffer)
}

// PageCompositeImages returns images inside composite objects in page placement order.
func (doc *Document) PageCompositeImages(index int) ([]ImageObject, error) {
	doc.mu.Lock()
	defer doc.mu.Unlock()
	if err := doc.requireOpen(); err != nil {
		return nil, err
	}
	if index < 0 {
		return nil, errors.New("negative page index")
	}
	runtime.LockOSThread()
	defer runtime.UnlockOSThread()
	var buffer nativeBuffer
	if err := nativeError(api.compositeImages(doc.handle, uintptr(index), &buffer)); err != nil {
		return nil, fmt.Errorf("read page composite images: %w", err)
	}
	defer api.bufferFree(buffer)
	data, err := bytesAt(buffer)
	if err != nil {
		return nil, err
	}
	var images []ImageObject
	if err := json.Unmarshal(data, &images); err != nil {
		return nil, fmt.Errorf("decode page composite images: %w", err)
	}
	return images, nil
}

// PageCompositeObjects returns composite graphic objects on a page.
func (doc *Document) PageCompositeObjects(index int) ([]CompositeObject, error) {
	doc.mu.Lock()
	defer doc.mu.Unlock()
	if err := doc.requireOpen(); err != nil {
		return nil, err
	}
	if index < 0 {
		return nil, errors.New("negative page index")
	}
	runtime.LockOSThread()
	defer runtime.UnlockOSThread()
	var buffer nativeBuffer
	if err := nativeError(api.compositeObjects(doc.handle, uintptr(index), &buffer)); err != nil {
		return nil, fmt.Errorf("read page composite objects: %w", err)
	}
	defer api.bufferFree(buffer)
	data, err := bytesAt(buffer)
	if err != nil {
		return nil, err
	}
	var objects []CompositeObject
	if err := json.Unmarshal(data, &objects); err != nil {
		return nil, fmt.Errorf("decode page composite objects: %w", err)
	}
	return objects, nil
}

// PageCompositeUnits returns graphic unit metadata available to a page.
func (doc *Document) PageCompositeUnits(index int) ([]CompositeUnit, error) {
	doc.mu.Lock()
	defer doc.mu.Unlock()
	if err := doc.requireOpen(); err != nil {
		return nil, err
	}
	if index < 0 {
		return nil, errors.New("negative page index")
	}
	runtime.LockOSThread()
	defer runtime.UnlockOSThread()
	var buffer nativeBuffer
	if err := nativeError(api.compositeUnits(doc.handle, uintptr(index), &buffer)); err != nil {
		return nil, fmt.Errorf("read page composite units: %w", err)
	}
	defer api.bufferFree(buffer)
	data, err := bytesAt(buffer)
	if err != nil {
		return nil, err
	}
	var units []CompositeUnit
	if err := json.Unmarshal(data, &units); err != nil {
		return nil, fmt.Errorf("decode page composite units: %w", err)
	}
	return units, nil
}

// SetPageCompositeUnitImages sets or clears a unit's thumbnail and substitution.
func (doc *Document) SetPageCompositeUnitImages(pageIndex int, unitID string, thumbnailID, substitutionID *string) error {
	doc.mu.Lock()
	defer doc.mu.Unlock()
	if err := doc.requireOpen(); err != nil {
		return err
	}
	if pageIndex < 0 {
		return errors.New("negative page index")
	}
	id, err := cString(unitID)
	if err != nil {
		return err
	}
	var thumbnail, substitution []byte
	var thumbnailPtr, substitutionPtr *byte
	if thumbnailID != nil {
		thumbnail, err = cString(*thumbnailID)
		if err != nil {
			return err
		}
		thumbnailPtr = &thumbnail[0]
	}
	if substitutionID != nil {
		substitution, err = cString(*substitutionID)
		if err != nil {
			return err
		}
		substitutionPtr = &substitution[0]
	}
	runtime.LockOSThread()
	defer runtime.UnlockOSThread()
	if err := nativeError(api.setPageCompositeUnitImages(doc.handle, uintptr(pageIndex), &id[0], thumbnailPtr, substitutionPtr)); err != nil {
		return fmt.Errorf("set page composite unit images: %w", err)
	}
	return nil
}

// PageImageData returns the original bytes of an image object.
func (doc *Document) PageImageData(pageIndex, imageIndex int) ([]byte, error) {
	doc.mu.Lock()
	defer doc.mu.Unlock()
	if err := doc.requireOpen(); err != nil {
		return nil, err
	}
	if pageIndex < 0 || imageIndex < 0 {
		return nil, errors.New("negative image index")
	}
	runtime.LockOSThread()
	defer runtime.UnlockOSThread()
	var buffer nativeBuffer
	if err := nativeError(api.imageData(doc.handle, uintptr(pageIndex), uintptr(imageIndex), &buffer)); err != nil {
		return nil, fmt.Errorf("read image data: %w", err)
	}
	defer api.bufferFree(buffer)
	return bytesAt(buffer)
}

// PageCompositeImageData returns original bytes of an image inside a composite object.
func (doc *Document) PageCompositeImageData(pageIndex, imageIndex int) ([]byte, error) {
	doc.mu.Lock()
	defer doc.mu.Unlock()
	if err := doc.requireOpen(); err != nil {
		return nil, err
	}
	if pageIndex < 0 || imageIndex < 0 {
		return nil, errors.New("negative composite image index")
	}
	runtime.LockOSThread()
	defer runtime.UnlockOSThread()
	var buffer nativeBuffer
	if err := nativeError(api.compositeImageData(doc.handle, uintptr(pageIndex), uintptr(imageIndex), &buffer)); err != nil {
		return nil, fmt.Errorf("read composite image data: %w", err)
	}
	defer api.bufferFree(buffer)
	return bytesAt(buffer)
}

// PageSVG returns an SVG preview of a page and its templates.
func (doc *Document) PageSVG(index int) (string, error) {
	return doc.PageSVGWithOptions(index, RenderOptions{})
}

// PageSVGWithOptions returns SVG with explicit image resource selection.
func (doc *Document) PageSVGWithOptions(index int, options RenderOptions) (string, error) {
	doc.mu.Lock()
	defer doc.mu.Unlock()
	if err := doc.requireOpen(); err != nil {
		return "", err
	}
	if index < 0 {
		return "", errors.New("negative page index")
	}
	var prefer int32
	if options.PreferImageSubstitution {
		prefer = 1
	}
	runtime.LockOSThread()
	defer runtime.UnlockOSThread()
	var buffer nativeBuffer
	if err := nativeError(api.pageSVGWithOptions(doc.handle, uintptr(index), prefer, &buffer)); err != nil {
		return "", fmt.Errorf("render page svg: %w", err)
	}
	defer api.bufferFree(buffer)
	data, err := bytesAt(buffer)
	if err != nil {
		return "", err
	}
	return string(data), nil
}

// PagePNG renders a page as PNG bytes at the requested DPI.
func (doc *Document) PagePNG(index int, dpi float64) ([]byte, error) {
	return doc.PagePNGWithOptions(index, dpi, RenderOptions{})
}

// PagePNGWithOptions renders PNG with explicit image resource selection.
func (doc *Document) PagePNGWithOptions(index int, dpi float64, options RenderOptions) ([]byte, error) {
	doc.mu.Lock()
	defer doc.mu.Unlock()
	if err := doc.requireOpen(); err != nil {
		return nil, err
	}
	if index < 0 {
		return nil, errors.New("negative page index")
	}
	var prefer int32
	if options.PreferImageSubstitution {
		prefer = 1
	}
	runtime.LockOSThread()
	defer runtime.UnlockOSThread()
	var buffer nativeBuffer
	if err := nativeError(api.pagePNGWithOptions(doc.handle, uintptr(index), dpi, prefer, &buffer)); err != nil {
		return nil, fmt.Errorf("render page png: %w", err)
	}
	defer api.bufferFree(buffer)
	return bytesAt(buffer)
}

// ToPDF exports all pages in document order to a vector PDF.
func (doc *Document) ToPDF() ([]byte, error) {
	return doc.ToPDFWithOptions(RenderOptions{})
}

// ToPDFWithOptions exports PDF with explicit image resource selection.
func (doc *Document) ToPDFWithOptions(options RenderOptions) ([]byte, error) {
	doc.mu.Lock()
	defer doc.mu.Unlock()
	if err := doc.requireOpen(); err != nil {
		return nil, err
	}
	var prefer int32
	if options.PreferImageSubstitution {
		prefer = 1
	}
	runtime.LockOSThread()
	defer runtime.UnlockOSThread()
	var buffer nativeBuffer
	if err := nativeError(api.toPDFWithOptions(doc.handle, prefer, &buffer)); err != nil {
		return nil, fmt.Errorf("export PDF: %w", err)
	}
	defer api.bufferFree(buffer)
	return bytesAt(buffer)
}

// Entry retrieves a raw package entry.
func (doc *Document) Entry(path string) ([]byte, error) {
	doc.mu.Lock()
	defer doc.mu.Unlock()
	if err := doc.requireOpen(); err != nil {
		return nil, err
	}
	pathBytes, err := cString(path)
	if err != nil {
		return nil, err
	}
	runtime.LockOSThread()
	defer runtime.UnlockOSThread()
	var buffer nativeBuffer
	if err := nativeError(api.getEntry(doc.handle, &pathBytes[0], &buffer)); err != nil {
		return nil, fmt.Errorf("read entry: %w", err)
	}
	defer api.bufferFree(buffer)
	return bytesAt(buffer)
}

// Attachments returns attachment metadata in document order.
func (doc *Document) Attachments() ([]Attachment, error) {
	doc.mu.Lock()
	defer doc.mu.Unlock()
	if err := doc.requireOpen(); err != nil {
		return nil, err
	}
	runtime.LockOSThread()
	defer runtime.UnlockOSThread()
	var buffer nativeBuffer
	if err := nativeError(api.attachments(doc.handle, &buffer)); err != nil {
		return nil, fmt.Errorf("list attachments: %w", err)
	}
	defer api.bufferFree(buffer)
	data, err := bytesAt(buffer)
	if err != nil {
		return nil, err
	}
	var items []Attachment
	if err := json.Unmarshal(data, &items); err != nil {
		return nil, fmt.Errorf("decode attachments: %w", err)
	}
	return items, nil
}

// CustomTags returns custom tag index entries from all document bodies.
func (doc *Document) CustomTags() ([]CustomTag, error) {
	doc.mu.Lock()
	defer doc.mu.Unlock()
	if err := doc.requireOpen(); err != nil {
		return nil, err
	}
	runtime.LockOSThread()
	defer runtime.UnlockOSThread()
	var buffer nativeBuffer
	if err := nativeError(api.customTags(doc.handle, &buffer)); err != nil {
		return nil, fmt.Errorf("list custom tags: %w", err)
	}
	defer api.bufferFree(buffer)
	data, err := bytesAt(buffer)
	if err != nil {
		return nil, err
	}
	var tags []CustomTag
	if err := json.Unmarshal(data, &tags); err != nil {
		return nil, fmt.Errorf("decode custom tags: %w", err)
	}
	return tags, nil
}

// Extensions returns extension entries from all document bodies.
func (doc *Document) Extensions() ([]Extension, error) {
	doc.mu.Lock()
	defer doc.mu.Unlock()
	if err := doc.requireOpen(); err != nil {
		return nil, err
	}
	runtime.LockOSThread()
	defer runtime.UnlockOSThread()
	var buffer nativeBuffer
	if err := nativeError(api.extensions(doc.handle, &buffer)); err != nil {
		return nil, fmt.Errorf("list extensions: %w", err)
	}
	defer api.bufferFree(buffer)
	data, err := bytesAt(buffer)
	if err != nil {
		return nil, err
	}
	var extensions []Extension
	if err := json.Unmarshal(data, &extensions); err != nil {
		return nil, fmt.Errorf("decode extensions: %w", err)
	}
	return extensions, nil
}

// AddExtension appends a document extension and its referenced metadata.
func (doc *Document) AddExtension(extension Extension) error {
	doc.mu.Lock()
	defer doc.mu.Unlock()
	if err := doc.requireOpen(); err != nil {
		return err
	}
	if extension.DocumentIndex < 0 {
		return errors.New("negative document index")
	}
	if extension.Properties == nil {
		extension.Properties = []ExtensionProperty{}
	}
	if extension.ExtendDataPaths == nil {
		extension.ExtendDataPaths = []string{}
	}
	if extension.DataXML == nil {
		extension.DataXML = []string{}
	}
	data, err := json.Marshal(extension)
	if err != nil {
		return err
	}
	bytes, err := cString(string(data))
	if err != nil {
		return err
	}
	runtime.LockOSThread()
	defer runtime.UnlockOSThread()
	if err := nativeError(api.addExtension(doc.handle, &bytes[0])); err != nil {
		return fmt.Errorf("add extension: %w", err)
	}
	return nil
}

// UpdateExtension replaces a global extension record without changing its document body.
func (doc *Document) UpdateExtension(index int, extension Extension) error {
	doc.mu.Lock()
	defer doc.mu.Unlock()
	if err := doc.requireOpen(); err != nil {
		return err
	}
	if index < 0 {
		return errors.New("negative extension index")
	}
	if extension.DocumentIndex < 0 {
		return errors.New("negative document index")
	}
	if extension.Properties == nil {
		extension.Properties = []ExtensionProperty{}
	}
	if extension.ExtendDataPaths == nil {
		extension.ExtendDataPaths = []string{}
	}
	if extension.DataXML == nil {
		extension.DataXML = []string{}
	}
	data, err := json.Marshal(extension)
	if err != nil {
		return err
	}
	bytes, err := cString(string(data))
	if err != nil {
		return err
	}
	runtime.LockOSThread()
	defer runtime.UnlockOSThread()
	if err := nativeError(api.updateExtension(doc.handle, uintptr(index), &bytes[0])); err != nil {
		return fmt.Errorf("update extension: %w", err)
	}
	return nil
}

// RemoveExtension removes a global extension record and retains its referenced files.
func (doc *Document) RemoveExtension(index int) error {
	doc.mu.Lock()
	defer doc.mu.Unlock()
	if err := doc.requireOpen(); err != nil {
		return err
	}
	if index < 0 {
		return errors.New("negative extension index")
	}
	runtime.LockOSThread()
	defer runtime.UnlockOSThread()
	return nativeError(api.removeExtension(doc.handle, uintptr(index)))
}

// CustomTagData returns the original XML bytes of one custom tag entry.
func (doc *Document) CustomTagData(index int) ([]byte, error) {
	doc.mu.Lock()
	defer doc.mu.Unlock()
	if err := doc.requireOpen(); err != nil {
		return nil, err
	}
	if index < 0 {
		return nil, errors.New("negative custom tag index")
	}
	runtime.LockOSThread()
	defer runtime.UnlockOSThread()
	var buffer nativeBuffer
	if err := nativeError(api.customTagData(doc.handle, uintptr(index), &buffer)); err != nil {
		return nil, fmt.Errorf("read custom tag data: %w", err)
	}
	defer api.bufferFree(buffer)
	return bytesAt(buffer)
}

// ReplaceCustomTagData replaces one tag's XML payload and retains its former entry.
func (doc *Document) ReplaceCustomTagData(index int, data []byte) (CustomTag, error) {
	doc.mu.Lock()
	defer doc.mu.Unlock()
	var tag CustomTag
	if err := doc.requireOpen(); err != nil {
		return tag, err
	}
	if index < 0 {
		return tag, errors.New("negative custom tag index")
	}
	var dataPtr *byte
	if len(data) > 0 {
		dataPtr = &data[0]
	}
	runtime.LockOSThread()
	defer runtime.UnlockOSThread()
	var buffer nativeBuffer
	if err := nativeError(api.replaceCustomTagData(doc.handle, uintptr(index), dataPtr, uintptr(len(data)), &buffer)); err != nil {
		return tag, fmt.Errorf("replace custom tag data: %w", err)
	}
	runtime.KeepAlive(data)
	defer api.bufferFree(buffer)
	value, err := bytesAt(buffer)
	if err != nil {
		return tag, err
	}
	if err := json.Unmarshal(value, &tag); err != nil {
		return tag, fmt.Errorf("decode custom tag: %w", err)
	}
	return tag, nil
}

// RemoveCustomTag removes a global custom tag record and retains its payload.
func (doc *Document) RemoveCustomTag(index int) error {
	doc.mu.Lock()
	defer doc.mu.Unlock()
	if err := doc.requireOpen(); err != nil {
		return err
	}
	if index < 0 {
		return errors.New("negative custom tag index")
	}
	runtime.LockOSThread()
	defer runtime.UnlockOSThread()
	return nativeError(api.removeCustomTag(doc.handle, uintptr(index)))
}

// AddCustomTag adds an XML payload and its document index entry atomically.
func (doc *Document) AddCustomTag(documentIndex int, typeID string, data []byte, options CustomTagOptions) (CustomTag, error) {
	doc.mu.Lock()
	defer doc.mu.Unlock()
	var tag CustomTag
	if err := doc.requireOpen(); err != nil {
		return tag, err
	}
	if documentIndex < 0 {
		return tag, errors.New("negative document index")
	}
	metadata, err := json.Marshal(struct {
		TypeID string `json:"type_id"`
		CustomTagOptions
	}{TypeID: typeID, CustomTagOptions: options})
	if err != nil {
		return tag, err
	}
	metadataBytes, err := cString(string(metadata))
	if err != nil {
		return tag, err
	}
	var dataPtr *byte
	if len(data) > 0 {
		dataPtr = &data[0]
	}
	runtime.LockOSThread()
	defer runtime.UnlockOSThread()
	var buffer nativeBuffer
	if err := nativeError(api.addCustomTag(doc.handle, uintptr(documentIndex), &metadataBytes[0], dataPtr, uintptr(len(data)), &buffer)); err != nil {
		return tag, fmt.Errorf("add custom tag: %w", err)
	}
	runtime.KeepAlive(data)
	defer api.bufferFree(buffer)
	value, err := bytesAt(buffer)
	if err != nil {
		return tag, err
	}
	if err := json.Unmarshal(value, &tag); err != nil {
		return tag, fmt.Errorf("decode custom tag: %w", err)
	}
	return tag, nil
}

// UpdateCustomTag changes metadata while retaining the XML payload and its path.
func (doc *Document) UpdateCustomTag(index int, typeID string, options CustomTagOptions) (CustomTag, error) {
	doc.mu.Lock()
	defer doc.mu.Unlock()
	var tag CustomTag
	if err := doc.requireOpen(); err != nil {
		return tag, err
	}
	if index < 0 {
		return tag, errors.New("negative custom tag index")
	}
	metadata, err := json.Marshal(struct {
		TypeID string `json:"type_id"`
		CustomTagOptions
	}{TypeID: typeID, CustomTagOptions: options})
	if err != nil {
		return tag, err
	}
	metadataBytes, err := cString(string(metadata))
	if err != nil {
		return tag, err
	}
	runtime.LockOSThread()
	defer runtime.UnlockOSThread()
	var buffer nativeBuffer
	if err := nativeError(api.updateCustomTag(doc.handle, uintptr(index), &metadataBytes[0], &buffer)); err != nil {
		return tag, fmt.Errorf("update custom tag: %w", err)
	}
	defer api.bufferFree(buffer)
	value, err := bytesAt(buffer)
	if err != nil {
		return tag, err
	}
	if err := json.Unmarshal(value, &tag); err != nil {
		return tag, fmt.Errorf("decode custom tag: %w", err)
	}
	return tag, nil
}

// Annotations returns annotation metadata from all document bodies.
func (doc *Document) Annotations() ([]Annotation, error) {
	doc.mu.Lock()
	defer doc.mu.Unlock()
	if err := doc.requireOpen(); err != nil {
		return nil, err
	}
	runtime.LockOSThread()
	defer runtime.UnlockOSThread()
	var buffer nativeBuffer
	if err := nativeError(api.annotations(doc.handle, &buffer)); err != nil {
		return nil, fmt.Errorf("list annotations: %w", err)
	}
	defer api.bufferFree(buffer)
	data, err := bytesAt(buffer)
	if err != nil {
		return nil, err
	}
	var items []Annotation
	if err := json.Unmarshal(data, &items); err != nil {
		return nil, fmt.Errorf("decode annotations: %w", err)
	}
	return items, nil
}

// AnnotationObjects returns direct appearance objects in painting order.
func (doc *Document) AnnotationObjects(pageIndex int, annotationID string) ([]AnnotationObject, error) {
	doc.mu.Lock()
	defer doc.mu.Unlock()
	if err := doc.requireOpen(); err != nil {
		return nil, err
	}
	if pageIndex < 0 {
		return nil, errors.New("negative page index")
	}
	id, err := cString(annotationID)
	if err != nil {
		return nil, err
	}
	runtime.LockOSThread()
	defer runtime.UnlockOSThread()
	var buffer nativeBuffer
	if err := nativeError(api.annotationObjects(doc.handle, uintptr(pageIndex), &id[0], &buffer)); err != nil {
		return nil, fmt.Errorf("list annotation objects: %w", err)
	}
	defer api.bufferFree(buffer)
	data, err := bytesAt(buffer)
	if err != nil {
		return nil, err
	}
	var objects []AnnotationObject
	if err := json.Unmarshal(data, &objects); err != nil {
		return nil, fmt.Errorf("decode annotation objects: %w", err)
	}
	return objects, nil
}

// AppendAnnotationObjects adds ordered path, text, image, or composite objects to an existing appearance.
// It returns the allocated OFD object IDs in painting order.
func (doc *Document) AppendAnnotationObjects(pageIndex int, annotationID string, objects []AnnotationGraphic) ([]string, error) {
	doc.mu.Lock()
	defer doc.mu.Unlock()
	if err := doc.requireOpen(); err != nil {
		return nil, err
	}
	if pageIndex < 0 {
		return nil, errors.New("negative page index")
	}
	annotation, err := cString(annotationID)
	if err != nil {
		return nil, err
	}
	prepared := make([]AnnotationGraphic, len(objects))
	copy(prepared, objects)
	for index := range prepared {
		if prepared[index].Kind == "path" && prepared[index].LineWidth == 0 {
			prepared[index].LineWidth = 0.4
		}
	}
	payload, err := json.Marshal(struct {
		Objects []AnnotationGraphic `json:"objects"`
	}{prepared})
	if err != nil {
		return nil, fmt.Errorf("encode annotation objects: %w", err)
	}
	request, err := cString(string(payload))
	if err != nil {
		return nil, err
	}
	runtime.LockOSThread()
	defer runtime.UnlockOSThread()
	var buffer nativeBuffer
	if err := nativeError(api.appendAnnotationObjects(doc.handle, uintptr(pageIndex), &annotation[0], &request[0], &buffer)); err != nil {
		return nil, fmt.Errorf("append annotation objects: %w", err)
	}
	defer api.bufferFree(buffer)
	data, err := bytesAt(buffer)
	if err != nil {
		return nil, err
	}
	var ids []string
	if err := json.Unmarshal(data, &ids); err != nil {
		return nil, fmt.Errorf("decode annotation object IDs: %w", err)
	}
	return ids, nil
}

// UpdateAnnotationObject changes one annotation appearance object.
func (doc *Document) UpdateAnnotationObject(pageIndex int, annotationID, objectID string, update AnnotationObjectUpdate) error {
	doc.mu.Lock()
	defer doc.mu.Unlock()
	if err := doc.requireOpen(); err != nil {
		return err
	}
	if pageIndex < 0 {
		return errors.New("negative page index")
	}
	if update.ClearCTM && update.CTM != nil {
		return errors.New("CTM and ClearCTM conflict")
	}
	annotation, err := cString(annotationID)
	if err != nil {
		return err
	}
	object, err := cString(objectID)
	if err != nil {
		return err
	}
	fields := make(map[string]any)
	if update.Boundary != nil {
		fields["boundary"] = update.Boundary
	}
	if update.CTM != nil {
		fields["ctm"] = update.CTM
	}
	if update.ClearCTM {
		fields["ctm"] = nil
	}
	if update.Visible != nil {
		fields["visible"] = update.Visible
	}
	if update.Alpha != nil {
		fields["alpha"] = update.Alpha
	}
	payload, err := json.Marshal(fields)
	if err != nil {
		return fmt.Errorf("encode annotation object update: %w", err)
	}
	request, err := cString(string(payload))
	if err != nil {
		return err
	}
	runtime.LockOSThread()
	defer runtime.UnlockOSThread()
	if err := nativeError(api.updateAnnotationObject(doc.handle, uintptr(pageIndex), &annotation[0], &object[0], &request[0])); err != nil {
		return fmt.Errorf("update annotation object: %w", err)
	}
	return nil
}

// SetAnnotationObjectName sets or clears a graphic object's optional name.
func (doc *Document) SetAnnotationObjectName(pageIndex int, annotationID, objectID string, name *string) error {
	return doc.setAnnotationObjectString(pageIndex, annotationID, objectID, name,
		api.setAnnotationObjectName, "set annotation object name")
}

// SetAnnotationObjectBlendMode sets or clears a graphic object's compositing mode.
func (doc *Document) SetAnnotationObjectBlendMode(pageIndex int, annotationID, objectID string, blendMode *string) error {
	return doc.setAnnotationObjectString(pageIndex, annotationID, objectID, blendMode,
		api.setAnnotationObjectBlendMode, "set annotation object BlendMode")
}

// SetAnnotationObjectDrawParam sets a visible DrawParam ID; an empty ID clears it.
func (doc *Document) SetAnnotationObjectDrawParam(pageIndex int, annotationID, objectID, drawParam string) error {
	var value *string
	if drawParam != "" {
		value = &drawParam
	}
	return doc.setAnnotationObjectString(pageIndex, annotationID, objectID, value,
		api.setAnnotationObjectDrawParam, "set annotation object DrawParam")
}

func (doc *Document) setAnnotationObjectString(
	pageIndex int, annotationID, objectID string, value *string,
	call func(uintptr, uintptr, *byte, *byte, *byte) int32, operation string,
) error {
	doc.mu.Lock()
	defer doc.mu.Unlock()
	if err := doc.requireOpen(); err != nil {
		return err
	}
	if pageIndex < 0 {
		return errors.New("negative page index")
	}
	annotation, err := cString(annotationID)
	if err != nil {
		return err
	}
	object, err := cString(objectID)
	if err != nil {
		return err
	}
	var encoded *byte
	if value != nil {
		data, err := cString(*value)
		if err != nil {
			return err
		}
		encoded = &data[0]
	}
	runtime.LockOSThread()
	defer runtime.UnlockOSThread()
	if err := nativeError(call(doc.handle, uintptr(pageIndex), &annotation[0], &object[0], encoded)); err != nil {
		return fmt.Errorf("%s: %w", operation, err)
	}
	return nil
}

// RemoveAnnotationObject removes one direct annotation appearance object by ID.
func (doc *Document) RemoveAnnotationObject(pageIndex int, annotationID, objectID string) error {
	doc.mu.Lock()
	defer doc.mu.Unlock()
	if err := doc.requireOpen(); err != nil {
		return err
	}
	if pageIndex < 0 {
		return errors.New("negative page index")
	}
	annotation, err := cString(annotationID)
	if err != nil {
		return err
	}
	object, err := cString(objectID)
	if err != nil {
		return err
	}
	runtime.LockOSThread()
	defer runtime.UnlockOSThread()
	if err := nativeError(api.removeAnnotationObject(doc.handle, uintptr(pageIndex), &annotation[0], &object[0])); err != nil {
		return fmt.Errorf("remove annotation object: %w", err)
	}
	return nil
}

// SetAnnotationPathData replaces one annotation PathObject's drawing commands.
func (doc *Document) SetAnnotationPathData(pageIndex int, annotationID, objectID, commands string) error {
	doc.mu.Lock()
	defer doc.mu.Unlock()
	if err := doc.requireOpen(); err != nil {
		return err
	}
	if pageIndex < 0 {
		return errors.New("negative page index")
	}
	annotation, err := cString(annotationID)
	if err != nil {
		return err
	}
	object, err := cString(objectID)
	if err != nil {
		return err
	}
	data, err := cString(commands)
	if err != nil {
		return err
	}
	runtime.LockOSThread()
	defer runtime.UnlockOSThread()
	if err := nativeError(api.setAnnotationPathData(
		doc.handle, uintptr(pageIndex), &annotation[0], &object[0], &data[0],
	)); err != nil {
		return fmt.Errorf("set annotation path data: %w", err)
	}
	return nil
}

// SetAnnotationPathStrokeStyle atomically replaces direct annotation path stroke geometry.
func (doc *Document) SetAnnotationPathStrokeStyle(pageIndex int, annotationID, objectID string, style PathStrokeStyle) error {
	doc.mu.Lock()
	defer doc.mu.Unlock()
	if err := doc.requireOpen(); err != nil {
		return err
	}
	if pageIndex < 0 {
		return errors.New("negative page index")
	}
	annotation, err := cString(annotationID)
	if err != nil {
		return err
	}
	object, err := cString(objectID)
	if err != nil {
		return err
	}
	data, err := json.Marshal(style)
	if err != nil {
		return fmt.Errorf("encode path stroke style: %w", err)
	}
	payload, err := cString(string(data))
	if err != nil {
		return err
	}
	runtime.LockOSThread()
	defer runtime.UnlockOSThread()
	if err := nativeError(api.setAnnotationPathStrokeStyle(
		doc.handle, uintptr(pageIndex), &annotation[0], &object[0], &payload[0],
	)); err != nil {
		return fmt.Errorf("set annotation path stroke style: %w", err)
	}
	return nil
}

// SetAnnotationTextTypography replaces direct typography on an annotation TextObject.
func (doc *Document) SetAnnotationTextTypography(pageIndex int, annotationID, objectID string, style TextTypographyStyle) error {
	doc.mu.Lock()
	defer doc.mu.Unlock()
	if err := doc.requireOpen(); err != nil {
		return err
	}
	if pageIndex < 0 {
		return errors.New("negative page index")
	}
	annotation, err := cString(annotationID)
	if err != nil {
		return err
	}
	object, err := cString(objectID)
	if err != nil {
		return err
	}
	data, err := json.Marshal(style)
	if err != nil {
		return fmt.Errorf("encode text typography: %w", err)
	}
	payload, err := cString(string(data))
	if err != nil {
		return err
	}
	runtime.LockOSThread()
	defer runtime.UnlockOSThread()
	if err := nativeError(api.setAnnotationTextTypography(
		doc.handle, uintptr(pageIndex), &annotation[0], &object[0], &payload[0],
	)); err != nil {
		return fmt.Errorf("set annotation text typography: %w", err)
	}
	return nil
}

// SetAnnotationTextPaintFlags updates direct annotation text paint switches atomically.
func (doc *Document) SetAnnotationTextPaintFlags(pageIndex int, annotationID, objectID string, flags TextPaintFlags) error {
	doc.mu.Lock()
	defer doc.mu.Unlock()
	if err := doc.requireOpen(); err != nil {
		return err
	}
	if pageIndex < 0 {
		return errors.New("negative page index")
	}
	annotation, err := cString(annotationID)
	if err != nil {
		return err
	}
	object, err := cString(objectID)
	if err != nil {
		return err
	}
	data, err := json.Marshal(flags)
	if err != nil {
		return fmt.Errorf("encode annotation text paint flags: %w", err)
	}
	payload, err := cString(string(data))
	if err != nil {
		return err
	}
	runtime.LockOSThread()
	defer runtime.UnlockOSThread()
	if err := nativeError(api.setAnnotationTextPaintFlags(
		doc.handle, uintptr(pageIndex), &annotation[0], &object[0], &payload[0],
	)); err != nil {
		return fmt.Errorf("set annotation text paint flags: %w", err)
	}
	return nil
}

// SetAnnotationTextCodeLayout replaces direct positioning of an annotation text run.
func (doc *Document) SetAnnotationTextCodeLayout(pageIndex int, annotationID, objectID string, codeIndex int, layout TextCodeLayout) error {
	doc.mu.Lock()
	defer doc.mu.Unlock()
	if err := doc.requireOpen(); err != nil {
		return err
	}
	if pageIndex < 0 || codeIndex < 0 {
		return errors.New("negative page or TextCode index")
	}
	annotation, err := cString(annotationID)
	if err != nil {
		return err
	}
	object, err := cString(objectID)
	if err != nil {
		return err
	}
	data, err := json.Marshal(layout)
	if err != nil {
		return fmt.Errorf("encode TextCode layout: %w", err)
	}
	payload, err := cString(string(data))
	if err != nil {
		return err
	}
	runtime.LockOSThread()
	defer runtime.UnlockOSThread()
	if err := nativeError(api.setAnnotationTextCodeLayout(
		doc.handle, uintptr(pageIndex), &annotation[0], &object[0], uintptr(codeIndex), &payload[0],
	)); err != nil {
		return fmt.Errorf("set annotation TextCode layout: %w", err)
	}
	return nil
}

// AppendAnnotationTextCode adds a positioned run to an unshaped annotation TextObject.
func (doc *Document) AppendAnnotationTextCode(pageIndex int, annotationID, objectID, text string, layout TextCodeLayout) error {
	doc.mu.Lock()
	defer doc.mu.Unlock()
	if err := doc.requireOpen(); err != nil {
		return err
	}
	if pageIndex < 0 {
		return errors.New("negative page index")
	}
	payload, err := textRequest(map[string]any{
		"operation": "append", "page_index": pageIndex,
		"annotation_id": annotationID, "object_id": objectID,
		"text": text, "layout": layout,
	})
	if err != nil {
		return err
	}
	runtime.LockOSThread()
	defer runtime.UnlockOSThread()
	if err := nativeError(api.editAnnotationTextCode(doc.handle, &payload[0])); err != nil {
		return fmt.Errorf("append annotation text code: %w", err)
	}
	return nil
}

// RemoveAnnotationTextCode removes one annotation text run and preserves the next run's position.
func (doc *Document) RemoveAnnotationTextCode(pageIndex int, annotationID, objectID string, codeIndex int) error {
	doc.mu.Lock()
	defer doc.mu.Unlock()
	if err := doc.requireOpen(); err != nil {
		return err
	}
	if pageIndex < 0 || codeIndex < 0 {
		return errors.New("negative page or TextCode index")
	}
	annotation, err := cString(annotationID)
	if err != nil {
		return err
	}
	object, err := cString(objectID)
	if err != nil {
		return err
	}
	runtime.LockOSThread()
	defer runtime.UnlockOSThread()
	if err := nativeError(api.removeAnnotationTextCode(
		doc.handle, uintptr(pageIndex), &annotation[0], &object[0], uintptr(codeIndex),
	)); err != nil {
		return fmt.Errorf("remove annotation text code: %w", err)
	}
	return nil
}

// SetAnnotationTextFont sets an annotation TextObject's font to a page-visible resource.
func (doc *Document) SetAnnotationTextFont(pageIndex int, annotationID, objectID, fontID string) error {
	doc.mu.Lock()
	defer doc.mu.Unlock()
	if err := doc.requireOpen(); err != nil {
		return err
	}
	if pageIndex < 0 {
		return errors.New("negative page index")
	}
	annotation, err := cString(annotationID)
	if err != nil {
		return err
	}
	object, err := cString(objectID)
	if err != nil {
		return err
	}
	font, err := cString(fontID)
	if err != nil {
		return err
	}
	runtime.LockOSThread()
	defer runtime.UnlockOSThread()
	if err := nativeError(api.setAnnotationTextFont(
		doc.handle, uintptr(pageIndex), &annotation[0], &object[0], &font[0],
	)); err != nil {
		return fmt.Errorf("set annotation text font: %w", err)
	}
	return nil
}

// SetAnnotationTextCode replaces one run of an unshaped annotation TextObject.
func (doc *Document) SetAnnotationTextCode(pageIndex int, annotationID, objectID string, codeIndex int, text string) error {
	doc.mu.Lock()
	defer doc.mu.Unlock()
	if err := doc.requireOpen(); err != nil {
		return err
	}
	if pageIndex < 0 || codeIndex < 0 {
		return errors.New("negative page or TextCode index")
	}
	payload, err := textRequest(map[string]any{
		"operation": "set", "page_index": pageIndex,
		"annotation_id": annotationID, "object_id": objectID,
		"code_index": codeIndex, "text": text,
	})
	if err != nil {
		return err
	}
	runtime.LockOSThread()
	defer runtime.UnlockOSThread()
	if err := nativeError(api.editAnnotationTextCode(doc.handle, &payload[0])); err != nil {
		return fmt.Errorf("set annotation text code: %w", err)
	}
	return nil
}

// SetAnnotationGraphicResource replaces an annotation image or composite resource.
func (doc *Document) SetAnnotationGraphicResource(pageIndex int, annotationID, objectID, resourceID string) error {
	doc.mu.Lock()
	defer doc.mu.Unlock()
	if err := doc.requireOpen(); err != nil {
		return err
	}
	if pageIndex < 0 {
		return errors.New("negative page index")
	}
	annotation, err := cString(annotationID)
	if err != nil {
		return err
	}
	object, err := cString(objectID)
	if err != nil {
		return err
	}
	resource, err := cString(resourceID)
	if err != nil {
		return err
	}
	runtime.LockOSThread()
	defer runtime.UnlockOSThread()
	if err := nativeError(api.setAnnotationGraphicResource(
		doc.handle, uintptr(pageIndex), &annotation[0], &object[0], &resource[0],
	)); err != nil {
		return fmt.Errorf("set annotation graphic resource: %w", err)
	}
	return nil
}

// RemoveAnnotation removes one annotation from a page by its OFD ID.
func (doc *Document) RemoveAnnotation(pageIndex int, annotationID string) error {
	doc.mu.Lock()
	defer doc.mu.Unlock()
	if err := doc.requireOpen(); err != nil {
		return err
	}
	if pageIndex < 0 {
		return errors.New("negative page index")
	}
	id, err := cString(annotationID)
	if err != nil {
		return err
	}
	runtime.LockOSThread()
	defer runtime.UnlockOSThread()
	if err := nativeError(api.removeAnnotation(doc.handle, uintptr(pageIndex), &id[0])); err != nil {
		return fmt.Errorf("remove annotation: %w", err)
	}
	return nil
}

// SetAnnotationCreator sets an annotation's required Creator attribute.
func (doc *Document) SetAnnotationCreator(pageIndex int, annotationID, creator string) error {
	doc.mu.Lock()
	defer doc.mu.Unlock()
	if err := doc.requireOpen(); err != nil {
		return err
	}
	if pageIndex < 0 {
		return errors.New("negative page index")
	}
	id, err := cString(annotationID)
	if err != nil {
		return err
	}
	value, err := cString(creator)
	if err != nil {
		return err
	}
	runtime.LockOSThread()
	defer runtime.UnlockOSThread()
	if err := nativeError(api.setAnnotationCreator(doc.handle, uintptr(pageIndex), &id[0], &value[0])); err != nil {
		return fmt.Errorf("set annotation creator: %w", err)
	}
	return nil
}

// SetAnnotationSubtype sets or removes an annotation's optional Subtype attribute.
func (doc *Document) SetAnnotationSubtype(pageIndex int, annotationID string, subtype *string) error {
	doc.mu.Lock()
	defer doc.mu.Unlock()
	if err := doc.requireOpen(); err != nil {
		return err
	}
	if pageIndex < 0 {
		return errors.New("negative page index")
	}
	id, err := cString(annotationID)
	if err != nil {
		return err
	}
	var value *byte
	if subtype != nil {
		encoded, err := cString(*subtype)
		if err != nil {
			return err
		}
		value = &encoded[0]
	}
	runtime.LockOSThread()
	defer runtime.UnlockOSThread()
	if err := nativeError(api.setAnnotationSubtype(doc.handle, uintptr(pageIndex), &id[0], value)); err != nil {
		return fmt.Errorf("set annotation subtype: %w", err)
	}
	return nil
}

// SetAnnotationLastModDate sets an annotation's LastModDate as YYYY-MM-DD.
func (doc *Document) SetAnnotationLastModDate(pageIndex int, annotationID, date string) error {
	doc.mu.Lock()
	defer doc.mu.Unlock()
	if err := doc.requireOpen(); err != nil {
		return err
	}
	if pageIndex < 0 {
		return errors.New("negative page index")
	}
	id, err := cString(annotationID)
	if err != nil {
		return err
	}
	value, err := cString(date)
	if err != nil {
		return err
	}
	runtime.LockOSThread()
	defer runtime.UnlockOSThread()
	if err := nativeError(api.setAnnotationLastModDate(doc.handle, uintptr(pageIndex), &id[0], &value[0])); err != nil {
		return fmt.Errorf("set annotation last modification date: %w", err)
	}
	return nil
}

// SetAnnotationAppearanceBoundary moves or resizes an annotation in page millimeters.
func (doc *Document) SetAnnotationAppearanceBoundary(pageIndex int, annotationID string, boundary [4]float64) error {
	doc.mu.Lock()
	defer doc.mu.Unlock()
	if err := doc.requireOpen(); err != nil {
		return err
	}
	if pageIndex < 0 {
		return errors.New("negative page index")
	}
	id, err := cString(annotationID)
	if err != nil {
		return err
	}
	runtime.LockOSThread()
	defer runtime.UnlockOSThread()
	if err := nativeError(api.setAnnotationAppearanceBoundary(doc.handle, uintptr(pageIndex), &id[0], boundary[0], boundary[1], boundary[2], boundary[3])); err != nil {
		return fmt.Errorf("set annotation appearance boundary: %w", err)
	}
	return nil
}

// SetAnnotationRemark sets or removes an annotation's optional Remark content.
func (doc *Document) SetAnnotationRemark(pageIndex int, annotationID string, remark *string) error {
	doc.mu.Lock()
	defer doc.mu.Unlock()
	if err := doc.requireOpen(); err != nil {
		return err
	}
	if pageIndex < 0 {
		return errors.New("negative page index")
	}
	id, err := cString(annotationID)
	if err != nil {
		return err
	}
	var value *byte
	if remark != nil {
		encoded, err := cString(*remark)
		if err != nil {
			return err
		}
		value = &encoded[0]
	}
	runtime.LockOSThread()
	defer runtime.UnlockOSThread()
	if err := nativeError(api.setAnnotationRemark(doc.handle, uintptr(pageIndex), &id[0], value)); err != nil {
		return fmt.Errorf("set annotation remark: %w", err)
	}
	return nil
}

// SetAnnotationParameter adds or replaces a named annotation parameter.
func (doc *Document) SetAnnotationParameter(pageIndex int, annotationID, name, value string) error {
	doc.mu.Lock()
	defer doc.mu.Unlock()
	if err := doc.requireOpen(); err != nil {
		return err
	}
	if pageIndex < 0 {
		return errors.New("negative page index")
	}
	id, err := cString(annotationID)
	if err != nil {
		return err
	}
	key, err := cString(name)
	if err != nil {
		return err
	}
	content, err := cString(value)
	if err != nil {
		return err
	}
	runtime.LockOSThread()
	defer runtime.UnlockOSThread()
	if err := nativeError(api.setAnnotationParameter(doc.handle, uintptr(pageIndex), &id[0], &key[0], &content[0])); err != nil {
		return fmt.Errorf("set annotation parameter: %w", err)
	}
	return nil
}

// RemoveAnnotationParameter removes a named annotation parameter.
func (doc *Document) RemoveAnnotationParameter(pageIndex int, annotationID, name string) error {
	doc.mu.Lock()
	defer doc.mu.Unlock()
	if err := doc.requireOpen(); err != nil {
		return err
	}
	if pageIndex < 0 {
		return errors.New("negative page index")
	}
	id, err := cString(annotationID)
	if err != nil {
		return err
	}
	key, err := cString(name)
	if err != nil {
		return err
	}
	runtime.LockOSThread()
	defer runtime.UnlockOSThread()
	if err := nativeError(api.removeAnnotationParameter(doc.handle, uintptr(pageIndex), &id[0], &key[0])); err != nil {
		return fmt.Errorf("remove annotation parameter: %w", err)
	}
	return nil
}

// SetAnnotationVisible controls an annotation in previews and PDF output.
func (doc *Document) SetAnnotationVisible(pageIndex int, annotationID string, visible bool) error {
	doc.mu.Lock()
	defer doc.mu.Unlock()
	if err := doc.requireOpen(); err != nil {
		return err
	}
	if pageIndex < 0 {
		return errors.New("negative page index")
	}
	id, err := cString(annotationID)
	if err != nil {
		return err
	}
	value := int32(0)
	if visible {
		value = 1
	}
	runtime.LockOSThread()
	defer runtime.UnlockOSThread()
	if err := nativeError(api.setAnnotationVisible(doc.handle, uintptr(pageIndex), &id[0], value)); err != nil {
		return fmt.Errorf("set annotation visible: %w", err)
	}
	return nil
}

// SetAnnotationPrint controls whether an annotation is included in PDF output.
func (doc *Document) SetAnnotationPrint(pageIndex int, annotationID string, print bool) error {
	doc.mu.Lock()
	defer doc.mu.Unlock()
	if err := doc.requireOpen(); err != nil {
		return err
	}
	if pageIndex < 0 {
		return errors.New("negative page index")
	}
	id, err := cString(annotationID)
	if err != nil {
		return err
	}
	value := int32(0)
	if print {
		value = 1
	}
	runtime.LockOSThread()
	defer runtime.UnlockOSThread()
	if err := nativeError(api.setAnnotationPrint(doc.handle, uintptr(pageIndex), &id[0], value)); err != nil {
		return fmt.Errorf("set annotation print: %w", err)
	}
	return nil
}

// SetAnnotationFlag updates NoZoom, NoRotate, or ReadOnly metadata.
func (doc *Document) SetAnnotationFlag(pageIndex int, annotationID, flag string, value bool) error {
	doc.mu.Lock()
	defer doc.mu.Unlock()
	if err := doc.requireOpen(); err != nil {
		return err
	}
	if pageIndex < 0 {
		return errors.New("negative page index")
	}
	if flag != "NoZoom" && flag != "NoRotate" && flag != "ReadOnly" {
		return errors.New("unsupported annotation flag")
	}
	id, err := cString(annotationID)
	if err != nil {
		return err
	}
	name, err := cString(flag)
	if err != nil {
		return err
	}
	var nativeValue int32
	if value {
		nativeValue = 1
	}
	runtime.LockOSThread()
	defer runtime.UnlockOSThread()
	if err := nativeError(api.setAnnotationFlag(doc.handle, uintptr(pageIndex), &id[0], &name[0], nativeValue)); err != nil {
		return fmt.Errorf("set annotation flag: %w", err)
	}
	return nil
}

// Fonts returns font resource metadata.
func (doc *Document) Fonts() ([]Font, error) {
	doc.mu.Lock()
	defer doc.mu.Unlock()
	if err := doc.requireOpen(); err != nil {
		return nil, err
	}
	runtime.LockOSThread()
	defer runtime.UnlockOSThread()
	var buffer nativeBuffer
	if err := nativeError(api.fonts(doc.handle, &buffer)); err != nil {
		return nil, fmt.Errorf("list fonts: %w", err)
	}
	defer api.bufferFree(buffer)
	data, err := bytesAt(buffer)
	if err != nil {
		return nil, err
	}
	var fonts []Font
	if err := json.Unmarshal(data, &fonts); err != nil {
		return nil, fmt.Errorf("decode fonts: %w", err)
	}
	return fonts, nil
}

// FontData returns the original bytes of an embedded font by zero-based index.
func (doc *Document) FontData(index int) ([]byte, error) {
	doc.mu.Lock()
	defer doc.mu.Unlock()
	if err := doc.requireOpen(); err != nil {
		return nil, err
	}
	if index < 0 {
		return nil, errors.New("negative font index")
	}
	runtime.LockOSThread()
	defer runtime.UnlockOSThread()
	var buffer nativeBuffer
	if err := nativeError(api.fontData(doc.handle, uintptr(index), &buffer)); err != nil {
		return nil, fmt.Errorf("read font data: %w", err)
	}
	defer api.bufferFree(buffer)
	return bytesAt(buffer)
}

// PageFonts returns fonts available to a page and its templates.
func (doc *Document) PageFonts(pageIndex int) ([]Font, error) {
	doc.mu.Lock()
	defer doc.mu.Unlock()
	if err := doc.requireOpen(); err != nil {
		return nil, err
	}
	if pageIndex < 0 {
		return nil, errors.New("negative page index")
	}
	runtime.LockOSThread()
	defer runtime.UnlockOSThread()
	var buffer nativeBuffer
	if err := nativeError(api.pageFonts(doc.handle, uintptr(pageIndex), &buffer)); err != nil {
		return nil, fmt.Errorf("list page fonts: %w", err)
	}
	defer api.bufferFree(buffer)
	data, err := bytesAt(buffer)
	if err != nil {
		return nil, err
	}
	var fonts []Font
	if err := json.Unmarshal(data, &fonts); err != nil {
		return nil, fmt.Errorf("decode page fonts: %w", err)
	}
	return fonts, nil
}

// PageFontData returns embedded bytes for an index in PageFonts.
func (doc *Document) PageFontData(pageIndex, fontIndex int) ([]byte, error) {
	doc.mu.Lock()
	defer doc.mu.Unlock()
	if err := doc.requireOpen(); err != nil {
		return nil, err
	}
	if pageIndex < 0 || fontIndex < 0 {
		return nil, errors.New("negative page or font index")
	}
	runtime.LockOSThread()
	defer runtime.UnlockOSThread()
	var buffer nativeBuffer
	if err := nativeError(api.pageFontData(doc.handle, uintptr(pageIndex), uintptr(fontIndex), &buffer)); err != nil {
		return nil, fmt.Errorf("read page font data: %w", err)
	}
	defer api.bufferFree(buffer)
	return bytesAt(buffer)
}

// Signatures returns descriptions of signatures in all document bodies.
func (doc *Document) Signatures() ([]Signature, error) {
	doc.mu.Lock()
	defer doc.mu.Unlock()
	if err := doc.requireOpen(); err != nil {
		return nil, err
	}
	runtime.LockOSThread()
	defer runtime.UnlockOSThread()
	var buffer nativeBuffer
	if err := nativeError(api.signatures(doc.handle, &buffer)); err != nil {
		return nil, fmt.Errorf("list signatures: %w", err)
	}
	defer api.bufferFree(buffer)
	data, err := bytesAt(buffer)
	if err != nil {
		return nil, err
	}
	var signatures []Signature
	if err := json.Unmarshal(data, &signatures); err != nil {
		return nil, fmt.Errorf("decode signatures: %w", err)
	}
	return signatures, nil
}

// SignedValueData returns the opaque cryptographic signature value.
func (doc *Document) SignedValueData(index int) ([]byte, error) {
	doc.mu.Lock()
	defer doc.mu.Unlock()
	if err := doc.requireOpen(); err != nil {
		return nil, err
	}
	if index < 0 {
		return nil, errors.New("negative signature index")
	}
	runtime.LockOSThread()
	defer runtime.UnlockOSThread()
	var buffer nativeBuffer
	if err := nativeError(api.signedValueData(doc.handle, uintptr(index), &buffer)); err != nil {
		return nil, fmt.Errorf("read signature value: %w", err)
	}
	defer api.bufferFree(buffer)
	return bytesAt(buffer)
}

// SealData returns the electronic seal attached to a signature.
func (doc *Document) SealData(index int) ([]byte, error) {
	doc.mu.Lock()
	defer doc.mu.Unlock()
	if err := doc.requireOpen(); err != nil {
		return nil, err
	}
	if index < 0 {
		return nil, errors.New("negative signature index")
	}
	runtime.LockOSThread()
	defer runtime.UnlockOSThread()
	var buffer nativeBuffer
	if err := nativeError(api.sealData(doc.handle, uintptr(index), &buffer)); err != nil {
		return nil, fmt.Errorf("read seal: %w", err)
	}
	defer api.bufferFree(buffer)
	return bytesAt(buffer)
}

// CheckSignatureReferences compares protected file digests without authenticating the signature.
func (doc *Document) CheckSignatureReferences(index int) ([]SignatureReferenceCheck, error) {
	doc.mu.Lock()
	defer doc.mu.Unlock()
	if err := doc.requireOpen(); err != nil {
		return nil, err
	}
	if index < 0 {
		return nil, errors.New("negative signature index")
	}
	runtime.LockOSThread()
	defer runtime.UnlockOSThread()
	var buffer nativeBuffer
	if err := nativeError(api.checkReferences(doc.handle, uintptr(index), &buffer)); err != nil {
		return nil, fmt.Errorf("check signature references: %w", err)
	}
	defer api.bufferFree(buffer)
	data, err := bytesAt(buffer)
	if err != nil {
		return nil, err
	}
	var checks []SignatureReferenceCheck
	if err := json.Unmarshal(data, &checks); err != nil {
		return nil, fmt.Errorf("decode signature reference checks: %w", err)
	}
	return checks, nil
}

// CheckSignatureDescriptionHash compares Signature.xml with the hash in an SES v4/v5 seal.
// It does not authenticate the seal's cryptographic signature value.
func (doc *Document) CheckSignatureDescriptionHash(index int) (SignatureReferenceCheck, error) {
	doc.mu.Lock()
	defer doc.mu.Unlock()
	var check SignatureReferenceCheck
	if err := doc.requireOpen(); err != nil {
		return check, err
	}
	if index < 0 {
		return check, errors.New("negative signature index")
	}
	runtime.LockOSThread()
	defer runtime.UnlockOSThread()
	var buffer nativeBuffer
	if err := nativeError(api.checkDescription(doc.handle, uintptr(index), &buffer)); err != nil {
		return check, fmt.Errorf("check signature description hash: %w", err)
	}
	defer api.bufferFree(buffer)
	data, err := bytesAt(buffer)
	if err != nil {
		return check, err
	}
	if err := json.Unmarshal(data, &check); err != nil {
		return check, fmt.Errorf("decode signature description check: %w", err)
	}
	return check, nil
}

// CheckSignatureSealMatch compares a detached SES seal with the embedded copy.
// Nil means the check does not apply to this signature.
func (doc *Document) CheckSignatureSealMatch(index int) (*bool, error) {
	doc.mu.Lock()
	defer doc.mu.Unlock()
	if err := doc.requireOpen(); err != nil {
		return nil, err
	}
	if index < 0 {
		return nil, errors.New("negative signature index")
	}
	runtime.LockOSThread()
	defer runtime.UnlockOSThread()
	var result int32
	if err := nativeError(api.checkSealMatch(doc.handle, uintptr(index), &result)); err != nil {
		return nil, fmt.Errorf("check signature seal match: %w", err)
	}
	if result == -1 {
		return nil, nil
	}
	matched := result == 1
	return &matched, nil
}

// CheckSESSealSignature verifies the electronic seal's SM2 signature using
// its embedded issuer certificate. It does not establish issuer trust.
func (doc *Document) CheckSESSealSignature(index int) (bool, error) {
	return doc.CheckSESSealSignatureWithDistID(index, defaultSM2DistID)
}

// CheckSESSealSignatureWithDistID verifies the seal with a supplied SM2 ID.
func (doc *Document) CheckSESSealSignatureWithDistID(index int, sealDistID string) (bool, error) {
	doc.mu.Lock()
	defer doc.mu.Unlock()
	if err := doc.requireOpen(); err != nil {
		return false, err
	}
	if index < 0 {
		return false, errors.New("negative signature index")
	}
	distID, err := cString(sealDistID)
	if err != nil {
		return false, err
	}
	runtime.LockOSThread()
	defer runtime.UnlockOSThread()
	var result int32
	if err := nativeError(api.checkSealWithID(doc.handle, uintptr(index), &distID[0], &result)); err != nil {
		return false, fmt.Errorf("check SES seal signature: %w", err)
	}
	return result == 1, nil
}

// CheckSESSealCertificateValidity checks the embedded seal issuer certificate
// at a caller-supplied Unix timestamp. It does not establish issuer trust.
func (doc *Document) CheckSESSealCertificateValidity(index int, atUnixSeconds uint64) (bool, error) {
	doc.mu.Lock()
	defer doc.mu.Unlock()
	if err := doc.requireOpen(); err != nil {
		return false, err
	}
	if index < 0 {
		return false, errors.New("negative signature index")
	}
	runtime.LockOSThread()
	defer runtime.UnlockOSThread()
	var valid int32
	if err := nativeError(api.checkSealCertValidity(doc.handle, uintptr(index), atUnixSeconds, &valid)); err != nil {
		return false, fmt.Errorf("check SES seal certificate validity: %w", err)
	}
	return valid != 0, nil
}

// CheckSESSealSignerAuthorization checks whether the signer certificate is
// included in the electronic seal's certificate or digest list.
func (doc *Document) CheckSESSealSignerAuthorization(index int) (bool, error) {
	doc.mu.Lock()
	defer doc.mu.Unlock()
	if err := doc.requireOpen(); err != nil {
		return false, err
	}
	if index < 0 {
		return false, errors.New("negative signature index")
	}
	runtime.LockOSThread()
	defer runtime.UnlockOSThread()
	var valid int32
	if err := nativeError(api.checkSealSignerAuthorization(doc.handle, uintptr(index), &valid)); err != nil {
		return false, fmt.Errorf("check SES seal signer authorization: %w", err)
	}
	return valid != 0, nil
}

// CheckSESSealValidity checks the electronic seal's validity at a
// caller-supplied Unix timestamp. The timestamp is not independently trusted.
func (doc *Document) CheckSESSealValidity(index int, atUnixSeconds uint64) (bool, error) {
	doc.mu.Lock()
	defer doc.mu.Unlock()
	if err := doc.requireOpen(); err != nil {
		return false, err
	}
	if index < 0 {
		return false, errors.New("negative signature index")
	}
	runtime.LockOSThread()
	defer runtime.UnlockOSThread()
	var valid int32
	if err := nativeError(api.checkSealValidity(doc.handle, uintptr(index), atUnixSeconds, &valid)); err != nil {
		return false, fmt.Errorf("check SES seal validity: %w", err)
	}
	return valid != 0, nil
}

// CheckSESSealIssuerOrderedSM2Chain verifies the embedded seal issuer's
// ordered certificate chain against a caller-trusted anchor. The seal
// signature is checked separately with CheckSESSealSignature.
func (doc *Document) CheckSESSealIssuerOrderedSM2Chain(index int, intermediatesDER [][]byte, anchorCertificateDER []byte, atUnixSeconds uint64, certificateDistID string) (bool, error) {
	doc.mu.Lock()
	defer doc.mu.Unlock()
	if err := doc.requireOpen(); err != nil {
		return false, err
	}
	if index < 0 {
		return false, errors.New("negative signature index")
	}
	distID, err := cString(certificateDistID)
	if err != nil {
		return false, err
	}
	if intermediatesDER == nil {
		intermediatesDER = [][]byte{}
	}
	certificatesJSON, err := json.Marshal(intermediatesDER)
	if err != nil {
		return false, fmt.Errorf("encode intermediate certificates: %w", err)
	}
	var anchorPtr *byte
	if len(anchorCertificateDER) > 0 {
		anchorPtr = &anchorCertificateDER[0]
	}
	runtime.LockOSThread()
	defer runtime.UnlockOSThread()
	var valid int32
	if err := nativeError(api.checkSealIssuerChain(doc.handle, uintptr(index), &certificatesJSON[0], uintptr(len(certificatesJSON)), anchorPtr, uintptr(len(anchorCertificateDER)), atUnixSeconds, &distID[0], &valid)); err != nil {
		return false, fmt.Errorf("check SES seal issuer chain: %w", err)
	}
	return valid != 0, nil
}

// CheckSESSignatureValue checks the SES SM2 signature against its embedded
// certificate public key. It does not establish certificate trust.
func (doc *Document) CheckSESSignatureValue(index int) (bool, error) {
	return doc.CheckSESSignatureValueWithDistID(index, defaultSM2DistID)
}

// CheckSESSignatureValueWithDistID checks an SES signature with a supplied ID.
func (doc *Document) CheckSESSignatureValueWithDistID(index int, signatureDistID string) (bool, error) {
	doc.mu.Lock()
	defer doc.mu.Unlock()
	if err := doc.requireOpen(); err != nil {
		return false, err
	}
	if index < 0 {
		return false, errors.New("negative signature index")
	}
	distID, err := cString(signatureDistID)
	if err != nil {
		return false, err
	}
	runtime.LockOSThread()
	defer runtime.UnlockOSThread()
	var valid int32
	if err := nativeError(api.checkSESWithID(doc.handle, uintptr(index), &distID[0], &valid)); err != nil {
		return false, fmt.Errorf("check SES signature value: %w", err)
	}
	return valid != 0, nil
}

// CheckDigitalSignatureWithCertificate checks a raw SM2 signature using a
// caller-supplied DER X.509 certificate. It does not establish certificate trust.
func (doc *Document) CheckDigitalSignatureWithCertificate(index int, certificateDER []byte) (bool, error) {
	return doc.CheckDigitalSignatureWithCertificateAndDistID(index, certificateDER, defaultSM2DistID)
}

// CheckDigitalSignatureWithCertificateAndDistID checks a raw SM2 signature
// with the supplied signer certificate and distinguishing ID.
func (doc *Document) CheckDigitalSignatureWithCertificateAndDistID(index int, certificateDER []byte, signatureDistID string) (bool, error) {
	doc.mu.Lock()
	defer doc.mu.Unlock()
	if err := doc.requireOpen(); err != nil {
		return false, err
	}
	if index < 0 {
		return false, errors.New("negative signature index")
	}
	distID, err := cString(signatureDistID)
	if err != nil {
		return false, err
	}
	var certificatePtr *byte
	if len(certificateDER) > 0 {
		certificatePtr = &certificateDER[0]
	}
	runtime.LockOSThread()
	defer runtime.UnlockOSThread()
	var valid int32
	if err := nativeError(api.checkDigitalWithID(doc.handle, uintptr(index), certificatePtr, uintptr(len(certificateDER)), &distID[0], &valid)); err != nil {
		return false, fmt.Errorf("check digital signature: %w", err)
	}
	return valid != 0, nil
}

// CheckSignatureCertificateValidity checks the signer certificate's validity
// interval at a supplied Unix timestamp in UTC seconds. SES uses its embedded
// certificate; raw digital signatures require certificateDER. This does not
// verify the signature or certificate chain.
func (doc *Document) CheckSignatureCertificateValidity(index int, atUnixSeconds uint64, certificateDER []byte) (bool, error) {
	doc.mu.Lock()
	defer doc.mu.Unlock()
	if err := doc.requireOpen(); err != nil {
		return false, err
	}
	if index < 0 {
		return false, errors.New("negative signature index")
	}
	var certificatePtr *byte
	if len(certificateDER) > 0 {
		certificatePtr = &certificateDER[0]
	}
	runtime.LockOSThread()
	defer runtime.UnlockOSThread()
	var valid int32
	if err := nativeError(api.checkCertValidity(doc.handle, uintptr(index), certificatePtr, uintptr(len(certificateDER)), atUnixSeconds, &valid)); err != nil {
		return false, fmt.Errorf("check signer certificate validity: %w", err)
	}
	return valid != 0, nil
}

// CheckSignatureDirectTrustAnchor checks one SM2 certificate issuance step,
// certificate validity and CA usage, and the OFD signature value. The anchor
// is trusted by the caller. This does not check revocation or file digests.
func (doc *Document) CheckSignatureDirectTrustAnchor(index int, signerCertificateDER, anchorCertificateDER []byte, atUnixSeconds uint64, certificateDistID string) (bool, error) {
	doc.mu.Lock()
	defer doc.mu.Unlock()
	if err := doc.requireOpen(); err != nil {
		return false, err
	}
	if index < 0 {
		return false, errors.New("negative signature index")
	}
	distID, err := cString(certificateDistID)
	if err != nil {
		return false, err
	}
	var signerPtr, anchorPtr *byte
	if len(signerCertificateDER) > 0 {
		signerPtr = &signerCertificateDER[0]
	}
	if len(anchorCertificateDER) > 0 {
		anchorPtr = &anchorCertificateDER[0]
	}
	runtime.LockOSThread()
	defer runtime.UnlockOSThread()
	var valid int32
	if err := nativeError(api.checkDirectAnchor(doc.handle, uintptr(index), signerPtr, uintptr(len(signerCertificateDER)), anchorPtr, uintptr(len(anchorCertificateDER)), atUnixSeconds, &distID[0], &valid)); err != nil {
		return false, fmt.Errorf("check direct trust anchor: %w", err)
	}
	return valid != 0, nil
}

// VerifySignatureWithDirectTrustAnchor reports protected file digests, the
// signature value, certificate validity, and one trusted issuance step.
func (doc *Document) VerifySignatureWithDirectTrustAnchor(index int, signerCertificateDER, anchorCertificateDER []byte, atUnixSeconds uint64, certificateDistID string) (SignatureVerificationReport, error) {
	return doc.VerifySignatureWithDirectTrustAnchorAndDistIDs(index, signerCertificateDER, anchorCertificateDER, atUnixSeconds, certificateDistID, defaultSM2DistID)
}

// VerifySignatureWithDirectTrustAnchorAndDistIDs uses separate certificate
// and OFD signature SM2 distinguishing identifiers.
func (doc *Document) VerifySignatureWithDirectTrustAnchorAndDistIDs(index int, signerCertificateDER, anchorCertificateDER []byte, atUnixSeconds uint64, certificateDistID, signatureDistID string) (SignatureVerificationReport, error) {
	doc.mu.Lock()
	defer doc.mu.Unlock()
	var report SignatureVerificationReport
	if err := doc.requireOpen(); err != nil {
		return report, err
	}
	if index < 0 {
		return report, errors.New("negative signature index")
	}
	distID, err := cString(certificateDistID)
	if err != nil {
		return report, err
	}
	signatureID, err := cString(signatureDistID)
	if err != nil {
		return report, err
	}
	var signerPtr, anchorPtr *byte
	if len(signerCertificateDER) > 0 {
		signerPtr = &signerCertificateDER[0]
	}
	if len(anchorCertificateDER) > 0 {
		anchorPtr = &anchorCertificateDER[0]
	}
	runtime.LockOSThread()
	defer runtime.UnlockOSThread()
	var buffer nativeBuffer
	if err := nativeError(api.verifyDirectWithID(doc.handle, uintptr(index), signerPtr, uintptr(len(signerCertificateDER)), anchorPtr, uintptr(len(anchorCertificateDER)), atUnixSeconds, &distID[0], &signatureID[0], &buffer)); err != nil {
		return report, fmt.Errorf("verify signature with direct trust anchor: %w", err)
	}
	defer api.bufferFree(buffer)
	data, err := bytesAt(buffer)
	if err != nil {
		return report, err
	}
	if err := json.Unmarshal(data, &report); err != nil {
		return report, fmt.Errorf("decode signature verification report: %w", err)
	}
	return report, nil
}

// VerifySignatureWithOrderedSM2Chain reports protected file, signature, and
// ordered certificate chain checks. Intermediates run from signer to anchor.
// It does not check revocation, policies, name constraints, or trusted time.
func (doc *Document) VerifySignatureWithOrderedSM2Chain(index int, signerCertificateDER []byte, intermediatesDER [][]byte, anchorCertificateDER []byte, atUnixSeconds uint64, certificateDistID string) (SignatureChainVerificationReport, error) {
	return doc.VerifySignatureWithOrderedSM2ChainAndDistIDs(index, signerCertificateDER, intermediatesDER, anchorCertificateDER, atUnixSeconds, certificateDistID, defaultSM2DistID)
}

// VerifySignatureWithOrderedSM2ChainAndDistIDs uses separate certificate and
// OFD signature SM2 distinguishing identifiers.
func (doc *Document) VerifySignatureWithOrderedSM2ChainAndDistIDs(index int, signerCertificateDER []byte, intermediatesDER [][]byte, anchorCertificateDER []byte, atUnixSeconds uint64, certificateDistID, signatureDistID string) (SignatureChainVerificationReport, error) {
	return doc.verifySignatureChain(index, signerCertificateDER, intermediatesDER, anchorCertificateDER, atUnixSeconds, certificateDistID, signatureDistID, false)
}

// VerifySignatureWithCertificatePool finds a valid signer-to-anchor SM2 chain
// among unordered intermediate candidates before checking the signature.
func (doc *Document) VerifySignatureWithCertificatePool(index int, signerCertificateDER []byte, candidatesDER [][]byte, anchorCertificateDER []byte, atUnixSeconds uint64, certificateDistID, signatureDistID string) (SignatureChainVerificationReport, error) {
	return doc.verifySignatureChain(index, signerCertificateDER, candidatesDER, anchorCertificateDER, atUnixSeconds, certificateDistID, signatureDistID, true)
}

func (doc *Document) verifySignatureChain(index int, signerCertificateDER []byte, intermediatesDER [][]byte, anchorCertificateDER []byte, atUnixSeconds uint64, certificateDistID, signatureDistID string, unordered bool) (SignatureChainVerificationReport, error) {
	doc.mu.Lock()
	defer doc.mu.Unlock()
	var report SignatureChainVerificationReport
	if err := doc.requireOpen(); err != nil {
		return report, err
	}
	if index < 0 {
		return report, errors.New("negative signature index")
	}
	distID, err := cString(certificateDistID)
	if err != nil {
		return report, err
	}
	signatureID, err := cString(signatureDistID)
	if err != nil {
		return report, err
	}
	if intermediatesDER == nil {
		intermediatesDER = [][]byte{}
	}
	certificatesJSON, err := json.Marshal(intermediatesDER)
	if err != nil {
		return report, fmt.Errorf("encode intermediate certificates: %w", err)
	}
	var signerPtr, anchorPtr *byte
	if len(signerCertificateDER) > 0 {
		signerPtr = &signerCertificateDER[0]
	}
	if len(anchorCertificateDER) > 0 {
		anchorPtr = &anchorCertificateDER[0]
	}
	runtime.LockOSThread()
	defer runtime.UnlockOSThread()
	var buffer nativeBuffer
	verify := api.verifyChainWithID
	if unordered {
		verify = api.verifySignaturePool
	}
	if err := nativeError(verify(doc.handle, uintptr(index), signerPtr, uintptr(len(signerCertificateDER)), &certificatesJSON[0], uintptr(len(certificatesJSON)), anchorPtr, uintptr(len(anchorCertificateDER)), atUnixSeconds, &distID[0], &signatureID[0], &buffer)); err != nil {
		return report, fmt.Errorf("verify signature with ordered SM2 chain: %w", err)
	}
	defer api.bufferFree(buffer)
	data, err := bytesAt(buffer)
	if err != nil {
		return report, err
	}
	if err := json.Unmarshal(data, &report); err != nil {
		return report, fmt.Errorf("decode signature chain verification report: %w", err)
	}
	return report, nil
}

// VerifySignatureWithOrderedSM2Chains checks both the signer and the seal
// issuer against their respective caller-trusted anchors. Signature IDs must
// be set explicitly in options; use the standard ID for ordinary OFD files.
func (doc *Document) VerifySignatureWithOrderedSM2Chains(index int, options SignatureTrustOptions) (SignatureTrustVerificationReport, error) {
	return doc.verifySignatureTrust(index, options, false)
}

// VerifySignatureWithCertificatePools searches separate unordered signer and
// seal issuer intermediate certificate pools before checking both branches.
func (doc *Document) VerifySignatureWithCertificatePools(index int, options SignatureTrustOptions) (SignatureTrustVerificationReport, error) {
	return doc.verifySignatureTrust(index, options, true)
}

func (doc *Document) verifySignatureTrust(index int, options SignatureTrustOptions, unordered bool) (SignatureTrustVerificationReport, error) {
	doc.mu.Lock()
	defer doc.mu.Unlock()
	var report SignatureTrustVerificationReport
	if err := doc.requireOpen(); err != nil {
		return report, err
	}
	if index < 0 {
		return report, errors.New("negative signature index")
	}
	if options.SignerIntermediatesDER == nil {
		options.SignerIntermediatesDER = [][]byte{}
	}
	if options.SealIntermediatesDER == nil {
		options.SealIntermediatesDER = [][]byte{}
	}
	if options.SignerCertificateDER == nil {
		options.SignerCertificateDER = []byte{}
	}
	if options.SealAnchorDER == nil {
		options.SealAnchorDER = []byte{}
	}
	payload, err := json.Marshal(options)
	if err != nil {
		return report, fmt.Errorf("encode signature trust options: %w", err)
	}
	runtime.LockOSThread()
	defer runtime.UnlockOSThread()
	var buffer nativeBuffer
	verify := api.verifyTwoChains
	if unordered {
		verify = api.verifyTwoPools
	}
	if err := nativeError(verify(doc.handle, uintptr(index), &payload[0], uintptr(len(payload)), &buffer)); err != nil {
		return report, fmt.Errorf("verify signature with ordered SM2 chains: %w", err)
	}
	defer api.bufferFree(buffer)
	data, err := bytesAt(buffer)
	if err != nil {
		return report, err
	}
	if err := json.Unmarshal(data, &report); err != nil {
		return report, fmt.Errorf("decode signature trust verification report: %w", err)
	}
	return report, nil
}

// AddDigitalSignature adds or appends a signature with a 32-byte SM2 private
// key scalar and a caller-supplied date in YYYY-MM-DD HH:MM:SS form. The
// corresponding certificate must be retained separately by the caller.
func (doc *Document) AddDigitalSignature(privateKey []byte, signatureDatetime string) (int, error) {
	return doc.AddDigitalSignatureWithDistID(privateKey, signatureDatetime, defaultSM2DistID)
}

// AddDigitalSignatureWithDistID adds or appends a raw SM2 signature using a
// supplied distinguishing ID. Retain the ID for later verification.
func (doc *Document) AddDigitalSignatureWithDistID(privateKey []byte, signatureDatetime, signatureDistID string) (int, error) {
	doc.mu.Lock()
	defer doc.mu.Unlock()
	if err := doc.requireOpen(); err != nil {
		return 0, err
	}
	date, err := cString(signatureDatetime)
	if err != nil {
		return 0, err
	}
	distID, err := cString(signatureDistID)
	if err != nil {
		return 0, err
	}
	var keyPtr *byte
	if len(privateKey) > 0 {
		keyPtr = &privateKey[0]
	}
	runtime.LockOSThread()
	defer runtime.UnlockOSThread()
	var index uintptr
	if err := nativeError(api.addDigitalWithID(doc.handle, keyPtr, uintptr(len(privateKey)), &date[0], &distID[0], &index)); err != nil {
		return 0, fmt.Errorf("add digital signature: %w", err)
	}
	return int(index), nil
}

// AddSESSignature adds an SES v4/v5 signature with an existing signed seal.
func (doc *Document) AddSESSignature(privateKey, signerCertificateDER, sealDER []byte, signatureDatetime string) (int, error) {
	return doc.AddSESSignatureWithDistIDs(privateKey, signerCertificateDER, sealDER, signatureDatetime, defaultSM2DistID, defaultSM2DistID)
}

// AddSESSignatureWithDistIDs uses separate SM2 IDs for the OFD signature and seal.
func (doc *Document) AddSESSignatureWithDistIDs(privateKey, signerCertificateDER, sealDER []byte, signatureDatetime, signatureDistID, sealDistID string) (int, error) {
	doc.mu.Lock()
	defer doc.mu.Unlock()
	if err := doc.requireOpen(); err != nil {
		return 0, err
	}
	date, err := cString(signatureDatetime)
	if err != nil {
		return 0, err
	}
	signatureID, err := cString(signatureDistID)
	if err != nil {
		return 0, err
	}
	sealID, err := cString(sealDistID)
	if err != nil {
		return 0, err
	}
	var keyPtr, certificatePtr, sealPtr *byte
	if len(privateKey) > 0 {
		keyPtr = &privateKey[0]
	}
	if len(signerCertificateDER) > 0 {
		certificatePtr = &signerCertificateDER[0]
	}
	if len(sealDER) > 0 {
		sealPtr = &sealDER[0]
	}
	runtime.LockOSThread()
	defer runtime.UnlockOSThread()
	var index uintptr
	if err := nativeError(api.addSESWithIDs(doc.handle, keyPtr, uintptr(len(privateKey)), certificatePtr, uintptr(len(signerCertificateDER)), sealPtr, uintptr(len(sealDER)), &date[0], &signatureID[0], &sealID[0], &index)); err != nil {
		return 0, fmt.Errorf("add SES signature: %w", err)
	}
	return int(index), nil
}

// AddSESSignatureAtPage places the seal image on a zero-based page. Boundary is
// x, y, width and height in millimeters.
func (doc *Document) AddSESSignatureAtPage(privateKey, signerCertificateDER, sealDER []byte, signatureDatetime string, pageIndex int, boundary [4]float64) (int, error) {
	return doc.AddSESSignatureAtPageWithDistIDs(privateKey, signerCertificateDER, sealDER, signatureDatetime, pageIndex, boundary, defaultSM2DistID, defaultSM2DistID)
}

// AddSESSignatureAtPageWithDistIDs places a seal using separate SM2 IDs.
func (doc *Document) AddSESSignatureAtPageWithDistIDs(privateKey, signerCertificateDER, sealDER []byte, signatureDatetime string, pageIndex int, boundary [4]float64, signatureDistID, sealDistID string) (int, error) {
	doc.mu.Lock()
	defer doc.mu.Unlock()
	if err := doc.requireOpen(); err != nil {
		return 0, err
	}
	if pageIndex < 0 {
		return 0, fmt.Errorf("negative page index: %d", pageIndex)
	}
	date, err := cString(signatureDatetime)
	if err != nil {
		return 0, err
	}
	signatureID, err := cString(signatureDistID)
	if err != nil {
		return 0, err
	}
	sealID, err := cString(sealDistID)
	if err != nil {
		return 0, err
	}
	var keyPtr, certificatePtr, sealPtr *byte
	if len(privateKey) > 0 {
		keyPtr = &privateKey[0]
	}
	if len(signerCertificateDER) > 0 {
		certificatePtr = &signerCertificateDER[0]
	}
	if len(sealDER) > 0 {
		sealPtr = &sealDER[0]
	}
	runtime.LockOSThread()
	defer runtime.UnlockOSThread()
	var index uintptr
	if err := nativeError(api.addSESWithStamp(doc.handle, keyPtr, uintptr(len(privateKey)), certificatePtr, uintptr(len(signerCertificateDER)), sealPtr, uintptr(len(sealDER)), &date[0], uintptr(pageIndex), boundary[0], boundary[1], boundary[2], boundary[3], &signatureID[0], &sealID[0], &index)); err != nil {
		return 0, fmt.Errorf("add positioned SES signature: %w", err)
	}
	return int(index), nil
}

// AttachmentData returns bytes of a zero-based attachment.
func (doc *Document) AttachmentData(index int) ([]byte, error) {
	doc.mu.Lock()
	defer doc.mu.Unlock()
	if err := doc.requireOpen(); err != nil {
		return nil, err
	}
	if index < 0 {
		return nil, errors.New("negative attachment index")
	}
	runtime.LockOSThread()
	defer runtime.UnlockOSThread()
	var buffer nativeBuffer
	if err := nativeError(api.attachData(doc.handle, uintptr(index), &buffer)); err != nil {
		return nil, fmt.Errorf("read attachment: %w", err)
	}
	defer api.bufferFree(buffer)
	return bytesAt(buffer)
}

// SetEntry replaces raw package bytes, including signed entries. XML entries
// are checked for well-formedness; verify signatures after replacing protected files.
func (doc *Document) SetEntry(path string, data []byte) error {
	doc.mu.Lock()
	defer doc.mu.Unlock()
	if err := doc.requireOpen(); err != nil {
		return err
	}
	pathBytes, err := cString(path)
	if err != nil {
		return err
	}
	var dataPtr *byte
	if len(data) > 0 {
		dataPtr = &data[0]
	}
	runtime.LockOSThread()
	defer runtime.UnlockOSThread()
	if err := nativeError(api.setEntry(doc.handle, &pathBytes[0], dataPtr, uintptr(len(data)))); err != nil {
		return fmt.Errorf("set entry: %w", err)
	}
	runtime.KeepAlive(data)
	return nil
}

// RemoveEntry removes a raw package entry and reports whether it existed.
// Removing a referenced or signed entry can invalidate the package.
func (doc *Document) RemoveEntry(path string) (bool, error) {
	doc.mu.Lock()
	defer doc.mu.Unlock()
	if err := doc.requireOpen(); err != nil {
		return false, err
	}
	encoded, err := cString(path)
	if err != nil {
		return false, err
	}
	runtime.LockOSThread()
	defer runtime.UnlockOSThread()
	var removed int32
	if err := nativeError(api.removeEntry(doc.handle, &encoded[0], &removed)); err != nil {
		return false, fmt.Errorf("remove entry: %w", err)
	}
	return removed == 1, nil
}

// AddTextPage appends a fixed-layout page to a newly created package.
func (doc *Document) AddTextPage(text string, widthMM, heightMM float64) error {
	doc.mu.Lock()
	defer doc.mu.Unlock()
	if err := doc.requireOpen(); err != nil {
		return err
	}
	payload, err := textRequest(map[string]any{
		"operation": "add", "text": text,
		"width_mm": widthMM, "height_mm": heightMM,
	})
	if err != nil {
		return err
	}
	runtime.LockOSThread()
	defer runtime.UnlockOSThread()
	if err := nativeError(api.writeTextPage(doc.handle, &payload[0], nil)); err != nil {
		return fmt.Errorf("add text page: %w", err)
	}
	return nil
}

// AddBlankPage appends an empty page to a newly created package.
func (doc *Document) AddBlankPage(widthMM, heightMM float64) error {
	doc.mu.Lock()
	defer doc.mu.Unlock()
	if err := doc.requireOpen(); err != nil {
		return err
	}
	runtime.LockOSThread()
	defer runtime.UnlockOSThread()
	if err := nativeError(api.addBlankPage(doc.handle, widthMM, heightMM)); err != nil {
		return fmt.Errorf("add blank page: %w", err)
	}
	return nil
}

// AddTextObject adds positioned text to a page and returns its OFD object ID.
// An empty fontID selects the first font resource in the page's document.
func (doc *Document) AddTextObject(pageIndex int, text string, boundary [4]float64, size float64, fontID string) (int, error) {
	return doc.addTextObject(pageIndex, text, boundary, size, fontID, false, "", nil)
}

// AddColoredTextObject adds positioned text with an opaque RGB fill.
func (doc *Document) AddColoredTextObject(pageIndex int, text string, boundary [4]float64, size float64, fontID string, color [3]uint8) (int, error) {
	return doc.addTextObject(pageIndex, text, boundary, size, fontID, false, "", &color)
}

// AddShapedTextObject shapes a single bidirectional line with an embedded font.
// The original text remains available for extraction.
func (doc *Document) AddShapedTextObject(pageIndex int, text string, boundary [4]float64, size float64, fontID string) (int, error) {
	return doc.addTextObject(pageIndex, text, boundary, size, fontID, true, "", nil)
}

// AddColoredShapedTextObject shapes positioned text with an opaque RGB fill.
func (doc *Document) AddColoredShapedTextObject(pageIndex int, text string, boundary [4]float64, size float64, fontID string, color [3]uint8) (int, error) {
	return doc.addTextObject(pageIndex, text, boundary, size, fontID, true, "", &color)
}

// AddShapedTextObjectWithLanguage selects language-specific OpenType forms.
func (doc *Document) AddShapedTextObjectWithLanguage(pageIndex int, text string, boundary [4]float64, size float64, fontID, language string) (int, error) {
	if language == "" {
		return 0, errors.New("empty shaping language")
	}
	return doc.addTextObject(pageIndex, text, boundary, size, fontID, true, language, nil)
}

// AddColoredShapedTextObjectWithLanguage also selects localized OpenType forms.
func (doc *Document) AddColoredShapedTextObjectWithLanguage(pageIndex int, text string, boundary [4]float64, size float64, fontID, language string, color [3]uint8) (int, error) {
	if language == "" {
		return 0, errors.New("empty shaping language")
	}
	return doc.addTextObject(pageIndex, text, boundary, size, fontID, true, language, &color)
}

func (doc *Document) addTextObject(pageIndex int, text string, boundary [4]float64, size float64, fontID string, shape bool, language string, fillColor *[3]uint8) (int, error) {
	doc.mu.Lock()
	defer doc.mu.Unlock()
	if err := doc.requireOpen(); err != nil {
		return 0, err
	}
	if pageIndex < 0 {
		return 0, errors.New("negative page index")
	}
	var selectedFont *string
	if fontID != "" {
		selectedFont = &fontID
	}
	payload, err := json.Marshal(struct {
		Text      string     `json:"text"`
		Boundary  [4]float64 `json:"boundary"`
		Size      float64    `json:"size"`
		FontID    *string    `json:"font_id,omitempty"`
		Shape     bool       `json:"shape,omitempty"`
		Language  string     `json:"language,omitempty"`
		FillColor *[3]uint8  `json:"fill_color,omitempty"`
	}{Text: text, Boundary: boundary, Size: size, FontID: selectedFont, Shape: shape, Language: language, FillColor: fillColor})
	if err != nil {
		return 0, fmt.Errorf("encode text object: %w", err)
	}
	bytes, err := cString(string(payload))
	if err != nil {
		return 0, err
	}
	runtime.LockOSThread()
	defer runtime.UnlockOSThread()
	var id uintptr
	if err := nativeError(api.addTextObject(doc.handle, uintptr(pageIndex), &bytes[0], &id)); err != nil {
		return 0, fmt.Errorf("add text object: %w", err)
	}
	return int(id), nil
}

// AddCompositeTextObject creates a document composite resource containing one text object.
// An empty fontID selects the first font resource in the page's document.
func (doc *Document) AddCompositeTextObject(pageIndex int, text string, boundary [4]float64, size float64, fontID string) (int, error) {
	doc.mu.Lock()
	defer doc.mu.Unlock()
	if err := doc.requireOpen(); err != nil {
		return 0, err
	}
	if pageIndex < 0 {
		return 0, errors.New("negative page index")
	}
	var selectedFont *string
	if fontID != "" {
		selectedFont = &fontID
	}
	payload, err := json.Marshal(struct {
		Text     string     `json:"text"`
		Boundary [4]float64 `json:"boundary"`
		Size     float64    `json:"size"`
		FontID   *string    `json:"font_id,omitempty"`
	}{Text: text, Boundary: boundary, Size: size, FontID: selectedFont})
	if err != nil {
		return 0, fmt.Errorf("encode composite text object: %w", err)
	}
	bytes, err := cString(string(payload))
	if err != nil {
		return 0, err
	}
	runtime.LockOSThread()
	defer runtime.UnlockOSThread()
	var id uintptr
	if err := nativeError(api.addCompositeTextObject(doc.handle, uintptr(pageIndex), &bytes[0], &id)); err != nil {
		return 0, fmt.Errorf("add composite text object: %w", err)
	}
	return int(id), nil
}

// DuplicatePageObject copies a page graphic object after itself and returns its new ID.
func (doc *Document) DuplicatePageObject(pageIndex int, objectID string) (int, error) {
	doc.mu.Lock()
	defer doc.mu.Unlock()
	if err := doc.requireOpen(); err != nil {
		return 0, err
	}
	if pageIndex < 0 {
		return 0, errors.New("negative page index")
	}
	idBytes, err := cString(objectID)
	if err != nil {
		return 0, err
	}
	runtime.LockOSThread()
	defer runtime.UnlockOSThread()
	var id uintptr
	if err := nativeError(api.duplicatePageObject(doc.handle, uintptr(pageIndex), &idBytes[0], &id)); err != nil {
		return 0, fmt.Errorf("duplicate page object: %w", err)
	}
	return int(id), nil
}

// RemovePageObject removes one text, path, image, or composite object by OFD ID.
func (doc *Document) RemovePageObject(pageIndex int, objectID string) error {
	doc.mu.Lock()
	defer doc.mu.Unlock()
	if err := doc.requireOpen(); err != nil {
		return err
	}
	if pageIndex < 0 {
		return errors.New("negative page index")
	}
	idBytes, err := cString(objectID)
	if err != nil {
		return err
	}
	runtime.LockOSThread()
	defer runtime.UnlockOSThread()
	if err := nativeError(api.removePageObject(doc.handle, uintptr(pageIndex), &idBytes[0])); err != nil {
		return fmt.Errorf("remove page object: %w", err)
	}
	return nil
}

func (doc *Document) colorSpaces(index int, call func(uintptr, uintptr, *nativeBuffer) int32) ([]ColorSpace, error) {
	doc.mu.Lock()
	defer doc.mu.Unlock()
	if err := doc.requireOpen(); err != nil {
		return nil, err
	}
	if index < 0 {
		return nil, errors.New("negative resource index")
	}
	runtime.LockOSThread()
	defer runtime.UnlockOSThread()
	var buffer nativeBuffer
	if err := nativeError(call(doc.handle, uintptr(index), &buffer)); err != nil {
		return nil, err
	}
	defer api.bufferFree(buffer)
	data, err := bytesAt(buffer)
	if err != nil {
		return nil, err
	}
	var spaces []ColorSpace
	if err := json.Unmarshal(data, &spaces); err != nil {
		return nil, fmt.Errorf("decode color spaces: %w", err)
	}
	return spaces, nil
}

// PageColorSpaces lists effective page color spaces in precedence order.
func (doc *Document) PageColorSpaces(pageIndex int) ([]ColorSpace, error) {
	return doc.colorSpaces(pageIndex, api.pageColorSpaces)
}

// DocumentColorSpaces lists document-level color spaces.
func (doc *Document) DocumentColorSpaces(documentIndex int) ([]ColorSpace, error) {
	return doc.colorSpaces(documentIndex, api.documentColorSpaces)
}

func (doc *Document) drawParams(index int, call func(uintptr, uintptr, *nativeBuffer) int32) ([]DrawParam, error) {
	doc.mu.Lock()
	defer doc.mu.Unlock()
	if err := doc.requireOpen(); err != nil {
		return nil, err
	}
	if index < 0 {
		return nil, errors.New("negative resource index")
	}
	runtime.LockOSThread()
	defer runtime.UnlockOSThread()
	var buffer nativeBuffer
	if err := nativeError(call(doc.handle, uintptr(index), &buffer)); err != nil {
		return nil, err
	}
	defer api.bufferFree(buffer)
	data, err := bytesAt(buffer)
	if err != nil {
		return nil, err
	}
	var params []DrawParam
	if err := json.Unmarshal(data, &params); err != nil {
		return nil, fmt.Errorf("decode drawing parameters: %w", err)
	}
	return params, nil
}

// DocumentDrawParams lists drawing parameters declared in document resources.
func (doc *Document) DocumentDrawParams(documentIndex int) ([]DrawParam, error) {
	return doc.drawParams(documentIndex, api.documentDrawParams)
}

// PageDrawParams lists drawing parameters visible on a page.
func (doc *Document) PageDrawParams(pageIndex int) ([]DrawParam, error) {
	return doc.drawParams(pageIndex, api.pageDrawParams)
}

// AddDocumentColorSpace creates a color space in public resources.
func (doc *Document) AddDocumentColorSpace(documentIndex int, spec ColorSpaceSpec) (int, error) {
	return doc.addColorSpace(documentIndex, spec, api.addDocumentColorSpace)
}

// AddPageColorSpace creates a page-local color space.
func (doc *Document) AddPageColorSpace(pageIndex int, spec ColorSpaceSpec) (int, error) {
	return doc.addColorSpace(pageIndex, spec, api.addPageColorSpace)
}

func (doc *Document) setColorSpace(index int, id string, spec ColorSpaceSpec,
	call func(uintptr, uintptr, *byte, *byte) int32) error {
	doc.mu.Lock()
	defer doc.mu.Unlock()
	if err := doc.requireOpen(); err != nil {
		return err
	}
	if index < 0 {
		return errors.New("negative resource index")
	}
	identifier, err := cString(id)
	if err != nil {
		return err
	}
	payload, err := json.Marshal(spec)
	if err != nil {
		return fmt.Errorf("encode ColorSpace: %w", err)
	}
	encoded, err := cString(string(payload))
	if err != nil {
		return err
	}
	runtime.LockOSThread()
	defer runtime.UnlockOSThread()
	if err := nativeError(call(doc.handle, uintptr(index), &identifier[0], &encoded[0])); err != nil {
		return fmt.Errorf("set ColorSpace: %w", err)
	}
	return nil
}

// SetDocumentColorSpace replaces a document ColorSpace without changing its ID.
func (doc *Document) SetDocumentColorSpace(documentIndex int, id string, spec ColorSpaceSpec) error {
	return doc.setColorSpace(documentIndex, id, spec, api.setDocumentColorSpace)
}

// SetPageColorSpace replaces a ColorSpace declared in one page's resources.
func (doc *Document) SetPageColorSpace(pageIndex int, id string, spec ColorSpaceSpec) error {
	return doc.setColorSpace(pageIndex, id, spec, api.setPageColorSpace)
}

func (doc *Document) removeColorSpace(index int, id string,
	call func(uintptr, uintptr, *byte) int32) error {
	doc.mu.Lock()
	defer doc.mu.Unlock()
	if err := doc.requireOpen(); err != nil {
		return err
	}
	if index < 0 {
		return errors.New("negative resource index")
	}
	identifier, err := cString(id)
	if err != nil {
		return err
	}
	runtime.LockOSThread()
	defer runtime.UnlockOSThread()
	if err := nativeError(call(doc.handle, uintptr(index), &identifier[0])); err != nil {
		return fmt.Errorf("remove ColorSpace: %w", err)
	}
	return nil
}

// RemoveDocumentColorSpace removes an unreferenced document ColorSpace.
func (doc *Document) RemoveDocumentColorSpace(documentIndex int, id string) error {
	return doc.removeColorSpace(documentIndex, id, api.removeDocumentColorSpace)
}

// RemovePageColorSpace removes an unreferenced page ColorSpace.
func (doc *Document) RemovePageColorSpace(pageIndex int, id string) error {
	return doc.removeColorSpace(pageIndex, id, api.removePageColorSpace)
}

func (doc *Document) addColorSpace(index int, spec ColorSpaceSpec,
	call func(uintptr, uintptr, *byte, *uintptr) int32) (int, error) {
	doc.mu.Lock()
	defer doc.mu.Unlock()
	if err := doc.requireOpen(); err != nil {
		return 0, err
	}
	if index < 0 {
		return 0, errors.New("negative resource index")
	}
	payload, err := json.Marshal(spec)
	if err != nil {
		return 0, fmt.Errorf("encode ColorSpace: %w", err)
	}
	encoded, err := cString(string(payload))
	if err != nil {
		return 0, err
	}
	runtime.LockOSThread()
	defer runtime.UnlockOSThread()
	var id uintptr
	if err := nativeError(call(doc.handle, uintptr(index), &encoded[0], &id)); err != nil {
		return 0, fmt.Errorf("add ColorSpace: %w", err)
	}
	return int(id), nil
}

// DocumentDefaultColorSpace returns the document's DefaultCS ID, or nil.
func (doc *Document) DocumentDefaultColorSpace(documentIndex int) (*string, error) {
	doc.mu.Lock()
	defer doc.mu.Unlock()
	if err := doc.requireOpen(); err != nil {
		return nil, err
	}
	if documentIndex < 0 {
		return nil, errors.New("negative document index")
	}
	runtime.LockOSThread()
	defer runtime.UnlockOSThread()
	var buffer nativeBuffer
	if err := nativeError(api.documentDefaultColorSpace(doc.handle, uintptr(documentIndex), &buffer)); err != nil {
		return nil, fmt.Errorf("read DefaultCS: %w", err)
	}
	defer api.bufferFree(buffer)
	data, err := bytesAt(buffer)
	if err != nil {
		return nil, err
	}
	var id *string
	if err := json.Unmarshal(data, &id); err != nil {
		return nil, fmt.Errorf("decode DefaultCS: %w", err)
	}
	return id, nil
}

// SetDocumentDefaultColorSpace sets DefaultCS; an empty ID clears it.
func (doc *Document) SetDocumentDefaultColorSpace(documentIndex int, colorSpaceID string) error {
	doc.mu.Lock()
	defer doc.mu.Unlock()
	if err := doc.requireOpen(); err != nil {
		return err
	}
	if documentIndex < 0 {
		return errors.New("negative document index")
	}
	var ptr *byte
	if colorSpaceID != "" {
		encoded, err := cString(colorSpaceID)
		if err != nil {
			return err
		}
		ptr = &encoded[0]
	}
	runtime.LockOSThread()
	defer runtime.UnlockOSThread()
	if err := nativeError(api.setDocumentDefaultColorSpace(doc.handle, uintptr(documentIndex), ptr)); err != nil {
		return fmt.Errorf("set DefaultCS: %w", err)
	}
	return nil
}

// AddDocumentDrawParam creates a reusable drawing parameter in public resources.
func (doc *Document) AddDocumentDrawParam(documentIndex int, style DrawParamStyle) (int, error) {
	doc.mu.Lock()
	defer doc.mu.Unlock()
	if err := doc.requireOpen(); err != nil {
		return 0, err
	}
	if documentIndex < 0 {
		return 0, errors.New("negative document index")
	}
	payload, err := json.Marshal(style)
	if err != nil {
		return 0, fmt.Errorf("encode DrawParam: %w", err)
	}
	encoded, err := cString(string(payload))
	if err != nil {
		return 0, err
	}
	runtime.LockOSThread()
	defer runtime.UnlockOSThread()
	var id uintptr
	if err := nativeError(api.addDocumentDrawParam(doc.handle, uintptr(documentIndex), &encoded[0], &id)); err != nil {
		return 0, fmt.Errorf("add document DrawParam: %w", err)
	}
	return int(id), nil
}

// AddPageDrawParam creates a drawing parameter visible through one page's resources.
func (doc *Document) AddPageDrawParam(pageIndex int, style DrawParamStyle) (int, error) {
	doc.mu.Lock()
	defer doc.mu.Unlock()
	if err := doc.requireOpen(); err != nil {
		return 0, err
	}
	if pageIndex < 0 {
		return 0, errors.New("negative page index")
	}
	payload, err := json.Marshal(style)
	if err != nil {
		return 0, fmt.Errorf("encode DrawParam: %w", err)
	}
	encoded, err := cString(string(payload))
	if err != nil {
		return 0, err
	}
	runtime.LockOSThread()
	defer runtime.UnlockOSThread()
	var id uintptr
	if err := nativeError(api.addPageDrawParam(doc.handle, uintptr(pageIndex), &encoded[0], &id)); err != nil {
		return 0, fmt.Errorf("add page DrawParam: %w", err)
	}
	return int(id), nil
}

func (doc *Document) setDrawParam(index int, id string, style DrawParamStyle,
	call func(uintptr, uintptr, *byte, *byte) int32) error {
	doc.mu.Lock()
	defer doc.mu.Unlock()
	if err := doc.requireOpen(); err != nil {
		return err
	}
	if index < 0 {
		return errors.New("negative resource index")
	}
	identifier, err := cString(id)
	if err != nil {
		return err
	}
	payload, err := json.Marshal(style)
	if err != nil {
		return fmt.Errorf("encode DrawParam: %w", err)
	}
	encoded, err := cString(string(payload))
	if err != nil {
		return err
	}
	runtime.LockOSThread()
	defer runtime.UnlockOSThread()
	if err := nativeError(call(doc.handle, uintptr(index), &identifier[0], &encoded[0])); err != nil {
		return fmt.Errorf("set DrawParam: %w", err)
	}
	return nil
}

// SetDocumentDrawParam replaces a document DrawParam without changing its ID.
func (doc *Document) SetDocumentDrawParam(documentIndex int, id string, style DrawParamStyle) error {
	return doc.setDrawParam(documentIndex, id, style, api.setDocumentDrawParam)
}

// SetPageDrawParam replaces a DrawParam declared in a page's resources.
func (doc *Document) SetPageDrawParam(pageIndex int, id string, style DrawParamStyle) error {
	return doc.setDrawParam(pageIndex, id, style, api.setPageDrawParam)
}

func (doc *Document) removeDrawParam(index int, id string,
	call func(uintptr, uintptr, *byte) int32) error {
	doc.mu.Lock()
	defer doc.mu.Unlock()
	if err := doc.requireOpen(); err != nil {
		return err
	}
	if index < 0 {
		return errors.New("negative resource index")
	}
	identifier, err := cString(id)
	if err != nil {
		return err
	}
	runtime.LockOSThread()
	defer runtime.UnlockOSThread()
	if err := nativeError(call(doc.handle, uintptr(index), &identifier[0])); err != nil {
		return fmt.Errorf("remove DrawParam: %w", err)
	}
	return nil
}

// RemoveDocumentDrawParam removes an unreferenced document resource DrawParam.
func (doc *Document) RemoveDocumentDrawParam(documentIndex int, id string) error {
	return doc.removeDrawParam(documentIndex, id, api.removeDocumentDrawParam)
}

// RemovePageDrawParam removes an unreferenced page resource DrawParam.
func (doc *Document) RemovePageDrawParam(pageIndex int, id string) error {
	return doc.removeDrawParam(pageIndex, id, api.removePageDrawParam)
}

// SetPageObjectDrawParam sets a direct page object's DrawParam ID; empty clears it.
func (doc *Document) SetPageObjectDrawParam(pageIndex int, objectID, drawParam string) error {
	doc.mu.Lock()
	defer doc.mu.Unlock()
	if err := doc.requireOpen(); err != nil {
		return err
	}
	if pageIndex < 0 {
		return errors.New("negative page index")
	}
	id, err := cString(objectID)
	if err != nil {
		return err
	}
	var reference *byte
	if drawParam != "" {
		value, err := cString(drawParam)
		if err != nil {
			return err
		}
		reference = &value[0]
	}
	runtime.LockOSThread()
	defer runtime.UnlockOSThread()
	if err := nativeError(api.setPageObjectDrawParam(doc.handle, uintptr(pageIndex), &id[0], reference)); err != nil {
		return fmt.Errorf("set page object DrawParam: %w", err)
	}
	return nil
}

// AddPageLayer creates a Background, Body, or Foreground layer and returns its ID.
func (doc *Document) AddPageLayer(pageIndex int, layerType string, drawParam string) (int, error) {
	doc.mu.Lock()
	defer doc.mu.Unlock()
	if err := doc.requireOpen(); err != nil {
		return 0, err
	}
	if pageIndex < 0 {
		return 0, errors.New("negative page index")
	}
	kind, err := cString(layerType)
	if err != nil {
		return 0, err
	}
	var reference *byte
	if drawParam != "" {
		value, err := cString(drawParam)
		if err != nil {
			return 0, err
		}
		reference = &value[0]
	}
	runtime.LockOSThread()
	defer runtime.UnlockOSThread()
	var id uintptr
	if err := nativeError(api.addPageLayer(doc.handle, uintptr(pageIndex), &kind[0], reference, &id)); err != nil {
		return 0, fmt.Errorf("add page layer: %w", err)
	}
	return int(id), nil
}

// SetPageLayerType changes a direct page layer's Background, Body, or Foreground type.
func (doc *Document) SetPageLayerType(pageIndex int, layerID, layerType string) error {
	doc.mu.Lock()
	defer doc.mu.Unlock()
	if err := doc.requireOpen(); err != nil {
		return err
	}
	if pageIndex < 0 {
		return errors.New("negative page index")
	}
	id, err := cString(layerID)
	if err != nil {
		return err
	}
	kind, err := cString(layerType)
	if err != nil {
		return err
	}
	runtime.LockOSThread()
	defer runtime.UnlockOSThread()
	if err := nativeError(api.setPageLayerType(doc.handle, uintptr(pageIndex), &id[0], &kind[0])); err != nil {
		return fmt.Errorf("set page layer type: %w", err)
	}
	return nil
}

// SetPageLayerName sets or clears a direct page layer's optional name.
// An empty name clears the attribute.
func (doc *Document) SetPageLayerName(pageIndex int, layerID, name string) error {
	doc.mu.Lock()
	defer doc.mu.Unlock()
	if err := doc.requireOpen(); err != nil {
		return err
	}
	if pageIndex < 0 {
		return errors.New("negative page index")
	}
	id, err := cString(layerID)
	if err != nil {
		return err
	}
	var value *byte
	if name != "" {
		encoded, err := cString(name)
		if err != nil {
			return err
		}
		value = &encoded[0]
	}
	runtime.LockOSThread()
	defer runtime.UnlockOSThread()
	if err := nativeError(api.setPageLayerName(doc.handle, uintptr(pageIndex), &id[0], value)); err != nil {
		return fmt.Errorf("set page layer name: %w", err)
	}
	return nil
}

// SetPageLayerDrawParam sets or clears a direct page layer's DrawParam reference.
// An empty drawParam clears the reference.
func (doc *Document) SetPageLayerDrawParam(pageIndex int, layerID, drawParam string) error {
	doc.mu.Lock()
	defer doc.mu.Unlock()
	if err := doc.requireOpen(); err != nil {
		return err
	}
	if pageIndex < 0 {
		return errors.New("negative page index")
	}
	id, err := cString(layerID)
	if err != nil {
		return err
	}
	var reference *byte
	if drawParam != "" {
		value, err := cString(drawParam)
		if err != nil {
			return err
		}
		reference = &value[0]
	}
	runtime.LockOSThread()
	defer runtime.UnlockOSThread()
	if err := nativeError(api.setPageLayerDrawParam(doc.handle, uintptr(pageIndex), &id[0], reference)); err != nil {
		return fmt.Errorf("set page layer DrawParam: %w", err)
	}
	return nil
}

// MovePageObjectToLayer moves a graphic object to a layer on the same page.
func (doc *Document) MovePageObjectToLayer(pageIndex int, objectID, layerID string) error {
	doc.mu.Lock()
	defer doc.mu.Unlock()
	if err := doc.requireOpen(); err != nil {
		return err
	}
	if pageIndex < 0 {
		return errors.New("negative page index")
	}
	object, err := cString(objectID)
	if err != nil {
		return err
	}
	layer, err := cString(layerID)
	if err != nil {
		return err
	}
	runtime.LockOSThread()
	defer runtime.UnlockOSThread()
	if err := nativeError(api.movePageObjectToLayer(doc.handle, uintptr(pageIndex), &object[0], &layer[0])); err != nil {
		return fmt.Errorf("move page object to layer: %w", err)
	}
	return nil
}

// GroupPageObjects wraps an inclusive range of ordered sibling graphic objects
// in a PageBlock. The objects keep their IDs and drawing order.
func (doc *Document) GroupPageObjects(pageIndex int, firstID, lastID string) error {
	doc.mu.Lock()
	defer doc.mu.Unlock()
	if err := doc.requireOpen(); err != nil {
		return err
	}
	if pageIndex < 0 {
		return errors.New("negative page index")
	}
	first, err := cString(firstID)
	if err != nil {
		return err
	}
	last, err := cString(lastID)
	if err != nil {
		return err
	}
	runtime.LockOSThread()
	defer runtime.UnlockOSThread()
	if err := nativeError(api.groupPageObjects(doc.handle, uintptr(pageIndex), &first[0], &last[0])); err != nil {
		return fmt.Errorf("group page objects: %w", err)
	}
	return nil
}

// UngroupPageObjects removes the PageBlock directly containing objectID and
// lifts all of that block's children into its parent in their original order.
func (doc *Document) UngroupPageObjects(pageIndex int, objectID string) error {
	doc.mu.Lock()
	defer doc.mu.Unlock()
	if err := doc.requireOpen(); err != nil {
		return err
	}
	if pageIndex < 0 {
		return errors.New("negative page index")
	}
	id, err := cString(objectID)
	if err != nil {
		return err
	}
	runtime.LockOSThread()
	defer runtime.UnlockOSThread()
	if err := nativeError(api.ungroupPageObjects(doc.handle, uintptr(pageIndex), &id[0])); err != nil {
		return fmt.Errorf("ungroup page objects: %w", err)
	}
	return nil
}

// MovePageObject moves an object before beforeID, or to the end of its layer
// or PageBlock when beforeID is empty. A target in another container moves
// the object there.
func (doc *Document) MovePageObject(pageIndex int, objectID, beforeID string) error {
	doc.mu.Lock()
	defer doc.mu.Unlock()
	if err := doc.requireOpen(); err != nil {
		return err
	}
	if pageIndex < 0 {
		return errors.New("negative page index")
	}
	object, err := cString(objectID)
	if err != nil {
		return err
	}
	var before []byte
	var beforePtr *byte
	if beforeID != "" {
		before, err = cString(beforeID)
		if err != nil {
			return err
		}
		beforePtr = &before[0]
	}
	runtime.LockOSThread()
	defer runtime.UnlockOSThread()
	if err := nativeError(api.movePageObject(doc.handle, uintptr(pageIndex), &object[0], beforePtr)); err != nil {
		return fmt.Errorf("move page object: %w", err)
	}
	return nil
}

// SetPageObjectName sets or clears a direct page object's optional name.
func (doc *Document) SetPageObjectName(pageIndex int, objectID string, name *string) error {
	doc.mu.Lock()
	defer doc.mu.Unlock()
	if err := doc.requireOpen(); err != nil {
		return err
	}
	if pageIndex < 0 {
		return errors.New("negative page index")
	}
	id, err := cString(objectID)
	if err != nil {
		return err
	}
	var value *byte
	if name != nil {
		encoded, err := cString(*name)
		if err != nil {
			return err
		}
		value = &encoded[0]
	}
	runtime.LockOSThread()
	defer runtime.UnlockOSThread()
	if err := nativeError(api.setPageObjectName(doc.handle, uintptr(pageIndex), &id[0], value)); err != nil {
		return fmt.Errorf("set page object name: %w", err)
	}
	return nil
}

// SetPageObjectBlendMode sets or clears a direct page object's compositing mode.
func (doc *Document) SetPageObjectBlendMode(pageIndex int, objectID string, blendMode *string) error {
	doc.mu.Lock()
	defer doc.mu.Unlock()
	if err := doc.requireOpen(); err != nil {
		return err
	}
	if pageIndex < 0 {
		return errors.New("negative page index")
	}
	id, err := cString(objectID)
	if err != nil {
		return err
	}
	var value *byte
	if blendMode != nil {
		encoded, err := cString(*blendMode)
		if err != nil {
			return err
		}
		value = &encoded[0]
	}
	runtime.LockOSThread()
	defer runtime.UnlockOSThread()
	if err := nativeError(api.setPageObjectBlendMode(doc.handle, uintptr(pageIndex), &id[0], value)); err != nil {
		return fmt.Errorf("set page object BlendMode: %w", err)
	}
	return nil
}

// SetPageObjectClipRects replaces object-local rectangular clips.
// Each rectangle is a separate clip, so only their intersection is visible.
// An empty slice clears existing clips.
func (doc *Document) SetPageObjectClipRects(pageIndex int, objectID string, rects [][4]float64, followsCTM bool) error {
	doc.mu.Lock()
	defer doc.mu.Unlock()
	if err := doc.requireOpen(); err != nil {
		return err
	}
	if pageIndex < 0 {
		return errors.New("negative page index")
	}
	id, err := cString(objectID)
	if err != nil {
		return err
	}
	if rects == nil {
		rects = [][4]float64{}
	}
	payload, err := json.Marshal(struct {
		Rects      [][4]float64 `json:"rects"`
		FollowsCTM bool         `json:"follows_ctm"`
	}{Rects: rects, FollowsCTM: followsCTM})
	if err != nil {
		return err
	}
	request, err := cString(string(payload))
	if err != nil {
		return err
	}
	runtime.LockOSThread()
	defer runtime.UnlockOSThread()
	if err := nativeError(api.setPageObjectClipRects(doc.handle, uintptr(pageIndex), &id[0], &request[0])); err != nil {
		return fmt.Errorf("set page object clip rectangles: %w", err)
	}
	return nil
}

// SetPageObjectPathClips replaces a page object's path clips.
// Areas within each clip unite; clips intersect. An empty slice clears them.
func (doc *Document) SetPageObjectPathClips(pageIndex int, objectID string, clips [][]PathClipArea, followsCTM bool) error {
	doc.mu.Lock()
	defer doc.mu.Unlock()
	if err := doc.requireOpen(); err != nil {
		return err
	}
	if pageIndex < 0 {
		return errors.New("negative page index")
	}
	id, err := cString(objectID)
	if err != nil {
		return err
	}
	if clips == nil {
		clips = [][]PathClipArea{}
	}
	payload, err := json.Marshal(struct {
		Clips      [][]PathClipArea `json:"clips"`
		FollowsCTM bool             `json:"follows_ctm"`
	}{Clips: clips, FollowsCTM: followsCTM})
	if err != nil {
		return err
	}
	request, err := cString(string(payload))
	if err != nil {
		return err
	}
	runtime.LockOSThread()
	defer runtime.UnlockOSThread()
	if err := nativeError(api.setPageObjectPathClips(doc.handle, uintptr(pageIndex), &id[0], &request[0])); err != nil {
		return fmt.Errorf("set page object path clips: %w", err)
	}
	return nil
}

// SetPageObjectClips replaces a page object's path and text clips.
// Areas inside a clip unite; clips intersect. An empty slice clears them.
func (doc *Document) SetPageObjectClips(pageIndex int, objectID string, clips [][]ClipAreaSpec, followsCTM bool) error {
	doc.mu.Lock()
	defer doc.mu.Unlock()
	if err := doc.requireOpen(); err != nil {
		return err
	}
	if pageIndex < 0 {
		return errors.New("negative page index")
	}
	id, err := cString(objectID)
	if err != nil {
		return err
	}
	if clips == nil {
		clips = [][]ClipAreaSpec{}
	}
	payload, err := json.Marshal(struct {
		Clips      [][]ClipAreaSpec `json:"clips"`
		FollowsCTM bool             `json:"follows_ctm"`
	}{Clips: clips, FollowsCTM: followsCTM})
	if err != nil {
		return err
	}
	request, err := cString(string(payload))
	if err != nil {
		return err
	}
	runtime.LockOSThread()
	defer runtime.UnlockOSThread()
	if err := nativeError(api.setPageObjectClips(doc.handle, uintptr(pageIndex), &id[0], &request[0])); err != nil {
		return fmt.Errorf("set page object clips: %w", err)
	}
	return nil
}

// SetPageObjectVisible controls an object's participation in rendering and text extraction.
func (doc *Document) SetPageObjectVisible(pageIndex int, objectID string, visible bool) error {
	doc.mu.Lock()
	defer doc.mu.Unlock()
	if err := doc.requireOpen(); err != nil {
		return err
	}
	if pageIndex < 0 {
		return errors.New("negative page index")
	}
	id, err := cString(objectID)
	if err != nil {
		return err
	}
	var value int32
	if visible {
		value = 1
	}
	runtime.LockOSThread()
	defer runtime.UnlockOSThread()
	if err := nativeError(api.setPageObjectVisible(doc.handle, uintptr(pageIndex), &id[0], value)); err != nil {
		return fmt.Errorf("set page object visible: %w", err)
	}
	return nil
}

// SetPageObjectAlpha sets whole-object opacity from 0 to 255.
func (doc *Document) SetPageObjectAlpha(pageIndex int, objectID string, alpha int) error {
	doc.mu.Lock()
	defer doc.mu.Unlock()
	if err := doc.requireOpen(); err != nil {
		return err
	}
	if pageIndex < 0 {
		return errors.New("negative page index")
	}
	if alpha < 0 || alpha > 255 {
		return errors.New("alpha must be 0..255")
	}
	id, err := cString(objectID)
	if err != nil {
		return err
	}
	runtime.LockOSThread()
	defer runtime.UnlockOSThread()
	if err := nativeError(api.setPageObjectAlpha(doc.handle, uintptr(pageIndex), &id[0], int32(alpha))); err != nil {
		return fmt.Errorf("set page object alpha: %w", err)
	}
	return nil
}

// PathStrokeStyle contains direct geometry to write on a path object.
// Omitted optional fields clear direct values so DrawParam inheritance can apply.
type PathStrokeStyle struct {
	LineWidth    float64     `json:"line_width"`
	Cap          string      `json:"cap,omitempty"`
	Join         string      `json:"join,omitempty"`
	MiterLimit   *float64    `json:"miter_limit,omitempty"`
	DashPattern  *[2]float64 `json:"dash_pattern,omitempty"`
	DashSequence []float64   `json:"dash_sequence"`
	DashOffset   *float64    `json:"dash_offset,omitempty"`
}

// SetPagePathStrokeStyle atomically replaces direct path stroke geometry.
func (doc *Document) SetPagePathStrokeStyle(pageIndex int, objectID string, style PathStrokeStyle) error {
	doc.mu.Lock()
	defer doc.mu.Unlock()
	if err := doc.requireOpen(); err != nil {
		return err
	}
	if pageIndex < 0 {
		return errors.New("negative page index")
	}
	id, err := cString(objectID)
	if err != nil {
		return err
	}
	data, err := json.Marshal(style)
	if err != nil {
		return fmt.Errorf("encode path stroke style: %w", err)
	}
	payload, err := cString(string(data))
	if err != nil {
		return err
	}
	runtime.LockOSThread()
	defer runtime.UnlockOSThread()
	if err := nativeError(api.setPagePathStrokeStyle(doc.handle, uintptr(pageIndex), &id[0], &payload[0])); err != nil {
		return fmt.Errorf("set page path stroke style: %w", err)
	}
	return nil
}

// SetPagePathDashSequence sets 1..256 positive dash lengths and an optional offset.
// A nil sequence clears both direct attributes and restores inherited values.
func (doc *Document) SetPagePathDashSequence(pageIndex int, objectID string, sequence []float64, offset *float64) error {
	doc.mu.Lock()
	defer doc.mu.Unlock()
	if err := doc.requireOpen(); err != nil {
		return err
	}
	if pageIndex < 0 {
		return errors.New("negative page index")
	}
	id, err := cString(objectID)
	if err != nil {
		return err
	}
	data, err := json.Marshal(struct {
		DashSequence []float64 `json:"dash_sequence"`
		DashOffset   *float64  `json:"dash_offset"`
	}{sequence, offset})
	if err != nil {
		return fmt.Errorf("encode path dash sequence: %w", err)
	}
	payload, err := cString(string(data))
	if err != nil {
		return err
	}
	runtime.LockOSThread()
	defer runtime.UnlockOSThread()
	if err := nativeError(api.setPagePathDashSequence(doc.handle, uintptr(pageIndex), &id[0], &payload[0])); err != nil {
		return fmt.Errorf("set page path dash sequence: %w", err)
	}
	return nil
}

// SetPagePathData replaces one direct page path's abbreviated OFD commands.
func (doc *Document) SetPagePathData(pageIndex int, objectID, commands string) error {
	doc.mu.Lock()
	defer doc.mu.Unlock()
	if err := doc.requireOpen(); err != nil {
		return err
	}
	if pageIndex < 0 {
		return errors.New("negative page index")
	}
	id, err := cString(objectID)
	if err != nil {
		return err
	}
	data, err := cString(commands)
	if err != nil {
		return err
	}
	runtime.LockOSThread()
	defer runtime.UnlockOSThread()
	if err := nativeError(api.setPagePathData(doc.handle, uintptr(pageIndex), &id[0], &data[0])); err != nil {
		return fmt.Errorf("set page path data: %w", err)
	}
	return nil
}

// TextTypographyStyle contains direct typography to write on a text object.
// Omitted optional fields restore their OFD defaults.
type TextTypographyStyle struct {
	Size          float64  `json:"size"`
	HScale        *float64 `json:"h_scale,omitempty"`
	ReadDirection *uint16  `json:"read_direction,omitempty"`
	CharDirection *uint16  `json:"char_direction,omitempty"`
	Weight        *uint16  `json:"weight,omitempty"`
	Italic        *bool    `json:"italic,omitempty"`
}

// SetPageTextTypography atomically replaces direct text typography.
func (doc *Document) SetPageTextTypography(pageIndex int, objectID string, style TextTypographyStyle) error {
	doc.mu.Lock()
	defer doc.mu.Unlock()
	if err := doc.requireOpen(); err != nil {
		return err
	}
	if pageIndex < 0 {
		return errors.New("negative page index")
	}
	id, err := cString(objectID)
	if err != nil {
		return err
	}
	data, err := json.Marshal(style)
	if err != nil {
		return fmt.Errorf("encode text typography: %w", err)
	}
	payload, err := cString(string(data))
	if err != nil {
		return err
	}
	runtime.LockOSThread()
	defer runtime.UnlockOSThread()
	if err := nativeError(api.setPageTextTypography(doc.handle, uintptr(pageIndex), &id[0], &payload[0])); err != nil {
		return fmt.Errorf("set page text typography: %w", err)
	}
	return nil
}

// SetPageTextCode replaces one TextCode run on an unshaped text object.
func (doc *Document) SetPageTextCode(pageIndex int, objectID string, codeIndex int, text string) error {
	doc.mu.Lock()
	defer doc.mu.Unlock()
	if err := doc.requireOpen(); err != nil {
		return err
	}
	if pageIndex < 0 || codeIndex < 0 {
		return errors.New("negative page or TextCode index")
	}
	payload, err := textRequest(map[string]any{
		"operation": "set", "page_index": pageIndex,
		"object_id": objectID, "code_index": codeIndex,
		"text": text,
	})
	if err != nil {
		return err
	}
	runtime.LockOSThread()
	defer runtime.UnlockOSThread()
	if err := nativeError(api.editTextCode(doc.handle, &payload[0])); err != nil {
		return fmt.Errorf("set page TextCode: %w", err)
	}
	return nil
}

// TextCodeLayout describes direct coordinates and per-character advances.
// Nil optional fields clear the corresponding XML attribute.
type TextCodeLayout struct {
	X      *float64  `json:"x"`
	Y      *float64  `json:"y"`
	DeltaX []float64 `json:"delta_x"`
	DeltaY []float64 `json:"delta_y"`
}

// SetPageTextCodeLayout updates one unshaped TextCode run's positioning.
func (doc *Document) SetPageTextCodeLayout(pageIndex int, objectID string, codeIndex int, layout TextCodeLayout) error {
	doc.mu.Lock()
	defer doc.mu.Unlock()
	if err := doc.requireOpen(); err != nil {
		return err
	}
	if pageIndex < 0 || codeIndex < 0 {
		return errors.New("negative page or TextCode index")
	}
	id, err := cString(objectID)
	if err != nil {
		return err
	}
	data, err := json.Marshal(layout)
	if err != nil {
		return fmt.Errorf("encode TextCode layout: %w", err)
	}
	payload, err := cString(string(data))
	if err != nil {
		return err
	}
	runtime.LockOSThread()
	defer runtime.UnlockOSThread()
	if err := nativeError(api.setPageTextCodeLayout(doc.handle, uintptr(pageIndex), &id[0], uintptr(codeIndex), &payload[0])); err != nil {
		return fmt.Errorf("set page TextCode layout: %w", err)
	}
	return nil
}

// AppendPageTextCode adds a positioned run to one direct, unshaped text object.
func (doc *Document) AppendPageTextCode(pageIndex int, objectID, text string, layout TextCodeLayout) error {
	doc.mu.Lock()
	defer doc.mu.Unlock()
	if err := doc.requireOpen(); err != nil {
		return err
	}
	if pageIndex < 0 {
		return errors.New("negative page index")
	}
	payload, err := textRequest(map[string]any{
		"operation": "append", "page_index": pageIndex,
		"object_id": objectID, "text": text, "layout": layout,
	})
	if err != nil {
		return err
	}
	runtime.LockOSThread()
	defer runtime.UnlockOSThread()
	if err := nativeError(api.editTextCode(doc.handle, &payload[0])); err != nil {
		return fmt.Errorf("append page TextCode: %w", err)
	}
	return nil
}

// RemovePageTextCode removes a run while retaining the next run's position.
func (doc *Document) RemovePageTextCode(pageIndex int, objectID string, codeIndex int) error {
	doc.mu.Lock()
	defer doc.mu.Unlock()
	if err := doc.requireOpen(); err != nil {
		return err
	}
	if pageIndex < 0 || codeIndex < 0 {
		return errors.New("negative page or TextCode index")
	}
	id, err := cString(objectID)
	if err != nil {
		return err
	}
	runtime.LockOSThread()
	defer runtime.UnlockOSThread()
	if err := nativeError(api.removePageTextCode(doc.handle, uintptr(pageIndex), &id[0], uintptr(codeIndex))); err != nil {
		return fmt.Errorf("remove page TextCode: %w", err)
	}
	return nil
}

// SetPageTextFont selects a font resource available to the page for one text object.
func (doc *Document) SetPageTextFont(pageIndex int, objectID, fontID string) error {
	doc.mu.Lock()
	defer doc.mu.Unlock()
	if err := doc.requireOpen(); err != nil {
		return err
	}
	if pageIndex < 0 {
		return errors.New("negative page index")
	}
	id, err := cString(objectID)
	if err != nil {
		return err
	}
	font, err := cString(fontID)
	if err != nil {
		return err
	}
	runtime.LockOSThread()
	defer runtime.UnlockOSThread()
	if err := nativeError(api.setPageTextFont(doc.handle, uintptr(pageIndex), &id[0], &font[0])); err != nil {
		return fmt.Errorf("set page text font: %w", err)
	}
	return nil
}

// TextPaintFlags controls text fill and stroke. Nil restores the OFD default.
type TextPaintFlags struct {
	Fill   *bool `json:"fill"`
	Stroke *bool `json:"stroke"`
}

// SetPageTextPaintFlags updates both direct text paint switches atomically.
func (doc *Document) SetPageTextPaintFlags(pageIndex int, objectID string, flags TextPaintFlags) error {
	doc.mu.Lock()
	defer doc.mu.Unlock()
	if err := doc.requireOpen(); err != nil {
		return err
	}
	if pageIndex < 0 {
		return errors.New("negative page index")
	}
	id, err := cString(objectID)
	if err != nil {
		return err
	}
	data, err := json.Marshal(flags)
	if err != nil {
		return fmt.Errorf("encode text paint flags: %w", err)
	}
	payload, err := cString(string(data))
	if err != nil {
		return err
	}
	runtime.LockOSThread()
	defer runtime.UnlockOSThread()
	if err := nativeError(api.setPageTextPaintFlags(doc.handle, uintptr(pageIndex), &id[0], &payload[0])); err != nil {
		return fmt.Errorf("set page text paint flags: %w", err)
	}
	return nil
}

// SetPageImageMask sets an image's mask resource, or clears it when maskResourceID is nil.
func (doc *Document) SetPageImageMask(pageIndex int, objectID string, maskResourceID *string) error {
	doc.mu.Lock()
	defer doc.mu.Unlock()
	if err := doc.requireOpen(); err != nil {
		return err
	}
	if pageIndex < 0 {
		return errors.New("negative page index")
	}
	id, err := cString(objectID)
	if err != nil {
		return err
	}
	var mask []byte
	var maskPtr *byte
	if maskResourceID != nil {
		mask, err = cString(*maskResourceID)
		if err != nil {
			return err
		}
		maskPtr = &mask[0]
	}
	runtime.LockOSThread()
	defer runtime.UnlockOSThread()
	if err := nativeError(api.setPageImageMask(doc.handle, uintptr(pageIndex), &id[0], maskPtr)); err != nil {
		return fmt.Errorf("set page image mask: %w", err)
	}
	return nil
}

// SetPageImageSubstitution sets an alternative image resource, or clears it when nil.
func (doc *Document) SetPageImageSubstitution(pageIndex int, objectID string, substitutionResourceID *string) error {
	doc.mu.Lock()
	defer doc.mu.Unlock()
	if err := doc.requireOpen(); err != nil {
		return err
	}
	if pageIndex < 0 {
		return errors.New("negative page index")
	}
	id, err := cString(objectID)
	if err != nil {
		return err
	}
	var substitution []byte
	var substitutionPtr *byte
	if substitutionResourceID != nil {
		substitution, err = cString(*substitutionResourceID)
		if err != nil {
			return err
		}
		substitutionPtr = &substitution[0]
	}
	runtime.LockOSThread()
	defer runtime.UnlockOSThread()
	if err := nativeError(api.setPageImageSubstitution(doc.handle, uintptr(pageIndex), &id[0], substitutionPtr)); err != nil {
		return fmt.Errorf("set page image substitution: %w", err)
	}
	return nil
}

// SetPageImageBorder replaces an image border, or removes it when style is nil.
func (doc *Document) SetPageImageBorder(pageIndex int, objectID string, style *ImageBorderStyle) error {
	doc.mu.Lock()
	defer doc.mu.Unlock()
	if err := doc.requireOpen(); err != nil {
		return err
	}
	if pageIndex < 0 {
		return errors.New("negative page index")
	}
	id, err := cString(objectID)
	if err != nil {
		return err
	}
	payload, err := json.Marshal(style)
	if err != nil {
		return fmt.Errorf("encode image border: %w", err)
	}
	encoded, err := cString(string(payload))
	if err != nil {
		return err
	}
	runtime.LockOSThread()
	defer runtime.UnlockOSThread()
	if err := nativeError(api.setPageImageBorder(doc.handle, uintptr(pageIndex), &id[0], &encoded[0])); err != nil {
		return fmt.Errorf("set page image border: %w", err)
	}
	return nil
}

// SetPageObjectGouraudFill replaces a path or text fill with a Gouraud mesh.
func (doc *Document) SetPageObjectGouraudFill(pageIndex int, objectID string, style GouraudFillStyle) error {
	return doc.setPageObjectPaint(pageIndex, objectID, style, "Gouraud fill", api.setPageObjectGouraudFill)
}

// SetPageObjectGouraudStroke replaces a path or text stroke with a Gouraud mesh.
func (doc *Document) SetPageObjectGouraudStroke(pageIndex int, objectID string, style GouraudFillStyle) error {
	return doc.setPageObjectPaint(pageIndex, objectID, style, "Gouraud stroke", api.setPageObjectGouraudStroke)
}

// SetPageObjectGradientFill replaces a path or text fill with axial or radial shading.
func (doc *Document) SetPageObjectGradientFill(pageIndex int, objectID string, style GradientFillStyle) error {
	return doc.setPageObjectPaint(pageIndex, objectID, style, "gradient fill", api.setPageObjectGradientFill)
}

// SetPageObjectGradientStroke replaces a path or text stroke with axial or radial shading.
func (doc *Document) SetPageObjectGradientStroke(pageIndex int, objectID string, style GradientFillStyle) error {
	return doc.setPageObjectPaint(pageIndex, objectID, style, "gradient stroke", api.setPageObjectGradientStroke)
}

// SetPageObjectPatternFill replaces a path or text fill with a repeated pattern.
func (doc *Document) SetPageObjectPatternFill(pageIndex int, objectID string, style PatternStyle) error {
	return doc.setPageObjectPaint(pageIndex, objectID, style, "pattern fill", api.setPageObjectPatternFill)
}

// SetPageObjectPatternStroke replaces a path or text stroke with a repeated pattern.
func (doc *Document) SetPageObjectPatternStroke(pageIndex int, objectID string, style PatternStyle) error {
	return doc.setPageObjectPaint(pageIndex, objectID, style, "pattern stroke", api.setPageObjectPatternStroke)
}

// SetPageObjectSolidFill replaces a path or text fill with an RGB color.
func (doc *Document) SetPageObjectSolidFill(pageIndex int, objectID string, style SolidPaintStyle) error {
	return doc.setPageObjectPaint(pageIndex, objectID, style, "solid fill", api.setPageObjectSolidFill)
}

// SetPageObjectSolidStroke replaces a path or text stroke with an RGB color.
func (doc *Document) SetPageObjectSolidStroke(pageIndex int, objectID string, style SolidPaintStyle) error {
	return doc.setPageObjectPaint(pageIndex, objectID, style, "solid stroke", api.setPageObjectSolidStroke)
}

func (doc *Document) setPageObjectPaint(pageIndex int, objectID string, style any, name string, native func(uintptr, uintptr, *byte, *byte) int32) error {
	doc.mu.Lock()
	defer doc.mu.Unlock()
	if err := doc.requireOpen(); err != nil {
		return err
	}
	if pageIndex < 0 {
		return errors.New("negative page index")
	}
	id, err := cString(objectID)
	if err != nil {
		return err
	}
	payload, err := json.Marshal(style)
	if err != nil {
		return fmt.Errorf("encode %s: %w", name, err)
	}
	encoded, err := cString(string(payload))
	if err != nil {
		return err
	}
	runtime.LockOSThread()
	defer runtime.UnlockOSThread()
	if err := nativeError(native(doc.handle, uintptr(pageIndex), &id[0], &encoded[0])); err != nil {
		return fmt.Errorf("set page object %s: %w", name, err)
	}
	return nil
}

// SetPageObjectBoundary sets an object's page boundary in millimeters.
func (doc *Document) SetPageObjectBoundary(pageIndex int, objectID string, boundary [4]float64) error {
	doc.mu.Lock()
	defer doc.mu.Unlock()
	if err := doc.requireOpen(); err != nil {
		return err
	}
	if pageIndex < 0 {
		return errors.New("negative page index")
	}
	id, err := cString(objectID)
	if err != nil {
		return err
	}
	runtime.LockOSThread()
	defer runtime.UnlockOSThread()
	if err := nativeError(api.setPageObjectBoundary(doc.handle, uintptr(pageIndex), &id[0], boundary[0], boundary[1], boundary[2], boundary[3])); err != nil {
		return fmt.Errorf("set page object boundary: %w", err)
	}
	return nil
}

// SetPageObjectCTM sets a local transform, or clears it when ctm is nil.
func (doc *Document) SetPageObjectCTM(pageIndex int, objectID string, ctm *[6]float64) error {
	doc.mu.Lock()
	defer doc.mu.Unlock()
	if err := doc.requireOpen(); err != nil {
		return err
	}
	if pageIndex < 0 {
		return errors.New("negative page index")
	}
	id, err := cString(objectID)
	if err != nil {
		return err
	}
	var encoded []byte
	var ctmPtr *byte
	if ctm != nil {
		payload, err := json.Marshal(ctm)
		if err != nil {
			return err
		}
		encoded = append(payload, 0)
		ctmPtr = &encoded[0]
	}
	runtime.LockOSThread()
	defer runtime.UnlockOSThread()
	if err := nativeError(api.setPageObjectCTM(doc.handle, uintptr(pageIndex), &id[0], ctmPtr)); err != nil {
		return fmt.Errorf("set page object CTM: %w", err)
	}
	return nil
}

// RemovePageLayer removes an empty page layer by ID. Its ID remains reserved.
func (doc *Document) RemovePageLayer(pageIndex int, layerID string) error {
	doc.mu.Lock()
	defer doc.mu.Unlock()
	if err := doc.requireOpen(); err != nil {
		return err
	}
	if pageIndex < 0 {
		return errors.New("negative page index")
	}
	id, err := cString(layerID)
	if err != nil {
		return err
	}
	runtime.LockOSThread()
	defer runtime.UnlockOSThread()
	if err := nativeError(api.removePageLayer(doc.handle, uintptr(pageIndex), &id[0])); err != nil {
		return fmt.Errorf("remove page layer: %w", err)
	}
	return nil
}

// PathStyle controls the appearance of a positioned vector path.
type PathStyle struct {
	Fill         bool        `json:"fill"`
	EvenOdd      bool        `json:"even_odd"`
	Alpha        *uint8      `json:"alpha,omitempty"`
	Stroke       bool        `json:"stroke"`
	LineWidth    float64     `json:"line_width"`
	Cap          string      `json:"cap,omitempty"`
	Join         string      `json:"join,omitempty"`
	MiterLimit   *float64    `json:"miter_limit,omitempty"`
	DashPattern  *[2]float64 `json:"dash_pattern,omitempty"`
	DashSequence []float64   `json:"dash_sequence"`
	DashOffset   *float64    `json:"dash_offset,omitempty"`
	FillColor    *[3]uint8   `json:"fill_color,omitempty"`
	FillAlpha    *uint8      `json:"fill_alpha,omitempty"`
	StrokeColor  *[3]uint8   `json:"stroke_color,omitempty"`
	StrokeAlpha  *uint8      `json:"stroke_alpha,omitempty"`
}

// AnnotationPath is one styled path object within a Path annotation.
// Boundary coordinates are local to the annotation appearance, in millimeters.
type AnnotationPath struct {
	Commands string     `json:"commands"`
	Boundary [4]float64 `json:"boundary"`
	PathStyle
}

// AnnotationGraphic is one path, text, image, or composite in an annotation appearance.
// Set Kind to "path", "text", "image", or "composite" and populate matching fields.
type AnnotationGraphic struct {
	Kind       string      `json:"kind"`
	Boundary   [4]float64  `json:"boundary"`
	Commands   string      `json:"commands,omitempty"`
	Text       string      `json:"text,omitempty"`
	ResourceID string      `json:"resource_id,omitempty"`
	Size       float64     `json:"size,omitempty"`
	FontID     string      `json:"font_id,omitempty"`
	Color      *[3]uint8   `json:"color,omitempty"`
	Opacity    *uint8      `json:"opacity,omitempty"`
	CTM        *[6]float64 `json:"ctm,omitempty"`
	DrawParam  string      `json:"draw_param,omitempty"`
	PathStyle
}

// AddPathObject adds a vector path to a page and returns its OFD object ID.
// A zero LineWidth selects the default width of 0.4 mm.
func (doc *Document) AddPathObject(pageIndex int, commands string, boundary [4]float64, style PathStyle) (int, error) {
	doc.mu.Lock()
	defer doc.mu.Unlock()
	if err := doc.requireOpen(); err != nil {
		return 0, err
	}
	if pageIndex < 0 {
		return 0, errors.New("negative page index")
	}
	if style.LineWidth == 0 {
		style.LineWidth = 0.4
	}
	payload, err := json.Marshal(struct {
		Commands string     `json:"commands"`
		Boundary [4]float64 `json:"boundary"`
		PathStyle
	}{commands, boundary, style})
	if err != nil {
		return 0, fmt.Errorf("encode path object: %w", err)
	}
	bytes, err := cString(string(payload))
	if err != nil {
		return 0, err
	}
	runtime.LockOSThread()
	defer runtime.UnlockOSThread()
	var id uintptr
	if err := nativeError(api.addPathObject(doc.handle, uintptr(pageIndex), &bytes[0], &id)); err != nil {
		return 0, fmt.Errorf("add path object: %w", err)
	}
	return int(id), nil
}

// AddCompositeObject places an existing composite graphic resource on a page.
func (doc *Document) AddCompositeObject(pageIndex int, resourceID string, boundary [4]float64, style CompositeStyle) (int, error) {
	doc.mu.Lock()
	defer doc.mu.Unlock()
	if err := doc.requireOpen(); err != nil {
		return 0, err
	}
	if pageIndex < 0 {
		return 0, errors.New("negative page index")
	}
	payload, err := json.Marshal(struct {
		ResourceID string     `json:"resource_id"`
		Boundary   [4]float64 `json:"boundary"`
		CompositeStyle
	}{resourceID, boundary, style})
	if err != nil {
		return 0, fmt.Errorf("encode composite object: %w", err)
	}
	bytes, err := cString(string(payload))
	if err != nil {
		return 0, err
	}
	runtime.LockOSThread()
	defer runtime.UnlockOSThread()
	var id uintptr
	if err := nativeError(api.addCompositeObject(doc.handle, uintptr(pageIndex), &bytes[0], &id)); err != nil {
		return 0, fmt.Errorf("add composite object: %w", err)
	}
	return int(id), nil
}

// AddCompositePathObject creates a document composite resource containing one path.
func (doc *Document) AddCompositePathObject(pageIndex int, commands string, boundary [4]float64, style PathStyle) (int, error) {
	doc.mu.Lock()
	defer doc.mu.Unlock()
	if err := doc.requireOpen(); err != nil {
		return 0, err
	}
	if pageIndex < 0 {
		return 0, errors.New("negative page index")
	}
	if style.LineWidth == 0 {
		style.LineWidth = 0.4
	}
	payload, err := json.Marshal(struct {
		Commands string     `json:"commands"`
		Boundary [4]float64 `json:"boundary"`
		PathStyle
	}{commands, boundary, style})
	if err != nil {
		return 0, fmt.Errorf("encode composite path object: %w", err)
	}
	bytes, err := cString(string(payload))
	if err != nil {
		return 0, err
	}
	runtime.LockOSThread()
	defer runtime.UnlockOSThread()
	var id uintptr
	if err := nativeError(api.addCompositePathObject(doc.handle, uintptr(pageIndex), &bytes[0], &id)); err != nil {
		return 0, fmt.Errorf("add composite path object: %w", err)
	}
	return int(id), nil
}

// TextWatermarkStyle controls the appearance of a text watermark annotation.
type TextWatermarkStyle struct {
	FontID       string
	Color        [3]uint8
	Opacity      uint8
	AngleDegrees float64
	TileSpacing  *[2]float64
	Bold         bool
	Italic       bool
}

// DefaultTextWatermarkStyle draws opaque black, unrotated text.
func DefaultTextWatermarkStyle() TextWatermarkStyle {
	return TextWatermarkStyle{Opacity: 255}
}

// AddTextWatermark adds a text watermark annotation to a page and returns its ID.
func (doc *Document) AddTextWatermark(pageIndex int, text string, boundary [4]float64, size float64) (int, error) {
	return doc.AddStyledTextWatermark(pageIndex, text, boundary, size, DefaultTextWatermarkStyle())
}

// AddStyledTextWatermark adds a colored, translucent, rotated watermark.
func (doc *Document) AddStyledTextWatermark(pageIndex int, text string, boundary [4]float64, size float64, style TextWatermarkStyle) (int, error) {
	return doc.addStyledTextAnnotation(pageIndex, text, boundary, size, style, false)
}

// AddTextStampAnnotation adds a text Stamp annotation with the default style.
func (doc *Document) AddTextStampAnnotation(pageIndex int, text string, boundary [4]float64, size float64) (int, error) {
	return doc.AddStyledTextStampAnnotation(pageIndex, text, boundary, size, DefaultTextWatermarkStyle())
}

// AddStyledTextStampAnnotation adds a text Stamp annotation with explicit style.
func (doc *Document) AddStyledTextStampAnnotation(pageIndex int, text string, boundary [4]float64, size float64, style TextWatermarkStyle) (int, error) {
	return doc.addStyledTextAnnotation(pageIndex, text, boundary, size, style, true)
}

func (doc *Document) addStyledTextAnnotation(pageIndex int, text string, boundary [4]float64, size float64, style TextWatermarkStyle, stamp bool) (int, error) {
	doc.mu.Lock()
	defer doc.mu.Unlock()
	if err := doc.requireOpen(); err != nil {
		return 0, err
	}
	if pageIndex < 0 {
		return 0, errors.New("negative page index")
	}
	var fontID any
	if style.FontID != "" {
		fontID = style.FontID
	}
	payload, err := json.Marshal(map[string]any{
		"text": text, "boundary": boundary, "size": size,
		"font_id": fontID, "color": style.Color, "opacity": style.Opacity,
		"angle_degrees": style.AngleDegrees, "tile_spacing": style.TileSpacing,
		"bold": style.Bold, "italic": style.Italic,
	})
	if err != nil {
		return 0, fmt.Errorf("encode watermark: %w", err)
	}
	bytes, err := cString(string(payload))
	if err != nil {
		return 0, err
	}
	runtime.LockOSThread()
	defer runtime.UnlockOSThread()
	var id uintptr
	add := api.addTextWatermark
	if stamp {
		add = api.addTextStampAnnotation
	}
	if err := nativeError(add(doc.handle, uintptr(pageIndex), &bytes[0], &id)); err != nil {
		return 0, fmt.Errorf("add text annotation: %w", err)
	}
	return int(id), nil
}

// AddTextWatermarkToAllPages atomically covers each current page with a watermark.
// It returns annotation IDs in page order.
func (doc *Document) AddTextWatermarkToAllPages(text string, size float64, style TextWatermarkStyle) ([]int, error) {
	doc.mu.Lock()
	defer doc.mu.Unlock()
	if err := doc.requireOpen(); err != nil {
		return nil, err
	}
	var fontID any
	if style.FontID != "" {
		fontID = style.FontID
	}
	payload, err := json.Marshal(map[string]any{
		"text": text, "size": size, "font_id": fontID,
		"color": style.Color, "opacity": style.Opacity,
		"angle_degrees": style.AngleDegrees, "tile_spacing": style.TileSpacing,
		"bold": style.Bold, "italic": style.Italic,
	})
	if err != nil {
		return nil, fmt.Errorf("encode watermark: %w", err)
	}
	bytes, err := cString(string(payload))
	if err != nil {
		return nil, err
	}
	runtime.LockOSThread()
	defer runtime.UnlockOSThread()
	var buffer nativeBuffer
	if err := nativeError(api.addTextWatermarkAllPages(doc.handle, &bytes[0], &buffer)); err != nil {
		return nil, fmt.Errorf("add text watermark to all pages: %w", err)
	}
	defer api.bufferFree(buffer)
	data, err := bytesAt(buffer)
	if err != nil {
		return nil, err
	}
	var ids []int
	if err := json.Unmarshal(data, &ids); err != nil {
		return nil, fmt.Errorf("decode watermark IDs: %w", err)
	}
	return ids, nil
}

// AddImageWatermark draws an existing page-visible image resource as a watermark.
func (doc *Document) AddImageWatermark(pageIndex int, resourceID string, boundary [4]float64, opacity uint8) (int, error) {
	return doc.addImageAnnotation(pageIndex, boundary, opacity, resourceID, "", nil, false)
}

// AddImageWatermarkFromBytes adds an image resource and watermark atomically.
func (doc *Document) AddImageWatermarkFromBytes(pageIndex int, data []byte, format string, boundary [4]float64, opacity uint8) (int, error) {
	return doc.addImageAnnotation(pageIndex, boundary, opacity, "", format, data, false)
}

// AddImageStampAnnotation draws an existing page-visible image resource as a Stamp annotation.
func (doc *Document) AddImageStampAnnotation(pageIndex int, resourceID string, boundary [4]float64, opacity uint8) (int, error) {
	return doc.addImageAnnotation(pageIndex, boundary, opacity, resourceID, "", nil, true)
}

// AddImageStampAnnotationFromBytes adds an image resource and Stamp annotation atomically.
func (doc *Document) AddImageStampAnnotationFromBytes(pageIndex int, data []byte, format string, boundary [4]float64, opacity uint8) (int, error) {
	return doc.addImageAnnotation(pageIndex, boundary, opacity, "", format, data, true)
}

// AddImageWatermarkToAllPages atomically covers every current page with image data.
// Image resources are reused within each document body; IDs are returned in page order.
func (doc *Document) AddImageWatermarkToAllPages(data []byte, format string, boundary [4]float64, opacity uint8) ([]int, error) {
	doc.mu.Lock()
	defer doc.mu.Unlock()
	if err := doc.requireOpen(); err != nil {
		return nil, err
	}
	payload, err := json.Marshal(map[string]any{
		"boundary": boundary, "opacity": opacity,
		"data_base64": base64.StdEncoding.EncodeToString(data), "format": format,
	})
	if err != nil {
		return nil, fmt.Errorf("encode image watermark: %w", err)
	}
	bytes, err := cString(string(payload))
	if err != nil {
		return nil, err
	}
	runtime.LockOSThread()
	defer runtime.UnlockOSThread()
	var buffer nativeBuffer
	if err := nativeError(api.addImageWatermarkAllPages(doc.handle, &bytes[0], &buffer)); err != nil {
		return nil, fmt.Errorf("add image watermark to all pages: %w", err)
	}
	defer api.bufferFree(buffer)
	result, err := bytesAt(buffer)
	if err != nil {
		return nil, err
	}
	var ids []int
	if err := json.Unmarshal(result, &ids); err != nil {
		return nil, fmt.Errorf("decode watermark IDs: %w", err)
	}
	return ids, nil
}

func (doc *Document) addImageAnnotation(pageIndex int, boundary [4]float64, opacity uint8, resourceID, format string, data []byte, stamp bool) (int, error) {
	doc.mu.Lock()
	defer doc.mu.Unlock()
	if err := doc.requireOpen(); err != nil {
		return 0, err
	}
	if pageIndex < 0 {
		return 0, errors.New("negative page index")
	}
	var encoded any
	if data != nil {
		encoded = base64.StdEncoding.EncodeToString(data)
	}
	var resource any
	if resourceID != "" {
		resource = resourceID
	}
	payload, err := json.Marshal(map[string]any{
		"boundary": boundary, "opacity": opacity,
		"resource_id": resource, "data_base64": encoded, "format": format,
	})
	if err != nil {
		return 0, fmt.Errorf("encode image watermark: %w", err)
	}
	bytes, err := cString(string(payload))
	if err != nil {
		return 0, err
	}
	runtime.LockOSThread()
	defer runtime.UnlockOSThread()
	var id uintptr
	call := api.addImageWatermark
	label := "image watermark"
	if stamp {
		call = api.addImageStampAnnotation
		label = "image stamp annotation"
	}
	if err := nativeError(call(doc.handle, uintptr(pageIndex), &bytes[0], &id)); err != nil {
		return 0, fmt.Errorf("add %s: %w", label, err)
	}
	return int(id), nil
}

// AddPathAnnotation adds a styled vector Path annotation and returns its ID.
// A zero LineWidth selects the default width of 0.4 mm.
func (doc *Document) AddPathAnnotation(pageIndex int, commands string, boundary [4]float64, style PathStyle) (int, error) {
	doc.mu.Lock()
	defer doc.mu.Unlock()
	if err := doc.requireOpen(); err != nil {
		return 0, err
	}
	if pageIndex < 0 {
		return 0, errors.New("negative page index")
	}
	if style.LineWidth == 0 {
		style.LineWidth = 0.4
	}
	payload, err := json.Marshal(struct {
		Commands string     `json:"commands"`
		Boundary [4]float64 `json:"boundary"`
		PathStyle
	}{commands, boundary, style})
	if err != nil {
		return 0, fmt.Errorf("encode path annotation: %w", err)
	}
	bytes, err := cString(string(payload))
	if err != nil {
		return 0, err
	}
	runtime.LockOSThread()
	defer runtime.UnlockOSThread()
	var id uintptr
	if err := nativeError(api.addPathAnnotation(doc.handle, uintptr(pageIndex), &bytes[0], &id)); err != nil {
		return 0, fmt.Errorf("add path annotation: %w", err)
	}
	return int(id), nil
}

// AddMultiPathAnnotation adds one Path annotation with several styled path objects.
func (doc *Document) AddMultiPathAnnotation(pageIndex int, boundary [4]float64, paths []AnnotationPath) (int, error) {
	doc.mu.Lock()
	defer doc.mu.Unlock()
	if err := doc.requireOpen(); err != nil {
		return 0, err
	}
	if pageIndex < 0 {
		return 0, errors.New("negative page index")
	}
	prepared := make([]AnnotationPath, len(paths))
	copy(prepared, paths)
	for index := range prepared {
		if prepared[index].LineWidth == 0 {
			prepared[index].LineWidth = 0.4
		}
	}
	payload, err := json.Marshal(struct {
		Boundary [4]float64       `json:"boundary"`
		Paths    []AnnotationPath `json:"paths"`
	}{boundary, prepared})
	if err != nil {
		return 0, fmt.Errorf("encode multi-path annotation: %w", err)
	}
	bytes, err := cString(string(payload))
	if err != nil {
		return 0, err
	}
	runtime.LockOSThread()
	defer runtime.UnlockOSThread()
	var id uintptr
	if err := nativeError(api.addMultiPathAnnotation(doc.handle, uintptr(pageIndex), &bytes[0], &id)); err != nil {
		return 0, fmt.Errorf("add multi-path annotation: %w", err)
	}
	return int(id), nil
}

// AddMixedAnnotation adds ordered path, text, image, and composite objects to one Path annotation.
// Object boundaries are local to the annotation appearance.
func (doc *Document) AddMixedAnnotation(pageIndex int, boundary [4]float64, objects []AnnotationGraphic) (int, error) {
	return doc.addMixedAnnotation(pageIndex, boundary, objects, "Path")
}

// AddMixedStampAnnotation adds ordered path, text, image, and composite objects to one Stamp annotation.
func (doc *Document) AddMixedStampAnnotation(pageIndex int, boundary [4]float64, objects []AnnotationGraphic) (int, error) {
	return doc.addMixedAnnotation(pageIndex, boundary, objects, "Stamp")
}

func (doc *Document) addMixedAnnotation(pageIndex int, boundary [4]float64, objects []AnnotationGraphic, annotationType string) (int, error) {
	doc.mu.Lock()
	defer doc.mu.Unlock()
	if err := doc.requireOpen(); err != nil {
		return 0, err
	}
	if pageIndex < 0 {
		return 0, errors.New("negative page index")
	}
	prepared := make([]AnnotationGraphic, len(objects))
	copy(prepared, objects)
	for index := range prepared {
		if prepared[index].Kind == "path" && prepared[index].LineWidth == 0 {
			prepared[index].LineWidth = 0.4
		}
	}
	payload, err := json.Marshal(struct {
		Boundary       [4]float64          `json:"boundary"`
		Objects        []AnnotationGraphic `json:"objects"`
		AnnotationType string              `json:"annotation_type"`
	}{boundary, prepared, annotationType})
	if err != nil {
		return 0, fmt.Errorf("encode mixed annotation: %w", err)
	}
	bytes, err := cString(string(payload))
	if err != nil {
		return 0, err
	}
	runtime.LockOSThread()
	defer runtime.UnlockOSThread()
	var id uintptr
	if err := nativeError(api.addMixedAnnotation(doc.handle, uintptr(pageIndex), &bytes[0], &id)); err != nil {
		return 0, fmt.Errorf("add mixed annotation: %w", err)
	}
	return int(id), nil
}

// AddHighlightAnnotation adds a translucent rectangular Highlight annotation.
// Opacity is an OFD alpha byte from 0 to 255.
func (doc *Document) AddHighlightAnnotation(pageIndex int, boundary [4]float64, color [3]uint8, opacity uint8) (int, error) {
	doc.mu.Lock()
	defer doc.mu.Unlock()
	if err := doc.requireOpen(); err != nil {
		return 0, err
	}
	if pageIndex < 0 {
		return 0, errors.New("negative page index")
	}
	payload, err := json.Marshal(struct {
		Boundary [4]float64 `json:"boundary"`
		Color    [3]uint8   `json:"color"`
		Opacity  uint8      `json:"opacity"`
	}{boundary, color, opacity})
	if err != nil {
		return 0, fmt.Errorf("encode highlight annotation: %w", err)
	}
	bytes, err := cString(string(payload))
	if err != nil {
		return 0, err
	}
	runtime.LockOSThread()
	defer runtime.UnlockOSThread()
	var id uintptr
	if err := nativeError(api.addHighlightAnnotation(doc.handle, uintptr(pageIndex), &bytes[0], &id)); err != nil {
		return 0, fmt.Errorf("add highlight annotation: %w", err)
	}
	return int(id), nil
}

// TextNoteOptions controls an editable Text note annotation.
type TextNoteOptions struct {
	Color   *[3]uint8 `json:"color,omitempty"`
	Opacity *uint8    `json:"opacity,omitempty"`
	Icon    string    `json:"icon,omitempty"`
	Open    bool      `json:"open"`
}

// AddTextNoteAnnotation adds an editable Text note with a simple OFD preview marker.
func (doc *Document) AddTextNoteAnnotation(pageIndex int, text string, boundary [4]float64, options TextNoteOptions) (int, error) {
	doc.mu.Lock()
	defer doc.mu.Unlock()
	if err := doc.requireOpen(); err != nil {
		return 0, err
	}
	if pageIndex < 0 {
		return 0, errors.New("negative page index")
	}
	payload, err := json.Marshal(struct {
		Text     string     `json:"text"`
		Boundary [4]float64 `json:"boundary"`
		TextNoteOptions
	}{text, boundary, options})
	if err != nil {
		return 0, fmt.Errorf("encode text note annotation: %w", err)
	}
	bytes, err := cString(string(payload))
	if err != nil {
		return 0, err
	}
	runtime.LockOSThread()
	defer runtime.UnlockOSThread()
	var id uintptr
	if err := nativeError(api.addTextNoteAnnotation(doc.handle, uintptr(pageIndex), &bytes[0], &id)); err != nil {
		return 0, fmt.Errorf("add text note annotation: %w", err)
	}
	return int(id), nil
}

// ShapeAnnotationOptions controls an editable Square or Circle annotation.
// Pointer fields distinguish an omitted value from zero; dimensions use millimeters.
type ShapeAnnotationOptions struct {
	StrokeColor  *[3]uint8   `json:"stroke_color,omitempty"`
	Stroke       *bool       `json:"stroke,omitempty"`
	FillColor    *[3]uint8   `json:"fill_color,omitempty"`
	LineWidth    *float64    `json:"line_width,omitempty"`
	DashSequence []float64   `json:"dash_sequence,omitempty"`
	Opacity      *uint8      `json:"opacity,omitempty"`
	Insets       *[4]float64 `json:"insets,omitempty"`
}

// AddShapeAnnotation adds an editable Square or Circle with a vector OFD preview.
func (doc *Document) AddShapeAnnotation(pageIndex int, kind string, boundary [4]float64, options ShapeAnnotationOptions) (int, error) {
	doc.mu.Lock()
	defer doc.mu.Unlock()
	if err := doc.requireOpen(); err != nil {
		return 0, err
	}
	if pageIndex < 0 {
		return 0, errors.New("negative page index")
	}
	payload, err := json.Marshal(struct {
		Kind     string     `json:"kind"`
		Boundary [4]float64 `json:"boundary"`
		ShapeAnnotationOptions
	}{kind, boundary, options})
	if err != nil {
		return 0, fmt.Errorf("encode shape annotation: %w", err)
	}
	bytes, err := cString(string(payload))
	if err != nil {
		return 0, err
	}
	runtime.LockOSThread()
	defer runtime.UnlockOSThread()
	var id uintptr
	if err := nativeError(api.addShapeAnnotation(doc.handle, uintptr(pageIndex), &bytes[0], &id)); err != nil {
		return 0, fmt.Errorf("add shape annotation: %w", err)
	}
	return int(id), nil
}

// LineAnnotationOptions controls the stroke of an editable Line annotation.
type LineAnnotationOptions struct {
	StrokeColor  *[3]uint8 `json:"stroke_color,omitempty"`
	Stroke       *bool     `json:"stroke,omitempty"`
	LineWidth    *float64  `json:"line_width,omitempty"`
	DashSequence []float64 `json:"dash_sequence,omitempty"`
	Opacity      *uint8    `json:"opacity,omitempty"`
}

// AddLineAnnotation adds an editable Line. Endpoints are boundary-local millimeters.
func (doc *Document) AddLineAnnotation(pageIndex int, boundary, endpoints [4]float64, options LineAnnotationOptions) (int, error) {
	doc.mu.Lock()
	defer doc.mu.Unlock()
	if err := doc.requireOpen(); err != nil {
		return 0, err
	}
	if pageIndex < 0 {
		return 0, errors.New("negative page index")
	}
	payload, err := json.Marshal(struct {
		Boundary  [4]float64 `json:"boundary"`
		Endpoints [4]float64 `json:"endpoints"`
		LineAnnotationOptions
	}{boundary, endpoints, options})
	if err != nil {
		return 0, fmt.Errorf("encode line annotation: %w", err)
	}
	bytes, err := cString(string(payload))
	if err != nil {
		return 0, err
	}
	runtime.LockOSThread()
	defer runtime.UnlockOSThread()
	var id uintptr
	if err := nativeError(api.addLineAnnotation(doc.handle, uintptr(pageIndex), &bytes[0], &id)); err != nil {
		return 0, fmt.Errorf("add line annotation: %w", err)
	}
	return int(id), nil
}

// AddURILinkAnnotation adds an invisible clickable URI Link annotation and returns its ID.
func (doc *Document) AddURILinkAnnotation(pageIndex int, uri string, boundary [4]float64) (int, error) {
	return doc.AddURILinkAnnotationWithQuads(pageIndex, uri, boundary, nil)
}

// AddURILinkAnnotationWithQuads adds a URI Link with convex quadrilateral click areas.
// Coordinates are page millimeters in vertex order around each area.
func (doc *Document) AddURILinkAnnotationWithQuads(pageIndex int, uri string, boundary [4]float64, quads [][4][2]float64) (int, error) {
	doc.mu.Lock()
	defer doc.mu.Unlock()
	if err := doc.requireOpen(); err != nil {
		return 0, err
	}
	if pageIndex < 0 {
		return 0, errors.New("negative page index")
	}
	if quads == nil {
		quads = [][4][2]float64{}
	}
	payload, err := json.Marshal(struct {
		URI        string          `json:"uri"`
		Boundary   [4]float64      `json:"boundary"`
		QuadPoints [][4][2]float64 `json:"quad_points"`
	}{uri, boundary, quads})
	if err != nil {
		return 0, fmt.Errorf("encode URI link annotation: %w", err)
	}
	bytes, err := cString(string(payload))
	if err != nil {
		return 0, err
	}
	runtime.LockOSThread()
	defer runtime.UnlockOSThread()
	var id uintptr
	if err := nativeError(api.addURILinkAnnotation(doc.handle, uintptr(pageIndex), &bytes[0], &id)); err != nil {
		return 0, fmt.Errorf("add URI link annotation: %w", err)
	}
	return int(id), nil
}

// AddGotoLinkAnnotation adds an invisible Link to a page in the same document body.
func (doc *Document) AddGotoLinkAnnotation(pageIndex int, destination Destination, boundary [4]float64) (int, error) {
	return doc.AddGotoLinkAnnotationWithQuads(pageIndex, destination, boundary, nil)
}

// AddGotoLinkAnnotationWithQuads adds a page-destination Link with convex quadrilateral click areas.
// Coordinates are page millimeters in vertex order around each area.
func (doc *Document) AddGotoLinkAnnotationWithQuads(pageIndex int, destination Destination, boundary [4]float64, quads [][4][2]float64) (int, error) {
	doc.mu.Lock()
	defer doc.mu.Unlock()
	if err := doc.requireOpen(); err != nil {
		return 0, err
	}
	if pageIndex < 0 {
		return 0, errors.New("negative page index")
	}
	if quads == nil {
		quads = [][4][2]float64{}
	}
	payload, err := json.Marshal(struct {
		Destination Destination     `json:"destination"`
		Boundary    [4]float64      `json:"boundary"`
		QuadPoints  [][4][2]float64 `json:"quad_points"`
	}{destination, boundary, quads})
	if err != nil {
		return 0, fmt.Errorf("encode Goto link annotation: %w", err)
	}
	bytes, err := cString(string(payload))
	if err != nil {
		return 0, err
	}
	runtime.LockOSThread()
	defer runtime.UnlockOSThread()
	var id uintptr
	if err := nativeError(api.addGotoLinkAnnotation(doc.handle, uintptr(pageIndex), &bytes[0], &id)); err != nil {
		return 0, fmt.Errorf("add Goto link annotation: %w", err)
	}
	return int(id), nil
}

// AddBookmarkLinkAnnotation adds an invisible Link to a named bookmark.
func (doc *Document) AddBookmarkLinkAnnotation(pageIndex int, bookmarkName string, boundary [4]float64) (int, error) {
	doc.mu.Lock()
	defer doc.mu.Unlock()
	if err := doc.requireOpen(); err != nil {
		return 0, err
	}
	if pageIndex < 0 {
		return 0, errors.New("negative page index")
	}
	payload, err := json.Marshal(struct {
		BookmarkName string     `json:"bookmark_name"`
		Boundary     [4]float64 `json:"boundary"`
	}{bookmarkName, boundary})
	if err != nil {
		return 0, fmt.Errorf("encode bookmark link annotation: %w", err)
	}
	bytes, err := cString(string(payload))
	if err != nil {
		return 0, err
	}
	runtime.LockOSThread()
	defer runtime.UnlockOSThread()
	var id uintptr
	if err := nativeError(api.addBookmarkLinkAnnotation(doc.handle, uintptr(pageIndex), &bytes[0], &id)); err != nil {
		return 0, fmt.Errorf("add bookmark link annotation: %w", err)
	}
	return int(id), nil
}

// AddAttachmentLinkAnnotation adds an invisible Link to an attachment.
// A nil newWindow omits the OFD NewWindow attribute.
func (doc *Document) AddAttachmentLinkAnnotation(pageIndex int, attachmentID string, boundary [4]float64, newWindow *bool) (int, error) {
	doc.mu.Lock()
	defer doc.mu.Unlock()
	if err := doc.requireOpen(); err != nil {
		return 0, err
	}
	if pageIndex < 0 {
		return 0, errors.New("negative page index")
	}
	payload, err := json.Marshal(struct {
		AttachmentID string     `json:"attachment_id"`
		Boundary     [4]float64 `json:"boundary"`
		NewWindow    *bool      `json:"new_window"`
	}{attachmentID, boundary, newWindow})
	if err != nil {
		return 0, fmt.Errorf("encode attachment link annotation: %w", err)
	}
	bytes, err := cString(string(payload))
	if err != nil {
		return 0, err
	}
	runtime.LockOSThread()
	defer runtime.UnlockOSThread()
	var id uintptr
	if err := nativeError(api.addAttachmentLinkAnnotation(doc.handle, uintptr(pageIndex), &bytes[0], &id)); err != nil {
		return 0, fmt.Errorf("add attachment link annotation: %w", err)
	}
	return int(id), nil
}

func (doc *Document) addMediaLinkAnnotation(pageIndex int, payload map[string]any) (int, error) {
	doc.mu.Lock()
	defer doc.mu.Unlock()
	if err := doc.requireOpen(); err != nil {
		return 0, err
	}
	if pageIndex < 0 {
		return 0, errors.New("negative page index")
	}
	encoded, err := json.Marshal(payload)
	if err != nil {
		return 0, fmt.Errorf("encode media link annotation: %w", err)
	}
	data, err := cString(string(encoded))
	if err != nil {
		return 0, err
	}
	runtime.LockOSThread()
	defer runtime.UnlockOSThread()
	var id uintptr
	if err := nativeError(api.addMediaLinkAnnotation(doc.handle, uintptr(pageIndex), &data[0], &id)); err != nil {
		return 0, fmt.Errorf("add media link annotation: %w", err)
	}
	return int(id), nil
}

// AddSoundLinkAnnotation adds an invisible Link that plays an Audio resource.
func (doc *Document) AddSoundLinkAnnotation(pageIndex int, resourceID string, boundary [4]float64, options SoundActionOptions) (int, error) {
	return doc.addMediaLinkAnnotation(pageIndex, map[string]any{
		"kind": "Sound", "resource_id": resourceID, "boundary": boundary,
		"volume": options.Volume, "repeat": options.Repeat, "synchronous": options.Synchronous,
	})
}

// AddMovieLinkAnnotation adds an invisible Link that controls a Video resource.
// An empty operator uses the OFD default Play behavior.
func (doc *Document) AddMovieLinkAnnotation(pageIndex int, resourceID string, boundary [4]float64, operator string) (int, error) {
	payload := map[string]any{"kind": "Movie", "resource_id": resourceID, "boundary": boundary}
	if operator != "" {
		payload["operator"] = operator
	}
	return doc.addMediaLinkAnnotation(pageIndex, payload)
}

// AddTextPageWithFont appends a text page using a declared font resource.
func (doc *Document) AddTextPageWithFont(text string, fontID int, widthMM, heightMM float64) error {
	doc.mu.Lock()
	defer doc.mu.Unlock()
	if err := doc.requireOpen(); err != nil {
		return err
	}
	if fontID < 0 {
		return errors.New("negative font identifier")
	}
	payload, err := textRequest(map[string]any{
		"operation": "add", "text": text,
		"width_mm": widthMM, "height_mm": heightMM,
		"font_id": fontID,
	})
	if err != nil {
		return err
	}
	runtime.LockOSThread()
	defer runtime.UnlockOSThread()
	if err := nativeError(api.writeTextPage(doc.handle, &payload[0], nil)); err != nil {
		return fmt.Errorf("add text page with font: %w", err)
	}
	return nil
}

// AddEmbeddedFont embeds a TrueType or OpenType font and returns its resource ID.
func (doc *Document) AddEmbeddedFont(name string, data []byte) (int, error) {
	doc.mu.Lock()
	defer doc.mu.Unlock()
	if err := doc.requireOpen(); err != nil {
		return 0, err
	}
	nameBytes, err := cString(name)
	if err != nil {
		return 0, err
	}
	var dataPtr *byte
	if len(data) > 0 {
		dataPtr = &data[0]
	}
	runtime.LockOSThread()
	defer runtime.UnlockOSThread()
	var id uintptr
	if err := nativeError(api.addFont(doc.handle, &nameBytes[0], dataPtr, uintptr(len(data)), &id)); err != nil {
		return 0, fmt.Errorf("add embedded font: %w", err)
	}
	runtime.KeepAlive(data)
	if id > uintptr(int(^uint(0)>>1)) {
		return 0, errors.New("font identifier exceeds Go int range")
	}
	return int(id), nil
}

// AddEmbeddedFontToDocument embeds a font in the selected document body.
func (doc *Document) AddEmbeddedFontToDocument(documentIndex int, name string, data []byte) (int, error) {
	doc.mu.Lock()
	defer doc.mu.Unlock()
	if err := doc.requireOpen(); err != nil {
		return 0, err
	}
	if documentIndex < 0 {
		return 0, errors.New("document index must be nonnegative")
	}
	nameBytes, err := cString(name)
	if err != nil {
		return 0, err
	}
	var dataPtr *byte
	if len(data) > 0 {
		dataPtr = &data[0]
	}
	runtime.LockOSThread()
	defer runtime.UnlockOSThread()
	var id uintptr
	if err := nativeError(api.addFontToDocument(
		doc.handle,
		uintptr(documentIndex),
		&nameBytes[0],
		dataPtr,
		uintptr(len(data)),
		&id,
	)); err != nil {
		return 0, fmt.Errorf("add embedded font to document: %w", err)
	}
	runtime.KeepAlive(data)
	if id > uintptr(int(^uint(0)>>1)) {
		return 0, errors.New("font identifier exceeds Go int range")
	}
	return int(id), nil
}

// AddEmbeddedFontToPage embeds a font in one page's resources.
func (doc *Document) AddEmbeddedFontToPage(pageIndex int, name string, data []byte) (int, error) {
	doc.mu.Lock()
	defer doc.mu.Unlock()
	if err := doc.requireOpen(); err != nil {
		return 0, err
	}
	if pageIndex < 0 {
		return 0, errors.New("page index must be nonnegative")
	}
	nameBytes, err := cString(name)
	if err != nil {
		return 0, err
	}
	var dataPtr *byte
	if len(data) > 0 {
		dataPtr = &data[0]
	}
	runtime.LockOSThread()
	defer runtime.UnlockOSThread()
	var id uintptr
	if err := nativeError(api.addFontToPage(doc.handle, uintptr(pageIndex), &nameBytes[0], dataPtr, uintptr(len(data)), &id)); err != nil {
		return 0, fmt.Errorf("add embedded font to page: %w", err)
	}
	runtime.KeepAlive(data)
	if id > uintptr(int(^uint(0)>>1)) {
		return 0, errors.New("font identifier exceeds Go int range")
	}
	return int(id), nil
}

// SetFontAttribute sets or clears a standard font attribute by Fonts index.
func (doc *Document) SetFontAttribute(fontIndex int, name string, value *string) error {
	doc.mu.Lock()
	defer doc.mu.Unlock()
	if err := doc.requireOpen(); err != nil {
		return err
	}
	if fontIndex < 0 {
		return errors.New("negative font index")
	}
	nameBytes, err := cString(name)
	if err != nil {
		return err
	}
	var valuePtr *byte
	var valueBytes []byte
	if value != nil {
		valueBytes, err = cString(*value)
		if err != nil {
			return err
		}
		valuePtr = &valueBytes[0]
	}
	runtime.LockOSThread()
	defer runtime.UnlockOSThread()
	if err := nativeError(api.setFontAttribute(doc.handle, uintptr(fontIndex), &nameBytes[0], valuePtr)); err != nil {
		return fmt.Errorf("set font attribute: %w", err)
	}
	runtime.KeepAlive(valueBytes)
	return nil
}

// SetPageFontAttribute edits a font by its index in PageFonts, including page-local fonts.
func (doc *Document) SetPageFontAttribute(pageIndex, fontIndex int, name string, value *string) error {
	doc.mu.Lock()
	defer doc.mu.Unlock()
	if err := doc.requireOpen(); err != nil {
		return err
	}
	if pageIndex < 0 || fontIndex < 0 {
		return errors.New("negative page or font index")
	}
	nameBytes, err := cString(name)
	if err != nil {
		return err
	}
	var valuePtr *byte
	var valueBytes []byte
	if value != nil {
		valueBytes, err = cString(*value)
		if err != nil {
			return err
		}
		valuePtr = &valueBytes[0]
	}
	runtime.LockOSThread()
	defer runtime.UnlockOSThread()
	if err := nativeError(api.setPageFontAttribute(doc.handle, uintptr(pageIndex), uintptr(fontIndex), &nameBytes[0], valuePtr)); err != nil {
		return fmt.Errorf("set page font attribute: %w", err)
	}
	runtime.KeepAlive(valueBytes)
	return nil
}

// ReplaceFontData replaces an embedded font by index in Fonts.
func (doc *Document) ReplaceFontData(fontIndex int, data []byte) error {
	doc.mu.Lock()
	defer doc.mu.Unlock()
	if err := doc.requireOpen(); err != nil {
		return err
	}
	if fontIndex < 0 {
		return errors.New("negative font index")
	}
	var dataPtr *byte
	if len(data) > 0 {
		dataPtr = &data[0]
	}
	runtime.LockOSThread()
	defer runtime.UnlockOSThread()
	if err := nativeError(api.replaceFontData(doc.handle, uintptr(fontIndex), dataPtr, uintptr(len(data)))); err != nil {
		return fmt.Errorf("replace font data: %w", err)
	}
	runtime.KeepAlive(data)
	return nil
}

// ReplacePageFontData replaces an embedded font by index in PageFonts.
func (doc *Document) ReplacePageFontData(pageIndex, fontIndex int, data []byte) error {
	doc.mu.Lock()
	defer doc.mu.Unlock()
	if err := doc.requireOpen(); err != nil {
		return err
	}
	if pageIndex < 0 || fontIndex < 0 {
		return errors.New("negative page or font index")
	}
	var dataPtr *byte
	if len(data) > 0 {
		dataPtr = &data[0]
	}
	runtime.LockOSThread()
	defer runtime.UnlockOSThread()
	if err := nativeError(api.replacePageFontData(doc.handle, uintptr(pageIndex), uintptr(fontIndex), dataPtr, uintptr(len(data)))); err != nil {
		return fmt.Errorf("replace page font data: %w", err)
	}
	runtime.KeepAlive(data)
	return nil
}

// AppendTextPage appends a text page to a body of a new or existing OFD package.
func (doc *Document) AppendTextPage(documentIndex int, text string, widthMM, heightMM float64) error {
	doc.mu.Lock()
	defer doc.mu.Unlock()
	if err := doc.requireOpen(); err != nil {
		return err
	}
	if documentIndex < 0 {
		return errors.New("negative document index")
	}
	payload, err := textRequest(map[string]any{
		"operation": "append", "document_index": documentIndex,
		"text": text, "width_mm": widthMM, "height_mm": heightMM,
	})
	if err != nil {
		return err
	}
	runtime.LockOSThread()
	defer runtime.UnlockOSThread()
	if err := nativeError(api.writeTextPage(doc.handle, &payload[0], nil)); err != nil {
		return fmt.Errorf("append text page: %w", err)
	}
	return nil
}

// AppendBlankPage appends an empty page to a document body.
func (doc *Document) AppendBlankPage(documentIndex int, widthMM, heightMM float64) error {
	doc.mu.Lock()
	defer doc.mu.Unlock()
	if err := doc.requireOpen(); err != nil {
		return err
	}
	if documentIndex < 0 {
		return errors.New("negative document index")
	}
	runtime.LockOSThread()
	defer runtime.UnlockOSThread()
	if err := nativeError(api.appendBlankPage(doc.handle, uintptr(documentIndex), widthMM, heightMM)); err != nil {
		return fmt.Errorf("append blank page: %w", err)
	}
	return nil
}

// InsertTextPage inserts a text page at a body-local position and returns its global index.
func (doc *Document) InsertTextPage(documentIndex, position int, text string, widthMM, heightMM float64) (int, error) {
	doc.mu.Lock()
	defer doc.mu.Unlock()
	if err := doc.requireOpen(); err != nil {
		return 0, err
	}
	if documentIndex < 0 || position < 0 {
		return 0, errors.New("document index and page position must be nonnegative")
	}
	payload, err := textRequest(map[string]any{
		"operation": "insert", "document_index": documentIndex,
		"position": position, "text": text,
		"width_mm": widthMM, "height_mm": heightMM,
	})
	if err != nil {
		return 0, err
	}
	runtime.LockOSThread()
	defer runtime.UnlockOSThread()
	var index uintptr
	if err := nativeError(api.writeTextPage(doc.handle, &payload[0], &index)); err != nil {
		return 0, fmt.Errorf("insert text page: %w", err)
	}
	if index > uintptr(int(^uint(0)>>1)) {
		return 0, errors.New("page index exceeds Go int range")
	}
	return int(index), nil
}

// InsertBlankPage inserts an empty page at a body-local position and returns its global index.
func (doc *Document) InsertBlankPage(documentIndex, position int, widthMM, heightMM float64) (int, error) {
	doc.mu.Lock()
	defer doc.mu.Unlock()
	if err := doc.requireOpen(); err != nil {
		return 0, err
	}
	if documentIndex < 0 || position < 0 {
		return 0, errors.New("document index and page position must be nonnegative")
	}
	runtime.LockOSThread()
	defer runtime.UnlockOSThread()
	var index uintptr
	if err := nativeError(api.insertBlankPage(doc.handle, uintptr(documentIndex), uintptr(position), widthMM, heightMM, &index)); err != nil {
		return 0, fmt.Errorf("insert blank page: %w", err)
	}
	if index > uintptr(int(^uint(0)>>1)) {
		return 0, errors.New("page index exceeds Go int range")
	}
	return int(index), nil
}

// AddFlowedText wraps plain text into newly appended pages and returns their indexes.
func (doc *Document) AddFlowedText(documentIndex int, text string, style FlowTextStyle) ([]int, error) {
	doc.mu.Lock()
	defer doc.mu.Unlock()
	if err := doc.requireOpen(); err != nil {
		return nil, err
	}
	if documentIndex < 0 {
		return nil, errors.New("negative document index")
	}
	if style == (FlowTextStyle{}) {
		style = DefaultFlowTextStyle()
	}
	payload, err := json.Marshal(struct {
		Text string `json:"text"`
		FlowTextStyle
	}{Text: text, FlowTextStyle: style})
	if err != nil {
		return nil, fmt.Errorf("encode flowed text: %w", err)
	}
	payloadBytes, err := cString(string(payload))
	if err != nil {
		return nil, err
	}
	runtime.LockOSThread()
	defer runtime.UnlockOSThread()
	var buffer nativeBuffer
	if err := nativeError(api.addFlowedText(doc.handle, uintptr(documentIndex), &payloadBytes[0], &buffer)); err != nil {
		return nil, fmt.Errorf("add flowed text: %w", err)
	}
	defer api.bufferFree(buffer)
	data, err := bytesAt(buffer)
	if err != nil {
		return nil, err
	}
	var indexes []int
	if err := json.Unmarshal(data, &indexes); err != nil {
		return nil, fmt.Errorf("decode flowed text page indexes: %w", err)
	}
	return indexes, nil
}

// AddTable appends a fixed-grid table and returns its new page indexes.
func (doc *Document) AddTable(documentIndex int, rows [][]string, style TableStyle) ([]int, error) {
	doc.mu.Lock()
	defer doc.mu.Unlock()
	if err := doc.requireOpen(); err != nil {
		return nil, err
	}
	if documentIndex < 0 {
		return nil, errors.New("negative document index")
	}
	payload, err := json.Marshal(struct {
		Rows [][]string `json:"rows"`
		TableStyle
	}{Rows: rows, TableStyle: style})
	if err != nil {
		return nil, fmt.Errorf("encode table: %w", err)
	}
	encoded, err := cString(string(payload))
	if err != nil {
		return nil, err
	}
	runtime.LockOSThread()
	defer runtime.UnlockOSThread()
	var buffer nativeBuffer
	if err := nativeError(api.addTable(doc.handle, uintptr(documentIndex), &encoded[0], &buffer)); err != nil {
		return nil, fmt.Errorf("add table: %w", err)
	}
	defer api.bufferFree(buffer)
	data, err := bytesAt(buffer)
	if err != nil {
		return nil, err
	}
	var indexes []int
	if err := json.Unmarshal(data, &indexes); err != nil {
		return nil, fmt.Errorf("decode table page indexes: %w", err)
	}
	return indexes, nil
}

// AddTableCells appends a table with cells that may span rows or columns.
func (doc *Document) AddTableCells(documentIndex, rowCount int, cells []TableCell, style TableStyle) ([]int, error) {
	doc.mu.Lock()
	defer doc.mu.Unlock()
	if err := doc.requireOpen(); err != nil {
		return nil, err
	}
	if documentIndex < 0 {
		return nil, errors.New("negative document index")
	}
	if rowCount < 0 {
		return nil, errors.New("negative row count")
	}
	payload, err := json.Marshal(struct {
		RowCount int         `json:"row_count"`
		Cells    []TableCell `json:"cells"`
		TableStyle
	}{RowCount: rowCount, Cells: cells, TableStyle: style})
	if err != nil {
		return nil, fmt.Errorf("encode table cells: %w", err)
	}
	encoded, err := cString(string(payload))
	if err != nil {
		return nil, err
	}
	runtime.LockOSThread()
	defer runtime.UnlockOSThread()
	var buffer nativeBuffer
	if err := nativeError(api.addTable(doc.handle, uintptr(documentIndex), &encoded[0], &buffer)); err != nil {
		return nil, fmt.Errorf("add table cells: %w", err)
	}
	defer api.bufferFree(buffer)
	data, err := bytesAt(buffer)
	if err != nil {
		return nil, err
	}
	var indexes []int
	if err := json.Unmarshal(data, &indexes); err != nil {
		return nil, fmt.Errorf("decode table pages: %w", err)
	}
	return indexes, nil
}

// AddFlowedSpans flows styled segments into newly appended pages.
func (doc *Document) AddFlowedSpans(documentIndex int, spans []FlowSpan, style FlowTextStyle) ([]int, error) {
	doc.mu.Lock()
	defer doc.mu.Unlock()
	if err := doc.requireOpen(); err != nil {
		return nil, err
	}
	if documentIndex < 0 {
		return nil, errors.New("negative document index")
	}
	if style == (FlowTextStyle{}) {
		style = DefaultFlowTextStyle()
	}
	payload, err := json.Marshal(struct {
		Spans []FlowSpan `json:"spans"`
		FlowTextStyle
	}{Spans: spans, FlowTextStyle: style})
	if err != nil {
		return nil, fmt.Errorf("encode flowed spans: %w", err)
	}
	payloadBytes, err := cString(string(payload))
	if err != nil {
		return nil, err
	}
	runtime.LockOSThread()
	defer runtime.UnlockOSThread()
	var buffer nativeBuffer
	if err := nativeError(api.addFlowedSpans(doc.handle, uintptr(documentIndex), &payloadBytes[0], &buffer)); err != nil {
		return nil, fmt.Errorf("add flowed spans: %w", err)
	}
	defer api.bufferFree(buffer)
	data, err := bytesAt(buffer)
	if err != nil {
		return nil, err
	}
	var indexes []int
	if err := json.Unmarshal(data, &indexes); err != nil {
		return nil, fmt.Errorf("decode flowed span page indexes: %w", err)
	}
	return indexes, nil
}

// AppendTextPageWithFont appends a text page with the selected document font.
func (doc *Document) AppendTextPageWithFont(
	documentIndex int,
	text string,
	fontID int,
	widthMM, heightMM float64,
) error {
	doc.mu.Lock()
	defer doc.mu.Unlock()
	if err := doc.requireOpen(); err != nil {
		return err
	}
	if documentIndex < 0 || fontID < 0 {
		return errors.New("document index and font ID must be nonnegative")
	}
	payload, err := textRequest(map[string]any{
		"operation": "append", "document_index": documentIndex,
		"text": text, "width_mm": widthMM, "height_mm": heightMM,
		"font_id": fontID,
	})
	if err != nil {
		return err
	}
	runtime.LockOSThread()
	defer runtime.UnlockOSThread()
	if err := nativeError(api.writeTextPage(doc.handle, &payload[0], nil)); err != nil {
		return fmt.Errorf("append text page with font: %w", err)
	}
	return nil
}

// AppendPathPage appends a vector path page to a new or existing document body.
func (doc *Document) AppendPathPage(
	documentIndex int,
	commands string,
	boundary [4]float64,
	pageWidthMM, pageHeightMM float64,
	fill, stroke bool,
) error {
	doc.mu.Lock()
	defer doc.mu.Unlock()
	if err := doc.requireOpen(); err != nil {
		return err
	}
	if documentIndex < 0 {
		return errors.New("negative document index")
	}
	commandsBytes, err := cString(commands)
	if err != nil {
		return err
	}
	var fillValue, strokeValue int32
	if fill {
		fillValue = 1
	}
	if stroke {
		strokeValue = 1
	}
	runtime.LockOSThread()
	defer runtime.UnlockOSThread()
	if err := nativeError(api.appendPathPage(
		doc.handle,
		uintptr(documentIndex),
		&commandsBytes[0],
		boundary[0],
		boundary[1],
		boundary[2],
		boundary[3],
		pageWidthMM,
		pageHeightMM,
		fillValue,
		strokeValue,
	)); err != nil {
		return fmt.Errorf("append path page: %w", err)
	}
	return nil
}

// AppendImagePage appends a PNG, JPEG, BMP, single-page TIFF, or JBIG2 image page.
func (doc *Document) AppendImagePage(
	documentIndex int,
	data []byte,
	format string,
	widthMM, heightMM float64,
) error {
	doc.mu.Lock()
	defer doc.mu.Unlock()
	if err := doc.requireOpen(); err != nil {
		return err
	}
	if documentIndex < 0 {
		return errors.New("negative document index")
	}
	formatBytes, err := cString(format)
	if err != nil {
		return err
	}
	var dataPtr *byte
	if len(data) > 0 {
		dataPtr = &data[0]
	}
	runtime.LockOSThread()
	defer runtime.UnlockOSThread()
	if err := nativeError(api.appendImagePage(
		doc.handle,
		uintptr(documentIndex),
		dataPtr,
		uintptr(len(data)),
		&formatBytes[0],
		widthMM,
		heightMM,
	)); err != nil {
		return fmt.Errorf("append image page: %w", err)
	}
	runtime.KeepAlive(data)
	return nil
}

// AddImagePage appends a page with one PNG, JPEG, BMP, single-page TIFF, or JBIG2 image.
func (doc *Document) AddImagePage(data []byte, format string, widthMM, heightMM float64) error {
	doc.mu.Lock()
	defer doc.mu.Unlock()
	if err := doc.requireOpen(); err != nil {
		return err
	}
	formatBytes, err := cString(format)
	if err != nil {
		return err
	}
	var dataPtr *byte
	if len(data) > 0 {
		dataPtr = &data[0]
	}
	runtime.LockOSThread()
	defer runtime.UnlockOSThread()
	if err := nativeError(api.addImagePage(doc.handle, dataPtr, uintptr(len(data)), &formatBytes[0], widthMM, heightMM)); err != nil {
		return fmt.Errorf("add image page: %w", err)
	}
	runtime.KeepAlive(data)
	return nil
}

// AddImageObject adds a PNG or JPEG image to a page.
func (doc *Document) AddImageObject(pageIndex int, data []byte, format string, boundary [4]float64) (int, error) {
	doc.mu.Lock()
	defer doc.mu.Unlock()
	if err := doc.requireOpen(); err != nil {
		return 0, err
	}
	if pageIndex < 0 {
		return 0, errors.New("negative page index")
	}
	formatBytes, err := cString(format)
	if err != nil {
		return 0, err
	}
	var dataPtr *byte
	if len(data) > 0 {
		dataPtr = &data[0]
	}
	runtime.LockOSThread()
	defer runtime.UnlockOSThread()
	var id uintptr
	if err := nativeError(api.addImageObject(doc.handle, uintptr(pageIndex), dataPtr, uintptr(len(data)), &formatBytes[0], boundary[0], boundary[1], boundary[2], boundary[3], &id)); err != nil {
		return 0, fmt.Errorf("add image object: %w", err)
	}
	runtime.KeepAlive(data)
	return int(id), nil
}

// AddCompositeImageObject adds an image inside a composite graphic resource.
func (doc *Document) AddCompositeImageObject(pageIndex int, data []byte, format string, boundary [4]float64) (int, error) {
	doc.mu.Lock()
	defer doc.mu.Unlock()
	if err := doc.requireOpen(); err != nil {
		return 0, err
	}
	if pageIndex < 0 {
		return 0, errors.New("negative page index")
	}
	formatBytes, err := cString(format)
	if err != nil {
		return 0, err
	}
	var dataPtr *byte
	if len(data) > 0 {
		dataPtr = &data[0]
	}
	runtime.LockOSThread()
	defer runtime.UnlockOSThread()
	var id uintptr
	if err := nativeError(api.addCompositeImageObject(doc.handle, uintptr(pageIndex), dataPtr, uintptr(len(data)), &formatBytes[0], boundary[0], boundary[1], boundary[2], boundary[3], &id)); err != nil {
		return 0, fmt.Errorf("add composite image object: %w", err)
	}
	runtime.KeepAlive(data)
	return int(id), nil
}

// AddPathPage appends a page with one OFD vector path.
func (doc *Document) AddPathPage(commands string, boundary [4]float64, pageWidthMM, pageHeightMM float64, fill, stroke bool) error {
	doc.mu.Lock()
	defer doc.mu.Unlock()
	if err := doc.requireOpen(); err != nil {
		return err
	}
	commandBytes, err := cString(commands)
	if err != nil {
		return err
	}
	var fillInt, strokeInt int32
	if fill {
		fillInt = 1
	}
	if stroke {
		strokeInt = 1
	}
	runtime.LockOSThread()
	defer runtime.UnlockOSThread()
	if err := nativeError(api.addPathPage(doc.handle, &commandBytes[0], boundary[0], boundary[1], boundary[2], boundary[3], pageWidthMM, pageHeightMM, fillInt, strokeInt)); err != nil {
		return fmt.Errorf("add path page: %w", err)
	}
	return nil
}

// AddMultimediaResource adds an Image, Audio or Video resource to a document body.
func (doc *Document) AddMultimediaResource(documentIndex int, mediaType, format string, data []byte) (int, error) {
	doc.mu.Lock()
	defer doc.mu.Unlock()
	if err := doc.requireOpen(); err != nil {
		return 0, err
	}
	if documentIndex < 0 {
		return 0, errors.New("negative document index")
	}
	media, err := cString(mediaType)
	if err != nil {
		return 0, err
	}
	formatValue, err := cString(format)
	if err != nil {
		return 0, err
	}
	var pointer *byte
	if len(data) > 0 {
		pointer = &data[0]
	}
	runtime.LockOSThread()
	defer runtime.UnlockOSThread()
	var id uintptr
	if err := nativeError(api.addMultimediaResource(doc.handle, uintptr(documentIndex), &media[0], &formatValue[0], pointer, uintptr(len(data)), &id)); err != nil {
		return 0, fmt.Errorf("add multimedia resource: %w", err)
	}
	runtime.KeepAlive(data)
	return int(id), nil
}

// AddPageMultimediaResource adds a page-local Image, Audio or Video resource.
func (doc *Document) AddPageMultimediaResource(pageIndex int, mediaType, format string, data []byte) (int, error) {
	doc.mu.Lock()
	defer doc.mu.Unlock()
	if err := doc.requireOpen(); err != nil {
		return 0, err
	}
	if pageIndex < 0 {
		return 0, errors.New("negative page index")
	}
	media, err := cString(mediaType)
	if err != nil {
		return 0, err
	}
	formatValue, err := cString(format)
	if err != nil {
		return 0, err
	}
	var pointer *byte
	if len(data) > 0 {
		pointer = &data[0]
	}
	runtime.LockOSThread()
	defer runtime.UnlockOSThread()
	var id uintptr
	if err := nativeError(api.addPageMultimediaResource(doc.handle, uintptr(pageIndex), &media[0], &formatValue[0], pointer, uintptr(len(data)), &id)); err != nil {
		return 0, fmt.Errorf("add page multimedia resource: %w", err)
	}
	runtime.KeepAlive(data)
	return int(id), nil
}

// ReplaceDocumentMultimediaData replaces resource bytes without changing its OFD ID.
func (doc *Document) ReplaceDocumentMultimediaData(documentIndex int, resourceID string, data []byte) error {
	doc.mu.Lock()
	defer doc.mu.Unlock()
	if err := doc.requireOpen(); err != nil {
		return err
	}
	if documentIndex < 0 {
		return errors.New("negative document index")
	}
	id, err := cString(resourceID)
	if err != nil {
		return err
	}
	var pointer *byte
	if len(data) > 0 {
		pointer = &data[0]
	}
	runtime.LockOSThread()
	defer runtime.UnlockOSThread()
	if err := nativeError(api.replaceDocumentMultimediaData(doc.handle, uintptr(documentIndex), &id[0], pointer, uintptr(len(data)))); err != nil {
		return fmt.Errorf("replace document multimedia data: %w", err)
	}
	runtime.KeepAlive(data)
	return nil
}

// ReplacePageMultimediaData replaces the resource visible to a page by ID.
func (doc *Document) ReplacePageMultimediaData(pageIndex int, resourceID string, data []byte) error {
	doc.mu.Lock()
	defer doc.mu.Unlock()
	if err := doc.requireOpen(); err != nil {
		return err
	}
	if pageIndex < 0 {
		return errors.New("negative page index")
	}
	id, err := cString(resourceID)
	if err != nil {
		return err
	}
	var pointer *byte
	if len(data) > 0 {
		pointer = &data[0]
	}
	runtime.LockOSThread()
	defer runtime.UnlockOSThread()
	if err := nativeError(api.replacePageMultimediaData(doc.handle, uintptr(pageIndex), &id[0], pointer, uintptr(len(data)))); err != nil {
		return fmt.Errorf("replace page multimedia data: %w", err)
	}
	runtime.KeepAlive(data)
	return nil
}

// RemoveDocumentMultimediaResource removes an unreferenced resource definition.
func (doc *Document) RemoveDocumentMultimediaResource(documentIndex int, resourceID string) error {
	doc.mu.Lock()
	defer doc.mu.Unlock()
	if err := doc.requireOpen(); err != nil {
		return err
	}
	if documentIndex < 0 {
		return errors.New("negative document index")
	}
	id, err := cString(resourceID)
	if err != nil {
		return err
	}
	runtime.LockOSThread()
	defer runtime.UnlockOSThread()
	if err := nativeError(api.removeDocumentMultimedia(doc.handle, uintptr(documentIndex), &id[0])); err != nil {
		return fmt.Errorf("remove document multimedia resource: %w", err)
	}
	return nil
}

// RemovePageMultimediaResource removes an unreferenced page-local definition.
func (doc *Document) RemovePageMultimediaResource(pageIndex int, resourceID string) error {
	doc.mu.Lock()
	defer doc.mu.Unlock()
	if err := doc.requireOpen(); err != nil {
		return err
	}
	if pageIndex < 0 {
		return errors.New("negative page index")
	}
	id, err := cString(resourceID)
	if err != nil {
		return err
	}
	runtime.LockOSThread()
	defer runtime.UnlockOSThread()
	if err := nativeError(api.removePageMultimedia(doc.handle, uintptr(pageIndex), &id[0])); err != nil {
		return fmt.Errorf("remove page multimedia resource: %w", err)
	}
	return nil
}

// AddAttachment embeds a file in a newly created document.
func (doc *Document) AddAttachment(name string, data []byte) error {
	doc.mu.Lock()
	defer doc.mu.Unlock()
	if err := doc.requireOpen(); err != nil {
		return err
	}
	nameBytes, err := cString(name)
	if err != nil {
		return err
	}
	var dataPtr *byte
	if len(data) > 0 {
		dataPtr = &data[0]
	}
	runtime.LockOSThread()
	defer runtime.UnlockOSThread()
	if err := nativeError(api.addAttachment(doc.handle, &nameBytes[0], dataPtr, uintptr(len(data)))); err != nil {
		return fmt.Errorf("add attachment: %w", err)
	}
	runtime.KeepAlive(data)
	return nil
}

// AddAttachmentToDocument embeds a file in the selected document body.
func (doc *Document) AddAttachmentToDocument(documentIndex int, name string, data []byte) error {
	doc.mu.Lock()
	defer doc.mu.Unlock()
	if err := doc.requireOpen(); err != nil {
		return err
	}
	if documentIndex < 0 {
		return errors.New("document index must be nonnegative")
	}
	nameBytes, err := cString(name)
	if err != nil {
		return err
	}
	var dataPtr *byte
	if len(data) > 0 {
		dataPtr = &data[0]
	}
	runtime.LockOSThread()
	defer runtime.UnlockOSThread()
	if err := nativeError(api.addAttachmentToDocument(
		doc.handle,
		uintptr(documentIndex),
		&nameBytes[0],
		dataPtr,
		uintptr(len(data)),
	)); err != nil {
		return fmt.Errorf("add attachment to document: %w", err)
	}
	runtime.KeepAlive(data)
	return nil
}

// RemoveAttachment removes one attachment by global index, retaining its package bytes.
func (doc *Document) RemoveAttachment(index int) error {
	doc.mu.Lock()
	defer doc.mu.Unlock()
	if err := doc.requireOpen(); err != nil {
		return err
	}
	if index < 0 {
		return errors.New("negative attachment index")
	}
	runtime.LockOSThread()
	defer runtime.UnlockOSThread()
	if err := nativeError(api.removeAttachment(doc.handle, uintptr(index))); err != nil {
		return fmt.Errorf("remove attachment: %w", err)
	}
	return nil
}

// RenameAttachment changes an attachment's name while preserving its ID and bytes.
func (doc *Document) RenameAttachment(index int, name string) error {
	doc.mu.Lock()
	defer doc.mu.Unlock()
	if err := doc.requireOpen(); err != nil {
		return err
	}
	if index < 0 {
		return errors.New("negative attachment index")
	}
	encoded, err := cString(name)
	if err != nil {
		return err
	}
	runtime.LockOSThread()
	defer runtime.UnlockOSThread()
	if err := nativeError(api.renameAttachment(doc.handle, uintptr(index), &encoded[0])); err != nil {
		return fmt.Errorf("rename attachment: %w", err)
	}
	return nil
}

// ReplaceAttachmentData replaces attachment bytes while preserving its ID.
func (doc *Document) ReplaceAttachmentData(index int, data []byte) error {
	doc.mu.Lock()
	defer doc.mu.Unlock()
	if err := doc.requireOpen(); err != nil {
		return err
	}
	if index < 0 {
		return errors.New("negative attachment index")
	}
	var dataPtr *byte
	if len(data) > 0 {
		dataPtr = &data[0]
	}
	runtime.LockOSThread()
	defer runtime.UnlockOSThread()
	if err := nativeError(api.replaceAttachmentData(doc.handle, uintptr(index), dataPtr, uintptr(len(data)))); err != nil {
		return fmt.Errorf("replace attachment data: %w", err)
	}
	runtime.KeepAlive(data)
	return nil
}

// SetAttachmentMetadata replaces an attachment's optional standard metadata.
func (doc *Document) SetAttachmentMetadata(index int, metadata AttachmentMetadata) error {
	doc.mu.Lock()
	defer doc.mu.Unlock()
	if err := doc.requireOpen(); err != nil {
		return err
	}
	if index < 0 {
		return errors.New("negative attachment index")
	}
	payload, err := json.Marshal(metadata)
	if err != nil {
		return fmt.Errorf("encode attachment metadata: %w", err)
	}
	encoded, err := cString(string(payload))
	if err != nil {
		return err
	}
	runtime.LockOSThread()
	defer runtime.UnlockOSThread()
	if err := nativeError(api.setAttachmentMetadata(doc.handle, uintptr(index), &encoded[0])); err != nil {
		return fmt.Errorf("set attachment metadata: %w", err)
	}
	return nil
}

// Save writes the package to an OFD file.
func (doc *Document) Save(path string) error {
	doc.mu.Lock()
	defer doc.mu.Unlock()
	if err := doc.requireOpen(); err != nil {
		return err
	}
	pathBytes, err := cString(path)
	if err != nil {
		return err
	}
	runtime.LockOSThread()
	defer runtime.UnlockOSThread()
	if err := nativeError(api.save(doc.handle, &pathBytes[0])); err != nil {
		return fmt.Errorf("save ofd: %w", err)
	}
	return nil
}

// ToBytes serializes the package to OFD ZIP bytes.
func (doc *Document) ToBytes() ([]byte, error) {
	doc.mu.Lock()
	defer doc.mu.Unlock()
	if err := doc.requireOpen(); err != nil {
		return nil, err
	}
	runtime.LockOSThread()
	defer runtime.UnlockOSThread()
	var buffer nativeBuffer
	if err := nativeError(api.toBytes(doc.handle, &buffer)); err != nil {
		return nil, fmt.Errorf("serialize ofd: %w", err)
	}
	defer api.bufferFree(buffer)
	return bytesAt(buffer)
}
