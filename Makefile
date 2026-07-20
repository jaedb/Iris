REGISTRY    ?= k8s1:31320
IMAGE       ?= mopidy_stable
TAG         ?= latest
BUILDER     ?= multi-arch
NAMESPACE   ?= snapserver
# Build-cache image stored alongside the main image in the registry.
# Persists layer cache across builder-container restarts.
CACHE_REF   ?= $(REGISTRY)/$(IMAGE):buildcache

# Upstream clone locations — kept outside this repo to avoid nested-git conflicts.
LIBRESPOT_UPSTREAM_DIR  ?= ../librespot-upstream
LIBRESPOT_UPSTREAM_URL  := https://github.com/librespot-org/librespot.git
GST_UPSTREAM_DIR        ?= ../gst-plugins-rs-upstream
GST_UPSTREAM_URL        := https://github.com/kingosticks/gst-plugins-rs.git
GST_UPSTREAM_BRANCH     := spotify-logging

# Our own Iris fork (github.com/FlatterAtMainz/Iris), already cloned as a
# sibling checkout. Unlike librespot/gst-plugins-rs, there's no separate
# patches/ layer for it: it's our fork, so local changes live directly in
# its own git history rather than as patch files here.
IRIS_UPSTREAM_DIR       ?= ../Iris
IRIS_UPSTREAM_URL       := https://github.com/FlatterAtMainz/Iris.git

.PHONY: build push deploy bootstrap-builder sync \
        init-upstreams update-librespot update-gst-plugins-rs update-iris \
        integrate-librespot integrate-gst-plugins-rs integrate-iris \
        apply-patches save-patches

# Pull the latest committed code from GitLab before building.
# --ff-only refuses to proceed if local and remote have diverged, so you
# won't silently build a merged state you didn't intend.
sync:
	git pull --ff-only origin main

# Build for the Pi5 cluster's arm64 nodes. The Rust cross-compile stage
# runs natively (no emulation); only the final stage (apt/pip) needs the
# QEMU-backed builder.
build: sync
	docker buildx build --builder $(BUILDER) --platform linux/arm64 \
		--cache-from type=registry,ref=$(CACHE_REF) \
		--cache-to   type=registry,ref=$(CACHE_REF),mode=max \
		-t $(REGISTRY)/$(IMAGE):$(TAG) --load .

push: sync
	docker buildx build --builder $(BUILDER) --platform linux/arm64 \
		--cache-from type=registry,ref=$(CACHE_REF) \
		--cache-to   type=registry,ref=$(CACHE_REF),mode=max \
		-t $(REGISTRY)/$(IMAGE):$(TAG) --push .

# keel.sh is already watching the mopidy* deployments in namespace
# snapserver with policy=force/trigger=poll, so pushing :latest is enough
# to get it rolled out; this target is for forcing it immediately instead
# of waiting for keel's poll interval.
deploy:
	kubectl -n $(NAMESPACE) rollout restart deployment mopidy1 mopidy2 mopidy3 mopidy4

# ---------------------------------------------------------------------------
# Upstream vendor management
#
# Local modifications to vendor/ are stored as patch files in patches/:
#   patches/librespot/player.patch
#   patches/gst-plugins-rs/spotifylyricssrc-imp.patch
#
# Patches are applied with patch -p1 from inside the vendor directory, so
# paths in the .patch files are relative to the vendor root (e.g.
# playback/src/player.rs, not vendor/librespot/playback/src/player.rs).
#
# Full update workflow for one vendor:
#   make update-librespot          # 1. review new upstream commits + file diffs
#   make integrate-librespot       # 2. rsync upstream + apply patches
#   # resolve any patch conflicts, then:
#   echo vX.Y.Z > vendor/librespot/.upstream-ref
#   git add vendor/librespot patches/ && git commit -m "Update librespot to vX.Y.Z"
#   make save-patches              # 3. regenerate .patch files from new state
#   git add patches/ && git commit --amend --no-edit
# ---------------------------------------------------------------------------

