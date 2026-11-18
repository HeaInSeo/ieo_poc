#!/usr/bin/env bash
# The outer launcher binds fixtures under /policy and starts in /scratch.
set -euo pipefail
export CARGO_TERM_COLOR=never
go version | tee results/go-version.txt | grep -Fx 'go version go1.26.10 linux/amd64'
rustc --version | tee results/rust-version.txt | grep -E '^rustc 1\.99\.1 '
cargo --version | tee results/cargo-version.txt | grep -E '^cargo 1\.99\.1 '
cargo fmt --version | tee results/rustfmt-version.txt | grep -E '^rustfmt 1\.10\.0-stable '
cargo clippy --version | tee results/clippy-version.txt | grep -E '^clippy 0\.1\.99 '
gcc -dumpversion | tee results/gcc-version.txt
test ! -e /home
awk -F: 'NR > 2 {gsub(/ /,"",$1); if ($1 != "lo") exit 1}' /proc/net/dev
printf '%s\n' 'Isolation: private scratch, no mounted host home, loopback only'
cp -r /policy/fixtures/{go,go-order,rust} .

run_case() {
  local name=$1 expected=$2 marker=$3 actual=0
  shift 3
  /usr/bin/time -f 'elapsed_seconds=%e peak_rss_kib=%M' -o "/scratch/results/$name.time" \
    timeout -k 2s 120s "$@" >"/scratch/results/$name.log" 2>&1 || actual=$?
  if [[ "$actual" -ne "$expected" ]] || ! grep -Eq -- "$marker" "/scratch/results/$name.log"; then
    printf 'FAIL %s actual_exit=%s expected_exit=%s\n' "$name" "$actual" "$expected"
    cat "/scratch/results/$name.log"
    return 1
  fi
  printf 'PASS %s exit=%s %s\n' "$name" "$actual" "$(tail -1 "/scratch/results/$name.time")"
}

(
  cd go
  run_case go-format 0 '^format clean$' /bin/bash -ec 'test -z "$(gofmt -l .)"; echo format clean'
  run_case go-vet 0 '^vet clean$' /bin/bash -ec 'go vet -mod=readonly ./...; echo vet clean'
  run_case go-test 0 '"Action":"pass".*"Test":"TestDouble"' go test -mod=readonly -count=1 -json ./...
  run_case go-race 0 '^ok[[:space:]]' go test -mod=readonly -race -count=1 ./...
  cp double.go baseline.txt
  sed -i 's/2 \* value/3 * value/' double.go
  run_case go-assertion 1 'Double\(3\) = 9; want 6' go test -mod=readonly -count=1 -json ./...
  mv baseline.txt double.go
  mv double_test.go baseline.txt
  sed '/func TestDouble(t \*testing.T) {/a\	t.Skip("synthetic required skip")' baseline.txt >double_test.go
  run_case go-required-skip 0 '"Action":"skip".*"Test":"TestDouble"' go test -mod=readonly -count=1 -json ./...
  if grep -Eq '"Action":"pass".*"Test":"TestDouble"' /scratch/results/go-required-skip.log; then exit 1; fi
  mv baseline.txt double_test.go
  cat > badfmt.go <<'GO'
package fixture
func badlyFormatted(){ println("an intentionally unformatted example") }
GO
  run_case go-format-failure 1 '^badfmt.go$' /bin/bash -ec 'files=$(gofmt -l .); printf "%s\n" "$files"; test -z "$files"'
  rm badfmt.go
  cat >> double.go <<'GO'

func vetFailure() {
    var n int
    _ = fmt.Sprintf("%s", n)
}
GO
  sed -i '/^package fixture$/a\
\nimport "fmt"' double.go
  run_case go-vet-failure 1 'format %s has arg n of wrong type int' go vet -mod=readonly ./...
)

(
  cd go-order
  run_case go-order-control 0 '^PASS$' go test -mod=readonly -count=1 -v ./...
  run_case go-order-shuffle 1 'shared-state dependency' go test -mod=readonly -count=1 -shuffle=2 -v ./...
  grep -Fx -- '-test.shuffle 2' /scratch/results/go-order-shuffle.log >/dev/null
  sed -i -e '/^var shared queue$/d' \
    -e '/^func TestSeed(t \*testing.T) {/a\	var shared queue' \
    -e '/^func TestConsume(t \*testing.T) {/a\	var shared queue\n	shared.add(7)' order_test.go
  run_case go-order-corrected 0 '^--- PASS: TestConsume' go test -mod=readonly -count=1 -shuffle=2 -v ./...
  grep -Fx -- '-test.shuffle 2' /scratch/results/go-order-corrected.log >/dev/null
)

(
  cd rust
  run_case rust-format 0 '^format clean$' /bin/bash -ec 'cargo fmt --all -- --check; echo format clean'
  run_case rust-clippy 0 'Finished' cargo clippy --offline --locked --all-targets -- -D warnings
  run_case rust-test 0 'test result: ok\. 1 passed; 0 failed; 0 ignored' cargo test --offline --locked --all-targets
  cp src/lib.rs baseline.txt
  sed -i 's/2 \* value/3 * value/' src/lib.rs
  run_case rust-assertion 101 'assertion .left == right. failed' cargo test --offline --locked --all-targets
  sed '/    #\[test\]/a\    #[ignore = "synthetic required skip"]' baseline.txt >src/lib.rs
  run_case rust-required-ignore 0 'test result: ok\. 0 passed; 0 failed; 1 ignored' cargo test --offline --locked --all-targets
  cat >src/lib.rs <<'RUST'
pub fn double(value:i32)->i32{2*value}
RUST
  run_case rust-format-failure 1 'Diff in .*src/lib.rs' cargo fmt --all -- --check
  cp baseline.txt src/lib.rs
  cat >>src/lib.rs <<'RUST'

pub fn literal_empty() -> bool {
    let text = "";
    text.len() == 0
}
RUST
  run_case rust-clippy-failure 101 'clippy::len_zero' cargo clippy --offline --locked --all-targets -- -D warnings
)
