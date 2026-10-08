package zinc

/*
#cgo LDFLAGS: -lzinc_core -L${SRCDIR}/../../target/release
#include "../../include/zinc.h"
#include <stdlib.h>
*/
import "C"
import (
	"errors"
	"fmt"
	"io"
	"strings"
	"sync"
	"unsafe"
)

var ErrInvalidOffset = errors.New("invalid shared-memory offset")

// SharedRegion copies share a handle and stream position. Close invalidates all copies.
type SharedRegion struct {
	state *regionState
}

type regionState struct {
	mu     sync.RWMutex
	ioMu   sync.Mutex
	handle C.ZincHandle
	data   []byte
	offset int64
}

func Create(name string, capacity uint) (*SharedRegion, error) {
	if strings.IndexByte(name, 0) >= 0 {
		return nil, fmt.Errorf("name contains NUL")
	}
	var h C.ZincHandle
	cname := C.CString(name)
	defer C.free(unsafe.Pointer(cname))
	if code := C.zinc_create(cname, C.uintptr_t(capacity), &h); code != 0 {
		return nil, fmt.Errorf("zinc_create: %d", code)
	}
	return newRegion(h), nil
}

func Open(name string) (*SharedRegion, error) {
	if strings.IndexByte(name, 0) >= 0 {
		return nil, fmt.Errorf("name contains NUL")
	}
	var h C.ZincHandle
	cname := C.CString(name)
	defer C.free(unsafe.Pointer(cname))
	if code := C.zinc_open(cname, &h); code != 0 {
		return nil, fmt.Errorf("zinc_open: %d", code)
	}
	return newRegion(h), nil
}

func newRegion(handle C.ZincHandle) *SharedRegion {
	data := unsafe.Slice((*byte)(C.zinc_ptr(handle)), C.zinc_capacity(handle))
	return &SharedRegion{state: &regionState{handle: handle, data: data}}
}

func (r *SharedRegion) acquire() *regionState {
	if r == nil || r.state == nil {
		return nil
	}
	s := r.state
	s.mu.RLock()
	if s.handle == nil {
		s.mu.RUnlock()
		return nil
	}
	return s
}

// Bytes borrows the mapping. Do not close any copy of the region while using it.
func (r *SharedRegion) Bytes() []byte {
	s := r.acquire()
	if s == nil {
		return nil
	}
	defer s.mu.RUnlock()
	return s.data
}

func (r *SharedRegion) Notify() {
	s := r.acquire()
	if s == nil {
		return
	}
	defer s.mu.RUnlock()
	C.zinc_notify(s.handle)
}

func (r *SharedRegion) Wait(timeoutMs uint32) bool {
	s := r.acquire()
	if s == nil {
		return false
	}
	defer s.mu.RUnlock()
	return C.zinc_wait(s.handle, C.uint(timeoutMs)) == 0
}

func (r *SharedRegion) TryWait() bool {
	s := r.acquire()
	if s == nil {
		return false
	}
	defer s.mu.RUnlock()
	return C.zinc_try_wait(s.handle) == 0
}

// Read copies bytes from the current position and advances it.
func (r *SharedRegion) Read(p []byte) (int, error) {
	s := r.acquire()
	if s == nil {
		return 0, io.ErrClosedPipe
	}
	defer s.mu.RUnlock()
	s.ioMu.Lock()
	defer s.ioMu.Unlock()
	n, err := readAt(s.data, p, s.offset)
	s.offset += int64(n)
	return n, err
}

// Write copies bytes at the current position and advances it. It does not notify.
func (r *SharedRegion) Write(p []byte) (int, error) {
	s := r.acquire()
	if s == nil {
		return 0, io.ErrClosedPipe
	}
	defer s.mu.RUnlock()
	s.ioMu.Lock()
	defer s.ioMu.Unlock()
	n, err := writeAt(s.data, p, s.offset)
	s.offset += int64(n)
	return n, err
}

// ReadAt copies bytes without changing the stream position.
func (r *SharedRegion) ReadAt(p []byte, offset int64) (int, error) {
	s := r.acquire()
	if s == nil {
		return 0, io.ErrClosedPipe
	}
	defer s.mu.RUnlock()
	s.ioMu.Lock()
	defer s.ioMu.Unlock()
	return readAt(s.data, p, offset)
}

// WriteAt copies bytes without changing the stream position. It does not notify.
func (r *SharedRegion) WriteAt(p []byte, offset int64) (int, error) {
	s := r.acquire()
	if s == nil {
		return 0, io.ErrClosedPipe
	}
	defer s.mu.RUnlock()
	s.ioMu.Lock()
	defer s.ioMu.Unlock()
	return writeAt(s.data, p, offset)
}

// Seek sets the position within [0, capacity]. The region cannot grow.
func (r *SharedRegion) Seek(offset int64, whence int) (int64, error) {
	s := r.acquire()
	if s == nil {
		return 0, io.ErrClosedPipe
	}
	defer s.mu.RUnlock()
	s.ioMu.Lock()
	defer s.ioMu.Unlock()
	capacity := int64(len(s.data))
	var base int64
	switch whence {
	case io.SeekStart:
	case io.SeekCurrent:
		base = s.offset
	case io.SeekEnd:
		base = capacity
	default:
		return s.offset, ErrInvalidOffset
	}
	if offset < -base || offset > capacity-base {
		return s.offset, ErrInvalidOffset
	}
	s.offset = base + offset
	return s.offset, nil
}

func readAt(data, p []byte, offset int64) (int, error) {
	if offset < 0 {
		return 0, ErrInvalidOffset
	}
	if len(p) == 0 {
		return 0, nil
	}
	if offset >= int64(len(data)) {
		return 0, io.EOF
	}
	n := copy(p, data[int(offset):])
	if n < len(p) {
		return n, io.EOF
	}
	return n, nil
}

func writeAt(data, p []byte, offset int64) (int, error) {
	if offset < 0 {
		return 0, ErrInvalidOffset
	}
	if len(p) == 0 {
		return 0, nil
	}
	if offset >= int64(len(data)) {
		return 0, io.ErrShortWrite
	}
	n := copy(data[int(offset):], p)
	if n < len(p) {
		return n, io.ErrShortWrite
	}
	return n, nil
}

// Close waits for active operations and releases the handle once.
func (r *SharedRegion) Close() {
	if r == nil || r.state == nil {
		return
	}
	s := r.state
	s.mu.Lock()
	defer s.mu.Unlock()
	if s.handle != nil {
		C.zinc_close(s.handle)
		s.handle = nil
		s.data = nil
	}
}
