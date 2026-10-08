package zinc

import (
	"bytes"
	"errors"
	"fmt"
	"io"
	"math"
	"os"
	"sync"
	"sync/atomic"
	"testing"
)

var testID uint32

func testName() string {
	return fmt.Sprintf("go_%d_%d", os.Getpid(), atomic.AddUint32(&testID, 1))
}

func TestCreateAndBytes(t *testing.T) {
	name := testName()
	r, err := Create(name, uint(os.Getpagesize()))
	if err != nil {
		t.Fatalf("create failed (core lib may not be built): %v", err)
	}
	defer r.Close()

	data := r.Bytes()
	if len(data) != os.Getpagesize() {
		t.Fatalf("expected uint(os.Getpagesize()) bytes, got %d", len(data))
	}
	data[0] = 0xAB
	if data[0] != 0xAB {
		t.Fatal("write/read mismatch")
	}
}

func TestOpenAndRead(t *testing.T) {
	name := testName()
	owner, err := Create(name, uint(os.Getpagesize()))
	if err != nil {
		t.Fatalf("create failed: %v", err)
	}
	defer owner.Close()

	copy(owner.Bytes()[:4], []byte("ZINC"))

	reader, err := Open(name)
	if err != nil {
		t.Fatalf("open failed: %v", err)
	}
	defer reader.Close()

	if string(reader.Bytes()[:4]) != "ZINC" {
		t.Fatal("data mismatch")
	}
}

func TestNotifyWait(t *testing.T) {
	name := testName()
	region, err := Create(name, uint(os.Getpagesize()))
	if err != nil {
		t.Fatalf("create failed: %v", err)
	}
	defer region.Close()

	done := make(chan bool, 1)
	go func() {
		r2, err := Open(name)
		if err != nil {
			t.Errorf("open failed: %v", err)
			done <- false
			return
		}
		defer r2.Close()
		r2.Bytes()[0] = 42
		r2.Notify()
		done <- true
	}()

	if !region.Wait(5000) {
		t.Fatal("wait timed out")
	}
	if region.Bytes()[0] != 42 {
		t.Fatal("data mismatch after notify")
	}
	<-done
}

func TestOpenNonexistent(t *testing.T) {
	_, err := Open("nonexistent_go_test_region_xyz")
	if err == nil {
		t.Fatal("expected error for nonexistent region")
	}
}

func TestCreateDuplicate(t *testing.T) {
	name := testName()
	r, err := Create(name, uint(os.Getpagesize()))
	if err != nil {
		t.Fatalf("create failed: %v", err)
	}
	defer r.Close()

	_, err = Create(name, uint(os.Getpagesize()))
	if err == nil {
		t.Fatal("expected error for duplicate create")
	}
}

func TestPendingAndClosedHandle(t *testing.T) {
	region, err := Create(testName(), uint(os.Getpagesize()))
	if err != nil {
		t.Fatal(err)
	}
	if region.TryWait() {
		t.Fatal("unexpected notification")
	}
	region.Notify()
	if !region.TryWait() {
		t.Fatal("missing notification")
	}
	if region.Wait(0) {
		t.Fatal("unexpected notification")
	}
	region.Close()
	region.Close()
	if len(region.Bytes()) != 0 {
		t.Fatal("closed handle has data")
	}
	if _, err := Create("bad\x00name", uint(os.Getpagesize())); err == nil {
		t.Fatal("NUL accepted")
	}
}

func TestStreamInterfaces(t *testing.T) {
	region, err := Create(testName(), uint(os.Getpagesize()))
	if err != nil {
		t.Fatal(err)
	}
	defer region.Close()
	var stream io.ReadWriteSeeker = region
	var readerAt io.ReaderAt = region
	var writerAt io.WriterAt = region
	if n, err := io.Copy(stream, bytes.NewBufferString("ZINC")); n != 4 || err != nil {
		t.Fatalf("copy: %d, %v", n, err)
	}
	if _, err := writerAt.WriteAt([]byte("_"), 1); err != nil {
		t.Fatal(err)
	}
	if position, err := stream.Seek(0, io.SeekCurrent); position != 4 || err != nil {
		t.Fatalf("WriteAt changed position: %d, %v", position, err)
	}
	buf := make([]byte, 4)
	if n, err := readerAt.ReadAt(buf, 0); n != 4 || err != nil || string(buf) != "Z_NC" {
		t.Fatalf("ReadAt: %d, %v, %q", n, err, buf)
	}
	if position, err := stream.Seek(-4, io.SeekCurrent); position != 0 || err != nil {
		t.Fatalf("seek: %d, %v", position, err)
	}
	if _, err := io.ReadFull(stream, buf); err != nil || string(buf) != "Z_NC" {
		t.Fatalf("ReadFull: %v, %q", err, buf)
	}
	if region.TryWait() {
		t.Fatal("data access notified implicitly")
	}
}

