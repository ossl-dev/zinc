package zinc

import (
	"fmt"
	"os"
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
