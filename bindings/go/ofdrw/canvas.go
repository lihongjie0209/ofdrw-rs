package ofdrw

import (
	"encoding/json"
	"errors"
	"fmt"
	"runtime"
)

// CanvasCommand is a tagged vector drawing operation. Type uses snake_case,
// for example "move_to", "rect", "stroke", or "save". Populate only the fields
// for that operation; the native parser rejects unrelated fields. Coordinates
// are millimeters and angles are degrees. Pointers preserve explicit zero values.
// Boolean flags default false. A nil Alpha clears global opacity.
// Image commands encode Data as base64. A nil PixelsPerMM uses Java's 3.78 default.
type CanvasCommand struct {
	Type        string          `json:"type"`
	Font        *CanvasFont     `json:"font,omitempty"`
	Text        *string         `json:"text,omitempty"`
	Data        []byte          `json:"data_base64,omitempty"`
	Format      string          `json:"format,omitempty"`
	Source      *[4]float64     `json:"source,omitempty"`
	PixelsPerMM *float64        `json:"pixels_per_mm,omitempty"`
	Point       *[2]float64     `json:"point,omitempty"`
	Control     *[2]float64     `json:"control,omitempty"`
	Control1    *[2]float64     `json:"control1,omitempty"`
	Control2    *[2]float64     `json:"control2,omitempty"`
	End         *[2]float64     `json:"end,omitempty"`
	Radii       *[2]float64     `json:"radii,omitempty"`
	Angle       *float64        `json:"angle,omitempty"`
	Large       bool            `json:"large,omitempty"`
	Sweep       bool            `json:"sweep,omitempty"`
	Center      *[2]float64     `json:"center,omitempty"`
	Radius      *float64        `json:"radius,omitempty"`
	StartAngle  *float64        `json:"start_angle,omitempty"`
	EndAngle    *float64        `json:"end_angle,omitempty"`
	Rect        *[4]float64     `json:"rect,omitempty"`
	EvenOdd     bool            `json:"even_odd,omitempty"`
	Offset      *[2]float64     `json:"offset,omitempty"`
	Factors     *[2]float64     `json:"factors,omitempty"`
	Matrix      *[6]float64     `json:"matrix,omitempty"`
	Style       *DrawParamStyle `json:"style,omitempty"`
	Width       *float64        `json:"width,omitempty"`
	Color       *[3]uint8       `json:"color,omitempty"`
	Alpha       *float64        `json:"alpha,omitempty"`
}

// DrawCanvas draws an atomic vector batch on an existing global page and returns
// painted object IDs. It starts with fresh state. Nil commands are an empty batch.
func (doc *Document) DrawCanvas(pageIndex int, boundary [4]float64, commands []CanvasCommand) ([]string, error) {
	doc.mu.Lock()
	defer doc.mu.Unlock()
	if err := doc.requireOpen(); err != nil {
		return nil, err
	}
	if pageIndex < 0 {
		return nil, errors.New("negative page index")
	}
	if commands == nil {
		commands = []CanvasCommand{}
	}
	payload, err := json.Marshal(struct {
		Boundary [4]float64      `json:"boundary"`
		Commands []CanvasCommand `json:"commands"`
	}{Boundary: boundary, Commands: commands})
	if err != nil {
		return nil, fmt.Errorf("encode canvas batch: %w", err)
	}
	request, err := cString(string(payload))
	if err != nil {
		return nil, err
	}
	runtime.LockOSThread()
	defer runtime.UnlockOSThread()
	var buffer nativeBuffer
	if err := nativeError(api.drawCanvas(
		doc.handle,
		uintptr(pageIndex),
		&request[0],
		&buffer,
	)); err != nil {
		return nil, fmt.Errorf("draw canvas: %w", err)
	}
	defer api.bufferFree(buffer)
	data, err := bytesAt(buffer)
	if err != nil {
		return nil, err
	}
	var ids []string
	if err := json.Unmarshal(data, &ids); err != nil {
		return nil, fmt.Errorf("decode canvas result: %w", err)
	}
	return ids, nil
}

// CanvasFont configures unshaped canvas text. Nil fields use native defaults:
// SimSun, 1 mm, weight 400, zero tracking/directions, and start alignment.
// Pointers preserve explicit zero values for native validation.
type CanvasFont struct {
	FontID        string   `json:"font_id,omitempty"`
	Size          *float64 `json:"size,omitempty"`
	LetterSpacing *float64 `json:"letter_spacing,omitempty"`
	Weight        *uint16  `json:"weight,omitempty"`
	Italic        bool     `json:"italic,omitempty"`
	ReadDirection *uint16  `json:"read_direction,omitempty"`
	CharDirection *uint16  `json:"char_direction,omitempty"`
	Align         string   `json:"align,omitempty"`
}

// CanvasTextMetrics contains logical width, first glyph offset, and signed
// increments along the reading axis. It excludes kerning and glyph ink bounds.
type CanvasTextMetrics struct {
	Width       float64    `json:"width"`
	FirstOffset [2]float64 `json:"first_offset"`
	Offsets     []float64  `json:"offsets"`
}

// MeasureCanvasText measures without modifying pages or allocating fonts.
func (doc *Document) MeasureCanvasText(pageIndex int, text string, font CanvasFont) (CanvasTextMetrics, error) {
	doc.mu.Lock()
	defer doc.mu.Unlock()
	if err := doc.requireOpen(); err != nil {
		return CanvasTextMetrics{}, err
	}
	if pageIndex < 0 {
		return CanvasTextMetrics{}, errors.New("negative page index")
	}
	payload, err := json.Marshal(struct {
		Text string     `json:"text"`
		Font CanvasFont `json:"font"`
	}{Text: text, Font: font})
	if err != nil {
		return CanvasTextMetrics{}, fmt.Errorf("encode canvas text measurement: %w", err)
	}
	request, err := cString(string(payload))
	if err != nil {
		return CanvasTextMetrics{}, err
	}
	runtime.LockOSThread()
	defer runtime.UnlockOSThread()
	var buffer nativeBuffer
	if err := nativeError(api.measureCanvasText(
		doc.handle,
		uintptr(pageIndex),
		&request[0],
		&buffer,
	)); err != nil {
		return CanvasTextMetrics{}, fmt.Errorf("measure canvas text: %w", err)
	}
	defer api.bufferFree(buffer)
	data, err := bytesAt(buffer)
	if err != nil {
		return CanvasTextMetrics{}, err
	}
	var metrics CanvasTextMetrics
	if err := json.Unmarshal(data, &metrics); err != nil {
		return CanvasTextMetrics{}, fmt.Errorf("decode canvas text measurement: %w", err)
	}
	return metrics, nil
}
