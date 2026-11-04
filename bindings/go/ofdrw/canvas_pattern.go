package ofdrw

import (
	"encoding/json"
	"errors"
	"fmt"
	"runtime"
)

// CanvasPatternOptions configures image tiling. Nil density defaults to 3.78;
// empty repetition uses repeat. Row/column modes reflect alternating cells.
type CanvasPatternOptions struct {
	Repetition  string      `json:"repetition,omitempty"`
	PixelsPerMM *float64    `json:"pixels_per_mm,omitempty"`
	ImageSize   *[2]float64 `json:"image_size,omitempty"`
	CTM         *[6]float64 `json:"ctm,omitempty"`
}

// CreateCanvasPattern adds an image resource and returns reusable fill/stroke
// settings without painting a page object. Errors preserve document bytes.
func (doc *Document) CreateCanvasPattern(
	pageIndex int,
	data []byte,
	format string,
	options CanvasPatternOptions,
) (PatternStyle, error) {
	doc.mu.Lock()
	defer doc.mu.Unlock()
	if err := doc.requireOpen(); err != nil {
		return PatternStyle{}, err
	}
	if pageIndex < 0 {
		return PatternStyle{}, errors.New("negative page index")
	}
	formatBytes, err := cString(format)
	if err != nil {
		return PatternStyle{}, err
	}
	payload, err := json.Marshal(options)
	if err != nil {
		return PatternStyle{}, fmt.Errorf("encode canvas pattern options: %w", err)
	}
	request, err := cString(string(payload))
	if err != nil {
		return PatternStyle{}, err
	}
	var dataPtr *byte
	if len(data) > 0 {
		dataPtr = &data[0]
	}
	runtime.LockOSThread()
	defer runtime.UnlockOSThread()
	var output nativeBuffer
	status := api.createCanvasPattern(
		doc.handle,
		uintptr(pageIndex),
		dataPtr,
		uintptr(len(data)),
		&formatBytes[0],
		&request[0],
		&output,
	)
	runtime.KeepAlive(data)
	runtime.KeepAlive(formatBytes)
	runtime.KeepAlive(request)
	if err := nativeError(status); err != nil {
		return PatternStyle{}, fmt.Errorf("create canvas pattern: %w", err)
	}
	defer api.bufferFree(output)
	encoded, err := bytesAt(output)
	if err != nil {
		return PatternStyle{}, err
	}
	var pattern PatternStyle
	if err := json.Unmarshal(encoded, &pattern); err != nil {
		return PatternStyle{}, fmt.Errorf("decode canvas pattern: %w", err)
	}
	return pattern, nil
}
