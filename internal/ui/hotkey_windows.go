//go:build windows

package ui

import (
	"fmt"
	"runtime"
	"syscall"

	"github.com/lxn/win"
)

const (
	hotkeyID        = 0xA71A
	modAlt          = 0x0001
	modControl      = 0x0002
	virtualKeySpace = 0x20
	wmHotkey        = 0x0312
)

var (
	user32               = syscall.NewLazyDLL("user32.dll")
	procRegisterHotKey   = user32.NewProc("RegisterHotKey")
	procUnregisterHotKey = user32.NewProc("UnregisterHotKey")
	procPostThreadMsg    = user32.NewProc("PostThreadMessageW")
)

// startHotkey 注册 Ctrl+Alt+Space。消息循环阻塞等待系统消息，不做轮询，常驻 CPU 几乎为零。
func startHotkey(onToggle func(), onError func(error)) func() {
	stop := make(chan struct{})
	ready := make(chan struct{})
	var threadID uint32
	go func() {
		runtime.LockOSThread()
		defer runtime.UnlockOSThread()
		threadID = win.GetCurrentThreadId()
		ret, _, err := procRegisterHotKey.Call(0, hotkeyID, modControl|modAlt, virtualKeySpace)
		if ret == 0 && onError != nil {
			onError(fmt.Errorf("全局快捷键 Ctrl+Alt+Space 注册失败，可能已被其他程序占用: %v", err))
		}
		close(ready)
		var msg win.MSG
		for {
			select {
			case <-stop:
				procUnregisterHotKey.Call(0, hotkeyID)
				return
			default:
			}
			if win.GetMessage(&msg, 0, 0, 0) <= 0 {
				procUnregisterHotKey.Call(0, hotkeyID)
				return
			}
			if msg.Message == wmHotkey && msg.WParam == uintptr(hotkeyID) {
				onToggle()
				continue
			}
			win.TranslateMessage(&msg)
			win.DispatchMessage(&msg)
		}
	}()
	<-ready
	return func() {
		close(stop)
		// 发送空消息唤醒 GetMessage，使热键线程能及时退出。
		if threadID != 0 {
			procPostThreadMsg.Call(uintptr(threadID), uintptr(win.WM_NULL), 0, 0)
		}
	}
}
