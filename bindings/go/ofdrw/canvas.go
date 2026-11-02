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
type CanvasCommand struct {
	Type       string          `json:"type"`
	Point      *[2]float64     `json:"point,omitempty"`
	Control    *[2]float64     `json:"control,omitempty"`
	Control1   *[2]float64     `json:"control1,omitempty"`
	Control2   *[2]float64     `json:"control2,omitempty"`
	End        *[2]float64     `json:"end,omitempty"`
	Radii      *[2]float64     `json:"radii,omitempty"`
	Angle      *float64        `json:"angle,omitempty"`
	Large      bool            `json:"large,omitempty"`
	Sweep      bool            `json:"sweep,omitempty"`
	Center     *[2]float64     `json:"center,omitempty"`
	Radius     *float64        `json:"radius,omitempty"`
	StartAngle *float64        `json:"start_angle,omitempty"`
	EndAngle   *float64        `json:"end_angle,omitempty"`
	Rect       *[4]float64     `json:"rect,omitempty"`
	EvenOdd    bool            `json:"even_odd,omitempty"`
	Offset     *[2]float64     `json:"offset,omitempty"`
	Factors    *[2]float64     `json:"factors,omitempty"`
	Matrix     *[6]float64     `json:"matrix,omitempty"`
	Style      *DrawParamStyle `json:"style,omitempty"`
	Width      *float64        `json:"width,omitempty"`
	Color      *[3]uint8       `json:"color,omitempty"`
	Alpha      *float64        `json:"alpha,omitempty"`
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