# Clone upstream repos on first run; pull on subsequent runs.
init-upstreams:
	@if [ ! -d $(LIBRESPOT_UPSTREAM_DIR)/.git ]; then \
		echo "Cloning librespot upstream…"; \
		git clone $(LIBRESPOT_UPSTREAM_URL) $(LIBRESPOT_UPSTREAM_DIR); \
	else \
		echo "Fetching librespot upstream…"; \
		git -C $(LIBRESPOT_UPSTREAM_DIR) fetch --tags; \
	fi
	@if [ ! -d $(GST_UPSTREAM_DIR)/.git ]; then \
		echo "Cloning gst-plugins-rs upstream ($(GST_UPSTREAM_BRANCH))…"; \
		git clone -b $(GST_UPSTREAM_BRANCH) $(GST_UPSTREAM_URL) $(GST_UPSTREAM_DIR); \
	else \
		echo "Fetching gst-plugins-rs upstream…"; \
		git -C $(GST_UPSTREAM_DIR) fetch origin; \
	fi
	@if [ ! -d $(IRIS_UPSTREAM_DIR)/.git ]; then \
		echo "Cloning Iris fork…"; \
		git clone $(IRIS_UPSTREAM_URL) $(IRIS_UPSTREAM_DIR); \
	else \
		echo "Fetching Iris fork…"; \
		git -C $(IRIS_UPSTREAM_DIR) fetch origin; \
	fi

# Show commits added upstream to librespot since the vendored ref (tag or SHA).
update-librespot: init-upstreams
	@BASE=$$(cat vendor/librespot/.upstream-ref); \
	echo "=== New upstream librespot commits since $$BASE ==="; \
	git -C $(LIBRESPOT_UPSTREAM_DIR) log $$BASE..HEAD --oneline; \
	echo ""; \
	echo "=== Files that differ between vendor and upstream HEAD ==="; \
	diff -rq --exclude='.git' --exclude='target' \
		$(LIBRESPOT_UPSTREAM_DIR) vendor/librespot || true; \
	echo ""; \
	echo "Upstream HEAD: $$(git -C $(LIBRESPOT_UPSTREAM_DIR) rev-parse HEAD)"; \
	echo ""; \
	echo "To integrate: make integrate-librespot"

# Show commits added upstream to gst-plugins-rs since the vendored ref.
update-gst-plugins-rs: init-upstreams
	@BASE=$$(cat vendor/gst-plugins-rs/.upstream-ref); \
	echo "=== Recent upstream gst-plugins-rs commits (ref: $$BASE) ==="; \
	git -C $(GST_UPSTREAM_DIR) log $$BASE..HEAD --oneline 2>/dev/null || \
		git -C $(GST_UPSTREAM_DIR) log -20 --oneline; \
	echo ""; \
	echo "=== Files that differ between vendor and upstream HEAD ==="; \
	diff -rq --exclude='.git' --exclude='target' \
		$(GST_UPSTREAM_DIR) vendor/gst-plugins-rs || true; \
	echo ""; \
	echo "Upstream HEAD: $$(git -C $(GST_UPSTREAM_DIR) rev-parse HEAD)"; \
	echo ""; \
	echo "To integrate: make integrate-gst-plugins-rs"

# Show commits added on our Iris fork since the vendored ref.
update-iris: init-upstreams
	@BASE=$$(cat vendor/iris/.upstream-ref); \
	echo "=== New Iris fork commits since $$BASE ==="; \
	git -C $(IRIS_UPSTREAM_DIR) log $$BASE..HEAD --oneline; \
	echo ""; \
	echo "=== Files that differ between vendor and fork HEAD ==="; \
	diff -rq --exclude='.git' --exclude='node_modules' --exclude='.coverage' \
		$(IRIS_UPSTREAM_DIR) vendor/iris || true; \
	echo ""; \
	echo "Fork HEAD: $$(git -C $(IRIS_UPSTREAM_DIR) rev-parse HEAD)"; \
	echo ""; \
	echo "To integrate: make integrate-iris"

# Overwrite vendor/librespot with upstream HEAD, then re-apply local patches.
integrate-librespot: init-upstreams
	rsync -av --exclude='.git' --exclude='target' \
		$(LIBRESPOT_UPSTREAM_DIR)/ vendor/librespot/
	@echo ""; echo "=== Applying local patches ==="
	cd vendor/librespot && patch -p1 --fuzz=3 -i ../../patches/librespot/player.patch
	cd vendor/librespot && patch -p1 --fuzz=3 -i ../../patches/librespot/playback-Cargo.patch
	@echo ""; echo "Done. Next steps:"
	@echo "  echo <new-tag> > vendor/librespot/.upstream-ref"
	@echo "  git add vendor/librespot && git commit -m 'Update librespot to vX.Y'"
	@echo "  make save-patches && git add patches/ && git commit --amend --no-edit"

