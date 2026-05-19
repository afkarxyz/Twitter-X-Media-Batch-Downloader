//go:build windows

package backend

import (
	"os/exec"
	"strconv"
	"syscall"
)

func hideWindow(cmd *exec.Cmd) {
	cmd.SysProcAttr = &syscall.SysProcAttr{
		HideWindow:    true,
		CreationFlags: 0x08000000,
	}
}

func terminateCommandProcess(cmd *exec.Cmd) error {
	if cmd == nil || cmd.Process == nil {
		return nil
	}

	taskkill := exec.Command("taskkill", "/T", "/F", "/PID", strconv.Itoa(cmd.Process.Pid))
	hideWindow(taskkill)
	if err := taskkill.Run(); err == nil {
		return nil
	}

	return cmd.Process.Kill()
}
