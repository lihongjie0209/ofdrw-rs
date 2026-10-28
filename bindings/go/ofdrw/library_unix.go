//go:build linux || darwin

package ofdrw

import "github.com/ebitengine/purego"

func openLibrary(path string) (uintptr, error) {
	return purego.Dlopen(path, purego.RTLD_NOW|purego.RTLD_LOCAL)
}

func lookupSymbol(library uintptr, name string) (uintptr, error) {
	return purego.Dlsym(library, name)
}
