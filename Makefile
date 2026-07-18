REGISTRY    ?= k8s1:31320
IMAGE       ?= mopidy_stable
TAG         ?= latest
BUILDER     ?= multi-arch
NAMESPACE   ?= snapserver

# Upstream clone locations — kept outside this repo to avoid nested-git conflicts.
LIBRESPOT_UPSTREAM_DIR  ?= ../librespot-upstream
LIBRESPOT_UPSTREAM_URL  := https://github.com/librespot-org/librespot.git
GST_UPSTREAM_DIR        ?= ../gst-plugins-rs-upstream
GST_UPSTREAM_URL        := https://github.com/kingosticks/gst-plugins-rs.git
GST_UPSTREAM_BRANCH     := spotify-logging

.PHONY: build push deploy bootstrap-builder sync \
        init-upstreams update-librespot update-gst-plugins-rs

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
		-t $(REGISTRY)/$(IMAGE):$(TAG) --load .

push: sync
	docker buildx build --builder $(BUILDER) --platform linux/arm64 \
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
# Upstream clones live in sibling directories (outside this repo) to avoid
# the nested-git-repo problem that would arise if they lived inside vendor/.
# vendor/librespot/.upstream-ref and vendor/gst-plugins-rs/.upstream-ref
# record the tag / branch each vendor tree was last synced from.
#
# Typical update workflow:
#   make update-librespot          # see what changed upstream
#   # review the diff, then integrate manually:
#   rsync -av --exclude='.git' --exclude='target' \
#         $(LIBRESPOT_UPSTREAM_DIR)/ vendor/librespot/
#   # re-apply local patches (player.rs etc.), then:
#   echo v0.X.Y > vendor/librespot/.upstream-ref
#   git add vendor/librespot && git commit -m "Update librespot to vX.Y"
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
	echo "Upstream HEAD: $$(git -C $(LIBRESPOT_UPSTREAM_DIR) rev-parse HEAD)"

# Show commits added upstream to gst-plugins-rs since the vendored ref.
# .upstream-ref holds either a branch name (initial) or a commit SHA (after
# first update). After syncing, update the file with the new upstream SHA.
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
	echo "Upstream HEAD: $$(git -C $(GST_UPSTREAM_DIR) rev-parse HEAD)"

# ---------------------------------------------------------------------------
# One-off: create the buildx builder with QEMU arm64 emulation if it
# doesn't exist yet on a fresh machine.
bootstrap-builder:
	docker buildx create --name $(BUILDER) --driver docker-container || true
	docker buildx inspect --bootstrap $(BUILDER)
