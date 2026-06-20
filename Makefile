REGISTRY    ?= k8s1:31320
IMAGE       ?= mopidy_stable
TAG         ?= latest
BUILDER     ?= multi-arch
NAMESPACE   ?= snapserver

.PHONY: build push deploy bootstrap-builder

# Build for the Pi5 cluster's arm64 nodes. The Rust cross-compile stage
# runs natively (no emulation); only the final stage (apt/pip) needs the
# QEMU-backed builder.
build:
	docker buildx build --builder $(BUILDER) --platform linux/arm64 \
		-t $(REGISTRY)/$(IMAGE):$(TAG) --load .

push:
	docker buildx build --builder $(BUILDER) --platform linux/arm64 \
		-t $(REGISTRY)/$(IMAGE):$(TAG) --push .

# keel.sh is already watching the mopidy* deployments in namespace
# snapserver with policy=force/trigger=poll, so pushing :latest is enough
# to get it rolled out; this target is for forcing it immediately instead
# of waiting for keel's poll interval.
deploy:
	kubectl -n $(NAMESPACE) rollout restart deployment mopidy1 mopidy2 mopidy3 mopidy4

# One-off: create the buildx builder with QEMU arm64 emulation if it
# doesn't exist yet on a fresh machine.
bootstrap-builder:
	docker buildx create --name $(BUILDER) --driver docker-container || true
	docker buildx inspect --bootstrap $(BUILDER)
