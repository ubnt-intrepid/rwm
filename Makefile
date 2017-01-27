.PHONY: all build clean fmt restart

SERVICE	:= vncserver@:2.service

all: build

build:
	cargo build

clean:
	cargo clean

fmt:
	cargo fmt

restart:
	systemctl --user restart $(SERVICE) || true
