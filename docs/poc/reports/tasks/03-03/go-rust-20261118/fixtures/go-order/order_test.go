package fixture

import "testing"

type queue struct {
	items []int
}

func (q *queue) add(value int) {
	q.items = append(q.items, value)
}

func (q *queue) length() int {
	return len(q.items)
}

var shared queue

func TestSeed(t *testing.T) {
	shared.add(7)
	if got := shared.length(); got != 1 {
		t.Fatalf("seeded queue length = %d; want 1", got)
	}
}

func TestConsume(t *testing.T) {
	if got := shared.length(); got != 1 {
		t.Fatalf("shared-state dependency: queue length = %d; want 1", got)
	}
}
