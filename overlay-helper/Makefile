# Makefile for Overlay Helper

APP_NAME = GhostWriterOverlayHelper
BUNDLE_NAME = GhostWriterOverlayHelper.app
CONTENTS_DIR = $(BUNDLE_NAME)/Contents
MACOS_DIR = $(CONTENTS_DIR)/MacOS
RESOURCES_DIR = $(CONTENTS_DIR)/Resources

HUD_HTML = ../hud.html
SOURCE_FILES = main.m HUDPanel.m
FRAMEWORKS = -framework Cocoa -framework WebKit

# Compiler settings
CC = clang
CFLAGS = -fobjc-arc -mmacosx-version-min=10.13
LDFLAGS = -framework Cocoa -framework WebKit

all: $(BUNDLE_NAME)

$(BUNDLE_NAME): build_app copy_resources

build_app: mkdirs $(APP_NAME)

$(APP_NAME): $(SOURCE_FILES)
	$(CC) $(CFLAGS) -o $(APP_NAME) $(SOURCE_FILES) $(LDFLAGS)

mkdirs:
	@mkdir -p $(MACOS_DIR)
	@mkdir -p $(RESOURCES_DIR)

copy_resources: mkdirs
	cp $(APP_NAME) $(MACOS_DIR)/
	cp Info.plist $(CONTENTS_DIR)/
	cp $(HUD_HTML) $(RESOURCES_DIR)/hud.html
	@echo "Built $(BUNDLE_NAME)"

# Install into src-tauri so Tauri bundles it without ../ traversal
install: all
	@mkdir -p ../src-tauri/overlay-helper
	@rm -rf ../src-tauri/overlay-helper/$(BUNDLE_NAME)
	@cp -r $(BUNDLE_NAME) ../src-tauri/overlay-helper/
	@echo "Installed $(BUNDLE_NAME) to src-tauri/overlay-helper/"

clean:
	rm -rf $(BUNDLE_NAME) $(APP_NAME)

run: all
	./$(BUNDLE_NAME)/Contents/MacOS/$(APP_NAME)

.PHONY: all clean run build_app mkdirs copy_resources install
