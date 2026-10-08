package zinc

import (
	"fmt"
	"os"
	"testing"
)

func BenchmarkCopyAt(b *testing.B) {
	for _, size := range []int{64, os.Getpagesize()} {
		b.Run(fmt.Sprint(size), func(b *testing.B) {
			region, err := Create(testName(), uint(os.Getpagesize()))
			if err != nil {
				b.Fatal(err)
			}
			b.Cleanup(region.Close)
			buf := make([]byte, size)
			b.SetBytes(int64(2 * size))
			b.ReportAllocs()
			b.ResetTimer()
			for range b.N {
				if _, err := region.WriteAt(buf, 0); err != nil {
					b.Fatal(err)
				}
				if _, err := region.ReadAt(buf, 0); err != nil {
					b.Fatal(err)
				}
			}
		})
	}
}
