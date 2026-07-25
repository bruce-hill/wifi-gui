PREFIX ?= /usr/local
TARGET_DIR := $(shell cd src-tauri && cargo metadata --format-version 1 --no-deps 2>/dev/null | sed -n 's/.*"target_directory":"\([^"]*\)".*/\1/p')

all: build

build:
	cd src-tauri && cargo build --release

run:
	cd src-tauri && cargo run

install: build
	install -Dm755 "$(TARGET_DIR)/release/wifi-gui" "$(DESTDIR)$(PREFIX)/bin/wifi-gui"

clean:
	cd src-tauri && cargo clean

.PHONY: all build run install clean
