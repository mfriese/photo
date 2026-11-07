.PHONY: app install uninstall pkg
app:
	python3 scripts/macos_app.py build
install:
	python3 scripts/macos_app.py install
uninstall:
	python3 scripts/macos_app.py uninstall
pkg:
	python3 scripts/macos_app.py package
