// Minimal report receiver for 01-04 E3 (test environment only; not product code).
//
// POST /reports  Authorization: Bearer <token>  body: JSON with "report_id"
//
//	new id -> 201, same id + same body -> 200 (replay, stored once), same id + different body -> 409
//
// GET  /reports/{id}  Authorization: Bearer <token>  -> 200 stored body | 404
// Missing or wrong token -> 401. Every accepted/rejected request is appended to /data/receiver.log.
package main

import (
	"crypto/sha256"
	"crypto/subtle"
	"encoding/hex"
	"encoding/json"
	"fmt"
	"io"
	"log"
	"net/http"
	"os"
	"strings"
	"sync"
	"time"
)

type entry struct {
	Digest string
	Body   []byte
}

var (
	mu    sync.Mutex
	store = map[string]entry{}
	token []byte
	logf  *os.File
)

func record(r *http.Request, code int, id string) {
	fmt.Fprintf(logf, "%s %s %s id=%q -> %d\n", time.Now().UTC().Format(time.RFC3339Nano), r.Method, r.URL.Path, id, code)
}

func authorized(r *http.Request) bool {
	h := r.Header.Get("Authorization")
	if !strings.HasPrefix(h, "Bearer ") {
		return false
	}
	return subtle.ConstantTimeCompare([]byte(strings.TrimPrefix(h, "Bearer ")), token) == 1
}

func reports(w http.ResponseWriter, r *http.Request) {
	if !authorized(r) {
		record(r, 401, "")
		http.Error(w, `{"error":"UNAUTHORIZED"}`, 401)
		return
	}
	switch {
	case r.Method == http.MethodPost && r.URL.Path == "/reports":
		body, err := io.ReadAll(io.LimitReader(r.Body, 1<<20))
		var m map[string]any
		if err != nil || json.Unmarshal(body, &m) != nil {
			record(r, 400, "")
			http.Error(w, `{"error":"BAD_REQUEST"}`, 400)
			return
		}
		id, _ := m["report_id"].(string)
		if id == "" {
			record(r, 400, "")
			http.Error(w, `{"error":"REPORT_ID_REQUIRED"}`, 400)
			return
		}
		sum := sha256.Sum256(body)
		d := hex.EncodeToString(sum[:])
		mu.Lock()
		old, ok := store[id]
		if !ok {
			store[id] = entry{d, body}
		}
		mu.Unlock()
		switch {
		case !ok:
			record(r, 201, id)
			w.WriteHeader(201)
			fmt.Fprintf(w, `{"report_id":%q,"stored":true,"digest":%q}`, id, d)
		case old.Digest == d:
			record(r, 200, id)
			fmt.Fprintf(w, `{"report_id":%q,"replay":true,"digest":%q}`, id, d)
		default:
			record(r, 409, id)
			w.WriteHeader(409)
			fmt.Fprintf(w, `{"error":"EVENT_CONTENT_CONFLICT","report_id":%q}`, id)
		}
	case r.Method == http.MethodGet && strings.HasPrefix(r.URL.Path, "/reports/"):
		id := strings.TrimPrefix(r.URL.Path, "/reports/")
		mu.Lock()
		e, ok := store[id]
		n := len(store)
		mu.Unlock()
		if !ok {
			record(r, 404, id)
			http.Error(w, `{"error":"NOT_FOUND"}`, 404)
			return
		}
		record(r, 200, id)
		w.Header().Set("X-Stored-Count", fmt.Sprint(n))
		w.Write(e.Body)
	default:
		record(r, 405, "")
		http.Error(w, `{"error":"METHOD_NOT_ALLOWED"}`, 405)
	}
}

func main() {
	t, err := os.ReadFile("/run/rx/token")
	if err != nil {
		log.Fatal(err)
	}
	token = []byte(strings.TrimSpace(string(t)))
	logf, err = os.OpenFile("/data/receiver.log", os.O_CREATE|os.O_APPEND|os.O_WRONLY, 0o600)
	if err != nil {
		log.Fatal(err)
	}
	http.HandleFunc("/reports", reports)
	http.HandleFunc("/reports/", reports)
	log.Fatal(http.ListenAndServe(":8080", nil))
}
