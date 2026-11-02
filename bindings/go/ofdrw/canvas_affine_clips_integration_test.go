//go:build integration

package ofdrw

import (
	"bytes"
	"encoding/json"
	"image/png"
	"os"
	"path/filepath"
	"strconv"
	"testing"
)

type affineClipRaster struct {
	width  int
	height int
	pixels [][3]uint8
}

func decodeAffineClipRaster(t *testing.T, data []byte) affineClipRaster {
	t.Helper()
	image, err := png.Decode(bytes.NewReader(data))
	if err != nil {
		t.Fatal(err)
	}
	bounds := image.Bounds()
	result := affineClipRaster{
		width:  bounds.Dx(),
		height: bounds.Dy(),
		pixels: make([][3]uint8, 0, bounds.Dx()*bounds.Dy()),
	}
	for y := bounds.Min.Y; y < bounds.Max.Y; y++ {
		for x := bounds.Min.X; x < bounds.Max.X; x++ {
			r, g, b, a := image.At(x, y).RGBA()
			if a != 65535 {
				t.Fatal("expected opaque page raster")
			}
			result.pixels = append(result.pixels, [3]uint8{uint8(r / 257), uint8(g / 257), uint8(b / 257)})
		}
	}
	return result
}

func assertAffineClipRasterMatches(t *testing.T, actualPNG, expectedPNG []byte) {
	t.Helper()
	actual, expected := decodeAffineClipRaster(t, actualPNG), decodeAffineClipRaster(t, expectedPNG)
	if actual.width != expected.width || actual.height != expected.height {
		t.Fatal("raster dimensions differ")
	}
	width, height := actual.width, actual.height
	offsets := []int{-width - 1, -width, -width + 1, -1, 0, 1, width - 1, width, width + 1}
	white := [3]uint8{255, 255, 255}
	errorSum, samples := 0, 0
	for pass := 0; pass < 2; pass++ {
		first, second := actual.pixels, expected.pixels
		if pass == 1 {
			first, second = second, first
		}
		for y := 1; y < height-1; y++ {
			for x := 1; x < width-1; x++ {
				index := y*width + x
				hasWhite := false
				for _, offset := range offsets {
					if first[index+offset] == white {
						hasWhite = true
						break
					}
				}
				if hasWhite {
					continue
				}
				best := 766
				for _, offset := range offsets {
					difference := 0
					for c := 0; c < 3; c++ {
						delta := int(first[index][c]) - int(second[index+offset][c])
						if delta < 0 {
							delta = -delta
						}
						difference += delta
					}
					if difference < best {
						best = difference
					}
					if best == 0 {
						break
					}
				}
				errorSum += best
				samples += 3
			}
		}
	}
	if samples <= 100 || float64(errorSum)/float64(samples) >= 4 {
		t.Fatalf("raster color error %d over %d samples", errorSum, samples)
	}
	extra := 0
	for y := 1; y < height-1; y++ {
		for x := 1; x < width-1; x++ {
			index := y*width + x
			allWhite := true
			for _, offset := range offsets {
				if expected.pixels[index+offset] != white {
					allWhite = false
					break
				}
			}
			pixel := actual.pixels[index]
			if allWhite && (pixel[0] < 248 || pixel[1] < 248 || pixel[2] < 248) {
				extra++
			}
		}
	}
	if float64(extra)/float64(width*height) >= 0.005 {
		t.Fatalf("unexpected paint over %d white interior pixels", extra)
	}
}

func TestCanvasAffineClipsMatchIndependentJava2D(t *testing.T) {
	root := filepath.Join("..", "..", "..", "crates", "ofdrw", "tests", "fixtures", "canvas-affine-clips-java")
	data, err := os.ReadFile(filepath.Join(root, "reference.json"))
	if err != nil {
		t.Fatal(err)
	}
	var reference struct {
		Cases []struct {
			Name     string          `json:"name"`
			Commands []CanvasCommand `json:"commands"`
		} `json:"cases"`
	}
	if err := json.Unmarshal(data, &reference); err != nil {
		t.Fatal(err)
	}
	if len(reference.Cases) != 5 {
		t.Fatalf("scene count: %d", len(reference.Cases))
	}
	for _, entry := range reference.Cases {
		t.Run(entry.Name, func(t *testing.T) {
			commands := []CanvasCommand{}
			for _, command := range entry.Commands {
				if command.Type == "fill_rect" {
					color := command.Color
					command.Color = nil
					width := 0.353
					commands = append(commands, CanvasCommand{Type: "set_line_width", Width: &width},
						CanvasCommand{Type: "set_fill_color", Color: color}, CanvasCommand{Type: "set_stroke_color", Color: color})
				}
				if command.Type == "circle_arc" {
					start, end := 0., 360.
					command.StartAngle, command.EndAngle, command.Sweep = &start, &end, true
				}
				commands = append(commands, command)
			}
			saved, err := json.Marshal(commands)
			if err != nil {
				t.Fatal(err)
			}
			doc, err := New()
			if err != nil {
				t.Fatal(err)
			}
			defer doc.Close()
			if err := doc.AddBlankPage(40, 30); err != nil {
				t.Fatal(err)
			}
			ids, err := doc.DrawCanvas(0, [4]float64{4, 3, 28, 22}, commands)
			if err != nil {
				t.Fatal(err)
			}
			if len(ids) == 0 {
				t.Fatal("no painted objects")
			}
			after, err := json.Marshal(commands)
			if err != nil {
				t.Fatal(err)
			}
			if !bytes.Equal(after, saved) {
				t.Fatal("canvas commands mutated")
			}
			encoded, err := doc.ToBytes()
			if err != nil {
				t.Fatal(err)
			}
			reopened, err := FromBytes(encoded)
			if err != nil {
				t.Fatal(err)
			}
			defer reopened.Close()
			for _, ppm := range []int{4, 12} {
				pngData, err := doc.PagePNG(0, float64(ppm)*25.4)
				if err != nil {
					t.Fatal(err)
				}
				expected, err := os.ReadFile(filepath.Join(root, entry.Name+"-geometry-"+strconv.Itoa(ppm)+".png"))
				if err != nil {
					t.Fatal(err)
				}
				assertAffineClipRasterMatches(t, pngData, expected)
				roundTrip, err := reopened.PagePNG(0, float64(ppm)*25.4)
				if err != nil {
					t.Fatal(err)
				}
				if !bytes.Equal(pngData, roundTrip) {
					t.Fatal("saved/reopened raster changed")
				}
			}
			issues, err := reopened.ValidateReferences()
			if err != nil {
				t.Fatal(err)
			}
			if len(issues) != 0 {
				t.Fatalf("invalid references: %+v", issues)
			}
		})
	}
}
