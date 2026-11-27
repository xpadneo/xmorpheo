# SPDX-License-Identifier: GPL-3.0-or-later
#
# Build as your user, install as root:
#
#   make
#   sudo make install
#
# PREFIX defaults to /usr/local, PREFIX=/opt/xmorpheo works too. The udev
# rule goes to /etc/udev/rules.d, because udev reads no rules below
# /usr/local or /opt; packages set UDEVRULESDIR=/usr/lib/udev/rules.d. No
# configuration is active after installing: copy or link an example into
# /etc/xmorpheo/conf.d. "make uninstall" removes everything "make install"
# created, and keeps the drop-in directories while they have files.
#
# "make site" builds the website into target/site, it needs python3 and
# mdbook, see site/AGENTS.md.

PREFIX ?= /usr/local
BINDIR ?= $(PREFIX)/bin
DATADIR ?= $(PREFIX)/share/xmorpheo
LIBDIR ?= $(PREFIX)/lib/xmorpheo
SYSCONFDIR ?= /etc
UDEVRULESDIR ?= /etc/udev/rules.d
# Packages set INSTALL_SYSCONFDIR=no, so nothing is installed below
# SYSCONFDIR; xmorpheo works without the directory.
INSTALL_SYSCONFDIR ?= yes
CARGO ?= cargo
# Building as root leaves files owned by root in the source tree and in the
# home of root. Set ALLOW_ROOT_BUILD=1 for package builds that run as root.
ALLOW_ROOT_BUILD ?=
BUILD_UID := $(shell id -u)

