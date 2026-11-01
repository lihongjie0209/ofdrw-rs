package ofdrw

import (
	"encoding/json"
	"testing"
)

func TestZeroWidthPlaceholderSerializesAsAReservation(t *testing.T) {
	data, err := json.Marshal(NewPlaceholderSpan(0, 4))
	if err != nil {
		t.Fatal(err)
	}
	var fields map[string]json.RawMessage
	if err := json.Unmarshal(data, &fields); err != nil {
		t.Fatal(err)
	}
	if string(fields["placeholder_width"]) != "0" || string(fields["text"]) != `""` {
		t.Fatalf("zero-width placeholder lost reservation: %s", data)
	}
	if string(fields["font_size"]) != "4" {
		t.Fatalf("height lost: %s", data)
	}
}