# Overwrite vendor/gst-plugins-rs with upstream HEAD, then re-apply local patches.
integrate-gst-plugins-rs: init-upstreams
	rsync -av --exclude='.git' --exclude='target' \
		$(GST_UPSTREAM_DIR)/ vendor/gst-plugins-rs/
	@echo ""; echo "=== Applying local patches ==="
	cd vendor/gst-plugins-rs && patch -p1 --fuzz=3 \
		-i ../../patches/gst-plugins-rs/spotifylyricssrc-imp.patch
	@echo ""; echo "Done. Next steps:"
	@echo "  git -C $(GST_UPSTREAM_DIR) rev-parse HEAD > vendor/gst-plugins-rs/.upstream-ref"
	@echo "  git add vendor/gst-plugins-rs && git commit -m 'Update gst-plugins-rs'"
	@echo "  make save-patches && git add patches/ && git commit --amend --no-edit"

# Overwrite vendor/iris with the fork's current HEAD. No patches to
# re-apply — it's our own fork, so changes are committed there directly.
integrate-iris: init-upstreams
	rsync -av --exclude='.git' --exclude='node_modules' --exclude='.coverage' \
		$(IRIS_UPSTREAM_DIR)/ vendor/iris/
	git -C $(IRIS_UPSTREAM_DIR) rev-parse HEAD > vendor/iris/.upstream-ref
	@echo ""; echo "Done. Next steps:"
	@echo "  git add vendor/iris && git commit -m 'Update Iris fork'"

# Apply all local patches to both vendor dirs (e.g. after a bare rsync).
apply-patches:
	cd vendor/librespot && patch -p1 --fuzz=3 -i ../../patches/librespot/player.patch
	cd vendor/librespot && patch -p1 --fuzz=3 -i ../../patches/librespot/playback-Cargo.patch
	cd vendor/gst-plugins-rs && patch -p1 --fuzz=3 \
		-i ../../patches/gst-plugins-rs/spotifylyricssrc-imp.patch

# Regenerate patch files from the current vendor state vs upstream HEAD.
# Run this after manually resolving patch conflicts following an integrate.
save-patches: init-upstreams
	@mkdir -p patches/librespot patches/gst-plugins-rs
	diff -u $(LIBRESPOT_UPSTREAM_DIR)/playback/src/player.rs \
	        vendor/librespot/playback/src/player.rs \
	  | sed 's|^--- .*|--- a/playback/src/player.rs|; s|^+++ .*|+++ b/playback/src/player.rs|' \
	  > patches/librespot/player.patch || true
	diff -u $(LIBRESPOT_UPSTREAM_DIR)/playback/Cargo.toml \
	        vendor/librespot/playback/Cargo.toml \
	  | sed 's|^--- .*|--- a/playback/Cargo.toml|; s|^+++ .*|+++ b/playback/Cargo.toml|' \
	  > patches/librespot/playback-Cargo.patch || true
	diff -u "$(GST_UPSTREAM_DIR)/audio/spotify/src/spotifylyricssrc/imp.rs" \
	        "vendor/gst-plugins-rs/audio/spotify/src/spotifylyricssrc/imp.rs" \
	  | sed 's|^--- .*|--- a/audio/spotify/src/spotifylyricssrc/imp.rs|; s|^+++ .*|+++ b/audio/spotify/src/spotifylyricssrc/imp.rs|' \
	  > patches/gst-plugins-rs/spotifylyricssrc-imp.patch || true
	@echo "Patches updated in patches/."

# ---------------------------------------------------------------------------
# One-off: create the buildx builder with QEMU arm64 emulation if it
# doesn't exist yet on a fresh machine.
bootstrap-builder:
	docker buildx create --name $(BUILDER) --driver docker-container || true
	docker buildx inspect --bootstrap $(BUILDER)
