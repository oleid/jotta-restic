#!/bin/sh
#
# please note: single-threaded tests are required, since
# 1) I start an actix instance per thread
# 2) tests depend on each other
#
RUST_TEST_THREADS=1 cargo test
