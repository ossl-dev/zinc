package zinc

/*
#cgo LDFLAGS: -lzinc_core -L${SRCDIR}/../../core/target/release
#include "../../include/zinc.h"
#include <stdlib.h>
*/
import "C"
import (
	"fmt"
	"unsafe"
)

type SharedRegion struct {
	handle C.ZincHandle
}

func Create(name string, capacity uint) (*SharedRegion, error) {
	var h C.ZincHandle
	cname := C.CString(name)
	defer C.free(unsafe.Pointer(cname))
	if code := C.zinc_create(cname, C.ulong(capacity), &h); code != 0 {
		return nil, fmt.Errorf("zinc_create: %d", code)
	}
	return &SharedRegion{h}, nil
}

func Open(name string) (*SharedRegion, error) {
	var h C.ZincHandle
	cname := C.CString(name)
	defer C.free(unsafe.Pointer(cname))
	if code := C.zinc_open(cname, &h); code != 0 {
		return nil, fmt.Errorf("zinc_open: %d", code)
	}
	return &SharedRegion{h}, nil
}

// Bytes returns a Go slice backed directly by shared memory — zero copy.
func (r *SharedRegion) Bytes() []byte {
	ptr := C.zinc_ptr(r.handle)
	cap := C.zinc_capacity(r.handle)
	return unsafe.Slice((*byte)(ptr), cap)
}

func (r *SharedRegion) Notify() {
	C.zinc_notify(r.handle)
}

func (r *SharedRegion) Wait(timeoutMs uint32) bool {
	return C.zinc_wait(r.handle, C.uint(timeoutMs)) == 0
}

func (r *SharedRegion) Close() {
	C.zinc_close(r.handle)
	r.handle = nil
}
