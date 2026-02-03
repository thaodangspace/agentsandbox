package container

import "testing"

func TestIsContainerForDir(t *testing.T) {
	tests := []struct {
		name      string
		container string
		dir       string
		want      bool
	}{
		{"exact match", "agentsandbox-myproject", "myproject", true},
		{"prefixed suffix", "agentsandbox-myproject-123", "myproject", true},
		{"legacy format", "agentsandbox-claude-myproject-987", "myproject", true},
		{"wrong dir", "agentsandbox-other", "myproject", false},
		{"non agentsandbox", "other-myproject", "myproject", false},
		{"empty dir", "agentsandbox-something", "", false},
	}

	for _, tt := range tests {
		t.Run(tt.name, func(t *testing.T) {
			if got := isContainerForDir(tt.container, tt.dir); got != tt.want {
				t.Fatalf("isContainerForDir(%q, %q) = %v, want %v", tt.container, tt.dir, got, tt.want)
			}
		})
	}
}
