PREFIX ?= /usr/local
TARGET_DIR := $(shell cd src-tauri && cargo metadata --format-version 1 --no-deps 2>/dev/null | sed -n 's/.*"target_directory":"\([^"]*\)".*/\1/p')
TSC := node_modules/.bin/tsc

all: build

$(TSC): package.json
	npm install

# The frontend is TypeScript compiled to ui/main.js, which index.html loads.
ui/main.js: ui/main.ts ui/types.d.ts tsconfig.json $(TSC)
	$(TSC)

build: ui/main.js
	cd src-tauri && cargo build --release

run: ui/main.js
	cd src-tauri && cargo run

install: build
	install -Dm755 "$(TARGET_DIR)/release/wifi-gui" "$(DESTDIR)$(PREFIX)/bin/wifi-gui"

clean:
	cd src-tauri && cargo clean
	rm -f ui/main.js

.PHONY: all build run install clean
