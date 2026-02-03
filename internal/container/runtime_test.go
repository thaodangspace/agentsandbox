package container

import (
	"testing"
)

func TestValidatePortMapping(t *testing.T) {
	tests := []struct {
		name    string
		port    string
		wantErr bool
	}{
		// Valid cases
		{"single port", "8080", false},
		{"host:container", "8080:80", false},
		{"ip:host:container", "127.0.0.1:8080:80", false},
		{"ip:host:container ipv6 bracketed", "[::1]:8080:80", false},
		{"ipv6 full bracketed", "[2001:db8::1]:8080:80", false},
		{"high port number", "65535", false},
		{"port range valid", "3000:3000", false},
		{"low port 1", "1", false},

		// Invalid cases
		{"empty string", "", true},
		{"invalid port letter", "abc", true},
		{"invalid host port", "abc:80", true},
		{"invalid container port", "8080:abc", true},
		{"invalid ip", "999.999.999.999:8080:80", true},
		{"too many colons", "1:2:3:4", true},
		{"negative port", "-1", true},
		{"port too high", "70000", true},
		{"port zero", "0", true},
		{"port 65536", "65536", true},
		{"ipv6 without brackets", "::1:8080:80", true},
	}

	for _, tt := range tests {
		t.Run(tt.name, func(t *testing.T) {
			err := validatePortMapping(tt.port)
			if (err != nil) != tt.wantErr {
				t.Errorf("validatePortMapping(%q) error = %v, wantErr %v", tt.port, err, tt.wantErr)
			}
		})
	}
}
