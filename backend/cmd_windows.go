//go:build windows

package backend

import (
	"os/exec"
	"syscall"
)

// hideWindow sets the command to run without showing a console window on Windows
func hideWindow(cmd *exec.Cmd) {
	cmd.SysProcAttr = &syscall.SysProcAttr{
		HideWindow:    true,
		CreationFlags: 0x08000000, // CREATE_NO_WINDOW
	}
}
