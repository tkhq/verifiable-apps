out/.dev-loaded: out/dev/index.json
	cd out/dev && tar -cf - . | docker load
	touch out/.dev-loaded

define run
	$(MAKE) out/.dev-loaded; \
	docker run \
		--interactive \
		--tty \
		--user $(shell id -u):$(shell id -g) \
		--workdir /home/user \
		--volume .:/home/user \
		$(2) \
		tkhq/verifiable-apps/dev \
		/bin/bash -c "set -eu; $(1)"
endef

define build_context
$$( \
	mkdir -p out; \
	self=$(1); \
	for each in $$(find out/ -maxdepth 2 -name index.json); do \
    	package=$$(basename $$(dirname $${each})); \
    	if [ "$${package}" = "$${self}" ]; then continue; fi; \
    	printf -- ' --build-context %s=oci-layout://./out/%s' "$${package}" "$${package}"; \
	done; \
)
endef

,:=,
define build
	$(eval NAME := $(1))
	$(eval TYPE := $(if $(2),$(2),dir))
	$(eval REGISTRY := tkhq/verifiable-apps)
	$(eval PLATFORM := $(if $(3),$(3),linux/amd64))
	DOCKER_BUILDKIT=1 \
	SOURCE_DATE_EPOCH=1 \
	BUILDKIT_MULTIPLATFORM=1 \
	docker build \
		--build-arg VERSION=$(VERSION) \
		--tag $(REGISTRY)/$(NAME) \
		--progress=plain \
		--platform=$(PLATFORM) \
		--label "org.opencontainers.image.source=https://github.com/tkhq/mono" \
		$(if $(filter common,$(NAME)),,$(call build_context,$(1))) \
		$(if $(filter 1,$(NOCACHE)),--no-cache) \
		--output "\
			type=oci,\
			$(if $(filter dir,$(TYPE)),tar=false$(,)) \
			rewrite-timestamp=true,\
			force-compression=true,\
			name=$(NAME),\
			$(if $(filter tar,$(TYPE)),dest=$@") \
			$(if $(filter dir,$(TYPE)),dest=out/$(NAME)") \
		-f images/$(NAME)/Containerfile \
		.
endef