func TestStreamBoundaries(t *testing.T) {
	region, err := Create(testName(), uint(os.Getpagesize()))
	if err != nil {
		t.Fatal(err)
	}
	defer region.Close()
	capacity := int64(os.Getpagesize())
	if position, err := region.Seek(-1, io.SeekEnd); position != capacity-1 || err != nil {
		t.Fatalf("seek end: %d, %v", position, err)
	}
	if n, err := region.Write([]byte("ab")); n != 1 || !errors.Is(err, io.ErrShortWrite) {
		t.Fatalf("short write: %d, %v", n, err)
	}
	if n, err := region.Read(make([]byte, 1)); n != 0 || !errors.Is(err, io.EOF) {
		t.Fatalf("end read: %d, %v", n, err)
	}
	buf := []byte{'_', '_'}
	if n, err := region.ReadAt(buf, capacity-1); n != 1 || !errors.Is(err, io.EOF) || string(buf) != "a_" {
		t.Fatalf("partial read: %d, %v, %q", n, err, buf)
	}
	for _, offset := range []int64{capacity, capacity + 1, math.MaxInt64} {
		if n, err := region.WriteAt([]byte("z"), offset); n != 0 || !errors.Is(err, io.ErrShortWrite) {
			t.Fatalf("write at %d: %d, %v", offset, n, err)
		}
		if n, err := region.ReadAt(buf, offset); n != 0 || !errors.Is(err, io.EOF) {
			t.Fatalf("read at %d: %d, %v", offset, n, err)
		}
	}
	if _, err := region.ReadAt(buf, -1); !errors.Is(err, ErrInvalidOffset) {
		t.Fatalf("negative read: %v", err)
	}
	if _, err := region.WriteAt(buf, -1); !errors.Is(err, ErrInvalidOffset) {
		t.Fatalf("negative write: %v", err)
	}
	for _, seek := range []struct {
		offset int64
		whence int
	}{{-1, io.SeekStart}, {capacity + 1, io.SeekStart}, {math.MaxInt64, io.SeekCurrent}, {math.MinInt64, io.SeekEnd}, {0, 99}} {
		if position, err := region.Seek(seek.offset, seek.whence); position != capacity || !errors.Is(err, ErrInvalidOffset) {
			t.Fatalf("invalid seek changed position: %d, %v", position, err)
		}
	}
	if n, err := region.Read(nil); n != 0 || err != nil {
		t.Fatalf("empty read: %d, %v", n, err)
	}
	if n, err := region.Write(nil); n != 0 || err != nil {
		t.Fatalf("empty write: %d, %v", n, err)
	}
}

func TestCopiesSharePositionAndClose(t *testing.T) {
	region, err := Create(testName(), uint(os.Getpagesize()))
	if err != nil {
		t.Fatal(err)
	}
	defer region.Close()
	copied := *region
	if _, err := copied.Write([]byte("a")); err != nil {
		t.Fatal(err)
	}
	if position, err := region.Seek(0, io.SeekCurrent); position != 1 || err != nil {
		t.Fatalf("copy position: %d, %v", position, err)
	}
	copied.Close()
	region.Close()
	for _, r := range []*SharedRegion{region, &copied, {}, nil} {
		r.Close()
		if len(r.Bytes()) != 0 || r.TryWait() || r.Wait(0) {
			t.Fatal("closed region is still accessible")
		}
		r.Notify()
		if _, err := r.Read(nil); !errors.Is(err, io.ErrClosedPipe) {
			t.Fatalf("closed read: %v", err)
		}
		if _, err := r.Write(nil); !errors.Is(err, io.ErrClosedPipe) {
			t.Fatalf("closed write: %v", err)
		}
		if _, err := r.ReadAt(nil, 0); !errors.Is(err, io.ErrClosedPipe) {
			t.Fatalf("closed ReadAt: %v", err)
		}
		if _, err := r.WriteAt(nil, 0); !errors.Is(err, io.ErrClosedPipe) {
			t.Fatalf("closed WriteAt: %v", err)
		}
		if _, err := r.Seek(0, io.SeekStart); !errors.Is(err, io.ErrClosedPipe) {
			t.Fatalf("closed seek: %v", err)
		}
	}
}

func TestConcurrentCloseAndOperations(t *testing.T) {
	region, err := Create(testName(), uint(os.Getpagesize()))
	if err != nil {
		t.Fatal(err)
	}
	defer region.Close()
	var wg sync.WaitGroup
	for range 16 {
		wg.Add(1)
		go func(r SharedRegion) {
			defer wg.Done()
			buf := make([]byte, 4)
			for range 32 {
				r.ReadAt(buf, 0)
				r.WriteAt(buf, 0)
				r.Notify()
				r.TryWait()
				r.Wait(0)
			}
			r.Close()
		}(*region)
	}
	wg.Wait()
}
