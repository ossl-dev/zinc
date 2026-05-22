package zinc

import (
	"fmt"
	"testing"
)

func TestCreateAndBytes(t *testing.T) {
	name := fmt.Sprintf("gotest_%d", testing.Short())
	r, err := Create(name, 4096)
	if err != nil {
		t.Skipf("create failed (core lib may not be built): %v", err)
	}
	defer r.Close()

	data := r.Bytes()
	if len(data) != 4096 {
		t.Fatalf("expected 4096 bytes, got %d", len(data))
	}
	data[0] = 0xAB
	if data[0] != 0xAB {
		t.Fatal("write/read mismatch")
	}
}

func TestOpenAndRead(t *testing.T) {
	name := fmt.Sprintf("gotest_open_%d", testing.Short())
	owner, err := Create(name, 4096)
	if err != nil {
		t.Skipf("create failed: %v", err)
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
	name := fmt.Sprintf("gotest_notify_%d", testing.Short())
	region, err := Create(name, 4096)
	if err != nil {
		t.Skipf("create failed: %v", err)
	}
	defer region.Close()

	done := make(chan bool)
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
	name := fmt.Sprintf("gotest_dup_%d", testing.Short())
	r, err := Create(name, 4096)
	if err != nil {
		t.Skipf("create failed: %v", err)
	}
	defer r.Close()

	_, err = Create(name, 4096)
	if err == nil {
		t.Fatal("expected error for duplicate create")
	}
}
