PLUGINS_DIR ?= $(shell pkg-config --variable=pluginsdir gstreamer-1.0)

OS=$(shell uname -s)
ifeq ($(OS),Linux)
  SO_SUFFIX=so
else
ifeq ($(OS),Darwin)
  SO_SUFFIX=dylib
else
  # FIXME: Bad hack, how to know we're on Windows?
  SO_SUFFIX=dll
endif
endif

ifeq ($(DEBUG),1)
  CARGO_FLAGS=
  BUILD_DIR=target/debug
else
  CARGO_FLAGS=--release
  BUILD_DIR=target/release
endif

all: build

build:
	cargo build --all $(CARGO_FLAGS)

install: build
	install -d $(DESTDIR)$(PLUGINS_DIR)
	install -m 755 $(BUILD_DIR)/*.$(SO_SUFFIX) $(DESTDIR)$(PLUGINS_DIR)

clean:
	cargo clean