BIN := target/release/xmorpheo
DISTDIR ?= target/dist
STAMP := $(DISTDIR)/paths
# Every dist/<name>.in becomes $(DISTDIR)/<name>, with these paths put in
# for @BINDIR@ and so on.
PATHS := BINDIR=$(BINDIR) DATADIR=$(DATADIR) LIBDIR=$(LIBDIR) SYSCONFDIR=$(SYSCONFDIR)
TEMPLATES := $(wildcard dist/*.in)
GENERATED := $(patsubst dist/%.in,$(DISTDIR)/%,$(TEMPLATES))
# Where generated files go, by kind. A new kind needs a line in install.
GENERATED_RULES := $(filter %.rules,$(GENERATED))
CONFD := $(SYSCONFDIR)/xmorpheo/conf.d
INSTALL_CONFD := $(if $(filter yes,$(INSTALL_SYSCONFDIR)),$(CONFD))
EXAMPLES := $(wildcard examples/*.toml)
CHECKDIR ?= target/install-check
PYTHON ?= python3
MDBOOK ?= mdbook
SITEDIR ?= target/site

.PHONY: all build generate check-build install uninstall install-check install-check-package site FORCE

define no_root
	@if [ "$(BUILD_UID)" = 0 ] && [ -z "$(ALLOW_ROOT_BUILD)" ]; then \
		echo "do not build as root: run make as your user, then sudo make install" >&2; \
		echo "(set ALLOW_ROOT_BUILD=1 for package builds that run as root)" >&2; \
		exit 1; \
	fi
endef

all: build

build: generate
	$(no_root)
	XMORPHEO_SYSCONFDIR=$(SYSCONFDIR) XMORPHEO_LIBDIR=$(LIBDIR) $(CARGO) build --release --locked

generate: $(GENERATED)

# Rewritten only when the paths change, so generated files follow PREFIX.
$(STAMP): FORCE
	$(no_root)
	@mkdir -p $(DISTDIR)
	@if [ "$$(cat $@ 2>/dev/null)" != '$(PATHS)' ]; then echo '$(PATHS)' > $@; fi

$(DISTDIR)/%: dist/%.in $(STAMP)
	sed -e 's|@BINDIR@|$(BINDIR)|g' -e 's|@DATADIR@|$(DATADIR)|g' \
		-e 's|@LIBDIR@|$(LIBDIR)|g' -e 's|@SYSCONFDIR@|$(SYSCONFDIR)|g' $< > $@

# install only copies what the build made, so nothing is generated as root.
check-build:
	@for f in $(BIN) $(STAMP) $(GENERATED); do \
		if [ ! -e $$f ]; then \
			echo "the build is missing $$f, run make as your user first" >&2; \
			exit 1; \
		fi; \
	done
	@if [ "$$(cat $(STAMP))" != '$(PATHS)' ]; then \
		echo "the build used $$(cat $(STAMP)), not $(PATHS):" >&2; \
		echo "run make and make install with the same PREFIX" >&2; \
		exit 1; \
	fi

install: check-build
	install -Dm755 $(BIN) $(DESTDIR)$(BINDIR)/xmorpheo
	install -d $(DESTDIR)$(UDEVRULESDIR)
	install -m644 $(GENERATED_RULES) $(DESTDIR)$(UDEVRULESDIR)/
	install -d $(DESTDIR)$(DATADIR)/examples
	install -m644 $(EXAMPLES) $(DESTDIR)$(DATADIR)/examples/
	install -d $(DESTDIR)$(LIBDIR)/conf.d $(addprefix $(DESTDIR),$(INSTALL_CONFD))

uninstall:
	@if [ -z "$(DESTDIR)" ] && [ -x "$(BINDIR)/xmorpheo" ]; then \
		"$(BINDIR)/xmorpheo" detach --all || true; \
	fi
	rm -f $(DESTDIR)$(BINDIR)/xmorpheo
	rm -f $(addprefix $(DESTDIR)$(UDEVRULESDIR)/,$(notdir $(GENERATED_RULES)))
	rm -f $(addprefix $(DESTDIR)$(DATADIR)/examples/,$(notdir $(EXAMPLES)))
	-rmdir $(DESTDIR)$(DATADIR)/examples $(DESTDIR)$(DATADIR)
	@for dir in $(LIBDIR)/conf.d $(INSTALL_CONFD); do \
		if rmdir "$(DESTDIR)$$dir" 2>/dev/null; then \
			rmdir "$$(dirname "$(DESTDIR)$$dir")" 2>/dev/null || true; \
		elif [ -d "$(DESTDIR)$$dir" ]; then \
			echo "keeping $$dir, it has configuration files"; \
		fi; \
	done

# Installs into target/install-check, uninstalls again, and fails if
# anything is left behind. The check directory always stays below target.
CHECKTMP = target/$(notdir $(CHECKDIR))

install-check: check-build
	$(no_root)
	rm -rf $(CHECKTMP)
	$(MAKE) --no-print-directory install DESTDIR=$(CURDIR)/$(CHECKTMP)
	test -x $(CHECKTMP)$(BINDIR)/xmorpheo
	grep -q 'RUN+="$(BINDIR)/xmorpheo udev bind %k"' $(CHECKTMP)$(UDEVRULESDIR)/60-xmorpheo.rules
	! grep -rq '@[A-Z]*DIR@' $(CHECKTMP)
	test -d $(CHECKTMP)$(LIBDIR)/conf.d
	$(if $(INSTALL_CONFD),test -d $(CHECKTMP)$(CONFD),test ! -e $(CHECKTMP)$(CONFD))
	test -z "$$(find $(CHECKTMP)$(SYSCONFDIR)/xmorpheo -type f 2>/dev/null)"
	$(MAKE) --no-print-directory uninstall DESTDIR=$(CURDIR)/$(CHECKTMP)
	test -z "$$(find $(CHECKTMP) -type f)"
	test ! -e $(CHECKTMP)$(CONFD) && test ! -e $(CHECKTMP)$(LIBDIR)
	rm -rf $(CHECKTMP)
	@echo "install-check: ok"

# The same for a distribution package: everything below /usr, the udev rule
# in /usr/lib/udev/rules.d, and nothing at all below /etc.
PACKAGE_VARS := PREFIX=/usr UDEVRULESDIR=/usr/lib/udev/rules.d INSTALL_SYSCONFDIR=no \
	DISTDIR=target/dist-package CHECKDIR=target/install-check-package

install-check-package:
	$(no_root)
	$(MAKE) --no-print-directory generate $(PACKAGE_VARS)
	$(MAKE) --no-print-directory install DESTDIR=$(CURDIR)/target/install-check-package $(PACKAGE_VARS)
	test ! -e target/install-check-package/etc
	test -f target/install-check-package/usr/lib/udev/rules.d/60-xmorpheo.rules
	$(MAKE) --no-print-directory install-check $(PACKAGE_VARS)
	@echo "install-check-package: ok"

# The website, see site/AGENTS.md. Preview it with
# "python3 -m http.server -d target/site".
site:
	$(no_root)
	$(PYTHON) site/build.py --out $(SITEDIR) --mdbook $(MDBOOK)
