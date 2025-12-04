//go:build !windows

package backend

import "os/exec"

// hideWindow is a no-op on non-Windows platforms
func hideWindow(cmd *exec.Cmd) {
	// No action needed on Unix-like systems
}
