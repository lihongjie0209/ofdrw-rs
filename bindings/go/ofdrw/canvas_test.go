package ofdrw

import (
	"encoding/json"
	"math"
	"testing"
)

func TestCanvasCommandJSONPreservesExplicitZeroesAndOmitsUnusedFields(t *testing.T) {
	zero := 0.0
	for _, tc := range []struct {
		name     string
		command  CanvasCommand
		expected string
	}{
		{
			name:     "width",
			command:  CanvasCommand{Type: "set_line_width", Width: &zero},
			expected: `{"type":"set_line_width","width":0}`,
		},
		{
			name:     "opacity",
			command:  CanvasCommand{Type: "set_global_alpha", Alpha: &zero},
			expected: `{"type":"set_global_alpha","alpha":0}`,
		},
		{
			name:     "clear opacity",
			command:  CanvasCommand{Type: "set_global_alpha"},
			expected: `{"type":"set_global_alpha"}`,
		},
		{
			name:     "stroke",
			command:  CanvasCommand{Type: "stroke"},
			expected: `{"type":"stroke"}`,
		},
		{
			name:     "origin",
			command:  CanvasCommand{Type: "move_to", Point: &[2]float64{}},
			expected: `{"type":"move_to","point":[0,0]}`,
		},
		{
			name:     "black",
			command:  CanvasCommand{Type: "set_fill_color", Color: &[3]uint8{}},
			expected: `{"type":"set_fill_color","color":[0,0,0]}`,
		},
	} {
		t.Run(tc.name, func(t *testing.T) {
			data, err := json.Marshal(tc.command)
			if err != nil {
				t.Fatal(err)
			}
			if string(data) != tc.expected {
				t.Fatalf("JSON = %s, want %s", data, tc.expected)
			}
		})
	}
	invalid := math.NaN()
	if _, err := json.Marshal(CanvasCommand{Type: "set_global_alpha", Alpha: &invalid}); err == nil {
		t.Fatal("nonfinite opacity must not become a clear-opacity command")
	}
}
