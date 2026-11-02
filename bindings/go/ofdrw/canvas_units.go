package ofdrw

import (
	"fmt"
	"runtime"
)

// CanvasPixelsToMM converts signed integer pixels to millimeters. Density must
// be finite and positive; pass 3.78 for Java's default. Tiny densities may overflow.
func CanvasPixelsToMM(pixels int32, pixelsPerMM float64) (float64, error) {
	if err := loadAPI(); err != nil {
		return 0, err
	}
	runtime.LockOSThread()
	defer runtime.UnlockOSThread()
	var output float64
	if err := nativeError(api.canvasPixelsToMM(pixels, pixelsPerMM, &output)); err != nil {
		return 0, fmt.Errorf("convert canvas pixels to millimeters: %w", err)
	}
	return output, nil
}

// CanvasMMToPixels truncates toward zero and saturates to signed 32-bit pixels.
// NaN becomes zero. Density must be finite and positive; Java's default is 3.78.
func CanvasMMToPixels(mm, pixelsPerMM float64) (int32, error) {
	if err := loadAPI(); err != nil {
		return 0, err
	}
	runtime.LockOSThread()
	defer runtime.UnlockOSThread()
	var output int32
	if err := nativeError(api.canvasMMToPixels(mm, pixelsPerMM, &output)); err != nil {
		return 0, fmt.Errorf("convert canvas millimeters to pixels: %w", err)
	}
	return output, nil
}
