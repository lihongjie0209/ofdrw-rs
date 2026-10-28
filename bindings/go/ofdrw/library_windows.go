//go:build windows

package ofdrw

import "syscall"

func openLibrary(path string) (uintptr, error) {
	library, err := syscall.LoadDLL(path)
	if err != nil {
		return 0, err
	}
	return uintptr(library.Handle), nil
}

func lookupSymbol(library uintptr, name string) (uintptr, error) {
	return syscall.GetProcAddress(syscall.Handle(library), name)
}
