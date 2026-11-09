.PHONY: app install uninstall pkg
app:
	python3 scripts/macos_app.py build
install:
	python3 scripts/macos_app.py install
uninstall:
	python3 scripts/macos_app.py uninstall
pkg:
	python3 scripts/macos_app.py package

.PHONY: test-audio
test-audio:
	python3 scripts/check_native_audio.py

.PHONY: check-architecture
check-architecture:
	python3 scripts/check_architecture.py

.PHONY: test-video test-menu test-native
test-video:
	python3 scripts/check_native_video.py
test-menu:
	python3 scripts/check_native_menu.py
test-native: test-audio test-video test-menu
